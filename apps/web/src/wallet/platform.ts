/**
 * Which browser the page runs in, for the wallet chooser (FRONTEND.md §5.4 (6)): a social app's
 * in-app browser (where no wallet is injected and a universal link may not leave the app), and the
 * browsers where the Mobile Wallet Adapter registers. Pure functions of the user agent, so the
 * shell can call them without the adapter's package.
 */

/**
 * A social app's in-app browser or an Android WebView (FE-33): Telegram, X (Twitter), Instagram,
 * Facebook (`FBAN` / `FBAV` on iOS, `FB_IAB` on Android), Line, and Android's `; wv)` token.
 * `\bLine\/` needs the slash, so a "Linux" desktop is not one.
 */
export function isInAppBrowser(ua: string): boolean {
    return /; wv\)|Telegram|Twitter|Instagram|FBAN|FBAV|FB_IAB|\bLine\//.test(ua);
}

/** wallet-standard-mobile 0.7.0's own WebView test (`isWebView`), verbatim. */
const LIBRARY_WEBVIEW =
    /(WebView|Version\/.+(Chrome)\/(\d+)\.(\d+)\.(\d+)\.(\d+)|; wv\).+(Chrome)\/(\d+)\.(\d+)\.(\d+)\.(\d+))/i;

/**
 * Where the app registers the Mobile Wallet Adapter (D3): a secure context on Android, not a WebView
 * (the library's test; the Solana Mobile Web Shell is exempt, as in the library), and not Firefox,
 * whose protocol path frames a `solana-wallet:` URL that the CSP refuses (FE-34). Everywhere else
 * the adapter's package is never fetched.
 */
export function mwaEligible(ua: string, secureContext: boolean): boolean {
    if (!secureContext || !/android/i.test(ua) || /Firefox\//.test(ua)) return false;
    return !LIBRARY_WEBVIEW.test(ua) || ua.includes('Solana Mobile Web Shell');
}
