/**
 * No blind signing (FRONTEND.md §5.8, §9.2): (1) every top-level instruction must be an allowed
 * (program, instruction) pair (`allowlist.ts`); (2) when the wallet returns a signed transaction
 * whose message differs from the one built and simulated, the difference may only be in
 * compute-budget instructions (a wallet may add or change a priority fee) and — when the caller
 * allows it and checks the new one itself — the blockhash; any other change refuses the send.
 * The same comparison checks a transaction a sending-only wallet landed, after the fact (§5.4).
 */
import {
    type AddressesByLookupTableAddress,
    decompileTransactionMessage,
    getCompiledTransactionMessageDecoder,
    getTransactionMessageComputeUnitLimit,
    getTransactionMessageComputeUnitPrice,
    type Instruction,
    type ReadonlyUint8Array,
} from '@solana/kit';
import { COMPUTE_BUDGET_PROGRAM } from '../config/programs.ts';
import { allowedInstructionName } from './allowlist.ts';

export class IntegrityError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'IntegrityError';
    }
}

/**
 * Refuses any instruction that is not an allowed (program, instruction) pair, and any
 * compute-budget instruction (the pipeline adds those itself, `message.ts`).
 */
export function assertAllowedInstructions(instructions: readonly Instruction[]): void {
    instructions.forEach((ix, i) => {
        if (ix.programAddress === COMPUTE_BUDGET_PROGRAM) {
            throw new IntegrityError(`refusing instruction ${i}: compute-budget instructions are the pipeline's own`);
        }
        if (allowedInstructionName(ix) === null) {
            throw new IntegrityError(
                `refusing instruction ${i}: ${ix.programAddress} is not allowed to run this instruction at the top level`,
            );
        }
    });
}

function bytesEqual(a: ReadonlyUint8Array | undefined, b: ReadonlyUint8Array | undefined): boolean {
    if (!a || !b) return a === b || ((a?.length ?? 0) === 0 && (b?.length ?? 0) === 0);
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
    return true;
}

function sameInstruction(x: Instruction, y: Instruction): boolean {
    if (x.programAddress !== y.programAddress) return false;
    const ax = x.accounts ?? [];
    const ay = y.accounts ?? [];
    if (ax.length !== ay.length) return false;
    for (let i = 0; i < ax.length; i++) {
        const p = ax[i];
        const q = ay[i];
        if (!p || !q || p.address !== q.address || p.role !== q.role) return false;
    }
    return bytesEqual(x.data, y.data);
}

/** What the comparison found besides the (identical) intent. */
export interface IntentCheck {
    /** The signed message differs from the built one (compute budget and / or lifetime). */
    readonly modified: boolean;
    /** The signed message's lifetime token (its blockhash) differs from the built one's. */
    readonly lifetimeChanged: boolean;
    /** The signed message's lifetime token (a blockhash). */
    readonly lifetimeToken: string;
    /** The signed message's compute-unit price (µlamports / CU; 0 without the instruction). */
    readonly computeUnitPrice: bigint;
    /** The signed message's compute-unit limit, `null` without the instruction. */
    readonly computeUnitLimit: number | null;
}

export interface IntentLimits {
    /** The highest compute-unit price a wallet may set (FE-2). */
    readonly maxComputeUnitPrice?: bigint;
    /**
     * Accept a changed lifetime token. Only for a caller that then checks the new blockhash and
     * bounds the confirmation itself (`send.ts`), or for the post hoc check of a landed transaction.
     */
    readonly allowLifetimeChange?: boolean;
}

/**
 * Throws `IntegrityError` unless `signedMessageBytes` carries the same fee payer and the same
 * non-compute-budget instructions, in order, as `expectedMessageBytes`, the same lifetime token
 * (unless `allowLifetimeChange`), and a compute-unit price (if the wallet set one) no higher than
 * `maxComputeUnitPrice`.
 */
export function assertSameIntent(
    expectedMessageBytes: ReadonlyUint8Array,
    signedMessageBytes: ReadonlyUint8Array,
    tables: AddressesByLookupTableAddress,
    limits: IntentLimits = {},
): IntentCheck {
    const dec = getCompiledTransactionMessageDecoder();
    const wantCompiled = dec.decode(expectedMessageBytes);
    const gotCompiled = dec.decode(signedMessageBytes);
    const opts = { addressesByLookupTableAddress: tables };
    const want = decompileTransactionMessage(wantCompiled, opts);
    const got = decompileTransactionMessage(gotCompiled, opts);
    const fees = got as Parameters<typeof getTransactionMessageComputeUnitPrice>[0];
    const check: IntentCheck = {
        modified: !bytesEqual(expectedMessageBytes, signedMessageBytes),
        lifetimeChanged: wantCompiled.lifetimeToken !== gotCompiled.lifetimeToken,
        lifetimeToken: gotCompiled.lifetimeToken,
        computeUnitPrice: getTransactionMessageComputeUnitPrice(fees) ?? 0n,
        computeUnitLimit: getTransactionMessageComputeUnitLimit(fees) ?? null,
    };
    if (!check.modified) return check;
    if (want.feePayer.address !== got.feePayer.address) throw new IntegrityError('the wallet changed the fee payer');
    const strip = (ixs: readonly Instruction[]) => ixs.filter((ix) => ix.programAddress !== COMPUTE_BUDGET_PROGRAM);
    const a = strip(want.instructions);
    const b = strip(got.instructions);
    if (a.length !== b.length) throw new IntegrityError('the wallet added or removed instructions');
    for (let i = 0; i < a.length; i++) {
        const x = a[i];
        const y = b[i];
        if (!x || !y || !sameInstruction(x, y)) throw new IntegrityError(`the wallet changed instruction ${i}`);
    }
    if (check.lifetimeChanged && !limits.allowLifetimeChange) {
        throw new IntegrityError('the wallet changed the transaction’s blockhash');
    }
    if (limits.maxComputeUnitPrice !== undefined && check.computeUnitPrice > limits.maxComputeUnitPrice) {
        throw new IntegrityError(
            `the wallet set a priority fee of ${check.computeUnitPrice} µlamports/CU, above the ceiling`,
        );
    }
    return check;
}
