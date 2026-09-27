//! Curve buybacks on the executed path: the plain curve buy, the straddle `B̂_str`
//! (V1-9) and the v1.1 two-leg `B̂₂` (ECON §3.1, §10.1–§10.2; ARCH §2.2, §4.3, §5.4).
//!
//! A buyback larger than the curve's real token reserve `rT` cannot execute on the curve
//! (pump reverts with 6021). The MVP executes it as a bundle: the keeper completes the
//! curve with its own SOL, `migrate_v2` seeds the canonical PumpSwap pool, and the
//! position then buys all of `q` on that pool. [`curve_straddle`] prices exactly that.

use super::{BuyQuote, sol_leg};
use crate::fees::{self, FeeBps, PammFeeSchedule};
use crate::num::Rounding;

/// A pump bonding-curve state `(vT, vQ, rT, rQ)` (R-PUMP §2.1), live or repriced
/// ([`curve_reprice`](super::curve_reprice)).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CurveState {
    /// `virtual_token_reserves`.
    pub vt: u128,
    /// `virtual_quote_reserves` (lamports).
    pub vq: u128,
    /// `real_token_reserves`.
    pub rt: u128,
    /// `real_quote_reserves` (lamports).
    pub rq: u128,
    /// `complete`: the curve no longer trades (pump reverts with 6005).
    pub complete: bool,
}

/// What `migrate_v2` seeds the canonical pool with (R-PUMP §7.4; ARCH §4.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MigrationParams {
    /// `Global.pool_migration_fee` (@146): lamports the curve keeps (15,000,001 today).
    pub migration_fee: u64,
    /// Tokens seeded into the pool: `curve_base_ata.amount − rT` on the live leg,
    /// 206,900,000,000,000 on the EMA leg (standard curve).
    pub pool_tokens: u128,
}

/// The deterministic post-migration pool `B̂_str` prices, as `(X_p, Y_p)` = (tokens,
/// effective SOL) (ECON §3.1; ARCH §4.3): `X_p = pool_tokens`,
/// `Y_p = rQ + n_curve(rT) − pool_migration_fee`, with `n_curve(rT)` the curve quote's
/// `n = ⌈rT·vQ/(vT − rT)⌉ + 1` of ECON §3.1 (0 if `rT = 0`), as the sim's straddle. The
/// completing buy adds the program's `⌊·⌋ + 1` to `rQ`, so `Y_p` is at most one lamport
/// above the pool `migrate_v2` creates (higher `Y_p`, higher quote: conservative). The
/// boost moves `⌊Y_p·2069/10⁴⌋` into `virtual_quote_reserves` without changing the
/// effective reserve, so it does not change the price (ECON §3.1).
///
/// `None` iff `rT ≥ vT` (with `rT > 0`), the migration fee exceeds `rQ + n_curve(rT)`,
/// `n_curve` overflows `u64`, or `Y_p` overflows `u128`.
pub fn migrated_pool(state: &CurveState, m: &MigrationParams) -> Option<(u128, u128)> {
    let n = if state.rt == 0 {
        0
    } else {
        let rest = state.vt.checked_sub(state.rt)?;
        let n = super::muldiv(state.rt, state.vq, rest, Rounding::Ceil)?.checked_add(1)?;
        u128::from(super::to_u64(n)?)
    };
    let fee = u128::from(m.migration_fee);
    let y = match state.rq.checked_sub(fee) {
        Some(rest) => rest.checked_add(n)?,
        None => n.checked_sub(fee.checked_sub(state.rq)?)?,
    };
    Some((m.pool_tokens, y))
}

/// Buys `q` on the executed path of a curve market (ECON §3.1, §10.2; ARCH §4.3, §5.4):
///
/// - `q ≤ rT`: the curve itself, [`sol_leg::buy_exact_out`] with `eps1 = true` (ECON's
///   quote) and `curve_fees`;
/// - `q > rT`: `B̂_str(q) = B̂_pool(q; X_p, Y_p)` on the post-migration pool of
///   [`migrated_pool`], at the PumpSwap fee tier of `mcap(Y_p, X_p)`
///   ([`fees::pump_amm`], including the schedule's risk cap).
///
/// Used on the live leg and on the EMA leg (a [`curve_reprice`](super::curve_reprice)d
/// state); [`CurvePricing`](super::CurvePricing) wraps both. For `q > rT`,
/// [`BuyQuote::net`] is the pool's pre-fee input (σ divides it by the curve's price, ARCH
/// §4.3) and the post-trade reserves are the pool's.
///
/// Not monotone at `q = rT` (review F-A2): the pool's `Y_p` sits ≈ 0.02 % below the
/// curve's completion price, so `B̂_str(rT + 1)` can be below the curve's `B̂(rT)` by up
/// to ≈ 0.02 % (and in `P` on the EMA leg where `rT(P)` crosses `q`). Each side is
/// monotone; [`CurvePricing::liquidation_price`](super::CurvePricing::liquidation_price)
/// searches them separately.
///
/// `None` iff `q = 0`, the curve is `complete`, the curve quote is `None`, or (for
/// `q > rT`) [`migrated_pool`] is `None`, the pool fee cannot be resolved, or the pool
/// quote is `None` (e.g. `q ≥ X_p`).
pub fn curve_straddle(
    q: u64,
    state: &CurveState,
    curve_fees: &FeeBps,
    m: &MigrationParams,
    pool_fees: &PammFeeSchedule<'_>,
) -> Option<BuyQuote> {
    if q == 0 || state.complete {
        return None;
    }
    if u128::from(q) <= state.rt {
        return sol_leg::buy_exact_out(q, state.vt, state.vq, true, curve_fees);
    }
    let (xp, yp) = migrated_pool(state, m)?;
    let fees = fees::pump_amm(yp, xp, pool_fees).ok()?;
    sol_leg::buy_exact_out(q, xp, yp, false, &fees)
}

/// v1.1 in-program straddle `B̂₂(q) = B̂_curve(rT) + B̂_pool(q − rT)` (ECON §3.1, §10.2;
/// ARCH §2.2): the position itself completes the curve and buys the rest on the new pool.
/// It understates the MVP bundle's cost and must not be used for MVP pricing (V1-9).
///
/// `cost` and `net` are the sums of both legs; the post-trade reserves are the pool's.
/// For `q ≤ rT` it equals [`curve_straddle`]. `None` iff either leg is `None`.
pub fn curve_two_leg(
    q: u64,
    state: &CurveState,
    curve_fees: &FeeBps,
    m: &MigrationParams,
    pool_fees: &PammFeeSchedule<'_>,
) -> Option<BuyQuote> {
    if q == 0 || state.complete {
        return None;
    }
    let q128 = u128::from(q);
    if q128 <= state.rt {
        return sol_leg::buy_exact_out(q, state.vt, state.vq, true, curve_fees);
    }
    let leg1 = if state.rt == 0 {
        None
    } else {
        Some(sol_leg::buy_exact_out(
            u64::try_from(state.rt).ok()?,
            state.vt,
            state.vq,
            true,
            curve_fees,
        )?)
    };
    let (xp, yp) = migrated_pool(state, m)?;
    let fees = fees::pump_amm(yp, xp, pool_fees).ok()?;
    let rest = u64::try_from(q128.checked_sub(state.rt)?).ok()?;
    let leg2 = sol_leg::buy_exact_out(rest, xp, yp, false, &fees)?;
    let (c1, n1) = leg1.map_or((0, 0), |l| (l.cost, l.net));
    Some(BuyQuote {
        cost: leg2.cost.checked_add(c1)?,
        net: leg2.net.checked_add(n1)?,
        ..leg2
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::fees::FeeTier;

    const CURVE: FeeBps = FeeBps::new(0, 95, 30);
    const TIERS: [FeeTier; 2] = [
        FeeTier {
            threshold: 0,
            fees: FeeBps::new(2, 93, 30),
        },
        FeeTier {
            threshold: 420_000_000_000,
            fees: FeeBps::new(20, 5, 95),
        },
    ];
    const MIG: MigrationParams = MigrationParams {
        migration_fee: 15_000_001,
        pool_tokens: 206_900_000_000_000,
    };

    fn sched() -> PammFeeSchedule<'static> {
        PammFeeSchedule {
            tiers: &TIERS,
            base_supply: 1_000_000_000_000_000,
            creator_fee_configurable: true,
            pool_creator_fee_bps: 0,
            coin_creator_is_default: false,
            cap_bps: None,
        }
    }

    /// R-PUMP §2.1/§7.4: the curve completed by `SfdLRmJp…` ends at vT 279.9e12,
    /// vQ 115,005,359,346, rQ 85,005,359,346; `migrate_v2` then moves 84,990,359,345.
    #[test]
    fn migration_amount() {
        let done = CurveState {
            vt: 279_900_000_000_000,
            vq: 115_005_359_346,
            rt: 0,
            rq: 85_005_359_346,
            complete: true,
        };
        assert_eq!(
            migrated_pool(&done, &MIG),
            Some((206_900_000_000_000, 84_990_359_345))
        );
    }

    #[test]
    fn straddle_prices_all_of_q_on_the_pool() {
        let s = CurveState {
            vt: 400_000_000_000_000,
            vq: 80_475_000_000,
            rt: 120_100_000_000_000,
            rq: 50_475_000_000,
            complete: false,
        };
        let (xp, yp) = migrated_pool(&s, &MIG).unwrap();
        let q = 150_000_000_000_000;
        let b = curve_straddle(q, &s, &CURVE, &MIG, &sched()).unwrap();
        let f = fees::pump_amm(yp, xp, &sched()).unwrap();
        assert_eq!(b, sol_leg::buy_exact_out(q, xp, yp, false, &f).unwrap());
        // The two-leg form is cheaper: the position buys rT on the curve (V1-9).
        let two = curve_two_leg(q, &s, &CURVE, &MIG, &sched()).unwrap();
        assert!(two.cost < b.cost);
        // At or below rT both are the curve quote.
        let rt = u64::try_from(s.rt).unwrap();
        let c = sol_leg::buy_exact_out(rt, s.vt, s.vq, true, &CURVE).unwrap();
        assert_eq!(curve_straddle(rt, &s, &CURVE, &MIG, &sched()), Some(c));
        assert_eq!(curve_two_leg(rt, &s, &CURVE, &MIG, &sched()), Some(c));
        assert!(
            curve_straddle(rt + 1, &s, &CURVE, &MIG, &sched())
                .unwrap()
                .cost
                > c.cost
        );
        // Complete curves and oversize buys have no quote.
        let done = CurveState {
            complete: true,
            ..s
        };
        assert_eq!(curve_straddle(1, &done, &CURVE, &MIG, &sched()), None);
        assert_eq!(
            curve_straddle(206_900_000_000_000, &s, &CURVE, &MIG, &sched()),
            None
        );
        assert_eq!(curve_straddle(0, &s, &CURVE, &MIG, &sched()), None);
    }
}
