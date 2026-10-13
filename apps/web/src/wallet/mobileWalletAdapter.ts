/**
 * Registers the Mobile Wallet Adapter (FRONTEND.md §5.4 (6); mobile.md §1.3, §2.2): a Wallet
 * Standard wallet (`@solana-mobile/wallet-standard-mobile` 0.7.0, protocol 3.0.0) that reaches the
 * wallet apps on an Android phone. Called once from `App.tsx`'s module scope; only an eligible
 * Android browser (`mwaEligible`) with the flag on fetches the package, as a lazy chunk. A framed
 * page never imports `App.tsx`, so it never registers.
 *
 * None of the library's own UI is ever shown: the app passes its own `onWalletNotFound` (so the
 * library's error modal, its `innerHTML` and its redirect are never built), runs its own
 * local-network step (`localNetwork.ts`), and removes every host the library appends to `<body>` as
 * it is inserted (`removeLibraryUi`). No remote (QR) association: neither `remoteHostAuthority`
 * nor `nostrRelay` is passed, not even as `undefined` (`registerMwa` tests `"nostrRelay" in config`).
 */
import type { AuthorizationCache } from '@solana-mobile/wallet-standard-mobile';
import type { AppConfig } from '../config/parseConfig.ts';
import { watchLocalNetwork } from './localNetwork.ts';
import { MWA_WALLET_NAME, mwaRegistered, mwaWalletNotFound } from './mwa.ts';
import { mwaEligible } from './platform.ts';

/** The part of the app client registration reads: the plugin's wallet list. */
export interface MwaRegistryClient {
    readonly wallet: { getState(): { readonly wallets: readonly { readonly name: string }[] } };
}

/**
 * The library's authorization cache (localStorage), scoped to the build's chain: an authorization
 * for another chain is dropped and cleared, so a devnet authorization never reaches a mainnet build
 * on the same origin. `set` and `clear` pass straight through (Disconnect clears it, locally only).
 */
export function chainScopedCache(chain: string, inner: AuthorizationCache): AuthorizationCache {
    return {
        async get() {
            const authorization = await inner.get();
            if (authorization && authorization.chain !== chain) {
                await inner.clear();
                return undefined;
            }
            return authorization;
        },
        set: (authorization) => inner.set(authorization),
        clear: () => inner.clear(),
    };
}

/** The spinner's host: the custom property is set through the CSSOM before the append (wsm-070:878). */
function isSpinnerHost(e: HTMLElement): boolean {
    return e.style.getPropertyValue('--spinner-color') !== '' || /--spinner-color/.test(e.getAttribute('style') ?? '');
}

/** Every embedded modal's host: its light DOM is the Google Fonts links (`host.innerHTML = fonts`, :182). */
function isModalHost(e: HTMLElement): boolean {
    return e.querySelector(':scope > link[href^="https://fonts.googleapis.com/"]') !== null;
}

/**
 * Removes each of the library's hosts as it is appended to `<body>` (a `childList` observer, not
 * `subtree`): the callback runs at the microtask checkpoint after the append, before any rendering,
 * so no overlay, tap-catcher or modal is ever painted. The library keeps working on the detached
 * nodes. Returns the stop function.
 */
export function removeLibraryUi(root: HTMLElement = document.body): () => void {
    const observer = new MutationObserver((records) => {
        for (const r of records) {
            for (const node of r.addedNodes) {
                if (node.nodeName !== 'DIV') continue;
                const e = node as HTMLElement;
                if (isSpinnerHost(e) || isModalHost(e)) e.remove();
            }
        }
    });
    observer.observe(root, { childList: true });
    return () => observer.disconnect();
}

let started = false;

/** How many wallets the plugin lists under the MWA name (its list updates synchronously on register). */
const mwaCount = (client: MwaRegistryClient) =>
    client.wallet.getState().wallets.filter((w) => w.name === MWA_WALLET_NAME).length;

/**
 * Registers the adapter once, on an eligible browser with the flag on. Resolves to whether it was
 * registered (the plugin's list gained exactly one "Mobile Wallet Adapter").
 */
export function registerMobileWalletAdapter(
    cfg: Pick<AppConfig, 'enableMobileWalletAdapter' | 'chain'>,
    client: MwaRegistryClient,
): Promise<boolean> {
    if (started || !cfg.enableMobileWalletAdapter) return Promise.resolve(false);
    if (typeof window === 'undefined' || !mwaEligible(navigator.userAgent ?? '', window.isSecureContext)) {
        return Promise.resolve(false);
    }
    started = true;
    return import('@solana-mobile/wallet-standard-mobile')
        .then(({ registerMwa, createDefaultAuthorizationCache, createDefaultChainSelector }) => {
            const stopUi = removeLibraryUi(); // before the first association can append anything
            try {
                const before = mwaCount(client);
                registerMwa({
                    appIdentity: { name: 'ShortWin', uri: location.origin, icon: 'icon-192.png' },
                    chains: [cfg.chain],
                    authorizationCache: chainScopedCache(cfg.chain, createDefaultAuthorizationCache()),
                    chainSelector: createDefaultChainSelector(),
                    // Never the library's modal (innerHTML, a redirect to solanamobile.com): the header
                    // chooser reopens with the open-in links instead. The library also calls this on a
                    // launch failure at Sign, where the wallet is connected: ignored (the review says it).
                    onWalletNotFound: async (wallet) => {
                        if (!(wallet as { readonly connected?: boolean }).connected) {
                            mwaWalletNotFound.set(Date.now());
                        }
                    },
                });
                // registerMwa re-checks the environment and may only warn: verify against the plugin.
                if (mwaCount(client) !== before + 1) {
                    stopUi();
                    return false;
                }
            } catch (e) {
                stopUi();
                throw e;
            }
            mwaRegistered.set(true);
            void watchLocalNetwork();
            return true;
        })
        .catch((e: unknown) => {
            console.warn('Mobile Wallet Adapter not loaded', e);
            return false;
        });
}
