/**
 * Mobile wallet access without a dependency (FRONTEND.md §5.4 "mobile wallets"): a phone browser has
 * no wallet extension, so a page without a wallet offers to open itself inside a wallet app's own
 * browser, where the wallet injects Wallet Standard and the normal connect / sign path works. Each
 * wallet's universal "browse" link, as its documentation gives it (re-checked 2026-10-09; re-check
 * before each release — a third-party contract; a broken link lands on the wallet's website):
 *
 *   Backpack  https://backpack.app/ul/v1/browse/<url>?ref=<ref>   docs.backpack.app/deeplinks/other-methods/browse
 *   Phantom   https://phantom.com/ul/browse/<url>?ref=<ref>       docs.phantom.com/phantom-deeplinks/other-methods/browse
 *   Solflare  https://solflare.com/ul/v1/browse/<url>?ref=<ref>   docs.solflare.com/solflare/technical/deeplinks/other-methods/browse
 *
 * `url` and `ref` are URL-encoded; `ref` is this app's origin (deliberately: the wallet shows who
 * sent the user). The links are meant to be tapped, never pasted (Backpack, Phantom), so they are
 * only ever rendered as `<a>`. Top-level, user-initiated navigations: the CSP is unchanged.
 */

/** Alphabetical: no wallet is favoured. */
export const MOBILE_WALLETS = ['backpack', 'phantom', 'solflare'] as const;
export type MobileWallet = (typeof MOBILE_WALLETS)[number];

const BROWSE: Record<MobileWallet, string> = {
    backpack: 'https://backpack.app/ul/v1/browse/',
    phantom: 'https://phantom.com/ul/browse/',
    solflare: 'https://solflare.com/ul/v1/browse/',
};

export const WALLET_NAMES: Record<MobileWallet, string> = {
    backpack: 'Backpack',
    phantom: 'Phantom',
    solflare: 'Solflare',
};

/** The wallets' websites (install). */
export const WALLET_SITES: Record<MobileWallet, { href: string; text: string }> = {
    backpack: { href: 'https://backpack.app/', text: 'backpack.app' },
    phantom: { href: 'https://phantom.com/', text: 'phantom.com' },
    solflare: { href: 'https://solflare.com/', text: 'solflare.com' },
};

/** The universal link that opens `href` in `wallet`'s in-app browser, from `origin`. */
export function browseLink(wallet: MobileWallet, href: string, origin: string): string {
    return `${BROWSE[wallet]}${encodeURIComponent(href)}?ref=${encodeURIComponent(origin)}`;
}
