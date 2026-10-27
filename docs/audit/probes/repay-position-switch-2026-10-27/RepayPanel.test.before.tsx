import { type Address, createNoopSigner } from '@solana/kit';
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ToastProvider } from '../../../components/Toast.tsx';
import { parseMint } from '../../../rpc/mint.ts';
import { mockIndexer, renderAt } from '../../markets/test/harness.tsx';
import { decodePosition } from '../snapshot.ts';
import { testMath, vectorCase, vectorSnapshot } from '../testing.ts';
import { plainAmount } from '../ui.tsx';
import { previewRepay, type RepayPreview, type RepayState } from './repay.ts';

const store = vi.hoisted(() => ({
    address: '',
    state: null as RepayState | null,
    preview: null as RepayPreview | null,
    submit: vi.fn(),
}));
vi.mock('../../../wallet/useWallet.ts', () => ({
    useWallet: () => ({
        ready: true,
        status: store.address ? 'connected' : 'disconnected',
        connected: store.address
            ? {
                  address: store.address,
                  signer: createNoopSigner(store.address as Address),
                  walletName: 'Test',
                  supportsV0: true,
              }
            : null,
    }),
}));
vi.mock('../../../tx/TxReviewModal.tsx', () => ({
    TxDoneContext: { Provider: ({ children }: { children: unknown }) => children },
    TxReviewModal: () => null,
}));
vi.mock('./useRepay.ts', () => ({
    useRepay: () => ({
        state: { data: store.state, error: null, refetch: vi.fn() },
        math: {},
        mathError: null,
        preview: store.preview,
        previewError: null,
        flow: { state: { stage: 'idle', signature: null } },
        building: false,
        buildError: null,
        submit: store.submit,
    }),
}));
vi.mock('../hooks.ts', async (original) => ({
    ...(await original<typeof import('../hooks.ts')>()),
    usePositionAccount: () => ({
        data: store.state?.position ?? null,
        isLoading: false,
        error: null,
        refetch: vi.fn(),
    }),
    useShortSnapshot: () => ({ data: null, error: new Error('Live price read unavailable'), math: undefined }),
    useChainNow: () => store.state?.now ?? 0n,
    useTokenLabel: () => ({ symbol: 'TOK' }),
}));

const { RepayPanel } = await import('./RepayPanel.tsx');

beforeEach(async () => {
    const c = vectorCase('spl-close-after-fall');
    const { s, get } = await vectorSnapshot(c);
    const position = decodePosition(get(c.position as Address));
    if (!position) throw new Error('position');
    const mint = get(s.mint);
    if (!mint) throw new Error('mint');
    const mintInfo = parseMint(s.mint, mint.owner, mint.data);
    if (!mintInfo) throw new Error('mint');
    store.address = position.borrower;
    store.submit.mockReset();
    store.state = {
        positionAddress: c.position as Address,
        position,
        marketAddress: s.marketAddress,
        market: s.market,
        mintInfo,
        sources: [
            {
                address: '11111111111111111111111111111111' as Address,
                owner: position.borrower,
                mint: s.mint,
                programAddress: s.tokenProgram,
                amount: position.q,
                cpiGuard: false,
                frozen: false,
            },
        ],
        walletLamports: 1_000_000_000n,
        now: s.now,
        slot: s.slot,
        readAt: Date.now(),
    };
    store.preview = previewRepay(testMath(), store.state, s.now);
});
afterEach(() => {
    vi.unstubAllGlobals();
});
const panel = () => {
    if (!store.state) throw new Error('state');
    return render(
        <ToastProvider>
            <RepayPanel position={store.state.positionAddress} symbol="TOK" />
        </ToastProvider>,
    );
};
const enterRent = (value?: string) => {
    if (!store.preview) throw new Error('preview');
    fireEvent.change(screen.getByTestId('repay-max-rent'), {
        target: { value: value ?? plainAmount(store.preview.rentDue + 10n, 9) },
    });
};
const disabled = () => (screen.getByTestId('repay-submit') as HTMLButtonElement).disabled;

describe('borrower token repayment panel', () => {
    it('starts with no rent authorization and shows full debt plus honest external-cost disclosure', () => {
        panel();
        expect(disabled()).toBe(true);
        expect((screen.getByTestId('repay-max-rent') as HTMLInputElement).value).toBe('');
        expect(screen.getByTestId('repay-q').dataset.value).toBe(store.state?.position.q.toString());
        expect(screen.getByTestId('repay-pnl-disclosure').textContent).toContain('not a profit figure');
        expect(screen.getByText(/while swaps or live price quotes are unavailable/)).toBeTruthy();
        fireEvent.click(screen.getByTestId('repay-use-estimate'));
        expect(disabled()).toBe(false);
        expect(screen.getByTestId('repay-exact-limit').textContent).toContain(`${store.preview?.rentDue} lamports`);
    });
    it('requires the active borrower wallet and a single account with all principal', () => {
        store.address = 'BPFLoaderUpgradeab1e11111111111111111111111';
        panel();
        enterRent();
        expect(disabled()).toBe(true);
        expect(screen.getByTestId('repay-reason').textContent).toContain('another wallet');
    });
    it('refuses an underfunded or CPI-guarded source', () => {
        if (!store.state) throw new Error('state');
        const source = store.state.sources[0];
        if (!source) throw new Error('source');
        store.state = { ...store.state, sources: [{ ...source, amount: store.state.position.q - 1n }] };
        panel();
        enterRent();
        expect(disabled()).toBe(true);
        expect(screen.getByTestId('repay-reason').textContent).toContain('full token debt');
    });
    it('rejects too many SOL decimals and a limit above the position vault', () => {
        panel();
        enterRent('0.0000000001');
        expect(disabled()).toBe(true);
        if (!store.state) throw new Error('state');
        enterRent(plainAmount(store.state.position.v + 1n, 9));
        expect(disabled()).toBe(true);
        expect(screen.getByTestId('repay-reason').textContent).toContain('cannot exceed');
    });
    it('passes exactly the selected source and explicit lamport bound to review, without a send action', () => {
        panel();
        enterRent();
        if (!store.state || !store.preview) throw new Error('state');
        fireEvent.click(screen.getByTestId('repay-submit'));
        expect(store.submit).toHaveBeenCalledExactlyOnceWith(
            store.state.sources[0]?.address,
            store.preview.rentDue + 10n,
            'TOK',
        );
    });
    it('keeps the repayment panel available on the position page when live pricing fails', async () => {
        if (!store.state) throw new Error('state');
        mockIndexer({});
        const { queryClient, view } = await renderAt(`/positions/${store.state.positionAddress}`);
        try {
            expect(await screen.findByRole('heading', { name: 'Repay with tokens' })).toBeTruthy();
            enterRent();
            expect(disabled()).toBe(false);
        } finally {
            view?.unmount();
            queryClient.clear();
        }
    });
});
