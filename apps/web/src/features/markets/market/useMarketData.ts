/**
 * The market page's data (FRONTEND.md §3.4, §6.5): the indexer's detail and the chain's `Market`,
 * merged into a `MarketView`; the protocol state that turns into flags (venue bits and the upgrade
 * latch from `/v1/venues`, else from the chain), the chain-only state (the EMA warm-up, PumpSwap's
 * graduation halt on a curve market) and the live tier terms for the rate.
 */
import type { GlobalConfig } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { ApiError } from '../../../api/client.ts';
import { indexerQueries } from '../../../api/queries.ts';
import { useChainConfig, useChainMarket, useChainOracle, useChainVenueGuard } from '../../../rpc/queries.ts';
import { usePammCreatePoolHalted } from '../shared/chain.ts';
import {
    CONFIG_FLAG_PAUSED_OPENS,
    type MarketFlagKey,
    marketFlags,
    upgradedFromGuards,
} from '../shared/marketFlags.ts';
import { type MarketView, viewFromChain, viewFromIndexer } from './marketView.ts';

export type MarketDataStatus = 'loading' | 'ready' | 'not-found' | 'error';

export interface MarketData {
    readonly status: MarketDataStatus;
    readonly view: MarketView | null;
    readonly flags: readonly MarketFlagKey[];
    readonly config: GlobalConfig | null;
    readonly oracle: { price: bigint; conf: bigint; expo: number; publishTime: number } | null;
    readonly error: unknown;
    readonly retry: () => void;
    /** The indexer answered for this mint with another token's detail: it is ignored (M9-SEC-3). */
    readonly indexerMismatch: boolean;
}

export function useMarketData(mint: Address): MarketData {
    const detail = useQuery(indexerQueries.market(mint));
    const chain = useChainMarket(mint);
    const venues = useQuery(indexerQueries.venues());
    const configQ = useChainConfig();
    const pools = useQuery({
        ...indexerQueries.marketPools(mint),
        enabled: detail.isSuccess && detail.data.data.mint === mint,
    });
    const guardQ = useChainVenueGuard(venues.isError);
    const oracleQ = useChainOracle();

    const chainMarket = chain.data?.exists ? chain.data.data : null;
    const config = configQ.data?.exists ? configQ.data.data : null;
    // A detail about another mint is an indexer error: the page falls back to the chain's view and
    // says so.
    const served = detail.data?.data ?? null;
    const indexerMismatch = !!served && served.mint !== mint;
    const trusted = served && !indexerMismatch ? served : null;
    const view = trusted
        ? viewFromIndexer(trusted, chainMarket, mint)
        : chainMarket
          ? viewFromChain(mint, chainMarket)
          : null;
    const halted = usePammCreatePoolHalted(view?.venue === 'pump_curve');

    const venueStatus = venues.data?.data ?? null;
    const guard = guardQ.data?.exists ? guardQ.data.data : null;
    const flags = view
        ? marketFlags({
              venue: view.venue,
              flags: view.flags,
              globalPausedOpens: config
                  ? (config.flags & CONFIG_FLAG_PAUSED_OPENS) !== 0
                  : (venueStatus?.paused_opens ?? undefined),
              routeBrokenBits: config ? config.venueRouteBroken : (venueStatus?.route_broken_bits ?? undefined),
              upgradedBits: venueStatus ? upgradedFromGuards(venueStatus.guards) : (guard?.upgraded ?? undefined),
              emaWarm: view.emaWarm ?? undefined,
              pammCreatePoolDisabled: halted.data ?? undefined,
              poolWindowOpen: pools.data ? pools.data.data.window !== null : undefined,
          })
        : [];

    const oracleAcc = oracleQ.data?.exists ? oracleQ.data.data : null;
    const oracle = oracleAcc
        ? {
              price: oracleAcc.price,
              conf: oracleAcc.conf,
              expo: oracleAcc.expo,
              publishTime: Number(oracleAcc.publishTime),
          }
        : null;

    const indexerMissing = detail.error instanceof ApiError && detail.error.status === 404;
    let status: MarketDataStatus;
    if (view) status = 'ready';
    else if (detail.isPending || chain.isPending) status = 'loading';
    else if (chain.isSuccess && !chain.data.exists && (indexerMissing || detail.isError || indexerMismatch))
        status = 'not-found';
    else status = 'error';

    return {
        status,
        view,
        flags,
        config,
        oracle,
        error: chain.error ?? detail.error,
        retry: () => {
            void detail.refetch();
            void chain.refetch();
        },
        indexerMismatch,
    };
}
