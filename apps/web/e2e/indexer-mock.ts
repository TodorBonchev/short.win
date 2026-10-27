/**
 * The indexer for the `chain` profile (FRONTEND.md §10.2): a Playwright-routed stand-in for
 * `services/indexer` that answers the app's REST requests (`page.route`) and WebSocket
 * (`page.routeWebSocket`) with OpenAPI-shaped responses (typed with `src/api/types.ts`) built from
 * the E2E validator's state at request time — markets, positions and lenders decoded with
 * `@shortwin/client`, venue guards and config from the chain, token labels from the seed. History
 * and analytics (events, liquidations, candles, fees) are empty: nothing in the E2E scenario has
 * them, and a real indexer serves them in the `full` profile.
 *
 * `mode: 'down'` makes every REST request fail (connection refused) and closes the WebSocket: the
 * app's RPC fallback (§6.5). `pushMarket` / `pushEvent` drive the live-update specs.
 */

import type { Page, Route, WebSocketRoute } from '@playwright/test';
import {
    getGlobalConfigDecoder,
    getLenderPositionDecoder,
    getMarketDecoder,
    getMarketVaultDecoder,
    getShortPositionDecoder,
    getVenueGuardDecoder,
    LENDER_POSITION_DISCRIMINATOR,
    type LenderPosition as LenderAccount,
    MARKET_DISCRIMINATOR,
    type Market,
    type MarketVault,
    podU128ToBigInt,
    SHORT_POSITION_DISCRIMINATOR,
    type ShortPosition,
} from '@shortwin/client';
import { type Address, getBase58Decoder, type ReadonlyUint8Array } from '@solana/kit';
import type {
    Page as ApiPage,
    CoverageStats,
    GlobalStats,
    IngestStatus,
    LenderDetail,
    LenderPosition,
    MarketCoverage,
    MarketDetail,
    MarketPools,
    MarketSummary,
    Position,
    PositionDetail,
    Problem,
    ProtocolConfig,
    SearchHit,
    TierName,
    VenueName,
    VenueStatus,
    WsChannel,
} from '../src/api/types.ts';
import { URLS } from './env.ts';
import { getAccount, getAccounts, PROGRAMS, pda, rpc } from './validator/chain.ts';
import { reserves } from './validator/mock-cpamm.ts';
import type { E2EState, MarketInfo } from './validator/state.ts';

const WAD = 10n ** 18n;
const VENUES: readonly VenueName[] = [
    'none',
    'pump_curve',
    'pump_migrating',
    'pump_swap',
    'raydium_v4',
    'raydium_cpmm',
    'mock',
];
const TIERS: readonly TierName[] = ['A', 'B', 'C', 'D'];
const GUARD_NAMES = ['pump', 'pump_amm', 'pump_fees', 'raydium_v4', 'raydium_cpmm', 'jupiter'] as const;
const GUARD_PROGRAMS = [
    PROGRAMS.pump,
    PROGRAMS.pumpAmm,
    PROGRAMS.pumpFees,
    PROGRAMS.raydiumV4,
    PROGRAMS.raydiumCpmm,
    PROGRAMS.jupiter,
];
/** SOL/USD the mock reports in `/v1/stats` (the E2E has no live oracle). */
export const MOCK_SOL_USD = 150;

const b58 = getBase58Decoder();

const venueName = (v: number): VenueName => VENUES[v] ?? 'unknown';
const tierName = (t: number): TierName => TIERS[t] ?? 'C';

interface MarketRow {
    readonly address: Address;
    readonly m: Market;
    readonly vault: MarketVault | null;
}

interface PositionRow {
    readonly address: Address;
    readonly p: ShortPosition;
}

interface LenderRow {
    readonly address: Address;
    readonly l: LenderAccount;
}

interface Snapshot {
    readonly slot: bigint;
    readonly time: number;
    readonly markets: readonly MarketRow[];
    readonly positions: readonly PositionRow[];
    readonly lenders: readonly LenderRow[];
}

async function programAccounts(discriminator: ReadonlyUint8Array): Promise<{ address: Address; data: Uint8Array }[]> {
    const res = await rpc()
        .getProgramAccounts(PROGRAMS.shortwin, {
            commitment: 'confirmed',
            encoding: 'base64',
            filters: [{ memcmp: { offset: 0n, bytes: b58.decode(discriminator) as never, encoding: 'base58' } }],
        })
        .send();
    return (res as unknown as { pubkey: Address; account: { data: [string, string] } }[]).map((r) => ({
        address: r.pubkey,
        data: new Uint8Array(Buffer.from(r.account.data[0], 'base64')),
    }));
}

function problem(status: number, code: Problem['code'], title: string): Problem {
    return { type: `https://short.win/problems/${code}`, title, status, code };
}

export type IndexerMode = 'mock' | 'down';

export class IndexerMock {
    private readonly labels = new Map<string, MarketInfo>();
    private snap: Promise<Snapshot> | null = null;
    private snapAt = 0;
    /** Positions seen open earlier: reported `closed` once their account is gone. */
    private readonly seenPositions = new Map<string, Position>();
    private readonly sockets = new Set<{ ws: WebSocketRoute; channels: Set<string>; timer: NodeJS.Timeout }>();
    private eventId = 1;
    /** Markets whose REST summary is pinned (`freezeMarket`): only a WebSocket push can change their row. */
    private readonly frozen = new Map<string, MarketSummary>();
    /** Every REST path the app requested (for specs that assert the app used the indexer). */
    readonly requests: string[] = [];

    readonly mode: IndexerMode;

    constructor(state: E2EState, mode: IndexerMode = 'mock') {
        this.mode = mode;
        for (const m of Object.values(state.markets)) this.labels.set(m.mint, m);
    }

    async install(page: Page): Promise<void> {
        await page.route(`${URLS.indexer}/**`, (route) => this.handle(route));
        await page.routeWebSocket(URLS.indexerWs, (ws) => this.socket(ws));
    }

    /** Stops the heartbeats of every routed socket (call at the end of a test). */
    dispose(): void {
        for (const s of this.sockets) clearInterval(s.timer);
        this.sockets.clear();
    }

    // ---- live --------------------------------------------------------------------------------

    /**
     * Pins `mint`'s summary in REST responses to its current value, so a later change can reach the
     * markets table only through the WebSocket (the live-row spec).
     */
    async freezeMarket(mint: Address): Promise<void> {
        this.snap = null;
        const s = await this.snapshot();
        const row = s.markets.find((r) => r.m.mint === mint);
        if (!row) throw new Error(`no market for ${mint}`);
        this.frozen.set(mint, this.summary(row));
    }

    /** Sends the market's current summary on `markets` and an event on `market:<mint>` (WS §6.4). */
    async pushMarket(mint: Address): Promise<void> {
        this.snap = null;
        const s = await this.snapshot();
        const row = s.markets.find((r) => r.m.mint === mint);
        if (!row) throw new Error(`no market for ${mint}`);
        const summary = this.summary(row);
        this.broadcast('markets', { type: 'market', channel: 'markets', data: summary });
        await this.pushEvent(`market:${mint}`, 'PriceCranked', row.address);
    }

    /** Sends an `event` message on `channel` (the app invalidates the channel's queries). */
    async pushEvent(channel: WsChannel, kind: string, market: Address | null): Promise<void> {
        const slot = Number(await rpc().getSlot({ commitment: 'confirmed' }).send());
        const id = this.eventId++;
        this.broadcast(channel, {
            type: 'event',
            channel,
            id,
            data: {
                id,
                kind,
                invocation: `${slot}:0:0`,
                source: 'cpi',
                market,
                data: {},
                signature: '1'.repeat(64),
                slot,
                tx_index: 0,
                ix_index: 0,
                ts: Math.floor(Date.now() / 1000),
                finalized: false,
            },
        });
    }

    private broadcast(channel: string, msg: unknown): void {
        for (const s of this.sockets) if (s.channels.has(channel)) s.ws.send(JSON.stringify(msg));
    }

    private socket(ws: WebSocketRoute): void {
        if (this.mode === 'down') {
            void ws.close({ code: 1011, reason: 'indexer down' });
            return;
        }
        const entry = {
            ws,
            channels: new Set<string>(),
            timer: setInterval(() => ws.send(JSON.stringify({ type: 'heartbeat' })), 15_000),
        };
        this.sockets.add(entry);
        ws.onMessage((raw) => {
            let msg: { op?: string; channels?: string[] };
            try {
                msg = JSON.parse(String(raw)) as typeof msg;
            } catch {
                ws.send(JSON.stringify({ type: 'error', code: 'bad_message', message: 'not JSON' }));
                return;
            }
            const channels = msg.channels ?? [];
            if (msg.op === 'subscribe') {
                for (const c of channels) entry.channels.add(c);
                ws.send(JSON.stringify({ type: 'subscribed', channels }));
            } else if (msg.op === 'unsubscribe') {
                for (const c of channels) entry.channels.delete(c);
                ws.send(JSON.stringify({ type: 'unsubscribed', channels }));
            } else if (msg.op === 'ping') {
                ws.send(JSON.stringify({ type: 'pong' }));
            }
        });
        ws.onClose(() => {
            clearInterval(entry.timer);
            this.sockets.delete(entry);
        });
    }

    // ---- REST --------------------------------------------------------------------------------

    private async handle(route: Route): Promise<void> {
        const req = route.request();
        if (this.mode === 'down') {
            await route.abort('connectionrefused');
            return;
        }
        const url = new URL(req.url());
        this.requests.push(`${url.pathname}${url.search}`);
        const cors = {
            'access-control-allow-origin': '*',
            'access-control-expose-headers': 'X-Indexer-Slot, X-Indexer-Finalized-Slot, Retry-After',
        };
        if (req.method() === 'OPTIONS') {
            await route.fulfill({ status: 204, headers: { ...cors, 'access-control-allow-headers': '*' } });
            return;
        }
        try {
            const { status, body } = await this.dispatch(url);
            const slot = (await this.snapshot()).slot;
            await route.fulfill({
                status,
                headers: {
                    ...cors,
                    'content-type': status >= 400 ? 'application/problem+json' : 'application/json',
                    'x-indexer-slot': slot.toString(),
                    'x-indexer-finalized-slot': (slot > 32n ? slot - 32n : 0n).toString(),
                },
                body: JSON.stringify(body),
            });
        } catch (e) {
            console.error('[indexer-mock]', url.pathname, e);
            await route.fulfill({
                status: 500,
                headers: { ...cors, 'content-type': 'application/problem+json' },
                body: JSON.stringify(problem(500, 'internal', String(e))),
            });
        }
    }

    private async dispatch(url: URL): Promise<{ status: number; body: unknown }> {
        const p = url.pathname.replace(/\/+$/, '');
        const q = url.searchParams;
        const ok = (body: unknown) => ({ status: 200, body });
        const notFound = () => ({ status: 404, body: problem(404, 'not_found', 'Not found') });
        const empty = (): ApiPage<never> => ({ data: [], next_cursor: null });
        const s = await this.snapshot();

        if (p === '/v1/health') return ok({ ok: true });
        if (p === '/v1/status') return ok(this.status(s));
        if (p === '/v1/stats') return ok(await this.stats(s));
        if (p === '/v1/search') return ok({ data: this.search(s, q.get('q') ?? '') });
        if (p === '/v1/markets') return ok(await this.marketList(s, q));
        if (p === '/v1/admin/config') return ok(await this.config(s));
        if (p === '/v1/venues') return ok(await this.venues());
        if (p === '/v1/liquidations' || p === '/v1/settlements' || p === '/v1/keepers') return ok(empty());
        if (p === '/v1/admin/changes' || p === '/v1/admin/events') return ok(empty());
        if (p === '/v1/stats/timeseries') return ok({ interval: q.get('interval') ?? '1h', data: [] });
        const mk = /^\/v1\/markets\/(\w+)(?:\/(\w+))?$/.exec(p);
        if (mk) {
            const m = mk;
            const row = s.markets.find((r) => r.m.mint === m[1]);
            if (!row) return notFound();
            switch (m[2]) {
                case undefined:
                    return ok(await this.detail(row, s));
                case 'positions':
                    return ok(
                        this.page(
                            this.positionsOf(s, (x) => x.market === row.address),
                            q,
                        ),
                    );
                case 'lenders': {
                    // `epoch` (FE-30): behind the market's share epoch, or at it; paged like the indexer.
                    const epoch = q.get('epoch');
                    const rows = this.lendersOf(s, row).filter(
                        (l) => epoch === null || l.epoch < row.m.shareEpoch === (epoch === 'behind'),
                    );
                    return ok(this.page(rows, q) satisfies ApiPage<LenderPosition>);
                }
                case 'pools':
                    return ok(this.pools(row));
                case 'coverage':
                    return ok({
                        mint: row.m.mint,
                        stats: this.coverage(row),
                        recent_gaps: [],
                        pauses: [],
                    } satisfies MarketCoverage);
                case 'candles':
                case 'stats':
                    return ok({ interval: q.get('interval') ?? '1h', data: [] });
                case 'fees':
                    return ok({ data: [] });
                default:
                    return ok(empty());
            }
        }
        const pos1 = /^\/v1\/positions\/(\w+)$/.exec(p);
        if (pos1) {
            const m = pos1;
            const pos = this.positionsOf(s, () => true, true).find((x) => x.position === m[1]);
            return pos ? ok({ position: pos, events: [], settlement: null } satisfies PositionDetail) : notFound();
        }
        const bor = /^\/v1\/borrowers\/(\w+)\/positions$/.exec(p);
        if (bor) {
            const m = bor;
            return ok(
                this.page(
                    this.positionsOf(s, (x) => x.borrower === m[1], true),
                    q,
                ),
            );
        }
        const len = /^\/v1\/lenders\/(\w+)$/.exec(p);
        if (len) {
            const m = len;
            const owner = m[1] as Address;
            const positions = s.markets.flatMap((row) => this.lendersOf(s, row).filter((l) => l.owner === owner));
            return ok({
                owner,
                positions,
                totals: {
                    markets: positions.length,
                    sol_paid: positions.reduce((a, l) => a + BigInt(l.sol_paid), 0n).toString(),
                },
            } satisfies LenderDetail);
        }
        if (/^\/v1\/lenders\/\w+\/events$/.test(p)) return ok(empty());
        const tx = /^\/v1\/tx\/(\w+)$/.exec(p);
        if (tx) {
            const m = tx;
            const t = await rpc()
                .getTransaction(m[1] as never, {
                    commitment: 'confirmed',
                    maxSupportedTransactionVersion: 0,
                    encoding: 'json',
                })
                .send();
            if (!t) return notFound();
            return ok({ signature: m[1], ok: t.meta?.err === null, err_code: null, events: [] });
        }
        return notFound();
    }

    // ---- snapshot ----------------------------------------------------------------------------

    private snapshot(): Promise<Snapshot> {
        if (this.snap && Date.now() - this.snapAt < 400) return this.snap;
        this.snapAt = Date.now();
        this.snap = (async () => {
            const [slot, marketAccts, positionAccts, lenderAccts] = await Promise.all([
                rpc().getSlot({ commitment: 'confirmed' }).send(),
                programAccounts(MARKET_DISCRIMINATOR),
                programAccounts(SHORT_POSITION_DISCRIMINATOR),
                programAccounts(LENDER_POSITION_DISCRIMINATOR),
            ]);
            const markets = marketAccts.map((a) => ({ address: a.address, m: getMarketDecoder().decode(a.data) }));
            const vaults = await getAccounts(markets.map((x) => x.m.marketVault));
            return {
                slot,
                time: Math.floor(Date.now() / 1000),
                markets: markets.map((x, i) => {
                    const v = vaults[i];
                    return { ...x, vault: v ? getMarketVaultDecoder().decode(v.data) : null };
                }),
                positions: positionAccts.map((a) => ({
                    address: a.address,
                    p: getShortPositionDecoder().decode(a.data),
                })),
                lenders: lenderAccts.map((a) => ({ address: a.address, l: getLenderPositionDecoder().decode(a.data) })),
            };
        })();
        return this.snap;
    }

    // ---- builders ----------------------------------------------------------------------------

    private status(s: Snapshot): IngestStatus {
        const slot = Number(s.slot);
        return {
            confirmed_slot: slot,
            finalized_slot: Math.max(0, slot - 32),
            latest_block_time: s.time,
            lag_s: 0,
            streams: [{ stream: 'shortwin', newest_slot: slot, backfill_complete: true }],
            rollups_complete_before: s.time - 3600,
            quarantined: 0,
            layout_warnings: 0,
            missing: 0,
            logs_truncated: 0,
        };
    }

    private flags(m: Market): MarketSummary['flags'] {
        return {
            paused_opens: (m.flags & 1) !== 0,
            mint_unsafe: (m.flags & 2) !== 0,
            route_broken: (m.flags & 4) !== 0,
            wipe_live: (m.flags & 32) !== 0,
            paused_coverage: (m.flags & 64) !== 0,
        };
    }

    private summary(row: MarketRow): MarketSummary {
        const m = row.m;
        const label = this.labels.get(m.mint);
        const ema = podU128ToBigInt(m.emaPrice);
        const inventory = m.tokensFree + m.borrowedQ;
        const inventoryValue = (inventory * ema) / WAD;
        const lenderSol = row.vault?.lenderSol ?? 0n;
        return {
            market: row.address,
            mint: m.mint,
            symbol: label?.symbol ?? null,
            name: label?.name ?? null,
            decimals: m.decimals,
            venue: venueName(m.venue),
            tier: tierName(m.tier),
            created_ts: Number(m.createdAt),
            flags: this.flags(m),
            tokens_free: m.tokensFree.toString(),
            borrowed_q: m.borrowedQ.toString(),
            inventory_tokens: inventory.toString(),
            util_bps: inventory > 0n ? Number((m.borrowedQ * 10_000n) / inventory) : 0,
            open_positions: m.openPositions,
            ema_price: ema > 0n ? ema.toString() : null,
            price_ts: m.emaTs > 0n ? Number(m.emaTs) : null,
            oi_lamports: m.totalNotionalOpen.toString(),
            tvl_lamports: (inventoryValue + lenderSol).toString(),
            tvl_usd: (Number(inventoryValue + lenderSol) / 1e9) * MOCK_SOL_USD,
        };
    }

    private async spotWad(row: MarketRow): Promise<bigint | null> {
        const info = this.labels.get(row.m.mint);
        if (!info?.pool) return null;
        const r = await reserves({
            pool: info.pool.pool,
            baseMint: info.mint,
            baseVault: info.pool.baseVault,
            quoteVault: info.pool.quoteVault,
            baseProgram: info.tokenProgram,
            authority: info.pool.authority,
        });
        return r.x > 0n ? (r.y * WAD) / r.x : null;
    }

    private coverage(row: MarketRow): CoverageStats {
        return {
            gaps: 0,
            gap_s_total: 0,
            gap_s_max: null,
            gaps_24h: 0,
            gap_s_24h: 0,
            coverage_pauses: this.flags(row.m).paused_coverage ? 1 : 0,
            coverage_paused_s: 0,
            paused_coverage: this.flags(row.m).paused_coverage,
            observations_24h: 0,
            failed_observations_24h: 0,
        };
    }

    private async detail(row: MarketRow, s: Snapshot): Promise<MarketDetail> {
        const m = row.m;
        const sum = this.summary(row);
        const lenders = this.lendersOf(s, row);
        const open = s.positions.filter((x) => x.p.market === row.address);
        const ema = podU128ToBigInt(m.emaPrice);
        const spot = await this.spotWad(row);
        const zero = '0';
        return {
            ...sum,
            creator: m.creator,
            coin_creator: null,
            token_program: m.tokenProgram,
            bonding_curve: m.bondingCurve,
            canonical_pool: m.canonicalPool,
            ray_pool: null,
            graduated_at: m.graduatedAt > 0n ? Number(m.graduatedAt) : null,
            total_shares: podU128ToBigInt(m.totalShares).toString(),
            share_epoch: m.shareEpoch,
            total_notional_open: m.totalNotionalOpen.toString(),
            open_vault_lamports: open.reduce((a, x) => a + x.p.v, 0n).toString(),
            lender_sol: (row.vault?.lenderSol ?? 0n).toString(),
            protocol_fees: (row.vault?.protocolFees ?? 0n).toString(),
            lender_count: lenders.length,
            lender_value_lamports: sum.tvl_lamports,
            inventory_value_lamports: (((m.tokensFree + m.borrowedQ) * ema) / WAD).toString(),
            spot_wad: spot?.toString() ?? null,
            route_chains: { no_route_since: Number(m.noRouteSince), pool_fail_since: Number(m.poolFailSince) },
            fees: [],
            exits: {
                positions: open.length,
                open: open.length,
                closes: 0,
                liquidations_cr: 0,
                liquidations_expired: 0,
                expiries: 0,
                settlements_s1: 0,
                settlements_s2: 0,
                settlements_s3: 0,
                exits: 0,
                avg_duration_s: null,
                close_rate_bps: null,
                liquidation_rate_bps: null,
                expiry_rate_bps: null,
                settlement_rate_bps: null,
                avg_open_impact_bps: null,
                avg_exit_slippage_bps: null,
                borrower_pnl: zero,
                borrower_pnl_known_exits: 0,
                borrower_pnl_unknown_exits: 0,
            },
            lender_loss: {
                settlements: 0,
                token_value: zero,
                lender_amount: zero,
                lender_pnl: zero,
                realised_lender_loss: zero,
                loss_events: 0,
                loss_bps_of_value: null,
                loss_bps_of_notional: null,
                realised_lender_loss_s1: zero,
                realised_lender_loss_s2: zero,
                realised_lender_loss_s3: zero,
            },
            creator_exposure: {
                coin_creator: null,
                coin_creator_shares: zero,
                coin_creator_share_bps: null,
                coin_creator_deposited_tokens: zero,
                creator_fee_income_lamports: zero,
                shortwin_volume_lamports: zero,
            },
            coverage: this.coverage(row),
            quarantined_events: 0,
            tier_terms: null,
        };
    }

    private async marketList(s: Snapshot, q: URLSearchParams): Promise<ApiPage<MarketSummary>> {
        let rows = s.markets.map((r) => this.frozen.get(r.m.mint) ?? this.summary(r));
        const text = (q.get('q') ?? '').trim().toLowerCase();
        if (text) {
            rows = rows.filter(
                (r) =>
                    r.mint.toLowerCase() === text ||
                    (r.symbol ?? '').toLowerCase().startsWith(text) ||
                    (r.name ?? '').toLowerCase().startsWith(text),
            );
        }
        const venue = q.get('venue');
        if (venue) rows = rows.filter((r) => r.venue === venue);
        const tier = q.get('tier');
        if (tier) rows = rows.filter((r) => r.tier === tier);
        if (q.get('has_positions') === 'true') rows = rows.filter((r) => r.open_positions > 0);
        const sort = q.get('sort') ?? 'tvl';
        const key = (r: MarketSummary): bigint =>
            sort === 'oi'
                ? BigInt(r.oi_lamports)
                : sort === 'util'
                  ? BigInt(r.util_bps)
                  : sort === 'created'
                    ? BigInt(r.created_ts)
                    : sort === 'open_positions'
                      ? BigInt(r.open_positions)
                      : BigInt(r.tvl_lamports);
        const asc = q.get('order') === 'asc';
        rows.sort((a, b) => {
            const d = key(a) - key(b);
            const c = d === 0n ? a.mint.localeCompare(b.mint) : d < 0n ? -1 : 1;
            return asc ? c : -c;
        });
        return this.page(rows, q);
    }

    /** Offset paging behind an opaque cursor (`o<offset>`), as the API's keyset cursors are opaque. */
    private page<T>(rows: readonly T[], q: URLSearchParams): ApiPage<T> {
        const limit = Math.min(Math.max(Number(q.get('limit') ?? 100) || 100, 1), 500);
        const cursor = q.get('cursor');
        const start = cursor?.startsWith('o') ? Number(cursor.slice(1)) || 0 : 0;
        const data = rows.slice(start, start + limit);
        return { data, next_cursor: start + limit < rows.length ? `o${start + limit}` : null };
    }

    private search(s: Snapshot, raw: string): SearchHit[] {
        const t = raw.trim().toLowerCase();
        if (!t) return [];
        const out: SearchHit[] = [];
        for (const r of s.markets) {
            const label = this.labels.get(r.m.mint);
            const symbol = label?.symbol ?? null;
            const name = label?.name ?? null;
            const match: SearchHit['match'] | null =
                r.m.mint.toLowerCase() === t
                    ? 'mint'
                    : symbol?.toLowerCase().startsWith(t)
                      ? 'symbol'
                      : name?.toLowerCase().startsWith(t)
                        ? 'name'
                        : null;
            if (match) out.push({ mint: r.m.mint, market: r.address, symbol, name, match });
        }
        return out.slice(0, 20);
    }

    private positionsOf(s: Snapshot, keep: (p: Position) => boolean, withClosed = false): Position[] {
        const byMarket = new Map(s.markets.map((r) => [r.address as string, r.m]));
        const open: Position[] = s.positions.map(({ address, p }) => {
            const market = byMarket.get(p.market);
            const pos: Position = {
                position: address,
                market: p.market,
                mint: market?.mint ?? p.market,
                borrower: p.borrower,
                nonce: p.nonce.toString(),
                status: 'open',
                tier: tierName(p.tier),
                venue: venueName(p.venueAtOpen),
                q: p.q.toString(),
                s: p.s.toString(),
                m: p.m.toString(),
                margin_added: '0',
                v: p.v.toString(),
                n_open: p.nOpen.toString(),
                fee_open: p.feeOpen.toString(),
                entry_price: p.entryPrice.toString(),
                deadline: Number(p.deadline),
                opened_ts: Number(p.tOpen),
                open_signature: '1'.repeat(64),
                armed: p.sArmed !== 0,
                exit: null,
            };
            this.seenPositions.set(address, pos);
            return pos;
        });
        const live = new Set(open.map((p) => p.position));
        const closed: Position[] = withClosed
            ? [...this.seenPositions.values()]
                  .filter((p) => !live.has(p.position))
                  .map((p) => ({
                      ...p,
                      status: 'closed',
                      exit: {
                          kind: 'Close',
                          ts: s.time,
                          signature: '1'.repeat(64),
                          keeper: null,
                          buyback_cost: null,
                          rent_paid: null,
                          penalty: null,
                          expiry_fee: null,
                          keeper_paid: null,
                          borrower_payout: '0',
                          settle_condition: null,
                          borrower_pnl: '0',
                          duration_s: Math.max(0, s.time - p.opened_ts),
                      },
                  }))
            : [];
        return [...open, ...closed].filter(keep);
    }

    private lendersOf(s: Snapshot, row: MarketRow): LenderPosition[] {
        const total = podU128ToBigInt(row.m.totalShares);
        return s.lenders
            .filter((x) => x.l.market === row.address && x.l.shares > 0n)
            .sort((a, b) => (a.l.shares === b.l.shares ? 0 : a.l.shares > b.l.shares ? -1 : 1))
            .map(({ l }) => ({
                market: row.address,
                mint: row.m.mint,
                owner: l.owner,
                shares: l.shares.toString(),
                share_bps: total > 0n ? Number((l.shares * 10_000n) / total) : null,
                epoch: l.epoch,
                lock: l.lock.toString(),
                lock_until: Number(l.lockUntil),
                deposited_tokens: '0',
                withdrawn_tokens: '0',
                sol_paid: l.claimed.toString(),
                pot_draw: '0',
                deficit_covered: '0',
                deposits: 1,
                withdrawals: 0,
                claims: 0,
                is_coin_creator: false,
                is_market_creator: l.owner === row.m.creator,
                first_ts: Number(row.m.createdAt),
                last_ts: s.time,
            }));
    }

    private pools(row: MarketRow): MarketPools {
        const info = this.labels.get(row.m.mint);
        const pool = info?.pool?.pool ?? null;
        return {
            mint: row.m.mint,
            current: pool,
            window: null,
            pools: pool
                ? [
                      {
                          pool,
                          venue: venueName(row.m.venue),
                          first_seen_ts: Number(row.m.createdAt),
                          last_d: podU128ToBigInt(row.m.poolD).toString(),
                          proposals: 0,
                          adopted_ts: Number(row.m.createdAt),
                          retired_ts: null,
                          is_current: true,
                      },
                  ]
                : [],
            events: [],
        };
    }

    private async stats(s: Snapshot): Promise<GlobalStats> {
        const sums = s.markets.map((r) => this.summary(r));
        const tvl = sums.reduce((a, r) => a + BigInt(r.tvl_lamports), 0n);
        const oi = sums.reduce((a, r) => a + BigInt(r.oi_lamports), 0n);
        const lenderSol = s.markets.reduce((a, r) => a + (r.vault?.lenderSol ?? 0n), 0n);
        const fees = s.markets.reduce((a, r) => a + (r.vault?.protocolFees ?? 0n), 0n);
        const owners = new Set(s.lenders.filter((x) => x.l.shares > 0n).map((x) => x.l.owner));
        const borrowers = new Set(s.positions.map((x) => x.p.borrower));
        const zero = '0';
        return {
            markets: s.markets.length,
            markets_with_positions: s.markets.filter((r) => r.m.openPositions > 0).length,
            tvl_lamports: tvl.toString(),
            tvl_usd: (Number(tvl) / 1e9) * MOCK_SOL_USD,
            oi_lamports: oi.toString(),
            notional_open_lamports: oi.toString(),
            open_positions: s.positions.length,
            lender_sol: lenderSol.toString(),
            protocol_fees_unswept: fees.toString(),
            protocol_revenue: zero,
            lender_fee_income: zero,
            keeper_income: zero,
            closes: 0,
            liquidations: 0,
            expiries: 0,
            settlements_s1: 0,
            settlements_s2: 0,
            settlements_s3: 0,
            realised_lender_loss: zero,
            settlement_lender_pnl: zero,
            unique_lenders: owners.size,
            unique_borrowers: borrowers.size,
            repeat_lenders: 0,
            repeat_borrowers: 0,
            avg_duration_s: null,
            coverage_gap_s: 0,
            creator_fee_income: zero,
            sol_usd: MOCK_SOL_USD,
            inventory_value_lamports: sums.reduce((a, r) => a + BigInt(r.tvl_lamports), 0n).toString(),
            util_bps: null,
            rental_fees: zero,
            avg_open_impact_bps: null,
            avg_exit_slippage_bps: null,
        };
    }

    private async globalConfig() {
        const a = await getAccount(await pda(PROGRAMS.shortwin, ['config']));
        if (!a) throw new Error('GlobalConfig missing');
        return getGlobalConfigDecoder().decode(a.data);
    }

    private async config(s: Snapshot): Promise<ProtocolConfig> {
        const c = await this.globalConfig();
        const big = (o: object) =>
            Object.fromEntries(Object.entries(o).map(([k, v]) => [k, typeof v === 'bigint' ? v.toString() : v]));
        return {
            as_of_slot: Number(s.slot),
            source: 'account',
            governance: c.governance,
            pause_authority: c.pauseAuthority,
            coverage_authority: c.coverageAuthority === PROGRAMS.system ? null : c.coverageAuthority,
            fee_recipient: c.feeRecipient,
            timelock_s: c.timelockS,
            venue_enabled: c.venueEnabled,
            paused_opens: (c.flags & 1) !== 0,
            route_broken_bits: c.venueRouteBroken,
            params: big(c.params),
            tiers: c.tiers.map(big),
            ema: big(c.emaParams),
            util_kink_bps: c.kink.utilKinkBps,
            pyth_sol_usd: c.pythSolUsd,
            pyth_feed_id: Buffer.from(c.pythFeedId).toString('hex'),
        };
    }

    private async venues(): Promise<VenueStatus> {
        const c = await this.globalConfig();
        const g = await getAccount(await pda(PROGRAMS.shortwin, ['venue_guard']));
        if (!g) throw new Error('VenueGuard missing');
        const guard = getVenueGuardDecoder().decode(g.data);
        const progData = await Promise.all(GUARD_PROGRAMS.map((p) => pda(PROGRAMS.upgradeableLoader, [p])));
        const pd = await getAccounts(progData);
        return {
            venue_enabled: c.venueEnabled,
            route_broken_bits: c.venueRouteBroken,
            paused_opens: (c.flags & 1) !== 0,
            guards: GUARD_NAMES.map((name, i) => {
                const acct = pd[i];
                const pdSlot =
                    acct && acct.data.length >= 12
                        ? new DataView(acct.data.buffer, acct.data.byteOffset).getBigUint64(4, true)
                        : (guard.guardSlot[i] ?? 0n);
                const flaggedAt = guard.flaggedAt[i] ?? 0n;
                return {
                    venue: i,
                    name,
                    upgraded: (guard.upgraded & (1 << i)) !== 0,
                    guard_slot: (guard.guardSlot[i] ?? 0n).toString(),
                    programdata_slot: pdSlot.toString(),
                    flagged_at: flaggedAt > 0n ? Number(flaggedAt) : null,
                    last_ts: Math.floor(Date.now() / 1000),
                };
            }),
            extras: [],
            admin_changes: [],
        };
    }
}
