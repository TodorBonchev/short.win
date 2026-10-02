/**
 * Chain-account queries only the MARKETS pages need (FRONTEND.md §6.2 `['chain', …]` keys, §6.5).
 * The protocol accounts every page shares (`Market`, `GlobalConfig`, `VenueGuard`, `OracleCache`)
 * are read through `src/rpc/queries.ts`.
 */
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { qk } from '../../../api/keys.ts';
import { useAppClient } from '../../../rpc/context.tsx';
import { PUMP_AMM_GLOBAL_CONFIG, pammCreatePoolDisabled, readRawAccount, readRentMinimums } from './chainReads.ts';

const DISPLAY = { staleTime: 2_000, refetchInterval: 15_000, retry: 1 } as const;

/** A raw account (owner, lamports, bytes), e.g. a mint for the safety verdict. */
export function useRawAccount(addr: Address | null) {
    const { rpc } = useAppClient();
    return useQuery({
        queryKey: [...qk.account(addr ?? ''), 'raw'],
        queryFn: () => readRawAccount(rpc, addr as Address),
        enabled: addr !== null,
        staleTime: 0,
        retry: 1,
    });
}

/** PumpSwap pool creation halted (`disable_flags` bit 0); only read for curve markets. */
export function usePammCreatePoolHalted(enabled: boolean) {
    const { rpc } = useAppClient();
    return useQuery({
        ...DISPLAY,
        queryKey: [...qk.account(PUMP_AMM_GLOBAL_CONFIG), 'pamm-disable-flags'],
        queryFn: async () => pammCreatePoolDisabled(await readRawAccount(rpc, PUMP_AMM_GLOBAL_CONFIG)),
        enabled,
    });
}

/** Rent minimums for `sizes` (lamports, same order). */
export function useRentMinimums(sizes: readonly number[], enabled = true) {
    const { rpc } = useAppClient();
    return useQuery({
        queryKey: [...qk.chain, 'rent', sizes.join(',')],
        queryFn: () => readRentMinimums(rpc, sizes),
        enabled: enabled && sizes.length > 0,
        staleTime: 10 * 60_000,
    });
}
