/**
 * Wallet state for features (FRONTEND.md §5.4): the connected account's address and signer, and
 * whether it can sign v0 transactions. Wallet discovery and connection are Wallet Standard through
 * `@solana/kit-plugin-wallet` (no wallet-adapter).
 */
import type { Address, TransactionSigner } from '@solana/kit';
import { useConnectedWallet, useIsWalletReady, useWalletStatus } from '@solana/kit-plugin-wallet/react';
import { useAppClient } from '../rpc/context.tsx';

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
    return {
        ready,
        status,
        connected: c
            ? {
                  address: c.account.address as Address,
                  signer: c.signer,
                  walletName: c.wallet.name,
                  walletIcon: c.wallet.icon ?? null,
                  supportsV0: c.supportedTransactionVersions.has(0),
              }
            : null,
    };
}
