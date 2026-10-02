/**
 * The Short panel on program-derived snapshots (`vectors/program-vectors.json`): the preview cells
 * carry the program's own numbers in `data-value`, the borrower disclosure gates the submit, a
 * refusal is shown before anything is built (a blocked market, a cap), a coverage pause replaces the
 * form, and the submit hands the pipeline an `open_short` plan with the preview's bounds.
 */
import { getOpenShortInstructionDataDecoder } from '@shortwin/client';
import { type Address, createNoopSigner } from '@solana/kit';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { T } from '../../config/testids.ts';
import type { MathModule } from '../../math/load.ts';
import { MARKET_FLAG } from '../../venues/constants.ts';
import { NO_EXTRAS } from '../../venues/lists.ts';
import type { ChainSnapshot } from './snapshot.ts';
import { testMath, vectorCase, vectorSnapshot } from './testing.ts';

const state = vi.hoisted(() => ({
    snapshot: null as unknown,
    math: null as unknown,
    walletQ: 0n,
    start: vi.fn(),
}));

// The panel's Retry rebuilds trades through the app's RPC client (M9-UX-1); not exercised here.
vi.mock('../../rpc/context.tsx', () => ({ useAppClient: () => ({ rpc: {}, rpcSubscriptions: {} }) }));
vi.mock('./hooks.ts', () => ({
    useShortSnapshot: () => ({
        data: state.snapshot,
        math: state.math,
        error: null,
        mathError: null,
        refetch: vi.fn(),
    }),
    useExposureQ: () => ({ data: state.walletQ }),
    useRentExempt: () => ({ data: 1_000_000n }),
    useTokenLabel: () => ({ symbol: 'MOCK', name: null }),
}));

vi.mock('../../wallet/useWallet.ts', () => {
    const address = 'BPFLoaderUpgradeab1e11111111111111111111111' as Address;
    return {
        useWallet: () => ({
            ready: true,
            status: 'connected',
            connected: { address, signer: createNoopSigner(address), walletName: 'Test', supportsV0: true },
        }),
    };
});

vi.mock('./useTradeFlow.ts', () => ({
    useTradeFlow: () => ({
        flow: {
            state: {
                stage: 'idle',
                plan: null,
                report: null,
                acks: new Set(),
                speed: 'normal',
                signature: null,
                error: null,
                refreshed: false,
            },
            start: vi.fn(),
            refresh: vi.fn(),
            setAck: vi.fn(),
            setSpeed: vi.fn(),
            sign: vi.fn(),
            close: vi.fn(),
        },
        start: state.start,
        building: false,
        buildError: null,
        fallback: null,
    }),
}));

const { ShortPanel } = await import('./ShortPanel.tsx');

let math: MathModule;
beforeAll(() => {
    math = testMath();
    state.math = math;
});

async function show(name: string, patch?: (s: ChainSnapshot) => ChainSnapshot) {
    const { s } = await vectorSnapshot(vectorCase(name));
    // A snapshot just read (an old one would block Submit, FRONTEND.md §6.6).
    const fresh = { ...s, fetchedAt: Date.now() };
    state.snapshot = patch ? patch(fresh) : fresh;
    state.walletQ = 0n;
    render(<ShortPanel mint={s.mint} />);
    return s;
}

const value = (id: string) => screen.getByTestId(id).dataset.value;

describe('ShortPanel', () => {
    it('shows the program’s numbers for the size it would open, and gates the submit on the disclosure', async () => {
        const c = vectorCase('spl-open-1');
        await show('spl-open-1');
        const e = c.result.event ?? {};
        // 2,000,000 tokens of 6 decimals = the vector's q.
        fireEvent.change(screen.getByTestId(T.shortQInput), { target: { value: '2000000' } });
        expect(value(T.shortPreview('q'))).toBe(c.q);
        expect(value(T.shortPreview('proceeds'))).toBe(String(e.s));
        expect(value(T.shortPreview('margin'))).toBe(String(e.m));
        expect(value(T.shortPreview('open-fee'))).toBe(String(e.feeOpen));
        expect(value(T.shortPreview('deadline'))).toBe(String(e.deadline));
        expect(value(T.shortPreview('min-rent'))).toBe(String(c.result.position?.rMin));
        expect(Number(value(T.shortPreview('cr-open')))).toBeGreaterThanOrEqual(20_000);
        expect(screen.getByTestId(T.shortPreview('caps')).dataset.cap).toBeTruthy();
        expect(screen.getByTestId(T.shortPreview('tier-terms')).dataset.value).toBe('C');
        expect(screen.getByTestId(T.disclosure('borrower.liquidation')).textContent).toContain('both the live price');
        const submit = screen.getByTestId(T.shortSubmit) as HTMLButtonElement;
        expect(submit.disabled).toBe(true);
        fireEvent.click(screen.getByTestId(T.disclosureAck('borrower.liquidation')));
        expect(submit.disabled).toBe(false);
        await act(async () => {
            fireEvent.click(submit);
        });
        await vi.waitFor(() => expect(state.start).toHaveBeenCalledTimes(1));
        const [build, candidates] = state.start.mock.calls[0] as [
            (x: typeof NO_EXTRAS) => Promise<{ kind: string; instructions: { data: Uint8Array }[] }>,
            unknown[],
        ];
        expect(candidates).toHaveLength(1);
        const plan = await build(NO_EXTRAS);
        expect(plan.kind).toBe('open_short');
        const data = getOpenShortInstructionDataDecoder().decode(plan.instructions[0]?.data ?? new Uint8Array());
        expect(data.q.toString()).toBe(c.q);
        expect(data.minProceeds).toBe((BigInt(String(e.s)) * 9_900n) / 10_000n);
    });

    it('warns at the binding cap with the program error it would return', async () => {
        await show('spl-open-position-cap');
        fireEvent.change(screen.getByTestId(T.shortQInput), { target: { value: '10000000' } });
        const w = screen.getByTestId(T.shortCapWarning);
        expect(w.dataset.cap).toBe('position');
        fireEvent.click(screen.getByTestId(T.disclosureAck('borrower.liquidation')));
        expect((screen.getByTestId(T.shortSubmit) as HTMLButtonElement).disabled).toBe(true);
    });

    it('names a blocked market with its program error before any size', async () => {
        await show('warmup-open');
        expect(screen.getByTestId(T.shortBlocked).dataset.code).toBe('MarketWarmingUp');
        expect(screen.getByTestId(T.shortQInput)).toBeTruthy();
    });

    it('replaces the form while coverage is paused', async () => {
        await show('spl-open-0', (s) => ({
            ...s,
            market: { ...s.market, flags: s.market.flags | MARKET_FLAG.pausedCoverage },
        }));
        expect(screen.getByTestId(T.shortBlocked).dataset.code).toBe('CoveragePaused');
        expect(screen.getByTestId(T.disclosure('market.coveragePause'))).toBeTruthy();
        expect(screen.queryByTestId(T.shortQInput)).toBeNull();
    });

    it('fills Max with the binding cap, in a form the input parses back', async () => {
        await show('spl-open-0');
        const caps = screen.getByTestId(T.shortPreview('caps'));
        fireEvent.click(screen.getByRole('button', { name: 'Max' }));
        expect(value(T.shortPreview('q'))).toBe(caps.dataset.value);
        expect(screen.queryByTestId(T.shortCapWarning)).toBeNull();
    });

    it('validates the slippage', async () => {
        await show('spl-open-0');
        const input = screen.getByTestId(T.shortSlippageInput);
        expect(input.dataset.value).toBe('100');
        fireEvent.change(input, { target: { value: '0.05' } });
        expect(input.getAttribute('aria-invalid')).toBe('true');
        fireEvent.change(input, { target: { value: '2.5' } });
        expect(input.dataset.value).toBe('250');
    });
});
