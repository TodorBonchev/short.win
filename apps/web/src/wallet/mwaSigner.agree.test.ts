/**
 * The signer policy against the real wallet (FRONTEND.md §5.4 (6); mobile.md §1.11): the real
 * wallet-standard-mobile 0.7.0 registered by the app, the real kit-plugin-wallet (`createAppClient`,
 * nothing sent over its RPC), and `localStorage` seeded with the plugin's `shortwin:wallet` and an
 * authorization in the library's cache format, so the plugin's silent reconnect runs with no
 * association. Which Kit signer the plugin builds follows the account's features resolved on the
 * wallet; `useWallet` keeps it for the Mobile Wallet Adapter only when it signs through
 * `solana:signTransaction`.
 */
import {
    getAddressEncoder,
    isTransactionModifyingSigner,
    isTransactionSendingSigner,
    type TransactionSigner,
} from '@solana/kit';
import { act, renderHook, waitFor } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { config } from '../config/env.ts';
import { type AppClient, createAppClient } from '../rpc/client.ts';
import { AppClientProvider } from '../rpc/context.tsx';
import { stubNavigator, UA } from '../test/userAgents.ts';
import { registerMobileWalletAdapter } from './mobileWalletAdapter.ts';
import { MWA_WALLET_NAME, mwaRegistered } from './mwa.ts';
import { useWallet } from './useWallet.ts';

// Vitest resolves the plugin with Node's conditions, whose build is the server's (no wallet registry,
// no storage: status `pending` for ever). The page runs its browser build; so does this test.
vi.mock('@solana/kit-plugin-wallet', async () => {
    const node = import.meta.resolve('@solana/kit-plugin-wallet');
    return import(/* @vite-ignore */ node.replace(/index\.node\.mjs$/, 'index.browser.mjs'));
});

const ADDRESS = '9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin';
const PUBLIC_KEY = Uint8Array.from(getAddressEncoder().encode(ADDRESS as never));
const CACHE_KEY = 'SolanaMobileWalletAdapterDefaultAuthorizationCache';
const DEFAULT_ACCOUNT_FEATURES = [
    'solana:signAndSendTransaction',
    'solana:signTransaction',
    'solana:signMessage',
    'solana:signIn',
];

/** An authorization as the library caches it after Connect (its accounts in Wallet Standard form). */
function seedMwa(o: { capabilities: readonly string[]; accountFeatures?: readonly string[] }) {
    const authorization = {
        auth_token: 'e2e',
        accounts: [
            {
                address: ADDRESS,
                publicKey: PUBLIC_KEY,
                label: 'E2E',
                chains: [config.chain],
                features: o.accountFeatures ?? DEFAULT_ACCOUNT_FEATURES,
            },
        ],
        chain: config.chain,
        capabilities: {
            max_transactions_per_request: 10,
            max_messages_per_request: 10,
            supported_transaction_versions: ['legacy', 0],
            features: o.capabilities,
            // Protocol v1: the protocol sets it (mobile-wallet-adapter-protocol 3.0.0).
            supports_sign_and_send_transactions: true,
        },
    };
    localStorage.setItem(CACHE_KEY, JSON.stringify(authorization));
    localStorage.setItem('shortwin:wallet', `${MWA_WALLET_NAME}:${ADDRESS}`);
}

/** A client whose plugin silently reconnects from the seeded storage; settles before returning. */
async function reconnected(): Promise<AppClient> {
    const client = createAppClient(config);
    await waitFor(() => expect(client.wallet.getState().status).toBe('connected'));
    return client;
}

function walletOf(client: AppClient) {
    const wrapper = ({ children }: { children: ReactNode }) => createElement(AppClientProvider, { client, children });
    return renderHook(() => useWallet(), { wrapper }).result.current.connected;
}

const kind = (s: TransactionSigner | null | undefined) =>
    !s
        ? 'none'
        : isTransactionModifyingSigner(s)
          ? 'modifying'
          : isTransactionSendingSigner(s)
            ? 'sending-only'
            : 'other';

let restoreUa: () => void = () => {};
const clients: AppClient[] = [];
beforeAll(async () => {
    restoreUa = stubNavigator({ userAgent: UA.androidChrome });
    Object.defineProperty(window, 'isSecureContext', { configurable: true, value: true });
    localStorage.clear();
    const first = createAppClient(config);
    expect(await registerMobileWalletAdapter({ enableMobileWalletAdapter: true, chain: config.chain }, first)).toBe(
        true,
    );
    expect(mwaRegistered.get()).toBe(true);
});
afterEach(async () => {
    // Disconnect is local: it clears the library's cache, so the next case reads a fresh seed.
    for (const c of clients.splice(0)) await act(() => c.wallet.disconnect());
    localStorage.clear();
});
afterAll(() => restoreUa());

describe('the signer policy against the real package and plugin (§1.11)', () => {
    it('v1 with solana:signTransactions: a modifying signer, kept', async () => {
        seedMwa({ capabilities: ['solana:signTransactions'] });
        const client = await reconnected();
        clients.push(client);
        const plugin = client.wallet.getState().connected;
        expect(plugin?.wallet.name).toBe(MWA_WALLET_NAME);
        expect(plugin?.account.address).toBe(ADDRESS);
        expect(kind(plugin?.signer)).toBe('modifying');
        expect(kind(walletOf(client)?.signer)).toBe('modifying');
    });

    it('v1 without solana:signTransactions (a sending-only wallet): the plugin builds no signer', async () => {
        seedMwa({ capabilities: [] });
        const client = await reconnected();
        clients.push(client);
        expect(client.wallet.getState().connected?.signer).toBeNull();
        const w = walletOf(client);
        expect(w?.address).toBe(ADDRESS);
        expect(w?.signer).toBeNull();
    });

    it('per-account features in MWA’s own ids: the plugin’s signer is sending-only, useWallet refuses it', async () => {
        seedMwa({
            capabilities: ['solana:signTransactions'],
            accountFeatures: ['solana:signTransactions', 'solana:signAndSendTransaction'],
        });
        const client = await reconnected();
        clients.push(client);
        expect(kind(client.wallet.getState().connected?.signer)).toBe('sending-only');
        expect(walletOf(client)?.signer).toBeNull();
    });

    it('the same per-account features on another wallet are kept (FE-21 unchanged)', async () => {
        const account = {
            address: ADDRESS,
            publicKey: PUBLIC_KEY,
            chains: [config.chain],
            features: ['solana:signTransactions', 'solana:signAndSendTransaction'],
        };
        const wallet = {
            version: '1.0.0',
            name: 'Sender E2E',
            icon: 'data:image/svg+xml;base64,PHN2Zy8+',
            chains: [config.chain],
            accounts: [account],
            features: {
                'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: [account] }) },
                'standard:events': { version: '1.0.0', on: () => () => {} },
                'solana:signAndSendTransaction': {
                    version: '1.0.0',
                    supportedTransactionVersions: ['legacy', 0],
                    signAndSendTransaction: async () => [],
                },
            },
        };
        window.dispatchEvent(
            new CustomEvent('wallet-standard:register-wallet', {
                detail: ({ register }: { register(w: unknown): () => void }) => register(wallet),
            }),
        );
        localStorage.setItem('shortwin:wallet', `Sender E2E:${ADDRESS}`);
        const client = await reconnected();
        clients.push(client);
        expect(client.wallet.getState().connected?.wallet.name).toBe('Sender E2E');
        const w = walletOf(client);
        expect(kind(w?.signer)).toBe('sending-only');
    });
});
