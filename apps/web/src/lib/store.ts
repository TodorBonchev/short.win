/** A tiny external store for `useSyncExternalStore` (wallet-independent global UI state). */
import { useSyncExternalStore } from 'react';

export interface Store<T> {
    get(): T;
    set(next: T | ((prev: T) => T)): void;
    subscribe(listener: () => void): () => void;
}

export function createStore<T>(initial: T): Store<T> {
    let value = initial;
    const listeners = new Set<() => void>();
    return {
        get: () => value,
        set(next) {
            const v = typeof next === 'function' ? (next as (prev: T) => T)(value) : next;
            if (Object.is(v, value)) return;
            value = v;
            for (const l of listeners) l();
        },
        subscribe(listener) {
            listeners.add(listener);
            return () => listeners.delete(listener);
        },
    };
}

export function useStore<T>(store: Store<T>): T {
    return useSyncExternalStore(store.subscribe, store.get, store.get);
}
