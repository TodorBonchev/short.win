import { afterEach, describe, expect, it } from 'vitest';
import { SHORTWIN_PROGRAM, TOKEN_2022_PROGRAM } from '../config/programs.ts';
import { localNetwork } from '../wallet/localNetwork.ts';
import { failingProgram, mapThrown, mapTransactionError } from './errors.ts';
import { WalletCallError } from './send.ts';
import type { TxPlan } from './types.ts';
import { retryMode, type TxFlowState } from './useTxFlow.ts';

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

    it('maps short.win’s AccountNotInitialized (3012, a position closed meanwhile) to a rebuild', () => {
        // KI-67: a claim that empties a lender position closes it; a plan built before that fails
        // with Anchor's 3012, which a fresh plan fixes (it drops a closed sync target, or refuses
        // the claim of a closed position).
        const e = mapTransactionError({ InstructionError: [3, { Custom: 3012 }] }, [
            `Program ${SHORTWIN_PROGRAM} invoke [1]`,
            'Program log: AnchorError caused by account: lender. Error Code: AccountNotInitialized. Error Number: 3012.',
            `Program ${SHORTWIN_PROGRAM} failed: custom program error: 0xbc4`,
        ]);
        expect(e).toMatchObject({
            kind: 'program',
            code: 3012,
            name: 'AccountNotInitialized',
            action: 'retry',
            instructionIndex: 3,
        });
        expect(e.message).toMatch(/no longer exists/);
        // The review's Retry rebuilds the plan from the chain instead of re-simulating it.
        const plan = { rebuild: async () => ({}) as TxPlan } as unknown as TxPlan;
        const state = { stage: 'failed', plan, error: e, signature: null } as unknown as TxFlowState;
        expect(retryMode(state)).toBe('rebuild');
        // Another program's 3012 (an Anchor venue) stays unknown.
        const other = mapTransactionError({ InstructionError: [3, { Custom: 3012 }] }, [
            `Program ${TOKEN_2022_PROGRAM} failed: custom program error: 0xbc4`,
        ]);
        expect(other).toMatchObject({ kind: 'unknown', action: 'report' });
    });

    it('maps short.win’s AccountOwnedByWrongProgram (3007) on a lender position like 3012', () => {
        // FE-30: a position closed by a claim or withdrawal (KI-67) whose address someone then
        // funded is a system account there: Anchor's `Account<LenderPosition>` refuses it with
        // 3007, not 3012. A fresh plan reads it as no position (`lenderPositionOf`).
        const lenderLogs = (account: string, program: string = SHORTWIN_PROGRAM) => [
            `Program ${program} invoke [1]`,
            `Program log: AnchorError caused by account: ${account}. Error Code: AccountOwnedByWrongProgram. Error Number: 3007. Error Message: The given account is owned by a different program than expected.`,
            'Program log: Left:',
            'Program log: 11111111111111111111111111111111',
            'Program log: Right:',
            `Program log: ${SHORTWIN_PROGRAM}`,
            `Program ${program} failed: custom program error: 0xbbf`,
        ];
        const plan = { rebuild: async () => ({}) as TxPlan } as unknown as TxPlan;
        // The claimant's own position (claim_sol, after the two compute-budget instructions) and a
        // sync target's (sync_lender at index 2): both rebuild.
        for (const index of [3, 2]) {
            const e = mapTransactionError({ InstructionError: [index, { Custom: 3007 }] }, lenderLogs('lender'));
            expect(e).toMatchObject({
                kind: 'program',
                code: 3007,
                name: 'AccountOwnedByWrongProgram',
                action: 'retry',
                instructionIndex: index,
            });
            expect(e.message).toMatch(/no longer exists/);
            const state = { stage: 'failed', plan, error: e, signature: null } as unknown as TxFlowState;
            expect(retryMode(state)).toBe('rebuild');
        }
        // 3012 still rebuilds whatever account it names.
        expect(
            mapTransactionError({ InstructionError: [3, { Custom: 3012 }] }, [
                `Program ${SHORTWIN_PROGRAM} failed: custom program error: 0xbc4`,
            ]),
        ).toMatchObject({ kind: 'program', name: 'AccountNotInitialized', action: 'retry' });
        // short.win's 3007 on another account is not a closed position: a new plan would not fix it.
        expect(
            mapTransactionError({ InstructionError: [3, { Custom: 3007 }] }, lenderLogs('token_program')),
        ).toMatchObject({ kind: 'unknown', action: 'report', code: 3007 });
        expect(
            mapTransactionError({ InstructionError: [3, { Custom: 3007 }] }, [
                `Program ${SHORTWIN_PROGRAM} failed: custom program error: 0xbbf`,
            ]),
        ).toMatchObject({ kind: 'unknown', action: 'report' });
        // Another program's 3007 (an Anchor venue's own `lender`-named account included) stays unknown.
        expect(
            mapTransactionError({ InstructionError: [3, { Custom: 3007 }] }, lenderLogs('lender', TOKEN_2022_PROGRAM)),
        ).toMatchObject({ kind: 'unknown', action: 'report' });
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

describe('the Mobile Wallet Adapter’s errors at Sign (FRONTEND §5.9; mobile.md §1.8)', () => {
    /** As the library throws them: its own error, wrapped `new Error(message, { cause })`. */
    const adapterError = (code: string, message: string, data?: unknown) =>
        Object.assign(new Error(message), { name: 'SolanaMobileWalletAdapterError', code, data });
    const protocolError = (code: number, message: string) =>
        Object.assign(new Error(message), {
            name: 'SolanaMobileWalletAdapterProtocolError',
            code,
            jsonRpcMessageId: 3,
        });
    const wrapped = (inner: Error) => new Error(inner.message, { cause: inner });
    const atSign = (inner: Error, sending = false) => new WalletCallError(wrapped(inner), { sending });
    afterEach(() => localNetwork.set('ready'));

    it('−3 on sign_transactions: declined in the wallet (Cancelled in your wallet)', () => {
        expect(mapThrown(atSign(protocolError(-3, 'transaction not signed')))).toMatchObject({
            kind: 'wallet-rejected',
            action: 'retry',
        });
    });

    it.each([
        [-1, 'authorization failed'],
        [-2, 'invalid payloads'],
        [-4, 'not submitted'],
        [-5, 'too many payloads'],
        [-100, 'origin attestation failed'],
    ])('protocol code %i: the generic wallet error', (code, message) => {
        const m = mapThrown(atSign(protocolError(code, message)));
        expect(m).toMatchObject({ kind: 'wallet', action: 'retry' });
        expect(m.message).toBe(`Your wallet could not sign: ${message}`);
    });

    it('ERROR_WALLET_NOT_FOUND: a launch to retry (the review stays), never "no wallet"', () => {
        const m = mapThrown(atSign(adapterError('ERROR_WALLET_NOT_FOUND', 'Found no installed wallet')));
        expect(m).toMatchObject({ kind: 'wallet-launch', action: 'retry' });
        expect(m.message).toBe("Couldn't open your wallet app. Tap Sign again");
    });

    it('ERROR_LOOPBACK_ACCESS_BLOCKED: the local-network notice (its "denied" never reads as a rejection)', () => {
        const m = mapThrown(
            atSign(adapterError('ERROR_LOOPBACK_ACCESS_BLOCKED', 'Local Network Access permission denied')),
        );
        expect(m).toMatchObject({ kind: 'local-network', action: 'retry' });
        expect(m.message).toBe("Allow 'Apps on device' so this page can reach your wallet app");
    });

    it('ERROR_SESSION_TIMEOUT: the local-network notice while the permission is missing, else generic', () => {
        const timeout = () =>
            atSign(adapterError('ERROR_SESSION_TIMEOUT', 'Failed to connect to the wallet websocket'));
        localNetwork.set('denied');
        expect(mapThrown(timeout()).kind).toBe('local-network');
        localNetwork.set('prompt');
        expect(mapThrown(timeout()).kind).toBe('local-network');
        localNetwork.set('ready');
        expect(mapThrown(timeout())).toMatchObject({ kind: 'wallet', action: 'retry' });
    });

    it('ERROR_ASSOCIATION_CANCELLED: a closed library modal (with its event) is a cancel; the timer is generic', () => {
        const closed = adapterError('ERROR_ASSOCIATION_CANCELLED', 'Wallet connection cancelled by user', {
            event: { type: 'click' },
        });
        expect(mapThrown(atSign(closed)).kind).toBe('wallet-rejected');
        const timer = adapterError('ERROR_ASSOCIATION_CANCELLED', 'Wallet connection timed out', { event: undefined });
        // Not a launch retry: an unfinished inner flow could still reach the wallet.
        expect(mapThrown(atSign(timer))).toMatchObject({ kind: 'wallet', action: 'retry' });
    });

    it.each([
        'ERROR_SESSION_CLOSED',
        'ERROR_ILLEGAL_TRANSPORT_STATE',
        'ERROR_INVALID_PROTOCOL_VERSION',
        'ERROR_FORBIDDEN_WALLET_BASE_URL',
        'ERROR_SECURE_CONTEXT_REQUIRED',
    ])('%s: the generic wallet error, with Retry', (code) => {
        const m = mapThrown(atSign(adapterError(code, 'The wallet session was closed')));
        expect(m).toMatchObject({ kind: 'wallet', action: 'retry' });
        expect(m.message).toMatch(/^Your wallet could not sign: /);
    });

    it('walks the cause chain (up to 6 levels) and matches on name and code, never instanceof', () => {
        let e: Error = adapterError('ERROR_WALLET_NOT_FOUND', 'x');
        for (let i = 0; i < 4; i++) e = new Error('outer', { cause: e });
        expect(mapThrown(new WalletCallError(e)).kind).toBe('wallet-launch');
        // Deeper than 6 levels: not found (an ordinary wallet error).
        let deep: Error = adapterError('ERROR_WALLET_NOT_FOUND', 'x');
        for (let i = 0; i < 7; i++) deep = new Error('outer', { cause: deep });
        expect(mapThrown(new WalletCallError(deep)).kind).toBe('wallet');
        // A plain object with the name works too (the package is loaded lazily).
        const plain = new Error('m', { cause: { name: 'SolanaMobileWalletAdapterProtocolError', code: -3 } });
        expect(mapThrown(new WalletCallError(plain)).kind).toBe('wallet-rejected');
    });

    it('the same objects outside a wallet call never map to a rejection or a notice', () => {
        for (const inner of [
            protocolError(-3, 'transaction not signed'),
            adapterError('ERROR_WALLET_NOT_FOUND', 'Found no installed wallet'),
            adapterError('ERROR_LOOPBACK_ACCESS_BLOCKED', 'Local Network Access permission denied'),
            adapterError('ERROR_ASSOCIATION_CANCELLED', 'Wallet connection cancelled by user', { event: {} }),
        ]) {
            const m = mapThrown(wrapped(inner));
            expect(['wallet-rejected', 'wallet-launch', 'local-network'], inner.message).not.toContain(m.kind);
        }
    });

    it('a transport error on the sending branch: the outcome is unknown (no blind retry)', () => {
        for (const code of ['ERROR_SESSION_CLOSED', 'ERROR_SESSION_TIMEOUT', 'ERROR_ILLEGAL_TRANSPORT_STATE']) {
            const m = mapThrown(atSign(adapterError(code, 'closed'), true));
            expect(m, code).toMatchObject({ kind: 'unknown-outcome', action: 'report' });
            expect(m.message).toContain('Your wallet sent this transaction itself and it has not been seen on chain.');
            expect(m.message).toContain('Check the explorer before trying again');
            const plan = { rebuild: undefined } as unknown as TxPlan;
            const state = { stage: 'failed', plan, error: m, signature: null } as unknown as TxFlowState;
            expect(retryMode(state), code).toBe('none');
            // The same on the signing branch: generic, with Retry.
            expect(mapThrown(atSign(adapterError(code, 'closed'), false)).kind, code).toBe('wallet');
        }
        // A non-MWA error on the sending branch maps as before.
        expect(mapThrown(new WalletCallError(new Error('internal'), { sending: true })).kind).toBe('wallet');
        expect(
            mapThrown(new WalletCallError(Object.assign(new Error('no'), { code: 4001 }), { sending: true })).kind,
        ).toBe('wallet-rejected');
    });

    it('wallet-changed (R-S2) is retried by closing the review, never by re-simulating', () => {
        const plan = { rebuild: async () => ({}) as TxPlan } as unknown as TxPlan;
        const state = {
            stage: 'failed',
            plan,
            error: { kind: 'wallet-changed', action: 'blocked', message: 'Connect a wallet that can sign.' },
            signature: null,
        } as unknown as TxFlowState;
        expect(retryMode(state)).toBe('close');
    });
});
