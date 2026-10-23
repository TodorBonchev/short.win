/**
 * Indexer freshness (FRONTEND.md §6.6), shared by the status bar and every indexer-backed panel:
 * panels show a `stale-badge` with the lag in seconds whenever the indexer is not `live`, so stale
 * numbers are never silent.
 */
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { indexerMeta, indexerQueries } from '../api/queries.ts';
import { config } from '../config/env.ts';
import { T } from '../config/testids.ts';
import { type Freshness, freshness, lagSeconds } from '../lib/staleness.ts';
import { useStore } from '../lib/store.ts';
import { useChainConfig } from '../rpc/queries.ts';
import { rpcSlot } from '../rpc/slot.ts';

/** Wall clock in ms, re-rendering every `ms`. */
export function useNow(ms: number): number {
    const [now, setNow] = useState(Date.now);
    useEffect(() => {
        const t = setInterval(() => setNow(Date.now()), ms);
        return () => clearInterval(t);
    }, [ms]);
    return now;
}

export function useIndexerFreshness(): { state: Freshness; lagS: number | null; ageS: number | null } {
    const meta = useStore(indexerMeta);
    const slot = useStore(rpcSlot);
    const status = useQuery(indexerQueries.status());
    const devnetConfig = useChainConfig(config.cluster === 'devnet');
    const now = useNow(2_000);
    const ageMs = meta ? Math.max(0, now - meta.receivedAt) : 0;
    // The applied-event slot stays fixed when nobody trades. The finalized watermark advances
    // only after reconciliation, so use the newer of those heads to judge service progress.
    const progressSlot =
        meta?.finalizedSlot === null || meta?.finalizedSlot === undefined
            ? (meta?.indexerSlot ?? null)
            : Math.max(meta.finalizedSlot, meta.indexerSlot ?? 0);
    const state = freshness({
        indexerSlot: progressSlot,
        rpcSlot: slot.slot,
        ageMs,
        indexerReachable: !status.isError,
        awaitingActivity:
            config.cluster === 'devnet' &&
            status.isSuccess &&
            status.data !== undefined &&
            now - status.data.meta.receivedAt <= 60_000 &&
            devnetConfig.data?.exists === false,
    });
    return {
        state,
        lagS: state === 'idle' ? null : lagSeconds(progressSlot, slot.slot),
        ageS: meta ? Math.round(ageMs / 1000) : null,
    };
}

const TEXT: Record<Freshness, string> = {
    live: 'Live',
    lagging: 'Lagging',
    stale: 'Stale',
    down: 'Indexer unavailable',
    unknown: 'Freshness unknown',
    idle: 'Awaiting activity',
};

/** The `stale-badge` of an indexer-backed panel; renders nothing while the data is live. */
export function FreshnessBadge() {
    const { state, lagS, ageS } = useIndexerFreshness();
    if (state === 'live') return null;
    const seconds = state === 'idle' ? null : (lagS ?? ageS);
    return (
        <span className="badge stale-badge" data-testid={T.staleBadge} data-state={state}>
            {TEXT[state]}
            {seconds !== null && state !== 'down' && state !== 'unknown' && <span className="num"> +{seconds}s</span>}
        </span>
    );
}
