import { getRepayShortInstructionDataDecoder } from '@shortwin/client';
import { type Address, createNoopSigner } from '@solana/kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { parseMint } from '../../../rpc/mint.ts';
import type { TxPlan } from '../../../tx/types.ts';
import { decodePosition } from '../snapshot.ts';
import { testMath, vectorCase, vectorSnapshot } from '../testing.ts';
import { previewRepay, type RepayState } from './repay.ts';

const store = vi.hoisted(() => ({ state: null as RepayState | null, start: vi.fn(), read: vi.fn() }));
vi.mock('./repay.ts', async (original) => ({
    ...(await original<typeof import('./repay.ts')>()),
    readRepayState: () => {
        store.read();
        return Promise.resolve(store.state);
    },
}));
vi.mock('../../../math/useMath.ts', () => ({ useMath: () => ({ math: testMath(), error: null }) }));
vi.mock('../../../rpc/context.tsx', () => ({ useAppClient: () => ({ rpc: {} }) }));
vi.mock('../../../wallet/useWallet.ts', () => ({
    useWallet: () => ({
        connected: store.state
            ? {
                  address: store.state.position.borrower,
                  signer: createNoopSigner(store.state.position.borrower),
                  walletName: 'Test',
                  supportsV0: true,
              }
            : null,
    }),
}));
vi.mock('../../../tx/useTxFlow.ts', () => ({ useTxFlow: () => ({ state: { stage: 'idle' }, start: store.start }) }));
const { useRepay } = await import('./useRepay.ts');

beforeEach(async () => {
    const c = vectorCase('spl-close-after-fall');
    const { s, get } = await vectorSnapshot(c);
    const position = decodePosition(get(c.position as Address));
    if (!position) throw new Error('position');
    const mint = get(s.mint);
    if (!mint) throw new Error('mint');
    const mintInfo = parseMint(s.mint, mint.owner, mint.data);
    if (!mintInfo) throw new Error('mint');
    store.state = {
        positionAddress: c.position as Address,
        position,
        marketAddress: s.marketAddress,
        market: s.market,
        mintInfo,
        sources: [
            {
                address: '11111111111111111111111111111111' as Address,
                mint: s.mint,
                owner: position.borrower,
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
    store.start.mockReset();
    store.read.mockReset();
});
function mounted() {
    if (!store.state) throw new Error('state');
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const wrapper = ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={qc}>{children}</QueryClientProvider>
    );
    const positionAddress = store.state.positionAddress;
    const hook = renderHook(() => useRepay(positionAddress), { wrapper });
    return {
        ...hook,
        stop: () => {
            hook.unmount();
            qc.clear();
        },
    };
}

describe('repayment review rebuild', () => {
    it('re-reads chain state on submit/retry and never silently raises the exact user rent bound', async () => {
        const hook = mounted();
        try {
            await waitFor(() => expect(hook.result.current.state.data).toBeTruthy());
            if (!store.state) throw new Error('state');
            const source = store.state.sources[0];
            if (!source) throw new Error('source');
            const maxRent = previewRepay(testMath(), store.state, store.state.now).rentDue + 100n;
            await act(() => hook.result.current.submit(source.address, maxRent, 'TOK'));
            expect(store.start).toHaveBeenCalledTimes(1);
            const plan = store.start.mock.calls[0]?.[0] as TxPlan;
            const data = plan.instructions[0]?.data;
            if (!data) throw new Error('instruction data');
            expect(getRepayShortInstructionDataDecoder().decode(data).maxRent).toBe(maxRent);
            const reads = store.read.mock.calls.length;
            store.state = { ...store.state, position: { ...store.state.position, rMin: maxRent + 1n } };
            await expect(plan.rebuild?.()).rejects.toThrow(/rent limit is below/);
            expect(store.read.mock.calls.length).toBe(reads + 1);
            expect(store.start).toHaveBeenCalledTimes(1);
            expect(getRepayShortInstructionDataDecoder().decode(data).maxRent).toBe(maxRent);
        } finally {
            hook.stop();
        }
    });
    it('refuses funds lost since the preview before entering simulation/review', async () => {
        const hook = mounted();
        try {
            await waitFor(() => expect(hook.result.current.state.data).toBeTruthy());
            if (!store.state) throw new Error('state');
            const source = store.state.sources[0];
            if (!source) throw new Error('source');
            const maxRent = previewRepay(testMath(), store.state, store.state.now).rentDue + 100n;
            store.state = { ...store.state, sources: [{ ...source, amount: store.state.position.q - 1n }] };
            await act(() => hook.result.current.submit(source.address, maxRent, 'TOK'));
            expect(store.start).not.toHaveBeenCalled();
            expect(hook.result.current.buildError).toMatch(/full token debt/);
        } finally {
            hook.stop();
        }
    });
});
