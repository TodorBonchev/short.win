/**
 * SHORT track — the borrower's positions (FRONTEND.md §3.8, §6.5): the list from the indexer
 * (`/v1/borrowers/{owner}/positions`), or — while the indexer is down — the open positions from
 * the chain (`getProgramAccounts` on `ShortPosition` with the borrower at offset 8); every open
 * position's account and its market's snapshot read through RPC, so the live columns (CR,
 * liquidation price, P&L, rent, risk) are the chain's (FE-D1).
 *
 * The market a position is priced on is the chain's (M9-SEC-2): the position account names its
 * `market`, the `Market` account at that address names the mint, and that mint's snapshot must be
 * of that very market. The indexer's mint and market only list and label the row; when they
 * disagree with the chain the row is flagged (`mismatch`) and no metric is shown from them.
 */
import {
    fetchAllMaybeShortPosition,
    getMarketDecoder,
    getShortPositionDecoder,
    getShortPositionSize,
    type ShortPosition,
} from '@shortwin/client';
import type { Address, Base58EncodedBytes } from '@solana/kit';
import { getBase64Encoder } from '@solana/kit';
import { useQueries, useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { indexerQueries } from '../../../api/queries.ts';
import type { Position, PositionStatus } from '../../../api/types.ts';
import { SHORTWIN_PROGRAM } from '../../../config/programs.ts';
import type { MathModule } from '../../../math/load.ts';
import { useMath } from '../../../math/useMath.ts';
import { useAppClient } from '../../../rpc/context.tsx';
import { fetchRaw } from '../../../venues/fetch.ts';
import { SNAPSHOT_REFRESH_MS, snapshotKey } from '../hooks.ts';
import { type PositionMetrics, positionMetrics } from '../previews.ts';
import { type ChainSnapshot, loadSnapshot } from '../snapshot.ts';

export type StatusFilter = PositionStatus | 'all';

/** One row of the dashboard: the indexer's record (or the chain's), and the live metrics when open. */
export interface PositionRowData {
    readonly position: Address;
    readonly mint: Address | null;
    readonly market: Address;
    readonly status: PositionStatus;
    readonly api: Position | null;
    readonly chain: ShortPosition | null;
    readonly snapshot: ChainSnapshot | null;
    readonly metrics: PositionMetrics | null;
    /** The indexer's market or mint for this position disagrees with the chain's (M9-SEC-2). */
    readonly mismatch: boolean;
}

/** The mint of each `Market` account (by address), read from the chain. */
async function readMarketMints(
    rpc: ReturnType<typeof useAppClient>['rpc'],
    markets: readonly Address[],
): Promise<Map<Address, Address>> {
    const { accounts } = await fetchRaw(rpc, markets);
    const out = new Map<Address, Address>();
    for (const [k, a] of accounts) {
        if (a && a.owner === SHORTWIN_PROGRAM) {
            try {
                out.set(k, getMarketDecoder().decode(a.data).mint);
            } catch {
                // not a market: no mint, no metrics
            }
        }
    }
    return out;
}

const b64 = getBase64Encoder();

/** The open positions of `owner` from the chain (indexer fallback). */
async function chainPositions(
    rpc: ReturnType<typeof useAppClient>['rpc'],
    owner: Address,
): Promise<{ position: Address; data: ShortPosition; mint: Address | null }[]> {
    const res = (await rpc
        .getProgramAccounts(SHORTWIN_PROGRAM, {
            encoding: 'base64',
            commitment: 'confirmed',
            filters: [
                { dataSize: BigInt(getShortPositionSize()) },
                { memcmp: { offset: 8n, bytes: owner as unknown as Base58EncodedBytes, encoding: 'base58' } },
            ],
        })
        .send()) as unknown as readonly { pubkey: Address; account: { data: readonly [string, string] } }[];
    const dec = getShortPositionDecoder();
    const rows = res.map((r) => ({
        position: r.pubkey,
        data: dec.decode(new Uint8Array(b64.encode(r.account.data[0]))),
    }));
    const mintOf = await readMarketMints(rpc, [...new Set(rows.map((r) => r.data.market))]);
    return rows.map((r) => ({ ...r, mint: mintOf.get(r.data.market) ?? null }));
}

export function usePositions(owner: Address | null, status: StatusFilter) {
    const client = useAppClient();
    const { math } = useMath();
    const api = useQuery({ ...indexerQueries.borrowerPositions(owner ?? 'none', status), enabled: !!owner });
    const fallback = useQuery({
        queryKey: ['chain', 'borrower-positions', owner ?? 'none'] as const,
        queryFn: () => (owner ? chainPositions(client.rpc, owner) : Promise.resolve([])),
        enabled: !!owner && api.isError,
        staleTime: 5_000,
        structuralSharing: false,
    });
    const usingChain = api.isError;

    // The open positions whose accounts and markets are read from the chain.
    const open: { position: Address; mint: Address | null; market: Address; api: Position | null }[] = useMemo(() => {
        if (usingChain) {
            return (fallback.data ?? []).map((r) => ({
                position: r.position,
                mint: r.mint,
                market: r.data.market,
                api: null,
            }));
        }
        return (api.data?.data.data ?? [])
            .filter((p) => p.status === 'open')
            .map((p) => ({
                position: p.position as Address,
                mint: p.mint as Address,
                market: p.market as Address,
                api: p,
            }));
    }, [usingChain, fallback.data, api.data]);

    const openKeys = open.map((o) => o.position);
    const accounts = useQuery({
        queryKey: ['chain', 'positions', ...openKeys] as const,
        queryFn: async () => {
            const all = await fetchAllMaybeShortPosition(client.rpc, openKeys, { commitment: 'confirmed' });
            return new Map(all.map((a) => [a.address, a.exists ? a.data : null] as const));
        },
        enabled: openKeys.length > 0,
        staleTime: 0,
        refetchInterval: SNAPSHOT_REFRESH_MS * 2,
        structuralSharing: false,
    });

    // The markets the position accounts name, and their mints, from the chain (M9-SEC-2).
    const chainMarkets = [
        ...new Set([...(accounts.data?.values() ?? [])].filter((p) => p !== null).map((p) => p.market)),
    ].sort();
    const marketMints = useQuery({
        queryKey: ['chain', 'market-mints', ...chainMarkets] as const,
        queryFn: () => readMarketMints(client.rpc, chainMarkets),
        enabled: chainMarkets.length > 0,
        staleTime: Number.POSITIVE_INFINITY, // a market's mint never changes
        structuralSharing: false,
    });
    const mints = [...new Set(marketMints.data?.values() ?? [])];
    const snapshots = useQueries({
        queries: mints.map((mint) => ({
            queryKey: snapshotKey(mint),
            queryFn: async ({ signal }: { signal: AbortSignal }) => {
                if (!math) throw new Error('math module not loaded');
                return (await loadSnapshot(client.rpc, math, mint, [], signal)).snapshot;
            },
            enabled: !!math,
            staleTime: 0,
            refetchInterval: SNAPSHOT_REFRESH_MS,
            structuralSharing: false,
        })),
    });
    const snapByMint = new Map<Address, ChainSnapshot | null>();
    mints.forEach((m, i) => {
        snapByMint.set(m, snapshots[i]?.data ?? null);
    });

    const rows = buildRows({
        math,
        usingChain,
        api: api.data?.data.data ?? [],
        chainOpen: fallback.data ?? [],
        accounts: accounts.data ?? null,
        marketMints: marketMints.data ?? null,
        snapshots: snapByMint,
        status,
    });

    return {
        rows,
        usingChain,
        loading: api.isLoading || (usingChain && fallback.isLoading),
        error: usingChain ? fallback.error : null,
        /** When the chain reads behind the live columns were made (ms; staleness, §6.6). */
        readAt: accounts.dataUpdatedAt || null,
        refetch: () => {
            void api.refetch();
            void accounts.refetch();
        },
    };
}

/** Assembles the rows (pure; exported for tests). */
export function buildRows(inp: {
    readonly math: MathModule | undefined;
    readonly usingChain: boolean;
    readonly api: readonly Position[];
    readonly chainOpen: readonly { position: Address; data: ShortPosition; mint: Address | null }[];
    readonly accounts: ReadonlyMap<Address, ShortPosition | null> | null;
    /** `Market` address → mint, read from the chain (the only mint the live columns are priced on). */
    readonly marketMints?: ReadonlyMap<Address, Address> | null;
    readonly snapshots: ReadonlyMap<Address, ChainSnapshot | null>;
    readonly status: StatusFilter;
}): PositionRowData[] {
    /**
     * The live part of an open row: the chain's position, the mint of ITS market from the chain,
     * and that mint's snapshot only if the snapshot is of that market. What the indexer said
     * (`claimed`) never selects the snapshot; it only raises `mismatch` when it disagrees.
     */
    const live = (
        position: Address,
        claimed: { readonly mint: Address | null; readonly market: Address | null },
        fallback: { readonly chain: ShortPosition | null; readonly mint: Address | null },
    ) => {
        const chain = inp.accounts?.get(position) ?? fallback.chain;
        const chainMint = chain ? (inp.marketMints?.get(chain.market) ?? fallback.mint) : null;
        const found = chainMint ? (inp.snapshots.get(chainMint) ?? null) : null;
        const snapshot = found && chain && found.marketAddress === chain.market ? found : null;
        let metrics: PositionMetrics | null = null;
        if (inp.math && chain && snapshot) {
            try {
                metrics = positionMetrics(inp.math, snapshot, chain);
            } catch {
                metrics = null;
            }
        }
        const mismatch =
            (!!chain && claimed.market !== null && claimed.market !== chain.market) ||
            (chainMint !== null && claimed.mint !== null && claimed.mint !== chainMint) ||
            (!!found && !!chain && found.marketAddress !== chain.market);
        return {
            chain,
            snapshot,
            metrics,
            mismatch,
            mint: chainMint ?? claimed.mint,
            market: chain?.market ?? claimed.market,
        };
    };
    if (inp.usingChain) {
        if (inp.status !== 'open' && inp.status !== 'all') return [];
        return inp.chainOpen.map((r) => {
            const l = live(r.position, { mint: null, market: null }, { chain: r.data, mint: r.mint });
            return {
                position: r.position,
                status: 'open' as const,
                api: null,
                ...l,
                market: l.market ?? r.data.market,
            };
        });
    }
    return inp.api.map((p) => {
        const position = p.position as Address;
        const base = { position, mint: p.mint as Address, market: p.market as Address, status: p.status, api: p };
        if (p.status !== 'open') return { ...base, chain: null, snapshot: null, metrics: null, mismatch: false };
        const l = live(position, { mint: base.mint, market: base.market }, { chain: null, mint: null });
        return { ...base, ...l, market: l.market ?? base.market };
    });
}

/** The live CR of one open position and the liquidation CR locked into it (market page, §3.4). */
export interface MarketPositionLive {
    readonly crBps: bigint | null;
    readonly crEmaBps: bigint | null;
    readonly crLiqBps: number;
}

/**
 * The live CR of a market's open positions (the market page's Positions tab, FRONTEND.md §3.4):
 * their `ShortPosition` accounts in one `getMultipleAccounts` and the market's snapshot (shared with
 * the Short panel under `snapshotKey(mint)`), through `positionMetrics` — the same numbers as the
 * positions dashboard.
 */
export function useMarketPositionsLive(
    mint: Address,
    positions: readonly Address[],
): ReadonlyMap<Address, MarketPositionLive> {
    const client = useAppClient();
    const { math } = useMath();
    const accounts = useQuery({
        queryKey: ['chain', 'positions', ...positions] as const,
        queryFn: async () => {
            const all = await fetchAllMaybeShortPosition(client.rpc, [...positions], { commitment: 'confirmed' });
            return new Map(all.map((a) => [a.address, a.exists ? a.data : null] as const));
        },
        enabled: positions.length > 0,
        staleTime: 0,
        refetchInterval: SNAPSHOT_REFRESH_MS * 2,
        structuralSharing: false,
    });
    const snapshot = useQuery({
        queryKey: snapshotKey(mint),
        queryFn: async ({ signal }) => {
            if (!math) throw new Error('math module not loaded');
            return (await loadSnapshot(client.rpc, math, mint, [], signal)).snapshot;
        },
        enabled: !!math && positions.length > 0,
        staleTime: 0,
        refetchInterval: SNAPSHOT_REFRESH_MS,
        structuralSharing: false,
    });
    return useMemo(() => {
        const out = new Map<Address, MarketPositionLive>();
        const snap = snapshot.data;
        if (!math || !snap || !accounts.data) return out;
        for (const [address, pos] of accounts.data) {
            // Only positions of this very market (the indexer listed them; the chain decides, M9-SEC-2).
            if (!pos || pos.market !== snap.marketAddress) continue;
            let crBps: bigint | null = null;
            let crEmaBps: bigint | null = null;
            try {
                const mx = positionMetrics(math, snap, pos);
                crBps = mx.crBps;
                crEmaBps = mx.crEmaBps;
            } catch {
                crBps = null;
            }
            out.set(address, { crBps, crEmaBps, crLiqBps: pos.crLiqBps });
        }
        return out;
    }, [math, snapshot.data, accounts.data]);
}
