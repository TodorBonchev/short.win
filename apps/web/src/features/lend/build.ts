/**
 * The lender plans built from one chain read (FRONTEND.md §3.6, §5.1): each form's Review and the
 * review's Retry (`TxPlan.rebuild`, M9-UX-1) run the same function — Review on the panel's current
 * read, Retry on a fresh one (`readLendChain`) with the same user inputs. So after a refusal a
 * fresh plan fixes (a moved quote under `min_shares` / `min_tokens` / `min_sol`, a step the plan
 * lacked — epoch records, the post-wipe sweep bundle, the memo program — or a changed account),
 * Retry re-reads the chain, re-quotes and re-derives the steps; inputs that no longer fit (a
 * balance gone, a source now behind CPI Guard) end in a `PlanError` the review shows.
 */
import { findShareEpochPda } from '@shortwin/client';
import type { Address, Instruction, TransactionSigner } from '@solana/kit';
import type { MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import type { TxPlan } from '../../tx/types.ts';
import { chainNowSeconds, liveWipeRecordEpoch, marketNow, readLendChain } from './chain.ts';
import { type LoadedChain, lendStateOf, lockGraceOf, planContextFor } from './ctx.ts';
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
    previewClaimView,
    previewClaimWithPot,
    previewDepositView,
    previewWithdrawView,
    type Refusal,
    wipeSweepPending,
} from './previews.ts';
import { pickSyncs, quoteWithdrawSol, readOtherLenders } from './quote.ts';
import { postWipeSweepIxs, readSweepFacts } from './sweep.ts';

/** What every lender plan builder needs besides the chain read. */
export interface LendEnv {
    readonly rpc: AppRpc;
    readonly math: MathModule;
    readonly mint: Address;
    readonly signer: TransactionSigner;
    /** Display only (titles, units). */
    readonly symbol: string;
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
    let sweepIxs: Instruction[] = [];
    if (sweep) {
        const facts = await readSweepFacts(env.rpc, market);
        const epoch = liveWipeRecordEpoch(market);
        const record = epoch === null ? null : (await findShareEpochPda({ market: chain.marketAddress, epoch }))[0];
        sweepIxs = await postWipeSweepIxs(env.signer, chain.marketAddress, market, facts, record);
    }
    const plan = await buildDepositPlan({
        ...planContextFor(chain, env.signer, display(env, chain)),
        source: source.address,
        sourceCpiGuard: source.cpiGuard,
        amount: inp.amount,
        minShares: preview.value.minShares,
        bracket: inp.bracket,
        sweep: sweepIxs,
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
        ...planContextFor(chain, env.signer, display(env, chain)),
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

/** Other lenders of the market (owners) a claim may sync first; the indexer lists them. */
export type OtherLenders = () => Promise<readonly Address[]>;

export async function claimPlanFrom(
    env: LendEnv,
    chain: LoadedChain,
    nowS: number,
    otherLenders: OtherLenders | null,
): Promise<TxPlan> {
    needRecords(chain);
    const now = BigInt(nowS);
    const view = previewClaimView(env.math, lendStateOf(chain), now);
    if (!view.ok) throw refused(view.refusal);
    const v = view.value;
    let syncs: SyncTarget[] = [];
    let expected = v.claimable;
    if (v.shortfall > 0n && otherLenders) {
        try {
            const owners = (await otherLenders()).filter((o) => o !== env.signer.address).slice(0, 24);
            const others = await readOtherLenders(env.rpc, chain.marketAddress, owners);
            const pick = pickSyncs(env.math, { market: chain.market, vault: chain.vault }, others, now, v.shortfall);
            syncs = [...pick.syncs];
            if (syncs.length) {
                const r = previewClaimWithPot(env.math, lendStateOf(chain), now, pick.gain);
                if (r.ok) expected = r.value;
            }
        } catch {
            // the indexer is unavailable: claim what the pot pays now
        }
    }
    const plan = await buildClaimPlan({
        ...planContextFor(chain, env.signer, display(env, chain)),
        syncs,
        expected,
    });
    return {
        ...plan,
        rebuild: async () => {
            const f = await freshChain(env);
            return claimPlanFrom(env, f.chain, f.nowS, otherLenders);
        },
    };
}

export async function coverPlanFrom(env: LendEnv, chain: LoadedChain, nowS: number): Promise<TxPlan> {
    needRecords(chain);
    const view = previewClaimView(env.math, lendStateOf(chain), BigInt(nowS));
    if (!view.ok) throw refused(view.refusal);
    const plan = await buildCoverPlan({
        ...planContextFor(chain, env.signer, display(env, chain)),
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
    const plan = await buildSyncPlan(planContextFor(chain, env.signer, display(env, chain)));
    return {
        ...plan,
        rebuild: async () => syncPlanFrom(env, (await freshChain(env)).chain),
    };
}
