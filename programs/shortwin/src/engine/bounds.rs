//! Exit bounds (ARCH §2.1 `engine/bounds.rs`, §3.6 step 6, §3.7; ECON §8.2): the reference
//! prices and `max_in` of the keeper-initiated buybacks, as thin wrappers over
//! `shortwin_math::bounds` with the position's **locked** terms and the risk fee
//! `min(φ_live, venue_fee_hard_cap_bps_pos)` (V2-1). A borrower's close has no reference: its
//! bound is `min(max_buyback_cost, V − R − ert)` ([`close_max_in`]).
//!
//! Used by `close_short` (M2 foundation) and by the `liquidate` / `expire` tracks.

use anchor_lang::prelude::*;
use shortwin_math::bounds;
use shortwin_math::observe::Observation;

use crate::errors::{CheckedExt, MathResultExt};
use crate::state::ShortPosition;
use crate::venues::VenueState;

/// A keeper-initiated buyback's bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitBound {
    /// `P_ref`.
    pub p_ref: u128,
    /// `L_b = max(L_live, L_ema, L_slow)` (V2-14).
    pub l_b: u128,
    /// `B̂_ref = B̂(Q; reprice(L_b, P_ref))` at the risk fee; `None` if undefined.
    pub b_ref: Option<u64>,
}

/// The liquidation reference (ARCH §3.6 step 6; v0.4.1 with the `lo_x·(1 + κ_eff)` cap):
/// `Ok(None)` when no completed bucket is fresh within the position's `ref_max_age_s`
/// (`ReferenceStale`).
pub fn liq_reference(
    pos: &ShortPosition,
    obs: &Observation,
    state: &VenueState,
    now: i64,
) -> Result<Option<ExitBound>> {
    let p =
        bounds::liq_reference(&obs.ema, &obs.ring, &obs.last, now, &pos.exit_params()).math()?;
    Ok(p.map(|p_ref| reference(pos, obs, state, p_ref)))
}

/// The expiry reference (ARCH §3.7): `P_ref = min(P_ema, min over the ring of p_i, lb_lo)`.
pub fn expiry_reference(
    pos: &ShortPosition,
    obs: &Observation,
    state: &VenueState,
    now: i64,
) -> Result<Option<ExitBound>> {
    let p =
        bounds::expiry_reference(&obs.ema, &obs.ring, &obs.last, now, &pos.exit_params()).math()?;
    Ok(p.map(|p_ref| reference(pos, obs, state, p_ref)))
}

fn reference(pos: &ShortPosition, obs: &Observation, state: &VenueState, p_ref: u128) -> ExitBound {
    let l_live = state.liquidity().unwrap_or(obs.ema.liq);
    let l_b = bounds::ref_liquidity(l_live, &obs.ema);
    let b_ref = state.pricing.and_then(|pr| {
        pr.capped(u64::from(pos.venue_fee_hard_cap_bps))
            .buy_repriced(pos.q, l_b, p_ref)
            .map(|b| b.cost)
    });
    ExitBound { p_ref, l_b, b_ref }
}

/// `max_in` of a liquidation: `min(⌊B̂_ref·(10⁴ + δ_liq)/10⁴⌋, V − K_s − ert)` (no reserve,
/// v0.4.1; `bounds::max_in_liq`).
pub fn max_in_liq(pos: &ShortPosition, b_ref: u64, k_s: u64) -> Option<u64> {
    bounds::max_in_liq(b_ref, pos.delta_liq_bps, pos.v, k_s, pos.ert)
}

/// `max_in` of an expiry: rent and the expiry fee senior (`bounds::max_in_exp`).
pub fn max_in_exp(pos: &ShortPosition, b_ref: u64, k_s: u64, r_due: u64) -> Result<Option<u64>> {
    let pi_ref = bounds::pi_ref(b_ref, pos.expiry_fee_bps).math()?;
    Ok(bounds::max_in_exp(
        b_ref,
        pos.delta_exp_bps,
        pos.exec_tol_bps,
        pos.v,
        k_s,
        r_due,
        pi_ref,
        pos.ert,
    ))
}

/// A borrower's close (ARCH §3.5 step 5; C0-14): `max_in = min(max_buyback_cost, V − R −
/// ert)`. `None` when `V < R + ert` (the vault cannot pay the rent: `InsufficientVault`).
pub fn close_max_in(v: u64, r_due: u64, ert: u64, max_buyback_cost: u64) -> Result<Option<u64>> {
    let reserved = r_due.checked_add(ert).or_math()?;
    Ok(v.checked_sub(reserved)
        .map(|room| room.min(max_buyback_cost)))
}
