/**
 * SHORT track — data hooks (FRONTEND.md §6.3, §6.5): the market snapshot behind every preview
 * (RPC at `confirmed`, refreshed every 2 s while shown and right after a confirmed transaction,
 * which invalidates `qk.marketAccount(mint)`), the wallet's exposure and SOL balance, rent
 * exemptions, the token's display symbol, and a ticking estimate of the cluster clock.
 */
import { fetchMaybeShortPosition, type ShortPosition } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { qk } from '../../api/keys.ts';
import { indexerQueries } from '../../api/queries.ts';
import { cleanName, cleanSymbol, tokenLabel } from '../../lib/displayText.ts';
import { shortAddress } from '../../lib/format.ts';
import { useMath } from '../../math/useMath.ts';
import { readExposure } from '../../rpc/accounts.ts';
import { useAppClient } from '../../rpc/context.tsx';
import { type ChainSnapshot, loadSnapshot } from './snapshot.ts';

/** Previews re-read the chain this often while shown (§6.3: chain accounts behind a preview). */
export const SNAPSHOT_REFRESH_MS = 2_000;

export function snapshotKey(mint: Address) {
    return [...qk.marketAccount(mint), 'short-snapshot'] as const;
}

/** The market's chain snapshot (`null`: no market for this mint; a `null` mint waits). */
export function useShortSnapshot(mint: Address | null, opts: { readonly live?: boolean } = {}) {
    const client = useAppClient();
    const { math, error: mathError } = useMath();
    const q = useQuery({
        queryKey: snapshotKey(mint ?? ('none' as Address)),
        queryFn: async ({ signal }): Promise<ChainSnapshot | null> => {
            if (!math || !mint) throw new Error('math module not loaded');
            return (await loadSnapshot(client.rpc, math, mint, [], signal)).snapshot;
        },
        enabled: !!math && !!mint,
        staleTime: 0,
        refetchInterval: opts.live === false ? false : SNAPSHOT_REFRESH_MS,
        refetchOnWindowFocus: true,
        structuralSharing: false,
        retry: 2,
    });
    return { ...q, math, mathError };
}

/** The wallet's open quantity in the market (`WalletExposure.q_open`). */
export function useExposureQ(mint: Address, owner: Address | null) {
    const client = useAppClient();
    return useQuery({
        queryKey: qk.exposure(mint, owner ?? 'none'),
        queryFn: async () => {
            if (!owner) return 0n;
            const e = await readExposure(client.rpc, mint, owner);
            return e.exists ? e.data.qOpen : 0n;
        },
        enabled: !!owner,
        staleTime: 0,
        refetchInterval: SNAPSHOT_REFRESH_MS * 5,
    });
}

/** The wallet's SOL balance (lamports). */
export function useSolBalance(owner: Address | null) {
    const client = useAppClient();
    return useQuery({
        queryKey: qk.balance(owner ?? 'none'),
        queryFn: async () =>
            owner ? (await client.rpc.getBalance(owner, { commitment: 'confirmed' }).send()).value : 0n,
        enabled: !!owner,
        staleTime: 5_000,
        refetchInterval: 15_000,
    });
}

/** `getMinimumBalanceForRentExemption(space)` (cached: rent changes by feature gate only). */
export function useRentExempt(space: number) {
    const client = useAppClient();
    return useQuery({
        queryKey: ['chain', 'rent-exempt', space] as const,
        queryFn: async () => await client.rpc.getMinimumBalanceForRentExemption(BigInt(space)).send(),
        staleTime: 60 * 60_000,
    });
}

/** An open position's account (`null` once closed). */
export function usePositionAccount(position: Address) {
    const client = useAppClient();
    return useQuery({
        queryKey: qk.positionAccount(position),
        queryFn: async (): Promise<ShortPosition | null> => {
            const p = await fetchMaybeShortPosition(client.rpc, position, { commitment: 'confirmed' });
            return p.exists ? p.data : null;
        },
        staleTime: 0,
        refetchInterval: SNAPSHOT_REFRESH_MS * 2,
        structuralSharing: false,
    });
}

/**
 * The token's display symbol: the indexer's (untrusted text, normalised and capped,
 * `lib/displayText.ts`), else the short mint (FE-12, R-MINT §10.11: token identity is the mint);
 * `label` is the symbol with the short mint, for every place a symbol names the token of a trade.
 * A detail the indexer serves for another mint is ignored (M9-SEC-3).
 */
export function useTokenLabel(mint: Address | null): { symbol: string; name: string | null; label: string } {
    const q = useQuery({ ...indexerQueries.market(mint ?? 'none'), retry: false, enabled: !!mint });
    const d = q.data?.data && q.data.data.mint === mint ? q.data.data : null;
    const sym = cleanSymbol(d?.symbol);
    return {
        symbol: sym ?? (mint ? shortAddress(mint, 4, 4) : '—'),
        name: cleanName(d?.name),
        label: mint ? tokenLabel(sym, mint) : '—',
    };
}

/** Seconds of cluster time: the snapshot's clock plus the wall time since it was read. */
export function useChainNow(s: { readonly now: bigint; readonly fetchedAt: number } | null | undefined): bigint {
    const [tick, setTick] = useState(() => Date.now());
    useEffect(() => {
        const t = setInterval(() => setTick(Date.now()), 1_000);
        return () => clearInterval(t);
    }, []);
    if (!s) return BigInt(Math.floor(tick / 1000));
    return s.now + BigInt(Math.max(0, Math.floor((tick - s.fetchedAt) / 1000)));
}
