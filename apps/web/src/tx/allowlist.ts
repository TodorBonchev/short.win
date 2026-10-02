/**
 * The top-level instruction allowlist (FRONTEND.md §5.8, §9.2): a transaction this app builds may
 * contain, besides the compute-budget instructions the pipeline itself adds (`message.ts`), only
 * these (program, instruction) pairs — not whole programs. A plan or venue builder that emitted,
 * say, a System transfer, an SPL Token `SetAuthority` / `Approve` / `TransferChecked` or a pump
 * `buy` is refused before simulation, so a builder bug cannot reach the wallet.
 *
 * | Program | Instructions | Why |
 * |---|---|---|
 * | short.win | every instruction of the program (identified by its discriminator) | the protocol |
 * | Associated Token | `CreateIdempotent` | a missing withdrawal destination, the sweep bundle's WSOL ATA, PumpSwap's coin-creator vault (§5.1) |
 * | Token-2022 | `EnableCpiGuard`, `DisableCpiGuard` | the CPI Guard bracket (FE-1, §5.7) |
 * | pump | `extend_account`, `claim_cashback` | a short curve before listing; the post-wipe sweep bundle |
 * | PumpSwap | `init_user_volume_accumulator`, `claim_cashback` | the post-wipe sweep bundle |
 *
 * `describe.ts` names the review's instructions from the same table, so what the review lists is
 * exactly what this gate let through.
 */
import { identifyShortwinInstruction, ShortwinInstruction } from '@shortwin/client';
import type { Address, Instruction, ReadonlyUint8Array } from '@solana/kit';
import {
    ASSOCIATED_TOKEN_PROGRAM,
    PUMP_AMM_PROGRAM,
    PUMP_PROGRAM,
    SHORTWIN_PROGRAM,
    TOKEN_2022_PROGRAM,
} from '../config/programs.ts';
import { IX_CLAIM_CASHBACK, IX_INIT_USER_VOLUME_ACCUMULATOR, PUMP_IX_EXTEND_ACCOUNT } from '../venues/constants.ts';

/** One allowed top-level instruction: its program and how to recognise its data. */
export interface AllowedInstruction {
    readonly program: Address;
    /** The instruction's name as its program's client names it (shown in the review). */
    readonly name: string;
    /** The exact data, or a discriminator prefix (`prefix: true`). */
    readonly data: readonly number[];
    readonly prefix: boolean;
}

const bytes = (b: ArrayLike<number>): number[] => Array.from(b);

/** Every (program, instruction) pair a plan may put at the top level, short.win's aside. */
export const ALLOWED_INSTRUCTIONS: readonly AllowedInstruction[] = [
    // spl-associated-token-account: `CreateIdempotent` (1); the legacy `Create` (empty data) is never built.
    { program: ASSOCIATED_TOKEN_PROGRAM, name: 'CreateAssociatedTokenIdempotent', data: [1], prefix: false },
    // Token-2022 `CpiGuardExtension` (34): `Enable` (0), `Disable` (1); nothing else of Token-2022.
    { program: TOKEN_2022_PROGRAM, name: 'EnableCpiGuard', data: [34, 0], prefix: false },
    { program: TOKEN_2022_PROGRAM, name: 'DisableCpiGuard', data: [34, 1], prefix: false },
    { program: PUMP_PROGRAM, name: 'extend_account', data: bytes(PUMP_IX_EXTEND_ACCOUNT), prefix: true },
    { program: PUMP_PROGRAM, name: 'claim_cashback', data: bytes(IX_CLAIM_CASHBACK), prefix: true },
    {
        program: PUMP_AMM_PROGRAM,
        name: 'init_user_volume_accumulator',
        data: bytes(IX_INIT_USER_VOLUME_ACCUMULATOR),
        prefix: true,
    },
    { program: PUMP_AMM_PROGRAM, name: 'claim_cashback', data: bytes(IX_CLAIM_CASHBACK), prefix: true },
];

function matches(entry: AllowedInstruction, data: ReadonlyUint8Array): boolean {
    if (entry.prefix ? data.length < entry.data.length : data.length !== entry.data.length) return false;
    for (let i = 0; i < entry.data.length; i++) if (data[i] !== entry.data[i]) return false;
    return true;
}

/** The short.win instruction's name, or `null` for data no short.win instruction starts with. */
export function shortwinInstructionName(data: ReadonlyUint8Array): string | null {
    try {
        return ShortwinInstruction[identifyShortwinInstruction(Uint8Array.from(data))] ?? null;
    } catch {
        return null;
    }
}

/**
 * The allowlist entry an instruction matches (its name), or `null` when the app must not sign it.
 * short.win instructions match by their own discriminators.
 */
export function allowedInstructionName(ix: Instruction): string | null {
    const data = ix.data ?? new Uint8Array();
    if (ix.programAddress === SHORTWIN_PROGRAM) return shortwinInstructionName(data);
    for (const entry of ALLOWED_INSTRUCTIONS) {
        if (entry.program === ix.programAddress && matches(entry, data)) return entry.name;
    }
    return null;
}

/** The programs with at least one allowed top-level instruction. */
export const ALLOWED_PROGRAMS: readonly Address[] = [
    SHORTWIN_PROGRAM,
    ...new Set(ALLOWED_INSTRUCTIONS.map((e) => e.program)),
];
