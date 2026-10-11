/**
 * The lender dashboard (FRONTEND.md §3.8): one row per market with the chain-computed cells, the
 * share-epoch standing and the indexer's sync state, per-row claim / sync, and "claim all" as one
 * reviewed transaction per market in turn (a review closed without landing stops the queue).
 */
import type { LenderPosition, MarketVault } from '@shortwin/client';
import { type Address, address, createNoopSigner } from '@solana/kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
    createMemoryHistory,
    createRootRoute,
    createRoute,
    createRouter,
    RouterProvider,
} from '@tanstack/react-router';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LenderPosition as ApiLender, MarketDetail } from '../../api/types.ts';
import { ToastProvider } from '../../components/Toast.tsx';
import { config } from '../../config/env.ts';
import { T } from '../../config/testids.ts';
import { createAppClient } from '../../rpc/client.ts';
import { AppClientProvider } from '../../rpc/context.tsx';
import type { TxPlan, TxStage } from '../../tx/types.ts';
import type { TxFlow } from '../../tx/useTxFlow.ts';
import type { LendChain } from './chain.ts';
import type { LoadedChain } from './ctx.ts';
import { indexerStateOf, type LenderRow } from './dashboard/rows.ts';
import { LenderDashboardPage } from './LenderDashboardPage.tsx';
import type { ClaimView, PositionView } from './previews.ts';
import { fakeMarket, simpleLender, simplePool } from './testkit.ts';

const h = vi.hoisted(() => ({
    wallet: { current: null as unknown },
    rows: { current: null as unknown },
    started: [] as unknown[],
    setStage: { current: (_s: string) => {} },
    /** The math module the page sees (undefined: the fallback plan without syncs). */
    math: { current: undefined as unknown },
    /** `claimPlanFrom`'s calls while `math` is set: [market, otherLenders given, the row's pick reused]. */
    claims: [] as [string, boolean, boolean][],
    /** The rows' searches for syncs (`searchPotSyncs`): the gain per market (absent: none found). */
    gains: new Map<string, bigint>(),
    /** Searches wait for this (in flight) until the test's end releases them. */
    hold: { current: null as Promise<void> | null, release: () => {} },
    /** Searches running now, and the most at once. */
    running: { now: 0, max: 0 },
}));

vi.mock('../../wallet/useWallet.ts', () => ({ useWallet: () => h.wallet.current }));
vi.mock('../../math/useMath.ts', () => ({ useMath: () => ({ math: h.math.current, error: null }) }));
vi.mock('./build.ts', async (orig) => ({
    ...(await orig<typeof import('./build.ts')>()),
    // The panel's builder (Lend → Claim), recorded: the row's claim is the same call.
    claimPlanFrom: async (_env: unknown, c: LoadedChain, _now: number, others: unknown, pick?: unknown) => {
        h.claims.push([c.marketAddress, others !== null, pick !== undefined]);
        const plan: TxPlan = {
            kind: 'claim_sol',
            title: `Claim ${c.marketAddress}`,
            instructions: [],
            watch: [],
            disclosures: [],
            invalidate: [],
        };
        return plan;
    },
    // The rows' searches for syncs: a gain per market, or held in flight.
    searchPotSyncs: async (a: { chain: LoadedChain }) => {
        h.running.now++;
        h.running.max = Math.max(h.running.max, h.running.now);
        try {
            if (h.hold.current) await h.hold.current;
            await new Promise((r) => setTimeout(r, 5));
            const gain = h.gains.get(a.chain.marketAddress) ?? 0n;
            return { syncs: [], gain, more: { count: 0, gain: 0n } };
        } finally {
            h.running.now--;
        }
    },
}));
vi.mock('./dashboard/rows.ts', async (orig) => ({
    ...(await orig<typeof import('./dashboard/rows.ts')>()),
    useLenderRows: () => h.rows.current,
}));
vi.mock('../../tx/useTxFlow.ts', async (orig) => {
    const actual = await orig<typeof import('../../tx/useTxFlow.ts')>();
    const { useState } = await import('react');
    return {
        ...actual,
        // A flow whose stage the test drives (the page reacts to review → confirmed → idle).
        useTxFlow: (): TxFlow => {
            const [stage, setStage] = useState<TxStage>('idle');
            h.setStage.current = setStage as (s: string) => void;
            const plans = h.started as TxPlan[];
            return {
                state: {
                    stage,
                    plan: plans[plans.length - 1] ?? null,
                    report: null,
                    acks: new Set(),
                    speed: 'normal',
                    signature: null,
                    error: null,
                    refreshed: false,
                },
                start: (p: TxPlan) => {
                    plans.push(p);
                    setStage('review');
                },
                refresh: () => {},
                setAck: () => {},
                setSpeed: () => {},
                sign: () => {},
                close: () => setStage('idle'),
            };
        },
    };
});

const OWNER = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const M1 = address('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE');
const M2 = address('8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj');
const MINT1 = address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const MINT2 = address('JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN');
const M3 = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
const MINT3 = address('So11111111111111111111111111111111111111112');

const started = h.started as TxPlan[];
const drive = (stage: TxStage) => act(() => h.setStage.current(stage));

const lender = (o: Parameters<typeof simpleLender>[1] = {}): LenderPosition => ({
    ...simpleLender(10n ** 9n, o),
    discriminator: new Uint8Array(8),
    owner: OWNER,
    market: M1,
    bump: 255,
    fwdShares: 0n,
    fwdEpoch: 0,
    fwd: 0,
    fwdLag: 0,
    reserved: new Uint8Array(10),
});

function chain(mint: Address, market: Address, shareEpoch = 0): LoadedChain {
    return {
        mint,
        marketAddress: market,
        vaultAddress: market,
        market: fakeMarket({ shareEpoch }, { mint }),
        vault: simplePool().vault as unknown as MarketVault,
        config: null,
        mintInfo: null,
        owner: OWNER,
        lenderAddress: market,
        lender: lender(),
        records: [],
        recordsComplete: shareEpoch === 0,
        tokenAccounts: [],
        ata: null,
        walletLamports: 0n,
        slot: 1n,
        readAt: 0,
    } as LoadedChain;
}

const position = (claimable: bigint): PositionView => ({
    shares: 10n ** 9n,
    shareBps: 100,
    value: 1_000_000n,
    claimable,
    pending: claimable,
    vesting: 5n,
    lock: 0n,
    lockUntil: 0n,
    allowance: 1_000_000n,
    standing: { kind: 'current' },
});

/** The panel's claim view of a row: `claimable` now and the shortfall its syncs may add. */
const claimView = (claimable: bigint, shortfall = 0n): ClaimView => ({
    claimable,
    entitled: claimable + shortfall,
    vesting: 5n,
    pot: 0n,
    deficit: 0n,
    shortfall,
});

/** A row; its claim view follows its position's claimable unless given. */
const row = (o: Partial<LenderRow> & Pick<LenderRow, 'market' | 'mint'>): LenderRow => {
    const pos = o.position === undefined ? position(1_000n) : o.position;
    return {
        symbol: 'TKN',
        decimals: 6,
        chain: chain(o.mint, o.market),
        chainError: null,
        position: pos,
        claim: pos ? claimView(pos.claimable) : null,
        standing: { kind: 'current' },
        indexer: 'ok',
        isCoinCreator: false,
        isMarketCreator: false,
        emaPriceWad: null,
        ...o,
    };
};

class NoSocket {
    readyState = 0;
    send() {}
    close() {}
    addEventListener() {}
    removeEventListener() {}
}

beforeEach(() => {
    started.length = 0;
    h.claims.length = 0;
    h.math.current = undefined;
    h.gains.clear();
    h.hold.current = null;
    h.running.now = 0;
    h.running.max = 0;
    vi.stubGlobal('fetch', () => new Promise(() => {}));
    vi.stubGlobal('WebSocket', NoSocket);
    h.wallet.current = {
        ready: true,
        status: 'connected',
        connected: { address: OWNER, signer: createNoopSigner(OWNER), walletName: 'Test', supportsV0: true },
    };
    h.rows.current = { rows: [], loading: false, indexerDown: false, error: null };
});

afterEach(() => {
    // Held searches end (the app-wide search slots are module state, shared by the tests).
    h.hold.release();
    vi.unstubAllGlobals();
});

async function renderPage() {
    const rootRoute = createRootRoute();
    const lend = createRoute({ getParentRoute: () => rootRoute, path: '/lend', component: LenderDashboardPage });
    const market = createRoute({ getParentRoute: () => rootRoute, path: '/m/$mint', component: () => null });
    const router = createRouter({
        routeTree: rootRoute.addChildren([lend, market]),
        history: createMemoryHistory({ initialEntries: ['/lend'] }),
    });
    const qc = new QueryClient();
    const tree = () => (
        <AppClientProvider client={createAppClient(config)}>
            <QueryClientProvider client={qc}>
                <ToastProvider>
                    <RouterProvider router={router as never} />
                </ToastProvider>
            </QueryClientProvider>
        </AppClientProvider>
    );
    let r: ReturnType<typeof render> | undefined;
    await act(async () => {
        r = render(tree());
        await router.load();
    });
    return r;
}

describe('indexerStateOf', () => {
    const api = { shares: '1000000000', epoch: 0 } as ApiLender;
    const c: LendChain = chain(MINT1, M1);
    it('compares the indexer’s row with the chain', () => {
        expect(indexerStateOf(null, c, undefined)).toBe('missing');
        expect(indexerStateOf(api, c, undefined)).toBe('ok');
        expect(indexerStateOf({ ...api, shares: '5' }, c, undefined)).toBe('unsynced');
        expect(indexerStateOf({ ...api, epoch: 3 }, c, undefined)).toBe('unsynced');
        expect(indexerStateOf(api, c, { quarantined_events: 2 } as MarketDetail)).toBe('quarantined');
    });
});

describe('LenderDashboardPage', () => {
    it('keeps the page id and asks for a wallet', async () => {
        h.wallet.current = { ready: true, status: 'disconnected', connected: null };
        await renderPage();
        expect(screen.getByTestId(T.page('lend'))).toBeTruthy();
        expect(screen.getByTestId(T.lenderEmpty)).toBeTruthy();
    });

    it('renders a row per market with the chain cells, the epoch standing and the indexer state', async () => {
        h.rows.current = {
            rows: [
                row({ market: M1, mint: MINT1, isCoinCreator: true }),
                row({
                    market: M2,
                    mint: MINT2,
                    chain: chain(MINT2, M2, 30),
                    standing: { kind: 'behind', epochs: 30 },
                    indexer: 'quarantined',
                }),
            ],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        const r1 = screen.getByTestId(T.lenderRow(M1));
        expect(within(r1).getByTestId(T.lenderCell('shares')).dataset.value).toBe(String(10n ** 9n));
        expect(within(r1).getByTestId(T.lenderCell('value')).dataset.value).toBe('1000000');
        expect(within(r1).getByTestId(T.lenderCell('claimable')).dataset.value).toBe('1000');
        expect(within(r1).getByTestId(T.lenderCell('vesting')).dataset.value).toBe('5');
        expect(within(r1).getByTestId(T.lenderCell('creator')).dataset.value).toBe('true');
        expect(within(r1).getByTestId(T.lenderEpochs(M1)).dataset.state).toBe('current');
        expect(within(r1).getByTestId(T.lenderIndexer(M1)).dataset.state).toBe('ok');
        const r2 = screen.getByTestId(T.lenderRow(M2));
        expect(within(r2).getByTestId(T.lenderEpochs(M2)).dataset.state).toBe('behind');
        expect(within(r2).getByTestId(T.lenderEpochs(M2)).dataset.value).toBe('30');
        expect(within(r2).getByTestId(T.lenderIndexer(M2)).dataset.state).toBe('quarantined');
        fireEvent.click(within(r2).getByTestId(T.lenderSync(M2)));
        await waitFor(() => expect(started.map((p) => p.kind)).toEqual(['sync_lender']));
    });

    it('claims one market per review, in turn, and stops when a review is closed without landing', async () => {
        h.rows.current = {
            rows: [row({ market: M1, mint: MINT1 }), row({ market: M2, mint: MINT2, position: position(2_000n) })],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        fireEvent.click(screen.getByTestId(T.lenderClaimAll));
        await waitFor(() => expect(started).toHaveLength(1));
        expect(started[0]?.kind).toBe('claim_sol');
        // The first review lands and is closed: the second opens.
        drive('confirmed');
        drive('idle');
        await waitFor(() => expect(started).toHaveLength(2));
        expect(started[1]?.title).toContain('0.000002');
        // The queue is empty now: closing the second review starts nothing.
        drive('confirmed');
        drive('idle');
        expect(started).toHaveLength(2);
    });

    it('stops "claim all" when a review is closed without landing', async () => {
        h.rows.current = {
            rows: [row({ market: M1, mint: MINT1 }), row({ market: M2, mint: MINT2 })],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        fireEvent.click(screen.getByTestId(T.lenderClaimAll));
        await waitFor(() => expect(started).toHaveLength(1));
        drive('idle');
        drive('confirmed');
        drive('idle');
        expect(started).toHaveLength(1);
    });

    it('shows the empty state', async () => {
        await renderPage();
        expect(screen.getByTestId(T.lenderEmpty).textContent).toContain('You do not lend in any market yet');
    });
});

describe('LenderDashboardPage on any width (M10-RESP, FRONTEND.md §3.8, §7.6)', () => {
    it('without a wallet: Connect wallet, no KPI strip and no table', async () => {
        h.wallet.current = { ready: true, status: 'disconnected', connected: null };
        await renderPage();
        expect(screen.getByTestId(T.connectCta('lend-dashboard'))).toBeTruthy();
        expect(screen.queryByTestId(T.lenderDashboard)).toBeNull();
        expect(screen.queryByTestId(T.lenderClaimAll)).toBeNull();
    });

    it('empty: Browse markets, and no Claim all', async () => {
        await renderPage();
        expect(screen.getByTestId(T.lendEmptyCta).getAttribute('href')).toBe('/markets');
        expect(screen.queryByTestId(T.lenderClaimAll)).toBeNull();
    });

    it('the KPIs: markets, the value at the EMA price, SOL claimable and the rent credited', async () => {
        h.rows.current = {
            rows: [
                // 1,000,000 raw units at 2 lamports per raw unit; the second has no price.
                row({ market: M1, mint: MINT1, emaPriceWad: 2n * 10n ** 18n }),
                row({ market: M2, mint: MINT2, position: position(2_000n) }),
            ],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        expect(screen.getByTestId(T.lenderKpi('markets')).dataset.value).toBe('2');
        const value = screen.getByTestId(T.lenderKpi('value'));
        expect(value.dataset.value).toBe('2000000');
        expect(value.dataset.priced).toBe('1');
        expect(value.textContent).toContain('1 of 2 priced');
        expect(screen.getByTestId(T.lenderKpi('claimable')).dataset.value).toBe('3000');
        expect(screen.getByTestId(T.lenderKpi('credited')).dataset.value).toBe('3000');
        expect(screen.queryByTestId(T.lenderSource)).toBeNull();
    });

    it('a row links into the market’s lend panel (the trade sheet under 1024 px) and says why Claim is off', async () => {
        h.rows.current = {
            rows: [row({ market: M1, mint: MINT1, position: position(0n) })],
            loading: false,
            indexerDown: true,
            error: null,
        };
        await renderPage();
        const r1 = screen.getByTestId(T.lenderRow(M1));
        const withdraw = within(r1).getByTestId(T.lenderWithdraw(M1)).getAttribute('href') ?? '';
        expect(withdraw).toContain(`/m/${MINT1}`);
        expect(withdraw).toMatch(/action=withdraw/);
        expect(withdraw).toMatch(/sheet=trade/);
        expect(within(r1).getByTestId(T.lenderDeposit(M1)).getAttribute('href')).toMatch(/action=deposit/);
        const claim = within(r1).getByTestId(T.lenderClaim(M1)) as HTMLButtonElement;
        expect(claim.disabled).toBe(true);
        const reason = within(r1).getByTestId(T.lenderClaimReason(M1));
        expect(reason.textContent).toBe('Nothing to claim yet');
        expect(claim.getAttribute('aria-describedby')).toBe(reason.id);
        // Nothing claimable anywhere: no Claim all; the indexer is down: the data source says so.
        expect(screen.queryByTestId(T.lenderClaimAll)).toBeNull();
        expect(screen.getByTestId(T.lenderSource).textContent).toContain('Chain only');
    });

    it('one "Sync" cell: quiet while current, a badge when behind, wiped or the indexer disagrees', async () => {
        h.rows.current = {
            rows: [
                row({ market: M1, mint: MINT1 }),
                row({ market: M2, mint: MINT2, standing: { kind: 'parked' }, indexer: 'unsynced' }),
            ],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        const r1 = screen.getByTestId(T.lenderRow(M1));
        expect(r1.querySelector('td[data-col="sync"] .badge')).toBeNull();
        const r2 = screen.getByTestId(T.lenderRow(M2));
        const badges = Array.from(r2.querySelectorAll('td[data-col="sync"] .badge')).map((b) => b.textContent);
        expect(badges).toEqual(['Pool wiped out', 'Indexer behind']);
        // The wiped / indexer cell is never a priority-3 (hidden) column.
        expect(r2.querySelector('td[data-col="sync"]')?.getAttribute('data-priority')).toBe('2');
    });

    it('Claim all: in an action bar, with the total', async () => {
        h.rows.current = {
            rows: [row({ market: M1, mint: MINT1 })],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        const all = screen.getByTestId(T.lenderClaimAll);
        expect(all.closest('.action-bar')).toBeTruthy();
        expect(all.textContent).toBe('Claim all · 0.000001SOL');
    });
});

describe('LenderDashboardPage: a row claims like Lend → Claim (the owner’s decision of 2026-10-11, D-21)', () => {
    it('labels each row’s Claim with the panel’s words, and opens it on a shortfall the syncs can pay', async () => {
        h.math.current = {};
        h.gains.set(M2, 10n ** 9n);
        h.rows.current = {
            rows: [
                // Claimable now, nothing out of reach: one line.
                row({ market: M1, mint: MINT1 }),
                // Nothing claimable now, 25,000 lamports the syncs can add: enabled, two lines.
                row({ market: M2, mint: MINT2, position: position(0n), claim: claimView(0n, 25_000n) }),
            ],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        const c1 = screen.getByTestId(T.lenderClaim(M1)) as HTMLButtonElement;
        expect(c1.textContent).toBe('Review claim · 0.000001 SOL');
        expect(c1.dataset.lines).toBeUndefined();
        expect(c1.disabled).toBe(false);
        const r2 = screen.getByTestId(T.lenderRow(M2));
        const c2 = within(r2).getByTestId(T.lenderClaim(M2)) as HTMLButtonElement;
        await waitFor(() => expect(c2.textContent).toBe('Review claim · 0 SOL now · syncs may add up to 0.000025 SOL'));
        expect(within(r2).getByTestId(T.claimSubmitSyncs).textContent).toBe('syncs may add up to 0.000025 SOL');
        expect(c2.dataset.lines).toBe('2');
        expect(c2.disabled).toBe(false);
        expect(within(r2).queryByTestId(T.lenderClaimReason(M2))).toBeNull();
        // "Claim all" keeps its label and counts both rows; its total is what is claimable now.
        expect(screen.getByTestId(T.lenderClaimAll).textContent).toBe('Claim all (2 markets) · 0.000001SOL');
    });

    it('only when syncs can pay (owner, 2026-10-11): {g} = min(shortfall, gain); none, dust or in flight is off', async () => {
        h.math.current = {};
        const M4 = address('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
        const MINT4 = address('Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB');
        // M1: no sync funds it (the only lender); M2: 9,999 lamports (dust); M3: 12,000 of 25,000;
        // M4: 1 SOL, more than its 25,000-lamport shortfall.
        h.gains.set(M2, 9_999n);
        h.gains.set(M3, 12_000n);
        h.gains.set(M4, 10n ** 9n);
        const short = (market: Address, mint: Address) =>
            row({
                market,
                mint,
                chain: chain(mint, market),
                position: position(0n),
                claim: claimView(0n, 25_000n),
            });
        h.rows.current = {
            rows: [short(M1, MINT1), short(M2, MINT2), short(M3, MINT3), short(M4, MINT4)],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        const claim = (m: Address) => screen.getByTestId(T.lenderClaim(m)) as HTMLButtonElement;
        await waitFor(() => expect(claim(M4).disabled).toBe(false));
        for (const m of [M1, M2]) {
            await waitFor(() =>
                expect(screen.getByTestId(T.lenderClaimReason(m)).textContent).toBe('Nothing to claim yet'),
            );
            expect(claim(m).textContent).toBe('Review claim · 0 SOL');
            expect(claim(m).disabled).toBe(true);
            expect(claim(m).dataset.lines).toBeUndefined();
        }
        expect(claim(M3).textContent).toBe('Review claim · 0 SOL now · syncs may add up to 0.000012 SOL');
        expect(claim(M4).textContent).toBe('Review claim · 0 SOL now · syncs may add up to 0.000025 SOL');
        // Claim all counts, totals and claims exactly the enabled rows: 0 SOL claimable now.
        expect(screen.getByTestId(T.lenderClaimAll).textContent).toBe('Claim all (2 markets) · 0SOL');
        expect(screen.getByTestId(T.lenderKpi('claimable')).dataset.value).toBe('0');
        fireEvent.click(screen.getByTestId(T.lenderClaimAll));
        await waitFor(() => expect(h.claims).toHaveLength(1));
        drive('confirmed');
        drive('idle');
        await waitFor(() => expect(h.claims.map(([m]) => m)).toEqual([M3, M4]));
        // Each claim took its row's fresh pick.
        expect(h.claims.every(([, others, reused]) => others && reused)).toBe(true);
        // At most two searches at once (a lender in many markets: no request storm).
        expect(h.running.max).toBeLessThanOrEqual(2);
    });

    it('while a row’s search runs: busy and off without a reason, not in Claim all; a claimable row stays on', async () => {
        h.math.current = {};
        h.hold.current = new Promise<void>((r) => {
            h.hold.release = r;
        });
        h.rows.current = {
            rows: [
                row({ market: M1, mint: MINT1, position: position(0n), claim: claimView(0n, 25_000n) }),
                row({ market: M2, mint: MINT2, position: position(2_000n), claim: claimView(2_000n, 25_000n) }),
            ],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        const c1 = screen.getByTestId(T.lenderClaim(M1)) as HTMLButtonElement;
        expect(c1.textContent).toBe('Review claim · 0 SOL');
        expect(c1.disabled).toBe(true);
        expect(c1.getAttribute('aria-busy')).toBe('true');
        expect(screen.queryByTestId(T.lenderClaimReason(M1))).toBeNull();
        const c2 = screen.getByTestId(T.lenderClaim(M2)) as HTMLButtonElement;
        expect(c2.textContent).toBe('Review claim · 0.000002 SOL');
        expect(c2.disabled).toBe(false);
        expect(c2.getAttribute('aria-busy')).toBeNull();
        expect(screen.getByTestId(T.lenderClaimAll).textContent).toBe('Claim all · 0.000002SOL');
    });

    it('with the indexer down, no row searches for syncs: a shortfall row is off at once, with its reason', async () => {
        h.math.current = {};
        h.gains.set(M1, 25_000n);
        h.rows.current = {
            rows: [row({ market: M1, mint: MINT1, position: position(0n), claim: claimView(0n, 25_000n) })],
            loading: false,
            indexerDown: true,
            error: null,
        };
        await renderPage();
        const claim = screen.getByTestId(T.lenderClaim(M1)) as HTMLButtonElement;
        expect(claim.textContent).toBe('Review claim · 0 SOL');
        expect(claim.disabled).toBe(true);
        expect(claim.getAttribute('aria-busy')).toBeNull();
        expect(screen.getByTestId(T.lenderClaimReason(M1)).textContent).toBe('Nothing to claim yet');
        await act(() => new Promise((r) => setTimeout(r, 30)));
        expect(h.running.max).toBe(0);
        expect(claim.disabled).toBe(true);
    });

    it('a row with nothing claimable and no shortfall (a dust one counts as none) is off, as in the panel', async () => {
        h.rows.current = {
            rows: [row({ market: M1, mint: MINT1, position: position(0n), claim: claimView(0n, 0n) })],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        const claim = screen.getByTestId(T.lenderClaim(M1)) as HTMLButtonElement;
        expect(claim.textContent).toBe('Review claim · 0 SOL');
        expect(claim.disabled).toBe(true);
        expect(screen.getByTestId(T.lenderClaimReason(M1)).textContent).toBe('Nothing to claim yet');
        expect(screen.queryByTestId(T.lenderClaimAll)).toBeNull();
    });

    it('Claim all counts every row whose Claim is enabled: one behind the share epoch with its records read too', async () => {
        h.rows.current = {
            rows: [
                row({ market: M1, mint: MINT1 }),
                // Behind, every record read: its own Claim is enabled (the claim walks the records).
                row({
                    market: M2,
                    mint: MINT2,
                    position: position(2_000n),
                    chain: { ...chain(MINT2, M2, 3), recordsComplete: true } as LoadedChain,
                    standing: { kind: 'behind', epochs: 3 },
                }),
                // Behind with records still to read: Sync, not Claim, and not in Claim all.
                row({
                    market: M3,
                    mint: MINT3,
                    position: position(4_000n),
                    chain: chain(MINT3, M3, 3),
                    standing: { kind: 'behind', epochs: 3 },
                }),
            ],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        const c2 = screen.getByTestId(T.lenderClaim(M2)) as HTMLButtonElement;
        expect(c2.disabled).toBe(false);
        expect(c2.textContent).toBe('Review claim · 0.000002 SOL');
        expect(screen.queryByTestId(T.lenderClaim(M3))).toBeNull();
        expect(screen.getByTestId(T.lenderSync(M3))).toBeTruthy();
        expect(screen.getByTestId(T.lenderClaimAll).textContent).toBe('Claim all (2 markets) · 0.000003SOL');
        expect(screen.getByTestId(T.lenderKpi('claimable')).dataset.value).toBe('3000');
    });

    it('builds the panel’s claim (with the indexer’s lenders for the syncs), per row and for Claim all', async () => {
        h.math.current = {};
        h.gains.set(M1, 10n ** 9n);
        h.rows.current = {
            rows: [
                row({ market: M1, mint: MINT1, position: position(0n), claim: claimView(0n, 25_000n) }),
                row({ market: M2, mint: MINT2 }),
            ],
            loading: false,
            indexerDown: false,
            error: null,
        };
        await renderPage();
        await waitFor(() => expect((screen.getByTestId(T.lenderClaim(M1)) as HTMLButtonElement).disabled).toBe(false));
        fireEvent.click(screen.getByTestId(T.lenderClaim(M1)));
        await waitFor(() => expect(h.claims).toEqual([[M1, true, true]]));
        drive('idle');
        fireEvent.click(screen.getByTestId(T.lenderClaimAll));
        await waitFor(() => expect(h.claims).toHaveLength(2));
        drive('confirmed');
        drive('idle');
        await waitFor(() =>
            // The shortfall row reuses its search's pick; the claimable one has none to reuse.
            expect(h.claims).toEqual([
                [M1, true, true],
                [M1, true, true],
                [M2, true, false],
            ]),
        );
    });
});
