//! Owner: oracle track. Bounds on keeper-initiated buybacks: the exit reference prices
//! and `max_in` (ECON §2.1, §8.2, §12.2; ARCH §3.6 step 6, §3.7, §5.2). Normative
//! reference: `Market.exit_reference`, `ema_growth_bps`, `kappa_eff` and the `max_in`
//! lines of `_buyback_exit` in `sim/shortwin_sim.py` v0.4.1 (default switches:
//! `ref_cap_lo = True`, `liq_reserve = "none"`, `exp_reserve = "rent+fee"`); every
//! division floors, as there.
//!
//! The reference cost `B̂_ref = B̂(Q; reprice(L_b, P_ref))` at the risk fee is the quote
//! track's; this module supplies `P_ref`, `L_b` ([`ref_liquidity`]) and, from `B̂_ref`,
//! the bounds [`max_in_liq`] and [`max_in_exp`]. The keeper executes iff
//! `B̂_live ≤ max_in`. δ is a fixed, locked term (`delta_liq_bps` / `delta_exp_bps`):
//! v0.4 removed the Dutch clock (ECON #49).

use crate::consts::EMA_TAU_S;
use crate::error::MathError;
use crate::num::{BPS, Rounding, mul_bps_u64, mul_div};
use crate::observe::{Ema, LastBucket, Ring, ring_liq_lower_median, sustained_growth};

/// Exit-bound terms, locked per position (ARCH §2.3.3, §2.3.5 "locked exit-bound and
/// persistence terms"; ECON §8.2, §15).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExitParams {
    /// κ_max (`kappa_liq_bps`).
    pub kappa_liq_bps: u16,
    /// κ_base (`kappa_base_bps`).
    pub kappa_base_bps: u16,
    /// Fixed liquidation δ over `B̂_ref` (`delta_liq_bps`).
    pub delta_liq_bps: u16,
    /// Expiry δ (`delta_exp_bps`).
    pub delta_exp_bps: u16,
    /// Steps of sustained growth (`kappa_growth_n`).
    pub kappa_growth_n: u8,
    /// Freshness of the completed bucket (`ref_max_age_s`).
    pub ref_max_age_s: u8,
}

impl ExitParams {
    /// Defaults (ECON §8.2, §15): κ_max 25 %, κ_base 0, δ_liq 2 %, δ_exp 1 %, growth over
    /// 3 steps, reference fresh within 2 s.
    pub const DEFAULT: Self = Self {
        kappa_liq_bps: 2_500,
        kappa_base_bps: 0,
        delta_liq_bps: 200,
        delta_exp_bps: 100,
        kappa_growth_n: 3,
        ref_max_age_s: 2,
    };
}

impl Default for ExitParams {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Reference liquidity `L_b = max(L_live, L_ema', L_slow)` (ECON §8.2, V2-14): the
/// deepest of the three gives the lowest reference cost, so an LP pull in a quiet second
/// cannot loosen the bound. `L_live` is the venue's `√(X·Y)` (`L_ema` when the venue has
/// no price, as the sim).
///
/// `L_ema' = L_ema`, except while the post-change reference guard is set (`ema.guard = 1`;
/// M4R, **PROPOSED**, owner item O18): then `L_ema' = min(L_ema, L_mlo)`, `L_mlo` the lower
/// median of the ring's bucket liquidity lows ([`ring_liq_lower_median`]). A restart's warm-up
/// seeds `L_ema` from the buckets' **highs** (`observe::integrate`), which capital pushes up
/// with a deposit around each crank; the guard keeps such a push from raising the reference
/// liquidity, and so from blocking the exit bound, until `L_ema` agrees with the lows within
/// `REF_GUARD_TOL_BPS` (an honest switch: at the end of the warm-up).
///
/// Errors: `InvalidInput` for a corrupted ring while the guard is set.
pub fn ref_liquidity(l_live: u128, ema: &Ema, ring: &Ring) -> Result<u128, MathError> {
    let l_ema = if ema.guard != 0 {
        ema.liq.min(ring_liq_lower_median(ring)?)
    } else {
        ema.liq
    };
    Ok(l_live.max(l_ema).max(ema.liq_slow))
}

/// `κ_eff = min(κ_max, κ_base + (EMA_TAU_S + 1)·g)` in bps, `g` the sustained growth in
/// bps/s (ECON §2.1, §8.2): in steady exponential growth the EMA lags spot by ≈ (τ + 1)·g.
pub fn kappa_eff(g_bps: u64, params: &ExitParams) -> u16 {
    let k = u128::from(EMA_TAU_S)
        .saturating_add(1)
        .saturating_mul(u128::from(g_bps))
        .saturating_add(u128::from(params.kappa_base_bps))
        .min(u128::from(params.kappa_liq_bps));
    u16::try_from(k).unwrap_or(params.kappa_liq_bps)
}

/// Liquidation reference price (ECON §2.1, §8.2; ARCH §3.6 step 6):
///
/// ```text
/// g     = sustained_growth(ring, kappa_growth_n)
/// κ_eff = min(κ_max, κ_base + (EMA_TAU_S + 1)·g)
/// lo_x  = ⌊lb_lo·(10⁴ + g·max(0, now − lb_sec))/10⁴⌋
/// P_ref = min( max(P_ema, lo_x), ⌊P_ema·(10⁴ + κ_eff)/10⁴⌋, ⌊lo_x·(10⁴ + κ_eff)/10⁴⌋ )
/// ```
///
/// The last term (v0.4.1, R4B-2) keeps a `P_ema` that one attacker-only second pushed and
/// later seconds froze from lifting the bound: a frozen EMA has no sustained growth, so
/// the reference stays at the honest bucket low; in a genuine pump `P_ema ≤ lo_x` and it
/// never binds. The bound is then `⌊B̂_ref·(10⁴ + delta_liq_bps)/10⁴⌋` ([`max_in_liq`]).
///
/// `Ok(None)` if no completed bucket is fresh within `ref_max_age_s` (`ReferenceStale`:
/// crank first). Errors: `Overflow` if `P_ref` itself exceeds `u128`; `InvalidInput` for a
/// corrupted ring.
pub fn liq_reference(
    ema: &Ema,
    ring: &Ring,
    last: &LastBucket,
    now: i64,
    params: &ExitParams,
) -> Result<Option<u128>, MathError> {
    if !last.is_fresh(now, params.ref_max_age_s) {
        return Ok(None);
    }
    let g = sustained_growth(ring, params.kappa_growth_n)?;
    let kappa = u128::from(kappa_eff(g, params));
    let bps = u128::from(BPS);
    // Fresh ⇒ age ≤ ref_max_age_s ≤ 255, so g·age < 2⁷².
    let age = u128::try_from(i128::from(now).saturating_sub(i128::from(last.sec))).unwrap_or(0);
    let growth = u128::from(g)
        .checked_mul(age)
        .and_then(|x| x.checked_add(bps))
        .ok_or(MathError::Overflow)?;
    let widen = bps.saturating_add(kappa);
    // Past u128 a term cannot be the minimum of a finite one: `None` stands for "above".
    let lo_x = above_u128(mul_div(last.lo, growth, bps, Rounding::Floor))?;
    let ema_cap = above_u128(mul_div(ema.price, widen, bps, Rounding::Floor))?;
    let lo_cap = match lo_x {
        Some(x) => above_u128(mul_div(x, widen, bps, Rounding::Floor))?,
        None => None,
    };
    let hi = lo_x.map(|x| x.max(ema.price));
    [hi, ema_cap, lo_cap]
        .into_iter()
        .flatten()
        .min()
        .map(Some)
        .ok_or(MathError::Overflow)
}

fn above_u128(r: Result<u128, MathError>) -> Result<Option<u128>, MathError> {
    match r {
        Ok(x) => Ok(Some(x)),
        Err(MathError::Overflow) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Expiry reference price (ECON §2.1, §8.2; ARCH §3.7): `P_ref = min(P_ema, min over the
/// ring of p_i, lb_lo)`, the lowest recent robust price (nothing is urgent at expiry); the
/// bound is `⌊B̂_ref·(10⁴ + delta_exp_bps)/10⁴⌋` ([`max_in_exp`]). `Ok(None)` if no
/// completed bucket is fresh. Errors: `InvalidInput` for a corrupted ring.
pub fn expiry_reference(
    ema: &Ema,
    ring: &Ring,
    last: &LastBucket,
    now: i64,
    params: &ExitParams,
) -> Result<Option<u128>, MathError> {
    ring.check()?;
    if !last.is_fresh(now, params.ref_max_age_s) {
        return Ok(None);
    }
    Ok(Some(
        ring.iter()
            .map(|e| e.p)
            .fold(ema.price.min(last.lo), u128::min),
    ))
}

/// The expiry fee on the reference cost, `π_ref = ⌈expiry_fee_bps·B̂_ref/10⁴⌉` (ECON §2.1,
/// §8.2; ARCH §3.7): the part of an expiry's reserve beyond rent in [`max_in_exp`].
///
/// Errors: `Overflow` (only for a fee above 10⁴·u64::MAX / B̂_ref, i.e. never in bounds).
pub fn pi_ref(b_ref: u64, expiry_fee_bps: u16) -> Result<u64, MathError> {
    mul_bps_u64(b_ref, u64::from(expiry_fee_bps), Rounding::Ceil)
}

/// `⌊b·(10⁴ + bps)/10⁴⌋` as `i128` (exact: `b < 2⁶⁴`, `bps < 2¹⁶`).
fn scaled(b: u64, bps: u16) -> Option<i128> {
    let s = i128::from(BPS);
    i128::from(b)
        .checked_mul(s.checked_add(i128::from(bps))?)?
        .checked_div(s)
}

/// `V − K_s − ert`: the most a buyback may spend so that S1 stays disjoint (ECON §5.6).
fn s1_budget(v: u64, k_s: u64, ert: u64) -> Option<i128> {
    i128::from(v)
        .checked_sub(i128::from(k_s))?
        .checked_sub(i128::from(ert))
}

/// Liquidation bound (ECON §8.2 v0.4.1; ARCH §3.6 step 6, §5.2; SPEC §5.2):
///
/// ```text
/// max_in = min( ⌊B̂_ref·(10⁴ + δ_liq)/10⁴⌋,  V − K_s − ert )
/// ```
///
/// No rent or penalty reserve (R4A-1, R4B-1): any reserve opens a dead band in which a due
/// liquidation waits while the S1 legs are false, so honest liquidations in fast pumps gap
/// into S1. Liveness (I-E11): a liquidatable, buyback-allowed position with a fresh
/// reference executes as soon as `B̂_live ≤ ⌊B̂_ref·(10⁴ + δ)/10⁴⌋`.
///
/// `None` if the minimum is negative (`V − K_s − ert < 0`: no buyback can be afforded).
/// `v`, `k_s` and `ert` are the position's (`ert` its effective venue-rent tolerance, 0 on
/// Raydium).
pub fn max_in_liq(b_ref: u64, delta_bps: u16, v: u64, k_s: u64, ert: u64) -> Option<u64> {
    let m = scaled(b_ref, delta_bps)?.min(s1_budget(v, k_s, ert)?);
    u64::try_from(m).ok()
}

/// Expiry bound (ECON §8.2; ARCH §3.7, §5.2): rent and the expiry fee stay senior unless
/// the fair cost itself leaves no room for them:
///
/// ```text
/// max_in = min( ⌊B̂_ref·(10⁴ + δ_exp)/10⁴⌋,  V − K_s − ert,  max(V − R − π_ref − ert, ⌊B̂_ref·(10⁴ + ε)/10⁴⌋) )
/// ```
///
/// with `R = R_due`, `π_ref` from [`pi_ref`] and ε the position's `exec_tol_bps` (the
/// expiry bound's tolerance since v0.4.1). `None` if the minimum is negative.
#[allow(clippy::too_many_arguments)]
pub fn max_in_exp(
    b_ref: u64,
    delta_bps: u16,
    eps_bps: u16,
    v: u64,
    k_s: u64,
    r: u64,
    pi_ref: u64,
    ert: u64,
) -> Option<u64> {
    let senior = i128::from(v)
        .checked_sub(i128::from(r))?
        .checked_sub(i128::from(pi_ref))?
        .checked_sub(i128::from(ert))?
        .max(scaled(b_ref, eps_bps)?);
    let m = scaled(b_ref, delta_bps)?
        .min(s1_budget(v, k_s, ert)?)
        .min(senior);
    u64::try_from(m).ok()
}

const _: () = assert!(core::mem::size_of::<ExitParams>() == 10);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::observe::RingEntry;

    const WAD: u128 = crate::num::WAD;

    fn state(prices: &[(i64, u128)], lb: (i64, u128)) -> (Ema, Ring, LastBucket) {
        let mut ring = Ring::default();
        for &(ts, p) in prices {
            ring.push(RingEntry {
                p,
                l: 1,
                lo: p,
                hi: p,
                ts,
                ..RingEntry::default()
            })
            .unwrap();
        }
        let (ts, p) = *prices.last().unwrap();
        let ema = Ema {
            price: p,
            liq: 1,
            liq_slow: 1,
            ts,
            slow_ts: ts,
            count: 3,
            warm: 0,
            guard: 0,
            _pad: [0; 6],
        };
        let last = LastBucket {
            lo: lb.1,
            hi: lb.1,
            sec: lb.0,
            _pad: [0; 8],
        };
        (ema, ring, last)
    }

    #[test]
    fn liquidation_reference_follows_sustained_growth() {
        let prm = ExitParams::DEFAULT;
        // +1 %/s for three steps: g = 100 bps/s, κ_eff = 600 bps.
        let p = [
            (0, 100 * WAD),
            (1, 101 * WAD),
            (2, 10_201 * WAD / 100),
            (3, 1_030_301 * WAD / 10_000),
        ];
        let (ema, ring, last) = state(&p, (2, 110 * WAD));
        let r = liq_reference(&ema, &ring, &last, 3, &prm).unwrap().unwrap();
        // lo_x = lo·(1 + 1 %·1 s) = 111.1 WAD, capped at P_ema·1.06 (and lo_x·1.06).
        assert_eq!(r, (ema.price * 10_600 / 10_000).min(1111 * WAD / 10));
        // Stale bucket.
        assert_eq!(liq_reference(&ema, &ring, &last, 5, &prm).unwrap(), None);
        // A flat ring gives no growth: P_ref = min(max(P_ema, lo), P_ema, lo) = P_ema.
        let flat = [
            (0, 100 * WAD),
            (1, 100 * WAD),
            (2, 100 * WAD),
            (3, 100 * WAD),
        ];
        let (ema, ring, last) = state(&flat, (3, 150 * WAD));
        assert_eq!(
            liq_reference(&ema, &ring, &last, 3, &prm).unwrap(),
            Some(100 * WAD)
        );
        assert_eq!(
            expiry_reference(&ema, &ring, &last, 3, &prm).unwrap(),
            Some(100 * WAD)
        );
    }

    #[test]
    fn pushed_and_frozen_ema_cannot_lift_the_reference() {
        // v0.4.1 (R4B-2): P_ema pushed 16 % above an honest bucket low, no sustained growth.
        let prm = ExitParams {
            kappa_base_bps: 1_000,
            ..ExitParams::DEFAULT
        };
        let (mut ema, ring, last) = state(&[(0, 100 * WAD), (1, 100 * WAD)], (1, 100 * WAD));
        ema.price = 116 * WAD;
        // v0.4: min(max(116, 100), 116·1.1) = 116; v0.4.1 also ≤ 100·1.1 = 110.
        assert_eq!(
            liq_reference(&ema, &ring, &last, 1, &prm).unwrap(),
            Some(110 * WAD)
        );
    }

    #[test]
    fn reference_overflow_edges() {
        let prm = ExitParams {
            kappa_base_bps: 100,
            ..ExitParams::DEFAULT
        };
        let (mut ema, ring, mut last) = state(&[(0, 1), (1, 1)], (1, u128::MAX));
        ema.price = u128::MAX;
        // lo_x = MAX, both caps (×1.01) above u128: P_ref = max(P_ema, lo_x) = MAX.
        assert_eq!(
            liq_reference(&ema, &ring, &last, 1, &prm).unwrap(),
            Some(u128::MAX)
        );
        last.lo = 7;
        assert_eq!(liq_reference(&ema, &ring, &last, 1, &prm).unwrap(), Some(7));
    }

    #[test]
    fn max_in_terms() {
        // Liquidation: bound or S1 budget, no reserve.
        assert_eq!(max_in_liq(1_000, 200, 10_000, 100, 0), Some(1_020));
        assert_eq!(max_in_liq(1_000, 200, 1_100, 100, 0), Some(1_000));
        assert_eq!(max_in_liq(1_000, 200, 1_100, 100, 50), Some(950));
        assert_eq!(max_in_liq(1_000, 200, 100, 200, 0), None);
        assert_eq!(
            max_in_liq(u64::MAX, u16::MAX, u64::MAX, 0, 0),
            Some(u64::MAX)
        );
        // Expiry: rent and fee senior unless the fair cost needs the room.
        let pi = pi_ref(1_000, 200).unwrap();
        assert_eq!(pi, 20);
        assert_eq!(
            max_in_exp(1_000, 100, 10, 10_000, 100, 50, pi, 0),
            Some(1_010)
        );
        assert_eq!(
            max_in_exp(1_000, 500, 10, 1_200, 100, 1_000, pi, 0),
            Some(1_001)
        );
        assert_eq!(max_in_exp(1_000, 500, 10, 1_060, 0, 20, pi, 0), Some(1_020));
        assert_eq!(max_in_exp(1_000, 100, 10, 100, 200, 0, 0, 0), None);
        assert_eq!(pi_ref(1_001, 200), Ok(21));
    }

    /// `L_b` with and without the post-change reference guard (M4R).
    #[test]
    fn reference_liquidity_under_the_guard() {
        let (mut ema, mut ring, _) = state(&[(0, WAD), (1, WAD), (2, WAD)], (2, WAD));
        // Bucket lows 300, 200, 100 (lower median 200); L_ema 900, L_slow 150.
        for (e, llo) in ring.entries.iter_mut().zip([300u128, 200, 100]) {
            e.llo = llo;
        }
        ema.liq = 900;
        ema.liq_slow = 150;
        assert_eq!(ref_liquidity(50, &ema, &ring), Ok(900));
        ema.guard = 1;
        assert_eq!(ref_liquidity(50, &ema, &ring), Ok(200));
        assert_eq!(ref_liquidity(250, &ema, &ring), Ok(250));
        ema.liq_slow = 400;
        assert_eq!(ref_liquidity(250, &ema, &ring), Ok(400));
        // L_ema below the lows: the guard changes nothing.
        ema.liq = 120;
        ema.liq_slow = 1;
        assert_eq!(ref_liquidity(50, &ema, &ring), Ok(120));
        ring.head = 9;
        assert_eq!(ref_liquidity(50, &ema, &ring), Err(MathError::InvalidInput));
        ema.guard = 0;
        assert_eq!(ref_liquidity(50, &ema, &ring), Ok(120));
    }

    #[test]
    fn kappa() {
        let prm = ExitParams::DEFAULT;
        assert_eq!(kappa_eff(0, &prm), 0);
        assert_eq!(kappa_eff(100, &prm), 600);
        assert_eq!(kappa_eff(u64::MAX, &prm), 2_500);
        let base = ExitParams {
            kappa_base_bps: 3_000,
            ..prm
        };
        assert_eq!(kappa_eff(0, &base), 2_500);
    }
}
