/**
 * Header wallet control: connect via any discovered Wallet Standard wallet; address + disconnect.
 *
 * Wallet identity (M9-SEC-8): two registered wallets can announce the same name and icon (an
 * impostor extension next to the real one). Entries are keyed by object identity, a duplicated
 * name is marked with its registration order and feature set, and a *silent* reconnect (the
 * plugin's auto-reconnect by the stored `name:address`) is undone when the name is ambiguous or the
 * wallet came back with another account — the user then picks the wallet explicitly. A wallet that
 * can only sign-and-send is labelled: the app cannot check its transaction before it is sent
 * (FRONTEND.md §5.4).
 */
import {
    useConnect,
    useConnectedWallet,
    useDisconnect,
    useWalletStatus,
    useWallets,
} from '@solana/kit-plugin-wallet/react';
import { useEffect, useRef, useState } from 'react';
import { Button } from '../components/Button.tsx';
import { T } from '../config/testids.ts';
import { shortAddress } from '../lib/format.ts';
import { useAppClient } from '../rpc/context.tsx';

/** The plugin's storage key (`rpc/client.ts`). */
export const WALLET_STORAGE_KEY = 'shortwin:wallet';

const SIGN_TX = 'solana:signTransaction';
const SIGN_AND_SEND = 'solana:signAndSendTransaction';

interface WalletLike {
    readonly name: string;
    readonly icon?: string;
    readonly features: readonly string[];
}

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

export function WalletButton() {
    const client = useAppClient();
    const status = useWalletStatus(client);
    const wallets = useWallets(client) as readonly (WalletLike & object)[];
    const connected = useConnectedWallet(client);
    const connect = useConnect(client);
    const disconnect = useDisconnect(client);
    const [open, setOpen] = useState(false);
    const [notice, setNotice] = useState<string | null>(null);
    const chosen = useRef(false);
    const box = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!open) return;
        const close = (e: MouseEvent) => {
            if (box.current && !box.current.contains(e.target as Node)) setOpen(false);
        };
        document.addEventListener('mousedown', close);
        return () => document.removeEventListener('mousedown', close);
    }, [open]);

    // A silent reconnect picks the first wallet with the stored name and, when the stored account
    // is gone, that wallet's first account: undo it when either is ambiguous (M9-SEC-8).
    const connectedName = connected?.wallet.name ?? null;
    const connectedAddress = connected?.account.address ?? null;
    const sameName = connectedName ? wallets.filter((w) => w.name === connectedName).length : 0;
    useEffect(() => {
        if (chosen.current || !connectedName || !connectedAddress) return;
        let reason: string | null = null;
        if (sameName > 1) {
            reason = `More than one installed wallet calls itself “${connectedName}”. Choose the one to use.`;
        } else if (SAVED && SAVED.name === connectedName && SAVED.address !== connectedAddress) {
            reason = `“${connectedName}” reconnected with another account than last time. Choose it again to confirm.`;
        }
        if (reason) {
            setNotice(reason);
            disconnect.dispatch();
        }
    }, [connectedName, connectedAddress, sameName, disconnect]);

    const ranks = walletRanks(wallets);
    const warming = status === 'pending' || status === 'reconnecting';
    return (
        <div ref={box} style={{ position: 'relative' }}>
            <Button
                variant={connected ? 'default' : 'primary'}
                busy={warming || status === 'connecting'}
                data-testid={T.walletButton}
                aria-haspopup="menu"
                aria-expanded={open}
                onClick={() => setOpen((o) => !o)}
            >
                {connected ? (
                    <span className="num" data-testid={T.walletAddress} data-value={connected.account.address}>
                        {shortAddress(connected.account.address)}
                    </span>
                ) : warming ? (
                    'Wallet…'
                ) : (
                    'Connect wallet'
                )}
            </Button>
            {open && (
                <div
                    className="menu"
                    role="menu"
                    data-testid={T.walletMenu}
                    onKeyDown={(e) => {
                        if (e.key === 'Escape') setOpen(false);
                    }}
                >
                    {notice && !connected && (
                        <div className="banner" data-tone="warn" role="status" style={{ margin: 4 }}>
                            {notice}
                        </div>
                    )}
                    {connected ? (
                        <>
                            <div className="muted" style={{ padding: '4px 8px', fontSize: 11 }}>
                                <bdi>{connected.wallet.name}</bdi>
                            </div>
                            <button
                                type="button"
                                role="menuitem"
                                className="menu-item"
                                data-testid={T.walletDisconnect}
                                onClick={() => {
                                    disconnect.dispatch();
                                    setOpen(false);
                                }}
                            >
                                Disconnect
                            </button>
                        </>
                    ) : wallets.length === 0 ? (
                        <div className="muted" style={{ padding: 8 }}>
                            No Solana wallet found. Install a Wallet Standard wallet (for example Phantom, Solflare or
                            Backpack).
                        </div>
                    ) : (
                        wallets.map((w, i) => {
                            const { rank, of } = ranks[i] ?? { rank: 1, of: 1 };
                            const selfSending = sendsItselfFeatures(w.features);
                            return (
                                <button
                                    key={identity(w)}
                                    type="button"
                                    role="menuitem"
                                    className="menu-item"
                                    data-testid={T.walletOption(rank === 1 ? w.name : `${w.name} (${rank})`)}
                                    data-duplicate={of > 1 ? String(rank) : undefined}
                                    disabled={connect.isRunning}
                                    title={
                                        of > 1 ? `Registered ${rank} of ${of} · ${featureHint(w.features)}` : undefined
                                    }
                                    onClick={() => {
                                        chosen.current = true;
                                        setNotice(null);
                                        connect.dispatch(w as Parameters<typeof connect.dispatch>[0]);
                                        setOpen(false);
                                    }}
                                >
                                    {w.icon && <img src={w.icon} alt="" />}
                                    <bdi>{w.name}</bdi>
                                    {of > 1 && (
                                        <span className="badge" data-tone="warn" data-testid={T.walletDuplicate}>
                                            same name · {rank} of {of} · {featureHint(w.features)}
                                        </span>
                                    )}
                                    {selfSending && (
                                        <span
                                            className="badge"
                                            data-tone="warn"
                                            data-testid={T.walletSendsItself}
                                            title="This wallet sends transactions itself: the app cannot check them before they are sent, only after they land."
                                        >
                                            sends itself · checked after landing
                                        </span>
                                    )}
                                </button>
                            );
                        })
                    )}
                </div>
            )}
        </div>
    );
}
