/**
 * The wallet UI's shared state (FRONTEND.md §5.4, §7.6): which chooser is open — the header's or a
 * page's inline one under a ConnectCta (`inline:<surface>`) — so at most one is ever mounted and
 * the `wallet-option-*` / `wallet-open-in-*` ids stay unique; whether the user picked a wallet
 * explicitly in this page load (the silent-reconnect check, M9-SEC-8, leaves that one alone); and
 * the notice that check leaves for the chooser.
 */
import { createStore } from '../lib/store.ts';

export type ConnectSurface = 'short' | 'lend' | 'positions' | 'lend-dashboard' | 'position';
export type ChooserOpen = 'header' | `inline:${ConnectSurface}` | null;

export const walletChooser = createStore<ChooserOpen>(null);
export const walletNotice = createStore<string | null>(null);

let chosen = false;

/** The user picked a wallet in the chooser (not a silent reconnect). */
export function markChosen(): void {
    chosen = true;
    walletNotice.set(null);
}

export function wasChosen(): boolean {
    return chosen;
}
