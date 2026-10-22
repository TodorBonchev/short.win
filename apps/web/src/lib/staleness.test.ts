import { describe, expect, it } from 'vitest';
import { freshness, lagSeconds } from './staleness.ts';

describe('staleness', () => {
    it('computes the indexer lag in seconds from slots', () => {
        expect(lagSeconds(1000, 1025n)).toBe(10);
        expect(lagSeconds(1000, 990n)).toBe(0);
        expect(lagSeconds(null, 1n)).toBeNull();
    });

    it('classifies live, lagging, stale and down', () => {
        const base = { indexerSlot: 1000, rpcSlot: 1010n, ageMs: 1000, indexerReachable: true };
        expect(freshness(base)).toBe('live');
        expect(freshness({ ...base, rpcSlot: 1100n })).toBe('lagging');
        expect(freshness({ ...base, rpcSlot: 2000n })).toBe('stale');
        expect(freshness({ ...base, ageMs: 120_000 })).toBe('stale');
        expect(freshness({ ...base, indexerReachable: false })).toBe('down');
        expect(freshness({ ...base, indexerSlot: null, rpcSlot: null })).toBe('unknown');
    });

    it('keeps an uninitialized devnet neutral only while its indexer responds', () => {
        const quiet = { indexerSlot: 1000, rpcSlot: 2000n, ageMs: 1000, indexerReachable: true };
        expect(freshness({ ...quiet, awaitingActivity: true })).toBe('idle');
        expect(freshness({ ...quiet, awaitingActivity: true, ageMs: 120_000 })).toBe('stale');
        expect(freshness(quiet)).toBe('stale');
        expect(freshness({ ...quiet, awaitingActivity: true, indexerReachable: false })).toBe('down');
    });
});
