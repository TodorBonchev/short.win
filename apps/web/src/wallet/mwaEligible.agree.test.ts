/**
 * `mwaEligible` agrees with the real library (FRONTEND.md §5.4 (6), §10.1): for every user agent of
 * the matrix, the app loads the package exactly where wallet-standard-mobile 0.7.0's own
 * `registerMwa` registers a wallet, so the shell never fetches the chunk for nothing and never
 * misses a browser the library supports. The one intended difference is Firefox for Android, where
 * the library registers and the app refuses (its protocol path frames a URL the CSP refuses; FE-34).
 * Registration is observed as an app sees it: the Wallet Standard `register-wallet` window event.
 */
import { registerMwa } from '@solana-mobile/wallet-standard-mobile';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { stubNavigator, UA, type UaName } from '../test/userAgents.ts';
import { mwaEligible } from './platform.ts';

const registered: { name: string }[] = [];
const onRegister = (e: Event) => {
    (e as CustomEvent<(api: { register(w: { name: string }): () => void }) => void>).detail({
        register: (w) => {
            registered.push(w);
            return () => {};
        },
    });
};

/** Registers as the app does (no remote association), and says whether a wallet registered. */
function libraryRegisters(): boolean {
    const before = registered.length;
    registerMwa({
        appIdentity: { name: 'ShortWin', uri: location.origin, icon: 'icon-192.png' },
        chains: ['solana:devnet'],
        onWalletNotFound: async () => {},
    });
    return registered.length === before + 1;
}

/** jsdom has no `isSecureContext`; its localhost page is one in a browser, as an https deploy is. */
const secureContext = (value: boolean) =>
    Object.defineProperty(window, 'isSecureContext', { configurable: true, value });

let restore: () => void = () => {};
let warn: ReturnType<typeof vi.spyOn>;
beforeAll(() => {
    secureContext(true);
    window.addEventListener('wallet-standard:register-wallet', onRegister);
    warn = vi.spyOn(console, 'warn').mockImplementation(() => {}); // "MWA not registered: …"
});
afterAll(() => {
    window.removeEventListener('wallet-standard:register-wallet', onRegister);
    warn.mockRestore();
});
afterEach(() => {
    restore();
    restore = () => {};
});

const MATRIX = Object.keys(UA).filter((n) => n !== 'firefoxAndroid') as UaName[];

describe('mwaEligible agrees with registerMwa', () => {
    it('the page is a secure context (as an https deploy is)', () => {
        expect(window.isSecureContext).toBe(true);
    });

    it.each(MATRIX)('%s', (name) => {
        restore = stubNavigator({ userAgent: UA[name] });
        const lib = libraryRegisters();
        expect(mwaEligible(UA[name], window.isSecureContext)).toBe(lib);
        if (lib) expect(registered.at(-1)?.name).toBe('Mobile Wallet Adapter');
    });

    it('both refuse an insecure context', () => {
        restore = stubNavigator({ userAgent: UA.androidChrome });
        secureContext(false);
        try {
            expect(libraryRegisters()).toBe(false);
            expect(mwaEligible(UA.androidChrome, window.isSecureContext)).toBe(false);
        } finally {
            secureContext(true);
        }
    });

    it('Firefox for Android: the library registers, the app refuses (the intended difference)', () => {
        restore = stubNavigator({ userAgent: UA.firefoxAndroid });
        expect(libraryRegisters()).toBe(true);
        expect(mwaEligible(UA.firefoxAndroid, true)).toBe(false);
    });

    it('registers on Android Chrome and nowhere on a desktop (no remote wallet without its keys)', () => {
        restore = stubNavigator({ userAgent: UA.androidChrome });
        expect(libraryRegisters()).toBe(true);
        restore();
        restore = stubNavigator({ userAgent: UA.windowsChrome });
        expect(libraryRegisters()).toBe(false);
    });
});
