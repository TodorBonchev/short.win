/**
 * The Mobile Wallet Adapter's state in the shell (FRONTEND.md §5.4 (6)): whether the app registered
 * it in this page load, when it last reported that no wallet app answered a Connect, and which of
 * its signers the app accepts. The adapter's package itself is loaded only by
 * `mobileWalletAdapter.ts`, on eligible Android browsers.
 */
import { isTransactionModifyingSigner, isTransactionPartialSigner, type TransactionSigner } from '@solana/kit';
import { createStore } from '../lib/store.ts';

/** The library's `SolanaMobileWalletAdapterWalletName` (pinned by `mwaPackage.test.ts`). */
export const MWA_WALLET_NAME = 'Mobile Wallet Adapter';

/**
 * True once the app registered the adapter and the plugin listed it. Never true on a desktop, so a
 * wallet that merely calls itself "Mobile Wallet Adapter" there is shown as it reports itself.
 */
export const mwaRegistered = createStore(false);

/** When the library last reported "no wallet app answered" at Connect (`Date.now()`), or null. */
export const mwaWalletNotFound = createStore<number | null>(null);

/** The header chooser reopens only for a not-found report younger than this. */
export const MWA_NOT_FOUND_FRESH_MS = 10_000;

/** The connected or listed wallet is the adapter the app registered (not a same-named impostor elsewhere). */
export function isMwaWallet(name: string | null | undefined): boolean {
    return mwaRegistered.get() && name === MWA_WALLET_NAME;
}

/**
 * The signer policy (§5.4 (6), mobile.md §1.11): an MWA wallet signs only through
 * `solana:signTransaction`, so `assertSameIntent` always runs before anything is sent. Its
 * sending-only signer (the post hoc path) is never used.
 */
export function mwaSignerOk(signer: TransactionSigner): boolean {
    return isTransactionModifyingSigner(signer) || isTransactionPartialSigner(signer);
}
