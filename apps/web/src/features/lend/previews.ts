/**
 * Lender previews (FRONTEND.md §3.6, §3.8, §6.7): the accounts as the program stores them
 * (`Market`'s lender-pool group and flags, `MarketVault`, `LenderPosition`, `ShareEpoch`
 * records) → `@shortwin/math-wasm`'s `PoolState` / `LenderState` / `ShareEpochs` — exactly the
 * program's own `Market::pool()`, `MarketVault::vault()` and `LenderPosition::lender()` — and
 * every number the lend panel and the lender dashboard show, computed by the facade (the frontend
 * never re-implements pool math; FRONTEND.md §6.7 rule 1).
 *
 * Every function here is pure: it creates the WASM objects, reads what it needs and frees them
 * before returning plain `bigint`s. `previews.test.ts` holds the results to `pool_model.py`
 * v0.4.2 with the AI-review fixes (`vectors/lend_vectors.json`).
 *
 * `now` is the program's `market_now`: `max(clock, Market.last_accrual_ts)` for `deposit` /
 * `withdraw` (`engine::prologue::market_now`); `claim_sol`, `cover_deficit` and `sync_lender`
 * read the clock. The previews run on the last stored state: the program's prologue first
 * accrues rent and releases the vest due since `last_accrual_ts`, which only adds to a lender's
 * entitlement — so a SOL preview is a lower bound and the review's simulation shows the exact
 * amount (FRONTEND.md §5.3). The deposit's unpaid-rent figure is a risk, where a lower bound is the
 * wrong direction: it adds the rent the prologue credits first (`previewUnpaidRent`).
 */
import type { LenderPosition, Market, MarketVault, PodI128, PodU128, ShareEpoch } from '@shortwin/client';
import { podI128ToBigInt, podU128ToBigInt } from '@shortwin/client';
import { UNPAID_RENT_WARN_BPS, UNPAID_RENT_WARN_LAMPORTS } from '../../config/disclosures.ts';
import { PROGRAM_ERRORS, type ProgramErrorInfo } from '../../config/errors.ts';
import { WAD } from '../../lib/format.ts';
import type { MathModule } from '../../math/load.ts';

// ---- the account shapes the previews read ----------------------------------------------

/** `Market`'s lender-pool group (ARCH §2.3.4) and the fields that decide a lender flow. */
export type MarketPoolFields = Pick<
    Market,
    | 'tokensFree'
    | 'borrowedQ'
    | 'totalShares'
    | 'acc'
    | 'carry'
    | 'accR'
    | 'carryR'
    | 'dust'
    | 'receivable'
    | 'maxRelease'
    | 'vestBal'
    | 'vestLast'
    | 'vestEnd'
    | 'depositHoldUntil'
    | 'shareEpoch'
    | 'flags'
>;

/** `MarketVault`'s lender-pool part (ARCH §2.3.6). */
export type VaultPoolFields = Pick<
    MarketVault,
    'lenderSol' | 'unmatched' | 'protocolFees' | 'pi' | 'lockNorm' | 'bMin' | 'piEpoch' | 'calDay' | 'lockCal'
>;

/**
 * A `LenderPosition`'s math state (ARCH §2.3.6), its forward chain included (KI-66 full: `fwd`,
 * `fwdEpoch`, `fwdShares`, `fwdLag`). While a chain is active, `snap` / `snapR` are the chain's
 * snapshots in `fwdEpoch`, not the lender's own: every preview reads the four fields too.
 */
export type LenderFields = Pick<
    LenderPosition,
    | 'shares'
    | 'snap'
    | 'pending'
    | 'snapR'
    | 'real'
    | 'claimed'
    | 'epoch'
    | 'lock'
    | 'lockPi'
    | 'lockEpoch'
    | 'lockTerm'
    | 'lockUntil'
    | 'fwd'
    | 'fwdEpoch'
    | 'fwdShares'
    | 'fwdLag'
>;

/** `LenderPosition.fwd` (`shortwin_math::pool::FWD_*`; KI-66 full). */
export const FWD = { none: 0, hold: 1, open: 2 } as const;

/**
 * A `ShareEpoch` record (`@shortwin/client` helpers; ARCH §2.3.6, A39), its conversion remainder
 * `convRem` included (KI-65): a lender behind a rebase record converts to `⌊(convRem + s)/den⌋`
 * shares, so the previews use the value read. Another lender converting through the same record
 * first can move it (the quote by one share); the review's Retry re-reads the records.
 */
export type RecordFields = Pick<
    ShareEpoch,
    'epoch' | 'accEnd' | 'accREnd' | 'sOld' | 'carry' | 'carryR' | 'num' | 'denLog1000' | 'convRem'
>;

// ---- Market flags (programs/shortwin/src/state/market.rs MARKET_FLAG_*) -------------------

export const MARKET_FLAG = {
    pausedOpens: 1 << 0,
    mintUnsafe: 1 << 1,
    routeBroken: 1 << 2,
    wipeLive: 1 << 5,
    pausedCoverage: 1 << 6,
    wipeTraderSwept: 1 << 7,
    wipeRewardsClaimed: 1 << 8,
} as const;

/** `shortwin_math::pool::WIPE_TRADER_SWEPT` / `WIPE_REWARDS_CLAIMED`. */
const WIPE_TRADER_SWEPT = 1;
const WIPE_REWARDS_CLAIMED = 2;

/** `VenueKind` codes that pay venue cashback (`engine::lender::rewards_required`). */
const PUMP_VENUES = new Set([1, 2, 3]);

/** The default (all-zero) key: an unset `Pubkey` field of an account. */
const DEFAULT_ADDRESS = '11111111111111111111111111111111';

export function hasFlag(market: Pick<Market, 'flags'>, flag: number): boolean {
    return (market.flags & flag) !== 0;
}

/**
 * Whether a deposit after a wipe or dust flush also waits for the venue-reward sweep
 * (`engine::lender::rewards_required`; R4A-7): every pump-curve, migrating and PumpSwap market and,
 * since AIR-05, every market that has the trader's pump UVA (`Market.pump_uva` set) — a curve market
 * that left pump (→ `None`, then possibly a Raydium pool) keeps its curve-era cashback there.
 */
export function rewardsRequired(market: Pick<Market, 'venue' | 'pumpUva'>): boolean {
    return market.pumpUva !== DEFAULT_ADDRESS || PUMP_VENUES.has(market.venue);
}

/** A full-wipe record is live (`S = 0` and `wipe_live`): the next deposit finalises it. */
export function wipeLive(market: MarketPoolFields): boolean {
    return podU128ToBigInt(market.totalShares) === 0n && hasFlag(market, MARKET_FLAG.wipeLive);
}

/**
 * A deposit waits for permissionless sweeps when its trader bit, or a required venue-reward
 * bit, is clear. A full wipe, the last lender's exit and a dust flush clear these bits; a dust
 * flush can retain shares. A new market starts with both bits set and is not pending.
 */
export function wipeSweepPending(market: MarketPoolFields & Pick<Market, 'venue' | 'pumpUva'>): boolean {
    // First-use migration rechecks sources even if legacy dust-flush bits still say complete.
    if (market.depositHoldUntil === 0n) return true;
    if (!hasFlag(market, MARKET_FLAG.wipeTraderSwept)) return true;
    return rewardsRequired(market) && !hasFlag(market, MARKET_FLAG.wipeRewardsClaimed);
}

// ---- conversions (the program's `Market::pool`, `MarketVault::vault`, `LenderPosition::lender`) ----

type Pool = InstanceType<MathModule['PoolState']>;
type Lender = ReturnType<MathModule['LenderState']['newIn']>;
type Records = InstanceType<MathModule['ShareEpochs']>;

const u128 = (p: PodU128): bigint => podU128ToBigInt(p);
const i128 = (p: PodI128): bigint => podI128ToBigInt(p);

/** Overrides for a pool copy (e.g. the state after a bundled post-wipe sweep). */
export interface PoolOverrides {
    readonly wipeSwept?: number;
    readonly totalShares?: bigint;
    readonly tokensFree?: bigint;
}

export function toPoolState(
    m: MathModule,
    market: MarketPoolFields,
    vault: VaultPoolFields,
    o: PoolOverrides = {},
): Pool {
    const p = new m.PoolState();
    try {
        p.totalShares = o.totalShares ?? u128(market.totalShares);
        p.acc = i128(market.acc);
        p.carry = i128(market.carry);
        p.accR = u128(market.accR);
        p.carryR = u128(market.carryR);
        p.dust = i128(market.dust);
        p.tokensFree = o.tokensFree ?? market.tokensFree;
        p.borrowedQ = market.borrowedQ;
        p.receivable = market.receivable;
        p.maxRelease = market.maxRelease;
        p.vestBal = market.vestBal;
        p.vestLast = market.vestLast;
        p.vestEnd = market.vestEnd;
        p.shareEpoch = market.shareEpoch;
        p.wipeLive = hasFlag(market, MARKET_FLAG.wipeLive);
        p.wipeSwept =
            o.wipeSwept ??
            (hasFlag(market, MARKET_FLAG.wipeTraderSwept) ? WIPE_TRADER_SWEPT : 0) |
                (hasFlag(market, MARKET_FLAG.wipeRewardsClaimed) ? WIPE_REWARDS_CLAIMED : 0);
        p.lenderSol = vault.lenderSol;
        p.unmatched = vault.unmatched;
        p.protocolFees = vault.protocolFees;
        p.pi = u128(vault.pi);
        p.lockNorm = u128(vault.lockNorm);
        p.bMin = vault.bMin;
        p.piEpoch = vault.piEpoch;
        p.calDay = vault.calDay;
        vault.lockCal.forEach((term, slot) => {
            const v = u128(term);
            if (v !== 0n) p.setLockBucket(slot, v);
        });
        return p;
    } catch (e) {
        p.free();
        throw e;
    }
}

/** The lender's math state; a lender that never deposited starts as `pool::Lender::new`. */
export function toLenderState(m: MathModule, pool: Pool, lender: LenderFields | null): Lender {
    const l = m.LenderState.newIn(pool);
    if (!lender) return l;
    try {
        l.shares = lender.shares;
        l.snap = lender.snap;
        l.snapR = lender.snapR;
        l.lock = lender.lock;
        l.lockPi = lender.lockPi;
        l.lockTerm = lender.lockTerm;
        l.pending = lender.pending;
        l.real = lender.real;
        l.claimed = lender.claimed;
        l.epoch = lender.epoch;
        l.lockEpoch = lender.lockEpoch;
        l.lockUntil = lender.lockUntil;
        l.fwd = lender.fwd;
        l.fwdEpoch = lender.fwdEpoch;
        l.fwdShares = lender.fwdShares;
        l.fwdLag = lender.fwdLag;
        return l;
    } catch (e) {
        l.free();
        throw e;
    }
}

/** Consecutive records, in epoch order (extra records are ignored by the math). */
export function toShareEpochs(m: MathModule, records: readonly RecordFields[]): Records {
    const r = new m.ShareEpochs();
    try {
        for (const x of [...records].sort((a, b) => a.epoch - b.epoch)) {
            r.push(x.epoch, x.accEnd, x.accREnd, x.sOld, x.carry, x.carryR, x.num, x.denLog1000, x.convRem);
        }
        return r;
    } catch (e) {
        r.free();
        throw e;
    }
}

// ---- refusals ---------------------------------------------------------------------------

/** `MathError::as_str()` → the program error the program maps it to (`errors.rs` `From<MathError>`). */
const MATH_ERRORS: Readonly<Record<string, string>> = {
    overflow: 'MathOverflow',
    underflow: 'MathOverflow',
    'division by zero': 'MathOverflow',
    undefined: 'MathOverflow',
    'invariant violated': 'MathOverflow',
    'invalid input': 'InvalidParams',
    'negative pending': 'NegativePending',
    'insufficient shares': 'InsufficientShares',
    'insufficient free inventory': 'InsufficientFreeInventory',
    'above pro-rata allowance': 'AboveProRataAllowance',
    'lender behind share epochs': 'LenderBehindEpochs',
    'deposit too small': 'ZeroAmount',
    'post-wipe sweep pending': 'WipeSweepPending',
    // AIR-07's residual: no deposit while at most 1/1,000 of the token claims is unborrowed.
    'pool drained': 'PoolDrained',
};

const BY_NAME: ReadonlyMap<string, ProgramErrorInfo> = new Map(
    [...PROGRAM_ERRORS.values()].map((e) => [e.name, e] as const),
);

/** The program error a refusal stands for, with its message (`config/errors.ts`). */
export function programErrorByName(name: string): ProgramErrorInfo | undefined {
    return BY_NAME.get(name);
}

/** A preview the program would refuse: the program error name, its code and message. */
export interface Refusal {
    readonly name: string;
    readonly code: number | null;
    readonly message: string;
}

/** Maps a thrown facade error to the program error it becomes on chain. */
export function refusalOf(e: unknown): Refusal {
    const msg = e instanceof Error ? e.message : String(e);
    const name = MATH_ERRORS[msg] ?? 'MathOverflow';
    const info = programErrorByName(name);
    return { name, code: info?.code ?? null, message: info?.message ?? msg };
}

export function refusalNamed(name: string): Refusal {
    const info = programErrorByName(name);
    return { name, code: info?.code ?? null, message: info?.message ?? name };
}

export type Result<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly refusal: Refusal };

/** A refusal raised by the frontend's own copy of a program check (not by the facade). */
export class LendRefusal extends Error {
    readonly refusal: Refusal;
    constructor(refusal: Refusal) {
        super(refusal.message);
        this.name = 'LendRefusal';
        this.refusal = refusal;
    }
}

function attempt<T>(fn: () => T): Result<T> {
    try {
        return { ok: true, value: fn() };
    } catch (e) {
        return { ok: false, refusal: e instanceof LendRefusal ? e.refusal : refusalOf(e) };
    }
}

// ---- sessions: the WASM objects of one preview ------------------------------------------

/** The inputs every lender preview reads. */
export interface LendState {
    readonly market: MarketPoolFields & Pick<Market, 'venue' | 'pumpUva'>;
    readonly vault: VaultPoolFields;
    readonly lender: LenderFields | null;
    /** The `ShareEpoch` records from the lender's epoch on (empty for a current lender). */
    readonly records: readonly RecordFields[];
}

function session<T>(
    m: MathModule,
    s: LendState,
    fn: (pool: Pool, lender: Lender, records: Records) => T,
    o: PoolOverrides = {},
): T {
    const pool = toPoolState(m, s.market, s.vault, o);
    let lender: Lender | null = null;
    let records: Records | null = null;
    try {
        lender = toLenderState(m, pool, s.lender);
        records = toShareEpochs(m, s.records);
        return fn(pool, lender, records);
    } finally {
        records?.free();
        lender?.free();
        pool.free();
    }
}

/** The lender after the program's `touch` at `now` (no state is changed). */
export interface Touched {
    readonly shares: bigint;
    readonly pending: bigint;
    readonly real: bigint;
    readonly claimed: bigint;
    readonly lock: bigint;
    readonly epoch: number;
}

export function previewTouched(m: MathModule, s: LendState, now: bigint): Result<Touched> {
    return attempt(() =>
        session(m, s, (pool, lender, records) => {
            const t = m.previewTouch(pool, lender, records, now);
            try {
                return {
                    shares: t.shares,
                    pending: t.pending,
                    real: t.real,
                    claimed: t.claimed,
                    lock: t.lock,
                    epoch: t.epoch,
                };
            } finally {
                t.free();
            }
        }),
    );
}

/** Basis points of `part / whole` (floor); 0 when `whole` is 0. */
export function shareBps(part: bigint, whole: bigint): number {
    return whole === 0n ? 0 : Number((part * 10_000n) / whole);
}

/** `⌊x·(10⁴ − bps)/10⁴⌋`: a lower bound `bps` below the quote (FE-D6: `min_shares`, `min_tokens`). */
export function lessBps(x: bigint, bps: number): bigint {
    return (x * BigInt(10_000 - bps)) / 10_000n;
}

/** Where a lender stands against the market's share epoch (C0-6, V2-12). */
export type Standing =
    | { readonly kind: 'none' }
    | { readonly kind: 'current' }
    | { readonly kind: 'parked' }
    | { readonly kind: 'behind'; readonly epochs: number };

export function standingOf(market: MarketPoolFields, lender: LenderFields | null): Standing {
    if (!lender) return { kind: 'none' };
    if (lender.epoch >= market.shareEpoch) return { kind: 'current' };
    if (wipeLive(market) && lender.epoch + 1 === market.shareEpoch) return { kind: 'parked' };
    return { kind: 'behind', epochs: market.shareEpoch - lender.epoch };
}

// ---- a sub-unit conversion while income vests (KI-65; the owner's decision of 2026-10-07) ----

/** The lender's own next conversion can take its position to 0 shares while the pool's income vests. */
export interface SubUnitConversion {
    /** The position's shares before the conversion, in the units of its epoch. */
    readonly shares: bigint;
    /** The pool's unreleased vest at `now`, lamports. */
    readonly unreleased: bigint;
    /** `Market.vest_end`: when the whole vest is released, unix seconds. */
    readonly vestEnd: bigint;
}

/**
 * Whether the conversion every lender instruction of the owner runs first (`claim_sol`,
 * `deposit`, `withdraw`, `cover_deficit`, its signed `sync_lender`) can take a position holding
 * shares through rebase records only (`1/1,000^k`; a full wipe zeroes shares anyway) to 0 shares
 * while the pool still has income vesting at `now` (`lender.subUnitConversion`, FRONTEND.md
 * §3.6): the position is worth less than one new share, and a lender converting after it can
 * receive that share and, holding every share, the vest still unreleased at its exit (ECON §9.5,
 * KI-65 / KI65-REV-SEC1). `null` otherwise, also when the records at hand do not reach the
 * current epoch (the conversion would stop behind) or the facade refuses.
 *
 * Judged on the worst case, not on the records' `conv_rem` at review time: other lenders'
 * conversions through the same records can land before the transaction and move each remainder
 * (no instruction takes a minimum of shares), and with every remainder at 0 the conversion is
 * the floor `⌊s/1,000^(Σk)⌋` (KI-66 review). A position whose floor is 0 is flagged even when
 * the remainders now would give it a share.
 */
export function previewSubUnitConversion(m: MathModule, s: LendState, now: bigint): SubUnitConversion | null {
    const l = s.lender;
    if (!l || l.shares === 0n || standingOf(s.market, l).kind !== 'behind') return null;
    const passed = s.records.filter((r) => r.epoch >= l.epoch && r.epoch < s.market.shareEpoch);
    if (passed.length !== s.market.shareEpoch - l.epoch || passed.some((r) => r.num !== 1)) return null;
    const floor = passed.reduce((x, r) => x / 1_000n ** BigInt(r.denLog1000), l.shares);
    if (floor > 0n) return null;
    const unreleased = attempt(() =>
        session(m, s, (pool, lender, records) => {
            // The facade must accept the conversion with the records at hand.
            m.previewTouch(pool, lender, records, now).free();
            return pool.vestBal - m.previewVestRelease(pool, now);
        }),
    );
    if (!unreleased.ok || unreleased.value <= 0n) return null;
    return { shares: l.shares, unreleased: unreleased.value, vestEnd: s.market.vestEnd };
}

// ---- the lender's position (panel summary, dashboard row) ---------------------------------

export interface PositionView {
    /** Shares after the touch (converted to the current epoch); 0 while parked. */
    readonly shares: bigint;
    /** Share of the pool's `S`, bps. */
    readonly shareBps: number;
    /** Token value of the shares (`previewTokenValue`). */
    readonly value: bigint;
    /** SOL `claim_sol` pays now (`previewClaim`, pot draw included). */
    readonly claimable: bigint;
    /** Credited SOL entitlement after the touch (`pending`, may be negative). */
    readonly pending: bigint;
    /** The lender's pro-rata share of the unreleased vest (`vest_bal`), lamports. */
    readonly vesting: bigint;
    /** Locked shares now (`previewLock`) and when the lock ends (0 without a lock). */
    readonly lock: bigint;
    readonly lockUntil: bigint;
    /** Tokens withdrawable now (`previewAllowance`). */
    readonly allowance: bigint;
    readonly standing: Standing;
}

/** The pro-rata part of `lamports` that `shares` of `S` receive (vest display; ECON §9.4). */
export function proRata(lamports: bigint, shares: bigint, total: bigint): bigint {
    return total === 0n ? 0n : (lamports * shares) / total;
}

export function previewPosition(m: MathModule, s: LendState, now: bigint): Result<PositionView> {
    return attempt(() =>
        session(m, s, (pool, lender, records) => {
            const t = m.previewTouch(pool, lender, records, now);
            let touched: Touched;
            try {
                touched = {
                    shares: t.shares,
                    pending: t.pending,
                    real: t.real,
                    claimed: t.claimed,
                    lock: t.lock,
                    epoch: t.epoch,
                };
            } finally {
                t.free();
            }
            const current = touched.epoch === s.market.shareEpoch;
            const shares = current ? touched.shares : 0n;
            const total = pool.totalShares;
            const lock = m.previewLock(pool, lender, records, now);
            return {
                shares,
                shareBps: shareBps(shares, total),
                value: m.previewTokenValue(pool, lender, records, now),
                claimable: m.previewClaim(pool, lender, records, now),
                pending: touched.pending,
                vesting: proRata(pool.vestBal, shares, total),
                lock,
                lockUntil: lock > 0n && s.lender ? s.lender.lockUntil : 0n,
                allowance: current ? m.previewAllowance(pool, lender, records, now) : 0n,
                standing: standingOf(s.market, s.lender),
            };
        }),
    );
}

// ---- deposit ----------------------------------------------------------------------------

/**
 * A settlement reserves its remaining vest for the incumbent lenders. A nonnegative marker
 * holds deposits until the whole current vest releases, including legacy markets whose new
 * field is zero. Later credits may extend the schedule. -1 means initialized with no hold.
 */
export function previewDepositHold(m: MathModule, s: LendState, now: bigint): Result<bigint | null> {
    if (s.market.depositHoldUntil < 0n) return { ok: true, value: null };
    return attempt(() =>
        session(m, s, (pool) => (pool.vestBal > m.previewVestRelease(pool, now) ? s.market.vestEnd : null)),
    );
}

export interface DepositView {
    /** Shares `deposit(amount)` mints (`previewDeposit`). */
    readonly shares: bigint;
    /** `min_shares` = the quote less the tolerance (FE-D6). */
    readonly minShares: bigint;
    /** The lender's share of the pool after the deposit, bps. */
    readonly shareBps: number;
    /** Token value of the minted shares in the pool after the deposit. */
    readonly value: bigint;
    /** Token value of the lender's whole position after the deposit. */
    readonly positionValue: bigint;
}

/**
 * `deposit(amount)` on the stored state. `sweepBundled`: pending sweeps after a wipe, dust flush
 * or last lender's exit (`wipeSweepPending`) run first in
 * the same transaction (FRONTEND.md §3.6), so the preview reads the pool with both sweep bits set.
 */
export function previewDepositView(
    m: MathModule,
    s: LendState,
    now: bigint,
    amount: bigint,
    toleranceBps: number,
    sweepBundled = false,
): Result<DepositView> {
    const hold = previewDepositHold(m, s, now);
    if (!hold.ok) return hold;
    if (hold.value !== null)
        return {
            ok: false,
            refusal: {
                ...refusalNamed('PoolDrained'),
                message: 'Settlement income is still vesting. Deposits resume when it has fully vested.',
            },
        };
    const o: PoolOverrides = sweepBundled
        ? { wipeSwept: WIPE_TRADER_SWEPT | WIPE_REWARDS_CLAIMED }
        : s.market.depositHoldUntil === 0n
          ? { wipeSwept: 0 }
          : {};
    const minted = attempt(() =>
        session(
            m,
            s,
            (pool, lender, records) => m.previewDeposit(pool, lender, records, now, amount, rewardsRequired(s.market)),
            o,
        ),
    );
    if (!minted.ok) return minted;
    const shares = minted.value;
    const touched = previewTouched(m, s, now);
    const before =
        touched.ok && touched.value.epoch === s.market.shareEpoch && !wipeLive(s.market) ? touched.value.shares : 0n;
    const mine = before + shares;
    const total = u128(s.market.totalShares) + shares;
    return attempt(() =>
        session(
            m,
            s,
            (pool) => ({
                shares,
                minShares: lessBps(shares, toleranceBps),
                shareBps: shareBps(mine, total),
                value: m.tokensForShares(pool, shares),
                positionValue: m.tokensForShares(pool, mine),
            }),
            { ...o, totalShares: total, tokensFree: s.market.tokensFree + amount },
        ),
    );
}

// ---- the pool's unpaid rent at a deposit (ECON §9.4; the owner's decision of 2026-10-03) -----

/** How the deposit review shows the pool's unpaid rent: hidden, collapsed or as a warning. */
export type UnpaidRentLevel = 'none' | 'info' | 'warn';

/** The `Market` fields the unpaid-rent preview reads (the accrual's inputs and the pool's size). */
export type UnpaidRentMarket = Pick<
    Market,
    | 'receivable'
    | 'totalShares'
    | 'rentBasis'
    | 'rentAggr'
    | 'lenderCreditRem'
    | 'lastAccrualTs'
    | 'utilKinkBps'
    | 'tokensFree'
    | 'borrowedQ'
> &
    Partial<Pick<Market, 'emaPrice'>>;

export interface UnpaidRentView {
    /**
     * Lender rent credited (entitlement) and not yet paid in at the deposit, lamports: the stored
     * `Market.receivable` plus {@link UnpaidRentView.pending}.
     */
    readonly receivable: bigint;
    /** `Market.receivable` as stored (as of `last_accrual_ts`). */
    readonly stored: bigint;
    /**
     * The rent the deposit's own prologue credits first, over `now − last_accrual_ts`
     * ({@link pendingLenderCredit}).
     */
    readonly pending: bigint;
    /**
     * This deposit's pro-rata part of `receivable` should none of it be paid, at the pool's size
     * after the deposit: `⌈receivable·shares/(S + shares)⌉` lamports (rounded up). It grows if other
     * lenders withdraw first (a shortfall is debited to the shares holding when it happens).
     */
    readonly exposure: bigint;
    /** This deposit's shares' share of the pool after it, bps (floor). */
    readonly shareBps: number;
    /** The deposit's value at `P_ema`, `⌊amount·P_ema/WAD⌋` lamports; `null` without a price. */
    readonly value: bigint | null;
    readonly level: UnpaidRentLevel;
}

/** `⌊amount·price/WAD⌋` lamports (the value of raw tokens at a WAD price); `null` at price 0. */
export function lamportsAtPrice(amount: bigint, priceWad: bigint): bigint | null {
    return priceWad > 0n ? (amount * priceWad) / WAD : null;
}

/**
 * The lender rent the next instruction's prologue credits to `receivable` at `now` (the program's
 * `market_now`): `RentBasis.lenderCredit` — `rent::accrue` over `now − last_accrual_ts` at `P_ema`
 * and the stored utilisation, weighed by `Market.rent_aggr`, with the remainder the market carries
 * from its last accrual (`Market.lender_credit_rem`, AIR-03; `engine::prologue::accrue`). 0 with no
 * open position, no price or no time elapsed. A math error is the program's too (the deposit's
 * prologue would refuse it), so it reads as 0 here.
 */
export function pendingLenderCredit(m: MathModule, market: UnpaidRentMarket, now: bigint): bigint {
    const dt = now - market.lastAccrualTs;
    const a = market.rentAggr.map(u128);
    if (dt <= 0n || a.every((x) => x === 0n)) return 0n;
    const j = market.rentBasis.map(u128);
    const basis = new m.RentBasis(j[0] ?? 0n, j[1] ?? 0n, j[2] ?? 0n);
    try {
        const util = m.utilizationBps(market.tokensFree, market.borrowedQ);
        const pEma = market.emaPrice ? u128(market.emaPrice) : 0n;
        const rem = u128(market.lenderCreditRem);
        return basis.lenderCredit(a[0] ?? 0n, a[1] ?? 0n, a[2] ?? 0n, pEma, util, market.utilKinkBps, dt, rem);
    } catch {
        return 0n;
    } finally {
        basis.free();
    }
}

/**
 * A deposit's exposure to `receivable` lamports of unpaid rent (plain integers; also what the E2E
 * recomputes from the figure the panel shows): `shares` minted into a pool of `totalShares`,
 * `amount` raw tokens valued at `emaPrice` (WAD, 0 without a price).
 */
export function unpaidRentExposure(
    receivable: bigint,
    totalShares: bigint,
    shares: bigint,
    amount: bigint,
    emaPrice: bigint,
): Pick<UnpaidRentView, 'exposure' | 'shareBps' | 'value' | 'level'> {
    const total = totalShares + shares;
    const exposure = total === 0n || shares === 0n ? 0n : (receivable * shares + total - 1n) / total;
    const value = lamportsAtPrice(amount, emaPrice);
    const material =
        exposure > 0n &&
        (exposure >= UNPAID_RENT_WARN_LAMPORTS ||
            (value !== null && exposure * 10_000n >= BigInt(UNPAID_RENT_WARN_BPS) * value));
    return {
        exposure,
        shareBps: shareBps(shares, total),
        value,
        level: receivable === 0n ? 'none' : material ? 'warn' : 'info',
    };
}

/**
 * The pool's unpaid rent and a deposit's exposure to it (FRONTEND.md §3.6, `lender.unpaidRent`).
 * The receivable is the one the deposit meets: the stored `Market.receivable` plus the rent its own
 * prologue credits first ({@link pendingLenderCredit}; the stored value alone lags while nothing
 * touches the market, and reads 0 on a pool whose positions accrue). The split itself is plain
 * integer arithmetic: the program has no instruction that quotes it (a shortfall is the exit's
 * `Δ < 0` booked into `acc`, `pool::repay` / `pool::settle`). `now` is the program's `market_now`;
 * `shares` the deposit's minted shares (`previewDepositView`), `amount` its raw tokens.
 */
export function previewUnpaidRent(
    m: MathModule,
    market: UnpaidRentMarket,
    now: bigint,
    shares: bigint,
    amount: bigint,
): UnpaidRentView {
    const pending = pendingLenderCredit(m, market, now);
    const receivable = market.receivable + pending;
    const emaPrice = market.emaPrice ? u128(market.emaPrice) : 0n;
    return {
        receivable,
        stored: market.receivable,
        pending,
        ...unpaidRentExposure(receivable, u128(market.totalShares), shares, amount, emaPrice),
    };
}

// ---- withdraw ---------------------------------------------------------------------------

export interface WithdrawLimits {
    /** Tokens withdrawable now (`previewAllowance`). */
    readonly allowance: bigint;
    /** The most shares whose tokens fit the allowance (`sharesForTokens`, capped at the holding). */
    readonly maxShares: bigint;
    /** The holding after the touch. */
    readonly shares: bigint;
    /** Locked shares now (`previewLock`). */
    readonly lock: bigint;
    /** `lock_until` a growing lock gets from a withdrawal now (`previewLockUntil`). */
    readonly lockUntilIfLocked: bigint;
    /** The current lock's end (0 without a lock). */
    readonly lockUntil: bigint;
}

export function previewWithdrawLimits(
    m: MathModule,
    s: LendState,
    now: bigint,
    lockGraceS: number,
): Result<WithdrawLimits> {
    return attempt(() =>
        session(m, s, (pool, lender, records) => {
            const t = m.previewTouch(pool, lender, records, now);
            let shares: bigint;
            let current: boolean;
            try {
                current = t.epoch === s.market.shareEpoch;
                shares = current ? t.shares : 0n;
            } finally {
                t.free();
            }
            const allowance = current ? m.previewAllowance(pool, lender, records, now) : 0n;
            const fit = allowance > 0n ? m.sharesForTokens(pool, allowance) : 0n;
            const lock = m.previewLock(pool, lender, records, now);
            return {
                allowance,
                maxShares: fit < shares ? fit : shares,
                shares,
                lock,
                lockUntilIfLocked: m.previewLockUntil(pool, now, lockGraceS),
                lockUntil: lock > 0n && s.lender ? s.lender.lockUntil : 0n,
            };
        }),
    );
}

export interface WithdrawView {
    /** Tokens `withdraw(shares)` pays (`previewWithdrawal`). */
    readonly tokens: bigint;
    /** `min_tokens` = the quote less the tolerance (FE-D6). */
    readonly minTokens: bigint;
    /**
     * The SOL part `withdraw` pays on the stored state (`previewWithdrawal`: `claim` right after the
     * burn, as the handler; a last lender's vest flush included, FE-15): a lower bound — the
     * prologue's rent accrual only adds to it; the exact quote is the simulation's
     * (`quoteWithdrawSol`).
     */
    readonly solAtLeast: bigint;
    /** The withdrawal takes `S` to 0: the program pays this lender the unreleased vest first (R3B-2). */
    readonly lastLenderExit: boolean;
    /** Unreleased vest at `now` (`vest_bal` − the release due), lamports. */
    readonly unreleased: bigint;
    /** The lender's share of the unreleased vest (the `lender.exitForfeitsVest` `{vesting}`). */
    readonly vesting: bigint;
    /** The part of it the withdrawn shares give up to the lenders who stay (0 on the last exit). */
    readonly vestingForfeit: bigint;
    /**
     * The lender's locked shares right after the withdrawal (`previewWithdrawal`'s lender: the
     * borrowed part of the burn added to the live lock, capped at the shares left; FE-15). A
     * withdrawal does not move `Π`, so this is `lock_of` on the state afterwards.
     */
    readonly lockAfter: bigint;
    /** When that lock ends (`lock_until` afterwards); 0 when the withdrawal leaves no lock. */
    readonly lockUntilAfter: bigint;
}

export function previewWithdrawView(
    m: MathModule,
    s: LendState,
    now: bigint,
    lockGraceS: number,
    shares: bigint,
    toleranceBps: number,
): Result<WithdrawView> {
    return attempt(() =>
        session(m, s, (pool, lender, records) => {
            const w = m.previewWithdrawal(pool, lender, records, now, lockGraceS, shares);
            const { tokens, sol, lastSharesBorrowed } = w;
            const after = w.lender;
            const lockAfter = after.lock;
            const lockUntilAfter = lockAfter > 0n ? after.lockUntil : 0n;
            after.free();
            w.free();
            const total = pool.totalShares;
            const t = m.previewTouch(pool, lender, records, now);
            let mine: bigint;
            try {
                mine = t.epoch === s.market.shareEpoch ? t.shares : 0n;
            } finally {
                t.free();
            }
            const released = m.previewVestRelease(pool, now);
            const unreleased = pool.vestBal - released;
            // R3B-2: a burn of all of `S` is paid the unreleased vest. Exact since KI-65 (no rebase
            // orphans, ECON §9.5): `S` counts every share a lender holds or will receive at its
            // conversion, so this is the exit of the only position holding a share of the epoch.
            const last = shares === total;
            if (lastSharesBorrowed) {
                // The program's own check on top of the pool math (M1-SEC-3): never `S = 0` while tokens are lent.
                throw new LendRefusal(refusalNamed('LastSharesBorrowed'));
            }
            return {
                tokens,
                minTokens: lessBps(tokens, toleranceBps),
                solAtLeast: sol,
                lastLenderExit: last && pool.vestBal > 0n,
                unreleased,
                vesting: proRata(unreleased, mine, total),
                vestingForfeit: last ? 0n : proRata(unreleased, shares, total),
                lockAfter,
                lockUntilAfter,
            };
        }),
    );
}

// ---- claim ------------------------------------------------------------------------------

/**
 * A claim shortfall under this many lamports counts as none (the owner's decision of 2026-10-11,
 * thirteenth round; FRONTEND.md §3.6, §4.3): rounding dust (ECON §9.4 — the pro-rata indices round
 * down, so a few lamports of what is credited can stay out of reach of the SOL allocated to the
 * shares). Below it the Claim button reads "Review claim · {c} SOL" (disabled, "Nothing to claim
 * yet", when {c} is 0), the shortfall note is not shown and no other lender's sync is looked for.
 */
export const CLAIM_DUST_LAMPORTS = 10_000n;

/** The shortfall a claim acts on: `raw`, or 0 under {@link CLAIM_DUST_LAMPORTS}. */
export function claimShortfall(raw: bigint): bigint {
    return raw >= CLAIM_DUST_LAMPORTS ? raw : 0n;
}

export interface ClaimView {
    /** Lamports `claim_sol` pays now (`previewClaim`, pot draw included). */
    readonly claimable: bigint;
    /** Credited entitlement after the touch (`max(pending, 0)`). */
    readonly entitled: bigint;
    /** The lender's pro-rata share of the unreleased vest. */
    readonly vesting: bigint;
    /** The market's pot (`MarketVault.unmatched`). */
    readonly pot: bigint;
    /** A negative balance after a shortfall debit (`−pending`), else 0: `cover_deficit` it to withdraw. */
    readonly deficit: bigint;
    /**
     * Entitlement not yet realised that the pot cannot cover now (max(0, pending − own − pot)):
     * other lenders' touches may fund it. 0 under {@link CLAIM_DUST_LAMPORTS} (rounding dust).
     */
    readonly shortfall: bigint;
}

export function previewClaimView(m: MathModule, s: LendState, now: bigint): Result<ClaimView> {
    return attempt(() =>
        session(m, s, (pool, lender, records) => {
            const t = m.previewTouch(pool, lender, records, now);
            let pending: bigint;
            let own: bigint;
            let shares: bigint;
            try {
                pending = t.pending;
                own = t.real - t.claimed;
                shares = t.epoch === s.market.shareEpoch ? t.shares : 0n;
            } finally {
                t.free();
            }
            const need = pending - own > 0n ? pending - own : 0n;
            return {
                claimable: m.previewClaim(pool, lender, records, now),
                entitled: pending > 0n ? pending : 0n,
                vesting: proRata(pool.vestBal, shares, pool.totalShares),
                pot: pool.unmatched,
                deficit: pending < 0n ? -pending : 0n,
                shortfall: claimShortfall(need > pool.unmatched ? need - pool.unmatched : 0n),
            };
        }),
    );
}

/** Where another lender stands after a third party's `sync_lender` (`pool::touch_pot`). */
export type PotStanding = 'current' | 'parked' | 'behind';

/** What a third party's `sync_lender` (unsigned) does to another lender (KI-66 full). */
export interface PotTouchView {
    /** Lamports it moves to the pot (`MarketVault.unmatched` after − before; R3A-2). */
    readonly potGain: bigint;
    /** `behind` for a lender held at a rebase record or walking its forward chain. */
    readonly standing: PotStanding;
    /**
     * The lender afterwards, as the program stores it: its shares, lock and epoch unchanged behind a
     * rebase record; `pending`, `real`, the snapshots and the forward chain (`fwd*`) moved.
     */
    readonly lender: LenderFields;
}

/** The stored fields of a WASM `LenderState` (frees nothing). */
function fieldsOf(l: Lender): LenderFields {
    return {
        shares: l.shares,
        snap: l.snap,
        pending: l.pending,
        snapR: l.snapR,
        real: l.real,
        claimed: l.claimed,
        epoch: l.epoch,
        lock: l.lock,
        lockPi: l.lockPi,
        lockEpoch: l.lockEpoch,
        lockTerm: l.lockTerm,
        lockUntil: l.lockUntil,
        fwd: l.fwd,
        fwdEpoch: l.fwdEpoch,
        fwdShares: l.fwdShares,
        fwdLag: l.fwdLag,
    };
}

/**
 * `sync_lender` of another lender without its owner's signature, on copies (`pool::touch_pot`,
 * the WASM `previewTouchPot`; ECON §9.4 R3A-2, §9.5; FRONTEND.md §3.6): a lender behind a rebase
 * record is held there and its forward chain walked through `s.records` — from its `fwdEpoch` for a
 * chained lender, else from its `epoch` (`potEpochs`) — and the surplus moves to the pot; any other
 * lender is touched as by its owner. Nothing about who sends it matters to the program but the
 * signature, so this is what the claim's prepended syncs do.
 */
export function previewTouchPot(m: MathModule, s: LendState, now: bigint): Result<PotTouchView> {
    return attempt(() =>
        session(m, s, (pool, lender, records) => {
            const t = m.previewTouchPot(pool, lender, records, now);
            try {
                const l = t.lender;
                try {
                    return { potGain: t.potGain, standing: t.standing as PotStanding, lender: fieldsOf(l) };
                } finally {
                    l.free();
                }
            } finally {
                t.free();
            }
        }),
    );
}

/**
 * What another lender's `sync_lender` (sent by the claimant, unsigned) adds to the pot: its
 * `previewTouchPot` gain, 0 where the facade refuses (the program would refuse the sync too).
 */
export function potGainOfSync(m: MathModule, s: LendState, now: bigint): bigint {
    if (!s.lender) return 0n;
    const t = previewTouchPot(m, s, now);
    return t.ok ? t.value.potGain : 0n;
}

/** `previewClaim` with the pot raised by `extraPot` (other lenders synced first in the same transaction). */
export function previewClaimWithPot(m: MathModule, s: LendState, now: bigint, extraPot: bigint): Result<bigint> {
    return attempt(() =>
        session(m, { ...s, vault: { ...s.vault, unmatched: s.vault.unmatched + extraPot } }, (pool, lender, records) =>
            m.previewClaim(pool, lender, records, now),
        ),
    );
}

/** What `claim_sol` pays and whether it then closes the position (KI-67). */
export interface ClaimOutcomeView {
    readonly sol: bigint;
    /** The claim leaves the position closable (`pool::closable`): the program closes it and returns its rent. */
    readonly closes: boolean;
}

/**
 * `claim_sol` on copies (the WASM `previewClaimOutcome`), with the pot raised by `extraPot`: what it
 * pays and whether the position closes afterwards (KI-67: current, no shares, `pending` 0, no lock,
 * no forward chain; `claim_sol` then closes it and returns its rent to the owner).
 */
export function previewClaimOutcome(m: MathModule, s: LendState, now: bigint, extraPot = 0n): Result<ClaimOutcomeView> {
    return attempt(() =>
        session(
            m,
            { ...s, vault: { ...s.vault, unmatched: s.vault.unmatched + extraPot } },
            (pool, lender, records) => {
                const o = m.previewClaimOutcome(pool, lender, records, now);
                try {
                    const sol = o.sol;
                    const closes = o.closes;
                    return { sol, closes };
                } finally {
                    o.free();
                }
            },
        ),
    );
}

/** Whether the stored position is closable as it is (`pool::closable`, KI-67). */
export function closable(m: MathModule, s: Pick<LendState, 'market' | 'vault' | 'lender'>): boolean {
    if (!s.lender) return false;
    return session(m, { ...s, records: [] }, (pool, lender) => m.closable(pool, lender));
}
