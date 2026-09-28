//! Route predicates and chains (ARCH §2.1 `engine/route.rs`, §3.0 step 8, §5.3; A23, A50, A71;
//! C0-4, C0-10, V2-1, R3A-4, R3B-8).
//!
//! Three predicates, each with one job:
//!
//! - **`venue_executable`**: [`VenueState::executable`] (the venue predicate, and every
//!   venue-state account parses, A63). Only it drives `B̂_live`, closes, liquidations,
//!   expiries, the "no route ⇒ spot leg true" rule and observation recording.
//! - **`s3_no_route`** = `!venue_executable ∨ market.route_broken ∨ venue_route_broken[venue] ∨
//!   φ_live > config.venue_fee_hard_cap_bps` ([`s3_no_route`]): the S3 chain, `sol_settle`'s
//!   "no route now" and the open gate only.
//! - **`pool_fail`** = `!venue_executable ∨ φ_live > config.venue_fee_hard_cap_bps`
//!   ([`pool_fail`]): the Raydium fast-path chain only. It never reads the Emergency flag
//!   (T-S3-BROKEN).
//!
//! Both market-level predicates use the **global** hard cap (R3A-4 c). The formulas are
//! `shortwin_math::settle::{s3_no_route, pool_fail, chain_step}`.

use shortwin_math::settle;

use crate::state::{GlobalConfig, Market, VenueKind, MARKET_FLAG_ROUTE_BROKEN};
use crate::venues::VenueState;

/// The Emergency route-broken flag of the market or of its venue (A40; O6). Opens and S3
/// evidence only: never an input to a quote, a leg, an exit or `pool_fail` (I10, I16).
pub fn route_broken(config: &GlobalConfig, market: &Market, kind: VenueKind) -> bool {
    market.has_flag(MARKET_FLAG_ROUTE_BROKEN)
        || (kind.venue_bit() != 0 && config.venue_route_broken & kind.venue_bit() != 0)
}

/// `s3_no_route` of the market now (module docs).
pub fn s3_no_route(config: &GlobalConfig, market: &Market, state: &VenueState) -> bool {
    settle::s3_no_route(
        state.executable,
        route_broken(config, market, state.kind),
        state.fee_ppm,
        u64::from(config.params.venue_fee_hard_cap_bps),
    )
}

/// `pool_fail` of the market now (module docs); never reads the flag.
pub fn pool_fail(config: &GlobalConfig, state: &VenueState) -> bool {
    settle::pool_fail(
        state.executable,
        state.fee_ppm,
        u64::from(config.params.venue_fee_hard_cap_bps),
    )
}

/// What [`observe_chains`] saw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chains {
    pub s3_no_route: bool,
    pub pool_fail: bool,
    /// A chain started or cleared (`no_route_since` or `pool_fail_since` changed;
    /// `crank_price` then emits `RouteChainChanged`). A continuing chain only refreshes its
    /// `last_*_obs` and is not a change.
    pub changed: bool,
}

/// ARCH §3.0 step 8: one observation of the S3 chain (`no_route_since`, `last_no_route_obs`)
/// with `s3_no_route`, and on Raydium markets (and the mock, their stand-in) of the fast-path
/// chain (`pool_fail_since`, `last_pool_fail_obs`) with `pool_fail`
/// (`settle::chain_step`: a restart after more than `S3_OBS_GAP_S`, a clear on a routed
/// observation; C0-16).
pub fn observe_chains(
    config: &GlobalConfig,
    market: &mut Market,
    state: &VenueState,
    now: i64,
) -> Chains {
    let before = (market.no_route_since, market.pool_fail_since);
    let s3 = s3_no_route(config, market, state);
    (market.no_route_since, market.last_no_route_obs) =
        settle::chain_step(market.no_route_since, market.last_no_route_obs, now, s3);
    let pf = pool_fail(config, state);
    if state.kind.has_pool_fail_chain() {
        (market.pool_fail_since, market.last_pool_fail_obs) =
            settle::chain_step(market.pool_fail_since, market.last_pool_fail_obs, now, pf);
    }
    let after = (market.no_route_since, market.pool_fail_since);
    Chains {
        s3_no_route: s3,
        pool_fail: pf,
        changed: before != after,
    }
}
