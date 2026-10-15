/**
 * Names a plan's instructions for the review (FRONTEND.md §5.3): the program and the instruction,
 * decoded from the instruction's own program id and discriminator — never from the feature's
 * description — so the review lists exactly what will be signed (e.g. the CPI Guard bracket's
 * `DisableCpiGuard → Deposit → EnableCpiGuard`, §5.7, or the prepended ATA creation and sweep
 * bundle). Allowed instructions are named from the same table the integrity gate uses
 * (`allowlist.ts`); anything else is named as well as it can be and marked not allowed
 * (`assertAllowedInstructions` refuses it before simulation, so it can never be signed).
 */
import type { Instruction } from '@solana/kit';
import {
    AssociatedTokenInstruction,
    identifyAssociatedTokenInstruction,
    identifyTokenInstruction,
    TokenInstruction,
} from '@solana-program/token';
import { identifyToken2022Instruction, Token2022Instruction } from '@solana-program/token-2022';
import {
    ASSOCIATED_TOKEN_PROGRAM,
    COMPUTE_BUDGET_PROGRAM,
    MEMO_PROGRAM,
    PUMP_AMM_PROGRAM,
    PUMP_PROGRAM,
    SHORTWIN_PROGRAM,
    SYSTEM_PROGRAM,
    TOKEN_2022_PROGRAM,
    TOKEN_PROGRAM,
} from '../config/programs.ts';
import { IX_CLAIM_CASHBACK, IX_INIT_USER_VOLUME_ACCUMULATOR, PUMP_IX_EXTEND_ACCOUNT } from '../venues/constants.ts';
import { allowedInstructionName, shortwinInstructionName } from './allowlist.ts';

export interface InstructionSummary {
    /** The program's display name (`short.win`, `Token-2022`, …) or its address when not pinned. */
    readonly program: string;
    /** The instruction's name as its program's client names it (`Deposit`, `DisableCpiGuard`, …). */
    readonly name: string;
    /** Whether the top-level allowlist lets it through (`allowlist.ts`); a refused one is never signed. */
    readonly allowed: boolean;
}

const PROGRAM_NAMES: Readonly<Record<string, string>> = {
    [SHORTWIN_PROGRAM]: 'short.win',
    [COMPUTE_BUDGET_PROGRAM]: 'Compute Budget',
    [SYSTEM_PROGRAM]: 'System',
    [ASSOCIATED_TOKEN_PROGRAM]: 'Associated Token',
    [TOKEN_PROGRAM]: 'Token',
    [TOKEN_2022_PROGRAM]: 'Token-2022',
    [MEMO_PROGRAM]: 'Memo',
    [PUMP_PROGRAM]: 'pump',
    [PUMP_AMM_PROGRAM]: 'PumpSwap',
};

const SYSTEM_NAMES: Readonly<Record<number, string>> = { 0: 'CreateAccount', 1: 'Assign', 2: 'TransferSol' };

/** Anchor-style instructions of the venue programs the app may prepend (8-byte discriminators). */
const ANCHOR_VENUE: readonly (readonly [Uint8Array | readonly number[], string])[] = [
    [PUMP_IX_EXTEND_ACCOUNT, 'extend_account'],
    [IX_CLAIM_CASHBACK, 'claim_cashback'],
    [IX_INIT_USER_VOLUME_ACCUMULATOR, 'init_user_volume_accumulator'],
];

function startsWith(data: Uint8Array, prefix: Uint8Array | readonly number[]): boolean {
    if (data.length < prefix.length) return false;
    for (let i = 0; i < prefix.length; i++) if (data[i] !== prefix[i]) return false;
    return true;
}

function named<E extends Record<number, string>>(enumObject: E, identify: () => number): string | null {
    try {
        return enumObject[identify()] ?? null;
    } catch {
        return null;
    }
}

function instructionName(program: string, data: Uint8Array): string | null {
    switch (program) {
        case SHORTWIN_PROGRAM:
            return shortwinInstructionName(data);
        case TOKEN_2022_PROGRAM:
            return named(Token2022Instruction, () => identifyToken2022Instruction(data));
        case TOKEN_PROGRAM:
            return named(TokenInstruction, () => identifyTokenInstruction(data));
        case ASSOCIATED_TOKEN_PROGRAM:
            // An empty data field is the legacy `Create`.
            return data.length === 0
                ? 'CreateAssociatedToken'
                : named(AssociatedTokenInstruction, () => identifyAssociatedTokenInstruction(data));
        case SYSTEM_PROGRAM:
            // u32 LE discriminator; only these appear in the app's plans (no System client at runtime).
            return data.length >= 4 && data[1] === 0 && data[2] === 0 && data[3] === 0
                ? (SYSTEM_NAMES[data[0] ?? -1] ?? null)
                : null;
        case MEMO_PROGRAM:
            return 'Memo';
        case PUMP_PROGRAM:
        case PUMP_AMM_PROGRAM:
            return ANCHOR_VENUE.find(([d]) => startsWith(data, d))?.[1] ?? null;
        default:
            return null;
    }
}

export function describeInstruction(ix: Instruction): InstructionSummary {
    const program = ix.programAddress;
    const allowedName = allowedInstructionName(ix);
    const data = ix.data ? Uint8Array.from(ix.data) : new Uint8Array();
    return {
        program: PROGRAM_NAMES[program] ?? program,
        name: allowedName ?? instructionName(program, data) ?? 'Unknown instruction',
        allowed: allowedName !== null,
    };
}
