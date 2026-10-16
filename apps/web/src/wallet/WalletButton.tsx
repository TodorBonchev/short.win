/**
 * Header wallet control: connect via any discovered Wallet Standard wallet; the account's menu.
 *
 * Wallet identity (M9-SEC-8): two registered wallets can announce the same name and icon (an
 * impostor extension next to the real one). Entries are keyed by object identity, a duplicated
 * name is marked with its registration order and feature set, and a *silent* reconnect (the
 * plugin's auto-reconnect by the stored `name:address`) is undone when the name is ambiguous or the
 * wallet came back with another account — the user then picks the wallet explicitly. A wallet that
 * can only sign-and-send is labelled: the app cannot check its transaction before it is sent
 * (FRONTEND.md §5.4). The chooser itself is `WalletChooser.tsx` (shared with ConnectCta).
 *
 * Responsive (FRONTEND.md §7.6): on a phone the button reads "Connect" (" wallet" stays in its
 * accessible name) and the menu is a bottom sheet over a scrim, with a visible title and a ✕, and
 * history-backed like the trade sheet: opening it pushes a history entry (`state.walletSheet`), so
 * the browser's / OS Back closes it; closing it any other way pops that entry, and a menu link
 * replaces it. Without a wallet the popup is a dialog with the open-in-wallet links (mobile) or the
 * install links (desktop); with wallets or an account, its entries are a `role="menu"`.
 *
 * The Mobile Wallet Adapter (O-6, §5.4 (6)): when the library reports that no wallet app answered a
 * Connect (`mwaWalletNotFound`, set only while that wallet is not connected), the header chooser
 * reopens — the open-in links are then in front of the user — once the wallet is idle; a report
 * older than `MWA_NOT_FOUND_FRESH_MS` is dropped without reopening.
 */

import type { Address } from '@solana/kit';
import { useConnectedWallet, useDisconnect, useWalletStatus } from '@solana/kit-plugin-wallet/react';
import { useQuery } from '@tanstack/react-query';
import { Link, useRouter } from '@tanstack/react-router';
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { qk } from '../api/keys.ts';
import { Button } from '../components/Button.tsx';
import { useSheetMode } from '../components/Sheet.tsx';
import { explorerAddress } from '../config/env.ts';
import { T } from '../config/testids.ts';
import { formatSol, shortAddress } from '../lib/format.ts';
import { useStore } from '../lib/store.ts';
import { useAppClient } from '../rpc/context.tsx';
import { NavIcon } from '../shell/icons.tsx';
import { PHONE, useMedia } from '../styles/breakpoints.ts';
import { MWA_NOT_FOUND_FRESH_MS, mwaWalletNotFound } from './mwa.ts';
import { NoWallet, useWalletList, WalletOptions } from './WalletChooser.tsx';
import { walletChooser, walletNotice, wasChosen } from './walletUi.ts';

export { sendsItselfFeatures, walletRanks } from './WalletChooser.tsx';

/** The plugin's storage key (`rpc/client.ts`). */
export const WALLET_STORAGE_KEY = 'shortwin:wallet';

/** The account the plugin will silently reconnect to, as stored before this page load. */
function savedAtLoad(): { name: string; address: string } | null {
    try {
        const v = globalThis.localStorage?.getItem(WALLET_STORAGE_KEY) ?? null;
        if (!v) return null;
        const i = v.lastIndexOf(':');
        return i < 0 ? null : { name: v.slice(0, i), address: v.slice(i + 1) };
    } catch {
        return null;
    }
}
const SAVED = savedAtLoad();

/** The connected account's menu: who, how much SOL, where to go, Disconnect. */
function AccountMenu({
    walletName,
    address,
    replace,
    onDone,
    onDisconnect,
}: {
    walletName: string;
    address: string;
    /** The popup has its own history entry (a phone): a destination replaces it. */
    replace: boolean;
    onDone: () => void;
    onDisconnect: () => void;
}) {
    const client = useAppClient();
    const [copied, setCopied] = useState(false);
    const balance = useQuery({
        queryKey: qk.balance(address as Address),
        queryFn: async () =>
            (await client.rpc.getBalance(address as Address, { commitment: 'confirmed' }).send()).value,
        staleTime: 5_000,
    });
    const explorer = explorerAddress(address);
    return (
        <>
            <div className="menu-label">
                <bdi>{walletName}</bdi>
            </div>
            <span className="menu-address" data-value={address}>
                {address}
            </span>
            <div className="menu-label">
                Balance{' '}
                <span className="num">{balance.data === undefined ? '—' : `${formatSol(balance.data, 4)} SOL`}</span>
            </div>
            <div className="menu-sep" />
            <div role="menu" aria-label="Account">
                <button
                    type="button"
                    role="menuitem"
                    className="menu-item"
                    data-testid={T.walletCopyAddress}
                    onClick={() => {
                        void navigator.clipboard?.writeText(address).then(() => {
                            setCopied(true);
                            setTimeout(() => setCopied(false), 1_200);
                        });
                    }}
                >
                    {copied ? 'Copied' : 'Copy address'}
                </button>
                {explorer && (
                    <a
                        role="menuitem"
                        className="menu-item"
                        href={explorer}
                        target="_blank"
                        rel="noopener noreferrer"
                        data-testid={T.walletExplorer}
                    >
                        View on explorer <NavIcon name="external" className="icon-sm" />
                    </a>
                )}
                <Link
                    to="/positions"
                    replace={replace}
                    role="menuitem"
                    className="menu-item"
                    data-testid={T.walletMyPositions}
                    onClick={onDone}
                >
                    My positions
                </Link>
                <Link
                    to="/lend"
                    replace={replace}
                    role="menuitem"
                    className="menu-item"
                    data-testid={T.walletMyLending}
                    onClick={onDone}
                >
                    My lending
                </Link>
                <button
                    type="button"
                    role="menuitem"
                    className="menu-item"
                    data-testid={T.walletDisconnect}
                    onClick={onDisconnect}
                >
                    Disconnect
                </button>
            </div>
        </>
    );
}

/**
 * The popup's title row: on a phone a visible title and a 44 px ✕ (the bottom sheet's); from 640 px
 * up not shown, its title still naming the popup (`aria-labelledby`).
 */
function MenuHead({ titleId, title, onClose }: { titleId: string; title: string; onClose: () => void }) {
    return (
        <div className="menu-head">
            <h2 id={titleId} className="menu-title">
                {title}
            </h2>
            <button
                type="button"
                className="btn sheet-close"
                data-variant="ghost"
                aria-label="Close"
                data-testid={T.walletSheetClose}
                onClick={onClose}
            >
                <NavIcon name="close" className="icon" />
            </button>
        </div>
    );
}

export function WalletButton() {
    const client = useAppClient();
    const status = useWalletStatus(client);
    const wallets = useWalletList();
    const connected = useConnectedWallet(client);
    const disconnect = useDisconnect(client);
    const open = useStore(walletChooser) === 'header';
    const router = useRouter({ warn: false }) as ReturnType<typeof useRouter> | undefined;
    const phone = useMedia(PHONE);
    /** Whether the current history entry is the popup's own (pushed on a phone). */
    const backed = () => router?.history.location.state.walletSheet === true;
    const setOpen = useCallback(
        (v: boolean) => {
            walletChooser.set(v ? 'header' : null);
            if (v && phone && router && router.history.location.state.walletSheet !== true) {
                void router.navigate({
                    href: router.history.location.href,
                    state: (prev) => ({ ...prev, walletSheet: true }),
                    resetScroll: false,
                });
            }
        },
        [phone, router],
    );
    const box = useRef<HTMLDivElement>(null);
    const menu = useRef<HTMLDivElement>(null);
    const titleId = useId();

    // History-backed on a phone: Back pops the popup's entry → close; closed any other way (✕, the
    // scrim, Escape, a pick, Disconnect) → pop it. A menu link has replaced it by then (no pop).
    useEffect(() => {
        if (!open || !router) return;
        return router.history.subscribe(() => {
            if (router.history.location.state.walletSheet !== true) {
                walletChooser.set((o) => (o === 'header' ? null : o));
            }
        });
    }, [open, router]);
    const wasOpen = useRef(false);
    // Picking a wallet closes the popup while the button is busy (disabled: it cannot take focus
    // back yet); focus goes to it once the connection settles, if it went nowhere else.
    const focusWhenReady = useRef(false);
    useEffect(() => {
        if (open) {
            wasOpen.current = true;
            return;
        }
        if (!wasOpen.current) return;
        wasOpen.current = false;
        const button = box.current?.querySelector<HTMLButtonElement>(`[data-testid="${T.walletButton}"]`);
        focusWhenReady.current = !!button?.disabled;
        if (router?.history.location.state.walletSheet === true) router.history.back();
    }, [open, router]);
    // The popup is modal at every width (the page behind it inert, focus inside, Escape, focus back
    // to the button): a bottom sheet over a scrim on a phone; on a desktop it no longer leaves the
    // page's controls under it half-covered yet live (axe target-size; FRONTEND.md §7.6).
    useSheetMode(menu, {
        open,
        onClose: () => walletChooser.set(null),
        query: 'all',
        labelledBy: titleId,
        // The first entry (not the phone's ✕): a menu is used from the keyboard by its items.
        initialFocus: () =>
            [...(menu.current?.querySelectorAll<HTMLElement>('a[href], button:not(:disabled)') ?? [])].find(
                (e) => !e.closest('.menu-head'),
            ) ?? null,
    });
    // The header chooser does not outlive the header.
    useEffect(() => () => walletChooser.set((o) => (o === 'header' ? null : o)), []);

    useEffect(() => {
        if (!open) return;
        // A press outside, or on the phone sheet's scrim (inside the anchor), closes it.
        const close = (e: PointerEvent) => {
            const t = e.target as Element | null;
            if (!box.current || !t) return;
            if (!box.current.contains(t) || t.classList?.contains('menu-scrim')) walletChooser.set(null);
        };
        // Focus goes back to the button from the sheet hook once the popup is gone (the button is
        // inert while it is open, so a focus call here would be lost).
        const onKey = (e: KeyboardEvent) => {
            if (e.key !== 'Escape' || !box.current?.contains(document.activeElement)) return;
            walletChooser.set(null);
        };
        document.addEventListener('pointerdown', close);
        document.addEventListener('keydown', onKey);
        return () => {
            document.removeEventListener('pointerdown', close);
            document.removeEventListener('keydown', onKey);
        };
    }, [open]);

    // A silent reconnect picks the first wallet with the stored name and, when the stored account
    // is gone, that wallet's first account: undo it when either is ambiguous (M9-SEC-8).
    const connectedName = connected?.wallet.name ?? null;
    const connectedAddress = connected?.account.address ?? null;
    const sameName = connectedName ? wallets.filter((w) => w.name === connectedName).length : 0;
    useEffect(() => {
        if (wasChosen() || !connectedName || !connectedAddress) return;
        let reason: string | null = null;
        if (sameName > 1) {
            reason = `More than one installed wallet calls itself “${connectedName}”. Choose the one to use.`;
        } else if (SAVED && SAVED.name === connectedName && SAVED.address !== connectedAddress) {
            reason = `“${connectedName}” reconnected with another account than last time. Choose it again to confirm.`;
        }
        if (reason) {
            walletNotice.set(reason);
            disconnect.dispatch();
        }
    }, [connectedName, connectedAddress, sameName, disconnect]);

    const warming = status === 'pending' || status === 'reconnecting';
    const noWallet = !connected && wallets.length === 0;
    const busy = warming || status === 'connecting';
    // No wallet app answered an MWA Connect: once idle, the header chooser reopens with the open-in
    // links (a fresh report only; a stale one is dropped).
    const notFoundAt = useStore(mwaWalletNotFound);
    const isConnected = !!connected;
    useEffect(() => {
        if (notFoundAt === null || isConnected || busy) return;
        mwaWalletNotFound.set(null);
        if (Date.now() - notFoundAt < MWA_NOT_FOUND_FRESH_MS) setOpen(true);
    }, [notFoundAt, isConnected, busy, setOpen]);
    useEffect(() => {
        if (busy || !focusWhenReady.current) return;
        focusWhenReady.current = false;
        const a = document.activeElement;
        if (a && a !== document.body) return;
        box.current?.querySelector<HTMLElement>(`[data-testid="${T.walletButton}"]`)?.focus({ preventScroll: true });
    }, [busy]);
    return (
        <div ref={box} className="wallet-anchor">
            <Button
                variant={connected ? 'default' : 'primary'}
                busy={busy}
                data-testid={T.walletButton}
                aria-haspopup={noWallet ? 'dialog' : 'menu'}
                aria-expanded={open}
                onClick={() => setOpen(!open)}
            >
                {connected ? (
                    <span className="num" data-testid={T.walletAddress} data-value={connected.account.address}>
                        {shortAddress(connected.account.address)}
                    </span>
                ) : warming ? (
                    'Wallet…'
                ) : (
                    <>
                        Connect<span className="wallet-word"> wallet</span>
                    </>
                )}
            </Button>
            {open && (
                <>
                    <div className="menu-scrim" aria-hidden="true" data-sheet-keep="" />
                    {noWallet ? (
                        <div
                            ref={menu}
                            className="menu"
                            role="dialog"
                            aria-labelledby={titleId}
                            data-testid={T.walletMenu}
                        >
                            <MenuHead titleId={titleId} title="Connect a wallet" onClose={() => setOpen(false)} />
                            <NoWallet />
                        </div>
                    ) : (
                        <div ref={menu} className="menu" data-testid={T.walletMenu}>
                            <MenuHead
                                titleId={titleId}
                                title={connected ? 'Wallet' : 'Connect a wallet'}
                                onClose={() => setOpen(false)}
                            />
                            {connected ? (
                                <AccountMenu
                                    walletName={connected.wallet.name}
                                    address={connected.account.address}
                                    replace={backed()}
                                    onDone={() => setOpen(false)}
                                    onDisconnect={() => {
                                        disconnect.dispatch();
                                        setOpen(false);
                                    }}
                                />
                            ) : (
                                <WalletOptions wallets={wallets} asMenu onPicked={() => setOpen(false)} />
                            )}
                        </div>
                    )}
                </>
            )}
        </div>
    );
}
