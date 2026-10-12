/**
 * The lender plans built from one chain read (FRONTEND.md §3.6, §5.1): each form's Review and the
 * review's Retry (`TxPlan.rebuild`, M9-UX-1) run the same function — Review on the panel's current
 * read, Retry on a fresh one (`readLendChain`) with the same user inputs. So after a refusal a
 * fresh plan fixes (a moved quote under `min_shares` / `min_tokens` / `min_sol`, a step the plan
 * lacked — epoch records, the post-wipe sweep bundle, the memo program — or a changed account),
 * Retry re-reads the chain, re-quotes and re-derives the steps; inputs that no longer fit (a
 * balance gone, a source now behind CPI Guard) end in a `PlanError` the review shows.
 */
import { findShareEpochPda, getLenderPositionSize } from '@shortwin/client';
import type { Address, Instruction, TransactionSigner } from '@solana/kit';
import type { MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { LEADING_BUDGET_INSTRUCTIONS, MAX_TRANSACTION_BYTES, wireSizeOf } from '../../tx/message.ts';
import { simulatePlan } from '../../tx/simulate.ts';
import type { TxPlan } from '../../tx/types.ts';
import { chainNowSeconds, liveWipeRecordEpoch, marketNow, readLendChain } from './chain.ts';
import { type LoadedChain, lendStateOf, lockGraceOf, planContextFor, subUnitVarsOf, unpaidRentVarsOf } from './ctx.ts';
import {
    buildClaimPlan,
    buildCoverPlan,
    buildDepositPlan,
    buildSyncPlan,
    buildWithdrawPlan,
    PlanError,
    type SyncTarget,
} from './plans.ts';
import {
    hasFlag,
    MARKET_FLAG,
    previewClaimOutcome,
    previewClaimView,
    previewDepositView,
    previewSubUnitConversion,
    previewUnpaidRent,
    previewWithdrawView,
    type Refusal,
    wipeLive,
    wipeSweepPending,
} from './previews.ts';
import {
    gatherPotSyncs,
    type OtherLenders,
    quoteWithdrawSol,
    type SearchProgress,
    type Simulate,
    type SyncPick,
} from './quote.ts';
import { postWipeSweepIxs, readSweepFacts } from './sweep.ts';

/** What every lender plan builder needs besides the chain read. */
export interface LendEnv {
    readonly rpc: AppRpc;
    readonly math: MathModule;
    readonly mint: Address;
    readonly signer: TransactionSigner;
    /** Display only (titles, units). */
    readonly symbol: string;
    /** The claim's check of its prepended syncs (tests inject one; default `simulatePlan`). */
    readonly simulate?: Simulate;
}

/** A fresh read of the market for the connected wallet (the review's Retry). */
export async function freshChain(env: LendEnv): Promise<{ chain: LoadedChain; nowS: number }> {
    const c = await readLendChain(env.rpc, env.mint, env.signer.address, { tokenAccounts: true });
    if (!c.market || !c.vault) throw new PlanError('This market is no longer readable.');
    return { chain: c as LoadedChain, nowS: chainNowSeconds(c, Date.now()) };
}

const refused = (r: Refusal) => new PlanError(r.message);

function display(env: LendEnv, chain: LoadedChain) {
    return { symbol: env.symbol, decimals: chain.market.decimals };
}

/**
 * The plan context, with `lender.subUnitConversion`'s variables when the plan's own conversion at
 * `now` takes the position to 0 shares while the pool's income vests (the owner's decision of
 * 2026-10-07: the review advises waiting for the vest's end).
 */
function contextAt(env: LendEnv, chain: LoadedChain, now: bigint) {
    const sub = previewSubUnitConversion(env.math, lendStateOf(chain), now);
    return {
        ...planContextFor(chain, env.signer, display(env, chain)),
        ...(sub ? { subUnit: subUnitVarsOf(sub) } : {}),
    };
}

function needRecords(chain: LoadedChain) {
    if (!chain.recordsComplete) {
        throw new PlanError('Your lender position must be synced first: close this review and use Sync position.');
    }
}

// ---- deposit ------------------------------------------------------------------------------

export interface DepositInputs {
    readonly source: Address;
    readonly amount: bigint;
    readonly toleranceBps: number;
    /** FE-1: the user opted into the CPI Guard bracket for this source. */
    readonly bracket: boolean;
}

export async function depositPlanFrom(
    env: LendEnv,
    chain: LoadedChain,
    nowS: number,
    inp: DepositInputs,
): Promise<TxPlan> {
    needRecords(chain);
    const market = chain.market;
    if (hasFlag(market, MARKET_FLAG.mintUnsafe)) throw new PlanError('Deposits are blocked on this market.');
    const source = chain.tokenAccounts.find((a) => a.address === inp.source);
    if (!source) throw new PlanError('The source token account no longer holds this token.');
    if (inp.amount > source.amount) throw new PlanError('The amount is above the balance of the source account.');
    const sweep = wipeSweepPending(market);
    const preview = previewDepositView(
        env.math,
        lendStateOf(chain),
        marketNow(market, nowS),
        inp.amount,
        inp.toleranceBps,
        sweep,
    );
    if (!preview.ok) throw refused(preview.refusal);
    const unpaid = previewUnpaidRent(env.math, market, marketNow(market, nowS), preview.value.shares, inp.amount);
    let sweepIxs: Instruction[] = [];
    if (sweep) {
        const facts = await readSweepFacts(env.rpc, market);
        const epoch = liveWipeRecordEpoch(market);
        const record = epoch === null ? null : (await findShareEpochPda({ market: chain.marketAddress, epoch }))[0];
        sweepIxs = await postWipeSweepIxs(env.signer, chain.marketAddress, market, facts, record);
    }
    const plan = await buildDepositPlan({
        ...contextAt(env, chain, marketNow(market, nowS)),
        source: source.address,
        sourceCpiGuard: source.cpiGuard,
        amount: inp.amount,
        minShares: preview.value.minShares,
        bracket: inp.bracket,
        sweep: sweepIxs,
        // AIR-08: no wipe record, so every lender left (the sweep books to the protocol).
        sweepAfterExit: sweep && !wipeLive(market),
        ...(unpaid.level === 'warn' ? { unpaidRent: unpaidRentVarsOf(unpaid) } : {}),
    });
    return {
        ...plan,
        rebuild: async () => {
            const f = await freshChain(env);
            return depositPlanFrom(env, f.chain, f.nowS, inp);
        },
    };
}

// ---- withdraw -----------------------------------------------------------------------------

export interface WithdrawInputs {
    readonly shares: bigint;
    readonly destination: Address;
    readonly toleranceBps: number;
}

export async function withdrawPlanFrom(
    env: LendEnv,
    chain: LoadedChain,
    nowS: number,
    inp: WithdrawInputs,
): Promise<TxPlan> {
    needRecords(chain);
    const view = previewWithdrawView(
        env.math,
        lendStateOf(chain),
        marketNow(chain.market, nowS),
        lockGraceOf(chain),
        inp.shares,
        inp.toleranceBps,
    );
    if (!view.ok) throw refused(view.refusal);
    // The destination's flags as the chain has them now: a memo-required one gets the disclosure
    // (`memo_program` goes to every Token-2022 destination, TMEMO-9).
    const info = chain.tokenAccounts.find((a) => a.address === inp.destination) ?? null;
    const create = !info && inp.destination === chain.ata;
    if (!info && !create) throw new PlanError('The destination token account no longer exists.');
    const base = {
        ...contextAt(env, chain, marketNow(chain.market, nowS)),
        destination: inp.destination,
        createDestination: create,
        memoRequired: info?.memoRequired ?? false,
        shares: inp.shares,
        minTokens: view.value.minTokens,
        vestingForfeit: view.value.vestingForfeit,
        vesting: view.value.vesting,
    };
    // The SOL part is quoted by simulating this very withdrawal with min_sol = 0 (vest included).
    const quote = await quoteWithdrawSol({
        rpc: env.rpc,
        plan: await buildWithdrawPlan({ ...base, minSol: 0n }),
        signer: env.signer,
        vault: chain.vaultAddress,
        solAtLeast: view.value.solAtLeast,
        vestOnLastExit: view.value.lastLenderExit ? view.value.unreleased : 0n,
        toleranceBps: inp.toleranceBps,
    });
    const minSol = quote.ok ? quote.lamports : view.value.solAtLeast;
    const plan = await buildWithdrawPlan({ ...base, minSol });
    return {
        ...plan,
        rebuild: async () => {
            const f = await freshChain(env);
            return withdrawPlanFrom(env, f.chain, f.nowS, inp);
        },
    };
}

// ---- claim, cover, sync -------------------------------------------------------------------

export type { OtherLenders } from './quote.ts';

/**
 * `LenderPosition`'s rent-exempt minimum, `(128 + 241)·3,480·2` = 2,568,240 lamports (the account's
 * size, ARCH §2.3.6, and the cluster's default rent), which `claim_sol` returns to the owner when it
 * closes the position (KI-67): what the review's `lender.claimCloses` names.
 */
export const LENDER_POSITION_RENT = (128n + BigInt(getLenderPositionSize())) * 3_480n * 2n;

/** No syncs: nothing to look for, no source, or a failed search. */
export const NO_SYNCS: SyncPick = { syncs: [], gain: 0n, more: { count: 0, gain: 0n } };

/**
 * The claim's search for other lenders whose pot-only sync funds the pot (FRONTEND.md §3.6):
 * `gatherPotSyncs` on the claim view's shortfall at `nowS` (0 under `CLAIM_DUST_LAMPORTS`), with
 * the inputs `claimPlanFrom` gives it. The review runs it, and — the owner's decision of 2026-10-11
 * ("only when syncs can pay") — so does the Claim button beforehand, in the background
 * (`claim/syncSearch.ts`), to say what the syncs can add. No shortfall, no source (no indexer), a
 * refused view or a failed read: no syncs ({@link NO_SYNCS}), as `gatherPotSyncs` treats a failure.
 */
export async function searchPotSyncs(a: {
    readonly rpc: AppRpc;
    readonly math: MathModule;
    readonly chain: LoadedChain;
    readonly claimant: Address;
    readonly nowS: number;
    readonly source: OtherLenders | null;
    /** Stops the search before its next indexer page (the Claim button's search dropped or timed out). */
    readonly signal?: AbortSignal;
    /** The pick so far and whether the search was cut short (`gatherPotSyncs`; a failure cuts it too). */
    readonly progress?: SearchProgress;
}): Promise<SyncPick> {
    if (!a.chain.lender || !a.source) return NO_SYNCS;
    try {
        const now = BigInt(a.nowS);
        const view = previewClaimView(a.math, lendStateOf(a.chain), now);
        if (!view.ok || view.value.shortfall === 0n) return NO_SYNCS;
        return await gatherPotSyncs({
            rpc: a.rpc,
            math: a.math,
            market: a.chain.marketAddress,
            base: { market: a.chain.market, vault: a.chain.vault },
            claimant: a.claimant,
            source: a.source,
            now,
            shortfall: view.value.shortfall,
            ...(a.signal ? { signal: a.signal } : {}),
            ...(a.progress ? { progress: a.progress } : {}),
        });
    } catch {
        // the indexer (or the reads) unavailable: no syncs; the claim pays what the pot holds
        if (a.progress) a.progress.cut = true;
        return NO_SYNCS;
    }
}

/**
 * The claim (FRONTEND.md §3.6; KI66-R4, KI-67). When the pot cannot pay the claimant's entitlement,
 * other lenders' unsigned `sync_lender` go first — the pot-only touch (`pool::touch_pot`), which
 * holds a lender behind a rebase record there and walks its forward chain — picked by
 * `gatherPotSyncs` from the indexer's lists (FE-30: the lenders behind the share epoch first,
 * then the current ones, 50 a page, ≤ 4 pages, until the best cover the shortfall; ≤ 4 synced,
 * largest gain first), or `pick`, the Claim button's own search when it is still fresh (the
 * owner's decision of 2026-10-11; `claim/syncSearch.ts`). They are fitted to the transaction: while the compiled transaction (no
 * lookup tables: an upper bound) is over 1,232 bytes, the smallest-gain sync is dropped, and the
 * expected payout is recomputed with the pot the rest moves. A plan with syncs is simulated here
 * once more: a sync that fails (its position closed or changed since the read) is dropped and the
 * plan rebuilt, so one stale target never blocks the claim; the review simulates the result again
 * before anything is signed, and does not offer Sign when that simulation pays 0 lamports
 * (`buildClaimPlan`'s `holdSign`). Syncs left out (by the cap or the size) are named in the review
 * (`lender.potSyncsDeferred`): the next claim picks them. Without the indexer, no syncs. Retry
 * (`rebuild`) searches again on a fresh read.
 */
export async function claimPlanFrom(
    env: LendEnv,
    chain: LoadedChain,
    nowS: number,
    otherLenders: OtherLenders | null,
    pick?: SyncPick,
): Promise<TxPlan> {
    needRecords(chain);
    if (!chain.lender) {
        throw new PlanError(
            'You have no lender position in this market (a claim or withdrawal that emptied it closed it).',
        );
    }
    const now = BigInt(nowS);
    const state = lendStateOf(chain);
    const view = previewClaimView(env.math, state, now);
    if (!view.ok) throw refused(view.refusal);
    const v = view.value;
    let syncs: SyncTarget[] = [];
    let more = { count: 0, gain: 0n };
    if (v.shortfall > 0n && otherLenders) {
        const found =
            pick ??
            (await searchPotSyncs({
                rpc: env.rpc,
                math: env.math,
                chain,
                claimant: env.signer.address,
                nowS,
                source: otherLenders,
            }));
        syncs = [...found.syncs];
        more = found.more;
    }
    const ctx = contextAt(env, chain, now);
    // Syncs the pre-review simulation failed on (their position closed or changed meanwhile):
    // dropped as invalid, not deferred — the next claim does not sync them first.
    const failed: SyncTarget[] = [];
    const make = async (kept: readonly SyncTarget[]): Promise<TxPlan> => {
        const gain = kept.reduce((x, s) => x + s.gain, 0n);
        const out = previewClaimOutcome(env.math, state, now, gain);
        // Left out by the cap or the size limit only (`lender.potSyncsDeferred`).
        const left = syncs.filter((s) => !kept.includes(s) && !failed.includes(s));
        const deferred = {
            count: more.count + left.length,
            gain: more.gain + left.reduce((x, s) => x + s.gain, 0n),
        };
        return buildClaimPlan({
            ...ctx,
            syncs: kept,
            expected: out.ok ? out.value.sol : v.claimable,
            ...(out.ok && out.value.closes ? { closes: { rent: LENDER_POSITION_RENT } } : {}),
            ...(deferred.count > 0 && v.shortfall > gain ? { deferred } : {}),
        });
    };
    let kept: SyncTarget[] = [...syncs];
    let plan = await make(kept);
    while (kept.length && wireSizeOf(plan.instructions, env.signer) > MAX_TRANSACTION_BYTES) {
        kept = kept.slice(0, -1);
        plan = await make(kept);
    }
    for (let tries = kept.length; tries > 0 && kept.length; tries--) {
        const at = await failingSync(env, plan, kept.length);
        if (at === null) break;
        failed.push(kept[at] as SyncTarget);
        kept = kept.filter((_, i) => i !== at);
        plan = await make(kept);
    }
    return {
        ...plan,
        rebuild: async () => {
            const f = await freshChain(env);
            return claimPlanFrom(env, f.chain, f.nowS, otherLenders);
        },
    };
}

/**
 * The index of the prepended sync a simulation of `plan` fails on (`null`: none failed, or the
 * simulation could not run — the review's own simulation then decides).
 */
async function failingSync(env: LendEnv, plan: TxPlan, count: number): Promise<number | null> {
    try {
        const { report } = await (env.simulate ?? simulatePlan)({
            rpc: env.rpc,
            plan,
            feePayer: env.signer,
            lookupTables: {},
            version: 0,
            priorityFeeMicroLamports: 0n,
        });
        const at = report.error?.instructionIndex;
        if (report.ok || at === undefined) return null;
        const i = at - LEADING_BUDGET_INSTRUCTIONS;
        return i >= 0 && i < count ? i : null;
    } catch {
        return null;
    }
}

export async function coverPlanFrom(env: LendEnv, chain: LoadedChain, nowS: number): Promise<TxPlan> {
    needRecords(chain);
    const view = previewClaimView(env.math, lendStateOf(chain), BigInt(nowS));
    if (!view.ok) throw refused(view.refusal);
    const plan = await buildCoverPlan({
        ...contextAt(env, chain, BigInt(nowS)),
        lamports: view.value.deficit,
    });
    return {
        ...plan,
        rebuild: async () => {
            const f = await freshChain(env);
            return coverPlanFrom(env, f.chain, f.nowS);
        },
    };
}

export async function syncPlanFrom(env: LendEnv, chain: LoadedChain): Promise<TxPlan> {
    const plan = await buildSyncPlan(contextAt(env, chain, BigInt(chainNowSeconds(chain, Date.now()))));
    return {
        ...plan,
        rebuild: async () => syncPlanFrom(env, (await freshChain(env)).chain),
    };
}
