/**
 * Keyset paging over `/v1/markets` (FRONTEND.md §3.3 "Load more"). Every page is its own query
 * under `['indexer', 'markets', params+cursor]` with the plain `Page<MarketSummary>` shape, so the
 * `markets` WebSocket channel patches rows in place on every loaded page (`applyWsMessage`, §6.4).
 * Changing the filters or the sort starts again from the first page.
 */
import { useQueries } from '@tanstack/react-query';
import { useCallback, useState } from 'react';
import type { MarketsParams } from '../../../api/keys.ts';
import { indexerQueries } from '../../../api/queries.ts';
import type { MarketSummary } from '../../../api/types.ts';

/** One page: the list's params plus the previous page's `next_cursor` (null on the first page). */
export function marketsPageQuery(params: MarketsParams, cursor: string | null) {
    return indexerQueries.markets(cursor === null ? params : { ...params, cursor });
}

export interface MarketPages {
    /** Rows of every loaded page, in order, each mint once. */
    readonly rows: readonly MarketSummary[] | undefined;
    readonly loading: boolean;
    readonly loadingMore: boolean;
    readonly error: unknown;
    readonly hasMore: boolean;
    readonly pageCount: number;
    loadMore(): void;
    refetch(): void;
}

export function useMarketPages(params: MarketsParams): MarketPages {
    const key = JSON.stringify(params);
    const [state, setState] = useState<{ key: string; cursors: string[] }>({ key, cursors: [] });
    const cursors = state.key === key ? state.cursors : [];
    const results = useQueries({
        queries: [null, ...cursors].map((c) => marketsPageQuery(params, c)),
    });
    const first = results[0];
    const last = results[results.length - 1];
    let rows: MarketSummary[] | undefined;
    if (first?.data) {
        const seen = new Set<string>();
        rows = [];
        for (const r of results) {
            for (const m of r.data?.data.data ?? []) {
                if (seen.has(m.mint)) continue;
                seen.add(m.mint);
                rows.push(m);
            }
        }
    }
    const nextCursor = last?.data?.data.next_cursor ?? null;
    const loadMore = useCallback(() => {
        if (!nextCursor) return;
        setState((s) => {
            const base = s.key === key ? s.cursors : [];
            return base.includes(nextCursor) ? s : { key, cursors: [...base, nextCursor] };
        });
    }, [key, nextCursor]);
    return {
        rows,
        loading: !!first?.isPending,
        loadingMore: results.length > 1 && !!last?.isPending,
        error: first?.error ?? null,
        hasMore: nextCursor !== null && !last?.isPending,
        pageCount: results.length,
        loadMore,
        refetch: () => {
            for (const r of results) void r.refetch();
        },
    };
}
