//! SPEC §4 size caps at open (ARCH §2.1 `engine/caps.rs`, §3.4 step 6, §4.6; ECON §6.1): a thin
//! wrapper that applies `shortwin_math::caps` in ARCH's order and maps each refusal to its
//! error. Caps are checked in `open_short` only and never force a close.

use anchor_lang::prelude::*;
use shortwin_math::caps;

use crate::errors::{CheckedExt, MathResultExt, ShortwinError};
use crate::state::{Params, TierParams};

/// The inputs of the open caps.
#[derive(Clone, Copy, Debug)]
pub struct CapInputs {
    /// The position's `q`.
    pub q: u64,
    /// `X_now`, the live pre-trade token reserve.
    pub x_now: u64,
    /// `L_slow` and `P_ema` after this instruction's roll.
    pub l_slow: u128,
    pub p_ema: u128,
    /// Raydium: `(D, pool_lock_min_bps)` (locked depth over `λ_min`, V2-8); `None` elsewhere.
    pub raydium_lock: Option<(u128, u32)>,
    /// The live one-way venue fee φ, ppm.
    pub phi_ppm: u32,
    /// `T` and `ΣB_Q` before the open.
    pub tokens_free: u64,
    pub borrowed_q: u64,
    /// The borrower's open quantity in this market (`WalletExposure.q_open`).
    pub wallet_q: u64,
    /// Tier D's curve guard `(ΣQ, rT)` (`curve_gamma_ok`); `None` off the curve.
    pub curve: Option<u64>,
}

/// What the caps computed (reported and reused by the open).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapOutputs {
    /// `X₀`, the conservative pre-trade token reserve (`caps::conservative_x0`).
    pub x0: u64,
    /// `X₁ = X₀ + q`.
    pub x1: u64,
    /// The binding per-market cap `OI_cap = min(SPEC impact cap, fee-scaled cap)` on `X₁`.
    pub oi_cap: u64,
}

/// ARCH §3.4 step 6 (ECON §6.1), in order: the SPEC per-position impact cap and the
/// fee-scaled per-position cap (`CapPositionImpact`); the SPEC per-market impact cap and the
/// fee-scaled per-market cap on `ΣB_Q + q` (`CapMarketOi`); utilisation after the open ≤
/// `util_cap_bps` (`CapUtilization`); Tier D's `ΣQ + q ≤ γ·(rT + q)` (`CapCurveGamma`); the
/// wallet cap `exposure + q ≤ wallet_cap_bps·OI_cap/10⁴` (`CapWallet`); `q ≤ T`
/// (`InsufficientInventory`).
pub fn check_open(inp: &CapInputs, tier: &TierParams, params: &Params) -> Result<CapOutputs> {
    let x0 = caps::conservative_x0(inp.x_now, inp.l_slow, inp.p_ema, inp.raydium_lock).math()?;
    let q = inp.q;
    require_gte!(
        caps::max_q_position(x0, tier.impact_pos_bps).math()?,
        q,
        ShortwinError::CapPositionImpact
    );
    if let Some(cap) = caps::fee_size_cap(x0, inp.phi_ppm, params.fee_size_cap_x100).math()? {
        require_gte!(cap, q, ShortwinError::CapPositionImpact);
    }
    let x1 = x0.checked_add(q).or_math()?;
    let oi_cap =
        caps::oi_cap(x1, tier.impact_mkt_bps, inp.phi_ppm, params.fee_oi_cap_x100).math()?;
    let total = inp.borrowed_q.checked_add(q).or_math()?;
    require_gte!(oi_cap, total, ShortwinError::CapMarketOi);
    require!(
        caps::util_ok(inp.tokens_free, inp.borrowed_q, q, params.util_cap_bps).math()?,
        ShortwinError::CapUtilization
    );
    if let Some(rt) = inp.curve {
        require!(
            caps::curve_gamma_ok(inp.borrowed_q, q, rt, params.curve_oi_gamma_bps).math()?,
            ShortwinError::CapCurveGamma
        );
    }
    require!(
        caps::wallet_ok(inp.wallet_q, q, oi_cap, params.wallet_cap_bps).math()?,
        ShortwinError::CapWallet
    );
    require_gte!(inp.tokens_free, q, ShortwinError::InsufficientInventory);
    Ok(CapOutputs { x0, x1, oi_cap })
}
