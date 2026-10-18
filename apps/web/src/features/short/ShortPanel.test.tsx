/**
 * The Short panel on program-derived snapshots (`vectors/program-vectors.json`): the preview cells
 * carry the program's own numbers in `data-value`, the borrower disclosure gates the submit, a
 * refusal is shown before anything is built (a blocked market, a cap), a coverage pause replaces the
 * form, and the submit hands the pipeline an `open_short` plan with the preview's bounds.
 */
import { getOpenShortInstructionDataDecoder } from '@shortwin/client';
import { type Address, createNoopSigner } from '@solana/kit';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { PageBannersContext } from '../../components/PageBanners.tsx';
import { ToastProvider } from '../../components/Toast.tsx';
import { T } from '../../config/testids.ts';
import type { MathModule } from '../../math/load.ts';
import { TxDoneContext } from '../../tx/TxReviewModal.tsx';
import type { TxPlan, TxStage } from '../../tx/types.ts';
import { COVERAGE_GAP_S, MARKET_FLAG } from '../../venues/constants.ts';
import { NO_EXTRAS } from '../../venues/lists.ts';
import type { ChainSnapshot } from './snapshot.ts';
import { testMath, vectorCase, vectorSnapshot } from './testing.ts';

const state = vi.hoisted(() => ({
    snapshot: null as unknown,
    math: null as unknown,
    walletQ: 0n,
    start: vi.fn(),
    close: vi.fn(),
    stage: 'idle' as TxStage,
    plan: null as TxPlan | null,
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
                stage: state.stage,
                plan: state.plan,
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
            close: state.close,
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
beforeEach(() => {
    state.start.mockClear();
    state.close.mockClear();
    state.stage = 'idle';
    state.plan = null;
});

async function show(name: string, patch?: (s: ChainSnapshot) => ChainSnapshot) {
    const { s } = await vectorSnapshot(vectorCase(name));
    // A snapshot just read (an old one would block Submit, FRONTEND.md §6.6).
    const fresh = { ...s, fetchedAt: Date.now() };
    state.snapshot = patch ? patch(fresh) : fresh;
    state.walletQ = 0n;
    render(
        <ToastProvider>
            <ShortPanel mint={s.mint} />
        </ToastProvider>,
    );
    return s;
}

const value = (id: string) => screen.getByTestId(id).dataset.value;

describe('ShortPanel', () => {
    it.each(['crank_price', 'open_short'] as const)(
        'Done after %s preserves the correct trade-sheet behavior',
        async (kind) => {
            const { s } = await vectorSnapshot(vectorCase('spl-open-0'));
            state.snapshot = s;
            state.stage = 'confirmed';
            state.plan = { kind, title: 'Confirmed', instructions: [], watch: [], disclosures: [], invalidate: [] };
            const done = vi.fn();
            render(
                <TxDoneContext.Provider value={done}>
                    <ToastProvider>
                        <ShortPanel mint={s.mint} />
                    </ToastProvider>
                </TxDoneContext.Provider>,
            );
            fireEvent.click(screen.getByRole('button', { name: 'Done' }));
            expect(state.close).toHaveBeenCalledTimes(1);
            expect(done).toHaveBeenCalledTimes(kind === 'crank_price' ? 0 : 1);
        },
    );

    it('offers an explicit price review on a stale market, with separate signatures and fees', async () => {
        await show('spl-open-0', (s) => ({ ...s, now: s.now + COVERAGE_GAP_S + 1n }));
        expect(screen.getByTestId(T.shortPriceRefreshStep).dataset.step).toBe('record');
        expect(screen.getByTestId(T.disclosure('market.priceRefresh')).textContent).toContain('sign each separately');
        expect(screen.getByTestId(T.disclosure('market.priceRefresh')).textContent).toContain('network fee');
        expect(state.start).not.toHaveBeenCalled();
        expect((screen.getByTestId(T.shortSubmit) as HTMLButtonElement).disabled).toBe(true);
        fireEvent.click(screen.getByTestId(T.shortPriceRefresh));
        expect(state.start).toHaveBeenCalledTimes(1);
        expect(state.start.mock.calls[0]?.[1]).toEqual([NO_EXTRAS]);
    });

    it('holds the next review in the recording second and resumes from chain state after reload', async () => {
        const s = await show('spl-open-0', (s) => {
            const now = s.now + COVERAGE_GAP_S + 1n;
            return { ...s, now, market: { ...s.market, emaTs: now, bSec: now, bLo: s.market.emaPrice } };
        });
        expect(screen.getByTestId(T.shortPriceRefreshStep).dataset.step).toBe('wait');
        expect((screen.getByTestId(T.shortPriceRefresh) as HTMLButtonElement).disabled).toBe(true);
        expect(state.start).not.toHaveBeenCalled();
        const current = state.snapshot as ChainSnapshot;
        state.snapshot = { ...current, now: current.now + 1n };
        // A newly mounted panel has no local first-transaction state to rely on.
        const { unmount } = render(
            <ToastProvider>
                <ShortPanel mint={s.mint} />
            </ToastProvider>,
        );
        const steps = screen.getAllByTestId(T.shortPriceRefreshStep);
        expect(steps[1]?.dataset.step).toBe('integrate');
        expect((screen.getAllByTestId(T.shortPriceRefresh)[1] as HTMLButtonElement).disabled).toBe(false);
        expect(state.start).not.toHaveBeenCalled();
        unmount();
    });

    it('does not offer a price refresh during ordinary listing warmup or on fresh prices', async () => {
        await show('warmup-open');
        expect(screen.queryByTestId(T.shortPriceRefresh)).toBeNull();
        const { s } = await vectorSnapshot(vectorCase('spl-open-0'));
        state.snapshot = s;
        render(
            <ToastProvider>
                <ShortPanel mint={s.mint} />
            </ToastProvider>,
        );
        expect(screen.queryByTestId(T.shortPriceRefresh)).toBeNull();
    });

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
        // The reason line names the acknowledgement, as a button that focuses its box.
        const reason = screen.getByTestId(T.shortSubmitReason);
        expect(reason.textContent).toBe('Tick “I understand…” above');
        const ack = screen.getByTestId(T.disclosureAck('borrower.liquidation'));
        fireEvent.click(screen.getByRole('button', { name: 'Tick “I understand…” above' }));
        expect(document.activeElement).toBe(ack);
        fireEvent.click(ack);
        expect(submit.disabled).toBe(false);
        expect(reason.textContent).toBe('');
        expect(reason.dataset.reason).toBeUndefined();
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
        expect(screen.getByTestId(T.shortSubmitReason).textContent).toBe(
            'Lower the size: it is above the position impact cap',
        );
    });

    it('says why the submit waits: no size, a size that does not parse, a bad slippage', async () => {
        await show('spl-open-0');
        const reason = () => screen.getByTestId(T.shortSubmitReason).textContent;
        expect(reason()).toBe('Enter a size');
        fireEvent.change(screen.getByTestId(T.shortQInput), { target: { value: '1e6' } });
        expect(reason()).toBe('Enter the size as a plain number');
        fireEvent.change(screen.getByTestId(T.shortQInput), { target: { value: '1000' } });
        fireEvent.change(screen.getByTestId(T.shortSlippageInput), { target: { value: '0' } });
        expect(reason()).toBe('Set a slippage between 0.1% and 50%');
    });

    it('names a blocked market with its program error before any size', async () => {
        await show('warmup-open');
        expect(screen.getByTestId(T.shortBlocked).dataset.code).toBe('MarketWarmingUp');
        expect(screen.getByTestId(T.shortQInput)).toBeTruthy();
        expect(screen.getByTestId(T.shortSubmitReason).textContent).toBe(
            'New shorts are unavailable on this market now',
        );
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

    it('links to the page’s coverage banner instead of repeating it (O-2), in words that fit a sheet', async () => {
        const focus = vi.fn();
        const paused = (x: ChainSnapshot) => ({
            ...x,
            market: { ...x.market, flags: x.market.flags | MARKET_FLAG.pausedCoverage },
        });
        // Beside the page (≥ 1,024 px): the banner is above.
        const { s } = await vectorSnapshot(vectorCase('spl-open-0'));
        state.snapshot = paused({ ...s, fetchedAt: Date.now() });
        const view = render(
            <PageBannersContext.Provider value={{ shown: new Set(['coverage-paused']), inSheet: false, focus }}>
                <ToastProvider>
                    <ShortPanel mint={s.mint} />
                </ToastProvider>
            </PageBannersContext.Provider>,
        );
        // The note stays; the disclosure the banner already shows does not.
        expect(screen.getByTestId(T.shortBlocked).dataset.code).toBe('CoveragePaused');
        expect(screen.queryByTestId(T.disclosure('market.coveragePause'))).toBeNull();
        const link = screen.getByTestId(T.shortCoverageLink);
        expect(link.textContent).toBe('Read the full coverage notice');
        fireEvent.click(link);
        expect(focus).toHaveBeenCalledWith('coverage-paused');
        // In the trade sheet the banner is behind it: the link says it closes the sheet.
        view.rerender(
            <PageBannersContext.Provider value={{ shown: new Set(['coverage-paused']), inSheet: true, focus }}>
                <ToastProvider>
                    <ShortPanel mint={s.mint} />
                </ToastProvider>
            </PageBannersContext.Provider>,
        );
        expect(screen.getByTestId(T.shortCoverageLink).textContent).toBe('Close and read the full coverage notice');
        // No banner to link to (the clear delay, the indexer's flags missing): the disclosure stays.
        view.rerender(
            <PageBannersContext.Provider value={{ shown: new Set(), inSheet: false, focus }}>
                <ToastProvider>
                    <ShortPanel mint={s.mint} />
                </ToastProvider>
            </PageBannersContext.Provider>,
        );
        expect(screen.getByTestId(T.disclosure('market.coveragePause'))).toBeTruthy();
        expect(screen.queryByTestId(T.shortCoverageLink)).toBeNull();
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
