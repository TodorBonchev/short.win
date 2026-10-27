/**
 * The market page against a mocked indexer and chain (FRONTEND.md §3.4, §10.1): the mapping of API
 * and account fields to the header and stat strip (the rate through the real WASM math), the flags
 * and banners from every source, "Why this tier", creator-wallet deposits (R1), the liquidations
 * and lenders tabs, the chain view when the indexer does not know the market, and Not found.
 */
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DISCLOSURES, renderDisclosure } from '../../config/disclosures.ts';
import { PUMP_AMM_PROGRAM } from '../../config/programs.ts';
import { T } from '../../config/testids.ts';
import { initMathSync } from '../../math/load.ts';
import * as accounts from '../../rpc/accounts.ts';
import { readRepoBytes } from '../../test/repo.ts';
import * as chainReads from './shared/chainReads.ts';
import { GUARD, MARKET_FLAG, VENUE_BIT } from './shared/marketFlags.ts';
import {
    CREATOR,
    chainMarket,
    detail,
    exists,
    exitEvent,
    globalConfig,
    LENDER,
    lender,
    MINT_A,
    missing,
    oracleCache,
    SIG_A,
    SIG_B,
    venues,
} from './test/fixtures.ts';
import { mockIndexer, problem, renderAt } from './test/harness.tsx';

vi.mock('../../rpc/accounts.ts', async (orig) => ({
    ...(await orig<typeof import('../../rpc/accounts.ts')>()),
    readMarket: vi.fn(),
    readGlobalConfig: vi.fn(),
    readVenueGuard: vi.fn(),
    readOracle: vi.fn(),
}));
vi.mock('./shared/chainReads.ts', async (orig) => ({
    ...(await orig<typeof import('./shared/chainReads.ts')>()),
    readRawAccount: vi.fn(),
}));

const math = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));

const POOLS_NONE = { mint: MINT_A, current: null, window: null, pools: [], events: [] };
const LENDERS = {
    data: [
        lender(CREATOR, { is_coin_creator: true, is_market_creator: true, share_bps: 2_000 }),
        lender(LENDER, { share_bps: 8_000 }),
    ],
    next_cursor: null,
};

function indexerFor(over: Record<string, unknown> = {}) {
    return mockIndexer({
        [`/v1/markets/${MINT_A}`]: detail({ symbol: 'BONK', name: 'Bonk Inu' }),
        [`/v1/markets/${MINT_A}/pools`]: POOLS_NONE,
        [`/v1/markets/${MINT_A}/lenders`]: LENDERS,
        ...over,
    });
}

beforeEach(() => {
    vi.mocked(accounts.readMarket).mockResolvedValue(exists(MINT_A, chainMarket()));
    vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, globalConfig()));
    vi.mocked(accounts.readVenueGuard).mockResolvedValue(missing(MINT_A));
    vi.mocked(accounts.readOracle).mockResolvedValue(exists(MINT_A, oracleCache()));
    vi.mocked(chainReads.readRawAccount).mockResolvedValue(null);
});

afterEach(() => {
    vi.unstubAllGlobals();
});

const stat = (k: Parameters<typeof T.marketStat>[0]) => screen.getByTestId(T.marketStat(k)).dataset.value;

describe('MarketPage', () => {
    it('maps the indexer detail and the chain to the header and the stat strip', async () => {
        indexerFor();
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        expect((await screen.findByTestId(T.marketSymbol)).textContent).toBe('BONK');
        expect(screen.getByTestId(T.marketMint).dataset.value).toBe(MINT_A);
        expect(screen.getByTestId(T.marketVenue).textContent).toBe('PumpSwap');
        expect(screen.getByTestId(T.marketTier).dataset.value).toBe('C');
        expect(screen.getByTestId(T.marketSource).dataset.value).toBe('indexer');
        expect(stat('inventory')).toBe('1000000000000');
        expect(stat('utilisation')).toBe('2500');
        expect(stat('oi')).toBe('12500000000');
        expect(stat('tvl')).toBe('40000000000');
        expect(stat('lenders')).toBe('7');
        expect(stat('price')).toBe('28000000000000');
        expect(screen.getByTestId(T.marketSpotPrice).dataset.value).toBe('29000000000000');
        // The rate a new open pays: Tier C's live terms, the market's kink, the current utilisation.
        await waitFor(() => expect(stat('rate')).toBe(String(math.rateBps(1_000, 2_000, 10_000, 2_500, 8_000))));
        expect(document.querySelector('[data-testid^="market-flag-"]')).toBeNull();
        expect(screen.getByTestId(T.marketRouteChain('no-route')).dataset.value).toBe('0');
        // The trade panel keeps its contract.
        expect(screen.getByTestId(T.shortPanel).dataset.mint).toBe(MINT_A);
        expect(screen.getByTestId(T.marketStat('caps'))).toBeTruthy();
        queryClient.clear();
    });

    it('keeps aggregate P&L unavailable when borrower token acquisition costs are unknown', async () => {
        const base = detail();
        indexerFor({
            [`/v1/markets/${MINT_A}`]: {
                ...base,
                exits: {
                    ...base.exits,
                    borrower_pnl: null,
                    borrower_pnl_known_exits: 5,
                    borrower_pnl_unknown_exits: 1,
                },
            },
        });
        const { queryClient, view } = await renderAt(`/m/${MINT_A}`);
        try {
            expect(await screen.findByText('Unavailable (external token cost)')).toBeTruthy();
            expect(screen.getByText('Positions with unknown P&L')).toBeTruthy();
        } finally {
            view?.unmount();
            queryClient.clear();
        }
    });

    it('shows creator-wallet deposits (SPEC R1) with the self-dealing disclosure', async () => {
        indexerFor();
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        const block = await screen.findByTestId(T.marketCreatorDeposits);
        expect(block.textContent).toContain('20%');
        await waitFor(() => expect(within(block).getByText('Coin creator', { selector: '.badge' })).toBeTruthy());
        expect(within(block).getByText('Market creator', { selector: '.badge' })).toBeTruthy();
        const disclosure = within(block).getByTestId(T.disclosure('market.creatorSelfDealing'));
        expect(disclosure.textContent).toContain(
            renderDisclosure('market.creatorSelfDealing', { creatorFee: '0.0035 SOL' })[2] ?? 'missing',
        );
        expect(screen.getByTestId(T.marketFees).textContent).toContain('Open fees');
        expect(screen.getByTestId(T.marketLenderLoss).dataset.value).toBe('200000000');
        queryClient.clear();
    });

    it('derives flags from the market, the protocol bits, the latch and the chain, with banners in order', async () => {
        vi.mocked(accounts.readMarket).mockResolvedValue(
            exists(MINT_A, chainMarket({ flags: MARKET_FLAG.pausedCoverage, emaWarm: 4 })),
        );
        vi.mocked(accounts.readGlobalConfig).mockResolvedValue(
            exists(MINT_A, globalConfig({ flags: 1, venueRouteBroken: VENUE_BIT.pump_swap })),
        );
        const guards = venues().guards.map((g) => (g.venue === GUARD.pump_amm ? { ...g, upgraded: true } : g));
        indexerFor({
            '/v1/venues': venues({ guards }),
            [`/v1/markets/${MINT_A}`]: detail({ route_chains: { no_route_since: 1_800_000_000, pool_fail_since: 0 } }),
            [`/v1/markets/${MINT_A}/pools`]: {
                ...POOLS_NONE,
                window: { candidate: LENDER, d_cand: '100', window_ends: Math.floor(Date.now() / 1000) + 60 },
            },
        });
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        await screen.findByTestId(T.marketFlag('coverage-paused'));
        for (const f of ['route-broken', 'paused-opens', 'venue-upgraded', 'warming-up', 'pool-window'] as const) {
            await waitFor(() => expect(screen.getByTestId(T.marketFlag(f))).toBeTruthy());
        }
        const banners = [...document.querySelectorAll<HTMLElement>('[data-testid^="market-banner-"]')].map((e) =>
            e.dataset.testid?.replace('market-banner-', ''),
        );
        expect(banners).toEqual(['route-broken', 'paused-opens', 'coverage-paused', 'venue-upgraded', 'warming-up']);
        expect(screen.getByTestId(T.marketBanner('route-broken')).textContent).toContain(
            DISCLOSURES['market.routeBroken'].body[0],
        );
        expect(screen.getByTestId(T.marketBanner('paused-opens')).textContent).toContain('New shorts are paused');
        expect(screen.getByTestId(T.disclosure('market.poolSwitch'))).toBeTruthy();
        // A running no-route chain (S3 evidence) is shown on the Overview.
        expect(screen.getByTestId(T.marketRouteChain('no-route')).dataset.value).toBe('1800000000');
        expect(screen.getByTestId(T.marketRouteChain('no-route')).textContent).toContain('UTC');
        expect(screen.getByTestId(T.marketRouteChain('pool-fail')).textContent).toBe('—');
        queryClient.clear();
    });

    it('flags a curve market whose graduation PumpSwap has halted (disable_flags bit 0)', async () => {
        vi.mocked(accounts.readMarket).mockResolvedValue(exists(MINT_A, chainMarket({ venue: 1, tier: 3 })));
        const gc = new Uint8Array(400);
        gc.set(chainReads.PAMM_GLOBAL_CONFIG_DISCRIMINATOR, 0);
        gc[chainReads.PAMM_DISABLE_FLAGS_OFFSET] = 1;
        vi.mocked(chainReads.readRawAccount).mockResolvedValue({
            address: chainReads.PUMP_AMM_GLOBAL_CONFIG,
            owner: PUMP_AMM_PROGRAM,
            lamports: 1n,
            data: gc,
            executable: false,
        });
        indexerFor({ [`/v1/markets/${MINT_A}`]: detail({ venue: 'pump_curve', tier: 'D' }) });
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        expect(await screen.findByTestId(T.marketFlag('graduation-halted'))).toBeTruthy();
        expect(screen.getByTestId(T.disclosure('market.graduationHalted'))).toBeTruthy();
        expect(screen.getByTestId(T.marketTier).dataset.value).toBe('D');
        queryClient.clear();
    });

    it('explains the tier in a popover that Escape closes', async () => {
        indexerFor();
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        const badge = await screen.findByTestId(T.marketTier);
        expect(badge.getAttribute('aria-expanded')).toBe('false');
        await act(async () => {
            fireEvent.click(badge);
        });
        const why = screen.getByTestId(T.marketTierWhy);
        expect(badge.getAttribute('aria-expanded')).toBe('true');
        expect(why.textContent).toContain('Pool age');
        expect(why.textContent).toContain('At most Tier B until');
        await waitFor(() => expect(why.textContent).toContain('$500,000'));
        expect(within(why).getByTestId(T.disclosure('market.tierAge'))).toBeTruthy();
        await act(async () => {
            fireEvent.keyDown(why, { key: 'Escape' });
        });
        expect(screen.queryByTestId(T.marketTierWhy)).toBeNull();
        queryClient.clear();
    });

    it('lists recent liquidations with their kind and settlement condition', async () => {
        indexerFor({
            [`/v1/markets/${MINT_A}/liquidations`]: {
                data: [
                    exitEvent(SIG_A),
                    exitEvent(SIG_B, {
                        kind: 'settlement',
                        settle_condition: 'S2',
                        buyback_cost: null,
                        lender_amount: '900000000',
                    }),
                ],
                next_cursor: null,
            },
        });
        const { queryClient } = await renderAt(`/m/${MINT_A}?tab=liquidations`);
        const a = await screen.findByTestId(T.marketLiquidationRow(SIG_A));
        expect(a.dataset.kind).toBe('liquidation');
        expect(a.dataset.condition).toBeUndefined();
        const b = screen.getByTestId(T.marketLiquidationRow(SIG_B));
        expect(b.dataset.kind).toBe('settlement');
        expect(b.dataset.condition).toBe('S2');
        expect(b.textContent).toContain('0.9');
        expect(screen.getByTestId(T.marketTab('liquidations')).getAttribute('aria-selected')).toBe('true');
        queryClient.clear();
    });

    it('refreshes an already-open Liquidations tab when an exit arrives without a WebSocket event', async () => {
        vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
        let exited = false;
        const path = `/v1/markets/${MINT_A}/liquidations`;
        const api = indexerFor({
            [path]: () => ({ data: exited ? [exitEvent(SIG_A)] : [], next_cursor: null }),
        });
        const { queryClient, view } = await renderAt(`/m/${MINT_A}?tab=liquidations`);
        try {
            await screen.findByText('No liquidations, expiries or SOL settlements on this market yet.');
            expect(api.callsTo(path)).toHaveLength(1);
            // The harness's socket never opens. The mounted tab must recover through REST.
            exited = true;
            await act(async () => {
                await vi.advanceTimersByTimeAsync(10_000);
            });
            expect(await screen.findByTestId(T.marketLiquidationRow(SIG_A))).toBeTruthy();
            expect(api.callsTo(path)).toHaveLength(2);
            view?.unmount();
            await act(async () => {
                await vi.advanceTimersByTimeAsync(30_000);
            });
            expect(api.callsTo(path)).toHaveLength(2);
        } finally {
            view?.unmount();
            queryClient.clear();
            vi.useRealTimers();
        }
    });

    it('marks creator wallets among the lenders and shows the coin creator’s share', async () => {
        indexerFor();
        const { queryClient } = await renderAt(`/m/${MINT_A}?tab=lenders`);
        const row = await screen.findByTestId(T.marketLenderRow(CREATOR));
        expect(row.dataset.coinCreator).toBe('true');
        expect(row.dataset.marketCreator).toBe('true');
        expect(screen.getByTestId(T.marketLenderRow(LENDER)).dataset.coinCreator).toBe('false');
        expect(screen.getByTestId(T.marketCreatorShare).dataset.value).toBe('2000');
        const loss = screen.getByTestId(T.disclosure('lender.lossHistory'));
        expect(loss.textContent).toContain('lending in Tier C breaks even');
        expect(loss.textContent).toContain('about once every 84 days');
        expect(loss.textContent).toContain('1 SOL settlements, realised lender loss 0.2 SOL');
        queryClient.clear();
    });

    it('renders the chain view when the indexer does not know the market (FE-D1)', async () => {
        const api = indexerFor({ [`/v1/markets/${MINT_A}`]: problem(404, 'not_found') });
        const { queryClient } = await renderAt(`/m/${MINT_A}?tab=lenders`);
        await waitFor(() => expect(screen.getByTestId(T.marketSource).dataset.value).toBe('chain'));
        expect(screen.getByTestId(T.marketVenue).textContent).toBe('PumpSwap');
        expect(stat('inventory')).toBe('1000000000000');
        expect(stat('utilisation')).toBe(String(math.utilizationBps(750_000_000_000n, 250_000_000_000n)));
        // History and analytics need the indexer: Overview only.
        expect(screen.getByTestId(T.marketTab('overview'))).toBeTruthy();
        expect(screen.queryByTestId(T.marketTab('lenders'))).toBeNull();
        expect(api.callsTo(`/v1/markets/${MINT_A}/lenders`)).toHaveLength(0);
        queryClient.clear();
    });

    it('says when a mint has no market and links to its creation', async () => {
        vi.mocked(accounts.readMarket).mockResolvedValue(missing(MINT_A));
        indexerFor({ [`/v1/markets/${MINT_A}`]: problem(404, 'not_found') });
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        const nf = await screen.findByTestId(T.marketNotFound);
        expect(within(nf).getByRole('link').getAttribute('href')).toBe(`/markets/new?mint=${MINT_A}`);
        expect(screen.getByTestId(T.page('market'))).toBeTruthy();
        expect(screen.queryByTestId(T.shortPanel)).toBeNull();
        queryClient.clear();
    });
});
