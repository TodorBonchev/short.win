/**
 * What the syncs can add to a claim, known before the Claim button offers them (the owner's
 * decision of 2026-10-11, "only when syncs can pay"; FRONTEND.md §3.6, §3.8). A claim with a
 * shortfall (credited SOL out of reach, `CLAIM_DUST_LAMPORTS` or more) runs, in the background, the
 * search its review runs (`searchPotSyncs`, `build.ts`: `gatherPotSyncs` with `claimPlanFrom`'s
 * inputs). The button's {g} is then min(shortfall, the picked syncs' gain), none under the dust
 * threshold; the claim opens only on something claimable now or such a {g}. Lend → Claim and each
 * Lending page row use it (`claimOffer`).
 *
 * - **Cached** (react-query) by market, owner and the stored state the pick is computed from and a
 *   price crank does not move (`searchInputs`): the claimant's own lender fields, the pot
 *   (`unmatched`), the share epoch and the total shares. Not the realised-rent index (`acc_r`): a
 *   crank's accrual releases vesting income into `acc` and `acc_r` together while a vest runs, which
 *   leaves every lender's surplus — what a sync moves to the pot — as it was, yet would start a new
 *   search on every crank. Not the lenders' SOL (`lender_sol`, which sweeps move often): a sync's
 *   gain does not read it. Not the claim view's figures, which move every second while rent
 *   accrues. A new search runs when one of those inputs changes (a deposit, a claim, a rebase),
 *   when a claim, deposit or withdrawal of the market lands (`lendInvalidations`), on a remount
 *   after `SYNC_SEARCH_STALE_MS`, and — with nothing claimable now, where the search decides whether
 *   the Claim opens — about every `SYNC_SEARCH_REFRESH_MS` (± `SYNC_SEARCH_JITTER_MS`, fixed per
 *   market) while the page is visible (rent paid in raises `acc_r` alone and can give another
 *   lender a surplus); never on a price crank.
 * - **No flicker:** while a new search for the same market, owner and own position runs, the last
 *   result stays (react-query's placeholder; not fresh, so the review searches again); a first
 *   search, or the first after the user's own claim, deposit or withdrawal (whose syncs that claim
 *   may have run), shows the in-flight state.
 * - **Only for a shortfall** of `CLAIM_DUST_LAMPORTS` or more (`previewClaimView` reports less as 0).
 * - **Bounded:** at most `SYNC_SEARCH_CONCURRENCY` searches run at once, app-wide (a lender in many
 *   markets queues them on the Lending page instead of a request storm). A search whose query is
 *   dropped (its inputs replaced by newer ones, its page left) leaves the queue without running,
 *   or, running, stops before its next indexer page and frees its slot (react-query's `signal`).
 * - **Cut short:** a failed read (the indexer or the RPC down; the indexer's pages are asked for
 *   once, without retries) ends the search with what the pages before found, as `gatherPotSyncs`
 *   treats it; so does `SYNC_SEARCH_TIMEOUT_MS` (the search stops and frees its slot). That result
 *   is shown but never fresh: the review searches again. The Lending page runs no search while it
 *   knows the indexer is down (its lenders list is where the syncs come from).
 * - **While a first search runs** nothing more than is known is promised: with something claimable
 *   now the button reads `claim.label.full` (enabled; the second line appears once the gain is
 *   known), with nothing claimable it is the disabled, busy "Review claim · 0 SOL" with no reason
 *   yet — then either the two-line label (enabled) or "Nothing to claim yet", so no reason shows
 *   that then flips.
 * - **The review** reuses the pick while it is fresh (younger than `SYNC_SEARCH_STALE_MS`, not a
 *   placeholder, not cut short), else searches again, and its simulation decides: one that pays 0 is not offered
 *   for signing (`buildClaimPlan`'s `holdSign`).
 */
import type { Address } from '@solana/kit';
import { type QueryClient, useQueries } from '@tanstack/react-query';
import type { MathModule } from '../../../math/load.ts';
import type { AppRpc } from '../../../rpc/client.ts';
import { useAppClient } from '../../../rpc/context.tsx';
import { NO_SYNCS, searchPotSyncs } from '../build.ts';
import { chainNowSeconds, lendKeys } from '../chain.ts';
import type { LoadedChain } from '../ctx.ts';
import { CLAIM_DUST_LAMPORTS, type ClaimView } from '../previews.ts';
import type { SearchProgress, SyncPick } from '../quote.ts';
import { indexerLenders } from './otherLenders.ts';

/** A search younger than this is reused (the review takes its pick; a remount does not search again). */
export const SYNC_SEARCH_STALE_MS = 15_000;

/**
 * A shown search with nothing claimable now (where it decides whether the Claim opens) is run
 * again about this often while the page is visible (never on a price crank): ± `SYNC_SEARCH_JITTER_MS`,
 * the offset fixed per market (`searchRefreshMs`), so a Lending page's rows do not search together.
 */
export const SYNC_SEARCH_REFRESH_MS = 60_000;
export const SYNC_SEARCH_JITTER_MS = 15_000;

/** A market's refresh period: `SYNC_SEARCH_REFRESH_MS` ± `SYNC_SEARCH_JITTER_MS`, the same for one mint. */
export function searchRefreshMs(mint: string): number {
    let h = 0;
    for (let i = 0; i < mint.length; i++) h = (h * 31 + mint.charCodeAt(i)) >>> 0;
    return SYNC_SEARCH_REFRESH_MS - SYNC_SEARCH_JITTER_MS + (h % (2 * SYNC_SEARCH_JITTER_MS + 1));
}

/** Searches running at once, app-wide (each pages through the indexer and the RPC, FE-30). */
export const SYNC_SEARCH_CONCURRENCY = 2;

/**
 * A search still running after this long stops and frees its slot (a hung indexer or RPC never
 * holds the queue; `MAX_SYNC_CANDIDATE_PAGES` pages take an estimated 2–3 s): its result is what
 * the pages before found, kept but never fresh (the review searches again).
 */
export const SYNC_SEARCH_TIMEOUT_MS = 20_000;

function abortError(): Error {
    return new DOMException('The search was dropped', 'AbortError');
}

/**
 * `run`'s result, or `fallback()` once `ms` have passed or `outer` aborts — and then `run`'s own
 * signal aborts, so the work stops at its next step (`gatherPotSyncs` checks it between pages).
 * `fallback` is called then, so it can give what `run` has found so far.
 */
export function within<T>(
    run: (signal: AbortSignal) => Promise<T>,
    ms: number,
    fallback: () => T,
    outer?: AbortSignal,
): Promise<T> {
    const stop = new AbortController();
    const end = () => stop.abort();
    if (outer?.aborted) end();
    else outer?.addEventListener('abort', end, { once: true });
    const timer = setTimeout(end, ms);
    const late = new Promise<T>((r) => {
        if (stop.signal.aborted) r(fallback());
        else stop.signal.addEventListener('abort', () => r(fallback()), { once: true });
    });
    return Promise.race([run(stop.signal), late]).finally(() => {
        clearTimeout(timer);
        outer?.removeEventListener('abort', end);
        end();
    });
}

let running = 0;
const waiting: (() => void)[] = [];

function release() {
    const next = waiting.shift();
    if (next) next();
    else running--;
}

/**
 * Runs `fn` once fewer than `SYNC_SEARCH_CONCURRENCY` searches run (a finished one hands its slot
 * on). A queued call whose `signal` aborts leaves the queue without running (an `AbortError`).
 */
export async function limited<T>(fn: () => Promise<T>, signal?: AbortSignal): Promise<T> {
    if (signal?.aborted) throw abortError();
    if (running < SYNC_SEARCH_CONCURRENCY) running++;
    else
        await new Promise<void>((resolve, reject) => {
            const go = () => {
                signal?.removeEventListener('abort', drop);
                resolve();
            };
            const drop = () => {
                const i = waiting.indexOf(go);
                if (i >= 0) waiting.splice(i, 1);
                reject(abortError());
            };
            waiting.push(go);
            signal?.addEventListener('abort', drop, { once: true });
        });
    try {
        return await fn();
    } finally {
        release();
    }
}

/**
 * The stored state a claim's pick is computed from that a price crank leaves alone (the cache
 * key's tail; see the header for what is left out and why).
 */
export function searchInputs(c: LoadedChain): string {
    const l = c.lender;
    return [
        c.market.shareEpoch,
        c.market.totalShares,
        c.vault.unmatched,
        l ? [l.shares, l.pending, l.real, l.claimed, l.epoch, l.fwd, l.fwdEpoch].join(',') : '-',
    ].join(':');
}

/** Where the Claim button's search stands for one market. */
export interface SyncSearch {
    /** `off`: nothing to look for (no shortfall, no wallet, no math); `pending`: no result yet for these inputs. */
    readonly status: 'off' | 'pending' | 'done';
    /** The picked syncs (`done`). */
    readonly pick: SyncPick | null;
    /** The pick is younger than `SYNC_SEARCH_STALE_MS`: the review may take it instead of searching again. */
    readonly fresh: boolean;
}

export const SEARCH_OFF: SyncSearch = { status: 'off', pick: null, fresh: false };

/** What a claim offers now, from its view and the search (the button's label, state and reason). */
export interface ClaimOffer {
    /** {c}: what `claim_sol` pays now. */
    readonly claimable: bigint;
    /**
     * {g} of `claim.label.syncs`: min(shortfall, the picked syncs' gain); 0 under
     * `CLAIM_DUST_LAMPORTS`, while the search runs, or when it found none.
     */
    readonly syncs: bigint;
    /** The Claim opens: something claimable now, or syncs that add `CLAIM_DUST_LAMPORTS` or more. */
    readonly open: boolean;
    /** Nothing claimable now and the search still runs: disabled and busy, no reason yet. */
    readonly waiting: boolean;
}

export function claimOffer(v: Pick<ClaimView, 'claimable' | 'shortfall'>, s: SyncSearch): ClaimOffer {
    const gain = s.status === 'done' && s.pick ? s.pick.gain : 0n;
    const g = gain < v.shortfall ? gain : v.shortfall;
    const syncs = g >= CLAIM_DUST_LAMPORTS ? g : 0n;
    return {
        claimable: v.claimable,
        syncs,
        open: v.claimable > 0n || syncs > 0n,
        waiting: v.claimable === 0n && v.shortfall > 0n && s.status === 'pending',
    };
}

/** One market's search: the chain read it runs on and the claim view's figures now. */
export interface SearchTarget {
    readonly chain: LoadedChain | undefined;
    /** The shortfall the syncs may cover (0: no search). */
    readonly shortfall: bigint;
    /** Claimable now: with none, the search decides whether the Claim opens, and is refreshed. */
    readonly claimable: bigint;
}

/** A search's cached result: the pick, and whether the search ran to its end (not timed out, no failed read). */
export interface Found {
    readonly pick: SyncPick;
    readonly whole: boolean;
}

/** The claimant's own lender fields: the last part of `searchInputs`. */
const ownPart = (inputs: string): string => inputs.slice(inputs.lastIndexOf(':') + 1);

/**
 * The latest search result for one market and owner under other inputs (the cache keeps the
 * earlier keys' results) — with the claimant's own lender fields as in `inputs`: a result from
 * before the user's own claim, deposit or withdrawal names syncs that claim may have run, so the
 * first search after one shows the in-flight state. `useQueries` gives a changed key a new
 * observer, so react-query's own `keepPreviousData` would not carry it over.
 */
export function lastPick(qc: QueryClient, target: readonly unknown[], inputs: string): Found | undefined {
    let best: { at: number; found: Found } | undefined;
    const own = ownPart(inputs);
    for (const q of qc.getQueryCache().findAll({ queryKey: target })) {
        const found = q.state.data as Found | undefined;
        const key = q.queryKey[target.length];
        if (found === undefined || typeof key !== 'string' || ownPart(key) !== own) continue;
        if (!best || q.state.dataUpdatedAt > best.at) best = { at: q.state.dataUpdatedAt, found };
    }
    return best?.found;
}

function searchQuery(
    a: { rpc: AppRpc; qc: QueryClient; math: MathModule | undefined; owner: Address | null },
    t: SearchTarget,
) {
    const c = t.chain;
    const enabled = !!c?.lender && !!a.math && !!a.owner && t.shortfall > 0n;
    const target = lendKeys.claimSyncs(c?.mint ?? '', a.owner);
    const inputs = c ? searchInputs(c) : '-';
    return {
        queryKey: [...target, inputs] as const,
        queryFn: ({ signal }: { signal: AbortSignal }): Promise<Found> => {
            // What the search has found so far: a timeout keeps it (cut short, so not fresh).
            const progress: SearchProgress = { pick: NO_SYNCS, cut: false };
            return limited(
                () =>
                    within(
                        async (stop): Promise<Found> => {
                            const pick = await searchPotSyncs({
                                rpc: a.rpc,
                                math: a.math as MathModule,
                                chain: c as LoadedChain,
                                claimant: a.owner as Address,
                                nowS: chainNowSeconds(c as LoadedChain, Date.now()),
                                source: indexerLenders(a.qc, (c as LoadedChain).mint, true),
                                signal: stop,
                                progress,
                            });
                            return { pick, whole: !progress.cut };
                        },
                        SYNC_SEARCH_TIMEOUT_MS,
                        () => ({ pick: progress.pick, whole: false }),
                        signal,
                    ),
                signal,
            );
        },
        // The last result for the same market, owner and own position stays while a new search
        // runs (no flicker).
        placeholderData: () => lastPick(a.qc, target, inputs),
        enabled,
        staleTime: SYNC_SEARCH_STALE_MS,
        refetchInterval: c && t.claimable === 0n ? searchRefreshMs(c.mint) : (false as const),
        refetchOnWindowFocus: false,
        retry: false,
    };
}

/**
 * A query's result as the button and the review see it: no data yet → `pending`; the last result
 * while a new search runs (a placeholder) or a search cut short (timed out, a failed read) → shown
 * but not fresh, so the review searches again; else fresh while younger than `SYNC_SEARCH_STALE_MS`.
 */
export function searchOf(
    q: { readonly data: Found | undefined; readonly isPlaceholderData: boolean; readonly dataUpdatedAt: number },
    now: number,
): SyncSearch {
    if (q.data === undefined) return { status: 'pending', pick: null, fresh: false };
    const fresh = q.data.whole && !q.isPlaceholderData && now - q.dataUpdatedAt < SYNC_SEARCH_STALE_MS;
    return { status: 'done', pick: q.data.pick, fresh };
}

/** The searches of `targets` (one per market, in order); `SEARCH_OFF` where there is nothing to look for. */
export function useClaimSyncSearches(
    targets: readonly SearchTarget[],
    math: MathModule | undefined,
    owner: Address | null,
    qc: QueryClient,
): SyncSearch[] {
    const client = useAppClient();
    const env = { rpc: client.rpc, qc, math, owner };
    const options = targets.map((t) => searchQuery(env, t));
    const queries = useQueries({ queries: options });
    return options.map((o, i) => {
        const q = queries[i];
        if (!q || !o.enabled) return SEARCH_OFF;
        // A failed search counts as no syncs (searchPotSyncs already turns failures into none).
        if (q.isError) return { status: 'done', pick: null, fresh: false };
        return searchOf(q, Date.now());
    });
}
