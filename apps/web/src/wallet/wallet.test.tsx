/**
 * The wallet chooser on phones and desktops (FRONTEND.md §5.4, §7.6, §10.1): without a wallet a
 * phone gets the three open-in-wallet links (exact hrefs) in a dialog, a desktop the install links;
 * one chooser at a time; a press outside closes it; the connected menu's account actions. The
 * mobile wallets (O-6, §5.4 (6)): the Mobile Wallet Adapter's entry and its local-network step, the
 * open-in links after the entries, the in-app-browser block, the not-found reopen, no "sends itself"
 * badge for the adapter, the connect guard.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { T } from '../config/testids.ts';
import { setMedia } from '../test/media.ts';
import { stubNavigator, UA } from '../test/userAgents.ts';
import { browseLink } from './deepLinks.ts';

const ADDRESS = '9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin';
const h = vi.hoisted(() => ({
    wallets: [] as unknown[],
    listeners: new Set<() => void>(),
    connected: null as unknown,
    status: null as string | null,
    connect: { dispatch: vi.fn(), isRunning: false },
}));
/** A wallet registers (or leaves) while the chooser is open. */
function setWallets(next: unknown[]) {
    h.wallets = next;
    for (const l of h.listeners) l();
}

vi.mock('../rpc/context.tsx', () => ({
    useAppClient: () => ({ rpc: { getBalance: () => ({ send: async () => ({ value: 2_500_000_000n }) }) } }),
}));
vi.mock('@solana/kit-plugin-wallet/react', async () => {
    const { useSyncExternalStore } = await import('react');
    const subscribe = (l: () => void) => {
        h.listeners.add(l);
        return () => h.listeners.delete(l);
    };
    return {
        useWalletStatus: () => h.status ?? (h.connected ? 'connected' : 'disconnected'),
        useWallets: () => useSyncExternalStore(subscribe, () => h.wallets),
        useConnectedWallet: () => h.connected,
        useIsWalletReady: () => true,
        useConnect: () => h.connect,
        useDisconnect: () => ({ dispatch: vi.fn() }),
    };
});
vi.mock('@tanstack/react-router', () => ({
    Link: ({ to, children, replace: _replace, ...rest }: { to: string; replace?: boolean; children: ReactNode }) => (
        <a href={to} {...rest}>
            {children}
        </a>
    ),
    useRouter: () => undefined,
}));
vi.mock('./mwaCall.ts', () => ({ guardMwaConnect: vi.fn(() => () => {}) }));

const { WalletButton } = await import('./WalletButton.tsx');
const { ConnectCta } = await import('./ConnectCta.tsx');
const { walletChooser } = await import('./walletUi.ts');
const { mwaRegistered, mwaWalletNotFound } = await import('./mwa.ts');
const { localNetwork } = await import('./localNetwork.ts');
const { guardMwaConnect } = await import('./mwaCall.ts');

function wrap(ui: ReactNode) {
    return <QueryClientProvider client={new QueryClient()}>{ui}</QueryClientProvider>;
}

const MWA = {
    name: 'Mobile Wallet Adapter',
    icon: 'data:image/svg+xml;base64,PHN2Zy8+',
    features: ['standard:connect', 'solana:signAndSendTransaction', 'solana:signTransaction'],
};
const PERMISSION = "Allow 'Apps on device' so this page can reach your wallet app";
const OPEN_IN = ['backpack', 'phantom', 'solflare'] as const;

let restoreUa: () => void = () => {};
/** An Android phone (Chrome): the Mobile Wallet Adapter registered, a phone-width sheet. */
function android(o: { registered?: boolean; ua?: string } = {}) {
    restoreUa = stubNavigator({ userAgent: o.ua ?? UA.androidChrome, maxTouchPoints: 5 });
    setMedia({ phone: true, coarse: true });
    act(() => mwaRegistered.set(o.registered ?? true));
}
const option = (name = MWA.name) => screen.getByTestId(T.walletOption(name));

afterEach(() => {
    h.wallets = [];
    h.connected = null;
    h.status = null;
    h.connect.dispatch.mockReset();
    vi.mocked(guardMwaConnect).mockClear();
    restoreUa();
    restoreUa = () => {};
    act(() => {
        walletChooser.set(null);
        mwaRegistered.set(false);
        mwaWalletNotFound.set(null);
        localNetwork.set('ready');
    });
    vi.unstubAllGlobals();
    delete (navigator as unknown as Record<string, unknown>).clipboard;
});

describe('no wallet', () => {
    it('on a phone: a dialog with the open-in links for this page, alphabetically, and no empty menu', () => {
        setMedia({ phone: true, coarse: true });
        render(wrap(<WalletButton />));
        expect(screen.getByTestId(T.walletButton).textContent).toBe('Connect wallet');
        fireEvent.click(screen.getByTestId(T.walletButton));
        const menu = screen.getByTestId(T.walletMenu);
        expect(menu.getAttribute('role')).toBe('dialog');
        const title = document.getElementById(menu.getAttribute('aria-labelledby') ?? '');
        expect(title?.textContent).toBe('Connect a wallet');
        expect(screen.getByTestId(T.walletSheetClose).getAttribute('aria-label')).toBe('Close');
        expect(menu.textContent).not.toMatch(/·/);
        expect(menu.querySelector('[role="menu"]')).toBeNull();
        const links = [...menu.querySelectorAll('a[data-testid^="wallet-open-in-"]')];
        expect(links.map((a) => a.getAttribute('data-testid'))).toEqual(OPEN_IN.map((w) => T.walletOpenIn(w)));
        for (const w of OPEN_IN) {
            expect(screen.getByTestId(T.walletOpenIn(w)).getAttribute('href')).toBe(
                browseLink(w, location.href, location.origin),
            );
        }
        expect(menu.textContent).toContain('Set your wallet to localnet first.');
        expect(screen.getByTestId(T.walletReload)).toBeTruthy();
        // Not an in-app browser: no "Open this page in your browser".
        expect(screen.queryByTestId(T.walletInAppBrowser)).toBeNull();
        fireEvent.click(screen.getByTestId(T.walletSheetClose));
        expect(screen.queryByTestId(T.walletMenu)).toBeNull();
    });

    it('on a desktop: the install links and Reload, no open-in links', () => {
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(screen.getByTestId(T.walletMenu).getAttribute('role')).toBe('dialog');
        expect(screen.queryByTestId(T.walletOpenIn('phantom'))).toBeNull();
        expect(screen.getByTestId(T.walletInstall('phantom')).getAttribute('href')).toBe('https://phantom.com/');
        expect(screen.getByTestId(T.walletReload)).toBeTruthy();
    });

    it('mounts one chooser at a time; a press outside closes the header one', () => {
        setMedia({ phone: true, coarse: true });
        render(
            wrap(
                <>
                    <WalletButton />
                    <ConnectCta surface="short" />
                </>,
            ),
        );
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(screen.getAllByTestId(T.walletOpenIn('phantom'))).toHaveLength(1);
        fireEvent.click(screen.getByTestId(T.connectCta('short')));
        expect(screen.queryByTestId(T.walletMenu)).toBeNull();
        expect(screen.getAllByTestId(T.walletOpenIn('phantom'))).toHaveLength(1);
        act(() => walletChooser.set('header'));
        expect(screen.getByTestId(T.walletMenu)).toBeTruthy();
        fireEvent.pointerDown(document.body);
        expect(screen.queryByTestId(T.walletMenu)).toBeNull();
    });
});

describe('connected', () => {
    it('the menu has the full address, copy, explorer, my positions, my lending and disconnect', async () => {
        h.connected = {
            wallet: { name: 'Phantom', features: [] },
            account: { address: ADDRESS },
            supportedTransactionVersions: new Set([0]),
        };
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        const menu = screen.getByTestId(T.walletMenu);
        expect(menu.textContent).toContain(ADDRESS);
        for (const id of [
            T.walletCopyAddress,
            T.walletExplorer,
            T.walletMyPositions,
            T.walletMyLending,
            T.walletDisconnect,
        ]) {
            expect(screen.getByTestId(id).getAttribute('role')).toBe('menuitem');
        }
        expect(screen.getByTestId(T.walletExplorer).getAttribute('href')).toContain(ADDRESS);
        expect(await screen.findByText('2.5 SOL')).toBeTruthy();
    });

    it('a ConnectCta renders nothing once connected', () => {
        h.connected = {
            wallet: { name: 'Phantom', features: [] },
            account: { address: ADDRESS },
            supportedTransactionVersions: new Set([0]),
        };
        render(wrap(<ConnectCta surface="lend" />));
        expect(screen.queryByTestId(T.connectCta('lend'))).toBeNull();
    });
});

describe('mobile wallets (O-6)', () => {
    it('(a) a desktop wallet named "Mobile Wallet Adapter" (not registered by the app): shown as reported', () => {
        h.wallets = [MWA];
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(option().textContent).toBe('Mobile Wallet Adapter');
        expect(option().hasAttribute('data-mwa')).toBe(false);
        expect(screen.queryByTestId(T.walletMwaHint)).toBeNull();
        expect(screen.queryByTestId(T.walletOpenElsewhere)).toBeNull();
    });

    it('(b) an iPhone without a wallet: the dialog as before, no MWA entry, no in-app-browser block', () => {
        restoreUa = stubNavigator({ userAgent: UA.iphoneSafari, maxTouchPoints: 5 });
        setMedia({ phone: true, coarse: true });
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(screen.getByTestId(T.walletMenu).getAttribute('role')).toBe('dialog');
        expect(screen.queryByTestId(T.walletOption(MWA.name))).toBeNull();
        expect(screen.queryByTestId(T.walletInAppBrowser)).toBeNull();
        expect(screen.getAllByTestId(/^wallet-open-in-/)).toHaveLength(3);
    });

    it('(c) a Telegram WebView without a wallet: "Open this page in your browser" first, Copy link', async () => {
        restoreUa = stubNavigator({ userAgent: UA.telegramAndroid, maxTouchPoints: 5 });
        setMedia({ phone: true, coarse: true });
        const writeText = vi.fn(async (_: string) => {});
        Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        const block = screen.getByTestId(T.walletInAppBrowser);
        expect(block.textContent).toContain('Open this page in your browser');
        // First in the chooser, before the open-in links.
        const chooser = block.parentElement as HTMLElement;
        expect(chooser.firstElementChild).toBe(block);
        const first = screen.getByTestId(T.walletOpenIn('backpack'));
        expect(block.compareDocumentPosition(first) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
        const copy = screen.getByTestId(T.walletCopyLink);
        expect(copy.textContent).toBe('Copy link');
        fireEvent.click(copy);
        expect(writeText).toHaveBeenCalledWith(location.href);
        await waitFor(() => expect(copy.textContent).toBe('Copied'));
    });

    it('(c) Copy link falls back to execCommand inside the chooser when the clipboard refuses', async () => {
        restoreUa = stubNavigator({ userAgent: UA.telegramAndroid, maxTouchPoints: 5 });
        setMedia({ phone: true, coarse: true });
        Object.defineProperty(navigator, 'clipboard', {
            configurable: true,
            value: { writeText: vi.fn(async () => Promise.reject(new Error('denied'))) },
        });
        const seen: { inBlock: boolean; value: string }[] = [];
        const exec = vi.fn((cmd: string) => {
            const t = document.activeElement as HTMLTextAreaElement | null;
            const box = screen.getByTestId(T.walletInAppBrowser);
            const area = box.querySelector('textarea');
            seen.push({ inBlock: !!area && box.contains(area), value: area?.value ?? t?.value ?? '' });
            return cmd === 'copy';
        });
        Object.defineProperty(document, 'execCommand', { configurable: true, value: exec });
        try {
            render(wrap(<WalletButton />));
            fireEvent.click(screen.getByTestId(T.walletButton));
            const copy = screen.getByTestId(T.walletCopyLink);
            fireEvent.click(copy);
            await waitFor(() => expect(exec).toHaveBeenCalledWith('copy'));
            expect(seen).toEqual([{ inBlock: true, value: location.href }]);
            // The temporary textarea is gone, and the button says so.
            expect(screen.getByTestId(T.walletInAppBrowser).querySelector('textarea')).toBeNull();
            await waitFor(() => expect(copy.textContent).toBe('Copied'));
        } finally {
            delete (document as unknown as Record<string, unknown>).execCommand;
        }
    });

    it('(d) Android with the adapter only: "Use installed wallet", then the open-in links outside the menu', () => {
        android();
        h.wallets = [MWA];
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        const entry = option();
        expect(entry.textContent).toContain('Use installed wallet');
        expect(entry.textContent).toContain('Opens your wallet app on this phone');
        expect(entry.textContent).not.toContain('Mobile Wallet Adapter');
        expect(screen.getByTestId(T.walletMwaHint).textContent).toBe('Opens your wallet app on this phone');
        expect(entry.getAttribute('data-mwa')).toBe('');
        expect(entry.getAttribute('data-local-network')).toBe('ready');
        expect(entry.querySelector('img')?.getAttribute('src')).toBe(MWA.icon);
        const menu = screen.getByRole('menu');
        expect([...menu.querySelectorAll('[role="menuitem"]')]).toEqual([entry]);
        const elsewhere = screen.getByTestId(T.walletOpenElsewhere);
        expect(menu.contains(elsewhere)).toBe(false);
        expect(elsewhere.textContent).toContain('Or open this page in a wallet app');
        expect(elsewhere.textContent).toContain('Set your wallet to localnet first.');
        const links = [...elsewhere.querySelectorAll('a')];
        expect(links.map((a) => a.getAttribute('href'))).toEqual(
            OPEN_IN.map((w) => browseLink(w, location.href, location.origin)),
        );
        // No "sends itself" badge or note for the adapter (it only signs through it).
        expect(screen.queryByTestId(T.walletSendsItself)).toBeNull();
        expect(screen.queryByText(/sends transactions itself/)).toBeNull();
    });

    it('(e) local network "prompt": the permission text; a tap asks Chrome and does not connect; ready: it connects', () => {
        android();
        h.wallets = [MWA];
        const fetch = vi.fn(async () => new Response(null));
        vi.stubGlobal('fetch', fetch);
        act(() => localNetwork.set('prompt'));
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(screen.getByTestId(T.walletMwaHint).textContent).toBe(PERMISSION);
        expect(option().getAttribute('data-local-network')).toBe('prompt');
        fireEvent.click(option());
        expect(fetch).toHaveBeenCalledOnce();
        expect(fetch).toHaveBeenCalledWith('http://localhost', expect.objectContaining({ mode: 'no-cors' }));
        expect(h.connect.dispatch).not.toHaveBeenCalled();
        expect(guardMwaConnect).not.toHaveBeenCalled();
        expect(screen.getByTestId(T.walletMenu)).toBeTruthy(); // the popup stays
        // Chrome's permission granted: the change event flips the store.
        act(() => localNetwork.set('ready'));
        expect(screen.getByTestId(T.walletMwaHint).textContent).toBe('Opens your wallet app on this phone');
        fireEvent.click(option());
        expect(h.connect.dispatch).toHaveBeenCalledWith(MWA);
        expect(fetch).toHaveBeenCalledOnce();
        expect(screen.queryByTestId(T.walletMenu)).toBeNull();
    });

    it('(f) local network "denied": a tap neither connects nor asks', () => {
        android();
        h.wallets = [MWA];
        const fetch = vi.fn(async () => new Response(null));
        vi.stubGlobal('fetch', fetch);
        act(() => localNetwork.set('denied'));
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(screen.getByTestId(T.walletMwaHint).textContent).toBe(PERMISSION);
        fireEvent.click(option());
        expect(fetch).not.toHaveBeenCalled();
        expect(h.connect.dispatch).not.toHaveBeenCalled();
        expect(screen.getByTestId(T.walletMenu)).toBeTruthy();
    });

    it('(g) two wallets named "Mobile Wallet Adapter": no friendly label, both duplicate badges', () => {
        android();
        h.wallets = [MWA, { ...MWA, features: ['standard:connect', 'solana:signAndSendTransaction'] }];
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        for (const name of [MWA.name, `${MWA.name} (2)`]) {
            expect(option(name).textContent).toContain('Mobile Wallet Adapter');
            expect(option(name).textContent).not.toContain('Use installed wallet');
        }
        expect(screen.queryByTestId(T.walletMwaHint)).toBeNull();
        expect(screen.getAllByTestId(T.walletDuplicate)).toHaveLength(2);
    });

    it('(h) a wallet registering while the chooser is open replaces the no-wallet dialog', () => {
        android();
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(screen.getByTestId(T.walletMenu).getAttribute('role')).toBe('dialog');
        expect(screen.getByTestId(T.walletReload)).toBeTruthy();
        act(() => setWallets([MWA]));
        expect(option().textContent).toContain('Use installed wallet');
        expect(screen.getByRole('menu')).toBeTruthy();
        expect(screen.getByTestId(T.walletOpenElsewhere)).toBeTruthy();
        expect(screen.queryByTestId(T.walletReload)).toBeNull();
    });

    it('(i) the inline ConnectCta with the adapter only: the same block, one chooser at a time', () => {
        android();
        h.wallets = [MWA];
        render(
            wrap(
                <>
                    <WalletButton />
                    <ConnectCta surface="lend" />
                </>,
            ),
        );
        fireEvent.click(screen.getByTestId(T.connectCta('lend')));
        expect(option().textContent).toContain('Use installed wallet');
        expect(screen.getAllByTestId(T.walletOpenElsewhere)).toHaveLength(1);
        expect(screen.queryByRole('menu')).toBeNull(); // inline: a plain group
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(screen.getAllByTestId(T.walletOpenElsewhere)).toHaveLength(1);
        expect(screen.getAllByTestId(T.walletOption(MWA.name))).toHaveLength(1);
        for (const w of OPEN_IN) expect(screen.getAllByTestId(T.walletOpenIn(w))).toHaveLength(1);
    });

    it('(j) no wallet app answered a Connect: the header chooser reopens; not while connected; not when stale', () => {
        android();
        h.wallets = [MWA];
        const { rerender } = render(wrap(<WalletButton />));
        expect(screen.queryByTestId(T.walletMenu)).toBeNull();
        act(() => mwaWalletNotFound.set(Date.now()));
        expect(screen.getByTestId(T.walletMenu)).toBeTruthy();
        expect(screen.getByTestId(T.walletOpenElsewhere)).toBeTruthy();
        expect(mwaWalletNotFound.get()).toBeNull();
        act(() => walletChooser.set(null));
        // Connected (a launch failure at Sign never sets it, but a report must not act then).
        h.connected = {
            wallet: { name: MWA.name, features: [] },
            account: { address: ADDRESS },
            supportedTransactionVersions: new Set([0]),
        };
        rerender(wrap(<WalletButton />));
        const t = Date.now();
        act(() => mwaWalletNotFound.set(t));
        expect(screen.queryByTestId(T.walletMenu)).toBeNull();
        expect(mwaWalletNotFound.get()).toBe(t);
        // A stale report (older than 10 s) is dropped without reopening once disconnected.
        act(() => mwaWalletNotFound.set(Date.now() - 11_000));
        h.connected = null;
        rerender(wrap(<WalletButton />));
        expect(mwaWalletNotFound.get()).toBeNull();
        expect(screen.queryByTestId(T.walletMenu)).toBeNull();
    });

    it('(k) the adapter never carries the "sends itself" badge; another sending-only wallet still does', () => {
        android();
        const sendingOnly = ['standard:connect', 'solana:signAndSendTransaction'];
        h.wallets = [{ ...MWA, features: sendingOnly }];
        const { unmount } = render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(option().textContent).toContain('Use installed wallet');
        expect(screen.queryByTestId(T.walletSendsItself)).toBeNull();
        expect(screen.queryByText(/sends transactions itself/)).toBeNull();
        unmount();
        act(() => walletChooser.set(null));
        h.wallets = [
            { ...MWA, features: sendingOnly },
            { name: 'Sender', features: sendingOnly },
        ];
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(screen.getAllByTestId(T.walletSendsItself)).toHaveLength(1);
        expect(option('Sender').querySelector(`[data-testid="${T.walletSendsItself}"]`)).toBeTruthy();
        expect(screen.getByText(/sends transactions itself/)).toBeTruthy();
    });

    it('(l) a tap on the adapter (ready) dispatches the connect, then starts the connect guard once', () => {
        android();
        h.wallets = [MWA, { name: 'Phantom', features: ['standard:connect', 'solana:signTransaction'] }];
        render(wrap(<WalletButton />));
        fireEvent.click(screen.getByTestId(T.walletButton));
        fireEvent.click(option());
        expect(h.connect.dispatch).toHaveBeenCalledOnce();
        expect(guardMwaConnect).toHaveBeenCalledOnce();
        expect(h.connect.dispatch.mock.invocationCallOrder[0]).toBeLessThan(
            vi.mocked(guardMwaConnect).mock.invocationCallOrder[0] ?? 0,
        );
        // Another wallet's tap starts no guard.
        act(() => walletChooser.set('header'));
        fireEvent.click(option('Phantom'));
        expect(h.connect.dispatch).toHaveBeenCalledTimes(2);
        expect(guardMwaConnect).toHaveBeenCalledOnce();
    });
});
