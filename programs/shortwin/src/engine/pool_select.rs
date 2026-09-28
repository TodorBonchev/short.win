//! Raydium pool selection (ARCH §2.1 `engine/pool_select.rs`, §2.5 `None`, §2.6 as amended by
//! A60, A71 and decision 7, §3.9 `propose_pool` / `execute_pool_switch`, §4.4
//! `rewrite_liquidity`; ECON §6.2, §10.3), with the M4 security review's amendments (M4-SEC-1 to
//! M4-SEC-6, M4-SEC-8; the deviations from ARCH v0.4.1 are listed below). The arithmetic (`D`,
//! the eligibility floor, the margins, the adoption liquidity) is `shortwin_math::pool_select`;
//! the candidate's verification and `eligible()` are the adapters' (`venues::raydium_v4::verify`,
//! `venues::raydium_cpmm::verify`); this module decides what a proposal does and applies an
//! adoption to the market.
//!
//! **What a proposal does** ([`decide`], on the market after the prologue of the proposing
//! instruction, so `pool_d` and the `pool_fail` chain include this instruction's observation of
//! the incumbent):
//!
//! | Market | Condition | Outcome |
//! |---|---|---|
//! | `None` (list-only), no open position | — | adopt ([`POOL_KIND_LISTING`]) |
//! | `None` with open positions (M5: a curve that graduated elsewhere; M4-SEC-4) | a collection window running: `D_cand > fp_d`; none: — | replace the window's candidate / open a window of `pool_fast_window_s` |
//! | Raydium, no open position, inside the listing window | `D_cand ≥ (1 + pool_list_margin_bps/10⁴)·D_inc` | adopt ([`POOL_KIND_LISTING_WINDOW`]) |
//! | Raydium, no open position, after the window | `D_cand ≥ (1 + pool_switch_margin_bps/10⁴)·D_inc` | adopt ([`POOL_KIND_ZERO_POSITION`]) |
//! | Raydium, a collection window running and the `pool_fail` chain that armed it unbroken | `D_cand > fp_d` and `D_cand ≥ pool_fast_min_bps·D_inc/10⁴` | replace the window's candidate |
//! | any, a collection window run out — or, on Raydium, its chain broken (M4-SEC-2) | — | `PoolWindowClosed` (`execute_pool_switch` resolves it first) |
//! | Raydium, the fast path armed (`pool_fail` now, the chain ≥ `pool_fast_route_s` old) | `D_cand ≥ pool_fast_min_bps·D_inc/10⁴` | open a window of `pool_fast_window_s` |
//! | otherwise | | `PositionsOpen` with an open position, else `PoolNotDominant` |
//!
//! `D_inc = pool_d`, the incumbent's locked depth at its last executable observation. A non-fast
//! adoption requires `open_positions == 0` **inside the listing window too, and on a list-only
//! market** (decision 7; I15: "a non-fast adoption happened only with `open_positions == 0`"): a
//! listing window can follow a fast path taken with positions open, and those positions must
//! not see a second pool change that no failure caused; and positions on a list-only market
//! (they exist from M5, when a live curve completes with zeroed reserves and no canonical pool,
//! ARCH §2.5) must not be pinned to whichever eligible pool is proposed first — they go through a
//! collection window and `execute_pool_switch` adopts the deepest candidate seen (M4-SEC-4; the
//! market has no route at all, so no arming delay). The fast path works whether or not positions
//! are open; a candidate that also qualifies for a zero-position adoption is adopted at once.
//!
//! **Execution** ([`window_adoptable`]): `execute_pool_switch` adopts the window's candidate only
//! if, on a Raydium market, the `pool_fail` chain that armed the window is still unbroken —
//! `pool_fail` now, `pool_fail_since ≠ 0` and `pool_fail_since + pool_fast_route_s ≤ fp_t0`
//! (M4-SEC-2: a pool that recovered after the window opened and failed again for one second must
//! not be switched away from; a stale window is not a standing licence). A list-only market's
//! window needs no chain. A candidate that is no longer eligible, fails the window floor, or
//! whose program is now latched (M4V-4) is not adoptable now: the window is cancelled.
//!
//! **Eligibility** (the adapters, `raydium::check_eligible`; M4-SEC-1, M4-SEC-3): executable,
//! fee ≤ `venue_fee_cap_bps`, and the locked-depth floor `⌊⌊D·isqrt(P_ref)/10⁹⌋·10⁴/λ_min⌋ ≥
//! pool_min_real_sol` with `P_ref` = the market's `P_ema` ([`reference_price`]) — or the
//! candidate's spot on a market that never had a price. No clause reads `λ_now` or the live `R`.
//! A collection window's candidate — its adoption moves open positions — also needs the **window
//! floor** ([`window_floor_ok`], M4V-1): its locked SOL at a reference price itself,
//! `⌊D·isqrt(P)/10⁹⌋ ≥ pool_min_real_sol` (no `/λ_min`). A proposal that opens or improves the
//! window judges it at `P = min(P_ema, P_cand)` ([`window_price`]; M4R-2, **PROPOSED**, owner item
//! O18: after a dump during the incumbent's halt the carried-over `P_ema` is stale-high, and at it
//! a pool far under the floor passed), `PoolTooThin`; the execution judges it again at `P_ema`
//! (else cancelled).
//!
//! **Every adoption** ([`adopt`]; ARCH §2.6 rule 4 as amended by M4R, **PROPOSED**, owner item
//! O18): the venue kind may change between AMM v4 and CPMM; the pool itself
//! ([`Adoption::write_pool`]: venue, pool cache, SOL side, `pool_d`); `P_ema` and the ring's prices
//! carry over; `L_ema` and every ring entry's `l`, `llo`, `lhi` become the **anchor** `cap =
//! ⌊D·10⁴/λ_min⌋` (`pool_select::depth_at_lambda`) — no sample of the new pool, which a deposit
//! or an unlocked LP's withdrawal around the adopting instruction would move (M4R-1) — each
//! entry's `(x, y)` recomputed, and `L_slow = min(L_slow, D)` (never raised); the open bucket
//! cleared (A72; `observe::rewrite_liquidity`, `VenueChange::RaydiumLocked`); then the state is
//! marked unconfirmed (`observe::mark_restart`: `ema_warm = 8`, `ema_guard = 1`), so the next
//! eight integrations re-seed `P_ema` and `L_ema` from the new pool's buckets on the side that
//! cannot force an exit, opens and keeper exits wait for them, and the exit reference reads the
//! ring's liquidity lows until they agree with `L_ema`. A list-only market that never had a price
//! starts the price state of the pool (`observe::initial_raydium`: its spot, `min(√(X·Y), cap)`,
//! marked) as a listing does; both route chains and the collection window cleared;
//! `graduated_at` kept (age is the token's time on an eligible pool), except that a `None` market
//! takes the candidate's `age_start` as a listing would (O13, O13b); `pool_seen_at = now` only if
//! the market is not already inside a listing window.
//!
//! **Deviations from ARCH v0.4.1** (the M4 security review and its verification, M4V; owner
//! item O18 records the rules): eligibility's `R ≥ pool_min_real_sol` and `λ ≥ pool_lock_min_bps`
//! are replaced by the locked-depth floor at `P_ref` (§2.6 "Eligibility", rule 5's last
//! sentence), and a window's candidate needs its locked SOL alone at the SOL floor (M4V-1);
//! rule 4's `L_ema = min(√(X·Y)_new, current)` is the anchor and the post-change warm-up above
//! and `L_slow` falls to at most `D`; the execution also requires the arming chain unbroken (rule 3 names only
//! `pool_fail`) and cancels on a latched candidate program (M4V-4); a list-only market with open
//! positions adopts through a collection window (§2.5 "a Raydium pool can then be adopted through
//! `propose_pool`"); the latch reads the candidate program's bit only (§3.9 names none).

use anchor_lang::prelude::*;
use shortwin_math::observe::VenueChange;
use shortwin_math::pool_select::{depth_at_lambda, dominates, fast_path_qualifies};

use crate::engine::observe;
use crate::errors::{CheckedExt, ShortwinError};
use crate::events::{POOL_KIND_LISTING, POOL_KIND_LISTING_WINDOW, POOL_KIND_ZERO_POSITION};
use crate::state::{Market, Params, VenueKind};
use crate::venues::raydium::Adoption;
use crate::venues::{raydium, raydium_cpmm, raydium_v4};

/// The pool-selection parameters (ARCH §2.3.3; `GlobalConfig.params`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectParams {
    /// `pool_list_window_s`: opens wait this long after an adoption outside a window.
    pub list_window_s: u32,
    /// `pool_list_margin_bps`: the replacement margin inside the listing window (5 %).
    pub list_margin_bps: u32,
    /// `pool_switch_margin_bps`: the zero-position margin after the window (25 %).
    pub switch_margin_bps: u32,
    /// `pool_fast_route_s`: how long `pool_fail` must hold before the fast path (120 s).
    pub fast_route_s: u32,
    /// `pool_fast_min_bps`: a fast-path candidate's minimum depth, bps of `pool_d` (25 %).
    pub fast_min_bps: u32,
    /// `pool_fast_window_s`: the collection window (60 s).
    pub fast_window_s: u32,
}

impl SelectParams {
    /// The parameters of `params`.
    pub fn of(params: &Params) -> Self {
        Self {
            list_window_s: params.pool_list_window_s,
            list_margin_bps: params.pool_list_margin_bps,
            switch_margin_bps: params.pool_switch_margin_bps,
            fast_route_s: params.pool_fast_route_s,
            fast_min_bps: params.pool_fast_min_bps,
            fast_window_s: params.pool_fast_window_s,
        }
    }
}

/// The market as a proposal sees it, after the proposing instruction's prologue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Situation {
    /// The incumbent venue: `None` or a Raydium kind.
    pub venue: VenueKind,
    /// The market time (`prologue::market_now`).
    pub now: i64,
    pub open_positions: u32,
    pub pool_seen_at: i64,
    /// `D_inc`: the incumbent's locked depth at its last executable observation.
    pub pool_d: u128,
    /// `pool_fail` of this instruction's observation of the incumbent.
    pub pool_fail: bool,
    /// The fast-path chain after this instruction's observation.
    pub pool_fail_since: i64,
    /// The collection window (`fp_t0 = 0`: none).
    pub fp_t0: i64,
    pub fp_d: u128,
}

impl Situation {
    /// The situation of `market` (on `venue`) at `now` with this instruction's `pool_fail`.
    pub fn of(market: &Market, venue: VenueKind, now: i64, pool_fail: bool) -> Self {
        Self {
            venue,
            now,
            open_positions: market.open_positions,
            pool_seen_at: market.pool_seen_at,
            pool_d: market.pool_d.get(),
            pool_fail,
            pool_fail_since: market.pool_fail_since,
            fp_t0: market.fp_t0,
            fp_d: market.fp_d.get(),
        }
    }
}

/// What a proposal does ([`decide`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proposal {
    /// Adopt the candidate now, with its `POOL_KIND_*`.
    Adopt(u8),
    /// Open a fast-path collection window on the candidate.
    OpenWindow,
    /// Replace the running window's candidate with this deeper one.
    ImproveWindow,
}

/// `now < pool_seen_at + pool_list_window_s`: inside the listing window (ARCH §2.6 rule 1).
pub fn in_listing_window(pool_seen_at: i64, list_window_s: u32, now: i64) -> bool {
    pool_seen_at
        .checked_add(i64::from(list_window_s))
        .is_some_and(|end| now < end)
}

/// The end of the listing window that starts at `pool_seen_at`.
pub fn listing_window_ends(pool_seen_at: i64, list_window_s: u32) -> Result<i64> {
    pool_seen_at.checked_add(i64::from(list_window_s)).or_math()
}

/// The end of the collection window opened at `fp_t0`.
pub fn window_ends(fp_t0: i64, fast_window_s: u32) -> Result<i64> {
    fp_t0.checked_add(i64::from(fast_window_s)).or_math()
}

/// The fast path is armed: `pool_fail` holds now and has held continuously for
/// `pool_fast_route_s` (ARCH §2.6 rule 3). Never reads the route-broken flag (V1-7): the chain
/// is fed by `pool_fail` alone (`engine::route`).
pub fn fast_path_armed(s: &Situation, p: &SelectParams) -> bool {
    s.pool_fail
        && s.pool_fail_since != 0
        && s.now.saturating_sub(s.pool_fail_since) >= i64::from(p.fast_route_s)
}

/// The `pool_fail` chain that armed a Raydium market's collection window is unbroken
/// (M4-SEC-2): `pool_fail` now, the chain running (`pool_fail_since ≠ 0`) and started at least
/// `pool_fast_route_s` before the window opened (`pool_fail_since + pool_fast_route_s ≤ fp_t0`).
/// A window opens only on an armed fast path, so a chain that started later is one that broke
/// (a routed observation cleared it, or observations more than `S3_OBS_GAP_S` apart restarted
/// it) and began again after the window opened. `false` without a window.
pub fn window_chain_holds(s: &Situation, p: &SelectParams) -> bool {
    s.fp_t0 != 0
        && s.pool_fail
        && s.pool_fail_since != 0
        && s.pool_fail_since
            .checked_add(i64::from(p.fast_route_s))
            .is_some_and(|armed| armed <= s.fp_t0)
}

/// A collection window may still lead to an adoption (M4-SEC-2, M4-SEC-4): on a Raydium market
/// iff [`window_chain_holds`]; on a list-only market (no venue, no chain: its positions have no
/// route at all) always. `false` without a window.
pub fn window_adoptable(s: &Situation, p: &SelectParams) -> bool {
    match s.venue {
        VenueKind::None => s.fp_t0 != 0,
        VenueKind::RaydiumV4 | VenueKind::RaydiumCpmm => window_chain_holds(s, p),
        _ => false,
    }
}

/// What a proposal of a candidate with locked depth `d_cand` does (module docs). The caller has
/// verified the candidate (`eligible()`) and refused pump-venue markets (D1).
///
/// Errors: `PoolWindowClosed` (a collection window ran out, or its arming chain broke, and
/// waits for `execute_pool_switch`), `PositionsOpen` (no failure, positions open),
/// `PoolNotDominant` (the candidate misses every margin that applies), `NoSupportedVenue` (not a
/// `None` or Raydium market).
pub fn decide(s: &Situation, d_cand: u128, p: &SelectParams) -> Result<Proposal> {
    match s.venue {
        VenueKind::None => {
            if s.open_positions == 0 {
                return Ok(Proposal::Adopt(POOL_KIND_LISTING));
            }
            // M4-SEC-4: positions on a list-only market collect the deepest candidate.
            if s.fp_t0 != 0 {
                require!(
                    s.now < window_ends(s.fp_t0, p.fast_window_s)?,
                    ShortwinError::PoolWindowClosed
                );
                require!(d_cand > s.fp_d, ShortwinError::PoolNotDominant);
                return Ok(Proposal::ImproveWindow);
            }
            return Ok(Proposal::OpenWindow);
        }
        VenueKind::RaydiumV4 | VenueKind::RaydiumCpmm => {}
        _ => return err!(ShortwinError::NoSupportedVenue),
    }
    if s.open_positions == 0 {
        let inside = in_listing_window(s.pool_seen_at, p.list_window_s, s.now);
        let (margin, kind) = if inside {
            (p.list_margin_bps, POOL_KIND_LISTING_WINDOW)
        } else {
            (p.switch_margin_bps, POOL_KIND_ZERO_POSITION)
        };
        if dominates(d_cand, s.pool_d, u64::from(margin)) {
            return Ok(Proposal::Adopt(kind));
        }
    }
    let qualifies = fast_path_qualifies(d_cand, s.pool_d, u64::from(p.fast_min_bps));
    if s.fp_t0 != 0 {
        require!(
            s.now < window_ends(s.fp_t0, p.fast_window_s)? && window_chain_holds(s, p),
            ShortwinError::PoolWindowClosed
        );
        require!(d_cand > s.fp_d && qualifies, ShortwinError::PoolNotDominant);
        return Ok(Proposal::ImproveWindow);
    }
    if fast_path_armed(s, p) {
        require!(qualifies, ShortwinError::PoolNotDominant);
        return Ok(Proposal::OpenWindow);
    }
    require!(s.open_positions == 0, ShortwinError::PositionsOpen);
    err!(ShortwinError::PoolNotDominant)
}

/// The reference price of eligibility (M4-SEC-3): the market's `P_ema` when it has a price (a
/// Raydium market; a list-only market that had one), `None` otherwise — then the candidate's
/// own spot is used (`raydium::check_eligible`).
pub fn reference_price(market: &Market) -> Option<u128> {
    let p = market.ema_price.get();
    (market.ring_count > 0 && p > 0).then_some(p)
}

/// Verifies a candidate ReadSet `read` of kind `kind` for the market's `mint` under
/// `token_program` and checks `eligible()` at `p_ref` ([`reference_price`]; ARCH §2.6; the
/// adapters' `verify`).
///
/// Errors: `NoSupportedVenue` for a kind other than the two Raydium ones; the adapters'
/// `BadRemainingAccounts`, `VenueAccountMismatch`, `PoolIneligible`, `PoolTooThin`.
pub fn verify_candidate(
    kind: VenueKind,
    read: &[AccountInfo],
    mint: &Pubkey,
    token_program: &Pubkey,
    params: &Params,
    now: i64,
    p_ref: Option<u128>,
) -> Result<Adoption> {
    match kind {
        VenueKind::RaydiumV4 => raydium_v4::verify(read, mint, token_program, params, now, p_ref),
        VenueKind::RaydiumCpmm => {
            raydium_cpmm::verify(read, mint, token_program, params, now, p_ref)
        }
        _ => err!(ShortwinError::NoSupportedVenue),
    }
}

/// The window floor of a collection window's candidate `a` (M4V-1; module docs): its locked SOL at
/// `p_ref` (the candidate's spot on a market without a price) reaches `pool_min_real_sol`
/// (`raydium::position_floor_ok`). A proposal passes [`window_price`], the execution
/// [`reference_price`].
pub fn window_floor_ok(a: &Adoption, params: &Params, p_ref: Option<u128>) -> bool {
    raydium::position_floor_ok(&a.state, params, p_ref)
}

/// The price a proposal that opens or improves a collection window judges the window floor at
/// (M4R-2, **PROPOSED**, owner item O18; module docs): `min(P_ema, P_cand)`, `P_ema` the market's
/// reference price ([`reference_price`]) and `P_cand` the candidate's pre-trade spot `⌊Y·WAD/X⌋`
/// in this instruction; `P_cand` alone on a market without a price (as before). After a dump
/// during the incumbent's halt `P_ema` is frozen high, and the candidate's locked SOL at it is
/// `floor·√(P_ema/P_live)` above the real one; the live spot closes that for an honest dump. A
/// proposer can only raise `P_cand` for its own proposal (a pump around it), never above
/// `P_ema`, so the floor is never looser than at `P_ema`.
pub fn window_price(p_ref: Option<u128>, a: &Adoption) -> Option<u128> {
    match (p_ref, a.state.price()) {
        (Some(e), Some(c)) => Some(e.min(c)),
        (e, c) => e.or(c),
    }
}

/// Clears the fast-path collection window.
pub fn clear_window(market: &mut Market) {
    market.fp_keys = [Pubkey::default(); 6];
    market.fp_d.set(0);
    market.fp_t0 = 0;
    market.fp_venue = 0;
    market._pad1 = 0;
    market._pad1a = [0; 2];
    market._pad1b = [0; 4];
}

/// Records `a` as the collection window's candidate (`fp_keys`, `fp_d`, `fp_venue`); `open` also
/// starts the window at `now` (`fp_t0`), an improvement keeps its start. Nothing of the pool's
/// liquidity is recorded (M4R: the adoption reads no sample).
pub fn record_window(market: &mut Market, a: &Adoption, now: i64, open: bool) {
    market.fp_keys = a.window_keys();
    market.fp_d.set(a.state.locked_depth);
    market.fp_venue = a.kind as u8;
    if open {
        market.fp_t0 = now;
    }
}

/// Adopts `a` on `market` at `now` (module docs, ARCH §2.6 rule 4 as amended by M4R): the pool;
/// on a market with a price the liquidity rewrite to the anchor `cap = ⌊D·10⁴/λ_min⌋` with `L_slow
/// ≤ D`, then the restart marks (`ema_warm`, `ema_guard`); on a list-only market that never had a
/// price a fresh, marked price state at the pool's spot with `L = min(√(X·Y), cap)`; both route
/// chains and the collection window cleared, `graduated_at` kept (a `None` market: the pool's
/// `age_start`), `pool_seen_at = now` unless the market is inside a listing window.
/// `lambda_min_bps` is `pool_lock_min_bps`. The priced path reads no liquidity of the pool.
///
/// Errors: `PoolIneligible` for a candidate without a defined price or liquidity on a market
/// without a price (an eligible pool always has both), `NoSupportedVenue` for a corrupt venue
/// byte, `MathOverflow` for a zero locked depth (an eligible pool has `D > 0`).
pub fn adopt(
    market: &mut Market,
    a: &Adoption,
    now: i64,
    list_window_s: u32,
    lambda_min_bps: u32,
) -> Result<()> {
    let from = market.venue_kind().ok_or(ShortwinError::NoSupportedVenue)?;
    let inside = from.is_raydium() && in_listing_window(market.pool_seen_at, list_window_s, now);
    let d = a.state.locked_depth;
    let cap = depth_at_lambda(d, u64::from(lambda_min_bps));
    let priced = market.ring_count > 0 && market.ema_price.get() > 0;
    a.write_pool(market)?;
    if from == VenueKind::None {
        market.graduated_at = a.age_start;
    }
    if !inside {
        market.pool_seen_at = now;
    }
    if priced {
        let mut obs = market.observation();
        observe::rewrite_liquidity(&mut obs, cap, VenueChange::RaydiumLocked { slow_cap: d })?;
        obs.mark_restart();
        market.set_observation(&obs);
    } else {
        let obs = observe::initial_raydium(now, &a.state, cap)?;
        market.set_observation(&obs);
    }
    market.no_route_since = 0;
    market.last_no_route_obs = 0;
    market.pool_fail_since = 0;
    market.last_pool_fail_obs = 0;
    clear_window(market);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::venues::VenueState;
    use bytemuck::Zeroable;
    use shortwin_math::num::WAD;
    use shortwin_math::quote::reprice;

    fn code(r: Result<Proposal>) -> Option<u32> {
        match r {
            Err(anchor_lang::error::Error::AnchorError(e)) => Some(e.error_code_number),
            _ => None,
        }
    }

    const P: SelectParams = SelectParams {
        list_window_s: 3_600,
        list_margin_bps: 500,
        switch_margin_bps: 2_500,
        fast_route_s: 120,
        fast_min_bps: 2_500,
        fast_window_s: 60,
    };

    /// A healthy Raydium market listed at 1,000, now 10,000, `pool_d` 10,000, no window.
    fn healthy() -> Situation {
        Situation {
            venue: VenueKind::RaydiumV4,
            now: 10_000,
            open_positions: 0,
            pool_seen_at: 1_000,
            pool_d: 10_000,
            pool_fail: false,
            pool_fail_since: 0,
            fp_t0: 0,
            fp_d: 0,
        }
    }

    #[test]
    fn a_list_only_market_adopts_any_eligible_candidate() {
        let s = Situation {
            venue: VenueKind::None,
            pool_d: 0,
            ..healthy()
        };
        assert_eq!(
            decide(&s, 1, &P).unwrap(),
            Proposal::Adopt(POOL_KIND_LISTING)
        );
    }

    /// M4-SEC-4: a list-only market with open positions (M5's curve that graduated elsewhere)
    /// is never pinned to the first eligible proposal: it collects candidates in a window of
    /// `pool_fast_window_s` (no arming — it has no route at all), keeps the deepest, and its
    /// execution needs no chain; once its positions are gone it adopts at once again.
    #[test]
    fn a_list_only_market_with_positions_collects_the_deepest_candidate() {
        let none = Situation {
            venue: VenueKind::None,
            pool_d: 0,
            open_positions: 3,
            ..healthy()
        };
        assert_eq!(decide(&none, 1, &P).unwrap(), Proposal::OpenWindow);
        let open = Situation {
            fp_t0: 10_000,
            fp_d: 1,
            ..none
        };
        assert!(
            window_adoptable(&open, &P),
            "no chain on a list-only market"
        );
        let later = Situation {
            now: 10_059,
            ..open
        };
        assert_eq!(
            decide(&later, 1_000_000, &P).unwrap(),
            Proposal::ImproveWindow
        );
        assert_eq!(
            code(decide(&later, 1, &P)),
            Some(u32::from(ShortwinError::PoolNotDominant))
        );
        let closed = Situation {
            now: 10_060,
            ..open
        };
        assert_eq!(
            code(decide(&closed, u128::MAX, &P)),
            Some(u32::from(ShortwinError::PoolWindowClosed))
        );
        let drained = Situation {
            open_positions: 0,
            ..closed
        };
        assert_eq!(
            decide(&drained, 1, &P).unwrap(),
            Proposal::Adopt(POOL_KIND_LISTING)
        );
        // The Raydium market it becomes refuses a much deeper pool while positions are open
        // (decision 7) — which is why the first adoption must already be the deepest.
        let after = Situation {
            venue: VenueKind::RaydiumCpmm,
            now: 10_100,
            pool_seen_at: 10_060,
            pool_d: 1_000_000,
            fp_t0: 0,
            fp_d: 0,
            ..none
        };
        assert_eq!(
            code(decide(&after, u128::MAX, &P)),
            Some(u32::from(ShortwinError::PositionsOpen))
        );
    }

    #[test]
    fn zero_position_margins_inside_and_after_the_listing_window() {
        let after = healthy();
        assert_eq!(
            decide(&after, 12_500, &P).unwrap(),
            Proposal::Adopt(POOL_KIND_ZERO_POSITION)
        );
        assert_eq!(
            code(decide(&after, 12_499, &P)),
            Some(u32::from(ShortwinError::PoolNotDominant))
        );
        let inside = Situation {
            pool_seen_at: 9_000,
            ..healthy()
        };
        assert!(in_listing_window(9_000, 3_600, 10_000));
        assert!(!in_listing_window(9_000, 3_600, 12_600));
        assert_eq!(
            decide(&inside, 10_500, &P).unwrap(),
            Proposal::Adopt(POOL_KIND_LISTING_WINDOW)
        );
        assert_eq!(
            code(decide(&inside, 10_499, &P)),
            Some(u32::from(ShortwinError::PoolNotDominant))
        );
        // The window's last second is still inside; the next is not.
        let edge = Situation {
            now: 9_000 + 3_599,
            ..inside
        };
        assert_eq!(
            decide(&edge, 10_500, &P).unwrap(),
            Proposal::Adopt(POOL_KIND_LISTING_WINDOW)
        );
        let out = Situation {
            now: 9_000 + 3_600,
            ..inside
        };
        assert_eq!(
            code(decide(&out, 10_500, &P)),
            Some(u32::from(ShortwinError::PoolNotDominant))
        );
    }

    #[test]
    fn open_positions_block_every_non_fast_adoption() {
        for seen in [1_000, 9_500] {
            let s = Situation {
                open_positions: 1,
                pool_seen_at: seen,
                ..healthy()
            };
            assert_eq!(
                code(decide(&s, u128::MAX, &P)),
                Some(u32::from(ShortwinError::PositionsOpen))
            );
        }
    }

    #[test]
    fn the_fast_path_arms_after_pool_fast_route_s_of_pool_fail() {
        let failing = |since: i64, positions: u32| Situation {
            open_positions: positions,
            pool_fail: true,
            pool_fail_since: since,
            ..healthy()
        };
        // 119 s: not armed; 120 s: armed, whatever the positions.
        for positions in [0, 5] {
            let not_yet = failing(10_000 - 119, positions);
            assert!(!fast_path_armed(&not_yet, &P));
            let armed = failing(10_000 - 120, positions);
            assert!(fast_path_armed(&armed, &P));
            assert_eq!(decide(&armed, 2_500, &P).unwrap(), Proposal::OpenWindow);
            assert_eq!(
                code(decide(&armed, 2_499, &P)),
                Some(u32::from(ShortwinError::PoolNotDominant))
            );
        }
        // Not armed without this instruction's pool_fail (the chain alone is not enough).
        let stale = Situation {
            pool_fail: false,
            ..failing(1, 1)
        };
        assert!(!fast_path_armed(&stale, &P));
        assert_eq!(
            code(decide(&stale, u128::MAX, &P)),
            Some(u32::from(ShortwinError::PositionsOpen))
        );
        // With no position a dominant candidate is adopted at once, armed or not.
        assert_eq!(
            decide(&failing(1, 0), 12_500, &P).unwrap(),
            Proposal::Adopt(POOL_KIND_ZERO_POSITION)
        );
    }

    #[test]
    fn the_collection_window_keeps_the_deepest_candidate() {
        let open = Situation {
            open_positions: 2,
            pool_fail: true,
            pool_fail_since: 9_000,
            fp_t0: 9_990,
            fp_d: 5_000,
            ..healthy()
        };
        assert!(window_chain_holds(&open, &P) && window_adoptable(&open, &P));
        assert_eq!(decide(&open, 5_001, &P).unwrap(), Proposal::ImproveWindow);
        for weaker in [5_000, 4_000, 2_499] {
            assert_eq!(
                code(decide(&open, weaker, &P)),
                Some(u32::from(ShortwinError::PoolNotDominant))
            );
        }
        // M4-SEC-2: once the chain that armed the window broke — the incumbent recovered (this
        // second's observation cleared it), or it recovered and failed again (the chain
        // restarted after the window opened), or it restarted after a gap — the window takes no
        // candidate and can only be cancelled.
        let recovered = Situation {
            pool_fail: false,
            pool_fail_since: 0,
            ..open
        };
        let restarted = Situation {
            pool_fail_since: 10_000 - 40,
            ..open
        };
        for broken in [recovered, restarted] {
            assert!(!window_chain_holds(&broken, &P) && !window_adoptable(&broken, &P));
            assert_eq!(
                code(decide(&broken, 6_000, &P)),
                Some(u32::from(ShortwinError::PoolWindowClosed))
            );
        }
        // The chain exactly `pool_fast_route_s` older than the window still holds.
        let edge = Situation {
            pool_fail_since: 9_990 - 120,
            ..open
        };
        assert!(window_chain_holds(&edge, &P));
        let late = Situation {
            pool_fail_since: 9_990 - 119,
            ..open
        };
        assert!(!window_chain_holds(&late, &P));
        assert!(!window_chain_holds(&Situation { fp_t0: 0, ..open }, &P));
        // The window's last second takes candidates; afterwards it waits for the execution.
        let last = Situation {
            now: 9_990 + 59,
            ..open
        };
        assert_eq!(decide(&last, 6_000, &P).unwrap(), Proposal::ImproveWindow);
        let closed = Situation {
            now: 9_990 + 60,
            ..open
        };
        assert_eq!(
            code(decide(&closed, u128::MAX, &P)),
            Some(u32::from(ShortwinError::PoolWindowClosed))
        );
        // With no position a dominant candidate still wins at once.
        let empty = Situation {
            open_positions: 0,
            ..closed
        };
        assert_eq!(
            decide(&empty, 12_500, &P).unwrap(),
            Proposal::Adopt(POOL_KIND_ZERO_POSITION)
        );
    }

    #[test]
    fn pump_venues_and_the_mock_never_select() {
        for venue in [
            VenueKind::PumpCurve,
            VenueKind::PumpMigrating,
            VenueKind::PumpSwap,
        ] {
            let s = Situation { venue, ..healthy() };
            assert_eq!(
                code(decide(&s, u128::MAX, &P)),
                Some(u32::from(ShortwinError::NoSupportedVenue))
            );
        }
    }

    /// An eligible-looking adoption of kind `kind` with reserves `(x, y)` and depth `d`.
    fn adoption(kind: VenueKind, x: u64, y: u64, d: u128, age_start: i64) -> Adoption {
        Adoption {
            kind,
            pool: Pubkey::new_unique(),
            vault_sol: Pubkey::new_unique(),
            vault_tok: Pubkey::new_unique(),
            amm_config: if kind == VenueKind::RaydiumCpmm {
                Pubkey::new_unique()
            } else {
                Pubkey::default()
            },
            observation: if kind == VenueKind::RaydiumCpmm {
                Pubkey::new_unique()
            } else {
                Pubkey::default()
            },
            lp_mint: Pubkey::new_unique(),
            tok_program: crate::constants::TOKEN_PROGRAM_ID,
            sol_first: false,
            state: VenueState {
                kind,
                executable: true,
                x,
                y,
                r: y,
                pricing: None,
                fee_ppm: 2_500,
                locked_depth: d,
            },
            age_start,
        }
    }

    /// M4R: a list-only market's first adoption starts the pool's price state at its spot with
    /// `L = min(√(X·Y), ⌊D·10⁴/λ_min⌋)`, marked for the post-change warm-up.
    #[test]
    fn a_list_only_market_starts_the_pools_price_state() {
        for (d, want_l) in [(1_900_000u128, 2_000_000u128), (900_000, 1_800_000)] {
            let mut m = Market::zeroed();
            m.venue = VenueKind::None as u8;
            m.no_route_since = 50;
            m.last_no_route_obs = 90;
            let a = adoption(VenueKind::RaydiumCpmm, 4_000_000, 1_000_000, d, 77);
            adopt(&mut m, &a, 100, 3_600, 5_000).unwrap();
            assert_eq!(m.venue_kind(), Some(VenueKind::RaydiumCpmm));
            assert_eq!((m.graduated_at, m.pool_seen_at), (77, 100));
            assert_eq!(m.pool_d.get(), d);
            assert_eq!(m.ema_price.get(), WAD / 4);
            assert_eq!((m.ema_liq.get(), m.liq_slow.get()), (want_l, want_l));
            assert_eq!((m.ring_count, m.ema_ts), (1, 100));
            assert_eq!((m.ema_warm, m.ema_guard), (8, 1), "marked");
            assert_eq!(
                (m.no_route_since, m.last_no_route_obs, m.pool_fail_since),
                (0, 0, 0)
            );
            assert_eq!(m.fp_t0, 0);
        }
    }

    /// M4R: every Raydium adoption of a priced market writes the anchor `⌊D·10⁴/λ_min⌋` into
    /// `L_ema` and the ring, lowers `L_slow` to at most `D`, keeps the prices and marks the state;
    /// it reads no liquidity of the pool, so a deposit or a withdrawal around the adopting
    /// instruction changes nothing.
    #[test]
    fn a_switch_rewrites_the_ring_and_keeps_the_age() {
        let mut m = Market::zeroed();
        let first = adoption(VenueKind::RaydiumV4, 4_000_000, 1_000_000, 1_990_000, 10);
        first.apply(&mut m, 10).unwrap();
        let mut obs = observe::initial(10, &first.state).unwrap();
        // Two more integrated seconds at a higher price.
        obs.record(11, WAD / 2, 2_000_000).unwrap();
        obs.roll(12, &shortwin_math::observe::EmaParams::DEFAULT)
            .unwrap();
        m.set_observation(&obs);
        m.pool_fail_since = 5_000;
        m.last_pool_fail_obs = 5_100;
        m.fp_t0 = 5_050;
        m.fp_d.set(400_000);
        m.fp_venue = VenueKind::RaydiumCpmm as u8;
        m._pad1a = [7, 7];
        let p_ema = m.ema_price.get();
        let (count, ts) = (m.ema_count, m.ema_ts);
        let ring_p: Vec<_> = m
            .ring
            .iter()
            .map(|e| (e.ts, e.p.get(), e.lo.get()))
            .collect();
        // A thinner CPMM pool (√(X·Y) = 500,000, D = 450,000) after the listing window: L_ema
        // the anchor ⌊D·10⁴/λ_min⌋ = 900,000, L_slow its locked depth.
        let thin = adoption(VenueKind::RaydiumCpmm, 1_000_000, 250_000, 450_000, 99_999);
        adopt(&mut m, &thin, 5_110, 3_600, 5_000).unwrap();
        assert_eq!(m.venue_kind(), Some(VenueKind::RaydiumCpmm));
        assert_eq!(m.graduated_at, 10, "age kept");
        assert_eq!(m.pool_seen_at, 5_110, "outside a window: opens wait again");
        assert_eq!(m.ema_price.get(), p_ema, "P_ema carries over");
        assert_eq!((m.ema_count, m.ema_ts), (count, ts), "no integration");
        assert_eq!((m.ema_liq.get(), m.liq_slow.get()), (900_000, 450_000));
        assert_eq!((m.ema_warm, m.ema_guard), (8, 1), "marked");
        let n = usize::from(m.ring_count);
        for e in m.ring.iter().take(n) {
            assert_eq!(
                (e.l.get(), e.llo.get(), e.lhi.get()),
                (900_000, 900_000, 900_000)
            );
            let (x, y) = reprice(900_000, e.p.get()).unwrap();
            assert_eq!((e.x.get(), e.y.get()), (x, y));
        }
        let after: Vec<_> = m
            .ring
            .iter()
            .map(|e| (e.ts, e.p.get(), e.lo.get()))
            .collect();
        assert_eq!(after, ring_p, "prices and bucket price extremes kept");
        assert_eq!((m.b_sec, m.b_lo.get()), (0, 0), "bucket cleared");
        assert_eq!((m.pool_fail_since, m.last_pool_fail_obs), (0, 0));
        assert_eq!(
            (m.fp_t0, m.fp_d.get(), m.fp_venue, m._pad1a),
            (0, 0, 0, [0, 0])
        );
        assert_eq!(m.fp_keys, [Pubkey::default(); 6]);
        assert_eq!(m.pool_d.get(), 450_000);
        // Inside the new listing window a further adoption does not extend it; a deeper pool
        // gets its own anchor (deeper or thinner than before alike), L_slow is never raised.
        let deep = adoption(VenueKind::RaydiumV4, 16_000_000, 4_000_000, 7_900_000, 1);
        let mut m0 = m;
        m0.ema_warm = 3;
        m0.ema_guard = 0;
        adopt(&mut m0, &deep, 5_200, 3_600, 5_000).unwrap();
        assert_eq!(
            m0.pool_seen_at, 5_110,
            "a chain of proposals cannot extend a window"
        );
        assert_eq!(m0.graduated_at, 10);
        assert_eq!(
            (m0.ema_liq.get(), m0.liq_slow.get()),
            (15_800_000, 450_000),
            "L_ema the new anchor, L_slow never raised"
        );
        assert_eq!((m0.ema_warm, m0.ema_guard), (8, 1), "re-marked");
        let n = usize::from(m0.ring_count);
        assert!(
            m0.ring
                .iter()
                .take(n)
                .all(|e| (e.l.get(), e.llo.get(), e.lhi.get())
                    == (15_800_000, 15_800_000, 15_800_000))
        );
        // No dependence on the pool's √(X·Y) now: a deposit of 100× (√(X·Y) 50,000,000) and an
        // unlocked LP's withdrawal (√(X·Y) = D) around the adopting instruction write the same.
        let honest = adoption(VenueKind::RaydiumCpmm, 2_400_000, 150_000, 450_000, 1);
        let with_reserves = |x: u64, y: u64| Adoption {
            state: VenueState {
                x,
                y,
                r: y,
                ..honest.state
            },
            ..honest
        };
        let withdrawn = with_reserves(900_000, 225_000);
        let deposit = with_reserves(100_000_000, 25_000_000);
        let mut outs = Vec::new();
        for a in [&withdrawn, &deposit, &honest] {
            let mut m2 = m;
            adopt(&mut m2, a, 5_300, 3_600, 5_000).unwrap();
            outs.push(m2);
        }
        for m2 in &outs {
            assert_eq!(m2.ema_liq.get(), 900_000);
            assert_eq!(bytemuck::bytes_of(m2), bytemuck::bytes_of(&outs[0]));
        }
        assert_eq!(
            m0.pool_cache[3],
            Pubkey::default(),
            "CPMM config slot cleared"
        );
        assert_eq!(
            m0.pool_cache[4],
            Pubkey::default(),
            "CPMM observation slot cleared"
        );
        // A zero locked depth cannot be adopted.
        let mut m3 = m;
        let zero = adoption(VenueKind::RaydiumCpmm, 1_000_000, 250_000, 0, 1);
        assert!(adopt(&mut m3, &zero, 5_300, 3_600, 5_000).is_err());
    }

    /// M4R-2: a proposal judges the window floor at `min(P_ema, P_cand)`; the candidate's spot
    /// alone without a price.
    #[test]
    fn the_proposal_window_floor_reads_min_of_ema_and_candidate() {
        // x = 4,000,000 tokens, y = 1,000,000 lamports: P_cand = WAD/4.
        let a = adoption(VenueKind::RaydiumCpmm, 4_000_000, 1_000_000, 1_000, 1);
        assert_eq!(window_price(Some(WAD), &a), Some(WAD / 4));
        assert_eq!(window_price(Some(WAD / 8), &a), Some(WAD / 8));
        assert_eq!(window_price(None, &a), Some(WAD / 4));
        let mut params = Params::defaults();
        // D = 10¹⁰: locked SOL ⌊D·isqrt(P)/10⁹⌋ = 10 SOL at P = WAD, 5 SOL at WAD/4.
        let a = adoption(
            VenueKind::RaydiumCpmm,
            4_000_000,
            1_000_000,
            10_000_000_000,
            1,
        );
        params.pool_min_real_sol = 8_000_000_000;
        assert!(window_floor_ok(&a, &params, Some(WAD)));
        assert!(!window_floor_ok(&a, &params, window_price(Some(WAD), &a)));
        params.pool_min_real_sol = 5_000_000_000;
        assert!(window_floor_ok(&a, &params, window_price(Some(WAD), &a)));
    }

    #[test]
    fn window_keys_follow_the_adapters_read_sets() {
        let mut v4 = adoption(VenueKind::RaydiumV4, 1, 1, 1, 0);
        v4.sol_first = true;
        let k = v4.window_keys();
        assert_eq!(
            k,
            [
                v4.pool,
                v4.vault_sol,
                v4.vault_tok,
                v4.lp_mint,
                Pubkey::default(),
                Pubkey::default()
            ]
        );
        let cpmm = adoption(VenueKind::RaydiumCpmm, 1, 1, 1, 0);
        assert_eq!(
            cpmm.window_keys(),
            [
                cpmm.pool,
                cpmm.amm_config,
                cpmm.vault_tok,
                cpmm.vault_sol,
                cpmm.lp_mint,
                cpmm.observation
            ]
        );
        let mut m = Market::zeroed();
        record_window(&mut m, &cpmm, 700, true);
        assert_eq!(
            (m.fp_t0, m.fp_d.get(), m.fp_venue, m._pad1a),
            (700, 1, VenueKind::RaydiumCpmm as u8, [0, 0])
        );
        record_window(&mut m, &v4, 750, false);
        assert_eq!(m.fp_t0, 700, "an improvement keeps the window's start");
        assert_eq!(m.fp_keys, v4.window_keys());
        m._pad1a = [1, 2];
        clear_window(&mut m);
        assert_eq!((m.fp_t0, m.fp_venue, m._pad1a), (0, 0, [0, 0]));
    }
}
