//! QuoteNativeV1 notional-weighted rent: no token quantity, token price or legacy rent state.
//!
//! `J = sum(WAD*dt, WAD*min(U,k)*dt/BPS, WAD*max(U-k,0)*dt/BPS)`.
//! WAD is divisible by BPS, so every increment is exact and unchanged by instruction
//! partitioning. A locked opening SOL notional weights the integral; no fake token price
//! is introduced. Rates, split, opening utilization/kink and minimum seconds are explicit.
//!
//! Required integration: accrue the old aggregate before each real open/exit; keep this
//! basis/aggregate/carry separate from legacy Market rent fields; add/remove exactly the
//! same position once; preserve carry across membership changes and empty intervals; book
//! continuous lender credit through common-pool accrue/receivable; on an authorized exit,
//! retire min(credited_for,receivable), realize the paid lender rent, debit any shortfall
//! immediately and vest the minimum-rent top-up through the existing pool hooks. This module
//! cannot prove position membership, actual payment, exit eligibility or price qualification.
//! The carry denominator is numerically equal to legacy rent's denominator, but this
//! module provides no adoption/conversion authority. Restore only quote-native carry;
//! backend adoption requires zero or separately reconciled old carry and reviewed handling
//! of fractional ownership across share/epoch changes. Never silently drop or transplant it.
//! It supplies arithmetic only; it selects no default/maturity policy and is not program-wired.
pub mod persistence;

use crate::error::MathError;
use crate::num::{BPS, Rounding, U256, WAD, mul_div, to_u64};

/// Caller storage must bind this distinct contract version; there is no legacy conversion.
pub const NOTIONAL_RENT_VERSION: u16 = 1;
pub const YEAR_SECONDS: u64 = 31_536_000;
const RENT_DEN: u128 = 315_360_000_000_000_000_000_000_000_000;
/// Fixed denominator for a carried fractional lender lamport, never a financial default.
pub const LENDER_DENOMINATOR: u128 = 3_153_600_000_000_000_000_000_000_000_000_000;
const WAD_PER_BPS: u128 = 100_000_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpeningNotional(u64);
impl OpeningNotional {
    pub fn new(lamports: u64) -> Result<Self, MathError> {
        if lamports == 0 {
            Err(MathError::InvalidInput)
        } else {
            Ok(Self(lamports))
        }
    }
    pub const fn lamports(self) -> u64 {
        self.0
    }
}

/// Dimensionless locked APR coefficients, supplied explicitly; no risk-profile defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockedRates {
    coefficients: [u32; 3],
}
impl LockedRates {
    pub const fn new(base_bps: u32, slope1_bps: u32, slope2_bps: u32) -> Self {
        Self {
            coefficients: [base_bps, slope1_bps, slope2_bps],
        }
    }
    pub const fn coefficients(self) -> [u32; 3] {
        self.coefficients
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Basis {
    time_util_wad: [u128; 3],
}
impl Basis {
    pub const fn zero() -> Self {
        Self {
            time_util_wad: [0; 3],
        }
    }
    /// Restore already-quote-native versioned storage only. Validation of numbers does not
    /// authorize conversion from any legacy price-weighted Basis, even if its fields align.
    pub fn from_indices(indices: [u128; 3]) -> Result<Self, MathError> {
        if !indices[0].is_multiple_of(WAD)
            || !indices[1].is_multiple_of(WAD_PER_BPS)
            || !indices[2].is_multiple_of(WAD_PER_BPS)
            || indices[1]
                .checked_add(indices[2])
                .is_none_or(|sum| sum > indices[0])
        {
            return Err(MathError::InvalidInput);
        }
        Ok(Self {
            time_util_wad: indices,
        })
    }
    pub const fn indices(self) -> [u128; 3] {
        self.time_util_wad
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotionalPosition {
    opening_notional: OpeningNotional,
    open_basis: Basis,
    rates: LockedRates,
    kink_bps: u32,
    protocol_bps: u32,
    minimum_rent: u64,
}
impl NotionalPosition {
    /// Restore versioned quote-native position fields only. The authenticated account
    /// boundary must bind the actual position/market/model and its immutable opening
    /// terms. In particular, a structurally valid minimum is not proof that it was
    /// derived from the qualified opening notional/utilization at opening time.
    pub fn from_parts(
        opening_notional: OpeningNotional,
        open_basis: Basis,
        rates: LockedRates,
        kink_bps: u32,
        protocol_bps: u32,
        minimum_rent: u64,
    ) -> Result<Self, MathError> {
        lender_share(protocol_bps)?;
        split_util(0, kink_bps)?;
        // Validate the stored basis independently of how it was constructed.
        Basis::from_indices(open_basis.indices())?;
        Ok(Self {
            opening_notional,
            open_basis,
            rates,
            kink_bps,
            protocol_bps,
            minimum_rent,
        })
    }
    /// A checkpoint must not precede this position's opening basis or imply more
    /// utilized time than elapsed time. Membership and the locked kink remain caller
    /// obligations; a numerically valid checkpoint is not authenticated storage.
    pub fn validate_at_basis(&self, basis: &Basis) -> Result<(), MathError> {
        let mut delta = [0; 3];
        for ((value, current), open) in delta
            .iter_mut()
            .zip(basis.indices())
            .zip(self.open_basis.indices())
        {
            *value = current.checked_sub(open).ok_or(MathError::Underflow)?;
        }
        if delta[1].checked_add(delta[2]).ok_or(MathError::Overflow)? > delta[0] {
            return Err(MathError::InvalidInput);
        }
        Ok(())
    }
    /// Lock after the common pool accrued through the actual opening time. Caller must
    /// obtain booked N_open from qualified opening accounting, never from raw token q.
    pub fn lock(
        notional: OpeningNotional,
        basis: &Basis,
        rates: LockedRates,
        protocol_bps: u32,
        opening_util_bps: u32,
        kink_bps: u32,
        minimum_seconds: u64,
    ) -> Result<Self, MathError> {
        lender_share(protocol_bps)?;
        let minimum_rent = minimum(notional, rates, opening_util_bps, kink_bps, minimum_seconds)?;
        Ok(Self {
            opening_notional: notional,
            open_basis: *basis,
            rates,
            kink_bps,
            protocol_bps,
            minimum_rent,
        })
    }
    pub const fn opening_notional(self) -> OpeningNotional {
        self.opening_notional
    }
    pub const fn rates(self) -> LockedRates {
        self.rates
    }
    pub const fn kink_bps(self) -> u32 {
        self.kink_bps
    }
    pub const fn protocol_bps(self) -> u32 {
        self.protocol_bps
    }
    pub const fn minimum_rent(self) -> u64 {
        self.minimum_rent
    }
    pub const fn opening_basis(self) -> Basis {
        self.open_basis
    }
}

/// Sum N_open*locked_coefficient*(BPS-locked_protocol_share). No token debt lives here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotionalAggregate {
    folded_lender_weights: [u128; 3],
    positions: u64,
    opening_notional_total: u128,
    kink_bps: Option<u32>,
}
impl NotionalAggregate {
    /// Restore a versioned quote-native aggregate. These structural bounds reject
    /// impossible totals and ghost weights in an empty pool, but do not establish
    /// real position membership or the sum of authenticated position contributions.
    pub fn from_parts(
        folded_lender_weights: [u128; 3],
        positions: u64,
        opening_notional_total: u128,
        kink_bps: Option<u32>,
    ) -> Result<Self, MathError> {
        if positions == 0 {
            if folded_lender_weights != [0; 3] || opening_notional_total != 0 || kink_bps.is_some()
            {
                return Err(MathError::InvalidInput);
            }
            return Ok(Self::empty());
        }
        let kink = kink_bps.ok_or(MathError::InvalidInput)?;
        split_util(0, kink)?;
        let count = u128::from(positions);
        let max_total = count
            .checked_mul(u128::from(u64::MAX))
            .ok_or(MathError::Overflow)?;
        if opening_notional_total < count || opening_notional_total > max_total {
            return Err(MathError::InvalidInput);
        }
        // Each coefficient is u32 and each lender share is at most BPS. If this
        // mathematical bound exceeds u128, every representable weight is in range;
        // saturating only this comparison bound is exact, not financial arithmetic.
        let max_weight = opening_notional_total.saturating_mul(
            u128::from(u32::MAX)
                .checked_mul(u128::from(BPS))
                .ok_or(MathError::Overflow)?,
        );
        if folded_lender_weights.iter().any(|weight| {
            *weight > max_weight
                || (positions == 1 && !weight.is_multiple_of(opening_notional_total))
        }) {
            return Err(MathError::InvalidInput);
        }
        Ok(Self {
            folded_lender_weights,
            positions,
            opening_notional_total,
            kink_bps: Some(kink),
        })
    }
    pub const fn empty() -> Self {
        Self {
            folded_lender_weights: [0; 3],
            positions: 0,
            opening_notional_total: 0,
            kink_bps: None,
        }
    }
    pub const fn kink_bps(self) -> Option<u32> {
        self.kink_bps
    }
    pub const fn positions(self) -> u64 {
        self.positions
    }
    pub const fn opening_notional_total(self) -> u128 {
        self.opening_notional_total
    }
    pub const fn folded_lender_weights(self) -> [u128; 3] {
        self.folded_lender_weights
    }
    pub fn add(&mut self, position: &NotionalPosition) -> Result<(), MathError> {
        if self.positions != 0 && self.kink_bps != Some(position.kink_bps) {
            return Err(MathError::InvalidInput);
        }
        let contribution = folded(position)?;
        let count = self.positions.checked_add(1).ok_or(MathError::Overflow)?;
        let total = self
            .opening_notional_total
            .checked_add(u128::from(position.opening_notional.0))
            .ok_or(MathError::Overflow)?;
        let mut weights = self.folded_lender_weights;
        for (value, item) in weights.iter_mut().zip(contribution) {
            *value = value.checked_add(item).ok_or(MathError::Overflow)?;
        }
        self.folded_lender_weights = weights;
        self.positions = count;
        self.opening_notional_total = total;
        self.kink_bps = Some(position.kink_bps);
        Ok(())
    }
    /// Caller must prove real membership and pair this atomically with actual position exit.
    /// Aggregate arithmetic cannot distinguish two positions with identical contributions.
    pub fn remove(&mut self, position: &NotionalPosition) -> Result<(), MathError> {
        let contribution = folded(position)?;
        let count = self.positions.checked_sub(1).ok_or(MathError::Underflow)?;
        if self.kink_bps != Some(position.kink_bps) {
            return Err(MathError::InvalidInput);
        }
        let total = self
            .opening_notional_total
            .checked_sub(u128::from(position.opening_notional.0))
            .ok_or(MathError::Underflow)?;
        let mut weights = self.folded_lender_weights;
        for (value, item) in weights.iter_mut().zip(contribution) {
            *value = value.checked_sub(item).ok_or(MathError::Underflow)?;
        }
        let restored = Self::from_parts(
            weights,
            count,
            total,
            if count == 0 {
                None
            } else {
                Some(position.kink_bps)
            },
        )?;
        *self = restored;
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LenderCarry(u128);
impl LenderCarry {
    pub const fn zero() -> Self {
        Self(0)
    }
    /// Restore already-quote-native state only; this does not convert legacy carry.
    pub fn from_numerator(value: u128) -> Result<Self, MathError> {
        if value >= LENDER_DENOMINATOR {
            Err(MathError::InvalidInput)
        } else {
            Ok(Self(value))
        }
    }
    pub const fn numerator(self) -> u128 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accrual {
    pub lender_lamports: u64,
    delta: [u128; 3],
}
impl Accrual {
    pub const fn basis_delta(self) -> [u128; 3] {
        self.delta
    }
}

fn split_util(util_bps: u32, kink_bps: u32) -> Result<(u32, u32), MathError> {
    if u64::from(util_bps) > BPS || u64::from(kink_bps) > BPS {
        return Err(MathError::InvalidInput);
    }
    Ok((util_bps.min(kink_bps), util_bps.saturating_sub(kink_bps)))
}
fn lender_share(protocol_bps: u32) -> Result<u128, MathError> {
    u128::from(BPS)
        .checked_sub(u128::from(protocol_bps))
        .ok_or(MathError::InvalidInput)
}
fn folded(position: &NotionalPosition) -> Result<[u128; 3], MathError> {
    let share = lender_share(position.protocol_bps)?;
    let mut out = [0; 3];
    for (value, coefficient) in out.iter_mut().zip(position.rates.coefficients) {
        *value = u128::from(position.opening_notional.0)
            .checked_mul(u128::from(coefficient))
            .and_then(|n| n.checked_mul(share))
            .ok_or(MathError::Overflow)?;
    }
    Ok(out)
}
fn exact_increment(util_bps: u32, kink_bps: u32, seconds: u64) -> Result<[u128; 3], MathError> {
    let (below, above) = split_util(util_bps, kink_bps)?;
    let time = u128::from(seconds);
    let d0 = WAD.checked_mul(time).ok_or(MathError::Overflow)?;
    let d1 = WAD_PER_BPS
        .checked_mul(u128::from(below))
        .and_then(|n| n.checked_mul(time))
        .ok_or(MathError::Overflow)?;
    let d2 = WAD_PER_BPS
        .checked_mul(u128::from(above))
        .and_then(|n| n.checked_mul(time))
        .ok_or(MathError::Overflow)?;
    Ok([d0, d1, d2])
}
fn dot_sum(a: [u128; 3], b: [u128; 3], remainder: u128) -> Result<U256, MathError> {
    a.into_iter()
        .zip(b)
        .try_fold(U256::from_u128(remainder), |sum, (x, y)| {
            sum.checked_add(U256::from_product(x, y))
        })
        .ok_or(MathError::Overflow)
}
fn rounded(numerator: U256, denominator: u128, rounding: Rounding) -> Result<u64, MathError> {
    let (whole, remainder) = numerator
        .div_rem_u128(denominator)
        .ok_or(MathError::DivByZero)?;
    let whole = if rounding == Rounding::Ceil && remainder != 0 {
        whole.checked_add(U256::ONE).ok_or(MathError::Overflow)?
    } else {
        whole
    };
    whole.to_u64().ok_or(MathError::Overflow)
}
fn position_numerator(position: &NotionalPosition, basis: &Basis) -> Result<U256, MathError> {
    let mut delta = [0; 3];
    let mut weights = [0; 3];
    for (((d, w), now), (open, coefficient)) in delta
        .iter_mut()
        .zip(weights.iter_mut())
        .zip(basis.time_util_wad)
        .zip(
            position
                .open_basis
                .time_util_wad
                .into_iter()
                .zip(position.rates.coefficients),
        )
    {
        *d = now.checked_sub(open).ok_or(MathError::Underflow)?;
        *w = u128::from(position.opening_notional.0)
            .checked_mul(u128::from(coefficient))
            .ok_or(MathError::Overflow)?;
    }
    if delta[1].checked_add(delta[2]).ok_or(MathError::Overflow)? > delta[0] {
        return Err(MathError::InvalidInput);
    }
    dot_sum(weights, delta, 0)
}

/// Preflight basis and carry before either mutation. A returned error preserves both.
/// Caller must book lender_lamports through pool::accrue in the same atomic transition.
pub fn accrue(
    basis: &mut Basis,
    aggregate: &NotionalAggregate,
    carry: &mut LenderCarry,
    util_bps: u32,
    kink_bps: u32,
    seconds: u64,
) -> Result<Accrual, MathError> {
    if aggregate.positions != 0 && aggregate.kink_bps != Some(kink_bps) {
        return Err(MathError::InvalidInput);
    }
    if carry.0 >= LENDER_DENOMINATOR {
        return Err(MathError::InvalidInput);
    }
    let delta = exact_increment(util_bps, kink_bps, seconds)?;
    let mut indices = basis.time_util_wad;
    for (index, amount) in indices.iter_mut().zip(delta) {
        *index = index.checked_add(amount).ok_or(MathError::Overflow)?;
    }
    let numerator = dot_sum(aggregate.folded_lender_weights, delta, carry.0)?;
    let (credit, remainder) = numerator
        .div_rem_u128(LENDER_DENOMINATOR)
        .ok_or(MathError::DivByZero)?;
    let lender_lamports = credit.to_u64().ok_or(MathError::Overflow)?;
    basis.time_util_wad = indices;
    carry.0 = remainder;
    Ok(Accrual {
        lender_lamports,
        delta,
    })
}
/// Ceil of locked opening notional's exact utilization-time integral, in lamports.
pub fn accrued(position: &NotionalPosition, basis: &Basis) -> Result<u64, MathError> {
    rounded(
        position_numerator(position, basis)?,
        RENT_DEN,
        Rounding::Ceil,
    )
}
pub fn due(position: &NotionalPosition, basis: &Basis) -> Result<u64, MathError> {
    Ok(accrued(position, basis)?.max(position.minimum_rent))
}
/// Ceiled lender integral used for per-position receivable retirement/true-up.
/// It is <= lender rent when full due is actually paid. Aggregate carried floors are
/// <= the sum of these ceils; neither statement guarantees full payment in a shortfall.
pub fn credited_for(position: &NotionalPosition, basis: &Basis) -> Result<u64, MathError> {
    let numerator = position_numerator(position, basis)?
        .checked_mul(U256::from_u128(lender_share(position.protocol_bps)?))
        .ok_or(MathError::Overflow)?;
    rounded(numerator, LENDER_DENOMINATOR, Rounding::Ceil)
}
/// Opening-utilization minimum, with WAD cancelled exactly before wide multiplication.
pub fn minimum(
    notional: OpeningNotional,
    rates: LockedRates,
    util_bps: u32,
    kink_bps: u32,
    seconds: u64,
) -> Result<u64, MathError> {
    let (below, above) = split_util(util_bps, kink_bps)?;
    let [base, low, high] = rates.coefficients;
    let rate = u128::from(base)
        .checked_mul(u128::from(BPS))
        .and_then(|n| {
            u128::from(low)
                .checked_mul(u128::from(below))
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            u128::from(high)
                .checked_mul(u128::from(above))
                .and_then(|m| n.checked_add(m))
        })
        .ok_or(MathError::Overflow)?;
    let numerator = U256::from_product(u128::from(notional.0), rate)
        .checked_mul(U256::from_u64(seconds))
        .ok_or(MathError::Overflow)?;
    let denominator = u128::from(BPS)
        .checked_mul(u128::from(BPS))
        .and_then(|n| n.checked_mul(u128::from(YEAR_SECONDS)))
        .ok_or(MathError::Overflow)?;
    rounded(numerator, denominator, Rounding::Ceil)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RentTrueUp {
    pub rent_due: u64,
    pub rent_paid: u64,
    pub protocol_rent: u64,
    pub lender_paid: u64,
    pub credited_for: u64,
    pub immediate_realized: u64,
    pub vest_top_up: u64,
    pub entitlement_adjustment: i128,
}
/// Arithmetic for an already-authorized ordinary rent payment, excluding settlement
/// principal, lump and default distributions. Partial payment does not authorize
/// debt extinguishment/default; no maturity/backstop policy is selected here.
pub fn true_up(
    position: &NotionalPosition,
    basis: &Basis,
    rent_paid: u64,
) -> Result<RentTrueUp, MathError> {
    let rent_due = due(position, basis)?;
    if rent_paid > rent_due {
        return Err(MathError::InvalidInput);
    }
    let protocol_rent = to_u64(mul_div(
        u128::from(rent_paid),
        u128::from(position.protocol_bps),
        u128::from(BPS),
        Rounding::Floor,
    )?)?;
    let lender_paid = rent_paid
        .checked_sub(protocol_rent)
        .ok_or(MathError::Underflow)?;
    let credited_for = credited_for(position, basis)?;
    let immediate_realized = lender_paid.min(credited_for);
    let vest_top_up = lender_paid
        .checked_sub(immediate_realized)
        .ok_or(MathError::Underflow)?;
    let entitlement_adjustment = i128::from(immediate_realized)
        .checked_sub(i128::from(credited_for))
        .ok_or(MathError::Underflow)?;
    Ok(RentTrueUp {
        rent_due,
        rent_paid,
        protocol_rent,
        lender_paid,
        credited_for,
        immediate_realized,
        vest_top_up,
        entitlement_adjustment,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use std::string::ToString;

    fn ceil_small(n: u128, d: u128) -> u64 {
        u64::try_from(n.div_ceil(d)).unwrap()
    }
    fn position(
        n: u64,
        rates: LockedRates,
        protocol: u32,
        util: u32,
        kink: u32,
        minimum_s: u64,
        basis: &Basis,
    ) -> NotionalPosition {
        NotionalPosition::lock(
            OpeningNotional::new(n).unwrap(),
            basis,
            rates,
            protocol,
            util,
            kink,
            minimum_s,
        )
        .unwrap()
    }
    fn rate(rates: LockedRates, u: u32, k: u32) -> u128 {
        let [b, l, h] = rates.coefficients();
        u128::from(b) * 10000
            + u128::from(l) * u128::from(u.min(k))
            + u128::from(h) * u128::from(u.saturating_sub(k))
    }
    fn big_ceil(n: BigUint, d: u128) -> Option<u64> {
        ((n + BigUint::from(d - 1)) / BigUint::from(d))
            .to_string()
            .parse()
            .ok()
    }

    // Generated by Python integer/rational arithmetic (seed 103410003), independent of
    // the Rust wide-int and basis implementation. Inputs/expected values are retained below.
    type Vector = (
        u64,
        [u32; 3],
        u32,
        u32,
        u64,
        u64,
        u32,
        u64,
        u64,
        u64,
        u64,
        u128,
    );
    const PYTHON_VECTORS: &[Vector] = &[
        (
            18446744073709551615,
            [10000, 0, 0],
            0,
            8000,
            1,
            0,
            1500,
            0,
            584942417356,
            497201054752,
            497201054751,
            2558287275000000000000000000000000,
        ),
        (
            1,
            [1, 0, 0],
            0,
            0,
            18446744073709551615,
            0,
            0,
            0,
            58494242,
            58494242,
            58494241,
            2319495516150000000000000000000000,
        ),
        (
            1000000000000,
            [10000, 37500, 550000],
            9999,
            8000,
            10000000000,
            14400,
            1500,
            6846803653,
            4754724759005581,
            4041516045154744,
            4041516045154743,
            2475200000000000000000000000000000,
        ),
        (
            9297858123201509,
            [389701, 549715, 101023],
            4985,
            3532,
            1911320,
            89127,
            279,
            1572816424174571,
            33728897952958588,
            32787861700071043,
            32787861700071042,
            2633700008519004852000000000000000,
        ),
        (
            2909215708921153,
            [331310, 569528, 849983],
            6554,
            1386,
            6040839,
            84365,
            1769,
            661156251209511,
            47341177827300526,
            38966523469651063,
            38966523469651062,
            2282084325200021270400000000000000,
        ),
        (
            2688846850075479,
            [623777, 389281, 677780],
            8084,
            4510,
            4779411,
            77663,
            6988,
            689710436688472,
            42445046520526961,
            12784448011982721,
            12784448011982720,
            1916569627797780684000000000000000,
        ),
        (
            2666635912402696,
            [8401, 748056, 789684],
            151,
            9949,
            8133066,
            95126,
            1553,
            15843408596922,
            1354576958809660,
            1144211157106520,
            1144211157106519,
            1808546444785762675200000000000000,
        ),
        (
            3218414135227406,
            [189835, 598025, 817961],
            4461,
            8172,
            3137980,
            28878,
            242,
            134571040371282,
            14622939028474050,
            14269063903984978,
            14269063903984977,
            2026800621628780600000000000000000,
        ),
        (
            6809007347936650,
            [126362, 145090, 254755],
            5493,
            9,
            6728660,
            28947,
            1744,
            166375422745140,
            38673563823826602,
            31928894292951243,
            31928894292951242,
            1669615088855232000000000000000000,
        ),
        (
            4173811718359983,
            [794023, 378910, 731722],
            5848,
            4406,
            4255075,
            66108,
            5106,
            933114720648383,
            60060402976385830,
            29393561216643225,
            29393561216643224,
            2958784569620126760000000000000000,
        ),
        (
            7340761612363562,
            [105226, 437514, 525602],
            7429,
            246,
            3903397,
            20071,
            9751,
            230577003845576,
            44842488420098986,
            1116577961660465,
            1116577961660464,
            2291975180738389746000000000000000,
        ),
        (
            7452805141451923,
            [350631, 651995, 23804],
            6473,
            7265,
            9198356,
            57589,
            2763,
            1051587137585052,
            167963896864475544,
            121555472160820951,
            121555472160820950,
            2635854948485654906000000000000000,
        ),
        (
            2365924301159510,
            [291521, 749032, 537019],
            5502,
            3770,
            3842440,
            14911,
            301,
            74605919578253,
            19225321549477680,
            18646639370838402,
            18646639370838401,
            1580004402424262880000000000000000,
        ),
        (
            4198683116397377,
            [855615, 532767, 380218],
            3009,
            9894,
            4420162,
            95000,
            4341,
            1284965802125906,
            59786915893225759,
            33833415703976457,
            33833415703976456,
            1886452763797387069800000000000000,
        ),
        (
            3365760539492584,
            [454860, 568259, 143291],
            4914,
            4623,
            5420886,
            43669,
            1798,
            336378449813311,
            41756606043066665,
            34248768276523279,
            34248768276523278,
            150211982888406662400000000000000,
        ),
        (
            6579630290344611,
            [60440, 34818, 831209],
            8986,
            1212,
            7173502,
            58802,
            3330,
            872087278166705,
            106389575764487797,
            70961847034913361,
            70961847034913360,
            1685983275689196468000000000000000,
        ),
        (
            2899846440481210,
            [591730, 56798, 497636],
            805,
            4177,
            1317522,
            25418,
            8227,
            139372217240989,
            7224249052001786,
            1280859356919917,
            1280859356919916,
            1523340368024710140000000000000000,
        ),
        (
            1602352648598974,
            [247275, 136373, 4251],
            1695,
            8924,
            6436299,
            47539,
            9276,
            65311930717954,
            8842573768233130,
            640202340820079,
            640202340820078,
            1875954928921127964000000000000000,
        ),
        (
            5472880797564060,
            [638926, 343315, 875874],
            501,
            4539,
            9148987,
            9053,
            193,
            103083511482460,
            104176483648224306,
            102165877513813577,
            102165877513813576,
            669479802325073010000000000000000,
        ),
        (
            2413021113789667,
            [80245, 253613, 286088],
            7895,
            8844,
            9054786,
            77483,
            1433,
            166284271304911,
            19432243096316626,
            16647602660614454,
            16647602660614453,
            262504401847270779000000000000000,
        ),
        (
            1157822295368546,
            [369005, 724329, 155185],
            9275,
            2977,
            1147290,
            54486,
            2512,
            136503009013107,
            2874289491073797,
            2152267970916059,
            2152267970916058,
            2307363786062695296000000000000000,
        ),
        (
            1666335229946331,
            [750063, 911969, 151155],
            4710,
            5246,
            5233596,
            16918,
            5010,
            105448328704777,
            32620519642747665,
            16277639301731085,
            16277639301731084,
            2311521887610878760000000000000000,
        ),
        (
            3312299133960569,
            [495560, 578764, 642362],
            7703,
            4784,
            5163702,
            84421,
            8815,
            851177851789345,
            52063216209714873,
            6169491120851213,
            6169491120851212,
            1360075347996161562000000000000000,
        ),
        (
            4173122307923443,
            [32311, 14747, 893608],
            3279,
            2146,
            5885712,
            56416,
            1829,
            102068936117456,
            10648545840430803,
            8700926806216009,
            8700926806216008,
            2581160861026161993600000000000000,
        ),
        (
            7041653180092447,
            [752937, 418134, 579810],
            2949,
            1400,
            1100567,
            13483,
            256,
            271342802391058,
            22148700882527545,
            21581694139934840,
            21581694139934839,
            1571601579715313424000000000000000,
        ),
        (
            3523799078284117,
            [980538, 868647, 293909],
            6183,
            5750,
            3508822,
            14871,
            3038,
            248043467491793,
            58526015445597972,
            40745811953225308,
            40745811953225307,
            2334580421345622163600000000000000,
        ),
        (
            468478232303915,
            [399952, 170059, 666094],
            6422,
            7838,
            7479528,
            41060,
            1432,
            31056989132576,
            5657370185406490,
            4847234774856281,
            4847234774856280,
            1237676259885749568000000000000000,
        ),
        (
            382508481686817,
            [135739, 755742, 324802],
            6245,
            7416,
            9391701,
            79215,
            9713,
            58389000228136,
            6922578196447359,
            198677994238040,
            198677994238039,
            595841250825465741000000000000000,
        ),
        (
            6257612147160406,
            [671862, 859970, 706499],
            7439,
            551,
            8957160,
            66988,
            1105,
            1602891655490568,
            214327297738309681,
            190644131338226462,
            190644131338226461,
            63361652860675440000000000000000,
        ),
        (
            5569619708678985,
            [174509, 438880, 458333],
            7847,
            132,
            3839301,
            78958,
            3012,
            744526188429239,
            36202286529073204,
            25298157826516355,
            25298157826516354,
            1702929781292688090000000000000000,
        ),
        (
            2366928795624477,
            [917594, 344913, 624980],
            3995,
            963,
            1815239,
            71759,
            9026,
            614151127093278,
            15535766632668709,
            1513183670021933,
            1513183670021932,
            777611860725508183800000000000000,
        ),
        (
            2308148680553107,
            [976713, 367764, 162377],
            1887,
            8210,
            8747337,
            63506,
            2152,
            486238467737440,
            66974643965342082,
            52561700584000466,
            52561700584000465,
            1423145673727976937600000000000000,
        ),
        (
            3499586510784606,
            [252434, 853107, 778855],
            6856,
            4424,
            8463729,
            35893,
            9146,
            326320874552657,
            76947913221426994,
            6571351789109866,
            6571351789109865,
            730534409840472508800000000000000,
        ),
        (
            3459920175944115,
            [1334, 677794, 892604],
            7437,
            6146,
            822647,
            23425,
            1607,
            137019208531002,
            4811886481980902,
            4038616324326571,
            4038616324326570,
            1648650666065961852000000000000000,
        ),
        (
            6507742657170530,
            [52103, 340924, 525615],
            9051,
            1192,
            32107,
            92733,
            7160,
            967956324520484,
            335136075737647,
            95178645509492,
            95178645509491,
            1872138376826168520000000000000000,
        ),
        (
            5091225839087706,
            [767887, 811510, 650280],
            3608,
            1556,
            405820,
            60515,
            9665,
            1003924260475962,
            6732422430576797,
            225536151424323,
            225536151424322,
            2176603216986938400000000000000000,
        ),
        (
            7889938915999793,
            [707121, 877376, 213516],
            2329,
            8544,
            8711943,
            47521,
            8002,
            1083655177669219,
            198664635414008662,
            39693194155718931,
            39693194155718930,
            1687788781301748700800000000000000,
        ),
        (
            2485162942216994,
            [543566, 256708, 962877],
            391,
            3696,
            4245547,
            76161,
            6602,
            332261144369552,
            18521688327289745,
            6293669693613056,
            6293669693613055,
            633424982965197259200000000000000,
        ),
    ];
    #[test]
    fn independent_python_wide_rational_vectors() {
        assert_eq!(RENT_DEN, 10000 * u128::from(YEAR_SECONDS) * WAD);
        assert_eq!(LENDER_DENOMINATOR, 10000 * RENT_DEN);
        assert_eq!(WAD, 10000 * WAD_PER_BPS);
        for &(
            n,
            c,
            u,
            k,
            seconds,
            minimum_s,
            protocol,
            expected_min,
            expected_ac,
            expected_credited,
            expected_credit,
            expected_carry,
        ) in PYTHON_VECTORS
        {
            let mut basis = Basis::zero();
            let rates = LockedRates { coefficients: c };
            let pos = position(n, rates, protocol, u, k, minimum_s, &basis);
            let mut aggregate = NotionalAggregate::empty();
            aggregate.add(&pos).unwrap();
            let mut carry = LenderCarry::zero();
            let step = accrue(&mut basis, &aggregate, &mut carry, u, k, seconds).unwrap();
            assert_eq!(pos.minimum_rent(), expected_min);
            assert_eq!(accrued(&pos, &basis), Ok(expected_ac));
            assert_eq!(due(&pos, &basis), Ok(expected_ac.max(expected_min)));
            assert_eq!(credited_for(&pos, &basis), Ok(expected_credited));
            assert_eq!(step.lender_lamports, expected_credit);
            assert_eq!(carry.numerator(), expected_carry);
            let paid = true_up(&pos, &basis, expected_ac.max(expected_min)).unwrap();
            assert!(paid.credited_for <= paid.lender_paid);
        }
    }
    #[test]
    fn exhaustive_small_states_and_every_payment_obey_independent_formula() {
        let denominator = 10000u128 * 10000 * u128::from(YEAR_SECONDS);
        for n in [1, 7, 100_000_000_000] {
            for coefficients in [[0, 0, 0], [1, 2, 3], [7, 11, 23]] {
                for (u, k) in [
                    (0, 0),
                    (1, 1),
                    (7999, 8000),
                    (8000, 8000),
                    (8001, 8000),
                    (10000, 8000),
                    (10000, 10000),
                ] {
                    for protocol in [0, 1, 3333, 9999, 10000] {
                        for seconds in 0..=7 {
                            let rates = LockedRates { coefficients };
                            let mut basis = Basis::zero();
                            let mut carry = LenderCarry::zero();
                            let pos = position(n, rates, protocol, u, k, 9, &basis);
                            let mut aggregate = NotionalAggregate::empty();
                            aggregate.add(&pos).unwrap();
                            let credit = accrue(&mut basis, &aggregate, &mut carry, u, k, seconds)
                                .unwrap()
                                .lender_lamports;
                            let num = u128::from(n) * rate(rates, u, k) * u128::from(seconds);
                            let expected = ceil_small(num, denominator);
                            let minimum =
                                ceil_small(u128::from(n) * rate(rates, u, k) * 9, denominator);
                            let cred =
                                ceil_small(num * u128::from(10000 - protocol), denominator * 10000);
                            assert_eq!(accrued(&pos, &basis), Ok(expected));
                            assert_eq!(credited_for(&pos, &basis), Ok(cred));
                            assert!(credit <= cred);
                            let total_due = expected.max(minimum);
                            for paid in [0, total_due / 2, total_due] {
                                let t = true_up(&pos, &basis, paid).unwrap();
                                assert_eq!(t.protocol_rent, paid * u64::from(protocol) / 10000);
                                assert_eq!(t.protocol_rent + t.lender_paid, paid);
                                assert_eq!(t.immediate_realized + t.vest_top_up, t.lender_paid);
                                assert_eq!(
                                    t.entitlement_adjustment + i128::from(cred),
                                    i128::from(t.immediate_realized)
                                );
                                assert!(t.entitlement_adjustment <= 0);
                                if paid == total_due {
                                    assert!(cred <= t.lender_paid);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn same_utilization_cadence_partition_has_identical_basis_credit_and_carry() {
        for u in [0, 1, 7999, 8000, 8001, 10000] {
            for total in 0..=32 {
                let start = Basis::zero();
                let rates = LockedRates::new(123, 4567, 98765);
                let pos = position(9_999_999_999_999, rates, 1500, u, 8000, 0, &start);
                let mut aggregate = NotionalAggregate::empty();
                aggregate.add(&pos).unwrap();
                let (mut whole, mut whole_rem) = (start, LenderCarry::zero());
                let expected = accrue(&mut whole, &aggregate, &mut whole_rem, u, 8000, total)
                    .unwrap()
                    .lender_lamports;
                for split in 0..=total {
                    let (mut pieces, mut remainder) = (start, LenderCarry::zero());
                    let a = accrue(&mut pieces, &aggregate, &mut remainder, u, 8000, split)
                        .unwrap()
                        .lender_lamports;
                    let b = accrue(
                        &mut pieces,
                        &aggregate,
                        &mut remainder,
                        u,
                        8000,
                        total - split,
                    )
                    .unwrap()
                    .lender_lamports;
                    assert_eq!(pieces, whole);
                    assert_eq!(remainder, whole_rem);
                    assert_eq!(a + b, expected);
                    assert_eq!(due(&pos, &pieces), due(&pos, &whole));
                }
            }
        }
    }
    #[test]
    fn arbitrary_precision_oracle_tracks_add_remove_util_changes_and_empty_intervals() {
        let mut basis = Basis::zero();
        let mut carry = LenderCarry::zero();
        let mut aggregate = NotionalAggregate::empty();
        let mut active = std::vec::Vec::<NotionalPosition>::new();
        let mut exact = BigUint::from(0u8);
        let mut credited_total = 0u128;
        let mut receivable = 0u64;
        let denominator = 10000u128 * 10000 * 10000 * u128::from(YEAR_SECONDS);
        for step in 0..200u32 {
            let u = (step * 7919) % 10001;
            let seconds = u64::from(step % 13);
            for pos in &active {
                exact += BigUint::from(pos.opening_notional.0)
                    * BigUint::from(rate(pos.rates, u, 8000))
                    * BigUint::from(seconds)
                    * BigUint::from(10000 - pos.protocol_bps);
            }
            let result = accrue(&mut basis, &aggregate, &mut carry, u, 8000, seconds).unwrap();
            credited_total += u128::from(result.lender_lamports);
            receivable += result.lender_lamports;
            assert_eq!(
                credited_total.to_string(),
                (&exact / BigUint::from(denominator)).to_string()
            );
            let expected_rem = (&exact % BigUint::from(denominator)) * BigUint::from(WAD_PER_BPS);
            assert_eq!(carry.numerator().to_string(), expected_rem.to_string());
            if step % 3 == 0 {
                let pos = position(
                    1_000_000_007 + u64::from(step) * 1_000_000,
                    LockedRates::new(101 + step, 503, 10007),
                    step % 10001,
                    u,
                    8000,
                    17,
                    &basis,
                );
                aggregate.add(&pos).unwrap();
                active.push(pos);
            }
            if step % 2 == 0 && !active.is_empty() {
                let pos = active.remove(0);
                let rent_due = due(&pos, &basis).unwrap();
                let payment = if step % 4 == 0 {
                    rent_due / 2
                } else {
                    rent_due
                };
                let t = true_up(&pos, &basis, payment).unwrap();
                receivable -= t.credited_for.min(receivable);
                aggregate.remove(&pos).unwrap();
            }
            let expected_count = u64::try_from(active.len()).unwrap();
            let expected_notional: u128 = active
                .iter()
                .map(|p| u128::from(p.opening_notional.0))
                .sum();
            assert_eq!(aggregate.positions(), expected_count);
            assert_eq!(aggregate.opening_notional_total(), expected_notional);
            if active.is_empty() {
                assert_eq!(receivable, 0);
            }
        }
        for pos in active {
            receivable -= credited_for(&pos, &basis).unwrap().min(receivable);
            aggregate.remove(&pos).unwrap();
        }
        assert_eq!(receivable, 0);
        assert_eq!(aggregate, NotionalAggregate::empty());
        let saved = carry;
        let result = accrue(&mut basis, &aggregate, &mut carry, 10000, 8000, 77).unwrap();
        assert_eq!(result.lender_lamports, 0);
        assert_eq!(carry, saved);
    }
    #[test]
    fn zero_minimum_full_rent_minimum_top_up_and_partial_shortfall_are_distinct() {
        let mut basis = Basis::zero();
        let pos = position(
            1000,
            LockedRates::new(10000, 0, 0),
            1500,
            0,
            8000,
            YEAR_SECONDS,
            &basis,
        );
        let early = true_up(&pos, &basis, 1000).unwrap();
        assert_eq!(
            (
                early.credited_for,
                early.protocol_rent,
                early.lender_paid,
                early.vest_top_up,
                early.entitlement_adjustment
            ),
            (0, 150, 850, 850, 0)
        );
        let mut aggregate = NotionalAggregate::empty();
        aggregate.add(&pos).unwrap();
        let mut carry = LenderCarry::zero();
        assert_eq!(
            accrue(&mut basis, &aggregate, &mut carry, 0, 8000, YEAR_SECONDS)
                .unwrap()
                .lender_lamports,
            850
        );
        let partial = true_up(&pos, &basis, 500).unwrap();
        assert_eq!(
            (
                partial.credited_for,
                partial.protocol_rent,
                partial.lender_paid,
                partial.immediate_realized,
                partial.vest_top_up,
                partial.entitlement_adjustment
            ),
            (850, 75, 425, 425, 0, -425)
        );
        assert_eq!(true_up(&pos, &basis, 1001), Err(MathError::InvalidInput));
    }
    #[test]
    fn wide_boundaries_and_all_error_paths_preserve_mutable_state() {
        assert_eq!(OpeningNotional::new(0), Err(MathError::InvalidInput));
        assert_eq!(
            NotionalPosition::lock(
                OpeningNotional::new(1).unwrap(),
                &Basis::zero(),
                LockedRates::new(1, 2, 3),
                10001,
                0,
                8000,
                0
            ),
            Err(MathError::InvalidInput)
        );
        assert_eq!(
            LenderCarry::from_numerator(LENDER_DENOMINATOR),
            Err(MathError::InvalidInput)
        );
        assert_eq!(Basis::from_indices([1, 0, 0]), Err(MathError::InvalidInput));
        assert_eq!(
            Basis::from_indices([WAD, WAD, WAD]),
            Err(MathError::InvalidInput)
        );
        let mut basis = Basis::zero();
        let rates = LockedRates::new(10000, 0, 0);
        let p = position(u64::MAX, rates, 0, 0, 8000, 0, &basis);
        let mut aggregate = NotionalAggregate::empty();
        aggregate.add(&p).unwrap();
        let mut carry = LenderCarry::zero();
        accrue(&mut basis, &aggregate, &mut carry, 0, 8000, YEAR_SECONDS).unwrap();
        assert_eq!(due(&p, &basis), Ok(u64::MAX));
        assert_eq!(
            minimum(
                OpeningNotional::new(u64::MAX).unwrap(),
                LockedRates::new(10001, 0, 0),
                0,
                8000,
                YEAR_SECONDS
            ),
            Err(MathError::Overflow)
        );
        assert_eq!(accrued(&p, &Basis::zero()), Ok(0));
        let later = position(1, rates, 0, 0, 8000, 0, &basis);
        assert_eq!(accrued(&later, &Basis::zero()), Err(MathError::Underflow));
        for (u, k, dt) in [(10001, 8000, 1), (1, 10001, 1), (10000, 8000, u64::MAX)] {
            let saved = (basis, carry);
            assert!(accrue(&mut basis, &aggregate, &mut carry, u, k, dt).is_err());
            assert_eq!((basis, carry), saved);
        }
        let mut near = Basis::from_indices([(u128::MAX / WAD) * WAD, 0, 0]).unwrap();
        let saved = (near, carry);
        assert_eq!(
            accrue(
                &mut near,
                &NotionalAggregate::empty(),
                &mut carry,
                0,
                8000,
                1
            ),
            Err(MathError::Overflow)
        );
        assert_eq!((near, carry), saved);
        let mut bad = NotionalAggregate {
            folded_lender_weights: [0, 0, u128::MAX],
            positions: 1,
            opening_notional_total: 1,
            kink_bps: Some(8000),
        };
        let p = position(2, LockedRates::new(1, 1, 1), 0, 0, 8000, 0, &Basis::zero());
        let saved = bad;
        assert_eq!(bad.add(&p), Err(MathError::Overflow));
        assert_eq!(bad, saved);
        let mut bad = NotionalAggregate {
            folded_lender_weights: [100000, 100000, 0],
            positions: 1,
            opening_notional_total: 2,
            kink_bps: Some(8000),
        };
        let saved = bad;
        assert_eq!(bad.remove(&p), Err(MathError::Underflow));
        assert_eq!(bad, saved);
    }
    #[test]
    fn seeded_arbitrary_precision_lifetimes_compare_wide_due_and_lender_ceil() {
        let mut seed = 0x103410003u64;
        for _ in 0..1000 {
            let mut draw = || {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                seed
            };
            let n = draw().max(1);
            let rates = LockedRates::new(
                u32::try_from(draw() % 1_000_000).unwrap(),
                u32::try_from(draw() % 1_000_000).unwrap(),
                u32::try_from(draw() % 1_000_000).unwrap(),
            );
            let protocol = u32::try_from(draw() % 10001).unwrap();
            let mut basis = Basis::zero();
            let pos = position(n, rates, protocol, 0, 8000, 0, &basis);
            let mut aggregate = NotionalAggregate::empty();
            aggregate.add(&pos).unwrap();
            let mut carry = LenderCarry::zero();
            let mut exact = BigUint::from(0u8);
            for _ in 0..5 {
                let u = u32::try_from(draw() % 10001).unwrap();
                let dt = draw() % 1_000_000;
                exact += BigUint::from(n) * BigUint::from(rate(rates, u, 8000)) * BigUint::from(dt);
                let before = (basis, carry);
                let expected_step = ((BigUint::from(n)
                    * BigUint::from(rate(rates, u, 8000))
                    * BigUint::from(dt)
                    * BigUint::from(10000 - protocol)
                    * BigUint::from(WAD_PER_BPS)
                    + BigUint::from(before.1.numerator()))
                    / BigUint::from(LENDER_DENOMINATOR))
                .to_string()
                .parse::<u64>()
                .ok();
                let step = accrue(&mut basis, &aggregate, &mut carry, u, 8000, dt);
                if step.is_err() {
                    assert_eq!(expected_step, None, "an in-range credit must not fail");
                    assert_eq!((basis, carry), before);
                    break;
                }
                assert_eq!(step.unwrap().lender_lamports, expected_step.unwrap());
                let expected =
                    big_ceil(exact.clone(), 10000u128 * 10000 * u128::from(YEAR_SECONDS));
                assert_eq!(accrued(&pos, &basis).ok(), expected);
                let expected = big_ceil(
                    &exact * BigUint::from(10000 - protocol),
                    10000u128 * 10000 * 10000 * u128::from(YEAR_SECONDS),
                );
                assert_eq!(credited_for(&pos, &basis).ok(), expected);
            }
        }
    }
    #[test]
    fn impossible_relative_indices_and_mixed_kink_groups_fail_closed() {
        let open = Basis::from_indices([WAD, 0, 0]).unwrap();
        let impossible = Basis::from_indices([WAD, WAD, 0]).unwrap();
        let pos = position(1000, LockedRates::new(1, 10000, 0), 1500, 0, 8000, 0, &open);
        assert_eq!(accrued(&pos, &impossible), Err(MathError::InvalidInput));
        assert_eq!(due(&pos, &impossible), Err(MathError::InvalidInput));
        assert_eq!(
            credited_for(&pos, &impossible),
            Err(MathError::InvalidInput)
        );
        assert_eq!(true_up(&pos, &impossible, 0), Err(MathError::InvalidInput));
        let other = position(1000, LockedRates::new(1, 10000, 0), 1500, 0, 7000, 0, &open);
        let mut aggregate = NotionalAggregate::empty();
        aggregate.add(&pos).unwrap();
        let before = aggregate;
        assert_eq!(aggregate.add(&other), Err(MathError::InvalidInput));
        assert_eq!(aggregate, before);
        let mut basis = open;
        let mut carry = LenderCarry::zero();
        let before = (basis, carry);
        assert_eq!(
            accrue(&mut basis, &aggregate, &mut carry, 0, 7000, 1),
            Err(MathError::InvalidInput)
        );
        assert_eq!((basis, carry), before);
        aggregate.remove(&pos).unwrap();
        assert_eq!(aggregate.kink_bps(), None);
        // With no old obligation, a new cohort snapshots the accumulated indices and locks its kink.
        aggregate.add(&other).unwrap();
        assert_eq!(aggregate.kink_bps(), Some(7000));
        accrue(&mut basis, &aggregate, &mut carry, 10000, 7000, 1).unwrap();
        assert!(accrued(&other, &basis).is_ok());
    }
}
