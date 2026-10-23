/**
 * Data freshness (FRONTEND.md §6.6): how far the indexer's reconciled chain watermark is behind
 * the RPC node (≈ 0.4 s per slot). The most recently applied event can be old on a quiet market.
 * An uninitialized devnet is a distinct neutral state while its response remains recent.
 */
export type Freshness = 'live' | 'lagging' | 'stale' | 'down' | 'unknown' | 'idle';

export const SLOT_MS = 400;
export const LIVE_LAG_S = 10;
export const STALE_LAG_S = 60;

export interface FreshnessInput {
    readonly indexerSlot: number | null;
    readonly rpcSlot: bigint | null;
    /** ms since the indexer response arrived. */
    readonly ageMs: number;
    readonly indexerReachable: boolean;
    /** Devnet has no GlobalConfig yet, and the indexer status request succeeded. */
    readonly awaitingActivity?: boolean;
}

export function lagSeconds(indexerSlot: number | null, rpcSlot: bigint | null): number | null {
    if (indexerSlot === null || rpcSlot === null) return null;
    const lag = Number(rpcSlot) - indexerSlot;
    return Math.max(0, Math.round((lag * SLOT_MS) / 1000));
}

export function freshness(i: FreshnessInput): Freshness {
    if (!i.indexerReachable) return 'down';
    // An uninitialized devnet has no market transactions to advance the indexed slot. This is
    // neither evidence of current ingest coverage nor a reason to call the service stale.
    if (i.awaitingActivity && i.ageMs <= STALE_LAG_S * 1000) return 'idle';
    const lag = lagSeconds(i.indexerSlot, i.rpcSlot);
    const effective = Math.max(lag ?? 0, Math.round(i.ageMs / 1000));
    if (lag === null && i.indexerSlot === null) return 'unknown';
    if (effective <= LIVE_LAG_S) return 'live';
    if (effective <= STALE_LAG_S) return 'lagging';
    return 'stale';
}
