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
import type { PositionView } from './previews.ts';
import { fakeMarket, simpleLender, simplePool } from './testkit.ts';

const h = vi.hoisted(() => ({
    wallet: { current: null as unknown },
    rows: { current: null as unknown },
    started: [] as unknown[],
    setStage: { current: (_s: string) => {} },
}));

vi.mock('../../wallet/useWallet.ts', () => ({ useWallet: () => h.wallet.current }));
vi.mock('../../math/useMath.ts', () => ({ useMath: () => ({ math: undefined, error: null }) }));
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

const started = h.started as TxPlan[];
const drive = (stage: TxStage) => act(() => h.setStage.current(stage));

const lender = (o: Parameters<typeof simpleLender>[1] = {}): LenderPosition => ({
    ...simpleLender(10n ** 9n, o),
    discriminator: new Uint8Array(8),
    owner: OWNER,
    market: M1,
    bump: 255,
    reserved: new Uint8Array(32),
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

const row = (o: Partial<LenderRow> & Pick<LenderRow, 'market' | 'mint'>): LenderRow => ({
    symbol: 'TKN',
    decimals: 6,
    chain: chain(o.mint, o.market),
    chainError: null,
    position: position(1_000n),
    standing: { kind: 'current' },
    indexer: 'ok',
    isCoinCreator: false,
    isMarketCreator: false,
    ...o,
});

class NoSocket {
    readyState = 0;
    send() {}
    close() {}
    addEventListener() {}
    removeEventListener() {}
}

beforeEach(() => {
    started.length = 0;
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
