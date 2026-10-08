/**
 * The lender dashboard's rows (FRONTEND.md §3.8): the markets the wallet lends in come from the
 * indexer (`/v1/lenders/{owner}`), or — while it is unavailable — from `getProgramAccounts` over
 * the owner's `LenderPosition`s (FRONTEND.md §6.5); every number comes from the chain through the
 * WASM previews (FE-D1). Each row also says where the lender stands against the share epoch and
 * whether the indexer's view of it is in sync.
 */
import { getLenderPositionSize } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { useQueries, useQuery } from '@tanstack/react-query';
import { indexerQueries } from '../../../api/queries.ts';
import type { LenderPosition as ApiLender, MarketDetail } from '../../../api/types.ts';
import { shortAddress } from '../../../lib/format.ts';
import type { MathModule } from '../../../math/load.ts';
import { useAppClient } from '../../../rpc/context.tsx';
import { chainNowSeconds, type LendChain, lendKeys, readLendChain, readOnChainLenderMarkets } from '../chain.ts';
import type { LoadedChain } from '../ctx.ts';
import { type PositionView, previewPosition, type Standing, standingOf } from '../previews.ts';

export type IndexerState = 'ok' | 'unsynced' | 'quarantined' | 'missing';

export interface LenderRow {
    readonly market: Address;
    readonly mint: Address;
    readonly symbol: string;
    readonly decimals: number;
    /** The chain state (undefined while loading). */
    readonly chain: LoadedChain | undefined;
    readonly chainError: unknown;
    readonly position: PositionView | null;
    readonly standing: Standing;
    readonly indexer: IndexerState;
    readonly isCoinCreator: boolean;
    readonly isMarketCreator: boolean;
}

/** How the indexer's row compares with the chain (display of "quarantined / unsynced" states). */
export function indexerStateOf(
    api: ApiLender | null,
    chain: LendChain | undefined,
    detail: MarketDetail | undefined,
): IndexerState {
    if (!api) return 'missing';
    if (detail && detail.quarantined_events > 0) return 'quarantined';
    if (chain) {
        const onChain = chain.lender ? chain.lender.shares : 0n;
        const sameEpoch = !chain.lender || chain.lender.epoch === api.epoch;
        if (BigInt(api.shares) !== onChain || !sameEpoch) return 'unsynced';
    }
    return 'ok';
}

/** `wallMs`: the wall clock; each row's previews run on its own chain read's cluster clock (M9-UX-5). */
export function useLenderRows(owner: Address | null, math: MathModule | undefined, wallMs: number) {
    const client = useAppClient();
    const idx = useQuery({ ...indexerQueries.lender(owner ?? ''), enabled: !!owner });
    const fallback = useQuery({
        queryKey: lendKeys.onChainPositions(owner ?? ''),
        queryFn: () => readOnChainLenderMarkets(client.rpc, owner as Address, getLenderPositionSize()),
        // While the indexer fails (and while it retries), the chain lists the positions (FRONTEND.md §6.5).
        enabled: !!owner && (idx.isError || idx.failureCount > 0),
        staleTime: 10_000,
    });
    const apiRows = idx.data?.data.positions ?? null;
    const base: { market: Address; mint: Address; api: ApiLender | null }[] = apiRows
        ? apiRows.map((p) => ({ market: p.market as Address, mint: p.mint as Address, api: p }))
        : (fallback.data ?? []).map((r) => ({ ...r, api: null }));

    const chains = useQueries({
        queries: base.map((b) => ({
            queryKey: [...lendKeys.state(b.mint, owner), 'row'] as const,
            queryFn: () => readLendChain(client.rpc, b.mint, owner, { tokenAccounts: false }),
            staleTime: 5_000,
            refetchInterval: 20_000,
            structuralSharing: false,
            enabled: !!owner,
        })),
    });
    const details = useQueries({
        queries: base.map((b) => ({ ...indexerQueries.market(b.mint), enabled: !idx.isError })),
    });

    const rows: LenderRow[] = [];
    base.forEach((b, i) => {
        const c = chains[i]?.data;
        // Display only, and only a detail about this very mint (M9-SEC-3).
        const served = details[i]?.data?.data;
        const detail = served && served.mint === b.mint ? served : undefined;
        // A closed position has no account: `withdraw` of everything or, since KI-67, a `claim_sol`
        // that left it with nothing (a wiped or sub-unit position) closes it. The chain decides
        // (FE-D1): an indexer row that still shows shares is behind on the close.
        if (c && !c.lender) return;
        const loaded = c?.market && c.vault ? (c as LoadedChain) : undefined;
        const pos =
            math && loaded?.lender
                ? previewPosition(
                      math,
                      { market: loaded.market, vault: loaded.vault, lender: loaded.lender, records: loaded.records },
                      BigInt(chainNowSeconds(loaded, wallMs)),
                  )
                : null;
        rows.push({
            market: b.market,
            mint: b.mint,
            symbol: detail?.symbol ?? shortAddress(b.mint),
            decimals: loaded?.market.decimals ?? detail?.decimals ?? 0,
            chain: loaded,
            chainError: chains[i]?.error ?? null,
            position: pos?.ok ? pos.value : null,
            standing: loaded ? standingOf(loaded.market, loaded.lender) : { kind: 'none' },
            indexer: indexerStateOf(b.api, c, detail),
            isCoinCreator: b.api?.is_coin_creator ?? false,
            isMarketCreator: b.api?.is_market_creator ?? false,
        });
    });
    const indexerDown = idx.isError || (!idx.data && idx.failureCount > 0);
    const listing = apiRows ? false : indexerDown ? fallback.isPending : idx.isPending;
    const loading = !!owner && (listing || chains.some((q) => q.isPending));
    return { rows, loading, indexerDown, error: indexerDown && fallback.isError ? fallback.error : null };
}
