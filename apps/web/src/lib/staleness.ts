/**
 * Data freshness (FRONTEND.md §6.6): how far the indexer is behind the RPC node, from the
 * `X-Indexer-Slot` header against the RPC's current slot (≈ 0.4 s per slot).
 */
export type Freshness = 'live' | 'lagging' | 'stale' | 'down' | 'unknown';

export const SLOT_MS = 400;
export const LIVE_LAG_S = 10;
export const STALE_LAG_S = 60;

export interface FreshnessInput {
    readonly indexerSlot: number | null;
    readonly rpcSlot: bigint | null;
    /** ms since the indexer response arrived. */
    readonly ageMs: number;
    readonly indexerReachable: boolean;
}

export function lagSeconds(indexerSlot: number | null, rpcSlot: bigint | null): number | null {
    if (indexerSlot === null || rpcSlot === null) return null;
    const lag = Number(rpcSlot) - indexerSlot;
    return Math.max(0, Math.round((lag * SLOT_MS) / 1000));
}

export function freshness(i: FreshnessInput): Freshness {
    if (!i.indexerReachable) return 'down';
    const lag = lagSeconds(i.indexerSlot, i.rpcSlot);
    const effective = Math.max(lag ?? 0, Math.round(i.ageMs / 1000));
    if (lag === null && i.indexerSlot === null) return 'unknown';
    if (effective <= LIVE_LAG_S) return 'live';
    if (effective <= STALE_LAG_S) return 'lagging';
    return 'stale';
}
