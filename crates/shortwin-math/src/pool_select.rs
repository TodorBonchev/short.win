//! Owner: quotes track. Raydium pool selection: locked LP share `λ`, locked depth `D`, eligibility and dominance
//! (ARCH §2.6, A42; ECON §2.1, §6.2).
//!
//! Only the arithmetic lives here. Account verification (owners, discriminators, vault
//! and mint keys, the SOL side) and `venue_executable` are the program's; their outcome
//! enters [`eligible`] as plain inputs. Rounding is ECON §2.1's (the sims' `RaydiumPool`):
//! `λ` is floored to whole bps first, then `D` is floored.

use crate::error::MathError;
use crate::fees::exceeds_cap;
use crate::num::{BPS, Rounding, mul_div, mul_le, sqrt_u128};

/// Permanently locked LP share `λ_bps = ⌊(lp_counter − lp_supply)·10⁴/lp_counter⌋`
/// (ECON §2.1; ARCH §2.6; the sims' `RaydiumPool.lock_bps`). `lp_counter` is the venue's
/// internal LP count (AMM v4 `lp_amount` @720, CPMM `lp_supply` @333) and `lp_supply` the
/// LP mint's circulating supply; the difference is LP that can never be redeemed. 0 for
/// a pool with no LP counter or a circulating supply at or above it; at most 10⁴.
pub fn lambda_bps(lp_counter: u64, lp_supply: u64) -> u32 {
    match lp_counter.checked_sub(lp_supply) {
        Some(locked) if lp_counter > 0 => {
            let lam = u128::from(locked)
                .saturating_mul(u128::from(BPS))
                .checked_div(u128::from(lp_counter))
                .unwrap_or(0);
            u32::try_from(lam).unwrap_or(u32::MAX)
        }
        _ => 0,
    }
}

/// Locked constant-product depth `D = ⌊λ_bps·isqrt(X·Y)/10⁴⌋` (ECON §2.1; ARCH §2.6; the
/// sims' `RaydiumPool.locked_depth`), with [`lambda_bps`] and `X`, `Y` the pricing reserves
/// (ARCH §4.1). v0.3 floored once on the raw ratio, `⌊(counter − supply)·isqrt(X·Y)/
/// counter⌋`, which can exceed this by up to `isqrt(X·Y)/10⁴`.
///
/// Rounded down: `D` bounds how much a market may be sized on (ARCH §2.6 rule 5), so the
/// pool is never credited with depth it does not have. `X·Y < 2¹²⁸` for `u64` reserves, so
/// the square root is `u128` (ARCH §2.2).
///
/// Errors: none in practice (`Overflow` is unreachable for `u64` reserves).
pub fn locked_depth(lp_counter: u64, lp_supply: u64, x: u64, y: u64) -> Result<u128, MathError> {
    let k = u128::from(x)
        .checked_mul(u128::from(y))
        .ok_or(MathError::Overflow)?;
    mul_div(
        u128::from(lambda_bps(lp_counter, lp_supply)),
        sqrt_u128(k, Rounding::Floor),
        u128::from(BPS),
        Rounding::Floor,
    )
}

/// `λ_bps ≥ min_bps` (ARCH §2.6 eligibility, `pool_lock_min_bps`), with [`lambda_bps`]'s
/// floor; equivalently `(lp_counter − lp_supply)·10⁴ ≥ min_bps·lp_counter`. False for a
/// pool with no LP counter or a circulating supply above it.
pub fn lock_share_ok(lp_counter: u64, lp_supply: u64, min_bps: u64) -> bool {
    lp_counter > 0
        && lp_supply <= lp_counter
        && u64::from(lambda_bps(lp_counter, lp_supply)) >= min_bps
}

/// The numeric facts of a verified candidate pool (ARCH §2.6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PoolFacts {
    /// `venue_executable(P)` (§5.3): status, open time, vaults at or above their counters.
    pub executable: bool,
    /// One-way fee φ in **ppm** ([`RayV4Fee::oneway_ppm`](crate::fees::RayV4Fee::oneway_ppm),
    /// [`CpmmFees::oneway_ppm`](crate::fees::CpmmFees::oneway_ppm); ECON §2.1).
    pub fee_ppm: u32,
    /// Real SOL reserve `R` (the SOL-side pricing reserve; Raydium has no virtual reserve).
    pub real_sol: u64,
    /// Internal LP counter.
    pub lp_counter: u64,
    /// LP mint supply (`supply` @36).
    pub lp_supply: u64,
}

/// The pool-selection parameters `eligible` reads (ARCH §2.3.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct EligibilityParams {
    /// `venue_fee_cap_bps` (150).
    pub venue_fee_cap_bps: u64,
    /// `pool_min_real_sol` (25 SOL).
    pub pool_min_real_sol: u64,
    /// `pool_lock_min_bps` λ_min (5,000).
    pub pool_lock_min_bps: u64,
}

/// `eligible(P)` (ARCH §2.6): executable, `φ_ppm ≤ venue_fee_cap_bps·100` (no rounding,
/// [`exceeds_cap`]), `R ≥ pool_min_real_sol` and `λ_bps ≥ pool_lock_min_bps`
/// ([`lock_share_ok`]). The caller has already verified the accounts.
pub fn eligible(p: &PoolFacts, params: &EligibilityParams) -> bool {
    p.executable
        && !exceeds_cap(p.fee_ppm, params.venue_fee_cap_bps)
        && p.real_sol >= params.pool_min_real_sol
        && lock_share_ok(p.lp_counter, p.lp_supply, params.pool_lock_min_bps)
}

/// `d_cand ≥ (1 + margin_bps/10⁴)·d_inc`, exactly (ARCH §2.6 rule 1 with
/// `pool_list_margin_bps`, rule 2 with `pool_switch_margin_bps`).
pub fn dominates(d_cand: u128, d_inc: u128, margin_bps: u64) -> bool {
    at_least_bps(
        d_cand,
        d_inc,
        u128::from(BPS).saturating_add(u128::from(margin_bps)),
    )
}

/// `d_cand ≥ min_bps·d_inc/10⁴`, exactly: a candidate that may open the fast-path
/// collection window (ARCH §2.6 rule 3, `pool_fast_min_bps`).
pub fn fast_path_qualifies(d_cand: u128, d_inc: u128, min_bps: u64) -> bool {
    at_least_bps(d_cand, d_inc, u128::from(min_bps))
}

fn at_least_bps(d_cand: u128, d_inc: u128, bps: u128) -> bool {
    mul_le(d_inc, bps, d_cand, u128::from(BPS))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn depth() {
        // Fartcoin AMM v4 (R-RAY §2.1): lp_amount 4,051,966,758,784, LP supply
        // 8,906,225,417 -> λ = 99.78 %.
        let (c, s) = (4_051_966_758_784u64, 8_906_225_417u64);
        let (x, y) = (24_145_801_124_347u64, 38_155_515_042_581u64);
        let d = locked_depth(c, s, x, y).unwrap();
        let root = sqrt_u128(u128::from(x) * u128::from(y), Rounding::Floor);
        // λ floored to whole bps first (ECON §2.1): 9,978 bps, then D floored.
        assert_eq!(lambda_bps(c, s), 9_978);
        assert_eq!(d, 9_978 * root / 10_000);
        // v0.3's single floor on the raw ratio is larger here.
        assert!((u128::from(c - s) * root) / u128::from(c) > d);
        assert!(lock_share_ok(c, s, 9_978) && !lock_share_ok(c, s, 9_979));
        assert_eq!(locked_depth(0, 0, x, y), Ok(0));
        assert_eq!(locked_depth(5, 7, x, y), Ok(0));
        assert_eq!(locked_depth(5, 0, x, y), Ok(root));
        assert_eq!(
            locked_depth(u64::MAX, 0, u64::MAX, u64::MAX),
            Ok(u128::from(u64::MAX))
        );
        assert_eq!(
            (lambda_bps(0, 0), lambda_bps(5, 7), lambda_bps(5, 5)),
            (0, 0, 0)
        );
        assert_eq!((lambda_bps(3, 1), lambda_bps(u64::MAX, 0)), (6_666, 10_000));
        assert!(!lock_share_ok(0, 0, 0));
        assert!(lock_share_ok(5, 5, 0) && !lock_share_ok(5, 6, 0));
    }

    #[test]
    fn rules() {
        let params = EligibilityParams {
            venue_fee_cap_bps: 150,
            pool_min_real_sol: 25_000_000_000,
            pool_lock_min_bps: 5_000,
        };
        let p = PoolFacts {
            executable: true,
            fee_ppm: 2_500,
            real_sol: 25_000_000_000,
            lp_counter: 1_000,
            lp_supply: 500,
        };
        assert!(eligible(&p, &params));
        assert!(!eligible(
            &PoolFacts {
                lp_supply: 501,
                ..p
            },
            &params
        ));
        // φ is compared in ppm with cap·100: 1.5 % passes, 1.5001 % does not.
        assert!(eligible(
            &PoolFacts {
                fee_ppm: 15_000,
                ..p
            },
            &params
        ));
        assert!(!eligible(
            &PoolFacts {
                fee_ppm: 15_001,
                ..p
            },
            &params
        ));
        assert!(!eligible(
            &PoolFacts {
                real_sol: 24_999_999_999,
                ..p
            },
            &params
        ));
        assert!(!eligible(
            &PoolFacts {
                executable: false,
                ..p
            },
            &params
        ));

        assert!(dominates(10_500, 10_000, 500) && !dominates(10_499, 10_000, 500));
        assert!(dominates(12_500, 10_000, 2_500) && !dominates(12_499, 10_000, 2_500));
        assert!(dominates(0, 0, 0) && dominates(u128::MAX, u128::MAX, 0));
        assert!(!dominates(u128::MAX, u128::MAX, 1));
        assert!(fast_path_qualifies(2_500, 10_000, 2_500));
        assert!(!fast_path_qualifies(2_499, 10_000, 2_500));
    }
}
