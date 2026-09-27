//! Owner: pool track. Lender pool: shares, epochs, pro-rata withdrawal with expiring locks and the lock calendar, SOL entitlement, realised claims, the pot, lump vesting (ECON §9; ARCH §3.3; normative `sim/pool_model.py` v0.4.1).
//!
//! A port of `pool_model.Pool` v0.4.1 with its default switches (option (a) of the
//! round-3 brief, owner decision O16: continuous rent entitlement, a pot for realised
//! surplus, vested lumps), exact to the lamport and the share (differential vectors in
//! `tests/vectors/pool/`). The state is split the way the program stores it:
//!
//! - [`Pool`]: the lender-pool group of `Market` (ARCH §2.3.4), including the vest,
//!   `max_release` and the post-wipe flags (`wipe_live` is `Market.flags` bit 5, the two
//!   [`WIPE_TRADER_SWEPT`] / [`WIPE_REWARDS_CLAIMED`] bits are bits 7 and 8).
//! - [`Vault`]: the lender-pool part of `MarketVault` (ARCH §2.3.6): `lender_sol`, the pot
//!   `unmatched`, `protocol_fees`, the lock state (`Π`, `pi_epoch`, `lock_norm`, `b_min`)
//!   and the lock expiry calendar (`cal_day`, [`LOCK_BUCKETS`] day buckets).
//! - [`Lender`]: a `LenderPosition` (ARCH §2.3.6); [`ShareEpoch`]: one share-epoch record
//!   (ARCH §2.3.6, A39).
//!
//! `touch`, `claim` and `cover` take `&Pool`: they write only the lender and the vault,
//! so `claim_sol`, `cover_deficit` and `sync_lender` run `pool_model`'s full touch without
//! write-locking `Market` (C0-17, A70). Math types are not account layouts (ARCH §2.2):
//! the program converts to and from its `PodU128` layouts. Every struct here still has an
//! identical layout on every target (all 128-bit fields at 16-byte offsets, all padding
//! explicit, sizes and offsets asserted at compile time), so a byte copy never differs
//! between SBF (`align_of::<u128>() == 8`), x86-64 and wasm32 (16).
//!
//! The rent index, positions and the rent aggregate `A_j` belong to `rent`: `accrue`,
//! `repay` and `settle` take the lender credit and the per-position true-up (`credited`,
//! `rent::credited_for`) that `rent` computes. The utilisation cap is `caps::util_ok`'s.
//!
//! **Time.** Every function that releases the vest or reads a lock takes `now` (unix
//! seconds, `Clock::unix_timestamp`); `pool_model` keeps it as `Pool.now`.
//!
//! **Epoch records.** Functions that touch a lender take `records`: consecutive
//! [`ShareEpoch`] records (by `epoch`) covering the epochs the lender is behind,
//! `records[0]` at or before `lender.epoch`; extra records are ignored. None are needed
//! for a lender in the current epoch or one without shares, lock and lock term (it jumps,
//! V2-12); a lender whose shares and lock floored to 0 but whose term did not needs the
//! record of its epoch, which scales the term before the jump (as `pool_model`).
//! The program key-checks the accounts. `sync_lender(max_epochs)` is [`convert`] with the
//! records at hand, then [`touch`] once the lender is no longer [`Standing::Behind`].
//!
//! **Errors and atomicity.** Every mutating function either succeeds or leaves all its
//! arguments unchanged. A refusal by a pool rule has its own [`MathError`] variant, one per
//! program error of ARCH §3.11 (`pool_model`'s messages in brackets):
//! [`MathError::DepositTooSmall`] ("deposit too small"), [`MathError::WipeSweepPending`]
//! ("WipeSweepPending"), [`MathError::NegativePending`] ("negative SOL balance: cover it
//! first"), [`MathError::InsufficientShares`] ("not enough shares"),
//! [`MathError::InsufficientInventory`] ("insufficient unborrowed inventory"),
//! [`MathError::AboveAllowance`] ("above pro-rata share of unborrowed inventory") and
//! [`MathError::LenderBehind`] (share-epoch records missing). `InvalidInput` is left for
//! malformed arguments (records out of order, a lender ahead of the pool, a missing wipe
//! record, a lock day outside the calendar). `Overflow` / `Underflow` mean a result does
//! not fit the account field (ARCH §2.3.4, §2.3.6).
//!
//! Rounding follows ECON §2.1: shares minted, tokens paid, SOL credits, claims, vest
//! releases, lock terms and the aggregate `locked` down; a lender's own lock, `Π`, the
//! value-weighted vest end, `lock_until` and the rent true-up (`credited`, computed by the
//! caller) up.

use core::mem::{offset_of, size_of};

use crate::consts::MAX_RELEASE_SPAN_S;
use crate::error::MathError;
use crate::num::{BPS, Rounding, WAD, mul_div, mul_div_i128, to_i128, to_u64};

/// `VS`: virtual shares (decimals offset 3; ECON §9.2, §15).
pub const VIRTUAL_SHARES: u128 = 1_000;
/// `VA`: virtual token units (ECON §9.2, §15).
pub const VIRTUAL_ASSETS: u128 = 1;
/// Shares per raw token unit above which an epoch rolls with a rebase (ECON §9.5, C0-6).
pub const R_MAX: u128 = 1_000_000;
/// Rebase factor: a rebase divides shares by `REBASE^k` (ECON §9.5).
pub const REBASE: u128 = 1_000;
/// `lock_epoch` of a lender without a lock (`pool_model.Lender`: −1).
pub const NO_LOCK_EPOCH: u32 = u32::MAX;
/// The lock calendar's bucket length and ring size (ECON §9.3, §15; ARCH §2.3.6, A75),
/// defined with the other compile-time constants.
pub use crate::consts::{LOCK_BUCKET_S, LOCK_BUCKETS};
/// `Pool::wipe_swept` bit: `sweep_trader` ran since the last full wipe (`Market.flags`
/// bit 7 `wipe_trader_swept`; R4A-7).
pub const WIPE_TRADER_SWEPT: u8 = 1;
/// `Pool::wipe_swept` bit: `claim_venue_rewards` ran since the last full wipe
/// (`Market.flags` bit 8 `wipe_rewards_claimed`; R4A-7).
pub const WIPE_REWARDS_CLAIMED: u8 = 2;

const LOCK_BUCKETS_I64: i64 = LOCK_BUCKETS as i64;
/// Largest `den_log1000` whose `1,000^k` fits `u128`.
const MAX_DEN_LOG: u8 = 12;
const WAD_I: i128 = 1_000_000_000_000_000_000;

/// Market-level lender-pool state: the lender-pool group and the wipe flags of `Market`
/// (ARCH §2.3.4; `pool_model.Pool`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pool {
    /// `S`: shares outstanding in the current share epoch.
    pub total_shares: u128,
    /// `acc`: SOL entitlement per share (WAD lamports per share, signed; ECON §9.4).
    pub acc: i128,
    /// Undistributed remainder of `acc` credits, lamports·WAD, in `[0, S)`.
    pub carry: i128,
    /// `acc_r`: realised lender SOL allocated per share (WAD; V2-2).
    pub acc_r: u128,
    /// Undistributed remainder of `acc_r` allocations, lamports·WAD.
    pub carry_r: u128,
    /// Carries dropped at epoch rolls, lamports·WAD (stay in the vault, unowned).
    pub dust: i128,
    /// `T`: unborrowed tokens (internal counter; donations ignored).
    pub tokens_free: u64,
    /// `ΣB_Q`: tokens lent out.
    pub borrowed_q: u64,
    /// Rent credited to lenders but not yet paid in.
    pub receivable: u64,
    /// Latest `deadline + S3 tail` of any position opened so far (R4A-8, R4B-8).
    pub max_release: i64,
    /// Unreleased lump SOL (already in `lender_sol`; R3B-2).
    pub vest_bal: u64,
    /// Time of the last vest release.
    pub vest_last: i64,
    /// When the whole `vest_bal` is released (value-weighted, R4B-5).
    pub vest_end: i64,
    /// Current share epoch (C0-6).
    pub share_epoch: u32,
    /// Non-zero while the latest record is a full wipe still open for top-ups (V2-12).
    pub wipe_live: u8,
    /// Post-wipe sweeps done since the last full wipe: [`WIPE_TRADER_SWEPT`] |
    /// [`WIPE_REWARDS_CLAIMED`] (R4A-7). Read only while `wipe_live`.
    pub wipe_swept: u8,
    /// Explicit padding (zero).
    pub _pad: [u8; 2],
}

/// The lender-pool part of `MarketVault` (ARCH §2.3.6; `pool_model.Pool`'s `sol_vault`,
/// `unmatched`, `protocol`, lock state and `lock_cal`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Vault {
    /// Lock expiry calendar: bucket `d mod LOCK_BUCKETS` holds the lock terms whose
    /// `lock_until` ends day `d`, for the days `cal_day .. cal_day + LOCK_BUCKETS`.
    pub lock_cal: [u128; LOCK_BUCKETS],
    /// `Π`: borrowed-inventory survival index (WAD; ECON §9.3, C0-9).
    pub pi: u128,
    /// Σ of the live calendar buckets (the aggregate lock term).
    pub lock_norm: u128,
    /// `lender_sol` (`pool_model.sol_vault`): realised lender SOL, including the pot and
    /// the unreleased vest.
    pub lender_sol: u64,
    /// The pot: realised SOL beyond its receivers' entitlement (R3A-2).
    pub unmatched: u64,
    /// Protocol fees. The pool adds lender credits that find no lender (`S = 0`, no live
    /// wipe record, R3A-10); every other protocol income is added by the program.
    pub protocol_fees: u64,
    /// Running minimum of `ΣB_Q` since the first lock of the `Π` epoch (V2-3).
    pub b_min: u64,
    /// Oldest calendar day not yet dropped.
    pub cal_day: i64,
    /// `Π` epoch: every lock and bucket clears when it rolls.
    pub pi_epoch: u32,
    /// Explicit padding (zero).
    pub _pad: [u8; 4],
}

/// Lender state (`LenderPosition`, ARCH §2.3.6; `pool_model.Lender`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Lender {
    /// Shares, in units of `epoch`.
    pub shares: u128,
    /// `acc` at the last touch.
    pub snap: i128,
    /// `acc_r` at the last touch.
    pub snap_r: u128,
    /// Locked shares at `lock_pi` (ECON §9.3).
    pub lock: u128,
    /// `Π` when the lock was set.
    pub lock_pi: u128,
    /// This lender's term in the calendar bucket of `lock_until`: `⌊lock·WAD/lock_pi⌋`.
    pub lock_term: u128,
    /// SOL entitlement not yet paid (negative after a shortfall debit; ECON §9.4).
    pub pending: i64,
    /// Cumulative realised SOL allocated to this lender, including pot draws (V2-2).
    pub real: u64,
    /// Cumulative SOL paid to this lender (V2-2).
    pub claimed: u64,
    /// When the lock expires (a calendar day end; R3B-1, R4A-2).
    pub lock_until: i64,
    /// Share epoch of `shares`.
    pub epoch: u32,
    /// `Π` epoch of `lock` ([`NO_LOCK_EPOCH`] without a lock).
    pub lock_epoch: u32,
    /// Explicit padding (zero).
    pub _pad: [u8; 8],
}

/// Share-epoch record (`ShareEpoch`, ARCH §2.3.6, A39; `pool_model.Pool.epochs[e]`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShareEpoch {
    /// `acc` at the roll (a live full-wipe record is topped up, V2-12).
    pub acc_end: i128,
    /// `acc_r` at the roll.
    pub acc_r_end: u128,
    /// `S` at the roll (> 0).
    pub s_old: u128,
    /// Remainder of `acc_end` top-ups, lamports·WAD.
    pub carry: i128,
    /// Remainder of `acc_r_end` top-ups, lamports·WAD.
    pub carry_r: u128,
    /// The epoch this record closes.
    pub epoch: u32,
    /// Conversion numerator: 0 (full wipe) or 1.
    pub num: u8,
    /// `k` of the conversion denominator `1,000^k` (0 for a full wipe).
    pub den_log1000: u8,
    /// Explicit padding (zero).
    pub _pad: [u8; 10],
}

// Layouts (see the module docs): every 128-bit field at a multiple of 16, all padding
// explicit, sizes multiples of 16, so the layout is the same whether `u128` is 8- or
// 16-aligned.
const _: () = {
    assert!(size_of::<Pool>() == 160);
    assert!(offset_of!(Pool, total_shares) == 0);
    assert!(offset_of!(Pool, acc) == 16);
    assert!(offset_of!(Pool, carry) == 32);
    assert!(offset_of!(Pool, acc_r) == 48);
    assert!(offset_of!(Pool, carry_r) == 64);
    assert!(offset_of!(Pool, dust) == 80);
    assert!(offset_of!(Pool, tokens_free) == 96);
    assert!(offset_of!(Pool, borrowed_q) == 104);
    assert!(offset_of!(Pool, receivable) == 112);
    assert!(offset_of!(Pool, max_release) == 120);
    assert!(offset_of!(Pool, vest_bal) == 128);
    assert!(offset_of!(Pool, vest_last) == 136);
    assert!(offset_of!(Pool, vest_end) == 144);
    assert!(offset_of!(Pool, share_epoch) == 152);
    assert!(offset_of!(Pool, wipe_live) == 156);
    assert!(offset_of!(Pool, wipe_swept) == 157);
    assert!(offset_of!(Pool, _pad) == 158);

    assert!(size_of::<Vault>() == 2_128);
    assert!(offset_of!(Vault, lock_cal) == 0);
    assert!(offset_of!(Vault, pi) == 2_048);
    assert!(offset_of!(Vault, lock_norm) == 2_064);
    assert!(offset_of!(Vault, lender_sol) == 2_080);
    assert!(offset_of!(Vault, unmatched) == 2_088);
    assert!(offset_of!(Vault, protocol_fees) == 2_096);
    assert!(offset_of!(Vault, b_min) == 2_104);
    assert!(offset_of!(Vault, cal_day) == 2_112);
    assert!(offset_of!(Vault, pi_epoch) == 2_120);
    assert!(offset_of!(Vault, _pad) == 2_124);

    assert!(size_of::<Lender>() == 144);
    assert!(offset_of!(Lender, shares) == 0);
    assert!(offset_of!(Lender, snap) == 16);
    assert!(offset_of!(Lender, snap_r) == 32);
    assert!(offset_of!(Lender, lock) == 48);
    assert!(offset_of!(Lender, lock_pi) == 64);
    assert!(offset_of!(Lender, lock_term) == 80);
    assert!(offset_of!(Lender, pending) == 96);
    assert!(offset_of!(Lender, real) == 104);
    assert!(offset_of!(Lender, claimed) == 112);
    assert!(offset_of!(Lender, lock_until) == 120);
    assert!(offset_of!(Lender, epoch) == 128);
    assert!(offset_of!(Lender, lock_epoch) == 132);
    assert!(offset_of!(Lender, _pad) == 136);

    assert!(size_of::<ShareEpoch>() == 96);
    assert!(offset_of!(ShareEpoch, acc_end) == 0);
    assert!(offset_of!(ShareEpoch, acc_r_end) == 16);
    assert!(offset_of!(ShareEpoch, s_old) == 32);
    assert!(offset_of!(ShareEpoch, carry) == 48);
    assert!(offset_of!(ShareEpoch, carry_r) == 64);
    assert!(offset_of!(ShareEpoch, epoch) == 80);
    assert!(offset_of!(ShareEpoch, num) == 84);
    assert!(offset_of!(ShareEpoch, den_log1000) == 85);
    assert!(offset_of!(ShareEpoch, _pad) == 86);
};

/// Where a lender stands after [`convert`] / [`touch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Standing {
    /// In the current share epoch (and, after `touch`, accrued).
    Current,
    /// On the live full-wipe record (V2-12): no current shares, so `withdraw` refuses;
    /// `pending` and `real` follow the record's top-ups.
    Parked,
    /// More [`ShareEpoch`] records are needed (`sync_lender` in steps, ARCH §3.3).
    Behind,
}

/// Which post-wipe sweep a [`sweep`] is (ARCH §3.3, §3.9; R4A-7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SweepSource {
    /// `sweep_trader`: SOL left on the trader PDA.
    Trader,
    /// `claim_venue_rewards`: venue cashback.
    VenueRewards,
}

impl SweepSource {
    const fn bit(self) -> u8 {
        match self {
            Self::Trader => WIPE_TRADER_SWEPT,
            Self::VenueRewards => WIPE_REWARDS_CLAIMED,
        }
    }
}

/// A position's resolution as the pool sees it: `repay` (close, liquidation, expiry) or
/// `settle` (S1/S2/S3) of `q` borrowed tokens (ARCH §3.3 "borrower-side pool calls").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Resolution {
    /// Tokens the position borrowed.
    pub q: u64,
    /// Lender rent already credited continuously on the position's behalf
    /// (`rent::credited_for`, ceil; `pool_model._credited_for`).
    pub credited: u64,
    /// Lender lump that vests: the lender part of the penalty or the expiry fee
    /// (`repay`); the part of an S2/S3 take above `⌊Q·P_ema/WAD⌋ + R_due` (`settle`,
    /// `waterfall::settle`; 0 for S1).
    pub lump: u64,
}

impl Pool {
    /// Pool of a new market: no shares (`pool_model.Pool()`).
    pub const fn new() -> Self {
        Self {
            total_shares: 0,
            acc: 0,
            carry: 0,
            acc_r: 0,
            carry_r: 0,
            dust: 0,
            tokens_free: 0,
            borrowed_q: 0,
            receivable: 0,
            max_release: 0,
            vest_bal: 0,
            vest_last: 0,
            vest_end: 0,
            share_epoch: 0,
            wipe_live: 0,
            wipe_swept: WIPE_TRADER_SWEPT | WIPE_REWARDS_CLAIMED,
            _pad: [0; 2],
        }
    }

    /// Whether the latest record is a full wipe still open for top-ups (V2-12).
    pub const fn is_wipe_live(&self) -> bool {
        self.wipe_live != 0
    }

    /// `T + ΣB_Q`: all lender token claims.
    pub fn token_claims(&self) -> Result<u128, MathError> {
        add_u128(u128::from(self.tokens_free), u128::from(self.borrowed_q))
    }
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}

impl Vault {
    /// The vault of a new market: `Π = WAD`, empty calendar (`pool_model.Pool()`).
    pub const fn new() -> Self {
        Self {
            lock_cal: [0; LOCK_BUCKETS],
            pi: WAD,
            lock_norm: 0,
            lender_sol: 0,
            unmatched: 0,
            protocol_fees: 0,
            b_min: 0,
            cal_day: 0,
            pi_epoch: 0,
            _pad: [0; 4],
        }
    }

    /// The live calendar buckets `(day, Σ lock terms)` in day order, non-zero only
    /// (`pool_model.Pool.lock_cal`).
    pub fn buckets(&self) -> impl Iterator<Item = (i64, u128)> + '_ {
        (0..LOCK_BUCKETS_I64).filter_map(move |i| {
            let day = self.cal_day.checked_add(i)?;
            let v = *self.lock_cal.get(slot(day).ok()?)?;
            (v != 0).then_some((day, v))
        })
    }
}

impl Default for Vault {
    fn default() -> Self {
        Self::new()
    }
}

impl Lender {
    /// A new lender at the pool's current state (`pool_model.Lender(acc, acc_r, epoch)`).
    pub const fn new(pool: &Pool) -> Self {
        Self {
            shares: 0,
            snap: pool.acc,
            snap_r: pool.acc_r,
            lock: 0,
            lock_pi: WAD,
            lock_term: 0,
            pending: 0,
            real: 0,
            claimed: 0,
            lock_until: 0,
            epoch: pool.share_epoch,
            lock_epoch: NO_LOCK_EPOCH,
            _pad: [0; 8],
        }
    }
}

impl ShareEpoch {
    /// `true` for a full wipe (`num = 0`: the old shares keep only their SOL).
    pub const fn is_full_wipe(&self) -> bool {
        self.num == 0
    }

    /// Conversion denominator `1,000^den_log1000`.
    pub fn den(&self) -> Result<u128, MathError> {
        den_of(self.den_log1000)
    }

    /// `(num, den)`, validated: `num ∈ {0, 1}`, `den = 1` for a full wipe.
    fn ratio(&self) -> Result<(u128, u128), MathError> {
        match (self.num, self.den_log1000) {
            (0, 0) => Ok((0, 1)),
            (1, k) => Ok((1, den_of(k)?)),
            _ => Err(MathError::InvalidInput),
        }
    }
}

// ---- helpers -----------------------------------------------------------------------------

fn ovf<T>(x: Option<T>) -> Result<T, MathError> {
    x.ok_or(MathError::Overflow)
}

fn add_u128(a: u128, b: u128) -> Result<u128, MathError> {
    ovf(a.checked_add(b))
}

fn sub_u128(a: u128, b: u128) -> Result<u128, MathError> {
    a.checked_sub(b).ok_or(MathError::Underflow)
}

fn add_u64(a: u64, b: u64) -> Result<u64, MathError> {
    ovf(a.checked_add(b))
}

fn sub_u64(a: u64, b: u64) -> Result<u64, MathError> {
    a.checked_sub(b).ok_or(MathError::Underflow)
}

fn add_i128(a: i128, b: i128) -> Result<i128, MathError> {
    ovf(a.checked_add(b))
}

fn sub_i128(a: i128, b: i128) -> Result<i128, MathError> {
    ovf(a.checked_sub(b))
}

fn add_i64(a: i64, b: i64) -> Result<i64, MathError> {
    ovf(a.checked_add(b))
}

fn den_of(k: u8) -> Result<u128, MathError> {
    if k > MAX_DEN_LOG {
        return Err(MathError::Overflow);
    }
    ovf(REBASE.checked_pow(u32::from(k)))
}

/// A signed value into an unsigned field: `Underflow` below 0, `Overflow` above the max.
fn to_unsigned<T: TryFrom<i128>>(x: i128) -> Result<T, MathError> {
    if x < 0 {
        return Err(MathError::Underflow);
    }
    T::try_from(x).map_err(|_| MathError::Overflow)
}

fn to_i64(x: i128) -> Result<i64, MathError> {
    i64::try_from(x).map_err(|_| MathError::Overflow)
}

/// `a − b` of two unsigned accumulators, signed.
fn diff_u128(a: u128, b: u128) -> Result<i128, MathError> {
    if a >= b {
        to_i128(sub_u128(a, b)?)
    } else {
        ovf(to_i128(sub_u128(b, a)?)?.checked_neg())
    }
}

/// `⌊shares·Δacc/WAD⌋` (toward −∞, like the model's `//`).
fn accrued(shares: u128, delta: i128) -> Result<i128, MathError> {
    mul_div_i128(delta, shares, WAD, Rounding::Floor)
}

/// Python's `divmod(num, d)` for `d > 0`: quotient toward −∞, remainder in `[0, d)`.
fn divmod_floor(num: i128, d: u128) -> Result<(i128, u128), MathError> {
    if d == 0 {
        return Err(MathError::DivByZero);
    }
    match i128::try_from(d) {
        Ok(di) => {
            let q = ovf(num.checked_div_euclid(di))?;
            let r = ovf(num.checked_rem_euclid(di))?;
            Ok((q, to_unsigned(r)?))
        }
        // d > i128::MAX ≥ |num| (except i128::MIN, whose magnitude is 2^127 < d).
        Err(_) if num >= 0 => Ok((0, num.unsigned_abs())),
        Err(_) => Ok((-1, sub_u128(d, num.unsigned_abs())?)),
    }
}

/// `⌈a/b⌉` for `b > 0` (Python's `cdiv`).
fn ceil_div_i64(a: i64, b: i64) -> Result<i64, MathError> {
    let q = ovf(a.checked_div_euclid(b))?;
    let r = ovf(a.checked_rem_euclid(b))?;
    if r == 0 { Ok(q) } else { add_i64(q, 1) }
}

/// Calendar day of a timestamp: `⌊t/LOCK_BUCKET_S⌋`.
fn day_of(t: i64) -> Result<i64, MathError> {
    ovf(t.checked_div_euclid(LOCK_BUCKET_S))
}

/// Ring slot of a calendar day: `day mod LOCK_BUCKETS`.
fn slot(day: i64) -> Result<usize, MathError> {
    let s = ovf(day.checked_rem_euclid(LOCK_BUCKETS_I64))?;
    usize::try_from(s).map_err(|_| MathError::Overflow)
}

/// `T + ΣB_Q + VA`.
fn claims_va(p: &Pool) -> Result<u128, MathError> {
    add_u128(p.token_claims()?, VIRTUAL_ASSETS)
}

/// `S + VS`.
fn shares_vs(p: &Pool) -> Result<u128, MathError> {
    add_u128(p.total_shares, VIRTUAL_SHARES)
}

/// The record for `epoch` in a consecutive slice, `None` if the slice does not reach it.
fn record_for(records: &[ShareEpoch], epoch: u32) -> Result<Option<&ShareEpoch>, MathError> {
    let Some(first) = records.first() else {
        return Ok(None);
    };
    let Some(idx) = epoch.checked_sub(first.epoch) else {
        return Ok(None);
    };
    let idx = usize::try_from(idx).map_err(|_| MathError::Overflow)?;
    match records.get(idx) {
        None => Ok(None),
        Some(r) if r.epoch == epoch => Ok(Some(r)),
        Some(_) => Err(MathError::InvalidInput),
    }
}

// ---- lock expiry calendar (ECON §9.3; ARCH §2.3.6, A75; R4A-2, R4B-3) ---------------------

/// `pool_model.Pool._cal_advance`: drops every bucket whose day has ended (`day ≤
/// ⌊now/LOCK_BUCKET_S⌋`) from `lock_norm`, so an expired lock leaves the aggregate at its
/// `lock_until` without its holder being touched; then `cal_day = ⌊now/LOCK_BUCKET_S⌋ + 1`.
/// At most [`LOCK_BUCKETS`] buckets are visited (a longer gap clears the ring). The
/// prologue of every lock read or write, including [`resolve`] and [`touch`].
///
/// # Errors
/// `Overflow` only for timestamps near `i64::MAX`.
pub fn cal_advance(vault: &mut Vault, now: i64) -> Result<(), MathError> {
    let today = day_of(now)?;
    if today < vault.cal_day {
        return Ok(());
    }
    let next = add_i64(today, 1)?;
    let span = ovf(next.checked_sub(vault.cal_day))?;
    if span >= LOCK_BUCKETS_I64 {
        for b in &mut vault.lock_cal {
            vault.lock_norm = vault.lock_norm.saturating_sub(*b);
            *b = 0;
        }
    } else {
        let mut d = vault.cal_day;
        while d < next {
            let b = vault
                .lock_cal
                .get_mut(slot(d)?)
                .ok_or(MathError::Invariant)?;
            vault.lock_norm = vault.lock_norm.saturating_sub(*b);
            *b = 0;
            d = add_i64(d, 1)?;
        }
    }
    vault.cal_day = next;
    Ok(())
}

/// `lock_norm` as [`cal_advance`] at `now` would leave it, without changing the vault.
fn norm_at(vault: &Vault, now: i64) -> Result<u128, MathError> {
    let today = day_of(now)?;
    if today < vault.cal_day {
        return Ok(vault.lock_norm);
    }
    let next = add_i64(today, 1)?;
    let span = ovf(next.checked_sub(vault.cal_day))?;
    let mut norm = vault.lock_norm;
    if span >= LOCK_BUCKETS_I64 {
        for b in &vault.lock_cal {
            norm = norm.saturating_sub(*b);
        }
    } else {
        let mut d = vault.cal_day;
        while d < next {
            norm = norm.saturating_sub(*vault.lock_cal.get(slot(d)?).ok_or(MathError::Invariant)?);
            d = add_i64(d, 1)?;
        }
    }
    Ok(norm)
}

/// The ring slot of a live day (`cal_day ≤ day < cal_day + LOCK_BUCKETS`), else `None`.
fn live_slot(vault: &Vault, day: i64) -> Result<Option<usize>, MathError> {
    let end = add_i64(vault.cal_day, LOCK_BUCKETS_I64)?;
    if day < vault.cal_day || day >= end {
        return Ok(None);
    }
    slot(day).map(Some)
}

/// `pool_model.Pool._cal_add`: adds a lock term to the bucket of `until`'s day and to
/// `lock_norm`. A term of 0 changes nothing.
///
/// # Errors
/// `InvalidInput` if `term > 0` and `until`'s day is not live (`cal_day ≤ day <
/// cal_day + LOCK_BUCKETS`: run [`cal_advance`] first; a new lock's `lock_until` from
/// [`lock_until_for`] is always inside); `Overflow` if the bucket or `lock_norm` leaves
/// `u128`.
pub fn cal_add(vault: &mut Vault, until: i64, term: u128) -> Result<(), MathError> {
    if term == 0 {
        return Ok(());
    }
    let i = live_slot(vault, day_of(until)?)?.ok_or(MathError::InvalidInput)?;
    let norm = add_u128(vault.lock_norm, term)?;
    let b = vault.lock_cal.get_mut(i).ok_or(MathError::Invariant)?;
    *b = add_u128(*b, term)?;
    vault.lock_norm = norm;
    Ok(())
}

/// `pool_model.Pool._cal_sub`: removes a live lock's term from the bucket of `until`'s
/// day (at most what the bucket holds) and from `lock_norm`. The caller checks that the
/// lock is live (`now < until`, current `Π` epoch); a day outside the ring holds nothing.
///
/// # Errors
/// `Overflow` only for timestamps near `i64::MAX`.
pub fn cal_sub(vault: &mut Vault, until: i64, term: u128) -> Result<(), MathError> {
    let Some(i) = live_slot(vault, day_of(until)?)? else {
        return Ok(());
    };
    let b = vault.lock_cal.get_mut(i).ok_or(MathError::Invariant)?;
    let x = term.min(*b);
    *b = sub_u128(*b, x)?;
    vault.lock_norm = vault.lock_norm.saturating_sub(x);
    Ok(())
}

/// `pool_model.Pool.lock_until_for`: `lock_until` of a new or extended lock,
/// `⌈(max(now, max_release) + lock_grace_s)/LOCK_BUCKET_S⌉·LOCK_BUCKET_S` (ECON §2.1,
/// §9.3; R4A-8, R4B-8): the latest release of any position opened so far plus the grace,
/// rounded up to its calendar day's end.
///
/// # Errors
/// `Overflow` for timestamps near `i64::MAX`.
pub fn lock_until_for(pool: &Pool, now: i64, lock_grace_s: u32) -> Result<i64, MathError> {
    let x = add_i64(now.max(pool.max_release), i64::from(lock_grace_s))?;
    ovf(ceil_div_i64(x, LOCK_BUCKET_S)?.checked_mul(LOCK_BUCKET_S))
}

// ---- vesting (ECON §9.4; R3B-2, R4B-5) ---------------------------------------------------

/// Release of the vest due at `now`; the caller credits it.
fn release_in(p: &mut Pool, now: i64) -> Result<u64, MathError> {
    if p.vest_bal == 0 {
        p.vest_last = now;
        return Ok(0);
    }
    if now < p.vest_last {
        return Err(MathError::InvalidInput);
    }
    let rel = if now >= p.vest_end {
        p.vest_bal
    } else {
        let elapsed =
            u128::try_from(ovf(now.checked_sub(p.vest_last))?).map_err(|_| MathError::Underflow)?;
        let span = u128::try_from(ovf(p.vest_end.checked_sub(p.vest_last))?)
            .map_err(|_| MathError::Underflow)?;
        to_u64(mul_div(
            u128::from(p.vest_bal),
            elapsed,
            span,
            Rounding::Floor,
        )?)?
    };
    p.vest_last = now;
    if rel != 0 {
        if p.total_shares == 0 {
            // Unreachable: the vest is flushed whenever S reaches 0 (wipe, last lender).
            return Err(MathError::Invariant);
        }
        p.vest_bal = sub_u64(p.vest_bal, rel)?;
        credit_pos(p, i128::from(rel), true)?;
    }
    Ok(rel)
}

/// `pool_model.Pool._release` (inside `accrue`, ECON §9.4): releases
/// `⌊vest_bal·(now − vest_last)/(vest_end − vest_last)⌋` (all of it once `now ≥ vest_end`)
/// into `acc` and `acc_r` together, and sets `vest_last = now`. Idempotent at a given
/// `now`. Returns the lamports released.
///
/// # Errors
/// `InvalidInput` if `now < vest_last` while a vest is running; `Invariant` if a vest runs
/// while `S = 0` (the pool flushes it whenever `S` reaches 0); `Overflow` if an
/// accumulator leaves its type.
pub fn release(pool: &mut Pool, now: i64) -> Result<u64, MathError> {
    let mut p = *pool;
    let rel = release_in(&mut p, now)?;
    *pool = p;
    Ok(rel)
}

fn vest_in(
    p: &mut Pool,
    v: &mut Vault,
    wipe: Option<&mut ShareEpoch>,
    now: i64,
    lump_vest_s: u32,
    lamports: u64,
) -> Result<(), MathError> {
    if lamports == 0 {
        return Ok(());
    }
    release_in(p, now)?;
    if p.total_shares == 0 {
        // Nobody to vest for: the live wipe record or the protocol.
        return credit_in(p, v, wipe, i128::from(lamports), true);
    }
    let x = u128::from(lamports);
    let span: u128 = if p.vest_bal != 0 && p.vest_end > now {
        // Value-weighted end (R4B-5): the end moves by ⌈x·(lump_vest_s − left)/(bal + x)⌉
        // s. Never earlier while `lump_vest_s ≥ left` (absent a governance decrease of
        // `lump_vest_s` below the remaining span; after one, a lump pulls the end earlier,
        // as in pool_model), and at most 1 s later when `x·(lump_vest_s − left) ≤ bal + x`
        // (a dust lump against a running vest) (M0B-A-2).
        let left =
            u128::try_from(ovf(p.vest_end.checked_sub(now))?).map_err(|_| MathError::Underflow)?;
        let bal = u128::from(p.vest_bal);
        let num = add_u128(
            ovf(bal.checked_mul(left))?,
            ovf(x.checked_mul(u128::from(lump_vest_s)))?,
        )?;
        mul_div(num, 1, add_u128(bal, x)?, Rounding::Ceil)?
    } else {
        u128::from(lump_vest_s)
    };
    let span = i64::try_from(span).map_err(|_| MathError::Overflow)?;
    p.vest_bal = add_u64(p.vest_bal, lamports)?;
    p.vest_last = now;
    p.vest_end = add_i64(now, span)?;
    Ok(())
}

/// `pool_model.Pool._vest` (ECON §9.4; ARCH §3.3 `vest`): a lump of `lamports`, already in
/// `lender_sol`, released linearly: first the release due at `now`; then, if `S = 0`, a
/// realised credit (to the live wipe record or the protocol, see [`credit`]); otherwise
/// `vest_end = now + ⌈(vest_bal·(vest_end − now) + x·lump_vest_s)/(vest_bal + x)⌉` while a
/// vest runs (value-weighted, R4B-5), else `now + lump_vest_s`, and `vest_bal += x`,
/// `vest_last = now`. Nothing happens for 0 lamports (not even the release).
///
/// A lump moves a running vest's end by `⌈x·(lump_vest_s − left)/(vest_bal + x)⌉` seconds
/// (`left = vest_end − now`): never earlier while `lump_vest_s ≥ left`, which holds unless
/// governance lowered `lump_vest_s` below the remaining span; after such a decrease a small
/// lump pulls the end earlier (by days at the compiled extremes), exactly as `pool_model`.
///
/// # Errors
/// As [`release`] and [`credit`]; `Overflow` if `vest_bal` or `vest_end` leave their types.
pub fn vest(
    pool: &mut Pool,
    vault: &mut Vault,
    wipe_record: Option<&mut ShareEpoch>,
    now: i64,
    lump_vest_s: u32,
    lamports: u64,
) -> Result<(), MathError> {
    let (mut p, mut v) = (*pool, *vault);
    let mut rec = wipe_record.as_deref().copied();
    vest_in(&mut p, &mut v, rec.as_mut(), now, lump_vest_s, lamports)?;
    commit_record(wipe_record, rec);
    *pool = p;
    *vault = v;
    Ok(())
}

fn commit_record(target: Option<&mut ShareEpoch>, rec: Option<ShareEpoch>) {
    if let (Some(t), Some(r)) = (target, rec) {
        *t = r;
    }
}

// ---- SOL crediting (ECON §9.4, §9.5) -------------------------------------------------------

/// Credit while `S > 0`: `acc += ⌊(x·WAD + carry)/S⌋` (remainder carried) and, if
/// `realised` and `x > 0`, the same allocation into `acc_r`.
fn credit_pos(p: &mut Pool, lamports: i128, realised: bool) -> Result<(), MathError> {
    let num = add_i128(ovf(lamports.checked_mul(WAD_I))?, p.carry)?;
    let (d, r) = divmod_floor(num, p.total_shares)?;
    p.carry = to_i128(r)?;
    p.acc = add_i128(p.acc, d)?;
    if realised && lamports > 0 {
        credit_r_in(p, to_unsigned(lamports)?)?;
    }
    Ok(())
}

fn credit_in(
    p: &mut Pool,
    v: &mut Vault,
    wipe: Option<&mut ShareEpoch>,
    lamports: i128,
    realised: bool,
) -> Result<(), MathError> {
    if lamports == 0 {
        return Ok(());
    }
    if p.total_shares != 0 {
        return credit_pos(p, lamports, realised);
    }
    if p.is_wipe_live() {
        // V2-12: top up the full-wipe record for the wiped lenders.
        let rec = wipe.ok_or(MathError::InvalidInput)?;
        *rec = top_up(p, *rec, lamports, realised)?;
        return Ok(());
    }
    // R3A-10: no lender to credit: the protocol (signed, like the model's `protocol`).
    if lamports > 0 {
        let x: u64 = to_unsigned(lamports)?;
        v.lender_sol = sub_u64(v.lender_sol, x)?;
        v.protocol_fees = add_u64(v.protocol_fees, x)?;
    } else {
        let x = u64::try_from(lamports.unsigned_abs()).map_err(|_| MathError::Overflow)?;
        v.protocol_fees = sub_u64(v.protocol_fees, x)?;
        v.lender_sol = add_u64(v.lender_sol, x)?;
    }
    Ok(())
}

/// `pool_model.Pool._credit` (ECON §9.4, §9.5; ARCH §3.3 `credit`): credits `lamports`
/// (negative: a debit) of entitlement pro-rata to the current shares,
/// `acc += ⌊(lamports·WAD + carry)/S⌋` (toward −∞, remainder carried); with `realised`
/// it also allocates positive amounts as realised SOL ([`credit_r`]). While `S = 0`: after
/// a full wipe (`wipe_live`) the amount tops up the wipe record (`wipe_record`, both
/// accumulators if `realised`, V2-12); otherwise the SOL has no lender and moves from
/// `lender_sol` to `protocol_fees` (R3A-10).
///
/// # Errors
/// `InvalidInput` if a top-up is due and `wipe_record` is not the live wipe record;
/// `Underflow`/`Overflow` if a counter leaves its type.
pub fn credit(
    pool: &mut Pool,
    vault: &mut Vault,
    wipe_record: Option<&mut ShareEpoch>,
    lamports: i128,
    realised: bool,
) -> Result<(), MathError> {
    let (mut p, mut v) = (*pool, *vault);
    let mut rec = wipe_record.as_deref().copied();
    credit_in(&mut p, &mut v, rec.as_mut(), lamports, realised)?;
    commit_record(wipe_record, rec);
    *pool = p;
    *vault = v;
    Ok(())
}

/// Top-up of the live full-wipe record by `lamports` (V2-12).
fn top_up(
    pool: &Pool,
    mut rec: ShareEpoch,
    lamports: i128,
    realised: bool,
) -> Result<ShareEpoch, MathError> {
    if !rec.is_full_wipe() || rec.epoch.checked_add(1) != Some(pool.share_epoch) {
        return Err(MathError::InvalidInput);
    }
    let lw = ovf(lamports.checked_mul(WAD_I))?;
    let (d, r) = divmod_floor(add_i128(lw, rec.carry)?, rec.s_old)?;
    rec.carry = to_i128(r)?;
    rec.acc_end = add_i128(rec.acc_end, d)?;
    if realised {
        let (d, r) = divmod_floor(add_i128(lw, to_i128(rec.carry_r)?)?, rec.s_old)?;
        rec.carry_r = r;
        rec.acc_r_end = to_unsigned(add_i128(to_i128(rec.acc_r_end)?, d)?)?;
    }
    Ok(rec)
}

fn credit_r_in(p: &mut Pool, lamports: u64) -> Result<(), MathError> {
    if lamports == 0 || p.total_shares == 0 {
        return Ok(());
    }
    let num = add_u128(ovf(u128::from(lamports).checked_mul(WAD))?, p.carry_r)?;
    let d = ovf(num.checked_div(p.total_shares))?;
    let carry_r = ovf(num.checked_rem(p.total_shares))?;
    p.acc_r = add_u128(p.acc_r, d)?;
    p.carry_r = carry_r;
    Ok(())
}

/// `pool_model.Pool._credit_r` (V2-2; ECON §9.4): allocates `lamports` of realised
/// lender SOL to the current shares, `acc_r += ⌊(lamports·WAD + carry_r)/S⌋`. No-op for
/// 0 or while `S = 0`.
///
/// # Errors
/// `Overflow` if `acc_r` leaves `u128`.
pub fn credit_r(pool: &mut Pool, lamports: u64) -> Result<(), MathError> {
    let mut p = *pool;
    credit_r_in(&mut p, lamports)?;
    *pool = p;
    Ok(())
}

// ---- lender touch (ECON §9.4, §9.5; ARCH §3.3 `touch`) ------------------------------------

/// Epoch conversion as far as `records` reach (the loop of `pool_model.Pool._touch`),
/// accumulating `pending` and `real` in `i128`.
fn convert_in(
    pool: &Pool,
    l: &mut Lender,
    records: &[ShareEpoch],
    pending: &mut i128,
    real: &mut i128,
) -> Result<Standing, MathError> {
    if l.epoch > pool.share_epoch {
        return Err(MathError::InvalidInput);
    }
    loop {
        if l.epoch == pool.share_epoch {
            return Ok(Standing::Current);
        }
        let parks = pool.is_wipe_live() && l.epoch.checked_add(1) == Some(pool.share_epoch);
        if l.shares == 0 && l.lock == 0 && l.lock_term == 0 && !parks {
            // V2-12 jump for an empty lender, without reading a record: the model reads
            // one, adds 0, scales nothing and jumps to the same state. A lender whose
            // shares and lock floored to 0 can still hold a term (`⌊lock·WAD/Π⌋ > lock`
            // once Π < WAD): the model scales it by this record before jumping, so such a
            // lender reads the record below (M0B-A-1).
            l.epoch = pool.share_epoch;
            l.snap = pool.acc;
            l.snap_r = pool.acc_r;
            continue;
        }
        let Some(rec) = record_for(records, l.epoch)? else {
            return Ok(Standing::Behind);
        };
        *pending = add_i128(*pending, accrued(l.shares, sub_i128(rec.acc_end, l.snap)?)?)?;
        *real = add_i128(
            *real,
            accrued(l.shares, diff_u128(rec.acc_r_end, l.snap_r)?)?,
        )?;
        if parks {
            // V2-12: the full-wipe record is still open for top-ups; stay on it.
            l.snap = rec.acc_end;
            l.snap_r = rec.acc_r_end;
            return Ok(Standing::Parked);
        }
        let (num, den) = rec.ratio()?;
        l.shares = mul_div(l.shares, num, den, Rounding::Floor)?;
        l.lock = mul_div(l.lock, num, den, Rounding::Floor)?;
        // v0.4.1: the term scales with the bucket it sits in (the roll scaled the buckets).
        l.lock_term = mul_div(l.lock_term, num, den, Rounding::Floor)?;
        l.snap = 0;
        l.snap_r = 0;
        l.epoch = ovf(l.epoch.checked_add(1))?;
        if l.shares == 0 && l.lock == 0 {
            // V2-12 jump: nothing left to convert.
            l.epoch = pool.share_epoch;
            l.snap = pool.acc;
            l.snap_r = pool.acc_r;
        }
    }
}

/// `pool_model.Pool._to_pot` (R3A-2): realised allocation beyond the lender's
/// entitlement, `surplus = (real − claimed) − max(pending, 0)`, moves to the pot.
fn to_pot(v: &mut Vault, claimed: u64, pending: i128, real: &mut i128) -> Result<(), MathError> {
    let surplus = sub_i128(sub_i128(*real, i128::from(claimed))?, pending.max(0))?;
    if surplus > 0 {
        *real = sub_i128(*real, surplus)?;
        v.unmatched = add_u64(v.unmatched, to_unsigned(surplus)?)?;
    }
    Ok(())
}

fn touch_in(
    pool: &Pool,
    v: &mut Vault,
    l: &mut Lender,
    records: &[ShareEpoch],
    now: i64,
) -> Result<Standing, MathError> {
    cal_advance(v, now)?;
    let mut pending = i128::from(l.pending);
    let mut real = i128::from(l.real);
    let st = convert_in(pool, l, records, &mut pending, &mut real)?;
    match st {
        Standing::Behind => return Err(MathError::LenderBehind),
        Standing::Parked => to_pot(v, l.claimed, pending, &mut real)?,
        Standing::Current => {
            pending = add_i128(pending, accrued(l.shares, sub_i128(pool.acc, l.snap)?)?)?;
            l.snap = pool.acc;
            real = add_i128(real, accrued(l.shares, diff_u128(pool.acc_r, l.snap_r)?)?)?;
            l.snap_r = pool.acc_r;
            to_pot(v, l.claimed, pending, &mut real)?;
            // R3B-1 / v0.4.1: the lender's own expired lock; the calendar already dropped
            // its term from the aggregate at `lock_until`.
            if l.lock != 0 && now >= l.lock_until && l.lock_epoch == v.pi_epoch {
                l.lock = 0;
                l.lock_term = 0;
                l.lock_epoch = NO_LOCK_EPOCH;
            }
        }
    }
    l.pending = to_i64(pending)?;
    l.real = to_unsigned(real)?;
    Ok(st)
}

/// Lazy share-epoch conversion only, as far as `records` reach (the loop of
/// `pool_model.Pool._touch`; ECON §9.5, V2-12 jump and parking; ARCH §3.3
/// `sync_lender`). Does not accrue the current epoch, move a surplus to the pot or read
/// the calendar: follow with [`touch`].
///
/// # Errors
/// `InvalidInput` if the lender is ahead of the pool, the records are not consecutive
/// or a record is malformed; `Overflow`/`Underflow` if `pending`/`real` leave their
/// field types.
pub fn convert(
    pool: &Pool,
    lender: &mut Lender,
    records: &[ShareEpoch],
) -> Result<Standing, MathError> {
    let mut l = *lender;
    let mut pending = i128::from(l.pending);
    let mut real = i128::from(l.real);
    let st = convert_in(pool, &mut l, records, &mut pending, &mut real)?;
    l.pending = to_i64(pending)?;
    l.real = to_unsigned(real)?;
    *lender = l;
    Ok(st)
}

/// `pool_model.Pool._touch(who)` (ECON §9.4, §9.5; ARCH §3.3 `touch`): [`cal_advance`];
/// convert; accrue `pending` and `real` to the current `acc`/`acc_r` (floor); move a
/// surplus of realised SOL over entitlement to the pot `unmatched` (R3A-2); clear the
/// lender's own lock if it expired (`now ≥ lock_until`; the aggregate is not touched, the
/// calendar dropped it). A lender parked on a live wipe record moves its surplus to the
/// pot and stops there. Writes only the lender and the vault. Idempotent at a given `now`.
/// Returns [`Standing::Current`] or [`Standing::Parked`].
///
/// # Errors
/// `LenderBehind` if records are missing (the lender would stay [`Standing::Behind`]);
/// otherwise as [`convert`]; `Overflow` if the pot leaves `u64`.
pub fn touch(
    pool: &Pool,
    vault: &mut Vault,
    lender: &mut Lender,
    records: &[ShareEpoch],
    now: i64,
) -> Result<Standing, MathError> {
    let (mut v, mut l) = (*vault, *lender);
    let st = touch_in(pool, &mut v, &mut l, records, now)?;
    *vault = v;
    *lender = l;
    Ok(st)
}

// ---- share value ---------------------------------------------------------------------------

/// Tokens a burn of `shares` pays: `⌊shares·(T + ΣB_Q + VA)/(S + VS)⌋` (ECON §9.2).
///
/// # Errors
/// `Overflow` if the result exceeds `u128`.
pub fn tokens_for_shares(pool: &Pool, shares: u128) -> Result<u128, MathError> {
    mul_div(shares, claims_va(pool)?, shares_vs(pool)?, Rounding::Floor)
}

/// Shares a deposit of `amount` mints: `⌊amount·(S + VS)/(T + ΣB_Q + VA)⌋` (ECON §9.2).
///
/// # Errors
/// `Overflow` if the result exceeds `u128`.
pub fn shares_for_tokens(pool: &Pool, amount: u64) -> Result<u128, MathError> {
    mul_div(
        u128::from(amount),
        shares_vs(pool)?,
        claims_va(pool)?,
        Rounding::Floor,
    )
}

/// Token claim of a converted lender, 0 unless it is in the current epoch
/// (`pool_model.token_value`; I-P1).
///
/// # Errors
/// As [`tokens_for_shares`].
pub fn token_value(pool: &Pool, lender: &Lender) -> Result<u128, MathError> {
    if lender.epoch != pool.share_epoch {
        return Ok(0);
    }
    tokens_for_shares(pool, lender.shares)
}

// ---- pro-rata withdrawal (ECON §9.3; C0-9, V2-3, R3B-1, R4A-2) ------------------------------

/// `pool_model.Pool._lock_of`: `lock_i(now) = min(⌈lock·Π/lock_Π⌉, shares)` if the lock
/// is from this `Π` epoch and `now < lock_until`, else 0 (ECON §9.3).
///
/// # Errors
/// `DivByZero` if `lock_pi` is 0; `Overflow` if the product exceeds `u128`.
pub fn lock_of(vault: &Vault, lender: &Lender, now: i64) -> Result<u128, MathError> {
    if lender.lock_epoch != vault.pi_epoch || lender.lock == 0 || now >= lender.lock_until {
        return Ok(0);
    }
    Ok(mul_div(lender.lock, vault.pi, lender.lock_pi, Rounding::Ceil)?.min(lender.shares))
}

/// `pool_model.Pool.locked_total`: `locked = min(⌊lock_norm·Π/WAD⌋, S)` (floor, v0.4.1)
/// with `lock_norm` as the calendar stands at `now` (ECON §9.3). Does not change the
/// vault.
///
/// # Errors
/// `Overflow` if the product exceeds `u128`.
pub fn locked_total(pool: &Pool, vault: &Vault, now: i64) -> Result<u128, MathError> {
    Ok(mul_div(norm_at(vault, now)?, vault.pi, WAD, Rounding::Floor)?.min(pool.total_shares))
}

/// Tokens the lender may withdraw now (`pool_model.Pool.free_allowance` after its
/// touch; ECON §9.3, SPEC §2.4): with `u = shares − lock_i(now)` and
/// `su = max(S − locked, u)`, `min(⌊u·T/su⌋, ⌊shares·(T + ΣB_Q + VA)/(S + VS)⌋, T)`;
/// 0 if `su = 0` or the lender is parked on a live wipe record. Call [`touch`] first;
/// the vault is only read (the calendar as it stands at `now`).
///
/// # Errors
/// `LenderBehind` if the lender has not been converted to the current epoch.
pub fn allowance(pool: &Pool, vault: &Vault, lender: &Lender, now: i64) -> Result<u64, MathError> {
    if lender.epoch != pool.share_epoch {
        let parked = pool.is_wipe_live() && lender.epoch.checked_add(1) == Some(pool.share_epoch);
        return if parked {
            Ok(0)
        } else {
            Err(MathError::LenderBehind)
        };
    }
    let u = sub_u128(lender.shares, lock_of(vault, lender, now)?)?;
    let su = sub_u128(pool.total_shares, locked_total(pool, vault, now)?)?.max(u);
    if su == 0 {
        return Ok(0);
    }
    let t = u128::from(pool.tokens_free);
    let by_free = mul_div(u, t, su, Rounding::Floor)?;
    let by_value = tokens_for_shares(pool, lender.shares)?;
    to_u64(by_free.min(by_value).min(t))
}

/// `pool_model.Pool.allowance_view`: the lender's [`allowance`] exactly as the program
/// computes it when only this lender is touched and every other lender is left as it is
/// (the state ECON §13 I-P4 must hold on, v0.4.1; T-POT). Pure: nothing changes.
///
/// # Errors
/// As [`touch`] and [`allowance`].
pub fn allowance_view(
    pool: &Pool,
    vault: &Vault,
    lender: &Lender,
    records: &[ShareEpoch],
    now: i64,
) -> Result<u64, MathError> {
    let (mut v, mut l) = (*vault, *lender);
    touch_in(pool, &mut v, &mut l, records, now)?;
    allowance(pool, &v, &l, now)
}

fn withdraw_in(
    p: &mut Pool,
    v: &mut Vault,
    l: &mut Lender,
    records: &[ShareEpoch],
    now: i64,
    lock_grace_s: u32,
    shares: u128,
) -> Result<u64, MathError> {
    release_in(p, now)?;
    let st = touch_in(p, v, l, records, now)?;
    if l.pending < 0 {
        return Err(MathError::NegativePending);
    }
    if st != Standing::Current || shares == 0 || shares > l.shares {
        return Err(MathError::InsufficientShares);
    }
    let claims = claims_va(p)?;
    let amount = tokens_for_shares(p, shares)?;
    let t = u128::from(p.tokens_free);
    if amount > t {
        return Err(MathError::InsufficientInventory);
    }
    let allow = u128::from(allowance(p, v, l, now)?);
    if amount > allow {
        return Err(MathError::AboveAllowance);
    }
    let cur = lock_of(v, l, now)?;
    let u = sub_u128(l.shares, cur)?;
    let burn_max = if allow > 0 {
        mul_div(allow, shares_vs(p)?, claims, Rounding::Ceil)?
    } else {
        0
    };
    let add = if p.borrowed_q > 0 && burn_max > 0 && u > burn_max {
        mul_div(
            shares.min(burn_max),
            sub_u128(u, burn_max)?,
            burn_max,
            Rounding::Ceil,
        )?
    } else {
        0
    };
    let remaining = sub_u128(l.shares, shares)?;
    let new_lock = add_u128(cur, add)?.min(remaining);
    if new_lock != 0 && v.lock_norm == 0 {
        v.b_min = p.borrowed_q;
    }
    if l.lock_epoch == v.pi_epoch && cur != 0 {
        // A live lock leaves its bucket (an expired one's bucket is already gone).
        cal_sub(v, l.lock_until, l.lock_term)?;
    }
    // v0.4.1: floor, so the aggregate never exceeds the lenders' own (ceil) locks.
    let term = if new_lock != 0 {
        mul_div(new_lock, WAD, v.pi, Rounding::Floor)?
    } else {
        0
    };
    let until = if add != 0 {
        let prev = if cur != 0 { l.lock_until } else { 0 };
        prev.max(lock_until_for(p, now, lock_grace_s)?)
    } else {
        l.lock_until
    };
    cal_add(v, until, term)?;
    l.lock = new_lock;
    l.lock_pi = v.pi;
    l.lock_epoch = v.pi_epoch;
    l.lock_term = term;
    l.lock_until = until;
    if shares == p.total_shares && p.vest_bal != 0 {
        // R3B-2 / R4B-4: the last lender is paid the unreleased vest first.
        let vb = p.vest_bal;
        p.vest_bal = 0;
        credit_pos(p, i128::from(vb), true)?;
        touch_in(p, v, l, records, now)?;
    }
    l.shares = sub_u128(l.shares, shares)?;
    p.total_shares = sub_u128(p.total_shares, shares)?;
    let amount = to_u64(amount)?;
    p.tokens_free = sub_u64(p.tokens_free, amount)?;
    Ok(amount)
}

/// `pool_model.Pool.withdraw` (ECON §9.3; ARCH §3.3 `withdraw`): releases the vest,
/// touches the lender, burns `shares` for `⌊shares·(T + ΣB_Q + VA)/(S + VS)⌋` tokens and
/// returns them. Refused unless `pending ≥ 0` (C0-13), `0 < shares ≤ lender.shares` in
/// the current epoch, and the tokens are `≤ T` and `≤` [`allowance`]. The borrowed part
/// of the withdrawn fraction becomes a lock: `lock_i + ⌈min(shares, b_max)·(u −
/// b_max)/b_max⌉` (capped at the remaining shares; none while `ΣB_Q = 0`), `b_max =
/// ⌈allowance·(S + VS)/(T + ΣB_Q + VA)⌉`, `u = shares_i − lock_i`; the first lock of a
/// `Π` epoch sets `b_min = ΣB_Q` (V2-3). A live lock's term leaves its calendar bucket;
/// the new term `⌊new_lock·WAD/Π⌋` (floor) enters the bucket of `lock_until`, which a
/// lock increase moves to `max(lock_until, lock_until_for(now))` (R3B-1, R4A-2, R4A-8).
/// If the burn takes `S` to 0 the unreleased vest is released to the lender first. The
/// program pays SOL with [`claim`] afterwards.
///
/// # Errors
/// In the model's order: `NegativePending`, `InsufficientShares`,
/// `InsufficientInventory`, `AboveAllowance`; `Overflow` if a lock term `⌊lock·WAD/Π⌋` or
/// `lock_norm` exceeds `u128` (possible only while `Π` is tiny; the withdrawal is then
/// refused until `ΣB_Q` reaches 0, ARCH §9 M0b note 1); as [`touch`] and [`release`].
pub fn withdraw(
    pool: &mut Pool,
    vault: &mut Vault,
    lender: &mut Lender,
    records: &[ShareEpoch],
    now: i64,
    lock_grace_s: u32,
    shares: u128,
) -> Result<u64, MathError> {
    let (mut p, mut v, mut l) = (*pool, *vault, *lender);
    let amount = withdraw_in(&mut p, &mut v, &mut l, records, now, lock_grace_s, shares)?;
    *pool = p;
    *vault = v;
    *lender = l;
    Ok(amount)
}

fn resolve_in(p: &Pool, v: &mut Vault, now: i64, q: u64) -> Result<(), MathError> {
    cal_advance(v, now)?;
    if q >= p.borrowed_q {
        v.pi_epoch = ovf(v.pi_epoch.checked_add(1))?;
        v.pi = WAD;
        v.lock_norm = 0;
        v.lock_cal = [0; LOCK_BUCKETS];
        v.b_min = 0;
        return Ok(());
    }
    let bn = sub_u64(p.borrowed_q, q)?;
    if v.lock_norm > 0 && bn < v.b_min {
        v.pi = mul_div(v.pi, u128::from(bn), u128::from(v.b_min), Rounding::Ceil)?;
        v.b_min = bn;
    }
    Ok(())
}

/// `pool_model.Pool._resolve` (A37 as amended by A57, A75, V2-3; ECON §9.3): `q` borrowed
/// tokens are resolved (returned or SOL-settled). Call before `ΣB_Q` is reduced
/// ([`repay`] and [`settle`] do). [`cal_advance`] first; then, with `B' = ΣB_Q − q`: if
/// `B' = 0` (or `q ≥ ΣB_Q`) the `Π` epoch rolls and every lock and calendar bucket
/// clears; else if locks exist and `B' < b_min`, `Π ← ⌈Π·B'/b_min⌉` and `b_min = B'`.
///
/// # Errors
/// `Overflow` if the `Π` epoch counter overflows.
pub fn resolve(pool: &Pool, vault: &mut Vault, now: i64, q: u64) -> Result<(), MathError> {
    let mut v = *vault;
    resolve_in(pool, &mut v, now, q)?;
    *vault = v;
    Ok(())
}

// ---- deposits, claims, covers ---------------------------------------------------------------

fn deposit_in(
    p: &mut Pool,
    v: &mut Vault,
    l: &mut Lender,
    records: &[ShareEpoch],
    now: i64,
    amount: u64,
    rewards_required: bool,
) -> Result<u128, MathError> {
    release_in(p, now)?;
    touch_in(p, v, l, records, now)?;
    let minted = shares_for_tokens(p, amount)?;
    if minted == 0 {
        return Err(MathError::DepositTooSmall);
    }
    let total = add_u128(p.total_shares, minted)?;
    if p.total_shares == 0 && p.is_wipe_live() {
        let need = if rewards_required {
            WIPE_TRADER_SWEPT | WIPE_REWARDS_CLAIMED
        } else {
            WIPE_TRADER_SWEPT
        };
        if p.wipe_swept & need != need {
            return Err(MathError::WipeSweepPending);
        }
        // V2-12: the full-wipe record is final from now on; a parked depositor converts.
        p.wipe_live = 0;
        touch_in(p, v, l, records, now)?;
    }
    l.shares = add_u128(l.shares, minted)?;
    p.total_shares = total;
    p.tokens_free = add_u64(p.tokens_free, amount)?;
    Ok(minted)
}

/// `pool_model.Pool.deposit` (ECON §9.2; ARCH §3.3 `deposit`): releases the vest, touches
/// the lender, mints `⌊amount·(S + VS)/(T + ΣB_Q + VA)⌋` shares (returned) and adds
/// `amount` to `T`. The first deposit after a full wipe needs the post-wipe sweeps
/// (R4A-7): the trader sweep and, if `rewards_required` (a pump venue that can execute),
/// the venue-reward claim; it then finalises the wipe record (clears `wipe_live`) and
/// touches again, so a parked depositor converts through it.
///
/// # Errors
/// `DepositTooSmall` if nothing would be minted; `Overflow` if `S` or `T` would leave
/// `u128` / `u64`; `WipeSweepPending` if a wipe record is live and a required sweep has not
/// run since the wipe; as [`touch`] and [`release`].
pub fn deposit(
    pool: &mut Pool,
    vault: &mut Vault,
    lender: &mut Lender,
    records: &[ShareEpoch],
    now: i64,
    amount: u64,
    rewards_required: bool,
) -> Result<u128, MathError> {
    let (mut p, mut v, mut l) = (*pool, *vault, *lender);
    let minted = deposit_in(
        &mut p,
        &mut v,
        &mut l,
        records,
        now,
        amount,
        rewards_required,
    )?;
    *pool = p;
    *vault = v;
    *lender = l;
    Ok(minted)
}

/// `pool_model.Pool.claim_sol` (C0-13, V2-2, R3A-2; ARCH §3.3 `claim_sol`): touches the
/// lender (no vest release: `Market` stays read-only), draws from the pot up to its own
/// deficit, `draw = min(unmatched, max(0, pending − (real − claimed)))`, then pays
/// `min(pending, real − claimed, lender_sol)` if positive, else 0 (the draw stays). A
/// second call in the same state pays 0 (I-P5). Returns the lamports to transfer.
///
/// # Errors
/// As [`touch`]; `Overflow` if `real` or `claimed` leave `u64`.
pub fn claim(
    pool: &Pool,
    vault: &mut Vault,
    lender: &mut Lender,
    records: &[ShareEpoch],
    now: i64,
) -> Result<u64, MathError> {
    let (mut v, mut l) = (*vault, *lender);
    touch_in(pool, &mut v, &mut l, records, now)?;
    let own = sub_i128(i128::from(l.real), i128::from(l.claimed))?;
    let need = sub_i128(i128::from(l.pending), own)?.max(0);
    let draw = u64::try_from(need.min(i128::from(v.unmatched))).map_err(|_| MathError::Overflow)?;
    if draw != 0 {
        v.unmatched = sub_u64(v.unmatched, draw)?;
        l.real = add_u64(l.real, draw)?;
    }
    let unclaimed = sub_i128(i128::from(l.real), i128::from(l.claimed))?;
    let pay = i128::from(l.pending)
        .min(unclaimed)
        .min(i128::from(v.lender_sol));
    let paid = if pay > 0 {
        let pay: u64 = to_unsigned(pay)?;
        l.pending = to_i64(sub_i128(i128::from(l.pending), i128::from(pay))?)?;
        l.claimed = add_u64(l.claimed, pay)?;
        v.lender_sol = sub_u64(v.lender_sol, pay)?;
        pay
    } else {
        0
    };
    *vault = v;
    *lender = l;
    Ok(paid)
}

/// `pool_model.Pool.cover` (C0-13; ARCH §3.3 `cover_deficit`): touches the lender, then
/// the lender pays `lamports` into `lender_sol`: `pending += lamports`, `real +=
/// lamports`. What exceeds the lender's entitlement reaches the pot at its next touch.
///
/// # Errors
/// `Overflow` if a counter leaves its type; as [`touch`].
pub fn cover(
    pool: &Pool,
    vault: &mut Vault,
    lender: &mut Lender,
    records: &[ShareEpoch],
    now: i64,
    lamports: u64,
) -> Result<(), MathError> {
    let (mut v, mut l) = (*vault, *lender);
    touch_in(pool, &mut v, &mut l, records, now)?;
    v.lender_sol = add_u64(v.lender_sol, lamports)?;
    l.pending = to_i64(add_i128(i128::from(l.pending), i128::from(lamports))?)?;
    l.real = add_u64(l.real, lamports)?;
    *vault = v;
    *lender = l;
    Ok(())
}

/// Continuous rent crediting (`pool_model.Pool.advance(dt, d_index)`; ECON §7, §9.4;
/// ARCH §3.0 accrue): releases the vest due at `now`, then credits `lamports`, the
/// lenders' credit for the elapsed interval (`rent::lender_credit`, floor), as
/// entitlement (`receivable += lamports`). A credit while `S = 0` (only possible with a
/// dust position left after the last lender, ARCH §9 M0b note 4) goes to the protocol.
///
/// # Errors
/// As [`release`] and [`credit`] (a credit while a full-wipe record is live cannot occur:
/// `ΣB_Q = 0`, so it is refused with `InvalidInput`).
pub fn accrue(
    pool: &mut Pool,
    vault: &mut Vault,
    now: i64,
    lamports: u64,
) -> Result<(), MathError> {
    let (mut p, mut v) = (*pool, *vault);
    release_in(&mut p, now)?;
    if lamports != 0 {
        p.receivable = add_u64(p.receivable, lamports)?;
        credit_in(&mut p, &mut v, None, i128::from(lamports), false)?;
    }
    *pool = p;
    *vault = v;
    Ok(())
}

/// Trader sweep or venue cashback to lenders (`pool_model.Pool.sweep_sol`; ARCH §3.3,
/// §3.9; A44, V2-12, R3A-10, R4A-7): `lender_sol += lamports`; while a full-wipe record is
/// live (`S = 0`), the post-wipe bit of `source` is set (even for 0 lamports); then
/// [`vest`] (with `S > 0` the SOL vests over `lump_vest_s`; with `S = 0` it tops up the
/// live wipe record — pass it — or goes to the protocol).
///
/// # Errors
/// As [`vest`]; `Overflow` if `lender_sol` leaves `u64`.
pub fn sweep(
    pool: &mut Pool,
    vault: &mut Vault,
    wipe_record: Option<&mut ShareEpoch>,
    now: i64,
    lump_vest_s: u32,
    lamports: u64,
    source: SweepSource,
) -> Result<(), MathError> {
    let (mut p, mut v) = (*pool, *vault);
    let mut rec = wipe_record.as_deref().copied();
    v.lender_sol = add_u64(v.lender_sol, lamports)?;
    if p.total_shares == 0 && p.is_wipe_live() {
        p.wipe_swept |= source.bit();
    }
    vest_in(&mut p, &mut v, rec.as_mut(), now, lump_vest_s, lamports)?;
    commit_record(wipe_record, rec);
    *pool = p;
    *vault = v;
    Ok(())
}

// ---- borrower side (ECON §9.4; ARCH §3.3–§3.8) ---------------------------------------------

/// An open lends `q` tokens (`pool_model.Pool.borrow`; ARCH §3.3): releases the vest,
/// `T −= q`, `ΣB_Q += q`, `max_release = max(max_release, release_at)` with `release_at =
/// deadline + max_migration_pause_s + grad_grace_s + s3_wait_s` (the position's snapshot
/// values; R4A-8, R4B-8). The utilisation cap is `caps::util_ok`'s; the rent aggregate
/// (`A_j += q·coef_j·(10⁴ − rent_protocol_bps)`) is `rent::Aggr::add`'s.
///
/// # Errors
/// `InvalidInput` if `q = 0` or `release_at` lies beyond the compiled horizon (`now` plus
/// 90 d of duration and 3 d of S3 tail: a later lock day could not be kept in the
/// [`LOCK_BUCKETS`]-day calendar); `InsufficientInventory` if `q > T`; `Overflow` if
/// `ΣB_Q` leaves `u64`; as [`release`].
pub fn borrow(pool: &mut Pool, now: i64, q: u64, release_at: i64) -> Result<(), MathError> {
    let horizon = add_i64(now, MAX_RELEASE_SPAN_S)?;
    if q == 0 || release_at > horizon {
        return Err(MathError::InvalidInput);
    }
    if q > pool.tokens_free {
        return Err(MathError::InsufficientInventory);
    }
    let mut p = *pool;
    release_in(&mut p, now)?;
    p.borrowed_q = add_u64(p.borrowed_q, q)?;
    p.tokens_free = sub_u64(p.tokens_free, q)?;
    p.max_release = p.max_release.max(release_at);
    *pool = p;
    Ok(())
}

/// The shared head of `repay` and `settle` (`pool_model.Pool._close_pos`): vest release,
/// [`resolve`], `receivable −= min(credited, receivable)`.
fn close_pos(p: &mut Pool, v: &mut Vault, now: i64, res: &Resolution) -> Result<(), MathError> {
    if res.q == 0 || res.q > p.borrowed_q {
        return Err(MathError::InvalidInput);
    }
    release_in(p, now)?;
    resolve_in(p, v, now, res.q)?;
    p.receivable = sub_u64(p.receivable, res.credited.min(p.receivable))?;
    Ok(())
}

/// `pool_model.Pool.repay` (close, liquidation, expiry; ECON §9.4; ARCH §3.3): releases
/// the vest, resolves the position's `q` ([`resolve`]; `receivable −= min(credited,
/// receivable)`), returns `q` to `T`, adds `lender_rent + lump` to `lender_sol` with
/// `lender_rent = rent_paid − ⌊rent_paid·rent_protocol_bps/10⁴⌋` (the position's snapshot
/// share; the protocol part is the program's), credits the entitlement difference
/// `lender_rent − credited` (signed: a shortfall is a debit), allocates `lender_rent` as
/// realised and vests the lump. Returns `lender_rent`.
///
/// # Errors
/// `InvalidInput` if `q = 0`, `q > ΣB_Q` or `rent_protocol_bps > 10⁴`;
/// `Overflow`/`Underflow` if a counter leaves its type.
pub fn repay(
    pool: &mut Pool,
    vault: &mut Vault,
    now: i64,
    lump_vest_s: u32,
    res: &Resolution,
    rent_paid: u64,
    rent_protocol_bps: u32,
) -> Result<u64, MathError> {
    if u64::from(rent_protocol_bps) > BPS {
        return Err(MathError::InvalidInput);
    }
    let (mut p, mut v) = (*pool, *vault);
    close_pos(&mut p, &mut v, now, res)?;
    p.tokens_free = add_u64(p.tokens_free, res.q)?;
    p.borrowed_q = sub_u64(p.borrowed_q, res.q)?;
    let protocol = to_u64(mul_div(
        u128::from(rent_paid),
        u128::from(rent_protocol_bps),
        u128::from(BPS),
        Rounding::Floor,
    )?)?;
    let lender_rent = sub_u64(rent_paid, protocol)?;
    v.lender_sol = add_u64(v.lender_sol, add_u64(lender_rent, res.lump)?)?;
    credit_in(
        &mut p,
        &mut v,
        None,
        sub_i128(i128::from(lender_rent), i128::from(res.credited))?,
        false,
    )?;
    credit_r_in(&mut p, lender_rent)?;
    vest_in(&mut p, &mut v, None, now, lump_vest_s, res.lump)?;
    *pool = p;
    *vault = v;
    Ok(lender_rent)
}

/// `pool_model.Pool.settle` (SOL settlement S1/S2/S3; ECON §8.6, §9.4, §9.5; ARCH §3.3):
/// the token debt `q` is extinguished ([`resolve`], `ΣB_Q −= q`; token claims fall
/// pro-rata), `sol_in` lamports reach `lender_sol`; the lump part `min(lump, sol_in)` vests
/// and the principal `sol_in − lump` is immediate: the entitlement is trued up by
/// `principal − credited` and the principal is allocated as realised SOL; then
/// [`maybe_roll`]. Returns the new share-epoch record if the epoch rolled (the program
/// writes it).
///
/// # Errors
/// `InvalidInput` if `q = 0` or `q > ΣB_Q`; `Overflow`/`Underflow` if a counter leaves
/// its type.
pub fn settle(
    pool: &mut Pool,
    vault: &mut Vault,
    now: i64,
    lump_vest_s: u32,
    res: &Resolution,
    sol_in: u64,
) -> Result<Option<ShareEpoch>, MathError> {
    let (mut p, mut v) = (*pool, *vault);
    close_pos(&mut p, &mut v, now, res)?;
    p.borrowed_q = sub_u64(p.borrowed_q, res.q)?;
    let lump = res.lump.min(sol_in);
    let principal = sub_u64(sol_in, lump)?;
    v.lender_sol = add_u64(v.lender_sol, sol_in)?;
    credit_in(
        &mut p,
        &mut v,
        None,
        sub_i128(i128::from(principal), i128::from(res.credited))?,
        false,
    )?;
    credit_r_in(&mut p, principal)?;
    vest_in(&mut p, &mut v, None, now, lump_vest_s, lump)?;
    let rec = maybe_roll_in(&mut p, &mut v, now)?;
    *pool = p;
    *vault = v;
    Ok(rec)
}

// ---- share epochs (ECON §9.5; C0-6) -----------------------------------------------------------

fn maybe_roll_in(p: &mut Pool, v: &mut Vault, now: i64) -> Result<Option<ShareEpoch>, MathError> {
    if p.total_shares == 0 {
        return Ok(None);
    }
    let claims = p.token_claims()?;
    let (num, k) = if claims == 0 {
        // The wiped epoch's lenders get the unreleased vest.
        release_in(p, now)?;
        let vb = p.vest_bal;
        p.vest_bal = 0;
        if vb != 0 {
            credit_pos(p, i128::from(vb), true)?;
        }
        (0u8, 0u8)
    } else {
        let cap = ovf(R_MAX.checked_mul(add_u128(claims, VIRTUAL_ASSETS)?))?;
        if shares_vs(p)? <= cap {
            return Ok(None);
        }
        let mut k: u8 = 1;
        while add_u128(ovf(p.total_shares.checked_div(den_of(k)?))?, VIRTUAL_SHARES)? > cap {
            k = ovf(k.checked_add(1))?;
        }
        (1, k)
    };
    let rec = ShareEpoch {
        acc_end: p.acc,
        acc_r_end: p.acc_r,
        s_old: p.total_shares,
        carry: 0,
        carry_r: 0,
        epoch: p.share_epoch,
        num,
        den_log1000: k,
        _pad: [0; 10],
    };
    let den = den_of(k)?;
    let num = u128::from(num);
    p.share_epoch = ovf(p.share_epoch.checked_add(1))?;
    p.total_shares = mul_div(p.total_shares, num, den, Rounding::Floor)?;
    // v0.4.1: every calendar bucket scales; lock_norm is their sum.
    let mut norm: u128 = 0;
    for b in &mut v.lock_cal {
        *b = mul_div(*b, num, den, Rounding::Floor)?;
        norm = add_u128(norm, *b)?;
    }
    v.lock_norm = norm;
    p.dust = add_i128(add_i128(p.dust, p.carry)?, to_i128(p.carry_r)?)?;
    p.carry = 0;
    p.carry_r = 0;
    p.acc = 0;
    p.acc_r = 0;
    p.wipe_live = u8::from(p.total_shares == 0);
    if p.is_wipe_live() {
        // R4A-7: a deposit must wait for the post-wipe sweeps.
        p.wipe_swept = 0;
    }
    Ok(Some(rec))
}

/// `pool_model.Pool._maybe_roll` (ECON §9.5): after a settlement, if `S > 0` and
/// `T + ΣB_Q = 0` the unreleased vest is released to the wiped shares and the epoch rolls
/// as a full wipe (`0/1`); else if `S + VS > R_MAX·(T + ΣB_Q + VA)` it rolls with the
/// smallest rebase `1/1,000^k` that restores the bound. A roll records `(acc, acc_r, S)`,
/// converts `S` and every calendar bucket (floor; `lock_norm` is their sum), moves the
/// carries to `dust`, restarts both accumulators, sets `wipe_live = (S = 0)` and, at a
/// full wipe, clears the post-wipe sweep bits. Returns the record to write.
///
/// # Errors
/// `Overflow` if a counter leaves its type; as [`release`].
pub fn maybe_roll(
    pool: &mut Pool,
    vault: &mut Vault,
    now: i64,
) -> Result<Option<ShareEpoch>, MathError> {
    let (mut p, mut v) = (*pool, *vault);
    let rec = maybe_roll_in(&mut p, &mut v, now)?;
    *pool = p;
    *vault = v;
    Ok(rec)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use core::mem::align_of;

    const D: i64 = LOCK_BUCKET_S;
    const VEST: u32 = 21_600;
    const GRACE: u32 = 86_400;

    #[test]
    fn layouts_are_target_independent() {
        for (size, align) in [
            (size_of::<Pool>(), align_of::<Pool>()),
            (size_of::<Vault>(), align_of::<Vault>()),
            (size_of::<Lender>(), align_of::<Lender>()),
            (size_of::<ShareEpoch>(), align_of::<ShareEpoch>()),
        ] {
            assert_eq!(size % 16, 0);
            assert!(align <= 16);
        }
    }

    #[test]
    fn divmod_matches_python() {
        let cases: [(i128, u128, i128, u128); 7] = [
            (7, 2, 3, 1),
            (-7, 2, -4, 1),
            (-8, 2, -4, 0),
            (0, 5, 0, 0),
            (i128::MIN, 1, i128::MIN, 0),
            (-1, u128::MAX, -1, u128::MAX - 1),
            (5, u128::MAX, 0, 5),
        ];
        for (n, d, q, r) in cases {
            assert_eq!(divmod_floor(n, d), Ok((q, r)), "{n} divmod {d}");
        }
        assert_eq!(divmod_floor(1, 0), Err(MathError::DivByZero));
        assert_eq!(
            divmod_floor(i128::MIN, i128::MAX.unsigned_abs() + 1),
            Ok((-1, 0))
        );
    }

    #[test]
    fn day_arithmetic_matches_python() {
        assert_eq!(ceil_div_i64(0, D), Ok(0));
        assert_eq!(ceil_div_i64(1, D), Ok(1));
        assert_eq!(ceil_div_i64(D, D), Ok(1));
        assert_eq!(ceil_div_i64(-1, D), Ok(0));
        assert_eq!(ceil_div_i64(-D - 1, D), Ok(-1));
        assert_eq!(day_of(-1), Ok(-1));
        assert_eq!(slot(-1), Ok(LOCK_BUCKETS - 1));
        assert_eq!(slot(128), Ok(0));
        let mut p = Pool::new();
        assert_eq!(lock_until_for(&p, 0, 3_600), Ok(D));
        assert_eq!(lock_until_for(&p, D - 3_600, 3_600), Ok(D));
        assert_eq!(lock_until_for(&p, D - 3_599, 3_600), Ok(2 * D));
        p.max_release = 30 * D + 4_800;
        assert_eq!(lock_until_for(&p, 5, GRACE), Ok(32 * D));
        assert_eq!(
            lock_until_for(&p, i64::MAX - 10, GRACE),
            Err(MathError::Overflow)
        );
    }

    #[test]
    fn den_bounds() {
        assert_eq!(den_of(0), Ok(1));
        assert_eq!(den_of(12), Ok(10u128.pow(36)));
        assert_eq!(den_of(13), Err(MathError::Overflow));
        let bad = ShareEpoch {
            num: 0,
            den_log1000: 1,
            ..rec0()
        };
        assert_eq!(bad.ratio(), Err(MathError::InvalidInput));
        let bad = ShareEpoch { num: 2, ..rec0() };
        assert_eq!(bad.ratio(), Err(MathError::InvalidInput));
    }

    fn rec0() -> ShareEpoch {
        ShareEpoch {
            acc_end: 0,
            acc_r_end: 0,
            s_old: 1,
            carry: 0,
            carry_r: 0,
            epoch: 0,
            num: 0,
            den_log1000: 0,
            _pad: [0; 10],
        }
    }

    #[test]
    fn calendar_ring() {
        let mut v = Vault::new();
        // A day outside the ring is refused (advance first).
        assert_eq!(cal_add(&mut v, 200 * D, 5), Err(MathError::InvalidInput));
        cal_advance(&mut v, 10 * D + 7).unwrap();
        assert_eq!(v.cal_day, 11);
        assert_eq!(cal_add(&mut v, 10 * D, 5), Err(MathError::InvalidInput));
        assert_eq!(cal_add(&mut v, 11 * D, 0), Ok(()));
        cal_add(&mut v, 11 * D, 5).unwrap();
        cal_add(&mut v, 20 * D, 7).unwrap();
        cal_add(&mut v, 138 * D, 9).unwrap();
        assert_eq!(cal_add(&mut v, 139 * D, 1), Err(MathError::InvalidInput));
        assert_eq!(v.lock_norm, 21);
        assert_eq!(
            v.buckets().collect::<std::vec::Vec<_>>(),
            [(11, 5), (20, 7), (138, 9)]
        );
        // Removal takes at most the bucket; a dead day holds nothing.
        cal_sub(&mut v, 20 * D, 100).unwrap();
        cal_sub(&mut v, 5 * D, 100).unwrap();
        assert_eq!(v.lock_norm, 14);
        // The aggregate sees an expiry at its day end, without a touch.
        assert_eq!(norm_at(&v, 11 * D - 1), Ok(14));
        assert_eq!(norm_at(&v, 11 * D), Ok(9));
        cal_advance(&mut v, 11 * D).unwrap();
        assert_eq!((v.lock_norm, v.cal_day), (9, 12));
        // Idempotent, and never backwards.
        cal_advance(&mut v, 11 * D).unwrap();
        cal_advance(&mut v, 3).unwrap();
        assert_eq!((v.lock_norm, v.cal_day), (9, 12));
        // A gap of more than LOCK_BUCKETS days clears the ring.
        assert_eq!(norm_at(&v, 1_000 * D), Ok(0));
        cal_advance(&mut v, 1_000 * D).unwrap();
        assert_eq!((v.lock_norm, v.cal_day), (0, 1_001));
        assert!(v.lock_cal.iter().all(|b| *b == 0));
    }

    #[test]
    fn first_deposit_and_inflation_attack() {
        // ECON §9.2: internal counters ignore donations; a victim's 10⁹ redeems 10⁹.
        let (mut p, mut v) = (Pool::new(), Vault::new());
        let mut a = Lender::new(&p);
        let mut b = Lender::new(&p);
        assert_eq!(deposit(&mut p, &mut v, &mut a, &[], 0, 1, false), Ok(1_000));
        assert_eq!(
            deposit(&mut p, &mut v, &mut b, &[], 0, 1_000_000_000, false),
            Ok(1_000_000_000_000)
        );
        assert_eq!(
            withdraw(&mut p, &mut v, &mut b, &[], 0, GRACE, 1_000_000_000_000),
            Ok(1_000_000_000)
        );
        let before = (p, v, b);
        assert_eq!(
            deposit(&mut p, &mut v, &mut b, &[], 0, 0, false),
            Err(MathError::DepositTooSmall)
        );
        assert_eq!(
            withdraw(&mut p, &mut v, &mut b, &[], 0, GRACE, 1),
            Err(MathError::InsufficientShares)
        );
        assert_eq!((p, v, b), before, "refusals change nothing");
        assert_eq!(
            borrow(&mut p, 0, 2, 0),
            Err(MathError::InsufficientInventory)
        );
        assert_eq!(borrow(&mut p, 0, 0, 0), Err(MathError::InvalidInput));
        assert_eq!(
            borrow(&mut p, 5, 1, 5 + 93 * D + 1),
            Err(MathError::InvalidInput)
        );
        assert_eq!(borrow(&mut p, 5, 1, 5 + 93 * D), Ok(()));
    }

    #[test]
    fn records_must_be_consecutive() {
        let (mut p, mut v) = (Pool::new(), Vault::new());
        let mut a = Lender::new(&p);
        deposit(&mut p, &mut v, &mut a, &[], 0, 1_000, false).unwrap();
        borrow(&mut p, 0, 1_000, D).unwrap();
        let res = Resolution {
            q: 1_000,
            credited: 0,
            lump: 0,
        };
        let r0 = settle(&mut p, &mut v, 0, VEST, &res, 5).unwrap().unwrap();
        assert!(r0.is_full_wipe() && p.is_wipe_live());
        assert_eq!(p.wipe_swept, 0);
        let (vc, mut stale) = (v, a);
        let mut vt = v;
        assert_eq!(
            touch(&p, &mut vt, &mut stale, &[], 0),
            Err(MathError::LenderBehind)
        );
        assert_eq!((vt, stale), (vc, a), "a refused touch changes nothing");
        assert_eq!(convert(&p, &mut stale, &[]), Ok(Standing::Behind));
        let later = ShareEpoch { epoch: 7, ..r0 };
        assert_eq!(
            touch(&p, &mut vt, &mut stale, &[later], 0),
            Err(MathError::LenderBehind)
        );
        let gap = [r0, ShareEpoch { epoch: 2, ..r0 }];
        assert_eq!(convert(&p, &mut stale, &gap[1..]), Ok(Standing::Behind));
        let dup = [ShareEpoch { epoch: 0, ..r0 }, ShareEpoch { epoch: 0, ..r0 }];
        assert_eq!(record_for(&dup, 1), Err(MathError::InvalidInput));
        assert_eq!(touch(&p, &mut v, &mut a, &[r0], 0), Ok(Standing::Parked));
        assert_eq!(a.pending, 5);
        assert_eq!(allowance(&p, &v, &a, 0), Ok(0));
    }

    #[test]
    fn wipe_gate_needs_both_sweeps_on_pump_venues() {
        // R4A-7: after a full wipe a deposit waits for the post-wipe sweeps.
        let (mut p, mut v) = (Pool::new(), Vault::new());
        let mut old = Lender::new(&p);
        deposit(&mut p, &mut v, &mut old, &[], 0, 1_000, false).unwrap();
        borrow(&mut p, 0, 1_000, D).unwrap();
        let res = Resolution {
            q: 1_000,
            credited: 0,
            lump: 0,
        };
        let mut rec = settle(&mut p, &mut v, 0, VEST, &res, 50).unwrap().unwrap();
        let mut dust = Lender::new(&p);
        for req in [false, true] {
            assert_eq!(
                deposit(&mut p, &mut v, &mut dust, &[rec], 1, 2, req),
                Err(MathError::WipeSweepPending)
            );
        }
        sweep(
            &mut p,
            &mut v,
            Some(&mut rec),
            1,
            VEST,
            0,
            SweepSource::Trader,
        )
        .unwrap();
        assert_eq!(p.wipe_swept, WIPE_TRADER_SWEPT);
        assert_eq!(
            deposit(&mut p, &mut v, &mut dust, &[rec], 1, 2, true),
            Err(MathError::WipeSweepPending)
        );
        // 1 SOL of cashback tops up the wipe record, not the next depositor.
        sweep(
            &mut p,
            &mut v,
            Some(&mut rec),
            1,
            VEST,
            1_000_000_000,
            SweepSource::VenueRewards,
        )
        .unwrap();
        assert_eq!(v.lender_sol, 1_000_000_050);
        assert_eq!(
            deposit(&mut p, &mut v, &mut dust, &[rec], 1, 2, true),
            Ok(2_000)
        );
        assert!(!p.is_wipe_live());
        assert_eq!(
            touch(&p, &mut v, &mut old, &[rec], 2),
            Ok(Standing::Current)
        );
        assert_eq!(old.pending, 1_000_000_050);
        assert_eq!(claim(&p, &mut v, &mut dust, &[], 2), Ok(0));
    }

    #[test]
    fn vest_releases_linearly_with_a_value_weighted_end() {
        let (mut p, mut v) = (Pool::new(), Vault::new());
        let mut h = Lender::new(&p);
        deposit(&mut p, &mut v, &mut h, &[], 0, 1_000, false).unwrap();
        sweep(&mut p, &mut v, None, 0, VEST, 6_000, SweepSource::Trader).unwrap();
        assert_eq!((p.vest_bal, p.vest_end), (6_000, 21_600));
        assert_eq!(release(&mut p, 3_600), Ok(1_000));
        assert_eq!(release(&mut p, 3_600), Ok(0), "idempotent at one time");
        // A dust lump moves the end by at most 1 s (ceil), never earlier (lump_vest_s
        // unchanged, so lump_vest_s ≥ vest_end − now).
        sweep(&mut p, &mut v, None, 3_600, VEST, 1, SweepSource::Trader).unwrap();
        assert_eq!(p.vest_end, 21_601);
        // A lump as large as the balance moves the end half way.
        let bal = p.vest_bal;
        sweep(&mut p, &mut v, None, 3_600, VEST, bal, SweepSource::Trader).unwrap();
        assert_eq!(p.vest_end, 3_600 + 19_801); // ⌈(18,001 + 21,600)/2⌉
        assert_eq!(release(&mut p, 1_000_000), Ok(2 * bal));
        assert_eq!(p.vest_bal, 0);
        assert_eq!(release(&mut p, 5), Ok(0), "no vest: vest_last follows now");
        assert_eq!(p.vest_last, 5);
    }

    /// M0B-A-2 (review): "never earlier" needs `lump_vest_s ≥ vest_end − now`. After a
    /// governance decrease of `lump_vest_s` (compiled bounds 3,600..604,800 s), a
    /// 1-lamport lump pulls a running vest's end three days earlier; `pool_model._vest`
    /// with `lump_vest_s` changed the same way gives the same 347,400.
    #[test]
    fn vest_end_moves_earlier_after_a_governance_decrease() {
        let mut p = Pool::new();
        p.total_shares = 1_000_000;
        p.tokens_free = 1;
        let mut v = Vault::new();
        v.lender_sol = 10;
        vest(&mut p, &mut v, None, 0, 604_800, 1).unwrap();
        assert_eq!(p.vest_end, 604_800);
        vest(&mut p, &mut v, None, 86_400, 3_600, 1).unwrap();
        // ⌈(1·518,400 + 1·3,600)/2⌉ = 261,000 s after now: day ≈ 4 instead of day 7.
        assert_eq!(p.vest_end, 86_400 + 261_000);
        assert_eq!(p.vest_bal, 2);
        // With lump_vest_s at or above the remaining span the end never moves earlier.
        vest(&mut p, &mut v, None, 86_400, 261_000, 1).unwrap();
        assert_eq!(p.vest_end, 86_400 + 261_000);
    }

    /// M0B-A-1 (review): a lender whose shares and lock floored to 0 while its lock term
    /// did not (`⌊lock·WAD/Π⌋ > lock` once Π < WAD) reads the record of its epoch and scales
    /// the term before the V2-12 jump, as `pool_model._touch` (`lock_term·num//den`); a
    /// lender without a term still jumps without records.
    #[test]
    fn empty_lender_with_a_term_scales_it() {
        let mut p = Pool::new();
        p.share_epoch = 1;
        p.total_shares = 1_000_000;
        p.tokens_free = 1_000;
        let wipe = ShareEpoch {
            acc_end: 0,
            acc_r_end: 0,
            s_old: 2_000,
            carry: 0,
            carry_r: 0,
            epoch: 0,
            num: 0,
            den_log1000: 0,
            _pad: [0; 10],
        };
        let mut v = Vault::new();
        let mut l = Lender::new(&p);
        l.epoch = 0;
        l.lock_term = 1;
        let before = (v, l);
        assert_eq!(
            touch(&p, &mut v, &mut l, &[], 0),
            Err(MathError::LenderBehind)
        );
        assert_eq!((v, l), before, "a refused touch changes nothing");
        assert_eq!(touch(&p, &mut v, &mut l, &[wipe], 0), Ok(Standing::Current));
        // pool_model: lock_term = 1·0 // 1 = 0 (the crate kept 1 before the fix).
        assert_eq!((l.epoch, l.lock_term), (1, 0));
        // Two 1/1000 rebases: the first record scales the term, then the lender jumps.
        p.share_epoch = 2;
        let rebase = ShareEpoch {
            num: 1,
            den_log1000: 1,
            ..wipe
        };
        let mut l = Lender::new(&p);
        l.epoch = 0;
        l.lock_term = 123_456;
        assert_eq!(convert(&p, &mut l, &[]), Ok(Standing::Behind));
        assert_eq!(convert(&p, &mut l, &[rebase]), Ok(Standing::Current));
        assert_eq!((l.epoch, l.lock_term, l.snap), (2, 123, p.acc));
        // Without a term the lender jumps without records (V2-12).
        let mut l = Lender::new(&p);
        l.epoch = 0;
        assert_eq!(convert(&p, &mut l, &[]), Ok(Standing::Current));
        assert_eq!((l.epoch, l.lock_term), (2, 0));
    }

    #[test]
    fn credits_without_shares_go_to_the_protocol() {
        // R3A-10: every lender left (no wipe), then a sweep arrives.
        let (mut p, mut v) = (Pool::new(), Vault::new());
        let mut a = Lender::new(&p);
        let s = deposit(&mut p, &mut v, &mut a, &[], 0, 1_000_000_000_000, false).unwrap();
        withdraw(&mut p, &mut v, &mut a, &[], 0, GRACE, s).unwrap();
        sweep(
            &mut p,
            &mut v,
            None,
            0,
            VEST,
            1_000_000_000,
            SweepSource::Trader,
        )
        .unwrap();
        assert_eq!((v.lender_sol, v.protocol_fees), (0, 1_000_000_000));
        let mut d = Lender::new(&p);
        deposit(&mut p, &mut v, &mut d, &[], 0, 1_000_000, false).unwrap();
        assert_eq!(claim(&p, &mut v, &mut d, &[], 30_000), Ok(0));
        // A debit while S = 0 takes back from the protocol (signed, as the model).
        let (mut q, mut w) = (Pool::new(), Vault::new());
        assert_eq!(
            credit(&mut q, &mut w, None, -5, false),
            Err(MathError::Underflow)
        );
        w.protocol_fees = 7;
        credit(&mut q, &mut w, None, -5, false).unwrap();
        assert_eq!((w.protocol_fees, w.lender_sol), (2, 5));
    }
}
