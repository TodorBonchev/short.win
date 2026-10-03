/**
 * PUMP-2026-10-2 (disclosed; the owner's decision of 2026-10-03): since pump's 2026-10-02 redeploys
 * the first trade on a PumpSwap pool shorter than 287 bytes, or a curve shorter than 141, grows the
 * account and charges that trade its rent, which the program nets out of an open's proceeds; an open
 * whose tolerance cannot cover it fails `ProceedsBelowMin` (6063) though no price moved
 * (`pamm_it::gates::t_rent_a_short_pool_grows_on_the_sale`: on the 211-byte pool at minimum rent no
 * open within the default wallet cap qualifies). The open plan carries a `refineError` that gives
 * that refusal the specific message, and the review shows it.
 */
import { address, createNoopSigner } from '@solana/kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PROGRAM_ERRORS } from '../../config/errors.ts';
import { SHORTWIN_PROGRAM } from '../../config/programs.ts';
import { mapTransactionError } from '../../tx/errors.ts';
import type { SimulationReport, TxError, TxPlan } from '../../tx/types.ts';
import type { KindName } from '../../venues/constants.ts';
import type { RawAccount } from '../../venues/readers.ts';
import { fixtureAccount } from '../../venues/testing.ts';
import { growthRefinement, VENUE_GROWTH_MESSAGES, venueGrowth } from './plans.ts';

const WALLET = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');

const h = vi.hoisted(() => ({ outcome: [] as (TxError | null)[] }));

vi.mock('../../rpc/context.tsx', () => ({ useAppClient: () => ({ rpc: {}, rpcSubscriptions: {} }) }));
vi.mock('../../wallet/useWallet.ts', () => ({
    useWallet: () => ({
        ready: true,
        status: 'connected',
        connected: { address: WALLET, signer: createNoopSigner(WALLET), walletName: 'Test', supportsV0: true },
    }),
}));
vi.mock('../../tx/priorityFee.ts', () => ({ estimatePriorityFee: async () => 0n }));
vi.mock('../../tx/simulate.ts', () => ({
    simulatePlan: vi.fn(async () => {
        const error = h.outcome.length ? (h.outcome.shift() ?? null) : null;
        const report: SimulationReport = {
            ok: error === null,
            unitsConsumed: 100_000n,
            computeUnitLimit: 125_000,
            priorityFeeMicroLamports: 0n,
            feeLamports: 5_000n,
            sizeBytes: 0,
            deltas: [],
            logs: [],
            simulatedAt: Date.now(),
            error,
        };
        return { report, lifetime: null };
    }),
}));

const { useTxFlow } = await import('../../tx/useTxFlow.ts');

/** short.win's refusal as the mapper builds it from a simulation whose logs name short.win failing. */
const refusal = (code: number): TxError =>
    mapTransactionError({ InstructionError: [2, { Custom: code }] }, [
        `Program ${SHORTWIN_PROGRAM} invoke [1]`,
        `Program ${SHORTWIN_PROGRAM} failed: custom program error: 0x${code.toString(16)}`,
    ]);

/** A ReadSet whose first account (P0 / C0) has `len` bytes. */
function snapshot(kind: KindName, len: number | null) {
    const acc = (n: number): RawAccount => ({
        address: WALLET,
        owner: WALLET,
        lamports: 0n,
        executable: false,
        data: new Uint8Array(n),
    });
    return { kind, read: [len === null ? null : acc(len), acc(8)] };
}

beforeEach(() => {
    h.outcome = [];
});

describe('PUMP-2026-10-2: an open on a pump account its first trade grows', () => {
    it('is a PumpSwap pool under 287 bytes or a curve under 141, as the dumped fixtures are', () => {
        for (const [name, want] of [
            ['pamm_legacy211_pool', 'pool'],
            ['pamm_legacy261_pool', 'pool'],
            ['pamm_mature_pool', null],
        ] as const) {
            const a = fixtureAccount(name);
            if (!a) throw new Error(name);
            expect(venueGrowth({ kind: 'pump_swap', read: [a] }), name).toBe(want);
        }
        for (const [len, want] of [
            [211, 'pool'],
            [271, 'pool'],
            [286, 'pool'],
            [287, null],
            [300, null],
            [301, null],
        ] as const) {
            expect(venueGrowth(snapshot('pump_swap', len)), `pool ${len}`).toBe(want);
        }
        for (const [len, want] of [
            [115, 'curve'],
            [125, 'curve'],
            [140, 'curve'],
            [141, null],
            [151, null],
        ] as const) {
            expect(venueGrowth(snapshot('pump_curve', len)), `curve ${len}`).toBe(want);
        }
        expect(venueGrowth(snapshot('pump_swap', null))).toBeNull();
        expect(venueGrowth(snapshot('raydium_cpmm', 211))).toBeNull();
        expect(venueGrowth(snapshot('mock', 100))).toBeNull();
    });

    it('rewrites only short.win’s ProceedsBelowMin, keeping its code and action', () => {
        const { refineError } = growthRefinement(snapshot('pump_swap', 211));
        if (!refineError) throw new Error('a refinement');
        const e = refineError(refusal(6063));
        expect(e.message).toBe(VENUE_GROWTH_MESSAGES.pool);
        expect([e.kind, e.code, e.action, e.name]).toEqual(['program', 6063, 'retry', 'ProceedsBelowMin']);
        expect(e.logs).toEqual(refusal(6063).logs);
        expect(refineError(refusal(6062))).toEqual(refusal(6062));
        // PumpSwap's own 6063 (`InsufficientRealQuoteReserves`) maps to `unknown`, not to this.
        const venue: TxError = { kind: 'unknown', code: 6063, action: 'report', message: 'in pAMM' };
        expect(refineError(venue)).toEqual(venue);
        // A 6063 whose logs name no failing program (truncated logs) maps to short.win's error, but
        // could be PumpSwap's: it keeps the generic message.
        const unattributed = mapTransactionError({ InstructionError: [2, { Custom: 6063 }] }, []);
        expect([unattributed.kind, unattributed.code]).toEqual(['program', 6063]);
        expect(refineError(unattributed)).toEqual(unattributed);
        const truncated = mapTransactionError({ InstructionError: [2, { Custom: 6063 }] }, [
            `Program ${SHORTWIN_PROGRAM} invoke [1]`,
            'Log truncated',
        ]);
        expect(refineError(truncated)).toEqual(truncated);
        const curve = growthRefinement(snapshot('pump_curve', 125)).refineError;
        expect(curve?.(refusal(6063)).message).toBe(VENUE_GROWTH_MESSAGES.curve);
        expect(growthRefinement(snapshot('pump_swap', 301)).refineError).toBeUndefined();
    });

    it('the review shows the refined refusal; a plan without it the generic slippage message', async () => {
        const qc = new QueryClient();
        const wrapper = ({ children }: { children: ReactNode }) => (
            <QueryClientProvider client={qc}>{children}</QueryClientProvider>
        );
        const plan = (o: Partial<TxPlan>): TxPlan => ({
            kind: 'open_short',
            title: 'Open short',
            instructions: [],
            watch: [],
            disclosures: [],
            invalidate: [],
            ...o,
        });
        const { result } = renderHook(() => useTxFlow(), { wrapper });
        h.outcome = [refusal(6063)];
        await act(async () => {
            result.current.start(plan(growthRefinement(snapshot('pump_swap', 211))));
        });
        expect(result.current.state.stage).toBe('failed');
        expect(result.current.state.error?.message).toBe(VENUE_GROWTH_MESSAGES.pool);
        expect(result.current.state.report?.error?.message).toBe(VENUE_GROWTH_MESSAGES.pool);
        h.outcome = [refusal(6063)];
        await act(async () => {
            result.current.start(plan({}));
        });
        expect(result.current.state.error?.message).toBe(PROGRAM_ERRORS.get(6063)?.message);
    });
});
