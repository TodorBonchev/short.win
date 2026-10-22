/**
 * Shared chain-account queries (FRONTEND.md §6.2 `['chain', …]` keys, §6.5). One hook per
 * protocol account that more than one page shows, so every page stores the same `MaybeAccount<…>`
 * shape (from `rpc/accounts.ts`) under the same key and the cache never holds two shapes for one
 * key. Display cadence; previews behind a transaction read their own accounts at the moment they
 * build (FE-D1) and hang their keys below these (e.g. `[...qk.marketAccount(mint), 'lend', owner]`),
 * so a prefix invalidation after a confirmed transaction refreshes both.
 */
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { qk } from '../api/keys.ts';
import { readGlobalConfig, readMarket, readOracle, readVenueGuard } from './accounts.ts';
import { useAppClient } from './context.tsx';

const DISPLAY = { staleTime: 2_000, refetchInterval: 15_000, retry: 1 } as const;

/** The `Market` account of `mint` (a `MaybeAccount`; `qk.marketAccount`). */
export function useChainMarket(mint: Address | null) {
    const { rpc } = useAppClient();
    return useQuery({
        ...DISPLAY,
        queryKey: qk.marketAccount(mint ?? ''),
        queryFn: () => readMarket(rpc, mint as Address),
        enabled: mint !== null,
    });
}

/** `GlobalConfig` (live tier terms, the creation fee, venue bits, the global pause; `qk.config`). */
export function useChainConfig(enabled = true) {
    const { rpc } = useAppClient();
    return useQuery({
        ...DISPLAY,
        refetchInterval: 60_000,
        queryKey: qk.config(),
        queryFn: () => readGlobalConfig(rpc),
        enabled,
    });
}

/** `VenueGuard` (the upgrade latch; `qk.venueGuard`). */
export function useChainVenueGuard(enabled = true) {
    const { rpc } = useAppClient();
    return useQuery({ ...DISPLAY, queryKey: qk.venueGuard(), queryFn: () => readVenueGuard(rpc), enabled });
}

/** The SOL/USD `OracleCache` (tier inputs; `qk.oracle`). */
export function useChainOracle(enabled = true) {
    const { rpc } = useAppClient();
    return useQuery({ ...DISPLAY, queryKey: qk.oracle(), queryFn: () => readOracle(rpc), enabled });
}
