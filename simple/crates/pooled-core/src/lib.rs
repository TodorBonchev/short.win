//! Token-principal pools and actually paid SOL income for the independent pooled version.
//!
//! No price, oracle, route, rent receivable, vesting, or transaction authority exists here.
//! Account ownership, exact token/lamport balance changes and debt identity belong to callers.
//! Every public mutation stages copies and commits only after all checks pass.
//!
//! SOL reward fractions follow a prospective rule: retained while receipt shares remain;
//! a full signed withdrawal clears that receipt's sub-lamport fraction. The last holder,
//! only with no outstanding debt, receives all remaining tracked SOL dust. This is not
//! a migration or waiver of any older model's rent carry.
#![no_std]
#![forbid(unsafe_code)]

mod wide;
pub use wide::U256;

/// Fixed precision for paid-SOL income. No interest accrues as a function of time.
pub const SOL_SCALE: u128 = 1_000_000_000_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Overflow,
    InvalidState,
    WrongGeneration,
    ZeroAmount,
    NoShares,
    InsufficientShares,
    InsufficientLiquidity,
    LastSharesBorrowed,
    Wiped,
    Retired,
    NotWiped,
    NotRetired,
}
pub type Result<T> = core::result::Result<T, Error>;

/// Runtime-serializable pool primitives. The program supplies account/PDA identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pool {
    pub generation: u64,
    pub cash: u64,
    pub principal_out: u64,
    pub shares: u128,
    pub loan_count: u32,
    pub sol_reserve: u64,
    pub sol_index: [u64; 4],
    pub retired: bool,
}
impl Pool {
    pub const fn new(generation: u64) -> Self {
        Self {
            generation,
            cash: 0,
            principal_out: 0,
            shares: 0,
            loan_count: 0,
            sol_reserve: 0,
            sol_index: [0; 4],
            retired: false,
        }
    }
    pub fn principal_assets(&self) -> Result<u64> {
        self.cash
            .checked_add(self.principal_out)
            .ok_or(Error::Overflow)
    }
    pub fn validate(&self) -> Result<()> {
        self.principal_assets()?;
        if (self.principal_out == 0) != (self.loan_count == 0)
            || u64::from(self.loan_count) > self.principal_out
            || (self.shares == 0
                && (self.cash != 0 || self.principal_out != 0 || self.sol_reserve != 0))
            || (self.retired && (self.cash != 0 || self.principal_out != 0 || self.loan_count != 0))
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
}
/// A lender's receipt in exactly one generation, including fractional paid SOL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Lender {
    pub generation: u64,
    pub shares: u128,
    pub checkpoint: [u64; 4],
    pub fraction: u128,
    pub pending_sol: u64,
}
impl Lender {
    pub const fn new(generation: u64) -> Self {
        Self {
            generation,
            shares: 0,
            checkpoint: [0; 4],
            fraction: 0,
            pending_sol: 0,
        }
    }
    pub fn validate(&self, pool: &Pool) -> Result<()> {
        if self.generation != pool.generation {
            return Err(Error::WrongGeneration);
        }
        if self.shares > pool.shares
            || self.fraction >= SOL_SCALE
            || self.pending_sol > pool.sol_reserve
            || U256::from_limbs(self.checkpoint) > U256::from_limbs(pool.sol_index)
            || (self.shares == 0 && self.fraction != 0)
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Withdrawal {
    pub tokens: u64,
    pub sol: u64,
}

fn active(pool: &Pool) -> Result<()> {
    pool.validate()?;
    if pool.retired {
        return Err(Error::Retired);
    }
    Ok(())
}
fn settle(pool: &Pool, lender: &mut Lender) -> Result<()> {
    lender.validate(pool)?;
    let index = U256::from_limbs(pool.sol_index);
    let delta = index
        .checked_sub(U256::from_limbs(lender.checkpoint))
        .ok_or(Error::InvalidState)?;
    let numerator = delta
        .checked_mul(U256::from_u128(lender.shares))
        .ok_or(Error::Overflow)?
        .checked_add(U256::from_u128(lender.fraction))
        .ok_or(Error::Overflow)?;
    let (whole, fraction) = numerator.div_rem_u128(SOL_SCALE).ok_or(Error::Overflow)?;
    lender.pending_sol = lender
        .pending_sol
        .checked_add(whole.to_u64().ok_or(Error::Overflow)?)
        .ok_or(Error::Overflow)?;
    lender.fraction = fraction;
    lender.checkpoint = pool.sol_index;
    if lender.pending_sol > pool.sol_reserve {
        return Err(Error::InvalidState);
    }
    Ok(())
}
fn income(pool: &mut Pool, amount: u64) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    if pool.shares == 0 {
        return Err(Error::NoShares);
    }
    let increment = U256::from_product(u128::from(amount), SOL_SCALE)
        .checked_div_u128(pool.shares)
        .ok_or(Error::Overflow)?;
    pool.sol_index = U256::from_limbs(pool.sol_index)
        .checked_add(increment)
        .ok_or(Error::Overflow)?
        .to_limbs();
    pool.sol_reserve = pool
        .sol_reserve
        .checked_add(amount)
        .ok_or(Error::Overflow)?;
    Ok(())
}
/// Credit only SOL already actually paid to this pool. No future income is booked.
pub fn credit_income(pool: &mut Pool, amount: u64) -> Result<()> {
    active(pool)?;
    let mut staged = *pool;
    income(&mut staged, amount)?;
    staged.validate()?;
    *pool = staged;
    Ok(())
}
/// Credit new net donated/surplus tokens to the current holders, after external checks.
pub fn credit_tokens(pool: &mut Pool, amount: u64) -> Result<()> {
    active(pool)?;
    if amount > 0 && pool.shares == 0 {
        return Err(Error::NoShares);
    }
    let mut staged = *pool;
    staged.cash = staged.cash.checked_add(amount).ok_or(Error::Overflow)?;
    staged.validate()?;
    *pool = staged;
    Ok(())
}
/// Alias naming actual surplus received after a full buyback; no debt is created.
pub fn donate_tokens(pool: &mut Pool, amount: u64) -> Result<()> {
    credit_tokens(pool, amount)
}

/// Mint floor(tokens*S/assets), or one share per raw token for the initial cohort.
pub fn deposit(pool: &mut Pool, lender: &mut Lender, tokens: u64) -> Result<u128> {
    active(pool)?;
    if tokens == 0 {
        return Err(Error::ZeroAmount);
    }
    let (mut p, mut l) = (*pool, *lender);
    settle(&p, &mut l)?;
    let assets = p.principal_assets()?;
    let minted = if p.shares == 0 {
        u128::from(tokens)
    } else {
        if assets == 0 {
            return Err(Error::Wiped);
        }
        U256::from_product(u128::from(tokens), p.shares)
            .checked_div_u128(u128::from(assets))
            .and_then(U256::to_u128)
            .ok_or(Error::Overflow)?
    };
    if minted == 0 {
        return Err(Error::ZeroAmount);
    }
    p.cash = p.cash.checked_add(tokens).ok_or(Error::Overflow)?;
    p.shares = p.shares.checked_add(minted).ok_or(Error::Overflow)?;
    l.shares = l.shares.checked_add(minted).ok_or(Error::Overflow)?;
    p.validate()?;
    l.validate(&p)?;
    *pool = p;
    *lender = l;
    Ok(minted)
}
/// Burn shares for their floor token-principal claim and all realized SOL income.
/// It never pays unavailable tokens or lets the final holder abandon outstanding debt.
pub fn withdraw(pool: &mut Pool, lender: &mut Lender, shares: u128) -> Result<Withdrawal> {
    pool.validate()?;
    if shares == 0 {
        return Err(Error::ZeroAmount);
    }
    let (mut p, mut l) = (*pool, *lender);
    settle(&p, &mut l)?;
    if shares > l.shares {
        return Err(Error::InsufficientShares);
    }
    let last = shares == p.shares;
    if last && (p.principal_out != 0 || p.loan_count != 0) {
        return Err(Error::LastSharesBorrowed);
    }
    let tokens = U256::from_product(shares, u128::from(p.principal_assets()?))
        .checked_div_u128(p.shares)
        .and_then(U256::to_u64)
        .ok_or(Error::Overflow)?;
    if tokens > p.cash {
        return Err(Error::InsufficientLiquidity);
    }
    let sol = if last { p.sol_reserve } else { l.pending_sol };
    p.cash = p.cash.checked_sub(tokens).ok_or(Error::InvalidState)?;
    p.sol_reserve = p.sol_reserve.checked_sub(sol).ok_or(Error::InvalidState)?;
    p.shares = p.shares.checked_sub(shares).ok_or(Error::InvalidState)?;
    l.shares = l.shares.checked_sub(shares).ok_or(Error::InvalidState)?;
    l.pending_sol = 0;
    if l.shares == 0 {
        l.fraction = 0;
    }
    p.validate()?;
    l.validate(&p)?;
    *pool = p;
    *lender = l;
    Ok(Withdrawal { tokens, sol })
}
/// Claim whole realized SOL. Fractional entitlement remains while shares remain.
pub fn claim(pool: &mut Pool, lender: &mut Lender) -> Result<u64> {
    pool.validate()?;
    let (mut p, mut l) = (*pool, *lender);
    settle(&p, &mut l)?;
    let sol = l.pending_sol;
    p.sol_reserve = p.sol_reserve.checked_sub(sol).ok_or(Error::InvalidState)?;
    l.pending_sol = 0;
    p.validate()?;
    l.validate(&p)?;
    *pool = p;
    *lender = l;
    Ok(sol)
}
/// Move actual pool principal into one positive-quantity loan. Caller persists its identity.
pub fn borrow(pool: &mut Pool, quantity: u64) -> Result<()> {
    active(pool)?;
    if quantity == 0 {
        return Err(Error::ZeroAmount);
    }
    if pool.shares == 0 {
        return Err(Error::NoShares);
    }
    let mut p = *pool;
    p.cash = p
        .cash
        .checked_sub(quantity)
        .ok_or(Error::InsufficientLiquidity)?;
    p.principal_out = p
        .principal_out
        .checked_add(quantity)
        .ok_or(Error::Overflow)?;
    p.loan_count = p.loan_count.checked_add(1).ok_or(Error::Overflow)?;
    p.validate()?;
    *pool = p;
    Ok(())
}
fn retire_principal(pool: &mut Pool, quantity: u64) -> Result<()> {
    if quantity == 0 {
        return Err(Error::ZeroAmount);
    }
    pool.principal_out = pool
        .principal_out
        .checked_sub(quantity)
        .ok_or(Error::InvalidState)?;
    pool.loan_count = pool.loan_count.checked_sub(1).ok_or(Error::InvalidState)?;
    Ok(())
}
/// After actual full-q net pool delivery, retire that caller-bound position's debt.
pub fn repay(pool: &mut Pool, quantity: u64) -> Result<()> {
    active(pool)?;
    let mut p = *pool;
    retire_principal(&mut p, quantity)?;
    p.cash = p.cash.checked_add(quantity).ok_or(Error::Overflow)?;
    p.validate()?;
    *pool = p;
    Ok(())
}
/// Whole-position principal loss plus actually received SOL recovery, never a token repayment.
pub fn default(pool: &mut Pool, quantity: u64, recovered_sol: u64) -> Result<()> {
    active(pool)?;
    let mut p = *pool;
    retire_principal(&mut p, quantity)?;
    income(&mut p, recovered_sol)?;
    p.validate()?;
    *pool = p;
    Ok(())
}
/// Retire a fully wiped, debt-free pool. Old receipts still withdraw/claim its SOL.
pub fn retire_wiped(pool: &mut Pool) -> Result<()> {
    active(pool)?;
    if pool.principal_assets()? != 0 || pool.loan_count != 0 || pool.shares == 0 {
        return Err(Error::NotWiped);
    }
    let mut p = *pool;
    p.retired = true;
    p.validate()?;
    *pool = p;
    Ok(())
}
/// Create a separate empty generation; no old SOL index, balance or fraction is copied.
pub fn next_generation(pool: &Pool) -> Result<Pool> {
    pool.validate()?;
    if !pool.retired {
        return Err(Error::NotRetired);
    }
    Ok(Pool::new(
        pool.generation.checked_add(1).ok_or(Error::Overflow)?,
    ))
}

/// Validate persisted pool primitives before a caller performs external effects.
pub fn validate_pool(pool: &Pool) -> Result<()> {
    pool.validate()
}
/// Validate one persisted receipt and its exact generation against the pool.
pub fn validate_lender(pool: &Pool, lender: &Lender) -> Result<()> {
    pool.validate()?;
    lender.validate(pool)
}

#[cfg(test)]
mod tests;
