/**
 * R-C1 against the real plugin (FRONTEND.md §5.4 (6), FE-35; mobile.md §1.10): the chooser's tap
 * dispatches the connect through Kit's `useConnect` and starts `guardMwaConnect` right after. The
 * guard keeps running only because that dispatch reaches the plugin's `connect` synchronously and
 * the plugin sets `connecting` before its first await (otherwise the guard's first check stops it
 * and the hung connect is never ended). The real kit-plugin-wallet (`createAppClient`, nothing sent
 * over its RPC), the real `useConnect`, and a wallet named as the adapter whose `standard:connect`
 * never settles (as wallet-standard-mobile 0.7.0's can, §1.10 P2).
 */
import { useConnect } from '@solana/kit-plugin-wallet/react';
import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { config } from '../config/env.ts';
import { createAppClient } from '../rpc/client.ts';
import { MWA_WALLET_NAME } from './mwa.ts';
import { guardMwaConnect, MWA_CONNECT_CEILING_MS } from './mwaCall.ts';

// Vitest resolves the plugin with Node's conditions, whose build is the server's (no wallet registry,
// no storage: status `pending` for ever). The page runs its browser build; so does this test.
vi.mock('@solana/kit-plugin-wallet', async () => {
    const node = import.meta.resolve('@solana/kit-plugin-wallet');
    return import(/* @vite-ignore */ node.replace(/index\.node\.mjs$/, 'index.browser.mjs'));
});

afterEach(() => {
    vi.useRealTimers();
    localStorage.clear();
});

describe('R-C1 with the real plugin and useConnect', () => {
    it('a connect the wallet never answers: still guarded after the dispatch, ended at the ceiling', async () => {
        const connect = vi.fn(() => new Promise<never>(() => {}));
        const disconnect = vi.fn(async () => {});
        const wallet = {
            version: '1.0.0',
            name: MWA_WALLET_NAME,
            icon: 'data:image/svg+xml;base64,PHN2Zy8+',
            chains: [config.chain],
            accounts: [],
            features: {
                'standard:connect': { version: '1.0.0', connect },
                'standard:disconnect': { version: '1.0.0', disconnect },
                'standard:events': { version: '1.0.0', on: () => () => {} },
                'solana:signTransaction': {
                    version: '1.0.0',
                    supportedTransactionVersions: ['legacy', 0],
                    signTransaction: async () => [],
                },
            },
        };
        const client = createAppClient(config);
        window.dispatchEvent(
            new CustomEvent('wallet-standard:register-wallet', {
                detail: ({ register }: { register(w: unknown): () => void }) => register(wallet),
            }),
        );
        await waitFor(() => expect(client.wallet.getState().status).toBe('disconnected'));
        const entry = client.wallet.getState().wallets.find((w) => w.name === MWA_WALLET_NAME);
        expect(entry).toBeDefined();
        const { result } = renderHook(() => useConnect(client));

        vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
        act(() => {
            // The chooser's tap (WalletChooser.tsx): the connect, then the guard.
            result.current.dispatch(entry as NonNullable<typeof entry>);
            guardMwaConnect(client);
        });
        // Past the guard's first check (a microtask): the plugin was already `connecting`.
        await act(async () => {
            await vi.advanceTimersByTimeAsync(0);
        });
        expect(connect).toHaveBeenCalledOnce();
        expect(client.wallet.getState().status).toBe('connecting');
        expect(vi.getTimerCount()).toBe(1); // the ceiling: the guard is still installed

        await act(async () => {
            await vi.advanceTimersByTimeAsync(MWA_CONNECT_CEILING_MS - 1);
        });
        expect(client.wallet.getState().status).toBe('connecting');
        expect(disconnect).not.toHaveBeenCalled();
        await act(async () => {
            await vi.advanceTimersByTimeAsync(1);
        });
        // The plugin's hung connect is superseded, and the wallet's own standard:disconnect ran (the
        // library's connect lock is reset there).
        expect(client.wallet.getState().status).toBe('disconnected');
        expect(disconnect).toHaveBeenCalledOnce();
        expect(vi.getTimerCount()).toBe(0);
    });
});
