import { describe, expect, it } from 'vitest';
import { SHORTWIN_PROGRAM, TOKEN_2022_PROGRAM } from '../config/programs.ts';
import { failingProgram, mapThrown, mapTransactionError } from './errors.ts';
import { WalletCallError } from './send.ts';

const logs = (failing: string) => [
    `Program ${SHORTWIN_PROGRAM} invoke [1]`,
    `Program ${failing} invoke [2]`,
    `Program ${failing} failed: custom program error: 0x2a`,
    `Program ${SHORTWIN_PROGRAM} failed: custom program error: 0x2a`,
];

describe('transaction error mapping', () => {
    it('maps short.win custom errors to their messages', () => {
        const e = mapTransactionError({ InstructionError: [2, { Custom: 6063 }] }, [
            `Program ${SHORTWIN_PROGRAM} failed: custom program error: 0x17af`,
        ]);
        expect(e).toMatchObject({
            kind: 'program',
            code: 6063,
            name: 'ProceedsBelowMin',
            action: 'retry',
            instructionIndex: 2,
        });
    });

    it('attributes token-program errors raised inside short.win’s CPI (CPI Guard, memo)', () => {
        // The innermost failing program raised the error (the validator logs it first).
        expect(failingProgram(logs(TOKEN_2022_PROGRAM))).toBe(TOKEN_2022_PROGRAM);
        // The full nested logs, as solana-test-validator returns them for a CPI-Guarded deposit.
        const e = mapTransactionError({ InstructionError: [2, { Custom: 42 }] }, logs(TOKEN_2022_PROGRAM));
        expect(e).toMatchObject({ kind: 'token', code: 42, instructionIndex: 2 });
        expect(e.message).toMatch(/CPI Guard/);
        const m = mapTransactionError({ InstructionError: [0, { Custom: 36 }] }, [
            `Program ${TOKEN_2022_PROGRAM} failed: custom program error: 0x24`,
        ]);
        expect(m.message).toMatch(/memo/);
    });

    it('maps transaction-level errors', () => {
        expect(mapTransactionError('InsufficientFundsForFee')).toMatchObject({ kind: 'funds', action: 'adjust' });
        expect(mapTransactionError('BlockhashNotFound')).toMatchObject({ kind: 'expired', action: 'retry' });
        expect(mapTransactionError({ InsufficientFundsForRent: { account_index: 1 } })).toMatchObject({
            kind: 'funds',
        });
        expect(mapTransactionError({ InstructionError: [1, 'ComputationalBudgetExceeded'] })).toMatchObject({
            action: 'retry',
        });
        expect(mapTransactionError({ Weird: 1 })).toMatchObject({ kind: 'unknown', action: 'report' });
    });

    it('maps wallet rejections and Kit preflight errors', () => {
        expect(mapThrown(new WalletCallError(new Error('User rejected the request.')))).toMatchObject({
            kind: 'wallet-rejected',
        });
        expect(mapThrown(Object.assign(new Error('x'), { code: 4001 }))).toMatchObject({ kind: 'wallet-rejected' });
        const preflight = Object.assign(new Error('preflight'), {
            context: { transactionError: { InstructionError: [0, { Custom: 6051 }] }, logs: [] },
        });
        expect(mapThrown(preflight)).toMatchObject({ kind: 'program', name: 'CoveragePaused' });
        expect(mapThrown(new Error('block height exceeded'))).toMatchObject({ kind: 'expired' });
        const tampered = Object.assign(new Error('the wallet changed instruction 2'), { name: 'IntegrityError' });
        expect(mapThrown(tampered)).toMatchObject({ kind: 'integrity', action: 'report' });
    });
});
