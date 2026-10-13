/**
 * Chrome's Local Network Access for the Mobile Wallet Adapter (FRONTEND.md §5.4 (6); mobile.md
 * §1.4). The association socket is on `ws://localhost`, which Chrome gates behind the "Apps on
 * device" permission (`loopback-network`). The library would otherwise show its own modals, which
 * render unstyled under the CSP and sit unreachable under the review's `<dialog>`; so the app asks
 * first, from a tap, and the library then finds the permission granted. The checks mirror the
 * library's (`isLocalWebSocketAvailable`, `checkLocalNetworkAccessPermission`), error test included.
 */
import { createStore } from '../lib/store.ts';

export type LocalNetwork = 'ready' | 'prompt' | 'denied';

/** `ready` until a browser that gates loopback says otherwise. */
export const localNetwork = createStore<LocalNetwork>('ready');

/** A loopback host (the library's `isLoopbackHost`): `localhost`, `*.localhost`, 127.0.0.0/8, `[::1]`. */
export function isLoopbackHost(hostname: string): boolean {
    return (
        hostname === 'localhost' ||
        hostname.endsWith('.localhost') ||
        hostname === '[::1]' ||
        /^127\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(hostname)
    );
}

/** Chrome before the permission existed: `query()` rejects with a TypeError naming it. */
const unknownPermission = (e: unknown) =>
    e instanceof TypeError && (e.message.includes('loopback-network') || e.message.includes('local-network-access'));

let watched: { readonly status: PermissionStatus; readonly apply: () => void } | null = null;

/** Reads Chrome's `loopback-network` permission and follows its changes. */
export async function watchLocalNetwork(): Promise<void> {
    // As the library: the Solana Mobile Web Shell and a loopback origin are exempt.
    if (navigator.userAgent.includes('Solana Mobile Web Shell') || isLoopbackHost(location.hostname)) return;
    try {
        const status = await navigator.permissions.query({ name: 'loopback-network' as PermissionName });
        const apply = () =>
            localNetwork.set(status.state === 'granted' ? 'ready' : status.state === 'denied' ? 'denied' : 'prompt');
        apply();
        if (watched) watched.status.removeEventListener('change', watched.apply);
        status.addEventListener('change', apply);
        watched = { status, apply };
    } catch (e) {
        // A browser that does not know the permission is not gated. Any other failure (including no
        // navigator.permissions at all) is what the library treats as blocked
        // (ERROR_LOOPBACK_ACCESS_BLOCKED): say so before the user taps, instead of after.
        localNetwork.set(unknownPermission(e) ? 'ready' : 'denied');
    }
}

/** From a tap: asks Chrome for "Apps on device" (the response is never read). */
export function requestLocalNetwork(): void {
    try {
        void fetch('http://localhost', { mode: 'no-cors', credentials: 'omit', cache: 'no-store' }).catch(() => {});
    } catch {
        // No fetch at all: nothing to ask with; the permission text stays.
    }
}
