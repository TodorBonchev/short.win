/**
 * TEST-ONLY init-script hooks for the Mobile Wallet Adapter E2E (FRONTEND.md §10.2; mobile.md §5.2),
 * installed with `page.addInitScript` by the `mwa` fixture only. Plain browser JS, no imports.
 *
 * Headless Chromium has no Android wallet app and `Location`'s members cannot be stubbed, so these
 * hooks stand in for Android's app switch around the protocol
 * (`@solana-mobile/mobile-wallet-adapter-protocol` 3.0.0):
 *
 * - The association. `URLSearchParams.prototype.set` is wrapped (calling through unchanged): when the
 *   protocol builds the intent URL it sets `association` and then `port` (getAssociateAndroidIntentURL),
 *   and each pair is recorded as `{ association, port, active, at }`, `active` being
 *   `navigator.userActivation.isActive` at that moment (Android hands the intent to the wallet app
 *   only within the tap's user activation) and `at` `performance.now()`.
 * - The external intent. The Navigation API cancels only `solana-wallet:` navigations. Chromium
 *   152 otherwise starts a native external-protocol handoff that leaves subsequent touch input
 *   blocked, even after a reload. The intent is already recorded above; its fake app switch and
 *   the real library's association socket still run. Ordinary page navigations are unchanged.
 * - The app switch. The protocol then waits up to 3 s for the page to lose focus (getDetectionPromise:
 *   `window.addEventListener('blur', …)`). `window.addEventListener` is wrapped: the first `blur`
 *   listener `window` gets after a recorded association is that one, and a synthetic `blur`
 *   follows 50 ms later — what the wallet app coming to the front does. With `launch = 'not-found'`
 *   none follows, and the protocol reports ERROR_WALLET_NOT_FOUND after its 3 s. (The app's own
 *   connect guard registers its `blur` listener in the tap handler, before the association exists,
 *   so it never matches; the synthetic `blur` reaches it too, as a real launch does.)
 * - The return to the browser: `returnToPage()` dispatches `focus` on `window`.
 *
 * `window.__mwaE2E`: `launches` (the recorded associations, in order), `launch` ('ok' |
 * 'not-found'), `returnToPage()`.
 */
(() => {
    if (window.__mwaE2E) return;
    const launches = [];
    let launch = 'ok';
    /** An association recorded and not yet launched (its blur listener not seen yet). */
    let pending = false;
    /** The `association` value whose `port` has not been set yet. */
    let current = null;

    // jsdom (the protocol unit tests) has no Navigation API or native external-protocol handoff.
    window.navigation?.addEventListener('navigate', (event) => {
        if (new URL(event.destination.url).protocol === 'solana-wallet:') event.preventDefault();
    });

    const set = URLSearchParams.prototype.set;
    URLSearchParams.prototype.set = function hookedSet(name, value) {
        const out = set.call(this, name, value);
        if (name === 'association') {
            current = {
                association: String(value),
                port: null,
                active: navigator.userActivation ? navigator.userActivation.isActive : null,
                at: performance.now(),
            };
        } else if (name === 'port' && current) {
            current.port = Number(value);
            launches.push(current);
            current = null;
            pending = true;
        }
        return out;
    };

    // `window`'s own `addEventListener` (shadowing EventTarget.prototype's, which it calls): the
    // protocol calls `window.addEventListener('blur', …)`.
    const add = window.addEventListener;
    window.addEventListener = function hookedAddEventListener(type, listener, options) {
        const out = add.call(this, type, listener, options);
        if (type === 'blur' && pending) {
            pending = false;
            if (launch !== 'not-found') setTimeout(() => window.dispatchEvent(new Event('blur')), 50);
        }
        return out;
    };

    window.__mwaE2E = {
        launches,
        get launch() {
            return launch;
        },
        set launch(v) {
            launch = v === 'not-found' ? 'not-found' : 'ok';
        },
        returnToPage() {
            window.dispatchEvent(new Event('focus'));
        },
    };
})();
