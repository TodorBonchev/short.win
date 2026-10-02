/**
 * The E2E harness itself (`@scaffold`): the seed's scenario is on chain as the specs expect it, the
 * routed indexer serves it in the OpenAPI shapes the app reads, and the Node-side actions the race
 * and price specs use (CPI Guard, required memos, price moves, a swept donation) do what they say.
 * These run before any feature page exists, so a harness defect is not mistaken for a track's.
 */
import { getMarketDecoder } from '@shortwin/client';
import type {
    LenderDetail,
    MarketDetail,
    MarketSummary,
    Page,
    Position,
    ProtocolConfig,
    VenueStatus,
} from '../../src/api/types.ts';
import { URLS } from '../env.ts';
import { expect, need, requireTrading, test } from '../fixtures.ts';
import {
    donateAndSweep,
    landedTransaction,
    movePrice,
    setCpiGuard,
    setRequireMemos,
    shortPosition,
    tokenExtensions,
} from '../validator/actions.ts';
import { getAccount, LAMPORTS_PER_SOL, PROGRAMS } from '../validator/chain.ts';
import { derivePool, reserves } from '../validator/mock-cpamm.ts';

test.describe('E2E harness', { tag: '@scaffold' }, () => {
    test.use({ wallet: null });

    test('the seeded scenario is on chain', async ({ state }) => {
        requireTrading(state);
        const positions = need(state.positions, 'seeded positions');
        for (const p of [positions.margin, positions.closer, positions.holder]) {
            const acct = need(await shortPosition(p.position), `position ${p.position}`);
            expect(acct.q).toBe(BigInt(p.q));
            expect(acct.borrower).toBe(p.borrower);
        }
        const g = state.tokenAccounts.guard;
        expect((await tokenExtensions(g.guarded)).lockCpi).toBe(true);
        expect((await tokenExtensions(g.race)).lockCpi).toBe(false);
        expect((await tokenExtensions(state.tokenAccounts.memo.ata)).requireMemos).toBe(true);
        // `memoRace`'s ATA has room for MemoTransfer (reallocated) but no requirement: absent or off.
        expect((await tokenExtensions(state.tokenAccounts.memoRace.ata)).requireMemos).not.toBe(true);
        const paused = need(await getAccount(state.markets.paused.market), 'paused market');
        expect(getMarketDecoder().decode(paused.data).flags & 64).toBe(64);
        for (const mint of Object.values(state.createMints)) expect(await getAccount(mint)).not.toBeNull();
    });

    test('the routed indexer serves the chain in the API shapes', async ({ app, state, page, indexerMock }) => {
        test.skip(!indexerMock, 'checks the routed indexer (profile chain)');
        await app.goto('/markets', 'markets');
        const get = async <T>(path: string): Promise<{ status: number; body: T; slot: string | null }> =>
            page.evaluate(async (u) => {
                const r = await fetch(u);
                return { status: r.status, body: await r.json(), slot: r.headers.get('x-indexer-slot') };
            }, `${URLS.indexer}${path}`);

        const list = await get<Page<MarketSummary>>('/v1/markets?sort=tvl&limit=100');
        expect(list.status).toBe(200);
        expect(Number(list.slot)).toBeGreaterThan(0);
        const byMint = new Map(list.body.data.map((m) => [m.mint, m]));
        const { main, paused, t22 } = state.markets;
        expect(byMint.get(main.mint)?.symbol).toBe('E2EMAIN');
        expect(byMint.get(main.mint)?.venue).toBe(state.program === 'test-venue' ? 'mock' : 'none');
        expect(byMint.get(paused.mint)?.flags.paused_coverage).toBe(true);
        expect(byMint.get(t22.mint)?.venue).toBe('none');

        const detail = await get<MarketDetail>(`/v1/markets/${main.mint}`);
        expect(detail.body.lender_count).toBeGreaterThanOrEqual(2);
        expect((await get<unknown>(`/v1/markets/${state.createMints.ok}`)).status).toBe(404);

        const cfg = await get<ProtocolConfig>('/v1/admin/config');
        expect(cfg.body.coverage_authority).toBe(state.admin.coverage);
        expect(cfg.body.paused_opens).toBe(false);
        const venues = await get<VenueStatus>('/v1/venues');
        expect(venues.body.guards.map((g) => [g.name, g.upgraded])).toEqual([
            ['pump', false],
            ['pump_amm', false],
            ['pump_fees', false],
            ['raydium_v4', false],
            ['raydium_cpmm', false],
            ['jupiter', false],
        ]);
        const lender = await get<LenderDetail>(`/v1/lenders/${state.wallets.claimer.address}`);
        expect(lender.body.positions.map((p) => p.market)).toContain(main.market);
        if (state.positions) {
            const p = state.positions.margin;
            const mine = await get<Page<Position>>(`/v1/borrowers/${p.borrower}/positions`);
            expect(mine.body.data.map((x) => x.position)).toEqual([p.position]);
        }
    });

    test('the routed indexer WebSocket subscribes and pushes market summaries', async ({
        app,
        state,
        page,
        indexerMock,
    }) => {
        test.skip(!indexerMock, 'checks the routed indexer (profile chain)');
        await app.goto('/markets', 'markets');
        const mint = state.markets.move.mint;
        // A socket from the page (the app's own opens once a page subscribes a channel).
        await page.evaluate((url) => {
            const w = window as unknown as { __e2eWs: WebSocket; __e2eMsgs: unknown[] };
            w.__e2eMsgs = [];
            w.__e2eWs = new WebSocket(url);
            w.__e2eWs.onmessage = (e) => w.__e2eMsgs.push(JSON.parse(String(e.data)));
            w.__e2eWs.onopen = () => w.__e2eWs.send(JSON.stringify({ op: 'subscribe', channels: ['markets'] }));
        }, URLS.indexerWs);
        const messages = () =>
            page.evaluate(
                () => (window as unknown as { __e2eMsgs: { type: string; data?: { mint?: string } }[] }).__e2eMsgs,
            );
        await expect.poll(async () => (await messages()).map((m) => m.type)).toContain('subscribed');
        await need(indexerMock, 'indexer mock').pushMarket(mint);
        await expect.poll(async () => (await messages()).find((m) => m.type === 'market')?.data?.mint).toBe(mint);
    });

    test('Node-side actions: CPI Guard, required memos, price moves, a swept donation', async ({ state }) => {
        const race = state.tokenAccounts.errors.race;
        const sig = await setCpiGuard('errors', race, true);
        expect((await tokenExtensions(race)).lockCpi).toBe(true);
        const landed = need(await landedTransaction(sig), 'landed EnableCpiGuard');
        expect(landed.instructions.map((ix) => ix.program)).toContain(PROGRAMS.token2022);
        await setCpiGuard('errors', race, false);
        expect((await tokenExtensions(race)).lockCpi).toBe(false);

        const memoAta = state.tokenAccounts.memoRace.ata;
        await setRequireMemos('memoRace', memoAta, true);
        expect((await tokenExtensions(memoAta)).requireMemos).toBe(true);
        await setRequireMemos('memoRace', memoAta, false);
        expect((await tokenExtensions(memoAta)).requireMemos).toBe(false);

        requireTrading(state);
        const m = state.markets.paused;
        const pool = await derivePool(m.mint, m.tokenProgram);
        const r0 = await reserves(pool);
        const r1 = await movePrice(m, 1_000);
        const p0 = (r0.y * 10n ** 12n) / r0.x;
        const p1 = (r1.y * 10n ** 12n) / r1.x;
        // +10 % ± 0.3 % (the swap fee and rounding).
        expect(Number(((p1 - p0) * 10_000n) / p0)).toBeGreaterThan(970);
        expect(Number(((p1 - p0) * 10_000n) / p0)).toBeLessThan(1_030);
        const r2 = await movePrice(m, -909);
        const p2 = (r2.y * 10n ** 12n) / r2.x;
        expect(Number(((p2 - p0) * 10_000n) / p0)).toBeLessThan(60);

        const vest = async () =>
            getMarketDecoder().decode(need(await getAccount(m.market), 'paused market').data).vestBal;
        const v0 = await vest();
        await donateAndSweep(m, LAMPORTS_PER_SOL / 10n);
        expect(await vest()).toBeGreaterThan(v0);
    });
});
