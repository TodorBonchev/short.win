//! Tier assignment at open and at crank (ARCH §2.1 `engine/tier.rs`, §3.4 step 5, §3.9
//! `crank_price`, §4.7; ECON §6.2–§6.3): assembles `shortwin_math::tier::TierInputs` from the
//! market, its venue state and the oracle cache, and calls `tier::assign`. Tiers apply to new
//! opens only; open positions keep their locked terms (A26).

use shortwin_math::quote::reprice;
use shortwin_math::tier::{self, OraclePrice, Tier, TierInputs, TierVenue};

use crate::state::{GlobalConfig, Market, OracleCache, VenueKind};
use crate::venues::VenueState;

/// The tier venue of a market (ARCH §4.7): the curve kinds are Tier D; PumpSwap and the mock
/// (a CPMM stand-in without pool selection, so without a locked-depth bound) price on
/// `min(R_now, R_slow)`; Raydium adds the locked-depth bound `D / λ_min` (V2-8).
pub fn tier_venue(config: &GlobalConfig, market: &Market, kind: VenueKind) -> Option<TierVenue> {
    match kind {
        VenueKind::PumpCurve | VenueKind::PumpMigrating => Some(TierVenue::PumpCurve),
        VenueKind::PumpSwap => Some(TierVenue::PumpSwap),
        VenueKind::RaydiumV4 | VenueKind::RaydiumCpmm => Some(TierVenue::Raydium {
            locked_depth: market.pool_d.get(),
            lambda_min_bps: config.params.pool_lock_min_bps,
        }),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => Some(TierVenue::PumpSwap),
        VenueKind::None => None,
    }
}

/// `tier::assign` for `market` now (ARCH §4.7): `R_now` the venue's real SOL, `Y(reprice(L_slow,
/// P_ema))` for `R_slow`, VQR 0 (the PumpSwap reader supplies it in M6), age from
/// `graduated_at`, the cached oracle (a stale or empty cache tiers as C, ADOPTED Q8). `None`
/// for a list-only market (no opens).
pub fn assign(
    config: &GlobalConfig,
    market: &Market,
    state: &VenueState,
    oracle: &OracleCache,
    now: i64,
) -> Option<Tier> {
    let venue = tier_venue(config, market, state.kind)?;
    let p_ema = market.ema_price.get();
    let y_slow = reprice(market.liq_slow.get(), p_ema).map_or(0, |(_, y)| y);
    Some(tier::assign(&TierInputs {
        venue,
        r_now: state.r,
        y_slow,
        vqr: 0,
        p_ema,
        now,
        graduated_at: market.graduated_at,
        oracle: OraclePrice {
            price: oracle.price,
            expo: oracle.expo,
            publish_time: oracle.publish_time,
        },
        oracle_stale_s: config.params.oracle_stale_s,
        theta_a_usd_e6: config.params.theta_a_usd_e6,
        theta_b_usd_e6: config.params.theta_b_usd_e6,
    }))
}
