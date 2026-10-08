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
 * - **The pot for a claim** (KI66-R4, the owner's eighth round): other lenders whose
 *   `sync_lender` — sent by the claimant, so without their owners' signatures: the pot-only touch
 *   `pool::touch_pot` (KI-66 full) — moves realised SOL beyond their entitlement to the pot
 *   (R3A-2), judged with `previewTouchPot` on their on-chain state and the records from where each
 *   walk starts (`potEpochs`: a chained lender's `fwdEpoch`, else its `epoch`; ≤ 8, read once for
 *   all of them). Current lenders and lenders behind a rebase record alike: the latter are held at
 *   the record and their forward chain walked, so what their unconverted shares earned after it
 *   reaches the pot without their owners. The best ≤ 4 run before `claim_sol`.
 */
import { decodeLenderPosition, findLenderPda, type LenderPosition, type ShareEpoch } from '@shortwin/client';
import { type Address, parseBase64RpcAccount, type TransactionSigner } from '@solana/kit';
import type { MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { simulatePlan } from '../../tx/simulate.ts';
import type { TxError, TxPlan } from '../../tx/types.ts';
import { potEpochs, readRecords } from './chain.ts';
import { MAX_SYNCS_PER_CLAIM, type SyncTarget, solPaidFrom } from './plans.ts';
import { type LendState, lessBps, potGainOfSync, type RecordFields } from './previews.ts';

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
    /** Largest gain first; each carries the epochs of its records (`potEpochs`). */
    readonly syncs: readonly SyncTarget[];
    /** What the pot gains from them (the gains add up: each sync moves only its own lender's surplus). */
    readonly gain: bigint;
    /** Lenders with a gain left out by the cap of `MAX_SYNCS_PER_CLAIM` while the shortfall is not covered. */
    readonly more: { readonly count: number; readonly gain: bigint };
}

/** Another lender of the market, as `readOtherLenders` reads it. */
export interface OtherLender {
    readonly address: Address;
    readonly lender: LenderPosition;
}

/**
 * The `ShareEpoch` records every candidate's pot-only sync needs (`potEpochs`), in one
 * `readRecords` over the union of their epochs.
 */
export async function readPotRecords(
    rpc: AppRpc,
    market: Address,
    shareEpoch: number,
    others: readonly OtherLender[],
): Promise<ShareEpoch[]> {
    const epochs = [...new Set(others.flatMap((o) => potEpochs(o.lender, shareEpoch)))].sort((a, b) => a - b);
    return readRecords(rpc, market, epochs);
}

/**
 * Picks other lenders whose `sync_lender` (unsigned: `pool::touch_pot`) funds the pot, largest gain
 * first, until `shortfall` is covered (≤ `MAX_SYNCS_PER_CLAIM`). Every lender is judged — current,
 * parked or behind — with `previewTouchPot` on its state and the records of its `potEpochs` among
 * `records`; a lender whose records are not all at hand is judged on those that are (the program
 * walks as far as the records it is given reach: a lender more than 8 records behind advances 8 per
 * claim).
 */
export function pickPotSyncs(
    m: MathModule,
    base: Omit<LendState, 'lender' | 'records'>,
    others: readonly OtherLender[],
    records: readonly RecordFields[],
    now: bigint,
    shortfall: bigint,
): SyncPick {
    const shareEpoch = base.market.shareEpoch;
    const judged = others
        .map((o) => {
            const epochs = potEpochs(o.lender, shareEpoch);
            const have = new Set(epochs);
            const recs = records.filter((r) => have.has(r.epoch));
            // Consecutive from the first needed epoch only (`ShareEpochs` must be in order, no gaps).
            const run: RecordFields[] = [];
            for (const e of epochs) {
                const r = recs.find((x) => x.epoch === e);
                if (!r) break;
                run.push(r);
            }
            return {
                lender: o.address,
                owner: o.lender.owner,
                epochs: run.map((r) => r.epoch),
                gain: potGainOfSync(m, { ...base, lender: o.lender, records: run }, now),
            };
        })
        .filter((o) => o.gain > 0n)
        .sort((a, b) => (a.gain > b.gain ? -1 : a.gain < b.gain ? 1 : 0));
    const syncs: SyncTarget[] = [];
    let gain = 0n;
    let i = 0;
    for (; i < judged.length; i++) {
        if (gain >= shortfall || syncs.length >= MAX_SYNCS_PER_CLAIM) break;
        const o = judged[i] as (typeof judged)[number];
        syncs.push(o);
        gain += o.gain;
    }
    const rest = gain < shortfall ? judged.slice(i) : [];
    return { syncs, gain, more: { count: rest.length, gain: rest.reduce((x, o) => x + o.gain, 0n) } };
}

/**
 * Reads the `LenderPosition`s of `owners` in `market` (other lenders, for `pickPotSyncs`); owners
 * without an account (never lent, or closed by `withdraw` / `claim_sol`, KI-67) are dropped.
 */
export async function readOtherLenders(
    rpc: AppRpc,
    market: Address,
    owners: readonly Address[],
): Promise<OtherLender[]> {
    if (!owners.length) return [];
    const addresses = await Promise.all(owners.map(async (owner) => (await findLenderPda({ market, owner }))[0]));
    const { value } = await rpc.getMultipleAccounts(addresses, { encoding: 'base64', commitment: 'confirmed' }).send();
    const out: OtherLender[] = [];
    addresses.forEach((address, i) => {
        const d = decodeLenderPosition(parseBase64RpcAccount(address, value[i] ?? null));
        if (d.exists) out.push({ address, lender: d.data });
    });
    return out;
}
