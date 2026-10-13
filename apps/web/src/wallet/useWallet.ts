/**
 * Wallet state for features (FRONTEND.md §5.4): the connected account's address and signer, and
 * whether it can sign v0 transactions. Wallet discovery and connection are Wallet Standard through
 * `@solana/kit-plugin-wallet` (no wallet-adapter).
 *
 * The signer policy for the Mobile Wallet Adapter (§5.4 (6)): it signs only through
 * `solana:signTransaction`, so `assertSameIntent` always runs before anything is sent. An MWA wallet
 * whose Kit signer is not a modifying (or partial) signer — a wallet without
 * `solana:signTransactions`, an MWA 1.x wallet without sign-and-send, per-account features that
 * hide `solana:signTransaction` from Kit — gets no signer, like a read-only wallet. Other wallets
 * are unchanged (FE-21).
 */
import type { Address, TransactionSigner } from '@solana/kit';
import { useConnectedWallet, useIsWalletReady, useWalletStatus } from '@solana/kit-plugin-wallet/react';
import { useStore } from '../lib/store.ts';
import { useAppClient } from '../rpc/context.tsx';
import { MWA_WALLET_NAME, mwaRegistered, mwaSignerOk } from './mwa.ts';

export interface ConnectedWallet {
    readonly address: Address;
    readonly signer: TransactionSigner | null;
    readonly walletName: string;
    /** The wallet's icon (a `data:` URI from the Wallet Standard registry). */
    readonly walletIcon?: string | null;
    readonly supportsV0: boolean;
}

export function useWallet(): { ready: boolean; status: string; connected: ConnectedWallet | null } {
    const client = useAppClient();
    const ready = useIsWalletReady(client);
    const status = useWalletStatus(client);
    const c = useConnectedWallet(client);
    const mwaOn = useStore(mwaRegistered);
    const mwa = mwaOn && c?.wallet.name === MWA_WALLET_NAME;
    return {
        ready,
        status,
        connected: c
            ? {
                  address: c.account.address as Address,
                  signer: mwa && c.signer && !mwaSignerOk(c.signer) ? null : c.signer,
                  walletName: c.wallet.name,
                  walletIcon: c.wallet.icon ?? null,
                  supportsV0: c.supportedTransactionVersions.has(0),
              }
            : null,
    };
}
