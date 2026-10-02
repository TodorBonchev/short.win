/**
 * Data hooks of the LEND track: the lend state of a market as the chain has it (FE-D1), kept fresh
 * by account subscriptions on `Market`, `MarketVault` and the owner's `LenderPosition` (at most one
 * refetch per ~slot, FRONTEND.md §6.3) with a slow polling fallback, and a seconds clock for the
 * time-dependent previews (vest release, lock expiry).
 */
import type { Address } from '@solana/kit';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import type { AppClient } from '../../rpc/client.ts';
import { useAppClient } from '../../rpc/context.tsx';
import { useNow } from '../../shell/freshness.tsx';
import { type LendChain, lendKeys, readLendChain } from './chain.ts';

/** One refetch per this many ms at most when accounts change (≈ one slot). */
const ACCOUNT_DEBOUNCE_MS = 400;
/** Polling fallback when no subscription is available. */
const POLL_MS = 15_000;

/**
 * Calls `onChange` (debounced, trailing) whenever one of `addresses` changes at `confirmed`.
 * Best effort: a failing subscription leaves the polling fallback in charge. Returns whether every
 * subscription is established (then a read stays current while nothing changes; §6.6).
 */
export function useAccountChanges(
    client: AppClient,
    addresses: readonly (Address | null)[],
    onChange: () => void,
): boolean {
    const cb = useRef(onChange);
    cb.current = onChange;
    const key = addresses.filter(Boolean).join(',');
    const [live, setLive] = useState(false);
    useEffect(() => {
        setLive(false);
        if (!key) return;
        const abort = new AbortController();
        const all = key.split(',');
        let established = 0;
        let timer: ReturnType<typeof setTimeout> | null = null;
        const fire = () => {
            if (timer) return;
            timer = setTimeout(() => {
                timer = null;
                if (!abort.signal.aborted) cb.current();
            }, ACCOUNT_DEBOUNCE_MS);
        };
        for (const a of all) {
            void (async () => {
                let counted = false;
                try {
                    const it = await client.rpcSubscriptions
                        .accountNotifications(a as Address, { commitment: 'confirmed', encoding: 'base64' })
                        .subscribe({ abortSignal: abort.signal });
                    counted = true;
                    if (++established === all.length && !abort.signal.aborted) setLive(true);
                    for await (const _ of it) fire();
                } catch {
                    // the polling fallback keeps the data fresh
                } finally {
                    // A subscription that ended (socket closed) no longer vouches for the read.
                    if (counted) established--;
                    if (!abort.signal.aborted) setLive(false);
                }
            })();
        }
        return () => {
            abort.abort();
            if (timer) clearTimeout(timer);
        };
    }, [client, key]);
    return live;
}

/** The lend state of `mint` for `owner` (token accounts included), live. */
export function useLendState(mint: Address, owner: Address | null) {
    const client = useAppClient();
    const qc = useQueryClient();
    const key = lendKeys.state(mint, owner);
    const q = useQuery({
        queryKey: key,
        queryFn: (): Promise<LendChain> => readLendChain(client.rpc, mint, owner, { tokenAccounts: true }),
        staleTime: 0,
        refetchInterval: POLL_MS,
        refetchOnWindowFocus: false,
        structuralSharing: false,
        retry: 2,
    });
    const d = q.data;
    const live = useAccountChanges(
        client,
        d ? [d.marketAddress, d.vaultAddress, d.lenderAddress] : [],
        () => void qc.invalidateQueries({ queryKey: key }),
    );
    return Object.assign(q, { live });
}

/** Unix seconds, ticking every `ms` (previews depend on `now`: vest release, lock expiry). */
export function useNowSeconds(ms = 5_000): number {
    return Math.floor(useNow(ms) / 1000);
}
