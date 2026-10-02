/**
 * The RPC's current slot (status bar, staleness): a `slotNotifications` subscription with a 10 s
 * polling fallback.
 */
import { useEffect } from 'react';
import { createStore } from '../lib/store.ts';
import type { AppClient } from './client.ts';

export interface RpcSlot {
    readonly slot: bigint | null;
    readonly at: number;
    readonly ok: boolean;
}

export const rpcSlot = createStore<RpcSlot>({ slot: null, at: 0, ok: false });

export function useRpcSlotTracker(client: AppClient): void {
    useEffect(() => {
        const abort = new AbortController();
        const poll = async () => {
            try {
                const slot = await client.rpc.getSlot({ commitment: 'confirmed' }).send({ abortSignal: abort.signal });
                rpcSlot.set({ slot, at: Date.now(), ok: true });
            } catch {
                if (!abort.signal.aborted) rpcSlot.set((p) => ({ ...p, ok: false }));
            }
        };
        void poll();
        const timer = setInterval(poll, 10_000);
        (async () => {
            try {
                const it = await client.rpcSubscriptions.slotNotifications().subscribe({ abortSignal: abort.signal });
                for await (const n of it) rpcSlot.set({ slot: n.slot, at: Date.now(), ok: true });
            } catch {
                // the poller keeps the value fresh
            }
        })();
        return () => {
            abort.abort();
            clearInterval(timer);
        };
    }, [client]);
}
