/**
 * The wallet chooser (FRONTEND.md §5.4, §7.6), shared by the header's wallet menu and a page's
 * ConnectCta — one at a time (`walletUi.ts`), so its test ids stay unique.
 *
 * - Wallets discovered: one entry per registered Wallet Standard wallet. Two with the same name are
 *   told apart by registration rank and feature set, and a wallet that can only sign-and-send is
 *   labelled (M9-SEC-8, M9-SEC-1).
 * - The Mobile Wallet Adapter (O-6, §5.4 (6); registered by the app on Android browsers): its
 *   entry reads "Use installed wallet" over "Opens your wallet app on this phone" (when it is the
 *   only wallet of that name), asks Chrome's local-network permission from the tap first
 *   (`localNetwork.ts`), and ends a connect the wallet never answers (`guardMwaConnect`). It never
 *   carries the "sends itself" badge: the app only signs through it (`mwaSignerOk`).
 * - On a phone or tablet with wallets listed: the open-in links follow the entries, so the route to
 *   a wallet app's browser survives a wallet registering.
 * - No wallet, on a phone or tablet: open this page in a wallet app's browser (universal links,
 *   `deepLinks.ts`), alphabetically; off mainnet, which network to set first; where to get one; and
 *   a Reload for a page already inside a wallet's browser whose wallet registered late. In a social
 *   app's in-app browser, first: open this page in your browser, with Copy link (FE-33).
 * - No wallet, on a desktop: where to get one, and Reload.
 * A wallet that registers while the chooser is open replaces the links with its entry.
 */
import { useConnect, useWallets } from '@solana/kit-plugin-wallet/react';
import { type ReactNode, useRef, useState } from 'react';
import { walletCopy } from '../config/copyWallet.ts';
import { config } from '../config/env.ts';
import { T } from '../config/testids.ts';
import { useStore } from '../lib/store.ts';
import { useAppClient } from '../rpc/context.tsx';
import { NavIcon } from '../shell/icons.tsx';
import { isMobileDevice } from '../styles/breakpoints.ts';
import { browseLink, MOBILE_WALLETS, WALLET_NAMES, WALLET_SITES } from './deepLinks.ts';
import { localNetwork, requestLocalNetwork, watchLocalNetwork } from './localNetwork.ts';
import { MWA_WALLET_NAME, mwaRegistered } from './mwa.ts';
import { guardMwaConnect } from './mwaCall.ts';
import { isInAppBrowser } from './platform.ts';
import { markChosen, walletNotice } from './walletUi.ts';

const SIGN_TX = 'solana:signTransaction';
const SIGN_AND_SEND = 'solana:signAndSendTransaction';

export interface WalletLike {
    readonly name: string;
    readonly icon?: string;
    readonly features: readonly string[];
}

/** Only `solana:signAndSendTransaction`: the wallet sends itself (no check before sending). */
export function sendsItselfFeatures(features: readonly string[]): boolean {
    return features.includes(SIGN_AND_SEND) && !features.includes(SIGN_TX);
}

/** Per entry: its 1-based rank among the wallets with the same name, and that name's count. */
export function walletRanks(wallets: readonly WalletLike[]): { rank: number; of: number }[] {
    const counts = new Map<string, number>();
    for (const w of wallets) counts.set(w.name, (counts.get(w.name) ?? 0) + 1);
    const seen = new Map<string, number>();
    return wallets.map((w) => {
        const rank = (seen.get(w.name) ?? 0) + 1;
        seen.set(w.name, rank);
        return { rank, of: counts.get(w.name) ?? 1 };
    });
}

const ids = new WeakMap<object, number>();
let nextId = 0;
function identity(w: object): number {
    let id = ids.get(w);
    if (id === undefined) {
        id = ++nextId;
        ids.set(w, id);
    }
    return id;
}

function featureHint(features: readonly string[]): string {
    if (sendsItselfFeatures(features)) return 'sends transactions itself';
    if (features.includes(SIGN_TX)) return 'signs transactions';
    return 'cannot sign transactions';
}

/** The discovered wallets (as the plugin reports them). */
export function useWalletList(): readonly (WalletLike & object)[] {
    return useWallets(useAppClient()) as readonly (WalletLike & object)[];
}

const pageHref = () => (typeof location === 'undefined' ? '' : location.href);

/**
 * "Open in <wallet>" for each wallet app (its universal browse link to this very page) and, off
 * mainnet, which network to set first: the content without a wallet, and the block after the
 * entries on a phone.
 */
function OpenInLinks() {
    const href = pageHref();
    const origin = typeof location === 'undefined' ? '' : location.origin;
    return (
        <>
            <div className="wallet-links">
                {MOBILE_WALLETS.map((w) => (
                    <a
                        key={w}
                        className="btn"
                        href={browseLink(w, href, origin)}
                        rel="noopener noreferrer"
                        data-testid={T.walletOpenIn(w)}
                    >
                        Open in {WALLET_NAMES[w]} <NavIcon name="external" className="icon-sm" />
                    </a>
                ))}
            </div>
            {config.cluster !== 'mainnet' && <p className="wallet-note">Set your wallet to {config.cluster} first.</p>}
        </>
    );
}

/**
 * A social app's in-app browser (FE-33): no wallet is injected there and a universal link may not
 * leave it, so first offer the way out: this page in the phone's browser, with Copy link.
 */
function InAppBrowser() {
    const box = useRef<HTMLDivElement>(null);
    const [copied, setCopied] = useState(false);
    const done = () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1_200);
    };
    // Without the async clipboard (or when it refuses): a read-only <textarea> inside the chooser
    // (outside it the sheet's siblings are inert), selected and copied, then removed.
    const fallback = () => {
        const host = box.current;
        if (!host) return;
        const t = document.createElement('textarea');
        t.value = pageHref();
        t.readOnly = true;
        t.className = 'visually-hidden';
        t.setAttribute('aria-hidden', 'true');
        host.appendChild(t);
        t.select();
        let ok = false;
        try {
            ok = document.execCommand?.('copy') ?? false;
        } catch {
            ok = false;
        }
        t.remove();
        if (ok) done();
    };
    const copyLink = () => {
        const clipboard = typeof navigator === 'undefined' ? undefined : navigator.clipboard;
        if (!clipboard?.writeText) {
            fallback();
            return;
        }
        clipboard.writeText(pageHref()).then(done, fallback);
    };
    return (
        <div ref={box} className="wallet-chooser" data-testid={T.walletInAppBrowser}>
            <p>{walletCopy('wallet.webview.open')}</p>
            <button type="button" className="btn" data-size="sm" data-testid={T.walletCopyLink} onClick={copyLink}>
                {copied ? 'Copied' : walletCopy('wallet.webview.copy')}
            </button>
        </div>
    );
}

/** No wallet in this browser: the open-in links (mobile) or the install links (desktop). */
export function NoWallet() {
    const mobile = isMobileDevice();
    const reload = (
        <button
            type="button"
            className="btn"
            data-size="sm"
            data-testid={T.walletReload}
            onClick={() => location.reload()}
        >
            Reload
        </button>
    );
    // A wrapping row of links with gaps (no text separators to strand at a line start).
    const sites = (
        <ul className="wallet-install" aria-label="Wallet websites">
            {MOBILE_WALLETS.map((w) => (
                <li key={w}>
                    <a
                        href={WALLET_SITES[w].href}
                        target="_blank"
                        rel="noopener noreferrer"
                        data-testid={T.walletInstall(w)}
                    >
                        {WALLET_SITES[w].text}
                    </a>
                </li>
            ))}
        </ul>
    );
    if (!mobile) {
        return (
            <div className="wallet-chooser">
                <p>No Solana wallet found in this browser. Install a Wallet Standard wallet, for example:</p>
                {sites}
                <p className="row">I installed one: {reload}</p>
            </div>
        );
    }
    const inApp = typeof navigator !== 'undefined' && isInAppBrowser(navigator.userAgent ?? '');
    return (
        <div className="wallet-chooser">
            {inApp && <InAppBrowser />}
            <p>This browser has no Solana wallet. Open this page in your wallet app:</p>
            <OpenInLinks />
            <p>Don’t have one?</p>
            {sites}
            <p className="row">Already in a wallet’s browser? {reload}</p>
        </div>
    );
}

/** The entries' box: a `role="menu"` inside the header's popup, a plain group inline. */
function ItemsBox({ asMenu, children }: { asMenu: boolean; children: ReactNode }) {
    return asMenu ? (
        <div role="menu" aria-label="Wallets" className="wallet-options">
            {children}
        </div>
    ) : (
        <div className="wallet-options">{children}</div>
    );
}

/**
 * The wallet entries (`asMenu`: `role="menuitem"`, inside the header's `role="menu"`). Picking one
 * connects it and calls `onPicked`.
 */
export function WalletOptions({
    wallets,
    asMenu,
    onPicked,
}: {
    wallets: readonly (WalletLike & object)[];
    asMenu: boolean;
    onPicked: () => void;
}) {
    const client = useAppClient();
    const connect = useConnect(client);
    const notice = useStore(walletNotice);
    const mwaOn = useStore(mwaRegistered);
    const lna = useStore(localNetwork);
    const ranks = walletRanks(wallets);
    const isMwa = (w: WalletLike) => mwaOn && w.name === MWA_WALLET_NAME;
    return (
        <>
            {notice && (
                <div className="banner" data-tone="warn" role="status">
                    {notice}
                </div>
            )}
            <ItemsBox asMenu={asMenu}>
                {wallets.map((w, i) => {
                    const { rank, of } = ranks[i] ?? { rank: 1, of: 1 };
                    const mwa = isMwa(w);
                    // The friendly label only when the name is unique: a same-named impostor keeps
                    // both entries as reported, with the duplicate badges (M9-SEC-8).
                    const friendly = mwa && of === 1;
                    // The app never uses the adapter's sending path (§5.4 (6)): no badge for it.
                    const selfSending = !mwa && sendsItselfFeatures(w.features);
                    return (
                        <button
                            key={identity(w)}
                            type="button"
                            role={asMenu ? 'menuitem' : undefined}
                            className="menu-item"
                            data-testid={T.walletOption(rank === 1 ? w.name : `${w.name} (${rank})`)}
                            data-duplicate={of > 1 ? String(rank) : undefined}
                            data-mwa={mwa ? '' : undefined}
                            data-local-network={mwa ? lna : undefined}
                            disabled={connect.isRunning}
                            title={of > 1 ? `Registered ${rank} of ${of} · ${featureHint(w.features)}` : undefined}
                            onClick={() => {
                                if (mwa) {
                                    // Chrome's "Apps on device" first, from this tap; the next tap
                                    // (a fresh activation) connects once it is granted.
                                    const state = localNetwork.get();
                                    if (state === 'prompt') {
                                        requestLocalNetwork();
                                        return;
                                    }
                                    if (state === 'denied') {
                                        void watchLocalNetwork();
                                        return;
                                    }
                                }
                                markChosen();
                                connect.dispatch(w as Parameters<typeof connect.dispatch>[0]);
                                // Listeners and timers only, after the dispatch: the wallet's launch
                                // keeps this tap's activation.
                                if (mwa) guardMwaConnect(client);
                                onPicked();
                            }}
                        >
                            {w.icon && <img src={w.icon} alt="" />}
                            {friendly ? (
                                <span className="menu-item-text">
                                    <span>{walletCopy('wallet.mwa.label')}</span>
                                    <span className="menu-item-hint" data-testid={T.walletMwaHint}>
                                        {walletCopy(lna === 'ready' ? 'wallet.mwa.hint' : 'wallet.mwa.localNetwork')}
                                    </span>
                                </span>
                            ) : (
                                <bdi>{w.name}</bdi>
                            )}
                            {of > 1 && (
                                <span className="badge" data-tone="warn" data-testid={T.walletDuplicate}>
                                    same name · {rank} of {of} · {featureHint(w.features)}
                                </span>
                            )}
                            {selfSending && (
                                <span className="badge" data-tone="warn" data-testid={T.walletSendsItself}>
                                    sends itself · checked after landing
                                </span>
                            )}
                        </button>
                    );
                })}
            </ItemsBox>
            {wallets.some((w) => !isMwa(w) && sendsItselfFeatures(w.features)) && (
                <p className="menu-note">
                    A wallet that sends transactions itself cannot have them checked before they are sent, only after
                    they land.
                </p>
            )}
            {isMobileDevice() && (
                <div className="wallet-chooser" data-testid={T.walletOpenElsewhere}>
                    <p className="menu-label">{walletCopy('wallet.openIn.more')}</p>
                    <OpenInLinks />
                </div>
            )}
        </>
    );
}
