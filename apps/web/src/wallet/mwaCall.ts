/**
 * The way out of Mobile Wallet Adapter calls that never settle (FRONTEND.md §5.4 (6), FE-35;
 * mobile.md §1.10). wallet-standard-mobile 0.7.0 has paths whose promise never settles: a wallet
 * that never answers after the association, the library's spinner closed after the handshake, a
 * failed reauthorization at Sign. At Connect the plugin then stays `connecting` (the header button
 * busy) and the library's own `#connecting` lock makes every later MWA connect return no accounts
 * without launching the wallet, until a reload.
 *
 * R-C1: `guardMwaConnect`, started by the MWA entry's tap, ends a connect the wallet never answers
 * {@link MWA_RETURN_GRACE_MS} after the page is back in front (the wallet answers before it hands
 * the screen back), or {@link MWA_CONNECT_CEILING_MS} after the tap, and resets the lock. At Sign the
 * review's Cancel is enabled {@link MWA_SIGN_CANCEL_AFTER_MS} after the wallet call starts (R-S1,
 * `useTxFlow`).
 */
import type { AppClient } from '../rpc/client.ts';
import { MWA_WALLET_NAME } from './mwa.ts';

/** R-S1: while signing with an MWA wallet, the review's Cancel works from this long after the call. */
export const MWA_SIGN_CANCEL_AFTER_MS = 5_000;
/** R-C1: an unanswered connect ends this long after the page comes back to the front. */
export const MWA_RETURN_GRACE_MS = 10_000;
/** R-C1: and at the latest this long after the tap (a user still deciding in the wallet). */
export const MWA_CONNECT_CEILING_MS = 120_000;

/** The part of the app client the guard uses (`client.wallet`). */
export interface MwaConnectClient {
    readonly wallet: Pick<AppClient['wallet'], 'getState' | 'subscribe' | 'disconnect'>;
}

/**
 * Ends the plugin's hung connect and resets the library's connect lock: first the plugin's
 * untargeted disconnect (with nothing active it supersedes the pending connect and sets
 * `disconnected`), then the targeted one on the MWA wallet, which reaches its `standard:disconnect`
 * (`#disconnect` sets `#connecting = false`). The next tap launches the wallet again.
 */
export async function resetMwaConnect(client: MwaConnectClient): Promise<void> {
    const mwa = client.wallet.getState().wallets.find((w) => w.name === MWA_WALLET_NAME);
    try {
        await client.wallet.disconnect();
        if (mwa) await client.wallet.disconnect(mwa);
    } catch (e) {
        console.warn('Mobile Wallet Adapter: the connect could not be reset', e);
    }
}

/**
 * R-C1. Call right after `connect.dispatch(mwa)`: only listeners and timers, synchronously, so the
 * wallet's launch keeps the tap's activation. Stops as soon as the plugin's status is no longer
 * `connecting` (connected, failed, or reset). Returns the stop function.
 */
export function guardMwaConnect(client: MwaConnectClient): () => void {
    let stopped = false;
    let left = false;
    let grace: ReturnType<typeof setTimeout> | undefined;
    const fire = () => {
        if (stopped) return;
        stop();
        void resetMwaConnect(client);
    };
    const away = () => {
        left = true;
        clearTimeout(grace);
        grace = undefined;
    };
    const back = () => {
        if (!left) return;
        clearTimeout(grace);
        grace = setTimeout(fire, MWA_RETURN_GRACE_MS);
    };
    const onVisibility = () => (document.visibilityState === 'hidden' ? away() : back());
    const ceiling = setTimeout(fire, MWA_CONNECT_CEILING_MS);
    const check = () => {
        if (client.wallet.getState().status !== 'connecting') stop();
    };
    window.addEventListener('blur', away);
    window.addEventListener('focus', back);
    document.addEventListener('visibilitychange', onVisibility);
    const unsubscribe = client.wallet.subscribe(check);
    function stop() {
        if (stopped) return;
        stopped = true;
        clearTimeout(grace);
        clearTimeout(ceiling);
        window.removeEventListener('blur', away);
        window.removeEventListener('focus', back);
        document.removeEventListener('visibilitychange', onVisibility);
        unsubscribe();
    }
    // The connect was dispatched just before: a status that is not (or no longer) `connecting` once
    // the dispatch's synchronous part has run means there is nothing to guard.
    queueMicrotask(check);
    return stop;
}
