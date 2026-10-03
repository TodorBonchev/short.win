/**
 * Quotes that need the program itself (FRONTEND.md §3.6, §5.10):
 *
 * - **`withdraw`'s SOL part** (`min_sol`, M1-SEC-2): the WASM previews read the stored state, but
 *   the program's prologue first accrues rent and releases the vest, and a last lender's full exit
 *   is paid the unreleased vest (R3B-2). The exact payout is what a simulation of the same
 *   withdrawal with `min_sol = 0` takes out of `MarketVault`; the plan the user signs carries it
 *   as `min_sol`, so a front-run that would cut it (a dust deposit making the exit "not last")
 *   reverts `SolBelowMin` instead. If that quote cannot be had, the WASM preview is used (its
 *   lower bound, a last exit's vest flush included), less the tolerance on that vest.
 * - **The pot for a claim**: other lenders whose touch moves realised SOL beyond their entitlement
 *   to the pot (R3A-2), judged with `previewTouch` on their on-chain state; `sync_lender(0)` of the
 *   best ≤ 4 runs before `claim_sol`.
 */
import { decodeLenderPosition, findLenderPda, type LenderPosition } from '@shortwin/client';
import { type Address, parseBase64RpcAccount, type TransactionSigner } from '@solana/kit';
import type { MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { simulatePlan } from '../../tx/simulate.ts';
import type { TxError, TxPlan } from '../../tx/types.ts';
import { MAX_SYNCS_PER_CLAIM, type SyncTarget, solPaidFrom } from './plans.ts';
import { type LendState, lessBps, potGainOfTouch } from './previews.ts';

export type Simulate = typeof simulatePlan;

export type SolQuote =
    | { readonly ok: true; readonly lamports: bigint; readonly source: 'simulation' | 'preview' }
    | { readonly ok: false; readonly error: TxError };

/**
 * `min_sol` for a withdrawal: the simulated payout of `plan` (built with `min_sol = 0`), else the
 * preview's lower bound `solAtLeast` (`previewWithdrawal`), which includes `vestOnLastExit`, the
 * unreleased vest a last lender's full exit is paid; the tolerance is taken off that vest.
 */
export async function quoteWithdrawSol(a: {
    readonly rpc: AppRpc;
    readonly plan: TxPlan;
    readonly signer: TransactionSigner;
    readonly vault: Address;
    readonly solAtLeast: bigint;
    readonly vestOnLastExit: bigint;
    readonly toleranceBps: number;
    readonly simulate?: Simulate;
}): Promise<SolQuote> {
    const simulate = a.simulate ?? simulatePlan;
    try {
        const { report } = await simulate({
            rpc: a.rpc,
            plan: a.plan,
            feePayer: a.signer,
            lookupTables: {},
            version: 0,
            priorityFeeMicroLamports: 0n,
        });
        const paid = report.ok ? solPaidFrom(report.deltas, a.vault) : null;
        if (paid !== null) return { ok: true, lamports: paid, source: 'simulation' };
    } catch {
        // fall through to the preview's bound
    }
    const cut = a.vestOnLastExit - lessBps(a.vestOnLastExit, a.toleranceBps);
    return { ok: true, lamports: a.solAtLeast > cut ? a.solAtLeast - cut : 0n, source: 'preview' };
}

export interface SyncPick {
    readonly syncs: readonly SyncTarget[];
    /** What the pot gains from them. */
    readonly gain: bigint;
}

/**
 * Picks other lenders whose touch funds the pot, largest gain first, until `shortfall` is covered
 * (≤ `MAX_SYNCS_PER_CLAIM`). Only lenders in the current share epoch are judged (no records
 * needed for `sync_lender(0)`).
 */
export function pickSyncs(
    m: MathModule,
    base: Omit<LendState, 'lender' | 'records'>,
    others: readonly { readonly address: Address; readonly lender: LenderPosition }[],
    now: bigint,
    shortfall: bigint,
): SyncPick {
    const judged = others
        .filter((o) => o.lender.epoch === base.market.shareEpoch)
        .map((o) => ({ address: o.address, gain: potGainOfTouch(m, { ...base, lender: o.lender, records: [] }, now) }))
        .filter((o) => o.gain > 0n)
        .sort((a, b) => (a.gain > b.gain ? -1 : a.gain < b.gain ? 1 : 0));
    const syncs: SyncTarget[] = [];
    let gain = 0n;
    for (const o of judged) {
        if (gain >= shortfall || syncs.length >= MAX_SYNCS_PER_CLAIM) break;
        syncs.push({ lender: o.address });
        gain += o.gain;
    }
    return { syncs, gain };
}

/** Reads the `LenderPosition`s of `owners` in `market` (other lenders, for `pickSyncs`). */
export async function readOtherLenders(
    rpc: AppRpc,
    market: Address,
    owners: readonly Address[],
): Promise<{ address: Address; lender: LenderPosition }[]> {
    if (!owners.length) return [];
    const addresses = await Promise.all(owners.map(async (owner) => (await findLenderPda({ market, owner }))[0]));
    const { value } = await rpc.getMultipleAccounts(addresses, { encoding: 'base64', commitment: 'confirmed' }).send();
    const out: { address: Address; lender: LenderPosition }[] = [];
    addresses.forEach((address, i) => {
        const d = decodeLenderPosition(parseBase64RpcAccount(address, value[i] ?? null));
        if (d.exists) out.push({ address, lender: d.data });
    });
    return out;
}
