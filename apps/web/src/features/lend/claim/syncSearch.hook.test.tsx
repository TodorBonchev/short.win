/**
 * The Claim button's background search as the hook runs it (`useClaimSyncSearches`, the owner's
 * decision of 2026-10-11; FRONTEND.md §3.6, §3.8), `searchPotSyncs` stubbed: a timed-out search
 * keeps what its pages found but is never fresh, the last result carries over only while the
 * claimant's own position is unchanged, the refresh runs only with nothing claimable now (its
 * period fixed per market), and the indexer's pages are asked for once.
 */
import type { LenderPosition, MarketVault } from '@shortwin/client';
import { address } from '@solana/kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { config } from '../../../config/env.ts';
import type { MathModule } from '../../../math/load.ts';
import { createAppClient } from '../../../rpc/client.ts';
import { AppClientProvider } from '../../../rpc/context.tsx';
import type { LoadedChain } from '../ctx.ts';
import type { SearchProgress, SyncPick } from '../quote.ts';
import { fakeMarket, simpleLender, simplePool } from '../testkit.ts';
import { indexerLenders } from './otherLenders.ts';
import {
    type SearchTarget,
    SYNC_SEARCH_JITTER_MS,
    SYNC_SEARCH_REFRESH_MS,
    SYNC_SEARCH_TIMEOUT_MS,
    searchRefreshMs,
    useClaimSyncSearches,
} from './syncSearch.ts';

const h = vi.hoisted(() => ({
    /** The stubbed `searchPotSyncs`. */
    search: { current: null as unknown },
}));

vi.mock('../build.ts', async (orig) => ({
    ...(await orig<typeof import('../build.ts')>()),
    searchPotSyncs: (...args: unknown[]) => (h.search.current as (...a: unknown[]) => unknown)(...args),
}));

const OWNER = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const MARKET = address('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE');
const MINT = address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const MATH = {} as MathModule;

const pickOf = (gain: bigint): SyncPick => ({ syncs: [], gain, more: { count: 0, gain: 0n } });

function chain(o: { claimed?: bigint; unmatched?: bigint } = {}): LoadedChain {
    const vault = simplePool().vault as unknown as MarketVault;
    const lender: LenderPosition = {
        ...simpleLender(10n ** 9n, { claimed: o.claimed ?? 0n }),
        discriminator: new Uint8Array(8),
        owner: OWNER,
        market: MARKET,
        bump: 255,
        fwdShares: 0n,
        fwdEpoch: 0,
        fwd: 0,
        fwdLag: 0,
        reserved: new Uint8Array(10),
    } as LenderPosition;
    return {
        mint: MINT,
        marketAddress: MARKET,
        vaultAddress: MARKET,
        market: fakeMarket({}, { mint: MINT }),
        vault: o.unmatched === undefined ? vault : { ...vault, unmatched: o.unmatched },
        config: null,
        mintInfo: null,
        owner: OWNER,
        lenderAddress: MARKET,
        lender,
        records: [],
        recordsComplete: true,
        tokenAccounts: [],
        ata: null,
        walletLamports: 0n,
        slot: 1n,
        readAt: 0,
    } as LoadedChain;
}

let qc: QueryClient;
let calls: number;

function render(target: SearchTarget) {
    const wrapper = ({ children }: { children: ReactNode }) => (
        <AppClientProvider client={createAppClient(config)}>
            <QueryClientProvider client={qc}>{children}</QueryClientProvider>
        </AppClientProvider>
    );
    return renderHook((t: SearchTarget) => useClaimSyncSearches([t], MATH, OWNER, qc)[0], {
        wrapper,
        initialProps: target,
    });
}

const settle = () => act(() => vi.advanceTimersByTimeAsync(50));

beforeEach(() => {
    vi.useFakeTimers();
    qc = new QueryClient();
    calls = 0;
});

afterEach(() => {
    qc.clear();
    vi.useRealTimers();
});

describe('useClaimSyncSearches', () => {
    it('a search past its timeout keeps the syncs its pages found, but is not fresh (the review searches again)', async () => {
        h.search.current = (a: { progress: SearchProgress; signal: AbortSignal }) => {
            calls++;
            // The first page found 15,000 lamports; the next never answers (a slow indexer).
            a.progress.pick = pickOf(15_000n);
            return new Promise<SyncPick>(() => {});
        };
        const r = render({ chain: chain(), shortfall: 25_000n, claimable: 0n });
        await settle();
        expect(r.result.current?.status).toBe('pending');
        await act(() => vi.advanceTimersByTimeAsync(SYNC_SEARCH_TIMEOUT_MS));
        expect(r.result.current?.status).toBe('done');
        expect(r.result.current?.pick?.gain).toBe(15_000n);
        expect(r.result.current?.fresh).toBe(false);
        r.unmount();
    });

    it('a search cut short by a failed read is shown, not fresh; a whole one is fresh', async () => {
        h.search.current = async (a: { progress: SearchProgress }) => {
            calls++;
            a.progress.cut = calls === 1;
            return pickOf(12_000n);
        };
        const r = render({ chain: chain(), shortfall: 25_000n, claimable: 0n });
        await settle();
        expect(r.result.current).toMatchObject({ status: 'done', fresh: false });
        expect(r.result.current?.pick?.gain).toBe(12_000n);
        await act(() => qc.invalidateQueries());
        await settle();
        expect(calls).toBe(2);
        expect(r.result.current).toMatchObject({ status: 'done', fresh: true });
        r.unmount();
    });

    it('the last result carries over a new search of the same position, not one after the user’s own claim', async () => {
        let release: (p: SyncPick) => void = () => {};
        h.search.current = () => {
            calls++;
            if (calls === 1) return Promise.resolve(pickOf(50_000n));
            return new Promise<SyncPick>((res) => {
                release = res;
            });
        };
        const r = render({ chain: chain(), shortfall: 25_000n, claimable: 0n });
        await settle();
        expect(r.result.current?.pick?.gain).toBe(50_000n);
        // The pot moved (another lender's deposit): the last result stays while the search runs.
        r.rerender({ chain: chain({ unmatched: 7n }), shortfall: 25_000n, claimable: 0n });
        await settle();
        expect(calls).toBe(2);
        expect(r.result.current).toMatchObject({ status: 'done', fresh: false });
        expect(r.result.current?.pick?.gain).toBe(50_000n);
        await act(async () => release(pickOf(0n)));
        await settle();
        // The user's own claim landed (its claimed moved): its syncs may be spent, nothing carries over.
        r.rerender({ chain: chain({ unmatched: 7n, claimed: 40_000n }), shortfall: 25_000n, claimable: 0n });
        await settle();
        expect(calls).toBe(3);
        expect(r.result.current).toEqual({ status: 'pending', pick: null, fresh: false });
        r.unmount();
    });

    it('refreshes only with nothing claimable now, about once a minute, the period fixed per market', async () => {
        h.search.current = async () => {
            calls++;
            return pickOf(0n);
        };
        const every = searchRefreshMs(MINT);
        expect(every).toBe(searchRefreshMs(MINT));
        expect(every).toBeGreaterThanOrEqual(SYNC_SEARCH_REFRESH_MS - SYNC_SEARCH_JITTER_MS);
        expect(every).toBeLessThanOrEqual(SYNC_SEARCH_REFRESH_MS + SYNC_SEARCH_JITTER_MS);
        const periods = new Set(['So11111111111111111111111111111111111111112', MINT, MARKET].map(searchRefreshMs));
        expect(periods.size).toBe(3);
        // Something claimable now: the Claim is open whatever the syncs, no refresh.
        const open = render({ chain: chain(), shortfall: 25_000n, claimable: 1n });
        await settle();
        expect(calls).toBe(1);
        await act(() => vi.advanceTimersByTimeAsync(SYNC_SEARCH_REFRESH_MS + SYNC_SEARCH_JITTER_MS + 1_000));
        expect(calls).toBe(1);
        open.unmount();
        qc.clear();
        calls = 0;
        // Nothing claimable now: the search decides, and runs again after the market's period.
        const shut = render({ chain: chain(), shortfall: 25_000n, claimable: 0n });
        await settle();
        expect(calls).toBe(1);
        await act(() => vi.advanceTimersByTimeAsync(every - 100));
        expect(calls).toBe(1);
        await act(() => vi.advanceTimersByTimeAsync(200));
        expect(calls).toBe(2);
        shut.unmount();
    });
});

describe('indexerLenders', () => {
    it('asks for each page once for the button’s search, with the retries for the review’s', async () => {
        const fetchQuery = vi.spyOn(qc, 'fetchQuery').mockResolvedValue({ data: { data: [], next_cursor: null } });
        const q = { epoch: 'behind', cursor: null, limit: 50 } as const;
        await indexerLenders(qc, MINT, true)(q);
        expect(fetchQuery.mock.calls[0]?.[0]).toMatchObject({ retry: false });
        await indexerLenders(qc, MINT)(q);
        expect(typeof fetchQuery.mock.calls[1]?.[0].retry).toBe('function');
    });
});
