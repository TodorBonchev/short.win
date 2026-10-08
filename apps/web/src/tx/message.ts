/**
 * Transaction message assembly (FRONTEND.md §5.2, §5.6): v0 with the protocol ALT (ARCH §6.4
 * A28) plus the plan's own tables, compute-unit limit and price first, the plan's instructions in
 * order. Legacy only when the wallet cannot sign v0 (and then without tables).
 */
import {
    type AddressesByLookupTableAddress,
    appendTransactionMessageInstructions,
    type Blockhash,
    compileTransaction,
    compressTransactionMessageUsingAddressLookupTables,
    createTransactionMessage,
    getBase64EncodedWireTransaction,
    getTransactionSize,
    type Instruction,
    setTransactionMessageComputeUnitLimit,
    setTransactionMessageComputeUnitPrice,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    type TransactionSigner,
} from '@solana/kit';

/** Solana's packet limit for a serialized transaction. */
export const MAX_TRANSACTION_BYTES = 1232;
/** The compute-unit ceiling used while simulating (the simulation measures, the real limit follows). */
export const SIMULATION_CU_LIMIT = 1_400_000;
/**
 * Compute-budget instructions `buildMessage` puts before the plan's (limit, price): a simulation
 * error's `instructionIndex` minus this is the index in `TxPlan.instructions`.
 */
export const LEADING_BUDGET_INSTRUCTIONS = 2;

export interface Lifetime {
    readonly blockhash: Blockhash;
    readonly lastValidBlockHeight: bigint;
}

export interface MessageInputs {
    readonly instructions: readonly Instruction[];
    readonly feePayer: TransactionSigner;
    readonly lifetime: Lifetime;
    readonly computeUnitLimit: number;
    readonly computeUnitPrice: bigint;
    readonly lookupTables: AddressesByLookupTableAddress;
    readonly version: 0 | 'legacy';
}

/** Builds the message the wallet will sign. */
export function buildMessage(i: MessageInputs) {
    if (i.version === 'legacy') {
        const m0 = createTransactionMessage({ version: 'legacy' });
        const m1 = setTransactionMessageFeePayerSigner(i.feePayer, m0);
        const m2 = setTransactionMessageLifetimeUsingBlockhash(i.lifetime, m1);
        const m3 = setTransactionMessageComputeUnitLimit(i.computeUnitLimit, m2);
        const m4 = setTransactionMessageComputeUnitPrice(i.computeUnitPrice, m3);
        return appendTransactionMessageInstructions([...i.instructions], m4);
    }
    const m0 = createTransactionMessage({ version: 0 });
    const m1 = setTransactionMessageFeePayerSigner(i.feePayer, m0);
    const m2 = setTransactionMessageLifetimeUsingBlockhash(i.lifetime, m1);
    const m3 = setTransactionMessageComputeUnitLimit(i.computeUnitLimit, m2);
    const m4 = setTransactionMessageComputeUnitPrice(i.computeUnitPrice, m3);
    const m5 = appendTransactionMessageInstructions([...i.instructions], m4);
    return Object.keys(i.lookupTables).length
        ? compressTransactionMessageUsingAddressLookupTables(m5, i.lookupTables)
        : m5;
}

export type BuiltMessage = ReturnType<typeof buildMessage>;

/** The unsigned wire transaction (base64) and its size. */
export function wireOf(message: BuiltMessage): {
    base64: ReturnType<typeof getBase64EncodedWireTransaction>;
    size: number;
} {
    const tx = compileTransaction(message);
    return { base64: getBase64EncodedWireTransaction(tx), size: getTransactionSize(tx) };
}

/**
 * The wire size of `instructions` as the pipeline would compile them for `feePayer` **without
 * lookup tables** — what a legacy-only wallet signs, and an upper bound of the v0 message with the
 * protocol ALT (FRONTEND.md §5.2) — for a plan that must fit `MAX_TRANSACTION_BYTES` before it is
 * simulated (the claim's prepended syncs). The blockhash is a placeholder of the same size.
 */
export function wireSizeOf(instructions: readonly Instruction[], feePayer: TransactionSigner): number {
    const message = buildMessage({
        instructions,
        feePayer,
        lifetime: { blockhash: '11111111111111111111111111111111' as Blockhash, lastValidBlockHeight: 0n },
        computeUnitLimit: SIMULATION_CU_LIMIT,
        computeUnitPrice: 1n,
        lookupTables: {},
        version: 0,
    });
    return wireOf(message).size;
}

/** The final compute-unit limit from a simulation: +15 % and +10,000 headroom, within [20,000, 1,400,000]. */
export function computeUnitLimitFor(unitsConsumed: bigint | null): number {
    if (unitsConsumed === null) return 400_000;
    const u = Number(unitsConsumed);
    return Math.min(SIMULATION_CU_LIMIT, Math.max(20_000, Math.ceil(u * 1.15) + 10_000));
}

/** Base fee (5,000 lamports per signature) plus the priority fee ⌈limit · price / 10⁶⌉. */
export function feeLamports(signatures: number, computeUnitLimit: number, microLamportsPerCu: bigint): bigint {
    const priority = (BigInt(computeUnitLimit) * microLamportsPerCu + 999_999n) / 1_000_000n;
    return 5_000n * BigInt(signatures) + priority;
}
