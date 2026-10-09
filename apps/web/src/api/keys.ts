/**
 * TanStack Query keys (FRONTEND.md §6.2). Two roots: `indexer` (REST reads, invalidated by the
 * WebSocket) and `chain` (RPC account reads, invalidated after a confirmed transaction and by
 * account subscriptions). Every key is built here; no component writes a key literal.
 */
import type { MarketSort, PositionStatus, SortOrder, TierName, VenueName } from './types.ts';

export interface MarketsParams {
    readonly q?: string;
    readonly venue?: VenueName;
    readonly tier?: TierName;
    readonly has_positions?: boolean;
    readonly sort?: MarketSort;
    readonly order?: SortOrder;
    readonly limit?: number;
    /** Keyset paging: the previous page's `next_cursor` (absent on the first page). */
    readonly cursor?: string;
}

/** A page of `/v1/markets/{mint}/lenders` by the lender's epoch against the market's share epoch. */
export interface LendersPageParams {
    readonly epoch: 'behind' | 'current';
    readonly limit: number;
    /** Keyset paging: the previous page's `next_cursor` (`null` on the first page). */
    readonly cursor: string | null;
}

export const qk = {
    indexer: ['indexer'] as const,
    status: () => ['indexer', 'status'] as const,
    health: () => ['indexer', 'health'] as const,
    search: (q: string) => ['indexer', 'search', q] as const,
    markets: (p: MarketsParams) => ['indexer', 'markets', p] as const,
    market: (mint: string) => ['indexer', 'market', mint] as const,
    marketPositions: (mint: string, status?: PositionStatus | 'all', borrower?: string) =>
        ['indexer', 'market', mint, 'positions', status ?? 'all', borrower ?? null] as const,
    marketLenders: (mint: string) => ['indexer', 'market', mint, 'lenders'] as const,
    /** One page of the lenders filtered by `epoch` (the claim's candidates; under `marketLenders`). */
    marketLendersPage: (mint: string, p: LendersPageParams) =>
        ['indexer', 'market', mint, 'lenders', p.epoch, p.limit, p.cursor] as const,
    marketPools: (mint: string) => ['indexer', 'market', mint, 'pools'] as const,
    marketCoverage: (mint: string) => ['indexer', 'market', mint, 'coverage'] as const,
    marketLiquidations: (mint: string) => ['indexer', 'market', mint, 'liquidations'] as const,
    marketEvents: (mint: string) => ['indexer', 'market', mint, 'events'] as const,
    marketSnapshots: (mint: string) => ['indexer', 'market', mint, 'snapshots'] as const,
    marketTrades: (mint: string) => ['indexer', 'market', mint, 'trades'] as const,
    marketCandles: (mint: string, interval: string) => ['indexer', 'market', mint, 'candles', interval] as const,
    marketStats: (mint: string) => ['indexer', 'market', mint, 'stats'] as const,
    position: (pubkey: string) => ['indexer', 'position', pubkey] as const,
    borrowerPositions: (owner: string, status?: PositionStatus | 'all') =>
        ['indexer', 'borrower', owner, 'positions', status ?? 'all'] as const,
    lender: (owner: string) => ['indexer', 'lender', owner] as const,
    lenderEvents: (owner: string) => ['indexer', 'lender', owner, 'events'] as const,
    stats: () => ['indexer', 'stats'] as const,
    liquidations: () => ['indexer', 'liquidations'] as const,
    settlements: () => ['indexer', 'settlements'] as const,
    keepers: () => ['indexer', 'keepers'] as const,
    adminConfig: () => ['indexer', 'admin', 'config'] as const,
    adminChanges: () => ['indexer', 'admin', 'changes'] as const,
    adminEvents: () => ['indexer', 'admin', 'events'] as const,
    venues: () => ['indexer', 'venues'] as const,
    tx: (signature: string) => ['indexer', 'tx', signature] as const,

    chain: ['chain'] as const,
    slot: () => ['chain', 'slot'] as const,
    account: (address: string) => ['chain', 'account', address] as const,
    config: () => ['chain', 'config'] as const,
    venueGuard: () => ['chain', 'venue-guard'] as const,
    oracle: () => ['chain', 'oracle'] as const,
    marketAccount: (mint: string) => ['chain', 'market', mint] as const,
    marketVault: (mint: string) => ['chain', 'market', mint, 'vault'] as const,
    venueAccounts: (mint: string) => ['chain', 'market', mint, 'venue'] as const,
    lenderAccount: (mint: string, owner: string) => ['chain', 'market', mint, 'lender', owner] as const,
    positionAccount: (pubkey: string) => ['chain', 'position', pubkey] as const,
    exposure: (mint: string, owner: string) => ['chain', 'market', mint, 'exposure', owner] as const,
    balance: (owner: string) => ['chain', 'balance', owner] as const,
    tokenAccounts: (owner: string, mint: string) => ['chain', 'token-accounts', owner, mint] as const,
    mint: (mint: string) => ['chain', 'mint', mint] as const,
} as const;
