import { describe, expect, it } from 'vitest';
import type { SimulationReport, TxPlan } from './types.ts';
import { canSign, requiredAcks, type TxFlowState, txReducer } from './useTxFlow.ts';

const plan: TxPlan = {
    kind: 'deposit',
    title: 'Deposit',
    instructions: [],
    watch: [],
    disclosures: ['lender.solRepayment', 'lender.risks'],
    invalidate: [],
};

const ok: SimulationReport = {
    ok: true,
    unitsConsumed: 1n,
    computeUnitLimit: 20_000,
    priorityFeeMicroLamports: 0n,
    feeLamports: 5_000n,
    sizeBytes: 500,
    deltas: [],
    logs: [],
    error: null,
    simulatedAt: 0,
};

const idle: TxFlowState = {
    stage: 'idle',
    plan: null,
    report: null,
    acks: new Set(),
    speed: 'normal',
    signature: null,
    error: null,
    refreshed: false,
};

describe('transaction flow', () => {
    it('gates signing on a successful simulation and every required acknowledgement', () => {
        expect(requiredAcks(plan)).toEqual(['lender.solRepayment']);
        let s = txReducer(idle, { t: 'start', plan });
        expect(s.stage).toBe('simulating');
        expect(canSign(s)).toBe(false);
        s = txReducer(s, { t: 'simulated', report: ok });
        expect(s.stage).toBe('review');
        expect(canSign(s)).toBe(false);
        s = txReducer(s, { t: 'ack', id: 'lender.solRepayment', v: true });
        expect(canSign(s)).toBe(true);
        s = txReducer(s, { t: 'ack', id: 'lender.solRepayment', v: false });
        expect(canSign(s)).toBe(false);
    });

    it('never offers Sign after a failed simulation', () => {
        let s = txReducer(idle, { t: 'start', plan });
        s = txReducer(s, { t: 'ack', id: 'lender.solRepayment', v: true });
        s = txReducer(s, {
            t: 'simulated',
            report: { ...ok, ok: false, error: { kind: 'program', code: 6051, message: 'paused', action: 'wait' } },
        });
        expect(s.stage).toBe('failed');
        expect(s.error?.code).toBe(6051);
        expect(canSign(s)).toBe(false);
    });

    it('moves signing → confirming → confirmed and resets on close, keeping the fee speed', () => {
        let s = txReducer(idle, { t: 'speed', speed: 'fast' });
        s = txReducer(s, { t: 'start', plan });
        s = txReducer(s, { t: 'simulated', report: ok });
        s = txReducer(s, { t: 'signing' });
        expect(s.stage).toBe('signing');
        s = txReducer(s, { t: 'signed', signature: 'sig' as never });
        expect(s.stage).toBe('confirming');
        s = txReducer(s, { t: 'confirmed' });
        expect(s.stage).toBe('confirmed');
        s = txReducer(s, { t: 'close' });
        expect(s).toEqual({ ...idle, speed: 'fast' });
    });
});
