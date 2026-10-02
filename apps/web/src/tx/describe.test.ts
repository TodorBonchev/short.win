import { getClaimSolInstructionDataEncoder, getDepositInstructionDataEncoder } from '@shortwin/client';
import { address, createNoopSigner, type Instruction } from '@solana/kit';
import { getCreateAssociatedTokenIdempotentInstruction } from '@solana-program/token';
import { getDisableCpiGuardInstruction, getEnableCpiGuardInstruction } from '@solana-program/token-2022';
import { describe, expect, it } from 'vitest';
import {
    COMPUTE_BUDGET_PROGRAM,
    MEMO_PROGRAM,
    PUMP_AMM_PROGRAM,
    PUMP_PROGRAM,
    SHORTWIN_PROGRAM,
    SYSTEM_PROGRAM,
    TOKEN_2022_PROGRAM,
} from '../config/programs.ts';
import { IX_CLAIM_CASHBACK, IX_INIT_USER_VOLUME_ACCUMULATOR, PUMP_IX_EXTEND_ACCOUNT } from '../venues/constants.ts';
import { describeInstruction } from './describe.ts';

const OWNER = createNoopSigner(address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin'));
const ACCOUNT = address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const ix = (programAddress: Instruction['programAddress'], data: Uint8Array): Instruction => ({ programAddress, data });

describe('describeInstruction (the review’s instruction list)', () => {
    it('names the CPI Guard bracket from its bytes: DisableCpiGuard → Deposit → EnableCpiGuard', () => {
        const deposit = ix(
            SHORTWIN_PROGRAM,
            Uint8Array.from(getDepositInstructionDataEncoder().encode({ amount: 1n, minShares: 0n })),
        );
        const steps = [
            getDisableCpiGuardInstruction({ token: ACCOUNT, owner: OWNER }),
            deposit,
            getEnableCpiGuardInstruction({ token: ACCOUNT, owner: OWNER }),
        ].map(describeInstruction);
        expect(steps).toEqual([
            { program: 'Token-2022', name: 'DisableCpiGuard', allowed: true },
            { program: 'short.win', name: 'Deposit', allowed: true },
            { program: 'Token-2022', name: 'EnableCpiGuard', allowed: true },
        ]);
        expect(getDisableCpiGuardInstruction({ token: ACCOUNT, owner: OWNER }).programAddress).toBe(TOKEN_2022_PROGRAM);
    });

    it('names prepended instructions: ATA creation, the sweep’s venue calls, extend_account', () => {
        const ata = getCreateAssociatedTokenIdempotentInstruction({
            payer: OWNER,
            ata: ACCOUNT,
            owner: OWNER.address,
            mint: ACCOUNT,
        });
        expect(describeInstruction(ata)).toEqual({
            program: 'Associated Token',
            name: 'CreateAssociatedTokenIdempotent',
            allowed: true,
        });
        expect(describeInstruction(ix(PUMP_PROGRAM, IX_CLAIM_CASHBACK)).name).toBe('claim_cashback');
        expect(describeInstruction(ix(PUMP_AMM_PROGRAM, IX_INIT_USER_VOLUME_ACCUMULATOR)).name).toBe(
            'init_user_volume_accumulator',
        );
        expect(describeInstruction(ix(PUMP_PROGRAM, Uint8Array.from(PUMP_IX_EXTEND_ACCOUNT)))).toEqual({
            program: 'pump',
            name: 'extend_account',
            allowed: true,
        });
        expect(
            describeInstruction(ix(SHORTWIN_PROGRAM, Uint8Array.from(getClaimSolInstructionDataEncoder().encode({})))),
        ).toEqual({ program: 'short.win', name: 'ClaimSol', allowed: true });
        // Named, but not allowed at the top level (the integrity gate refuses them, M9-SEC-7).
        expect(describeInstruction(ix(SYSTEM_PROGRAM, Uint8Array.from([2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0])))).toEqual({
            program: 'System',
            name: 'TransferSol',
            allowed: false,
        });
        expect(describeInstruction(ix(MEMO_PROGRAM, new TextEncoder().encode('x')))).toEqual({
            program: 'Memo',
            name: 'Memo',
            allowed: false,
        });
    });

    it('shows an undecodable instruction as unknown, with the program named', () => {
        expect(describeInstruction(ix(SHORTWIN_PROGRAM, Uint8Array.from([1, 2, 3])))).toEqual({
            program: 'short.win',
            name: 'Unknown instruction',
            allowed: false,
        });
        expect(describeInstruction(ix(COMPUTE_BUDGET_PROGRAM, Uint8Array.from([2, 0, 0, 0, 0]))).program).toBe(
            'Compute Budget',
        );
        const stranger = address('Stake11111111111111111111111111111111111111');
        expect(describeInstruction(ix(stranger, new Uint8Array()))).toEqual({
            program: stranger,
            name: 'Unknown instruction',
            allowed: false,
        });
    });
});
