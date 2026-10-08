//! Lender-pool previews: shares, allowance, locks, withdrawals, deposits and claims on
//! copies of the on-chain state (`shortwin_math::pool`; ECON §9; ARCH §3.3). Nothing
//! here changes the caller's objects.

use shortwin_math::pool::{
    self as mpool, LOCK_BUCKETS, Lender, NO_LOCK_EPOCH, Pool, ShareEpoch, Vault,
};
use wasm_bindgen::JsError;
use wasm_bindgen::prelude::wasm_bindgen;

/// The lender-pool fields of `Market` (`pool::Pool`; ARCH §2.3.4) and of `MarketVault`
/// (`pool::Vault`; ARCH §2.3.6). The lock calendar is set with `setLockBucket`.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolState {
    /// `S`.
    #[wasm_bindgen(js_name = totalShares)]
    pub total_shares: u128,
    /// `acc`.
    pub acc: i128,
    /// Remainder of `acc` credits.
    pub carry: i128,
    /// `acc_r`.
    #[wasm_bindgen(js_name = accR)]
    pub acc_r: u128,
    /// Remainder of `acc_r` allocations.
    #[wasm_bindgen(js_name = carryR)]
    pub carry_r: u128,
    /// Dropped carries.
    pub dust: i128,
    /// `Π`.
    pub pi: u128,
    /// `lock_norm`.
    #[wasm_bindgen(js_name = lockNorm)]
    pub lock_norm: u128,
    /// `T`.
    #[wasm_bindgen(js_name = tokensFree)]
    pub tokens_free: u64,
    /// `ΣB_Q`.
    #[wasm_bindgen(js_name = borrowedQ)]
    pub borrowed_q: u64,
    /// `receivable`.
    pub receivable: u64,
    /// `max_release`.
    #[wasm_bindgen(js_name = maxRelease)]
    pub max_release: i64,
    /// `vest_bal`.
    #[wasm_bindgen(js_name = vestBal)]
    pub vest_bal: u64,
    /// `vest_last`.
    #[wasm_bindgen(js_name = vestLast)]
    pub vest_last: i64,
    /// `vest_end`.
    #[wasm_bindgen(js_name = vestEnd)]
    pub vest_end: i64,
    /// `b_min`.
    #[wasm_bindgen(js_name = bMin)]
    pub b_min: u64,
    /// `share_epoch`.
    #[wasm_bindgen(js_name = shareEpoch)]
    pub share_epoch: u32,
    /// `pi_epoch`.
    #[wasm_bindgen(js_name = piEpoch)]
    pub pi_epoch: u32,
    /// `wipe_live` flag.
    #[wasm_bindgen(js_name = wipeLive)]
    pub wipe_live: bool,
    /// Post-wipe sweep bits (`Market.flags` bits 7, 8 → 1, 2).
    #[wasm_bindgen(js_name = wipeSwept)]
    pub wipe_swept: u8,
    /// `MarketVault.lender_sol`.
    #[wasm_bindgen(js_name = lenderSol)]
    pub lender_sol: u64,
    /// `MarketVault.unmatched` (the pot).
    pub unmatched: u64,
    /// `MarketVault.protocol_fees`.
    #[wasm_bindgen(js_name = protocolFees)]
    pub protocol_fees: u64,
    /// `MarketVault.cal_day`.
    #[wasm_bindgen(js_name = calDay)]
    pub cal_day: i64,
    lock_cal: Vec<u128>,
}

impl PoolState {
    fn pool(&self) -> Pool {
        Pool {
            total_shares: self.total_shares,
            acc: self.acc,
            carry: self.carry,
            acc_r: self.acc_r,
            carry_r: self.carry_r,
            dust: self.dust,
            tokens_free: self.tokens_free,
            borrowed_q: self.borrowed_q,
            receivable: self.receivable,
            max_release: self.max_release,
            vest_bal: self.vest_bal,
            vest_last: self.vest_last,
            vest_end: self.vest_end,
            share_epoch: self.share_epoch,
            wipe_live: u8::from(self.wipe_live),
            wipe_swept: self.wipe_swept,
            ..Pool::new()
        }
    }

    fn vault(&self) -> Vault {
        let mut lock_cal = [0u128; LOCK_BUCKETS];
        for (b, v) in lock_cal.iter_mut().zip(&self.lock_cal) {
            *b = *v;
        }
        Vault {
            lock_cal,
            pi: self.pi,
            lock_norm: self.lock_norm,
            lender_sol: self.lender_sol,
            unmatched: self.unmatched,
            protocol_fees: self.protocol_fees,
            b_min: self.b_min,
            cal_day: self.cal_day,
            pi_epoch: self.pi_epoch,
            ..Vault::new()
        }
    }
}

/// A `LenderPosition` (`pool::Lender`; ARCH §2.3.6).
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LenderState {
    /// Shares in `epoch`.
    pub shares: u128,
    /// `acc` at the last touch.
    pub snap: i128,
    /// `acc_r` at the last touch.
    #[wasm_bindgen(js_name = snapR)]
    pub snap_r: u128,
    /// Locked shares at `lockPi`.
    pub lock: u128,
    /// `Π` when the lock was set.
    #[wasm_bindgen(js_name = lockPi)]
    pub lock_pi: u128,
    /// Contribution to `lock_norm`.
    #[wasm_bindgen(js_name = lockTerm)]
    pub lock_term: u128,
    /// SOL entitlement not yet paid.
    pub pending: i64,
    /// Realised SOL allocated.
    pub real: u64,
    /// SOL paid.
    pub claimed: u64,
    /// Share epoch of `shares`.
    pub epoch: u32,
    /// `Π` epoch of `lock` (4294967295 without a lock).
    #[wasm_bindgen(js_name = lockEpoch)]
    pub lock_epoch: u32,
    /// When the lock expires.
    #[wasm_bindgen(js_name = lockUntil)]
    pub lock_until: i64,
}

impl LenderState {
    fn lender(&self) -> Lender {
        Lender {
            shares: self.shares,
            snap: self.snap,
            snap_r: self.snap_r,
            lock: self.lock,
            lock_pi: self.lock_pi,
            lock_term: self.lock_term,
            pending: self.pending,
            real: self.real,
            claimed: self.claimed,
            epoch: self.epoch,
            lock_epoch: self.lock_epoch,
            lock_until: self.lock_until,
            fwd_epoch: 0,
            fwd: 0,
            fwd_lag: 0,
            _pad: [0; 2],
            fwd_shares: 0,
        }
    }

    fn of(l: &Lender) -> LenderState {
        LenderState {
            shares: l.shares,
            snap: l.snap,
            snap_r: l.snap_r,
            lock: l.lock,
            lock_pi: l.lock_pi,
            lock_term: l.lock_term,
            pending: l.pending,
            real: l.real,
            claimed: l.claimed,
            epoch: l.epoch,
            lock_epoch: l.lock_epoch,
            lock_until: l.lock_until,
        }
    }
}

#[wasm_bindgen]
impl LenderState {
    /// A lender that has never deposited, at `pool`'s current state.
    #[wasm_bindgen(js_name = newIn)]
    pub fn new_in(pool: &PoolState) -> LenderState {
        LenderState::of(&Lender::new(&pool.pool()))
    }

    /// `lockEpoch` of a lender that never locked.
    #[wasm_bindgen(js_name = noLockEpoch)]
    pub fn no_lock_epoch() -> u32 {
        NO_LOCK_EPOCH
    }
}

/// Consecutive `ShareEpoch` records from the lender's epoch on (`pool` module docs). A
/// preview converts on a copy: the program writes the records' `conv_rem` (KI-65), a
/// preview never changes them.
#[wasm_bindgen]
#[derive(Clone, Debug, Default)]
pub struct ShareEpochs(Vec<ShareEpoch>);

#[wasm_bindgen]
impl ShareEpochs {
    /// No records (enough for a lender in the current epoch).
    #[wasm_bindgen(constructor)]
    pub fn new() -> ShareEpochs {
        ShareEpochs::default()
    }

    /// Appends the record of `epoch` (the next one in order), with its conversion remainder
    /// `conv_rem` as fetched (KI-65: a behind lender's conversion depends on it).
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        epoch: u32,
        acc_end: i128,
        acc_r_end: u128,
        s_old: u128,
        carry: i128,
        carry_r: u128,
        num: u8,
        den_log1000: u8,
        conv_rem: u128,
    ) {
        self.0.push(ShareEpoch {
            acc_end,
            acc_r_end,
            s_old,
            carry,
            carry_r,
            conv_rem,
            epoch,
            num,
            den_log1000,
            _pad: [0; 10],
        });
    }
}

/// Shares a deposit of `amount` mints (ECON §9.2).
#[wasm_bindgen(js_name = sharesForTokens)]
pub fn shares_for_tokens(pool: &PoolState, amount: u64) -> Result<u128, JsError> {
    Ok(mpool::shares_for_tokens(&pool.pool(), amount)?)
}

/// Tokens a burn of `shares` pays (ECON §9.2).
#[wasm_bindgen(js_name = tokensForShares)]
pub fn tokens_for_shares(pool: &PoolState, shares: u128) -> Result<u128, JsError> {
    Ok(mpool::tokens_for_shares(&pool.pool(), shares)?)
}

/// The lender after the program's `touch` at `now` (epoch conversion, accrual, surplus to
/// the pot, own expired lock).
#[wasm_bindgen(js_name = previewTouch)]
pub fn preview_touch(
    pool: &PoolState,
    lender: &LenderState,
    records: &ShareEpochs,
    now: i64,
) -> Result<LenderState, JsError> {
    let (mut v, mut l) = (pool.vault(), lender.lender());
    mpool::touch(&pool.pool(), &mut v, &mut l, &mut records.0.clone(), now)?;
    Ok(LenderState::of(&l))
}

/// Tokens the lender may withdraw now (ECON §9.3), after its touch.
#[wasm_bindgen(js_name = previewAllowance)]
pub fn preview_allowance(
    pool: &PoolState,
    lender: &LenderState,
    records: &ShareEpochs,
    now: i64,
) -> Result<u64, JsError> {
    Ok(mpool::allowance_view(
        &pool.pool(),
        &pool.vault(),
        &lender.lender(),
        &records.0,
        now,
    )?)
}

/// The lender's locked shares now (ECON §9.3), after its touch.
#[wasm_bindgen(js_name = previewLock)]
pub fn preview_lock(
    pool: &PoolState,
    lender: &LenderState,
    records: &ShareEpochs,
    now: i64,
) -> Result<u128, JsError> {
    let (mut v, mut l) = (pool.vault(), lender.lender());
    mpool::touch(&pool.pool(), &mut v, &mut l, &mut records.0.clone(), now)?;
    Ok(mpool::lock_of(&v, &l, now)?)
}

/// The lender's token claim (ECON §9.2), after its touch.
#[wasm_bindgen(js_name = previewTokenValue)]
pub fn preview_token_value(
    pool: &PoolState,
    lender: &LenderState,
    records: &ShareEpochs,
    now: i64,
) -> Result<u128, JsError> {
    let (p, mut v, mut l) = (pool.pool(), pool.vault(), lender.lender());
    mpool::touch(&p, &mut v, &mut l, &mut records.0.clone(), now)?;
    Ok(mpool::token_value(&p, &l)?)
}

/// `lock_until` a withdrawal at `now` sets on a new or extended lock (`pool::lock_until_for`,
/// ECON §9.3): `⌈(max(now, max_release) + lock_grace_s)/86,400⌉·86,400`.
#[wasm_bindgen(js_name = previewLockUntil)]
pub fn preview_lock_until(pool: &PoolState, now: i64, lock_grace_s: u32) -> Result<i64, JsError> {
    Ok(mpool::lock_until_for(&pool.pool(), now, lock_grace_s)?)
}

/// Lamports of the vesting lump balance released to the shares at `now` (`pool::release`,
/// ECON §9.4: linear to `vest_end`, all of it after).
#[wasm_bindgen(js_name = previewVestRelease)]
pub fn preview_vest_release(pool: &PoolState, now: i64) -> Result<u64, JsError> {
    let mut p = pool.pool();
    Ok(mpool::release(&mut p, now)?)
}

/// Locked shares of the whole pool at `now` (`pool::locked_total`, floored; expired
/// calendar days excluded).
#[wasm_bindgen(js_name = previewLockedTotal)]
pub fn preview_locked_total(pool: &PoolState, now: i64) -> Result<u128, JsError> {
    Ok(mpool::locked_total(&pool.pool(), &pool.vault(), now)?)
}

/// Tokens `withdraw(shares)` would pay; throws the refusal (e.g. `"negative pending"`,
/// `"above pro-rata allowance"`).
#[wasm_bindgen(js_name = previewWithdraw)]
pub fn preview_withdraw(
    pool: &PoolState,
    lender: &LenderState,
    records: &ShareEpochs,
    now: i64,
    lock_grace_s: u32,
    shares: u128,
) -> Result<u64, JsError> {
    let (mut p, mut v, mut l) = (pool.pool(), pool.vault(), lender.lender());
    Ok(mpool::withdraw(
        &mut p,
        &mut v,
        &mut l,
        &mut records.0.clone(),
        now,
        lock_grace_s,
        shares,
    )?)
}

/// What `withdraw(shares)` does, step for step as the program's handler after its prologue
/// (ARCH §3.3 `withdraw`): `pool::withdraw` (the tokens, the lock and `lock_until`, the last
/// lender's vest flush, R3B-2), the program's own check on top of it and the SOL part, which is
/// `pool::claim` right after (as `claim_sol`: the pot draw, `min(pending, real − claimed,
/// lender_sol)`).
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Withdrawal {
    /// Tokens paid.
    pub tokens: u64,
    /// Lamports paid (the vest flush of a last lender's exit included).
    pub sol: u64,
    /// The burn leaves `S = 0` while `ΣB_Q > 0`: the program refuses it (`LastSharesBorrowed`,
    /// M1-SEC-3; ARCH §9 M0b note 4).
    #[wasm_bindgen(js_name = lastSharesBorrowed)]
    pub last_shares_borrowed: bool,
    /// The lender afterwards (`lock`, `lockUntil`, `shares`, `pending`, …).
    pub lender: LenderState,
}

/// [`Withdrawal`] of `shares` at `now` on the stored state (the program's prologue may first
/// accrue rent, so its SOL part can only be larger); throws `withdraw`'s and `claim`'s
/// refusals as [`preview_withdraw`] does.
#[wasm_bindgen(js_name = previewWithdrawal)]
pub fn preview_withdrawal(
    pool: &PoolState,
    lender: &LenderState,
    records: &ShareEpochs,
    now: i64,
    lock_grace_s: u32,
    shares: u128,
) -> Result<Withdrawal, JsError> {
    let (mut p, mut v, mut l) = (pool.pool(), pool.vault(), lender.lender());
    let mut recs = records.0.clone();
    let tokens = mpool::withdraw(&mut p, &mut v, &mut l, &mut recs, now, lock_grace_s, shares)?;
    let last_shares_borrowed = p.total_shares == 0 && p.borrowed_q != 0;
    let sol = mpool::claim(&p, &mut v, &mut l, &mut recs, now)?;
    Ok(Withdrawal {
        tokens,
        sol,
        last_shares_borrowed,
        lender: LenderState::of(&l),
    })
}

/// Shares `deposit(amount)` would mint; throws `"post-wipe sweep pending"` while a wipe
/// record waits for its sweeps.
#[wasm_bindgen(js_name = previewDeposit)]
pub fn preview_deposit(
    pool: &PoolState,
    lender: &LenderState,
    records: &ShareEpochs,
    now: i64,
    amount: u64,
    rewards_required: bool,
) -> Result<u128, JsError> {
    let (mut p, mut v, mut l) = (pool.pool(), pool.vault(), lender.lender());
    Ok(mpool::deposit(
        &mut p,
        &mut v,
        &mut l,
        &mut records.0.clone(),
        now,
        amount,
        rewards_required,
    )?)
}

/// Lamports `claim_sol` would pay (pot draw included).
#[wasm_bindgen(js_name = previewClaim)]
pub fn preview_claim(
    pool: &PoolState,
    lender: &LenderState,
    records: &ShareEpochs,
    now: i64,
) -> Result<u64, JsError> {
    let (mut v, mut l) = (pool.vault(), lender.lender());
    Ok(mpool::claim(
        &pool.pool(),
        &mut v,
        &mut l,
        &mut records.0.clone(),
        now,
    )?)
}

#[wasm_bindgen]
impl PoolState {
    /// A new market's pool (`Π = WAD`, nothing deposited).
    #[wasm_bindgen(constructor)]
    pub fn new() -> PoolState {
        let (p, v) = (Pool::new(), Vault::new());
        PoolState {
            total_shares: p.total_shares,
            acc: p.acc,
            carry: p.carry,
            acc_r: p.acc_r,
            carry_r: p.carry_r,
            dust: p.dust,
            pi: v.pi,
            lock_norm: v.lock_norm,
            tokens_free: p.tokens_free,
            borrowed_q: p.borrowed_q,
            receivable: p.receivable,
            max_release: p.max_release,
            vest_bal: p.vest_bal,
            vest_last: p.vest_last,
            vest_end: p.vest_end,
            b_min: v.b_min,
            share_epoch: p.share_epoch,
            pi_epoch: v.pi_epoch,
            wipe_live: p.is_wipe_live(),
            wipe_swept: p.wipe_swept,
            lender_sol: v.lender_sol,
            unmatched: v.unmatched,
            protocol_fees: v.protocol_fees,
            cal_day: v.cal_day,
            lock_cal: Vec::new(),
        }
    }

    /// Sets calendar slot `slot` (`day mod 128`) to `term` (`MarketVault.lock_cal`).
    #[wasm_bindgen(js_name = setLockBucket)]
    pub fn set_lock_bucket(&mut self, slot: u32, term: u128) {
        let i = slot as usize % LOCK_BUCKETS;
        if self.lock_cal.len() < LOCK_BUCKETS {
            self.lock_cal.resize(LOCK_BUCKETS, 0);
        }
        self.lock_cal[i] = term;
    }
}

impl Default for PoolState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn previews_leave_the_state_alone() {
        let mut pool = PoolState::new();
        let none = ShareEpochs::new();
        let lender = LenderState::new_in(&pool);
        assert_eq!(
            preview_deposit(&pool, &lender, &none, 0, 1, false).unwrap(),
            1_000
        );
        // Deposit 10⁹ for real (mirrors what the program writes back).
        let (mut p, mut v, mut l) = (pool.pool(), pool.vault(), lender.lender());
        mpool::deposit(&mut p, &mut v, &mut l, &mut [], 0, 1_000_000_000, false).unwrap();
        pool.total_shares = p.total_shares;
        pool.tokens_free = p.tokens_free;
        let lender = LenderState::of(&l);
        assert_eq!(
            preview_allowance(&pool, &lender, &none, 0).unwrap(),
            1_000_000_000
        );
        assert_eq!(
            preview_withdraw(&pool, &lender, &none, 0, 86_400, lender.shares).unwrap(),
            1_000_000_000
        );
        assert_eq!(pool.tokens_free, 1_000_000_000);
        let w = preview_withdrawal(&pool, &lender, &none, 0, 86_400, lender.shares).unwrap();
        assert_eq!(
            (w.tokens, w.sol, w.last_shares_borrowed),
            (1_000_000_000, 0, false)
        );
        assert_eq!((w.lender.shares, w.lender.lock), (0, 0));
        assert_eq!(pool.total_shares, lender.shares);
        assert_eq!(preview_claim(&pool, &lender, &none, 0).unwrap(), 0);
        assert_eq!(
            tokens_for_shares(&pool, lender.shares).unwrap(),
            1_000_000_000
        );
        assert_eq!(preview_lock(&pool, &lender, &none, 0).unwrap(), 0);
        assert_eq!(LenderState::no_lock_epoch(), u32::MAX);
        pool.set_lock_bucket(3, 7);
        assert_eq!(pool.vault().lock_cal[3], 7);
    }

    /// KI-65: a lender behind a rebase record converts with the record's `conv_rem`
    /// (here the unit lands on it: 600 + 600 ≥ 1,000), and a preview never writes it.
    #[test]
    fn previews_convert_on_a_copy_of_the_records() {
        let mut pool = PoolState::new();
        pool.share_epoch = 1;
        pool.total_shares = 1;
        let mut recs = ShareEpochs::new();
        recs.push(0, 0, 0, 1_200, 0, 0, 1, 1, 600);
        let mut lender = LenderState::new_in(&pool);
        (lender.epoch, lender.shares, lender.snap, lender.snap_r) = (0, 600, 0, 0);
        let before = recs.clone();
        for _ in 0..2 {
            let l = preview_touch(&pool, &lender, &recs, 0).unwrap();
            assert_eq!((l.epoch, l.shares), (1, 1));
        }
        assert_eq!(recs.0, before.0);
        recs.0[0].conv_rem = 0;
        assert_eq!(preview_touch(&pool, &lender, &recs, 0).unwrap().shares, 0);
    }

    #[test]
    fn vest_and_lock_until() {
        let mut pool = PoolState::new();
        pool.max_release = 1_000_000;
        // ⌈(1,000,000 + 86,400)/86,400⌉·86,400 = 13 days.
        assert_eq!(preview_lock_until(&pool, 0, 86_400).unwrap(), 13 * 86_400);
        assert_eq!(
            preview_lock_until(&pool, 2_000_000, 3_600).unwrap(),
            24 * 86_400
        );
        pool.total_shares = 1_000;
        (pool.vest_bal, pool.vest_last, pool.vest_end) = (100, 0, 100);
        assert_eq!(preview_vest_release(&pool, 50).unwrap(), 50);
        assert_eq!(preview_vest_release(&pool, 500).unwrap(), 100);
        assert_eq!(pool.vest_bal, 100);
        assert_eq!(preview_locked_total(&pool, 0).unwrap(), 0);
    }
}
