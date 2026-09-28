//! What to do with a market and each of its positions this second (ARCHITECTURE §5.1
//! decision tree, §7.1 duties 1–5; ECON §5.8).
//!
//! **Exactly the program's view.** A decision is taken on the state the next instruction
//! will see: [`project`] runs the program's own market prologue
//! (`shortwin::engine::prologue::run`: params sync, accrual, the observation roll, the
//! record, the route chains) on a copy of the market at the cluster's clock; the legs, the
//! references and the bounds are the math crate's and the program engine's
//! (`risk::{spot_leg, liquidatable, buyback_allowed}`, `persist::liq_ema_leg`,
//! `engine::bounds::{liq_reference, max_in_liq, expiry_reference, max_in_exp}`,
//! `settle::s3_due`), with the position's locked terms and the risk fee
//! `min(φ_live, venue_fee_hard_cap_bps_pos)` (V2-1). The keeper therefore acts at the first
//! second the program would accept; every send is still simulated first (duty 3), and the
//! program remains the only judge.
//!
//! **Plans** ([`Plan`]), per position:
//!
//! | Situation | Plan |
//! |---|---|
//! | venue not executable, S3 due on the market | `S3Probe` (simulate `sol_settle`; send if it settles) |
//! | not liquidatable, not armed | `Idle` / `Watch` (near a threshold: per-second coverage) |
//! | armed and not liquidatable | `SolSettle` (it keeps the clock or disarms on box evidence; duty 4) |
//! | liquidatable, completed bucket stale | `Observe` (observe this second, act the next) |
//! | liquidatable, normal buyback allowed, expired and not CR-liquidatable | `Expire` / `ExpireWait` |
//! | liquidatable, normal buyback allowed | `Liquidate` / `LiquidateWait` (fixed δ; retry next second) |
//! | liquidatable, no normal buyback (S1/S2 legs may hold), or armed | `SolSettle` |
//! | a `*Wait` while the market's S3 is due on an executable venue (flag, fee cap) | `SolSettle` |

use anchor_lang::prelude::Pubkey;
use anyhow::anyhow;
use shortwin::engine::{bounds, prologue};
use shortwin::state::{GlobalConfig, Market, VenueKind};
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
        Some(mv.state),
        clock.unix_timestamp,
        clock.slot,
    )
    .map_err(|e| anyhow!("prologue of {}: {e}", mv.key))?;
    Ok(Projection {
        now: pro.now,
        slot: clock.slot,
        obs: market.observation(),
        market,
        s3_no_route: pro.s3_no_route,
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
}

impl Plan {
    /// Whether the plan sends (or probes) an exit instruction.
    pub const fn acts(self) -> bool {
        matches!(
            self,
            Self::Liquidate | Self::Expire | Self::SolSettle | Self::S3Probe
        )
    }

    /// Order of attempts within a market: liquidations, expiries, settlements, probes.
    const fn rank(self) -> u8 {
        match self {
            Self::Liquidate => 0,
            Self::Expire => 1,
            Self::SolSettle => 2,
            Self::S3Probe => 3,
            Self::LiquidateWait | Self::ExpireWait | Self::Observe => 4,
            Self::Watch => 5,
            Self::Idle => 6,
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
}

/// The market-level coverage needs (duties 1, 2, 5).
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
}

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
    let risk_pricing = st.pricing.map(|p| p.capped(cap));

    // Live CR at the risk fee (duty 1's "live CR").
    let b_live_risk = st.buy_risk(q, cap).map(|b| b.cost);
    let cr_live_bps = risk::cr_bps(v_eff, b_live_risk).unwrap_or(0);
    let near_cr = u128::from(cr_live_bps).saturating_mul(10_000)
        < u128::from(l_bps).saturating_mul(u128::from(cov.near_cr_bps));
    let near_deadline = pos.deadline.saturating_sub(now) <= cov.deadline_window_s;

    // The legs (ARCH §4.5), on the projection.
    let fresh = obs.last.is_fresh(now, pos.ref_max_age_s);
    let p_live = st.price();
    let l_live = st.liquidity();
    let p_rs = p_live.and_then(|p| observe::robust_spot(p, &obs.last, now, pos.ref_max_age_s));
    let b_rs = match (risk_pricing, p_rs, l_live) {
        (Some(pr), Some(p), Some(l)) => pr.buy_repriced(q, l, p).map(|b| b.cost),
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
            risk_pricing.and_then(|pr| pr.buy(q, x, y)).map(|b| b.cost)
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

    let mut max_in = None;
    let plan = if !st.executable {
        if s3_timer {
            Plan::S3Probe
        } else if near_deadline || armed {
            Plan::Watch
        } else {
            Plan::Idle
        }
    } else if !liquidatable && !armed {
        if near_cr || near_deadline {
            Plan::Watch
        } else {
            Plan::Idle
        }
    } else if !liquidatable {
        // Armed, not liquidatable: the call keeps the clock or disarms (box evidence).
        Plan::SolSettle
    } else if !fresh {
        if armed {
            Plan::SolSettle
        } else {
            Plan::Observe
        }
    } else if buyback_allowed {
        let b = b_live.unwrap_or(u64::MAX);
        let (bound, ok, wait) = if expired && !cr_liquidatable {
            let r = bounds::expiry_reference(pos, obs, st, now).map_err(|e| anyhow!("{e}"))?;
            let mi = match r.and_then(|r| r.b_ref) {
                Some(b_ref) => {
                    bounds::max_in_exp(pos, b_ref, k_s, r_due).map_err(|e| anyhow!("{e}"))?
                }
                None => None,
            };
            (mi, Plan::Expire, Plan::ExpireWait)
        } else {
            let r = bounds::liq_reference(pos, obs, st, now).map_err(|e| anyhow!("{e}"))?;
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
        near: near_cr || near_deadline || armed,
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

/// The market's coverage needs (duties 1 and 5) from its evaluations.
pub fn market_needs(
    mv: &MarketView,
    proj: &Projection,
    evals: &[PositionEval],
    cov: &CoverageConfig,
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
    MarketNeeds {
        per_second: executable && (open && (near || ema_deviates) || s3_due_executable),
        no_route: proj.s3_no_route && open,
        start_chain: proj.s3_no_route && open && mv.market.no_route_since == 0,
        open_positions: open,
        s3_due_executable,
        ema_deviates,
    }
}
