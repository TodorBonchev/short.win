/**
 * The mobile-wallet strings (FRONTEND.md §4.3, the owner file's section 8; §5.4 (6)): the wallet
 * chooser on a phone and the Mobile Wallet Adapter's notices in the review. Drafted by the mobile
 * survey and approved as drafted by the owner on 2026-10-13 (O-6 lifted), byte for byte: straight
 * apostrophes and no final full stops, unlike the rest of the copy (mobile.md §6 R9). A change needs
 * the owner's approval, this code and FRONTEND §4.3 together.
 *
 * A module of its own so the shell (the chooser) carries these seven strings, not the whole of
 * `copy.ts`, whose `COPY` spreads them (one home, one id each; `copy.test.ts` pins them).
 */
import { type CopyEntry, c } from './copyNav.ts';

export const WALLET_COPY = {
    'wallet.openIn.more': c(
        'Or open this page in a wallet app',
        'A phone or tablet with wallets listed (header sheet and inline under Connect wallet): the label above “Open in Backpack / Phantom / Solflare”, after the entries (`wallet-open-elsewhere`)',
    ),
    'wallet.webview.open': c(
        'Open this page in your browser',
        'A social app’s in-app browser with no wallet (Telegram, X, Instagram, Facebook, Line, an Android WebView): the chooser’s first line (`wallet-in-app-browser`)',
        'FE-33: a universal link may not leave an in-app browser, and no wallet is injected there (wallet/platform.ts isInAppBrowser).',
    ),
    'wallet.webview.copy': c(
        'Copy link',
        'The button after it: copies this page’s address (“Copied” for 1.2 s, as “Copy address”) (`wallet-copy-link`)',
    ),
    'wallet.mwa.label': c(
        'Use installed wallet',
        'The Mobile Wallet Adapter’s entry (Android browsers): its label. The wallet’s own name, “Mobile Wallet Adapter”, stays its test id and shows in the account menu and the review',
        'The Mobile Wallet Adapter reaches the wallet apps installed on the phone (FE §5.4 (6); @solana-mobile/wallet-standard-mobile 0.7.0).',
    ),
    'wallet.mwa.hint': c(
        'Opens your wallet app on this phone',
        'The Mobile Wallet Adapter entry’s second line (`wallet-mwa-hint`)',
        'The association launches the wallet app through an Android intent (solana-wallet:; mobile-wallet-adapter-protocol 3.0.0 startScenario).',
    ),
    'wallet.mwa.localNetwork': c(
        "Allow 'Apps on device' so this page can reach your wallet app",
        'The Mobile Wallet Adapter entry’s second line while Chrome’s local-network permission is not granted (asked or denied); and beside Sign in the review when the connected wallet is the Mobile Wallet Adapter and the permission is missing (`tx-sign-notice`, `data-reason="local-network"`)',
        'Chrome gates loopback access (the association socket on ws://localhost) behind its local-network permission, “Apps on device” in third-party reports (wallet/localNetwork.ts; the device checklist B1 records Chrome’s own wording).',
    ),
    'wallet.mwa.retry': c(
        "Couldn't open your wallet app. Tap Sign again",
        'The review, beside Sign, when the wallet app could not be launched; the review stays open and nothing was sent (`tx-sign-notice`, `data-reason="launch"`)',
        'ERROR_WALLET_NOT_FOUND at Sign: the launch did not leave the page within 3 s, and the protocol throws before any socket exists (mobile-wallet-adapter-protocol 3.0.0), so nothing reached the wallet.',
    ),
} as const satisfies Record<string, CopyEntry>;

export type WalletCopyId = keyof typeof WALLET_COPY;

/** A mobile-wallet string (none has a placeholder). */
export function walletCopy(id: WalletCopyId): string {
    return WALLET_COPY[id].text;
}
