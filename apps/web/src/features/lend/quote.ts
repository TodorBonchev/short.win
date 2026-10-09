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
 *   reaches the pot without their owners. The best ≤ 4 run before `claim_sol`. The candidates
 *   (`gatherPotSyncs`, FE-30): the indexer's lenders behind the share epoch first — where an
 *   absent surplus holder sits, and every lender with a forward chain except one whose owner's own
 *   conversion reached the share epoch inside it (that one is current) — then the current
 *   ones, a page at a time until the best ≤ 4 cover the shortfall or `MAX_SYNC_CANDIDATE_PAGES`
 *   pages are read.
 */
import { findLenderPda, type LenderPosition, type ShareEpoch } from '@shortwin/client';
import { type Address, parseBase64RpcAccount, type TransactionSigner } from '@solana/kit';
import type { MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { simulatePlan } from '../../tx/simulate.ts';
import type { TxError, TxPlan } from '../../tx/types.ts';
import { lenderPositionOf, potEpochs, readRecords } from './chain.ts';
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

/** One page of a market's lenders, as the indexer lists them (`/v1/markets/{mint}/lenders`). */
export interface LenderPage {
    /** Owners with the epoch the indexer holds for them (`LenderPosition.epoch`). */
    readonly lenders: readonly { readonly owner: Address; readonly epoch: number }[];
    /** The next page's cursor; `null` on the last page. */
    readonly next: string | null;
}

/**
 * Other lenders of the market a claim may sync first, a page at a time: `behind` — `epoch` before
 * the market's share epoch, so held at a rebase record (or parked on a live full-wipe record) and
 * possibly walking a forward chain — or `current`, by the indexer's own share epoch. Its order
 * within each (largest raw share count first) no longer ranks the syncs (the gains do), but it
 * still decides which lenders are read first and which fall inside `MAX_SYNC_CANDIDATE_PAGES`.
 */
export type OtherLenders = (q: {
    readonly epoch: 'behind' | 'current';
    readonly cursor: string | null;
    readonly limit: number;
}) => Promise<LenderPage>;

/**
 * Lenders per indexer page a claim reads (FE-30): one `getMultipleAccounts` (≤ 100) reads them; a
 * longer page (an indexer ignoring `limit`) is cut to it. The indexer must allow it
 * (`max_page_limit` ≥ 50; INDEXER §9.1).
 */
export const SYNC_CANDIDATE_PAGE = 50;

/**
 * The most indexer pages a claim reads (FE-30), both lists together: ≤ 200 candidates. Each page
 * costs three sequential reads (the indexer, the positions, the records not read yet) while the
 * user waits for the review, so 4 pages are about 12 round trips (an estimated 2–3 s on a public
 * RPC), and at most 4 syncs fit one claim anyway. The lenders behind the share epoch come first,
 * so the bound cuts into them only on a market with more than 200 of them; the behind list's
 * request counts even when it is empty, so a market with nobody behind is judged on at most 150
 * current lenders (FRONTEND §13, FE-30).
 */
export const MAX_SYNC_CANDIDATE_PAGES = 4;

/** `getMultipleAccounts`' account limit per call. */
const MAX_ACCOUNTS_PER_READ = 100;

/** Another lender of the market, as `readOtherLenders` reads it. */
export interface OtherLender {
    readonly address: Address;
    readonly lender: LenderPosition;
}

/**
 * The `ShareEpoch` records every candidate's pot-only sync needs (`potEpochs`) but `known`, read
 * over the union of their epochs (one `readRecords` per 100).
 */
export async function readPotRecords(
    rpc: AppRpc,
    market: Address,
    shareEpoch: number,
    others: readonly OtherLender[],
    known: ReadonlySet<number> = new Set(),
): Promise<ShareEpoch[]> {
    const epochs = [...new Set(others.flatMap((o) => potEpochs(o.lender, shareEpoch)))]
        .filter((e) => !known.has(e))
        .sort((a, b) => a - b);
    const out: ShareEpoch[] = [];
    for (let i = 0; i < epochs.length; i += MAX_ACCOUNTS_PER_READ) {
        out.push(...(await readRecords(rpc, market, epochs.slice(i, i + MAX_ACCOUNTS_PER_READ))));
    }
    return out;
}

/**
 * Judges other lenders' `sync_lender` (unsigned: `pool::touch_pot`): every lender — current, parked
 * or behind — with `previewTouchPot` on its state and the records of its `potEpochs` among
 * `records`; a lender whose records are not all at hand is judged on those that are (the program
 * walks as far as the records it is given reach: a lender more than 8 records behind advances 8 per
 * claim). Lenders that move nothing are left out.
 */
export function judgePotSyncs(
    m: MathModule,
    base: Omit<LendState, 'lender' | 'records'>,
    others: readonly OtherLender[],
    records: readonly RecordFields[],
    now: bigint,
): SyncTarget[] {
    const shareEpoch = base.market.shareEpoch;
    return others
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
        .filter((o) => o.gain > 0n);
}

/**
 * Picks among `judged` the syncs that fund the pot, largest gain first, until `shortfall` is
 * covered (≤ `MAX_SYNCS_PER_CLAIM`).
 */
export function choosePotSyncs(judged: readonly SyncTarget[], shortfall: bigint): SyncPick {
    const ranked = [...judged].sort((a, b) => (a.gain > b.gain ? -1 : a.gain < b.gain ? 1 : 0));
    const syncs: SyncTarget[] = [];
    let gain = 0n;
    let i = 0;
    for (; i < ranked.length; i++) {
        if (gain >= shortfall || syncs.length >= MAX_SYNCS_PER_CLAIM) break;
        const o = ranked[i] as SyncTarget;
        syncs.push(o);
        gain += o.gain;
    }
    const rest = gain < shortfall ? ranked.slice(i) : [];
    return { syncs, gain, more: { count: rest.length, gain: rest.reduce((x, o) => x + o.gain, 0n) } };
}

/**
 * Picks other lenders whose `sync_lender` funds the pot (`judgePotSyncs`, then `choosePotSyncs`):
 * largest gain first until `shortfall` is covered, ≤ `MAX_SYNCS_PER_CLAIM`. The claim pages
 * through `gatherPotSyncs`; this one judges a given list at once (the unit tests' entry).
 */
export function pickPotSyncs(
    m: MathModule,
    base: Omit<LendState, 'lender' | 'records'>,
    others: readonly OtherLender[],
    records: readonly RecordFields[],
    now: bigint,
    shortfall: bigint,
): SyncPick {
    return choosePotSyncs(judgePotSyncs(m, base, others, records, now), shortfall);
}

/**
 * The claim's pot-only syncs from the indexer's lenders (FE-30; FRONTEND.md §3.6): the lenders
 * behind the share epoch first, then the current ones, `SYNC_CANDIDATE_PAGE` at a time — each
 * page's positions read on chain (the claimant's own and any owner already read skipped), the
 * records they need and no earlier page read, every lender judged with `previewTouchPot` — until
 * the best ≤ 4 cover `shortfall`, both lists end, or `MAX_SYNC_CANDIDATE_PAGES` pages are read.
 * The two lists are the indexer's split, by its own share epoch, and every row not read yet is
 * read whatever its epoch: an indexer that has not ingested the latest rebase serves the newly
 * behind lenders in its current list, one ahead of the RPC serves lenders the chain read calls
 * current in its behind list, and an indexer without the `epoch` filter serves the whole list in
 * the first pass (the current pass then finds nothing new) — so a lag only reorders the reads,
 * and the judgement (on the chain's state) does not depend on the split. A failed read (the
 * indexer or the RPC down) ends the search with what the pages before it found: nothing when the
 * first fails, and the claim then pays what the pot holds.
 */
export async function gatherPotSyncs(a: {
    readonly rpc: AppRpc;
    readonly math: MathModule;
    readonly market: Address;
    readonly base: Omit<LendState, 'lender' | 'records'>;
    readonly claimant: Address;
    readonly source: OtherLenders;
    readonly now: bigint;
    readonly shortfall: bigint;
}): Promise<SyncPick> {
    const shareEpoch = a.base.market.shareEpoch;
    const seen = new Set<string>([a.claimant]);
    const records = new Map<number, ShareEpoch>();
    const judged: SyncTarget[] = [];
    let pick = choosePotSyncs([], a.shortfall);
    let pages = 0;
    for (const epoch of ['behind', 'current'] as const) {
        let cursor: string | null = null;
        do {
            if (pages >= MAX_SYNC_CANDIDATE_PAGES) return pick;
            try {
                const page: LenderPage = await a.source({ epoch, cursor, limit: SYNC_CANDIDATE_PAGE });
                pages++;
                cursor = page.next;
                // Each owner once, also when one page repeats it: a lender judged twice would get two
                // syncs and its gain counted twice.
                const owners: Address[] = [];
                for (const l of page.lenders.slice(0, SYNC_CANDIDATE_PAGE)) {
                    if (seen.has(l.owner)) continue;
                    seen.add(l.owner);
                    owners.push(l.owner);
                }
                if (!owners.length) continue;
                const others = await readOtherLenders(a.rpc, a.market, owners);
                const read = await readPotRecords(a.rpc, a.market, shareEpoch, others, new Set(records.keys()));
                for (const r of read) records.set(r.epoch, r);
                judged.push(...judgePotSyncs(a.math, a.base, others, [...records.values()], a.now));
                pick = choosePotSyncs(judged, a.shortfall);
                if (pick.gain >= a.shortfall) return pick;
            } catch {
                return pick;
            }
        } while (cursor !== null);
    }
    return pick;
}

/**
 * Reads the `LenderPosition`s of `owners` in `market` (other lenders, for `gatherPotSyncs`; ≤ 100
 * per `getMultipleAccounts`); owners without a position (never lent, or closed by `withdraw` /
 * `claim_sol`, KI-67) are dropped, and
 * so is any account at a lender PDA that is not one (`lenderPositionOf`: a system account someone
 * funded there), each on its own, so one such account costs only its own sync.
 */
export async function readOtherLenders(
    rpc: AppRpc,
    market: Address,
    owners: readonly Address[],
): Promise<OtherLender[]> {
    if (!owners.length) return [];
    const addresses = await Promise.all(owners.map(async (owner) => (await findLenderPda({ market, owner }))[0]));
    const out: OtherLender[] = [];
    for (let i = 0; i < addresses.length; i += MAX_ACCOUNTS_PER_READ) {
        const chunk = addresses.slice(i, i + MAX_ACCOUNTS_PER_READ);
        const { value } = await rpc.getMultipleAccounts(chunk, { encoding: 'base64', commitment: 'confirmed' }).send();
        chunk.forEach((address, k) => {
            const lender = lenderPositionOf(parseBase64RpcAccount(address, value[k] ?? null));
            if (lender) out.push({ address, lender });
        });
    }
    return out;
}
