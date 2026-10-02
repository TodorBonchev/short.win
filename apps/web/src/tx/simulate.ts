/**
 * Simulate before signing (SPEC §9; ARCH §7.3; FRONTEND.md §5.3): the plan is compiled exactly as
 * it will be signed (fee payer = the connected wallet, v0 + ALTs, compute budget at the simulation
 * ceiling) and simulated with `sigVerify: false` and `replaceRecentBlockhash: true` at `confirmed`,
 * returning the post-state of every watched account; the review shows the decoded deltas, the
 * measured compute units and the fee.
 */
import { type AddressesByLookupTableAddress, getBase64Encoder, type TransactionSigner } from '@solana/kit';
import type { AppRpc } from '../rpc/client.ts';
import { type AccountSnapshot, computeDeltas } from './deltas.ts';
import { mapThrown, mapTransactionError } from './errors.ts';
import { assertAllowedInstructions, type IntegrityError } from './integrity.ts';
import {
    buildMessage,
    computeUnitLimitFor,
    feeLamports,
    type Lifetime,
    MAX_TRANSACTION_BYTES,
    SIMULATION_CU_LIMIT,
    wireOf,
} from './message.ts';
import type { SimulationReport, TxPlan } from './types.ts';

export interface SimulateArgs {
    readonly rpc: AppRpc;
    readonly plan: TxPlan;
    readonly feePayer: TransactionSigner;
    readonly lookupTables: AddressesByLookupTableAddress;
    readonly version: 0 | 'legacy';
    readonly priorityFeeMicroLamports: bigint;
}

const b64 = getBase64Encoder();

function snapshot(
    acc: { lamports: bigint | number; data: readonly [string, string] | string[] } | null,
): AccountSnapshot | null {
    if (!acc) return null;
    const raw = Array.isArray(acc.data) ? (acc.data[0] ?? '') : '';
    return { lamports: BigInt(acc.lamports), data: new Uint8Array(b64.encode(raw)) };
}

function failed(partial: Partial<SimulationReport>, error: SimulationReport['error']): SimulationReport {
    return {
        ok: false,
        unitsConsumed: null,
        computeUnitLimit: 0,
        priorityFeeMicroLamports: 0n,
        feeLamports: 0n,
        sizeBytes: 0,
        deltas: [],
        logs: [],
        simulatedAt: Date.now(),
        ...partial,
        error,
    };
}

export async function simulatePlan(a: SimulateArgs): Promise<{ report: SimulationReport; lifetime: Lifetime | null }> {
    try {
        assertAllowedInstructions(a.plan.instructions);
    } catch (e) {
        return {
            report: failed({}, { kind: 'integrity', action: 'report', message: (e as IntegrityError).message }),
            lifetime: null,
        };
    }
    try {
        const { value: lifetime } = await a.rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
        const message = buildMessage({
            instructions: a.plan.instructions,
            feePayer: a.feePayer,
            lifetime,
            computeUnitLimit: SIMULATION_CU_LIMIT,
            computeUnitPrice: a.priorityFeeMicroLamports,
            lookupTables: a.lookupTables,
            version: a.version,
        });
        const wire = wireOf(message);
        if (wire.size > MAX_TRANSACTION_BYTES) {
            return {
                report: failed(
                    { sizeBytes: wire.size },
                    {
                        kind: 'size',
                        action: 'report',
                        message: `The transaction is ${wire.size} bytes, over the ${MAX_TRANSACTION_BYTES}-byte limit.`,
                    },
                ),
                lifetime,
            };
        }
        const addresses = a.plan.watch.map((w) => w.address);
        const [pre, sim] = await Promise.all([
            addresses.length
                ? a.rpc.getMultipleAccounts(addresses, { encoding: 'base64', commitment: 'confirmed' }).send()
                : Promise.resolve({ value: [] as null[] }),
            a.rpc
                .simulateTransaction(wire.base64, {
                    encoding: 'base64',
                    sigVerify: false,
                    replaceRecentBlockhash: true,
                    commitment: 'confirmed',
                    accounts: { addresses, encoding: 'base64' },
                })
                .send(),
        ]);
        const v = sim.value;
        const logs = v.logs ?? [];
        const unitsConsumed = v.unitsConsumed ?? null;
        const computeUnitLimit = computeUnitLimitFor(unitsConsumed);
        const deltas = computeDeltas(
            a.plan.watch,
            pre.value.map((x) => snapshot(x as Parameters<typeof snapshot>[0])),
            (v.accounts ?? []).map((x) => snapshot(x as Parameters<typeof snapshot>[0])),
        );
        const report: SimulationReport = {
            ok: v.err === null,
            unitsConsumed,
            computeUnitLimit,
            priorityFeeMicroLamports: a.priorityFeeMicroLamports,
            feeLamports: feeLamports(1, computeUnitLimit, a.priorityFeeMicroLamports),
            sizeBytes: wire.size,
            deltas,
            logs,
            error: v.err === null ? null : mapTransactionError(v.err, logs),
            simulatedAt: Date.now(),
        };
        return { report, lifetime };
    } catch (e) {
        return { report: failed({}, mapThrown(e)), lifetime: null };
    }
}
