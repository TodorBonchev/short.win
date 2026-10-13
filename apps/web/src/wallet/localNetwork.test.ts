/**
 * @vitest-environment-options { "url": "https://short.win/markets" }
 *
 * Chrome's Local Network Access step for the Mobile Wallet Adapter (FRONTEND.md §5.4 (6); mobile.md
 * §1.4): the permission's state as the library reads it, its changes, the library's exact error test
 * (a TypeError naming the permission: not gated; anything else: blocked), the exemptions, and the
 * request that raises Chrome's prompt.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { stubNavigator, UA } from '../test/userAgents.ts';
import { isLoopbackHost, localNetwork, requestLocalNetwork, watchLocalNetwork } from './localNetwork.ts';

class FakeStatus extends EventTarget {
    state: PermissionState;
    constructor(state: PermissionState) {
        super();
        this.state = state;
    }
}

const original = Object.getOwnPropertyDescriptor(navigator, 'permissions');
function stubPermissions(query: (d: { name: string }) => Promise<unknown>) {
    const spy = vi.fn(query);
    Object.defineProperty(navigator, 'permissions', { configurable: true, value: { query: spy } });
    return spy;
}

let restoreUa: () => void = () => {};
beforeEach(() => localNetwork.set('ready'));
afterEach(() => {
    restoreUa();
    restoreUa = () => {};
    if (original) Object.defineProperty(navigator, 'permissions', original);
    else delete (navigator as unknown as Record<string, unknown>).permissions;
    vi.unstubAllGlobals();
});

describe('watchLocalNetwork', () => {
    it.each([
        ['granted', 'ready'],
        ['prompt', 'prompt'],
        ['denied', 'denied'],
    ] as const)('%s gives %s', async (state, expected) => {
        expect(location.hostname).toBe('short.win');
        localNetwork.set(state === 'granted' ? 'denied' : 'ready');
        const query = stubPermissions(async () => new FakeStatus(state));
        await watchLocalNetwork();
        expect(query).toHaveBeenCalledWith({ name: 'loopback-network' });
        expect(localNetwork.get()).toBe(expected);
    });

    it('follows the permission’s changes (an allow in Chrome’s prompt or in site settings)', async () => {
        const status = new FakeStatus('prompt');
        stubPermissions(async () => status);
        await watchLocalNetwork();
        expect(localNetwork.get()).toBe('prompt');
        status.state = 'granted';
        status.dispatchEvent(new Event('change'));
        expect(localNetwork.get()).toBe('ready');
        status.state = 'denied';
        status.dispatchEvent(new Event('change'));
        expect(localNetwork.get()).toBe('denied');
        // Watching again (a tap while denied) follows the new status only.
        const next = new FakeStatus('denied');
        stubPermissions(async () => next);
        await watchLocalNetwork();
        status.state = 'granted';
        status.dispatchEvent(new Event('change'));
        expect(localNetwork.get()).toBe('denied');
        next.state = 'granted';
        next.dispatchEvent(new Event('change'));
        expect(localNetwork.get()).toBe('ready');
    });

    it('a browser that does not know the permission is not gated (the library’s TypeError test)', async () => {
        for (const message of [
            "Failed to execute 'query' on 'Permissions': The provided value 'loopback-network' is not a valid enum value",
            'local-network-access is not a known permission',
        ]) {
            localNetwork.set('denied');
            stubPermissions(async () => {
                throw new TypeError(message);
            });
            await watchLocalNetwork();
            expect(localNetwork.get(), message).toBe('ready');
        }
    });

    it('any other failure, or no navigator.permissions, reads as blocked (as the library)', async () => {
        for (const thrown of [new TypeError('query is not a function'), new Error('boom'), 'odd']) {
            localNetwork.set('ready');
            stubPermissions(async () => {
                throw thrown;
            });
            await watchLocalNetwork();
            expect(localNetwork.get(), String(thrown)).toBe('denied');
        }
        localNetwork.set('ready');
        Object.defineProperty(navigator, 'permissions', { configurable: true, value: undefined });
        await watchLocalNetwork();
        expect(localNetwork.get()).toBe('denied');
    });

    it('the Solana Mobile Web Shell is exempt: ready, without a query', async () => {
        restoreUa = stubNavigator({ userAgent: UA.webShell });
        const query = stubPermissions(async () => new FakeStatus('denied'));
        localNetwork.set('ready');
        await watchLocalNetwork();
        expect(query).not.toHaveBeenCalled();
        expect(localNetwork.get()).toBe('ready');
    });

    it('a loopback origin (the E2E server’s 127.0.0.1) is exempt: ready, without a query', async () => {
        // Vitest's jsdom environment exposes its JSDOM instance; reconfigure moves the page.
        const dom = (globalThis as { jsdom?: { reconfigure(o: { url: string }): void } }).jsdom;
        expect(dom).toBeDefined();
        dom?.reconfigure({ url: 'http://127.0.0.1:14173/markets' });
        try {
            expect(location.hostname).toBe('127.0.0.1');
            const query = stubPermissions(async () => new FakeStatus('denied'));
            await watchLocalNetwork();
            expect(query).not.toHaveBeenCalled();
            expect(localNetwork.get()).toBe('ready');
        } finally {
            dom?.reconfigure({ url: 'https://short.win/markets' });
        }
    });

    it('a loopback origin is exempt (the library’s isLoopbackHost)', () => {
        for (const h of ['localhost', 'app.localhost', '127.0.0.1', '127.255.0.9', '[::1]']) {
            expect(isLoopbackHost(h), h).toBe(true);
        }
        for (const h of ['short.win', 'localhost.short.win', '128.0.0.1', '10.0.0.1', '127.0.0.1.nip.io']) {
            expect(isLoopbackHost(h), h).toBe(false);
        }
    });
});

describe('requestLocalNetwork', () => {
    it('fetches http://localhost once, no-cors, and swallows the refusal', async () => {
        const fetch = vi.fn(async () => {
            throw new TypeError('Failed to fetch');
        });
        vi.stubGlobal('fetch', fetch);
        const unhandled = vi.fn();
        process.on('unhandledRejection', unhandled);
        requestLocalNetwork();
        await new Promise((r) => setTimeout(r, 0));
        process.off('unhandledRejection', unhandled);
        expect(fetch).toHaveBeenCalledOnce();
        expect(fetch).toHaveBeenCalledWith('http://localhost', {
            mode: 'no-cors',
            credentials: 'omit',
            cache: 'no-store',
        });
        expect(unhandled).not.toHaveBeenCalled();
    });
});
