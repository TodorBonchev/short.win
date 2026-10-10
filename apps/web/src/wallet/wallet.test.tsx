/**
 * The wallet chooser on phones and desktops (FRONTEND.md §5.4, §7.6, §10.1): without a wallet a
 * phone gets the three open-in-wallet links (exact hrefs) in a dialog, a desktop the install links;
 * one chooser at a time; a press outside closes it; the connected menu's account actions.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { T } from '../config/testids.ts';
import { setMedia } from '../test/media.ts';
import { browseLink } from './deepLinks.ts';

const ADDRESS = '9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin';
const h = vi.hoisted(() => ({
    wallets: [] as unknown[],
    connected: null as unknown,
}));

vi.mock('../rpc/context.tsx', () => ({
    useAppClient: () => ({ rpc: { getBalance: () => ({ send: async () => ({ value: 2_500_000_000n }) }) } }),
}));
vi.mock('@solana/kit-plugin-wallet/react', () => ({
    useWalletStatus: () => (h.connected ? 'connected' : 'disconnected'),
    useWallets: () => h.wallets,
    useConnectedWallet: () => h.connected,
    useIsWalletReady: () => true,
    useConnect: () => ({ dispatch: vi.fn(), isRunning: false }),
    useDisconnect: () => ({ dispatch: vi.fn() }),
}));
vi.mock('@tanstack/react-router', () => ({
    Link: ({ to, children, replace: _replace, ...rest }: { to: string; replace?: boolean; children: ReactNode }) => (
        <a href={to} {...rest}>
            {children}
        </a>
    ),
    useRouter: () => undefined,
}));

const { WalletButton } = await import('./WalletButton.tsx');
const { ConnectCta } = await import('./ConnectCta.tsx');
const { walletChooser } = await import('./walletUi.ts');

function wrap(ui: ReactNode) {
    return <QueryClientProvider client={new QueryClient()}>{ui}</QueryClientProvider>;
}

afterEach(() => {
    h.wallets = [];
    h.connected = null;
    act(() => walletChooser.set(null));
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
        expect(links.map((a) => a.getAttribute('data-testid'))).toEqual([
            T.walletOpenIn('backpack'),
            T.walletOpenIn('phantom'),
            T.walletOpenIn('solflare'),
        ]);
        for (const w of ['backpack', 'phantom', 'solflare'] as const) {
            expect(screen.getByTestId(T.walletOpenIn(w)).getAttribute('href')).toBe(
                browseLink(w, location.href, location.origin),
            );
        }
        expect(menu.textContent).toContain('Set your wallet to localnet first.');
        expect(screen.getByTestId(T.walletReload)).toBeTruthy();
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
