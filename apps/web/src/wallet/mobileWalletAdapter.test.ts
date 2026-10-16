/**
 * Registering the Mobile Wallet Adapter (FRONTEND.md §5.4 (6); mobile.md §1.3, §2.2): only an
 * eligible Android browser with the flag on loads the package; `registerMwa`'s arguments (the app's
 * identity, the build's chain, the chain-scoped cache, the app's own not-found handler, no remote
 * association); registration verified against the plugin's list; the cache's chain rule; the
 * observer that removes the library's hosts as they are inserted; the icon.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { decodePng, isIconOf, renderIcon192, renderPixels } from '../../scripts/icon-192.ts';
import { stubNavigator, UA, type UaName } from '../test/userAgents.ts';

const h = vi.hoisted(() => ({
    /** How many times the package was imported (the mock factory ran). */
    imported: 0,
    calls: [] as Record<string, unknown>[],
    /** What the mocked registerMwa does to the plugin's list. */
    registers: true,
    wallets: [] as { name: string }[],
    defaultNotFound: async () => {},
    defaultCache: null as unknown,
}));

vi.mock('@solana-mobile/wallet-standard-mobile', () => {
    h.imported++;
    return {
        registerMwa: (cfg: Record<string, unknown>) => {
            h.calls.push(cfg);
            if (h.registers) h.wallets.push({ name: 'Mobile Wallet Adapter' });
        },
        createDefaultAuthorizationCache: () => h.defaultCache,
        createDefaultChainSelector: () => ({ select: async (c: string[]) => c[0] }),
        createDefaultWalletNotFoundHandler: () => h.defaultNotFound,
    };
});
vi.mock('./localNetwork.ts', () => ({ watchLocalNetwork: vi.fn(async () => {}) }));

const client = { wallet: { getState: () => ({ wallets: h.wallets }) } };
const ON = { enableMobileWalletAdapter: true, chain: 'solana:devnet' } as const;

/** A fresh copy of the modules (the registration runs once per page load). */
async function load() {
    vi.resetModules();
    return {
        ...(await import('./mobileWalletAdapter.ts')),
        mwa: await import('./mwa.ts'),
        localNetwork: await import('./localNetwork.ts'),
    };
}

/** jsdom has no `isSecureContext`; its localhost page is one in a browser, as an https deploy is. */
const secureContext = (value: boolean) =>
    Object.defineProperty(window, 'isSecureContext', { configurable: true, value });

let restore: () => void = () => {};
beforeEach(() => {
    // A fresh <body> per test: an earlier registration's observer stays on the old one (as it stays
    // installed for the page's life in the app).
    document.body = document.createElement('body');
    secureContext(true);
    h.calls = [];
    h.registers = true;
    h.wallets = [{ name: 'Phantom' }];
    h.defaultCache = { get: vi.fn(), set: vi.fn(), clear: vi.fn() };
});
afterEach(() => {
    restore();
    restore = () => {};
    vi.unstubAllGlobals();
});

describe('registerMobileWalletAdapter: gating', () => {
    it('flag off: the package is not imported', async () => {
        restore = stubNavigator({ userAgent: UA.androidChrome });
        const m = await load();
        const before = h.imported;
        expect(await m.registerMobileWalletAdapter({ ...ON, enableMobileWalletAdapter: false }, client)).toBe(false);
        expect(h.imported).toBe(before);
        expect(m.mwa.mwaRegistered.get()).toBe(false);
    });

    it.each(['windowsChrome', 'iphoneSafari', 'androidWebView', 'telegramAndroid', 'firefoxAndroid'] as UaName[])(
        'flag on, %s: not imported',
        async (name) => {
            restore = stubNavigator({ userAgent: UA[name] });
            const m = await load();
            const before = h.imported;
            expect(await m.registerMobileWalletAdapter(ON, client)).toBe(false);
            expect(h.imported).toBe(before);
            expect(h.calls).toHaveLength(0);
        },
    );

    it('flag on, Android Chrome but an insecure context: not imported', async () => {
        restore = stubNavigator({ userAgent: UA.androidChrome });
        secureContext(false);
        const m = await load();
        const before = h.imported;
        expect(await m.registerMobileWalletAdapter(ON, client)).toBe(false);
        expect(h.imported).toBe(before);
    });
});

describe('registerMobileWalletAdapter: Android Chrome in a secure context', () => {
    it('calls registerMwa once with the app’s identity, the chain, the scoped cache and its own not-found handler', async () => {
        restore = stubNavigator({ userAgent: UA.androidChrome });
        expect(window.isSecureContext).toBe(true);
        const m = await load();
        const before = h.imported;
        expect(await m.registerMobileWalletAdapter(ON, client)).toBe(true);
        expect(h.imported).toBe(before + 1);
        expect(h.calls).toHaveLength(1);
        const cfg = h.calls[0] as Record<string, unknown>;
        expect(cfg.appIdentity).toEqual({ name: 'ShortWin', uri: location.origin, icon: 'icon-192.png' });
        expect(cfg.chains).toEqual(['solana:devnet']);
        expect(cfg.chainSelector).toBeDefined();
        expect(typeof cfg.onWalletNotFound).toBe('function');
        expect(cfg.onWalletNotFound).not.toBe(h.defaultNotFound);
        // No remote (QR) association: neither key, not even as undefined.
        expect('remoteHostAuthority' in cfg).toBe(false);
        expect('nostrRelay' in cfg).toBe(false);
        // The library's default cache, wrapped (the chain rule below).
        const cache = cfg.authorizationCache as { get(): Promise<unknown> };
        expect(cache).not.toBe(h.defaultCache);
        const inner = h.defaultCache as { get: ReturnType<typeof vi.fn>; clear: ReturnType<typeof vi.fn> };
        inner.get.mockResolvedValueOnce({ chain: 'solana:mainnet', accounts: [] });
        expect(await cache.get()).toBeUndefined();
        expect(inner.clear).toHaveBeenCalledOnce();
        expect(m.mwa.mwaRegistered.get()).toBe(true);
        expect(m.localNetwork.watchLocalNetwork).toHaveBeenCalledOnce();
        // Once per page load.
        expect(await m.registerMobileWalletAdapter(ON, client)).toBe(false);
        expect(h.calls).toHaveLength(1);
    });

    it.each(['samsungInternet', 'braveAndroid', 'webShell'] as UaName[])('%s registers too', async (name) => {
        restore = stubNavigator({ userAgent: UA[name] });
        const m = await load();
        expect(await m.registerMobileWalletAdapter(ON, client)).toBe(true);
    });

    it('a registerMwa that registers nothing (the library only warned): false, not registered, no LNA watch', async () => {
        restore = stubNavigator({ userAgent: UA.androidChrome });
        h.registers = false;
        const m = await load();
        expect(await m.registerMobileWalletAdapter(ON, client)).toBe(false);
        expect(h.calls).toHaveLength(1);
        expect(m.mwa.mwaRegistered.get()).toBe(false);
        expect(m.localNetwork.watchLocalNetwork).not.toHaveBeenCalled();
        // And its host observer was stopped: an app element of the spinner's shape stays.
        const div = document.createElement('div');
        div.style.setProperty('--spinner-color', '#FFFFFF');
        document.body.append(div);
        await Promise.resolve();
        expect(div.isConnected).toBe(true);
        div.remove();
    });

    it('a failure while registering is reported, its observer stopped, the app as it was', async () => {
        restore = stubNavigator({ userAgent: UA.androidChrome });
        const m = await load();
        const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
        h.wallets = null as unknown as { name: string }[]; // the plugin's list cannot be read
        expect(await m.registerMobileWalletAdapter(ON, client)).toBe(false);
        expect(warn).toHaveBeenCalledWith('Mobile Wallet Adapter not loaded', expect.anything());
        expect(m.mwa.mwaRegistered.get()).toBe(false);
        expect(m.localNetwork.watchLocalNetwork).not.toHaveBeenCalled();
        const div = document.createElement('div');
        div.style.setProperty('--spinner-color', '#FFFFFF');
        document.body.append(div);
        await Promise.resolve();
        expect(div.isConnected).toBe(true);
        div.remove();
        warn.mockRestore();
    });

    it('onWalletNotFound sets mwaWalletNotFound only while the wallet is not connected', async () => {
        restore = stubNavigator({ userAgent: UA.androidChrome });
        const m = await load();
        await m.registerMobileWalletAdapter(ON, client);
        const notFound = (h.calls[0] as { onWalletNotFound: (w: unknown) => Promise<void> }).onWalletNotFound;
        await notFound({ connected: true });
        expect(m.mwa.mwaWalletNotFound.get()).toBeNull();
        const t0 = Date.now();
        await notFound({ connected: false });
        expect(m.mwa.mwaWalletNotFound.get()).toBeGreaterThanOrEqual(t0);
    });
});

describe('chainScopedCache', () => {
    const auth = (chain: string) => ({ chain, auth_token: 't', accounts: [] }) as never;

    it('passes the build’s chain through; another chain is dropped and cleared once', async () => {
        const { chainScopedCache } = await load();
        const inner = { get: vi.fn(), set: vi.fn(async () => {}), clear: vi.fn(async () => {}) };
        const cache = chainScopedCache('solana:mainnet', inner);
        inner.get.mockResolvedValueOnce(auth('solana:mainnet'));
        expect(await cache.get()).toEqual(auth('solana:mainnet'));
        expect(inner.clear).not.toHaveBeenCalled();
        inner.get.mockResolvedValueOnce(auth('solana:devnet'));
        expect(await cache.get()).toBeUndefined();
        expect(inner.clear).toHaveBeenCalledOnce();
        inner.get.mockResolvedValueOnce(undefined);
        expect(await cache.get()).toBeUndefined();
        expect(inner.clear).toHaveBeenCalledOnce();
    });

    it('delegates set and clear', async () => {
        const { chainScopedCache } = await load();
        const inner = { get: vi.fn(), set: vi.fn(async () => {}), clear: vi.fn(async () => {}) };
        const cache = chainScopedCache('solana:mainnet', inner);
        await cache.set(auth('solana:mainnet'));
        expect(inner.set).toHaveBeenCalledWith(auth('solana:mainnet'));
        await cache.clear();
        expect(inner.clear).toHaveBeenCalledOnce();
    });
});

describe('removeLibraryUi (the library’s hosts never render)', () => {
    /** The library's `fonts` (every embedded modal's host holds them). */
    const FONTS = `
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Inter+Tight:ital,wght@0,100..900;1,100..900&display=swap" rel="stylesheet">
`;
    const tick = () => new Promise<void>((r) => queueMicrotask(r));
    let stop: () => void = () => {};
    afterEach(() => {
        stop();
        document.body.replaceChildren();
    });

    it('removes the spinner’s host (its --spinner-color) right after the append', async () => {
        const { removeLibraryUi } = await load();
        stop = removeLibraryUi();
        const host = document.createElement('div');
        host.attachShadow({ mode: 'closed' });
        host.style.setProperty('--spinner-color', '#FFFFFF');
        document.body.appendChild(host);
        await tick();
        expect(host.isConnected).toBe(false);
    });

    it('removes a modal’s host (the fonts links in its light DOM)', async () => {
        const { removeLibraryUi } = await load();
        stop = removeLibraryUi();
        const host = document.createElement('div');
        host.innerHTML = FONTS;
        host.attachShadow({ mode: 'closed' });
        document.body.appendChild(host);
        await tick();
        expect(host.isConnected).toBe(false);
    });

    it('leaves the app’s own elements, fonts links nested deeper, and anything appended inside a child', async () => {
        const { removeLibraryUi } = await load();
        stop = removeLibraryUi();
        const portal = document.createElement('div');
        portal.className = 'toast-region';
        const nested = document.createElement('div');
        nested.innerHTML = `<div>${FONTS}</div>`;
        document.body.append(portal, nested);
        const inside = document.createElement('div');
        inside.style.setProperty('--spinner-color', '#FFFFFF');
        portal.appendChild(inside);
        await tick();
        expect(portal.isConnected).toBe(true);
        expect(nested.isConnected).toBe(true);
        expect(inside.isConnected).toBe(true);
        // Stopped: nothing is removed any more.
        stop();
        const late = document.createElement('div');
        late.style.setProperty('--spinner-color', '#FFFFFF');
        document.body.appendChild(late);
        await tick();
        expect(late.isConnected).toBe(true);
    });
});

describe('the icon wallets show (appIdentity.icon)', () => {
    it('public/icon-192.png is the 192 × 192 rendering of favicon.svg (node scripts/icon-192.ts)', () => {
        const pub = join(process.cwd(), 'public');
        const png = readFileSync(join(pub, 'icon-192.png'));
        const svg = readFileSync(join(pub, 'favicon.svg'), 'utf8');
        // The decoded pixels, not the compressed bytes: another zlib may deflate the same image
        // differently (a Node upgrade), and that is not a change of the icon.
        const file = decodePng(png);
        expect([file.width, file.height]).toEqual([192, 192]);
        expect(Buffer.compare(Buffer.from(file.pixels), Buffer.from(renderPixels(svg)))).toBe(0);
        expect(isIconOf(png, svg)).toBe(true);
        // The decoder reads what the script writes, and another colour of the mark is not this icon.
        const rendered = renderIcon192(svg);
        expect(Buffer.compare(Buffer.from(decodePng(rendered).pixels), Buffer.from(file.pixels))).toBe(0);
        const recoloured = svg.replace('fill="#f0b90b"', 'fill="#123456"');
        expect(recoloured).not.toBe(svg);
        expect(isIconOf(png, recoloured)).toBe(false);
    });
});
