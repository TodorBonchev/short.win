/**
 * The review's M10-RESP changes (FRONTEND.md §5.3, §10.1): the error right under the stage, a
 * timestamp delta as a UTC date, the sign hint only while an acknowledgement blocks Sign, the
 * wallet rejection's copy and single way on, the confirmed link.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { T } from '../config/testids.ts';
import { TxDoneContext, TxReviewModal } from './TxReviewModal.tsx';
import type { Delta, SimulationReport, TxPlan } from './types.ts';
import type { TxFlow, TxFlowState } from './useTxFlow.ts';

const report = (deltas: Delta[] = []): SimulationReport => ({
    ok: true,
    unitsConsumed: 100_000n,
    computeUnitLimit: 125_000,
    priorityFeeMicroLamports: 0n,
    feeLamports: 5_000n,
    sizeBytes: 800,
    deltas,
    logs: [],
    error: null,
    simulatedAt: Date.now(),
});

const plan = (o: Partial<TxPlan> = {}): TxPlan => ({
    kind: 'deposit',
    title: 'Deposit',
    instructions: [],
    watch: [],
    disclosures: ['lender.solRepayment'],
    invalidate: [],
    ...o,
});

function flowFor(o: Partial<TxFlowState> = {}, p: TxPlan = plan()): TxFlow {
    return {
        state: {
            stage: 'review',
            plan: p,
            report: report(),
            acks: new Set(),
            speed: 'normal',
            signature: null,
            error: null,
            refreshed: false,
            ...o,
        },
        start: () => {},
        refresh: () => {},
        setAck: () => {},
        setSpeed: () => {},
        sign: () => {},
        close: () => {},
    };
}

const SOL: Delta = {
    key: 'w:lamports',
    label: 'Wallet SOL',
    before: 2_000_000_000n,
    after: 1_000_000_000n,
    unit: 'SOL',
    scale: 9,
};
const DEADLINE: Delta = {
    key: 'p:deadline',
    label: 'Position · Deadline',
    before: null,
    after: 1_791_456_300n,
    unit: '',
    scale: 0,
    kind: 'timestamp',
};

describe('TxReviewModal (M10-RESP)', () => {
    it('puts the error before the deltas', () => {
        render(
            <TxReviewModal
                flow={flowFor({
                    stage: 'failed',
                    report: { ...report([SOL]), ok: false },
                    error: { kind: 'program', action: 'retry', message: 'Moved.', code: 6001 },
                })}
            />,
        );
        const err = screen.getByTestId(T.txError);
        const delta = screen.getByTestId(T.txDelta(SOL.key));
        expect(err.compareDocumentPosition(delta) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    });

    it('shows a timestamp delta as a UTC date with an empty change', () => {
        render(<TxReviewModal flow={flowFor({ report: report([DEADLINE]) })} />);
        const row = screen.getByTestId(T.txDelta(DEADLINE.key));
        const cells = row.querySelectorAll('td');
        expect(cells[2]?.textContent).toContain('2026-10-08 10:45 UTC');
        expect(cells[3]?.textContent?.trim()).toBe('');
        expect(row.dataset.kind).toBe('timestamp');
    });

    it('hints at the acknowledgement only while it blocks Sign, and the hint focuses it', () => {
        const { rerender } = render(<TxReviewModal flow={flowFor()} />);
        const hint = screen.getByTestId(T.txSignHint);
        expect(hint.textContent).toBe('Tick the acknowledgement above to sign');
        fireEvent.click(hint);
        expect(document.activeElement).toBe(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        rerender(<TxReviewModal flow={flowFor({ acks: new Set(['lender.solRepayment']) })} />);
        expect(screen.queryByTestId(T.txSignHint)).toBeNull();
    });

    it('a simulation the plan holds: Sign disabled with its reason beside it, no acknowledgement hint', () => {
        const held = plan({ kind: 'claim_sol', title: 'Claim 0 SOL', holdSign: () => 'Nothing to claim yet' });
        render(<TxReviewModal flow={flowFor({}, held)} />);
        expect(screen.getByTestId(T.txSignHold).textContent).toBe('Nothing to claim yet');
        expect((screen.getByTestId(T.txSign) as HTMLButtonElement).disabled).toBe(true);
        expect(screen.queryByTestId(T.txSignHint)).toBeNull();
        // Refresh simulates again.
        expect(screen.getByTestId(T.txRetry).dataset.mode).toBe('resimulate');
    });

    it('a wallet rejection: "Cancelled in your wallet", Try again, no Sign, no simulation badge', () => {
        render(
            <TxReviewModal
                flow={flowFor({
                    stage: 'failed',
                    error: {
                        kind: 'wallet-rejected',
                        action: 'retry',
                        message: 'You rejected the request in your wallet.',
                    },
                })}
            />,
        );
        expect(screen.getByTestId(T.txError).textContent).toContain('Cancelled in your wallet');
        expect(screen.getByTestId(T.txError).dataset.kind).toBe('wallet-rejected');
        // The stage says what happened — the user's own cancel — not "Failed".
        expect(screen.getByTestId(T.txStage).textContent).toBe('Cancelled');
        expect(screen.getByTestId(T.txStage).dataset.stage).toBe('failed');
        expect(screen.getByTestId(T.txRetry).textContent).toBe('Try again');
        expect(screen.queryByTestId(T.txSign)).toBeNull();
        expect(screen.queryByTestId(T.txSimStatus)).toBeNull();
    });

    it('a failure that is not the wallet’s cancel still reads Failed', () => {
        render(
            <TxReviewModal
                flow={flowFor({ stage: 'failed', error: { kind: 'program', action: 'retry', message: 'x' } as never })}
            />,
        );
        expect(screen.getByTestId(T.txStage).textContent).toBe('Failed');
    });

    it('keeps a shortened id on one line in the title and the account labels', () => {
        const label = 'Position · Borrowed (E2EMAIN · 8oBK…6cTC)';
        render(
            <TxReviewModal
                flow={flowFor(
                    { report: report([{ ...SOL, label }]) },
                    plan({ title: 'Open short · 250,000 E2EMAIN · 8oBK…6cTC' }),
                )}
            />,
        );
        const title = document.getElementById(`${T.txModal}-title`);
        expect(title?.textContent).toBe('Open short · 250,000 E2EMAIN · 8oBK…6cTC');
        expect(title?.querySelector('.nowrap')?.textContent).toBe('8oBK…6cTC');
        const row = screen.getByTestId(T.txDelta(SOL.key));
        expect(row.querySelector('td .nowrap')?.textContent).toBe('8oBK…6cTC)');
    });

    it('Done on a confirmed review also runs the host sheet’s callback', () => {
        const done = vi.fn();
        const close = vi.fn();
        const flow = flowFor({ stage: 'confirmed', signature: 'sig' as never });
        render(
            <TxDoneContext.Provider value={done}>
                <TxReviewModal flow={{ ...flow, close }} />
            </TxDoneContext.Provider>,
        );
        fireEvent.click(screen.getByRole('button', { name: 'Done' }));
        expect(close).toHaveBeenCalledOnce();
        expect(done).toHaveBeenCalledOnce();
    });

    it('✕ and Escape on a confirmed review end where Done does; before confirmation the sheet stays', () => {
        const confirmed = flowFor({ stage: 'confirmed', signature: 'sig' as never });
        const close = (how: 'x' | 'escape') => {
            if (how === 'x') fireEvent.click(screen.getByRole('button', { name: 'Close' }));
            else fireEvent(screen.getByTestId(T.txModal), new Event('cancel', { cancelable: true }));
        };
        for (const how of ['x', 'escape'] as const) {
            const done = vi.fn();
            const flowClose = vi.fn();
            const { unmount } = render(
                <TxDoneContext.Provider value={done}>
                    <TxReviewModal flow={{ ...confirmed, close: flowClose }} />
                </TxDoneContext.Provider>,
            );
            close(how);
            expect(flowClose, how).toHaveBeenCalledOnce();
            expect(done, how).toHaveBeenCalledOnce();
            unmount();
        }
        // Reviewed, cancelled in the wallet, failed: the review closes, the host's sheet stays.
        const before: Partial<TxFlowState>[] = [
            { stage: 'review' },
            { stage: 'failed', error: { kind: 'wallet-rejected', action: 'retry', message: 'Rejected' } },
            { stage: 'failed', error: { kind: 'program', action: 'retry', message: 'Failed' } },
        ];
        for (const o of before) {
            for (const how of ['x', 'escape'] as const) {
                const done = vi.fn();
                const flowClose = vi.fn();
                const { unmount } = render(
                    <TxDoneContext.Provider value={done}>
                        <TxReviewModal flow={{ ...flowFor(o), close: flowClose }} />
                    </TxDoneContext.Provider>,
                );
                close(how);
                expect(flowClose, `${o.stage} ${how}`).toHaveBeenCalledOnce();
                expect(done, `${o.stage} ${how}`).not.toHaveBeenCalled();
                unmount();
            }
        }
        // Cancel (the footer's own button) never runs it either.
        const done = vi.fn();
        render(
            <TxDoneContext.Provider value={done}>
                <TxReviewModal flow={flowFor()} />
            </TxDoneContext.Provider>,
        );
        fireEvent.click(screen.getByTestId(T.txCancel));
        expect(done).not.toHaveBeenCalled();
    });

    it('once confirmed, links the next step; the explorer link is the signature row’s only', () => {
        const { rerender } = render(
            <TxReviewModal
                flow={flowFor(
                    { stage: 'confirmed', signature: 'sig' as never },
                    plan({ next: { href: '/positions/abc', text: 'View position' } }),
                )}
            />,
        );
        expect(screen.getByTestId(T.txConfirmedLink).textContent).toBe('View position');
        expect(screen.getByTestId(T.txConfirmedLink).getAttribute('href')).toBe('/positions/abc');
        expect(screen.getAllByText('Open in explorer')).toHaveLength(1);
        rerender(<TxReviewModal flow={flowFor({ stage: 'confirmed', signature: 'sig' as never })} />);
        expect(screen.queryByTestId(T.txConfirmedLink)).toBeNull();
        expect(screen.getByTestId(T.txExplorer).getAttribute('href')).toContain('sig');
        expect(screen.getAllByRole('link', { name: /explorer/i })).toHaveLength(1);
    });
});
