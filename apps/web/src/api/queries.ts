/**
 * Query options for every indexer endpoint the app reads (FRONTEND.md §6.1–§6.3). Components use
 * `useQuery(indexerQueries.market(mint))`; the cached value is an `ApiResult` (data + slot meta).
 * Retries: transient errors (429, 503, network) back off with the server's `Retry-After`; 4xx
 * other than 429 are not retried.
 */
import { queryOptions } from '@tanstack/react-query';
import { config } from '../config/env.ts';
import { createStore } from '../lib/store.ts';
import { ApiError, type ApiMeta, type ApiResult, createApiClient, keySegment, signatureSegment } from './client.ts';
import { type LendersPageParams, type MarketsParams, qk } from './keys.ts';
import type {
    ExitEvent,
    GlobalStats,
    IndexerEvent,
    IngestStatus,
    LenderDetail,
    LenderPosition,
    MarketCoverage,
    MarketDetail,
    MarketPools,
    MarketSummary,
    Page,
    Position,
    PositionDetail,
    PositionStatus,
    ProtocolConfig,
    SearchHit,
    VenueStatus,
} from './types.ts';

/** The newest indexer slot meta seen on any response (status bar, staleness badges). */
export const indexerMeta = createStore<ApiMeta | null>(null);

export const api = createApiClient(config.indexerUrl, {
    onMeta: (m) => indexerMeta.set((prev) => (prev && (prev.indexerSlot ?? 0) > (m.indexerSlot ?? 0) ? prev : m)),
});

export function retryIndexer(failureCount: number, error: unknown): boolean {
    if (error instanceof ApiError) return error.transient && failureCount < 4;
    return failureCount < 2;
}

export function retryDelayIndexer(failureCount: number, error: unknown): number {
    if (error instanceof ApiError && error.retryAfterS !== null) return error.retryAfterS * 1000;
    return Math.min(1000 * 2 ** failureCount, 15_000);
}

const base = { retry: retryIndexer, retryDelay: retryDelayIndexer } as const;

export const indexerQueries = {
    status: () =>
        queryOptions({
            ...base,
            queryKey: qk.status(),
            queryFn: ({ signal }) => api.get<IngestStatus>('/v1/status', undefined, signal),
            refetchInterval: 10_000,
        }),
    search: (q: string) =>
        queryOptions({
            ...base,
            queryKey: qk.search(q),
            queryFn: ({ signal }) => api.get<{ data: SearchHit[] }>('/v1/search', { q }, signal),
            enabled: q.trim().length > 0 && q.length <= 64,
            staleTime: 30_000,
        }),
    markets: (p: MarketsParams) =>
        queryOptions({
            ...base,
            queryKey: qk.markets(p),
            queryFn: ({ signal }) =>
                api.get<Page<MarketSummary>>(
                    '/v1/markets',
                    {
                        q: p.q,
                        venue: p.venue,
                        tier: p.tier,
                        has_positions: p.has_positions,
                        sort: p.sort,
                        order: p.order,
                        limit: p.limit,
                        cursor: p.cursor,
                    },
                    signal,
                ),
            staleTime: 2_000,
        }),
    market: (mint: string) =>
        queryOptions({
            ...base,
            queryKey: qk.market(mint),
            queryFn: ({ signal }) => api.get<MarketDetail>(`/v1/markets/${keySegment(mint)}`, undefined, signal),
            staleTime: 2_000,
        }),
    marketPositions: (mint: string, status: PositionStatus | 'all' = 'open') =>
        queryOptions({
            ...base,
            queryKey: qk.marketPositions(mint, status),
            queryFn: ({ signal }) =>
                api.get<Page<Position>>(
                    `/v1/markets/${keySegment(mint)}/positions`,
                    { status: status === 'all' ? undefined : status },
                    signal,
                ),
        }),
    marketLenders: (mint: string) =>
        queryOptions({
            ...base,
            queryKey: qk.marketLenders(mint),
            queryFn: ({ signal }) =>
                api.get<Page<LenderPosition>>(`/v1/markets/${keySegment(mint)}/lenders`, undefined, signal),
        }),
    marketLendersPage: (mint: string, p: LendersPageParams) =>
        queryOptions({
            ...base,
            queryKey: qk.marketLendersPage(mint, p),
            queryFn: ({ signal }) =>
                api.get<Page<LenderPosition>>(
                    `/v1/markets/${keySegment(mint)}/lenders`,
                    { epoch: p.epoch, limit: p.limit, cursor: p.cursor ?? undefined },
                    signal,
                ),
        }),
    marketPools: (mint: string) =>
        queryOptions({
            ...base,
            queryKey: qk.marketPools(mint),
            queryFn: ({ signal }) => api.get<MarketPools>(`/v1/markets/${keySegment(mint)}/pools`, undefined, signal),
        }),
    marketCoverage: (mint: string) =>
        queryOptions({
            ...base,
            queryKey: qk.marketCoverage(mint),
            queryFn: ({ signal }) =>
                api.get<MarketCoverage>(`/v1/markets/${keySegment(mint)}/coverage`, undefined, signal),
        }),
    marketLiquidations: (mint: string) =>
        queryOptions({
            ...base,
            queryKey: qk.marketLiquidations(mint),
            queryFn: ({ signal }) =>
                api.get<Page<ExitEvent>>(`/v1/markets/${keySegment(mint)}/liquidations`, { limit: 50 }, signal),
        }),
    marketEvents: (mint: string) =>
        queryOptions({
            ...base,
            queryKey: qk.marketEvents(mint),
            queryFn: ({ signal }) =>
                api.get<Page<IndexerEvent>>(`/v1/markets/${keySegment(mint)}/events`, { limit: 100 }, signal),
        }),
    position: (pubkey: string) =>
        queryOptions({
            ...base,
            queryKey: qk.position(pubkey),
            queryFn: ({ signal }) => api.get<PositionDetail>(`/v1/positions/${keySegment(pubkey)}`, undefined, signal),
        }),
    borrowerPositions: (owner: string, status: PositionStatus | 'all' = 'open') =>
        queryOptions({
            ...base,
            queryKey: qk.borrowerPositions(owner, status),
            queryFn: ({ signal }) =>
                api.get<Page<Position>>(
                    `/v1/borrowers/${keySegment(owner)}/positions`,
                    { status: status === 'all' ? undefined : status, limit: 200 },
                    signal,
                ),
        }),
    lender: (owner: string) =>
        queryOptions({
            ...base,
            queryKey: qk.lender(owner),
            queryFn: ({ signal }) => api.get<LenderDetail>(`/v1/lenders/${keySegment(owner)}`, undefined, signal),
        }),
    stats: () =>
        queryOptions({
            ...base,
            queryKey: qk.stats(),
            queryFn: ({ signal }) => api.get<GlobalStats>('/v1/stats', undefined, signal),
            refetchInterval: 10_000,
        }),
    liquidations: () =>
        queryOptions({
            ...base,
            queryKey: qk.liquidations(),
            queryFn: ({ signal }) => api.get<Page<ExitEvent>>('/v1/liquidations', { limit: 50 }, signal),
        }),
    adminConfig: () =>
        queryOptions({
            ...base,
            queryKey: qk.adminConfig(),
            queryFn: ({ signal }) => api.get<ProtocolConfig>('/v1/admin/config', undefined, signal),
            staleTime: 60_000,
        }),
    venues: () =>
        queryOptions({
            ...base,
            queryKey: qk.venues(),
            queryFn: ({ signal }) => api.get<VenueStatus>('/v1/venues', undefined, signal),
            refetchInterval: 30_000,
        }),
    tx: (signature: string) =>
        queryOptions({
            ...base,
            queryKey: qk.tx(signature),
            queryFn: ({ signal }) =>
                api.get<{ signature: string; ok: boolean; err_code: number | null; events: IndexerEvent[] }>(
                    `/v1/tx/${signatureSegment(signature)}`,
                    undefined,
                    signal,
                ),
        }),
};

export type { ApiResult };
