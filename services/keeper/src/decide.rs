//! What to do with a market and each of its positions this second (ARCHITECTURE §5.1
//! decision tree, §7.1 duties 1–5; ECON §5.8).
//!
//! **Exactly the program's view.** A decision is taken on the state the next instruction
//! will see: [`project`] runs the program's own market prologue
//! (`shortwin::engine::prologue::run`: params sync, accrual, the observation roll, the
//! record, the route chains) on a copy of the market at the cluster's clock; the legs, the
//! references and the bounds are the math crate's and the program engine's
//! (`risk::{spot_leg, liquidatable, buyback_allowed}` — the spot leg at `max(L_live, L_ema')`,
//! `bounds::spot_liquidity`, FITS-2 (a) —, `persist::liq_ema_leg`,
//! `engine::bounds::{liq_reference, max_in_liq, expiry_reference, max_in_exp}`,
//! `settle::s3_due`), with the position's locked terms and the risk fee
//! `min(φ_live, venue_fee_hard_cap_bps_pos)` (V2-1), through the venue's own quoter
//! (`VenueState::quoter`: a constant-product pool, or the pump curve with `B̂_str` above `rT`,
//! ARCH §4.3) — the program's `engine::bounds` and `engine::arming` quote the same way. The
//! keeper therefore acts at the first second the program would accept; every send is still
//! simulated first (duty 3), and the program remains the only judge.
//!
//! **Plans** ([`Plan`]), per position:
//!
//! | Situation | Plan |
//! |---|---|
//! | venue not executable, S3 due on the market | `S3Probe` (simulate `sol_settle`; send if it settles) |
//! | venue executable, the post-change warm-up running (`ema_warm > 0` after the roll; M4R) | `Watch` (exits refuse `EmaWarmingUp`, `sol_settle` decides nothing: observe every second until the gate) |
//! | not liquidatable, not armed | `Idle` / `Watch` (near a threshold: per-second coverage) |
//! | armed and not liquidatable | `SolSettle` (it keeps the clock or disarms on box evidence; duty 4) |
//! | liquidatable on a live pump curve with `Q > rT` (M5; ARCH §5.1 "straddle") | `Straddle` (the ≥ 3-transaction bundle of §5.4, [`crate::graduation`]; until M6 adds its exit transaction the keeper falls back to `sol_settle`) |
//! | liquidatable, completed bucket stale | `Observe` (observe this second, act the next) |
//! | liquidatable, normal buyback allowed, expired and not CR-liquidatable | `Expire` / `ExpireWait` |
//! | liquidatable, normal buyback allowed | `Liquidate` / `LiquidateWait` (fixed δ; retry next second) |
//! | liquidatable, no normal buyback (S1/S2 legs may hold), or armed | `SolSettle` |
//! | a `*Wait` while the market's S3 is due on an executable venue (flag, fee cap) | `SolSettle` |

use anchor_lang::prelude::Pubkey;
use anyhow::anyhow;
use shortwin::engine::{bounds, prologue};
use shortwin::state::{GlobalConfig, Market, VenueKind};
use shortwin_math::bounds as bounds_math;
use shortwin_math::observe::{self, Observation};
use shortwin_math::risk::{self, Budget};
use shortwin_math::settle::{self, S3Chain};
use shortwin_math::{persist, rent};
use shortwin_services_common::ChainClock;

use crate::config::CoverageConfig;
use crate::view::{MarketView, PositionView};

/// The market after the program's prologue at the cluster clock (what the next instruction's
/// body reads).
#[derive(Clone)]
pub struct Projection {
    pub now: i64,
    pub slot: u64,
    pub market: Box<Market>,
    pub obs: Box<Observation>,
    /// `s3_no_route` now (ARCH §5.3).
    pub s3_no_route: bool,
    /// `pool_fail` now (ARCH §2.6 rule 3: the Raydium fast path's own predicate, venue-side
    /// evidence only); the projected market's `pool_fail_since` is the chain after this
    /// second's observation.
    pub pool_fail: bool,
    /// The cap on the exit reference liquidity's EMA terms (M4F-1:
    /// `engine::bounds::ref_depth_cap`, `⌊pool_d·10⁴/λ_min⌋` on Raydium, `None` elsewhere) on the
    /// projected market.
    pub depth_cap: Option<u128>,
}

/// Runs the program's market prologue on a copy of `mv` at `clock` ([module docs](self)).
pub fn project(
    config: &GlobalConfig,
    mv: &MarketView,
    clock: ChainClock,
) -> anyhow::Result<Projection> {
    let mut market = mv.market.clone();
    let mut vault = mv.vault.clone();
    let pro = prologue::run(
        config,
        &mv.key,
        &mut market,
        &mut vault,
        Some(&mv.state),
        clock.unix_timestamp,
        clock.slot,
    )
    .map_err(|e| anyhow!("prologue of {}: {e}", mv.key))?;
    Ok(Projection {
        now: pro.now,
        slot: clock.slot,
        obs: market.observation(),
        depth_cap: bounds::ref_depth_cap(config, &market, mv.kind),
        market,
        s3_no_route: pro.s3_no_route,
        pool_fail: pro.pool_fail,
    })
}

/// What the keeper does with one position this second (module table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Plan {
    Idle,
    Watch,
    Observe,
    Liquidate,
    LiquidateWait,
    Expire,
    ExpireWait,
    SolSettle,
    S3Probe,
    /// A liquidatable position on a live pump curve whose `Q` exceeds the curve's `rT` (ARCH
    /// §5.1, §5.4): pump reverts any buy above `rT`, so no exit can run on the curve; the
    /// keeper's straddle bundle ([`crate::graduation`]) completes the curve (`complete_curve`),
    /// migrates it (`migrate_v2`) and exits on the new pool (M6's transaction).
    Straddle,
}

impl Plan {
    /// Whether the plan sends (or probes) an exit instruction.
    pub const fn acts(self) -> bool {
        matches!(
            self,
            Self::Liquidate | Self::Expire | Self::SolSettle | Self::S3Probe | Self::Straddle
        )
    }

    /// Order of attempts within a market: liquidations, expiries, straddles, settlements,
    /// probes.
    const fn rank(self) -> u8 {
        match self {
            Self::Liquidate => 0,
            Self::Expire => 1,
            Self::Straddle => 2,
            Self::SolSettle => 3,
            Self::S3Probe => 4,
            Self::LiquidateWait | Self::ExpireWait | Self::Observe => 5,
            Self::Watch => 6,
            Self::Idle => 7,
        }
    }
}

/// One position's evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionEval {
    pub key: Pubkey,
    pub plan: Plan,
    /// Near a threshold (duty 1): live CR below `near_cr_bps`·L, deadline within the window,
    /// or armed.
    pub near: bool,
    pub expired: bool,
    pub armed: bool,
    /// CR on the live state at the risk fee (0 for an undefined quote).
    pub cr_live_bps: u64,
    pub spot_leg: bool,
    pub ema_leg: bool,
    /// A completed bucket fresh within the position's `ref_max_age_s`.
    pub fresh: bool,
    pub buyback_allowed: bool,
    /// `B̂_live` at the live fee (the execution quote).
    pub b_live: Option<u64>,
    /// The liquidation or expiry `max_in` of the plan, if computed.
    pub max_in: Option<u64>,
    /// `R_due`, `K_s`.
    pub r_due: u64,
    pub k_s: u64,
    /// The market's S3 timer is due for this position (flag, fee cap or no route).
    pub s3_due: bool,
    /// On a live pump curve, `Q` exceeds the curve's real token reserves `rT` (no buy on the
    /// curve can close it: the straddle of ARCH §5.4).
    pub straddle: bool,
}

/// The market-level coverage needs (duties 1, 2, 5; M4F-1's guard coverage).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MarketNeeds {
    /// At least one observation this second (duty 1: a near position, or
    /// `|P_spot/P_ema − 1|` above `ema_dev_bps`; duty 5: an S3 due on an executable venue).
    pub per_second: bool,
    /// The market has no route (`s3_no_route`): observe every `no_route_interval_s`.
    pub no_route: bool,
    /// The market has open positions: observe every `idle_interval_s`.
    pub open_positions: bool,
    /// An S3 is due on an executable venue: once per second, never per slot (duty 5).
    pub s3_due_executable: bool,
    /// `|P_spot/P_ema − 1|` above the threshold.
    pub ema_deviates: bool,
    /// The route is gone and no S3 chain runs on chain yet: observe now, so the chain (and
    /// the S3 timer of every liquidatable or expired position) starts at the outage, not up to
    /// `no_route_interval_s` later.
    pub start_chain: bool,
    /// A Raydium market without open positions inside its listing window (M4V-5): observe
    /// every `listing_interval_s`, so the EMA the first open after the window reads follows the
    /// pool and not the spot a listing or an adoption started it at.
    pub listing: bool,
    /// Once per second (M4V-5, M4R): a Raydium market without open positions in the last
    /// [`LISTING_WARMUP_S`] seconds of its listing window, or after it while its post-change
    /// warm-up runs (`ema_warm > 0`; opens refuse `EmaWarmingUp` until then).
    pub warm_up: bool,
    /// The post-change warm-up runs on an executable venue (`ema_warm > 0` after the roll; M4R):
    /// with open positions the market is observed every second (duty 5) until `ema_warm == 0`,
    /// and no liquidation or expiry is sent before.
    pub warming: bool,
    /// The post-change reference guard is set on an executable venue (`ema_guard == 1` after
    /// the roll; M4F-1): with open positions the market stays on the per-second cadence after
    /// the warm-up too, until the guard clears. The guard holds the exit reference at the
    /// ring's liquidity lows and clears once `L_ema` agrees with them; an honest bucket low in
    /// every second keeps a deposit-wrapped crank in the idle seconds from lifting the lows
    /// (`L_mlo`) and clearing it. Exits are sent as usual (the guard gates nothing).
    pub guarded: bool,
    /// Liquidity-triggered coverage runs (FITS-2 (b); `coverage::MarketTrack::liq_covered`): a
    /// foreign bucket liquidity off the keeper's own `√(X·Y)` in the last `liq_cover_s` seconds
    /// on this market with open positions — an LP-wrapped crank (the withdraw drag, the deposit
    /// freeze) — keeps it on the per-second cadence (never per slot), so every second carries an
    /// honest bucket extreme. Only on an executable venue with open positions.
    pub liq_covered: bool,
}

/// Seconds at the end of a listing window observed every second (M4V-5):
/// `EMA_WARMUP_BUCKETS + 2`, so at the window's end the post-change warm-up of a listing or an
/// adoption is over (8 integrations of the last seconds' honest buckets).
pub const LISTING_WARMUP_S: i64 = shortwin_math::consts::EMA_WARMUP_BUCKETS as i64 + 2;

fn s3_chain(m: &Market, kind: VenueKind, no_route_now: bool) -> S3Chain {
    let opt = |t: i64| (t != 0).then_some(t);
    S3Chain {
        no_route_now,
        no_route_since: opt(m.no_route_since),
        graduated_at: opt(m.graduated_at),
        migrating: kind == VenueKind::PumpMigrating,
        migration_seen_at: opt(m.migration_seen_at),
    }
}

/// Evaluates one position on the projection ([module docs](self)).
pub fn evaluate(
    mv: &MarketView,
    proj: &Projection,
    pv: &PositionView,
    cov: &CoverageConfig,
) -> anyhow::Result<PositionEval> {
    let pos = &pv.pos;
    let st = &mv.state;
    let m = &proj.market;
    let obs = &proj.obs;
    let now = proj.now;
    let q = pos.q;
    let cap = u64::from(pos.venue_fee_hard_cap_bps);
    let l_bps = pos.cr_liq_bps;
    let math = |e: shortwin_math::MathError| anyhow!("math: {e:?}");

    let r_due = rent::due(&pos.pos_rent(), &m.basis()).map_err(math)?;
    let v_eff = risk::v_eff(pos.v, r_due);
    let k_s = risk::k_s(pos.v, &pos.settle_keeper_terms()).map_err(math)?;
    let budget = Budget {
        v: pos.v,
        k_s,
        ert: pos.ert,
    };
    let expired = now >= pos.deadline;
    let armed = pos.s_armed != 0;
    // The risk view of the venue's own quoter (the program's `engine::bounds` / `arming`):
    // a pool, or the curve with `B̂_str` above `rT` (ARCH §4.3).
    let risk_pricing = st.quoter().map(|p| p.capped(cap));

    // Live CR at the risk fee (duty 1's "live CR").
    let b_live_risk = st.buy_risk(q, cap).map(|b| b.cost);
    let cr_live_bps = risk::cr_bps(v_eff, b_live_risk).unwrap_or(0);
    let near_cr = u128::from(cr_live_bps).saturating_mul(10_000)
        < u128::from(l_bps).saturating_mul(u128::from(cov.near_cr_bps));
    let near_deadline = pos.deadline.saturating_sub(now) <= cov.deadline_window_s;

    // The legs (ARCH §4.5), on the projection. The spot leg at `max(L_live, L_ema')` (FITS-2 (a),
    // the program's `assess`: a withdrawal inside a transaction cannot make it true).
    let fresh = obs.last.is_fresh(now, pos.ref_max_age_s);
    let p_live = st.price();
    let l_live = st.liquidity().unwrap_or(obs.ema.liq);
    let l_spot = bounds_math::spot_liquidity(l_live, &obs.ema, &obs.ring).map_err(math)?;
    let p_rs = p_live.and_then(|p| observe::robust_spot(p, &obs.last, now, pos.ref_max_age_s));
    let b_rs = match (risk_pricing, p_rs) {
        (Some(pr), Some(p)) => pr.buy_repriced(q, l_spot, p).map(|b| b.cost),
        _ => None,
    };
    let spot_leg =
        risk::spot_leg(st.executable, p_rs.is_some(), v_eff, b_rs, l_bps).map_err(math)?;
    let ema_leg = persist::liq_ema_leg(
        &obs.ring,
        risk::ring_below(v_eff, l_bps, |x, y| {
            if x == 0 {
                return None;
            }
            risk_pricing
                .and_then(|pr| pr.buy_xy(q, x, y))
                .map(|b| b.cost)
        }),
        pos.liq_persist_n,
    )
    .map_err(math)?;
    let cr_liquidatable = spot_leg && ema_leg;
    let liquidatable = risk::liquidatable(spot_leg, ema_leg, now, pos.deadline);

    // Normal buyback allowed (execution quote at the live fee).
    let b_live_q = st.buy(q);
    let b_live = b_live_q.map(|b| b.cost);
    let sigma_live = match p_live {
        Some(p) => risk::sigma(b_live_q.map(|b| b.net), q, p).map_err(math)?,
        None => u64::MAX,
    };
    let buyback_allowed = st.executable
        && risk::buyback_allowed(&budget, b_live, sigma_live, u32::from(pos.max_liq_slip_bps))
            .map_err(math)?;

    // The S3 timer (market-level chain, the position's locked timers).
    let chain = s3_chain(m, mv.kind, proj.s3_no_route);
    let s3_timer = settle::s3_due(&chain, &pos.s3_terms(), now, true);
    let s3_due = s3_timer && (liquidatable || !st.executable);

    // ARCH §5.1 "curve live && q > rT": the exits refuse `CurveStraddle` (pump reverts a buy
    // above `rT`, 6021), the bundle of §5.4 is the way out.
    let straddle = mv.kind == VenueKind::PumpCurve
        && st
            .curve
            .as_ref()
            .is_some_and(|c| !c.live.complete && u128::from(q) > c.live.rt);

    let mut max_in = None;
    // M4R: the post-change warm-up gates keeper exits on an executable venue.
    let warming = st.executable && m.ema_warm > 0;
    let plan = if !st.executable {
        if s3_timer {
            Plan::S3Probe
        } else if near_deadline || armed {
            Plan::Watch
        } else {
            Plan::Idle
        }
    } else if warming {
        Plan::Watch
    } else if !liquidatable && !armed {
        if near_cr || near_deadline {
            Plan::Watch
        } else {
            Plan::Idle
        }
    } else if !liquidatable {
        // Armed, not liquidatable: the call keeps the clock or disarms (box evidence).
        Plan::SolSettle
    } else if straddle {
        Plan::Straddle
    } else if !fresh {
        if armed {
            Plan::SolSettle
        } else {
            Plan::Observe
        }
    } else if buyback_allowed {
        let b = b_live.unwrap_or(u64::MAX);
        let (bound, ok, wait) = if expired && !cr_liquidatable {
            let r = bounds::expiry_reference(pos, obs, st, now, proj.depth_cap)
                .map_err(|e| anyhow!("{e}"))?;
            let mi = match r.and_then(|r| r.b_ref) {
                Some(b_ref) => {
                    bounds::max_in_exp(pos, b_ref, k_s, r_due).map_err(|e| anyhow!("{e}"))?
                }
                None => None,
            };
            (mi, Plan::Expire, Plan::ExpireWait)
        } else {
            let r = bounds::liq_reference(pos, obs, st, now, proj.depth_cap)
                .map_err(|e| anyhow!("{e}"))?;
            let mi = r
                .and_then(|r| r.b_ref)
                .and_then(|b_ref| bounds::max_in_liq(pos, b_ref, k_s));
            (mi, Plan::Liquidate, Plan::LiquidateWait)
        };
        max_in = bound;
        if bound.is_some_and(|mi| b <= mi) {
            ok
        } else if s3_due {
            Plan::SolSettle
        } else {
            wait
        }
    } else {
        Plan::SolSettle
    };

    Ok(PositionEval {
        key: pv.key,
        plan,
        near: near_cr || near_deadline || armed || warming,
        expired,
        armed,
        cr_live_bps,
        spot_leg,
        ema_leg,
        fresh,
        buyback_allowed,
        b_live,
        max_in,
        r_due,
        k_s,
        s3_due,
        straddle,
    })
}

/// Evaluates every open position of `mv`, most urgent first (liquidations, expiries,
/// settlements, probes; lower live CR first).
pub fn evaluate_all(
    mv: &MarketView,
    proj: &Projection,
    cov: &CoverageConfig,
) -> Vec<Result<PositionEval, (Pubkey, String)>> {
    let mut out: Vec<Result<PositionEval, (Pubkey, String)>> = mv
        .positions
        .iter()
        .map(|pv| evaluate(mv, proj, pv, cov).map_err(|e| (pv.key, e.to_string())))
        .collect();
    out.sort_by_key(|r| match r {
        Ok(e) => (e.plan.rank(), e.cr_live_bps),
        Err(_) => (u8::MAX, u64::MAX),
    });
    out
}

/// The market's coverage needs (duties 1 and 5; the listing window of a Raydium market without
/// positions, M4V-5; the post-change warm-up and reference guard of a market with positions,
/// M4R / M4F-1; duty 2's liquidity-triggered coverage, FITS-2 (b)) from its evaluations;
/// `list_window_s` is `pool_list_window_s`, `liq_covered` whether the market's liquidity-triggered
/// coverage window runs now (`coverage::MarketTrack::liq_covered`).
pub fn market_needs(
    mv: &MarketView,
    proj: &Projection,
    evals: &[PositionEval],
    cov: &CoverageConfig,
    list_window_s: u32,
    liq_covered: bool,
) -> MarketNeeds {
    let open = !mv.positions.is_empty() || proj.market.open_positions > 0;
    let ema_deviates = match (mv.state.price(), proj.obs.ema.price) {
        (Some(spot), ema) if ema > 0 => {
            let diff = spot.abs_diff(ema);
            diff.saturating_mul(10_000) > ema.saturating_mul(u128::from(cov.ema_dev_bps))
        }
        _ => false,
    };
    let s3_due_executable = mv.state.executable && evals.iter().any(|e| e.s3_due);
    let near = evals.iter().any(|e| e.near || e.plan != Plan::Idle);
    // Only a venue that can execute records observations (A64): a halted one is kept on the
    // no-route cadence (duty 5), never per second.
    let executable = mv.state.executable;
    // M4R: the post-change warm-up (a listing, an adoption) on an executable venue; M4F-1: the
    // reference guard it sets, until it clears.
    let warming = executable && proj.market.ema_warm > 0;
    let guarded = executable && proj.market.ema_guard != 0;
    // FITS-2 (b): the liquidity-triggered window, on an executable venue with open positions.
    let liq_covered = executable && open && liq_covered;
    // M4V-5: a Raydium market without positions, in its listing window or not yet warmed up.
    let (listing, warm_up) = if mv.kind.is_raydium() && !open && executable {
        let m = &proj.market;
        let end = m.pool_seen_at.saturating_add(i64::from(list_window_s));
        let warm = m.ema_warm == 0;
        let inside = proj.now < end;
        (
            inside,
            proj.now >= end.saturating_sub(LISTING_WARMUP_S) && (inside || !warm),
        )
    } else {
        (false, false)
    };
    MarketNeeds {
        per_second: executable
            && (open && (near || ema_deviates || warming || guarded || liq_covered)
                || s3_due_executable),
        no_route: proj.s3_no_route && open,
        start_chain: proj.s3_no_route && open && mv.market.no_route_since == 0,
        open_positions: open,
        s3_due_executable,
        ema_deviates,
        listing,
        warm_up,
        warming,
        guarded,
        liq_covered,
    }
}
