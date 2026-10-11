/**
 * The Claim button's search for syncs (the owner's decision of 2026-10-11, "only when syncs can
 * pay"; FRONTEND.md §3.6, §3.8): what a claim offers from its view and the search (`claimOffer`),
 * the app-wide bound on searches running at once (`limited`) and the timeout that frees a slot.
 */
import { describe, expect, it, vi } from 'vitest';
import { NO_SYNCS } from '../build.ts';
import { CLAIM_DUST_LAMPORTS } from '../previews.ts';
import type { SyncPick } from '../quote.ts';
import { claimOffer, limited, SEARCH_OFF, SYNC_SEARCH_CONCURRENCY, type SyncSearch, within } from './syncSearch.ts';

const done = (gain: bigint): SyncSearch => ({
    status: 'done',
    pick: { syncs: [], gain, more: { count: 0, gain: 0n } } satisfies SyncPick,
    fresh: true,
});
const pending: SyncSearch = { status: 'pending', pick: null, fresh: false };

describe('claimOffer', () => {
    it('{g} = min(shortfall, gain), none under the dust threshold; open on claimable or {g}', () => {
        const v = { claimable: 0n, shortfall: 25_000n };
        expect(CLAIM_DUST_LAMPORTS).toBe(10_000n);
        // gain 0, < dust, = dust, between, > shortfall
        expect(claimOffer(v, done(0n))).toEqual({ claimable: 0n, syncs: 0n, open: false, waiting: false });
        expect(claimOffer(v, done(9_999n)).open).toBe(false);
        expect(claimOffer(v, done(10_000n))).toEqual({ claimable: 0n, syncs: 10_000n, open: true, waiting: false });
        expect(claimOffer(v, done(12_000n)).syncs).toBe(12_000n);
        expect(claimOffer(v, done(50_000n)).syncs).toBe(25_000n);
        // A shortfall under 10,000 lamports (the view reports 0) is never offered, whatever the gain.
        expect(claimOffer({ claimable: 0n, shortfall: 0n }, done(50_000n)).open).toBe(false);
        // Something claimable now: open whatever the syncs.
        expect(claimOffer({ claimable: 1n, shortfall: 25_000n }, done(0n))).toEqual({
            claimable: 1n,
            syncs: 0n,
            open: true,
            waiting: false,
        });
    });

    it('in flight: waiting (no reason yet) only with nothing claimable now; never a {g} before it is known', () => {
        expect(claimOffer({ claimable: 0n, shortfall: 25_000n }, pending)).toEqual({
            claimable: 0n,
            syncs: 0n,
            open: false,
            waiting: true,
        });
        expect(claimOffer({ claimable: 5n, shortfall: 25_000n }, pending)).toEqual({
            claimable: 5n,
            syncs: 0n,
            open: true,
            waiting: false,
        });
        // Nothing to search for: not waiting.
        expect(claimOffer({ claimable: 0n, shortfall: 0n }, SEARCH_OFF).waiting).toBe(false);
        // A failed search reads as done with no pick: off, with its reason.
        expect(claimOffer({ claimable: 0n, shortfall: 25_000n }, { status: 'done', pick: null, fresh: false })).toEqual(
            {
                claimable: 0n,
                syncs: 0n,
                open: false,
                waiting: false,
            },
        );
    });
});

describe('the search queue', () => {
    it('runs at most SYNC_SEARCH_CONCURRENCY searches at once, each slot handed on', async () => {
        expect(SYNC_SEARCH_CONCURRENCY).toBe(2);
        let now = 0;
        let max = 0;
        const ends: (() => void)[] = [];
        const job = () =>
            limited(async () => {
                now++;
                max = Math.max(max, now);
                await new Promise<void>((r) => ends.push(r));
                now--;
                return now;
            });
        const all = Promise.all([job(), job(), job(), job(), job()]);
        for (let i = 0; i < 5; i++) {
            await vi.waitFor(() => expect(ends.length).toBeGreaterThan(i));
            expect(now).toBeLessThanOrEqual(2);
            ends[i]?.();
        }
        await all;
        expect(max).toBe(2);
    });

    it('a queued search whose query is dropped leaves the queue without running', async () => {
        const ends: (() => void)[] = [];
        const ran: string[] = [];
        const job = (name: string, signal?: AbortSignal) =>
            limited(async () => {
                ran.push(name);
                await new Promise<void>((r) => ends.push(r));
                return name;
            }, signal);
        // Two hold the slots; three queue, two of them then superseded (their queries dropped).
        const a = job('a');
        const b = job('b');
        const drops = [new AbortController(), new AbortController()];
        const stale1 = job('stale1', drops[0]?.signal);
        const stale2 = job('stale2', drops[1]?.signal);
        const latest = job('latest');
        await vi.waitFor(() => expect(ends).toHaveLength(2));
        for (const d of drops) d.abort();
        await expect(stale1).rejects.toMatchObject({ name: 'AbortError' });
        await expect(stale2).rejects.toMatchObject({ name: 'AbortError' });
        // The first slot freed goes to the latest, not to the dropped ones.
        ends[0]?.();
        await vi.waitFor(() => expect(ran).toEqual(['a', 'b', 'latest']));
        ends[1]?.();
        ends[2]?.();
        expect(await Promise.all([a, b, latest])).toEqual(['a', 'b', 'latest']);
        // A call already dropped never queues.
        const gone = new AbortController();
        gone.abort();
        await expect(limited(async () => 'x', gone.signal)).rejects.toMatchObject({ name: 'AbortError' });
        expect(ran).toEqual(['a', 'b', 'latest']);
    });

    it('a search past its timeout gives the fallback, asked for then, and is told to stop', async () => {
        vi.useFakeTimers();
        let told: AbortSignal | undefined;
        try {
            const out = within(
                (signal) => {
                    told = signal;
                    return new Promise<SyncPick>(() => {});
                },
                20_000,
                () => NO_SYNCS,
            );
            expect(told?.aborted).toBe(false);
            await vi.advanceTimersByTimeAsync(20_000);
            expect(await out).toEqual(NO_SYNCS);
            expect(told?.aborted).toBe(true);
        } finally {
            vi.useRealTimers();
        }
        expect(
            await within(
                async () => NO_SYNCS,
                1_000,
                () => null,
            ),
        ).toEqual(NO_SYNCS);
    });

    it('a running search whose query is dropped stops and frees its slot', async () => {
        const outer = new AbortController();
        let told: AbortSignal | undefined;
        const out = within(
            (signal) => {
                told = signal;
                return new Promise<SyncPick>(() => {});
            },
            20_000,
            () => NO_SYNCS,
            outer.signal,
        );
        outer.abort();
        expect(await out).toEqual(NO_SYNCS);
        expect(told?.aborted).toBe(true);
    });
});
