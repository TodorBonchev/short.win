/**
 * Regressions of the M9 review — what the screens show: `tx.simulation` in every review
 * (M9-UX-3), who signs and what Retry will do (M9-SEC-8, M9-SEC-1, M9-UX-1), visible price units
 * (M9-UX-8), the age of a preview's chain read (M9-UX-6), the clickjacking guard and the deploy
 * headers (M9-SEC-6).
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { Price } from '../components/Num.tsx';
import { ANTI_FRAMING_HEADERS, missingSecurityHeaders, securityHeaders } from '../config/csp.ts';
import { PROGRAM_ERRORS } from '../config/errors.ts';
import { T } from '../config/testids.ts';
import { FrameGuard, isFramed } from '../shell/FrameGuard.tsx';
import { ReadAgeView, readAge, STALE_READ_MS } from '../shell/SlotAge.tsx';
import { TxReviewModal } from '../tx/TxReviewModal.tsx';
import type { TxError, TxKind, TxPlan } from '../tx/types.ts';
import type { SigningWallet, TxFlow, TxFlowState } from '../tx/useTxFlow.ts';

const WALLET = '9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin';

function flowFor(plan: TxPlan, o: Partial<TxFlowState> = {}, wallet: SigningWallet | null = null): TxFlow {
    return {
        state: {
            stage: 'review',
            plan,
            report: {
                ok: true,
                unitsConsumed: 100_000n,
                computeUnitLimit: 125_000,
                priorityFeeMicroLamports: 0n,
                feeLamports: 5_000n,
                sizeBytes: 800,
                deltas: [],
                logs: [],
                error: null,
                simulatedAt: Date.now(),
            },
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
        wallet,
    };
}

const plan = (kind: TxKind, o: Partial<TxPlan> = {}): TxPlan => ({
    kind,
    title: kind,
    instructions: [],
    watch: [],
    disclosures: [],
    invalidate: [],
    ...o,
});

describe('M9-UX-3: `tx.simulation` is in every review', () => {
    it.each(['close_short', 'add_margin', 'claim_sol', 'cover_deficit', 'sync_lender', 'withdraw'] as const)(
        '%s',
        (kind) => {
            render(<TxReviewModal flow={flowFor(plan(kind))} />);
            expect(screen.getByTestId(T.disclosure('tx.simulation'))).toBeTruthy();
        },
    );
});

describe('M9-SEC-8 / M9-SEC-1: the review names who signs, and a wallet that sends itself', () => {
    it('shows "Signing with <wallet> · <account>"', () => {
        const wallet = { name: 'Phantom', icon: null, address: WALLET, sendsItself: false };
        render(<TxReviewModal flow={flowFor(plan('deposit'), {}, wallet)} />);
        const el = screen.getByTestId(T.txWallet);
        expect(el.textContent).toContain('Signing with');
        expect(el.textContent).toContain('Phantom');
        expect(el.textContent).toContain('9xQe…VFin');
        expect(el.dataset.value).toBe(WALLET);
        expect(el.dataset.sendsItself).toBe('false');
    });

    it('labels a sending-only wallet: it cannot be checked before it is sent', () => {
        const wallet = { name: 'Sender', icon: null, address: WALLET, sendsItself: true };
        render(<TxReviewModal flow={flowFor(plan('deposit'), {}, wallet)} />);
        expect(screen.getByTestId(T.txWallet).dataset.sendsItself).toBe('true');
        expect(screen.getByRole('note').textContent).toMatch(/cannot be checked before it is sent/);
    });

    it('shows the fee as the wallet signed it when it changed it (FE-2)', () => {
        render(<TxReviewModal flow={flowFor(plan('deposit'), { stage: 'confirming', finalFeeLamports: 6_000n })} />);
        expect(screen.getByTestId(T.txFinalFee).dataset.value).toBe('6000');
    });
});

describe('M9-UX-1: the Retry button says what it will do', () => {
    const failed = (error: TxError, o: Partial<TxPlan> = {}, s: Partial<TxFlowState> = {}) =>
        flowFor(plan('open_short', o), { stage: 'failed', error, ...s });
    const refusal: TxError = {
        kind: 'program',
        code: 6063,
        name: 'ProceedsBelowMin',
        action: 'retry',
        message: PROGRAM_ERRORS.get(6063)?.message ?? '',
    };

    it('rebuilds with a new quote when the plan can, else closes for a new review', () => {
        const { unmount } = render(
            <TxReviewModal flow={failed(refusal, { rebuild: async () => plan('open_short') })} />,
        );
        expect(screen.getByTestId(T.txRetry).textContent).toBe('Retry with a new quote');
        expect(screen.getByTestId(T.txRetry).dataset.mode).toBe('rebuild');
        unmount();
        render(<TxReviewModal flow={failed(refusal)} />);
        expect(screen.getByTestId(T.txRetry).textContent).toBe('Close and review again');
    });

    it('offers no Retry when the outcome is unknown or the transaction landed', () => {
        const unknown: TxError = { kind: 'unknown-outcome', action: 'report', message: 'May still land.' };
        const { unmount } = render(<TxReviewModal flow={failed(unknown, {}, { signature: 'sig' as never })} />);
        expect(screen.queryByTestId(T.txRetry)).toBeNull();
        expect(screen.getByTestId(T.txCancel).textContent).toBe('Close');
        unmount();
        const landed: TxError = { kind: 'integrity', action: 'report', message: 'differs' };
        render(<TxReviewModal flow={failed(landed, {}, { signature: 'sig' as never })} />);
        expect(screen.queryByTestId(T.txRetry)).toBeNull();
    });

    it('announces a rebuilt plan', () => {
        render(<TxReviewModal flow={flowFor(plan('open_short'), { rebuilt: true })} />);
        expect(screen.getByTestId(T.txRebuilt).textContent).toMatch(/new quote/);
    });
});

describe('M9-UX-8: prices carry a visible unit', () => {
    it('a liquidation price reads "… SOL", not a bare number', () => {
        const { container } = render(<Price wad={28_000_000_000_000n} decimals={6} testId="p" />);
        const visible = [...container.querySelectorAll('[aria-hidden="true"]')].map((e) => e.textContent).join(' ');
        expect(visible).toBe('0.0₇28 SOL');
        expect(screen.getByTestId('p').title).toBe('0.000000028 SOL per token');
    });

    it('the close form labels the SOL buyback cost as SOL', () => {
        const src = readFileSync(join(process.cwd(), 'src/features/short/positions/CloseForm.tsx'), 'utf8');
        expect(src).toContain('<dt>Buyback cost (SOL)</dt>');
        expect(src).not.toContain('Buyback ({symbol})');
    });
});

describe('M9-UX-6: a preview says how old its chain read is, and blocks when stale', () => {
    const now = 1_700_000_000_000;
    it('fresh within 2 slots of the confirmed tip; aging beyond; stale past 75 slots or 30 s', () => {
        expect(readAge({ slot: 100n, readAt: now }, 104n, now).state).toBe('fresh');
        const aging = readAge({ slot: 100n, readAt: now - 2_000 }, 110n, now);
        expect(aging.state).toBe('aging');
        expect(aging.slots).toBe(8n);
        expect(readAge({ slot: 100n, readAt: now }, 300n, now).state).toBe('stale');
        expect(readAge({ slot: null, readAt: now - STALE_READ_MS - 1 }, null, now).state).toBe('stale');
        // A live account subscription vouches for an unchanged read.
        expect(readAge({ slot: 100n, readAt: now - 60_000 }, 300n, now, true).state).toBe('fresh');
    });

    it('renders the age, and the stale state as a status', () => {
        render(<ReadAgeView age={{ state: 'stale', slots: 80n, seconds: 32 }} />);
        const el = screen.getByTestId(T.slotAge);
        expect(el.dataset.state).toBe('stale');
        expect(el.textContent).toContain('80 slots, 32s old');
        expect(el.getAttribute('role')).toBe('status');
    });

    it('every indexer-backed money view shows the freshness badge', () => {
        for (const f of [
            'src/features/short/PositionsPage.tsx',
            'src/features/lend/LenderDashboardPage.tsx',
            'src/features/short/PositionDetailPage.tsx',
        ]) {
            expect(readFileSync(join(process.cwd(), f), 'utf8'), f).toContain('<FreshnessBadge');
        }
    });
});

describe('M9-SEC-6: the app never runs framed; hosts must send the anti-framing headers', () => {
    it('detects a frame (and a cross-origin top that throws)', () => {
        const self = {} as Window;
        expect(isFramed({ top: self, self } as never)).toBe(false);
        expect(isFramed({ top: {} as Window, self } as never)).toBe(true);
        const throwing = Object.defineProperty({ self }, 'top', {
            get() {
                throw new Error('cross-origin');
            },
        });
        expect(isFramed(throwing as never)).toBe(true);
        expect(isFramed()).toBe(false); // jsdom: not framed
    });

    it('main.tsx renders the guard instead of the app when framed', () => {
        const main = readFileSync(join(process.cwd(), 'src/main.tsx'), 'utf8');
        expect(main).toMatch(/if \(isFramed\(\)\) root\.render\(<FrameGuard/);
        render(<FrameGuard href="https://short.win/markets" />);
        expect(screen.getByTestId(T.frameGuard).textContent).toMatch(/does not run inside another page/);
        expect(screen.queryByTestId(T.walletButton)).toBeNull();
    });

    it('the deploy check accepts the host headers and rejects a host without them', () => {
        const cfg = {
            cluster: 'mainnet' as const,
            rpcUrl: 'https://rpc.example',
            rpcWsUrl: 'wss://rpc.example',
            indexerUrl: 'https://api.example',
            indexerWsUrl: 'wss://api.example/v1/ws',
        };
        expect(missingSecurityHeaders(new Headers(securityHeaders(cfg)))).toEqual([]);
        expect(missingSecurityHeaders(new Headers({}))).toHaveLength(4);
        expect(ANTI_FRAMING_HEADERS['X-Frame-Options']).toBe('DENY');
        expect(ANTI_FRAMING_HEADERS['Content-Security-Policy']).toBe("frame-ancestors 'none'");
    });

    it('vite preview sends them, and source maps are hidden and kept out of dist/', () => {
        const vite = readFileSync(join(process.cwd(), 'vite.config.ts'), 'utf8');
        expect(vite).toContain("sourcemap: 'hidden'");
        expect(vite).toContain('headers: { ...ANTI_FRAMING_HEADERS }');
        expect(vite).toContain("const SOURCEMAPS_DIR = 'dist-sourcemaps'");
    });
});
