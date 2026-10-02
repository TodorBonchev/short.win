/**
 * Regression of M9-SEC-8 (wallet identity) and M9-SEC-1 (sending-only label): two registered
 * wallets with the same name and icon are told apart in the menu (distinct ids and keys, a
 * duplicate badge with the registration rank and feature set), a wallet that only signs-and-sends
 * is labelled, and a silent reconnect to an ambiguous name is undone.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { T } from '../config/testids.ts';

const ICON = 'data:image/svg+xml;base64,PHN2Zy8+';
const SIGN = ['standard:connect', 'solana:signTransaction'];
const real = { name: 'Phantom', icon: ICON, accounts: [], chains: ['solana:localnet'], features: SIGN };
const impostor = {
    name: 'Phantom',
    icon: ICON,
    accounts: [],
    chains: ['solana:localnet'],
    features: ['standard:connect', 'solana:signAndSendTransaction'],
};

const h = vi.hoisted(() => ({
    wallets: [] as unknown[],
    connected: null as unknown,
    disconnect: vi.fn(),
}));

vi.mock('../rpc/context.tsx', () => ({ useAppClient: () => ({}) }));
vi.mock('@solana/kit-plugin-wallet/react', () => ({
    useWalletStatus: () => (h.connected ? 'connected' : 'disconnected'),
    useWallets: () => h.wallets,
    useConnectedWallet: () => h.connected,
    useConnect: () => ({ dispatch: vi.fn(), isRunning: false }),
    useDisconnect: () => ({ dispatch: h.disconnect }),
}));

const { WalletButton, sendsItselfFeatures, walletRanks } = await import('../wallet/WalletButton.tsx');

afterEach(() => {
    h.wallets = [];
    h.connected = null;
    h.disconnect.mockReset();
});

describe('M9-SEC-8: same-name wallets are told apart', () => {
    it('distinct entries, ids and keys; a duplicate badge with rank and feature set', () => {
        h.wallets = [real, impostor];
        const errors = vi.spyOn(console, 'error').mockImplementation(() => {});
        render(<WalletButton />);
        fireEvent.click(screen.getByTestId(T.walletButton));
        const first = screen.getByTestId(T.walletOption('Phantom'));
        const second = screen.getByTestId(T.walletOption('Phantom (2)'));
        expect(first.innerHTML).not.toBe(second.innerHTML);
        expect(screen.getAllByTestId(T.walletDuplicate).map((b) => b.textContent)).toEqual([
            'same name · 1 of 2 · signs transactions',
            'same name · 2 of 2 · sends transactions itself',
        ]);
        expect(errors.mock.calls.flat().join(' ')).not.toMatch(/same key/);
        errors.mockRestore();
    });

    it('labels a wallet that can only sign-and-send (M9-SEC-1)', () => {
        h.wallets = [{ ...impostor, name: 'Sender' }];
        render(<WalletButton />);
        fireEvent.click(screen.getByTestId(T.walletButton));
        expect(screen.getByTestId(T.walletSendsItself).textContent).toMatch(/checked after landing/);
        expect(sendsItselfFeatures(['solana:signAndSendTransaction'])).toBe(true);
        expect(sendsItselfFeatures(['solana:signAndSendTransaction', 'solana:signTransaction'])).toBe(false);
        expect(walletRanks([real, impostor, { ...real, name: 'Other' }])).toEqual([
            { rank: 1, of: 2 },
            { rank: 2, of: 2 },
            { rank: 1, of: 1 },
        ]);
    });

    it('undoes a silent reconnect when another installed wallet uses the same name', () => {
        h.wallets = [real, impostor];
        h.connected = { wallet: real, account: { address: '9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin' } };
        render(<WalletButton />);
        expect(h.disconnect).toHaveBeenCalled();
    });

    it('keeps a silent reconnect when the name is unique', () => {
        h.wallets = [real];
        h.connected = { wallet: real, account: { address: '9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin' } };
        render(<WalletButton />);
        expect(h.disconnect).not.toHaveBeenCalled();
    });
});
