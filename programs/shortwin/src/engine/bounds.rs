//! Exit bounds (ARCH §2.1 `engine/bounds.rs`, §3.6 step 6, §3.7; ECON §8.2): the reference
//! prices and `max_in` of the keeper-initiated buybacks, as thin wrappers over
//! `shortwin_math::bounds` with the position's **locked** terms and the risk fee
//! `min(φ_live, venue_fee_hard_cap_bps_pos)` (V2-1). A borrower's close has no reference: its
//! bound is `min(max_buyback_cost, V − R − ert)` ([`close_max_in`]).
//!
//! The reference liquidity of a Raydium market is capped at the depth its sizing credits
//! (M4F-1, [`ref_depth_cap`]).
//!
//! Used by `close_short` (M2 foundation) and by the `liquidate` / `expire` tracks.

use anchor_lang::prelude::*;
use shortwin_math::bounds;
use shortwin_math::observe::Observation;

use crate::errors::{CheckedExt, MathResultExt};
use crate::state::{GlobalConfig, Market, ShortPosition, VenueKind};
use crate::venues::VenueState;

/// A keeper-initiated buyback's bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitBound {
    /// `P_ref`.
    pub p_ref: u128,
    /// `L_b = max(L_live, min(max(L_ema', L_slow), cap))` (V2-14), `L_ema' = min(L_ema, L_mlo)`
    /// while the post-change reference guard is set (M4R), `cap` = [`ref_depth_cap`] on Raydium
    /// (M4F-1).
    pub l_b: u128,
    /// `B̂_ref = B̂(Q; reprice(L_b, P_ref))` at the risk fee; `None` if undefined.
    pub b_ref: Option<u64>,
}

/// The cap on the reference liquidity's EMA terms (M4F-1; ECON §8.2): on a Raydium market
/// `⌊D·10⁴/λ_min⌋` (`shortwin_math::bounds::ref_depth_cap`) with `D = pool_d` — the locked depth
/// at the last executable observation, which the exit's own prologue has just refreshed — and
/// `λ_min = pool_lock_min_bps`: never more depth than the open caps, the tier and the adoption
/// anchor credit the pool with (V2-8). `None` on the other venues (the pump venues and the mock
/// have no locked depth and no pool selection, hence no switch and no post-change guard).
pub fn ref_depth_cap(config: &GlobalConfig, market: &Market, kind: VenueKind) -> Option<u128> {
    kind.is_raydium()
        .then(|| bounds::ref_depth_cap(market.pool_d.get(), config.params.pool_lock_min_bps))
}

/// The liquidation reference (ARCH §3.6 step 6; v0.4.1 with the `lo_x·(1 + κ_eff)` cap):
/// `Ok(None)` when no completed bucket is fresh within the position's `ref_max_age_s`
/// (`ReferenceStale`). `depth_cap` is [`ref_depth_cap`].
pub fn liq_reference(
    pos: &ShortPosition,
    obs: &Observation,
    state: &VenueState,
    now: i64,
    depth_cap: Option<u128>,
) -> Result<Option<ExitBound>> {
    let p =
        bounds::liq_reference(&obs.ema, &obs.ring, &obs.last, now, &pos.exit_params()).math()?;
    p.map(|p_ref| reference(pos, obs, state, p_ref, depth_cap))
        .transpose()
}

/// The expiry reference (ARCH §3.7): `P_ref = min(P_ema, min over the ring of p_i, lb_lo)`.
/// `depth_cap` is [`ref_depth_cap`].
pub fn expiry_reference(
    pos: &ShortPosition,
    obs: &Observation,
    state: &VenueState,
    now: i64,
    depth_cap: Option<u128>,
) -> Result<Option<ExitBound>> {
    let p =
        bounds::expiry_reference(&obs.ema, &obs.ring, &obs.last, now, &pos.exit_params()).math()?;
    p.map(|p_ref| reference(pos, obs, state, p_ref, depth_cap))
        .transpose()
}

fn reference(
    pos: &ShortPosition,
    obs: &Observation,
    state: &VenueState,
    p_ref: u128,
    depth_cap: Option<u128>,
) -> Result<ExitBound> {
    let l_live = state.liquidity().unwrap_or(obs.ema.liq);
    let l_b = bounds::ref_liquidity(l_live, &obs.ema, &obs.ring, depth_cap).math()?;
    let b_ref = state.quoter().and_then(|pr| {
        pr.capped(u64::from(pos.venue_fee_hard_cap_bps))
            .buy_repriced(pos.q, l_b, p_ref)
            .map(|b| b.cost)
    });
    Ok(ExitBound { p_ref, l_b, b_ref })
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
