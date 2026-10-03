//! The keeper loop (ARCHITECTURE §7.1): one [`Keeper::tick`] per slot.
//!
//! Each tick:
//!
//! 1. reads the clock, re-discovers markets every `discovery_interval_s`, reads the open
//!    positions, and loads every covered market that has positions or anything time-critical
//!    pending, and the others ("cold") every `cold_interval_s`, staggered (M7-SEC-8;
//!    [`crate::view::load_with_positions`]);
//! 2. per market: records its own spot and depth view and checks the buckets for a foreign
//!    deviating observation (duty 2: a price starts a per-slot burst; on a market with open
//!    positions a liquidity starts `liq_cover_s` of per-second coverage, FITS-2 (b)); projects the prologue at the clock and evaluates every open
//!    position ([`crate::decide`]); sends the exits the projection allows — `liquidate` at the
//!    first second its bound is met (duty 3), `expire` when the expiry bound is met, and
//!    `sol_settle` at most once per second for armed positions and positions without a normal
//!    buyback (duty 4) or whose S3 is due — each simulated first, trading exits with the first
//!    extras set that simulates (duty 10); a landed exit **is** the market's observation for the
//!    second (duty 7);
//! 3. queues a `crank_price` for every market whose observation is due and was not made by an
//!    exit ([`crate::coverage`]), and sends the queue in batches: several markets per v1
//!    transaction (6), v0 batches (4) where v1 is not accepted; each batch is simulated, a
//!    market whose crank fails simulation is dropped and cranked alone (duty 7);
//! 4. closes the previous second's coverage accounting (misses → degraded mode; `CoverageGap`
//!    events from the program → degraded mode; duty 9) and the metrics (duty 8).
//!
//! After the exits and before the cranks, Raydium pool selection (duty 5's fast path and duty
//! 12, the pool monitor; [`crate::pool_monitor`]) on every covered Raydium and list-only
//! market: discover the mint's Raydium SOL pools, rank the eligible ones with the program's own
//! verifier, and send `propose_pool` / `execute_pool_switch` exactly when the program's own
//! `pool_select::decide` accepts — a list-only market's first pool, a listing-window
//! replacement (5 %), a zero-position adoption (25 %; in the tick the last position closes),
//! the fast path's window (opened at the first armed second on the deepest candidate, improved
//! by a deeper one, executed at the first second after it); pool events are reported, a market
//! pinned by open positions to a much thinner pool is alerted (R4B-10). A market adopted in
//! the tick is re-read, so its crank that tick names the new pool.
//!
//! Before the markets, the venue drift monitor (duty 10, [`crate::monitor`]): the venue
//! ProgramData headers against the `VenueGuard` latch — an upgrade is flagged with
//! `flag_venue_upgraded` in the slot the keeper sees it, at the exit priority ceiling — and
//! every covered market's pool and CPMM config against the last tick (Raydium admin actions:
//! route lost or restored, status, open time, fee against the opens and hard caps), as alerts.
//!
//! **List-only markets** (M5): covered like any market with their detection set as ReadSet — one
//! with positions open (a curve that graduated elsewhere) is observed on the no-route cadence and
//! its positions are settled in S3 when due (duty 5, `sol_settle` on the detection set,
//! M2-SEC-5); one whose detection set shows a pump curve again is cranked onto it, and the
//! trader's curve accounts are then created ([`crate::graduation::redetection_due`]).
//!
//! **Graduation** (duty 6, [`crate::graduation`]; M5, M6): before a curve market's exits, pump
//! `migrate_v2` top-level as soon as its curve is complete and unmigrated, then — as soon as its
//! canonical pool is valid, whoever migrated — the `→ PumpSwap` transition (the pool accounts its
//! trades require, `crank_price` onto the pool, the pool's first observation) in one transaction,
//! which is the market's observation for the second; a PumpSwap market whose pool accounts are
//! missing gets them created; a liquidatable position whose `Q` exceeds its live curve's `rT`
//! (`Plan::Straddle`) gets the straddle bundle (`complete_curve`, `migrate_v2`, the transition)
//! through the chain's block engine, and its exit on the pool follows at the post-change
//! warm-up's gate (the keeper's ordinary PumpSwap duties). While the bundle does not go out, the
//! keeper calls `sol_settle` for the position.
//!
//! **The trader side** (duties 6 and 13, [`crate::sweep`]): right after a full wipe, the venue
//! cashback claims the trader needs and `sweep_trader` (a deposit, and hence an open, waits for
//! both post-wipe bits); every `sweep.check_interval_s`, `sweep_trader` when the trader side holds
//! anything, and the weekly cashback claim on cashback coins.
//!
//! Batches are **packed by size**: a batch takes up to `batch_v1` / `batch_v0` cranks and is cut
//! where its transaction would exceed the format's size or account limit, so any mix of venue
//! account sets (the mock's 3 ReadSet keys, AMM v4's 4, CPMM's 5) is batched as densely as fits.
//!
//! Every transaction: simulated first, compute-unit limit `⌈simulated × 1.1⌉`, priority price
//! from recent per-account fees, escalated after misses, capped at the O17 ceiling (duty 14).
//! Duty 3's Jito bundle path for ordinary exits is not built yet (see the crate README); the
//! straddle's bundle is duty 6's.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::Arc;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use anyhow::anyhow;
use shortwin::engine::pool_select::{self, FitContext, Proposal, SelectParams, Situation};
use shortwin::engine::{guard, prologue};
use shortwin::errors::ShortwinError;
use shortwin::events::{CoverageGap, SolSettle};
use shortwin::state::{EXTRAS_IX_BUY, MARKET_FLAG_PAUSED_COVERAGE, Market, VenueGuard, VenueKind};
use shortwin::venues::ExtrasSel;
use shortwin_math::tier::OraclePrice;
use shortwin_services_common::tx::{self, ComputeBudget, TxFormat};
use shortwin_services_common::{AccountData, Chain, ChainClock, TxError, TxOutcome};
use solana_keypair::Keypair;
use solana_message::AddressLookupTableAccount;
use solana_signer::Signer;

use crate::alerts::{Alert, PauseUnavailable};
use crate::budget::{self, Admission, BudgetParams, ChargeClass, ChargeReq, GlobalBudget, Tier};
use crate::config::{KeeperConfig, TxFormatPolicy};
use crate::coverage::{Due, FlagSet, MarketTrack};
use crate::decide::{self, MarketNeeds, Plan, PositionEval};
use crate::events::{self, PoolEvent, PoolEventKind};
use crate::fees;
use crate::graduation::{self, MigrateRecord, StraddlePlan, StraddleRecord, StraddleResult};
use crate::ixs;
use crate::metrics::Metrics;
use crate::monitor::{self, FeeCaps, LatchAction, PoolWatch};
use crate::pda;
use crate::pool_monitor::{self, Action, PoolPlan, PoolTrack, Seen};
use crate::sweep::{self, SweepFacts, SweepKind, SweepRecord};
use crate::venue::{self, PammAccount};
use crate::view::{self, ChainView, MarketView, PositionView};

/// M7-SEC-8: a cold market's poll phase in `[0, every)` (from its key), so the cold markets'
/// reads spread over the interval.
fn cold_phase(key: &Pubkey, every: i64) -> i64 {
    let b = key.to_bytes();
    let h = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    i64::try_from(h % u64::try_from(every.max(1)).unwrap_or(1)).unwrap_or(0)
}

/// Whether the cluster accepts v1 transactions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V1Support {
    Unknown,
    Yes,
    /// Refused; re-probed from `until`.
    No {
        until: i64,
    },
}

/// The exit instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExitKind {
    Liquidate,
    Expire,
    SolSettle,
}

/// What became of an exit attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExitResult {
    /// Landed (the transaction's outcome, its format, the extras set used, the compute-unit
    /// limit it requested — `⌈simulated × 1.1⌉`, what a leader charges against the per-account
    /// block budget, C0-17 — and its serialized size).
    Landed {
        outcome: TxOutcome,
        format: TxFormat,
        extras: ExtrasSel,
        unit_limit: u32,
        bytes: usize,
    },
    /// The program refused it in simulation (`code`: its custom error, if any).
    Refused {
        code: Option<u32>,
        outcome: TxOutcome,
    },
    /// Sent, and failed on chain.
    Failed { outcome: TxOutcome },
    /// Dry run: simulated only.
    DryRun { outcome: TxOutcome },
    /// An S3 probe whose simulation settles nothing: not sent.
    NoOp,
    /// T-CASCADE: simulated, but its compute-unit limit would take the market's requests this
    /// slot above `budget.market_cu_per_slot`; not sent (retried next slot).
    Deferred { unit_limit: u32 },
    /// A transport or building error.
    Error(String),
}

impl ExitResult {
    /// The landed outcome, if any.
    pub fn landed(&self) -> Option<&TxOutcome> {
        match self {
            Self::Landed { outcome, .. } => Some(outcome),
            _ => None,
        }
    }
}

/// One exit attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExitRecord {
    pub market: Pubkey,
    pub position: Pubkey,
    pub kind: ExitKind,
    /// The plan that led to it.
    pub plan: Plan,
    pub result: ExitResult,
}

/// One crank batch sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchRecord {
    pub markets: Vec<Pubkey>,
    pub format: TxFormat,
    /// Serialized size and account keys of the sent transaction.
    pub bytes: usize,
    pub accounts: usize,
    pub units: u64,
    pub fee: u64,
    /// Priority price, micro-lamports per CU.
    pub price: u64,
    /// Compute-unit limit requested (`⌈simulated × 1.1⌉`).
    pub unit_limit: u32,
}

/// What became of a pool-selection attempt (`propose_pool`, `execute_pool_switch`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PoolResult {
    /// Landed, with its pool events.
    Landed {
        outcome: TxOutcome,
        events: Vec<PoolEvent>,
    },
    /// The program refused it in simulation (`code`: its custom error, if any).
    Refused {
        code: Option<u32>,
        outcome: TxOutcome,
    },
    /// Sent, and failed on chain.
    Failed { outcome: TxOutcome },
    /// Dry run: simulated only.
    DryRun { outcome: TxOutcome },
    /// A transport or building error.
    Error(String),
}

impl PoolResult {
    /// Whether it landed.
    pub fn landed(&self) -> bool {
        matches!(self, Self::Landed { .. })
    }
}

/// One pool-selection attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolActionRecord {
    pub market: Pubkey,
    pub action: Action,
    pub result: PoolResult,
}

/// Pool-selection refusals that are the program's normal "not now" answers after a race (a
/// proposal or adoption landed between the keeper's read and its send; the latch) — no alert.
fn benign_pool(code: Option<u32>) -> bool {
    [
        ShortwinError::PoolNotDominant,
        ShortwinError::PositionsOpen,
        ShortwinError::PoolWindowClosed,
        ShortwinError::PoolSwitchNotDue,
        ShortwinError::NoPendingSwitch,
        ShortwinError::VenueUpgraded,
    ]
    .into_iter()
    .any(|e| code == Some(u32::from(e)))
}

/// A failing Raydium market without positions whose candidate the fast path could take (the
/// plan's `keep_chain`): duty 5's no-route cadence does not observe a market without positions,
/// so the pool monitor queues its crank — at once while the chain has not started, then every
/// `every` seconds (< `S3_OBS_GAP_S`, so the chain stays continuous until the fast path arms).
fn keep_pool_fail_chain(
    key: Pubkey,
    mv: Option<&MarketView>,
    now: i64,
    every: i64,
    report: &TickReport,
    crank_queue: &mut Vec<(Pubkey, Due)>,
) {
    let Some(mv) = mv else { return };
    let m = &mv.market;
    if crank_queue.iter().any(|(k, _)| *k == key) || report.observed(&key) {
        return;
    }
    if m.pool_fail_since == 0 || now.saturating_sub(m.last_pool_fail_obs) >= every {
        crank_queue.push((key, Due::NoRoute));
    }
}

/// What one tick did (for tests, logs and metrics).
#[derive(Clone, Debug, Default)]
pub struct TickReport {
    pub clock: Option<ChainClock>,
    pub exits: Vec<ExitRecord>,
    /// Markets whose `crank_price` landed.
    pub cranked: Vec<Pubkey>,
    /// Markets whose crank failed (alone), with why.
    pub crank_failed: Vec<(Pubkey, String)>,
    pub batches: Vec<BatchRecord>,
    /// Markets observed this tick (by a crank or an exit) and why an observation was due
    /// (`None`: an exit observed a market that needed none).
    pub observed: Vec<(Pubkey, Option<Due>)>,
    /// Markets whose observation was due, with why.
    pub due: Vec<(Pubkey, Due)>,
    pub alerts: Vec<Alert>,
    pub evaluations: BTreeMap<Pubkey, Vec<PositionEval>>,
    pub needs: BTreeMap<Pubkey, MarketNeeds>,
    /// Foreign deviating price observations detected (per-slot bursts, duty 2): (market,
    /// second).
    pub bursts: Vec<(Pubkey, i64)>,
    /// Foreign deviating bucket liquidity detected on markets with open positions (per-second
    /// coverage started or extended, FITS-2 (b)): (market, second).
    pub liq_covers: Vec<(Pubkey, i64)>,
    /// `flag_venue_upgraded` calls that landed this tick: (venue, deploy slot) (duty 10).
    pub flagged: Vec<(u8, u64)>,
    /// Pool selection (duty 5's fast path, duty 12): the plan of every market whose candidates
    /// were evaluated this tick.
    pub pool_plans: BTreeMap<Pubkey, PoolPlan>,
    /// `propose_pool` / `execute_pool_switch` attempts.
    pub pool_actions: Vec<PoolActionRecord>,
    /// The pool events of the keeper's landed pool-selection transactions.
    pub pool_events: Vec<PoolEvent>,
    /// Duty 6: the keeper's `migrate_v2` attempts.
    pub migrations: Vec<MigrateRecord>,
    /// Duty 6 (M5): the trader accounts the keeper created on a curve market (after a `None →`
    /// pump re-detection), with the result.
    pub curve_accounts: Vec<(Pubkey, ExitResult)>,
    /// Duty 6 (M6): the pool accounts the keeper created on a PumpSwap market (ARCH §3.12's
    /// existence requirement), with the result.
    pub pamm_accounts: Vec<(Pubkey, ExitResult)>,
    /// Duty 6 (M6): the keeper's `→ PumpSwap` transitions (its own transaction, or a straddle
    /// bundle's tx3).
    pub transitions: Vec<TransitionRecord>,
    /// Duty 6: the straddle bundles (sent, refused in pre-flight, or waiting).
    pub straddles: Vec<StraddleRecord>,
    /// Duties 6 and 13: the keeper's sweeps (post-wipe, trader side, cashback claims).
    pub sweeps: Vec<SweepRecord>,
    /// O17: the keeper's `set_coverage_pause` attempts (the designated pauser only).
    pub coverage_pauses: Vec<CoveragePauseRecord>,
    /// Duty 11: the keeper's `crank_sol_usd` attempt this tick (the update's publish time).
    pub oracle: Option<(i64, ExitResult)>,
    /// T-CASCADE: per market, the compute units the keeper's landed exits requested this tick's
    /// slot (their `⌈simulated × 1.1⌉` limits, against `budget.market_cu_per_slot`).
    pub exit_cu: BTreeMap<Pubkey, u64>,
    pub skipped: Vec<(Pubkey, String)>,
    pub errors: Vec<String>,
}

impl TickReport {
    /// The exits of `kind` that landed.
    pub fn landed(&self, kind: ExitKind) -> impl Iterator<Item = &ExitRecord> {
        self.exits
            .iter()
            .filter(move |e| e.kind == kind && e.result.landed().is_some())
    }

    /// Whether `market` was observed this tick.
    pub fn observed(&self, market: &Pubkey) -> bool {
        self.observed.iter().any(|(m, _)| m == market)
    }
}

/// One `set_coverage_pause` attempt of the keeper (O17; the designated pauser).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoveragePauseRecord {
    pub market: Pubkey,
    pub paused: bool,
    pub result: ExitResult,
}

/// One `→ PumpSwap` transition of the keeper (duty 6, M6; [`crate::graduation::transition_ixs`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionRecord {
    pub market: Pubkey,
    /// Pool accounts created in it.
    pub created: usize,
    /// Tx3 of a straddle bundle (else the keeper's own transaction).
    pub bundled: bool,
    /// The `VenueStateChanged` events of the transaction: `(from, to)` venue bytes.
    pub venue_changes: Vec<(u8, u8)>,
    pub result: ExitResult,
}

/// The `VenueStateChanged` events of `out`: `(from, to)`.
fn venue_changes(out: &TxOutcome) -> Vec<(u8, u8)> {
    events::of::<shortwin::events::VenueStateChanged>(out)
        .iter()
        .map(|e| (e.from, e.to))
        .collect()
}

/// What a send attempt came to.
enum Sent {
    Landed {
        out: TxOutcome,
        format: TxFormat,
        bytes: usize,
        accounts: usize,
        price: u64,
        unit_limit: u32,
    },
    SimFailed {
        out: TxOutcome,
        format: TxFormat,
    },
    Failed {
        out: TxOutcome,
    },
    DryRun {
        out: TxOutcome,
    },
    TooLarge,
    /// Simulated; its limit exceeds the room left (`send_ixs_within`). Not sent.
    Deferred {
        unit_limit: u32,
    },
}

/// Custom errors that are the program's normal "not now" answers (no alert).
fn benign(code: u32) -> bool {
    [
        ShortwinError::ExitBoundNotMet,
        ShortwinError::ReferenceStale,
        ShortwinError::NotLiquidatable,
        ShortwinError::UseSolSettle,
        ShortwinError::UseExpire,
        ShortwinError::NotExpired,
        ShortwinError::SettlementNotAllowed,
    ]
    .into_iter()
    .any(|e| u32::from(e) == code)
}

/// U28: `⌊B̂_live · 10⁴ / B̂_ref⌋` of an evaluation, when both are defined.
fn b_ratio_bps(e: &PositionEval) -> Option<u64> {
    let (live, r) = (e.b_live?, e.b_ref?);
    (r > 0).then(|| u64::try_from(u128::from(live) * 10_000 / u128::from(r)).unwrap_or(u64::MAX))
}

fn is(code: Option<u32>, e: ShortwinError) -> bool {
    code == Some(u32::from(e))
}

/// The keeper (module docs).
pub struct Keeper<C: Chain> {
    chain: C,
    payer: Keypair,
    cfg: KeeperConfig,
    selection: Option<Vec<Pubkey>>,
    markets: Vec<Pubkey>,
    last_discovery: Option<i64>,
    tracks: BTreeMap<Pubkey, MarketTrack>,
    v1: V1Support,
    lookup_tables: Vec<AddressLookupTableAccount>,
    lookup_loaded: bool,
    alerts: Vec<Alert>,
    /// Duty 10: each covered market's pool as last seen.
    pools: BTreeMap<Pubkey, PoolWatch>,
    /// Duty 10: each watched ProgramData's deploy slot as last seen (`None`: unreadable).
    program_slots: BTreeMap<Pubkey, Option<u64>>,
    /// Upgrades already alerted (venue, deploy slot), and flag failures already alerted.
    upgrades_alerted: HashSet<(u8, u64)>,
    flag_failures_alerted: HashSet<(u8, u64)>,
    /// The second of the last `flag_venue_upgraded` attempt per (venue, deploy slot): at most
    /// one a second for the same upgrade; a new deploy slot is attempted at once.
    flag_tried: BTreeMap<(u8, u64), i64>,
    /// Duties 5 and 12: each covered Raydium or list-only market's pool-selection state.
    pool_tracks: BTreeMap<Pubkey, PoolTrack>,
    /// The chain the pool discovery's `getProgramAccounts` calls go to (`None`: `chain`).
    discovery: Option<C>,
    /// Duty 6: the second of the last `migrate_v2` attempt per market (one a second).
    migrate_tried: BTreeMap<Pubkey, i64>,
    /// Duty 6: the second of the last straddle attempt per market (one bundle a second: it
    /// completes the curve for every position of the market).
    straddle_tried: BTreeMap<Pubkey, i64>,
    /// Duty 6: graduation alerts already raised, per (market or position, detail).
    graduation_alerted: HashSet<(Pubkey, String)>,
    /// Duty 6 (M5): curve markets whose trader accounts (`trader_base_ata`, the pump UVA) the
    /// keeper found present; the second of the last attempt to create them per market.
    curve_ready: HashSet<Pubkey>,
    curve_prepare_tried: BTreeMap<Pubkey, i64>,
    /// Duty 6 (M6): per PumpSwap market, the pool accounts found present (re-checked when the list
    /// changes: a coin-creator change); the second of the last attempt to create them.
    pamm_ready: BTreeMap<Pubkey, Vec<Pubkey>>,
    pamm_prepare_tried: BTreeMap<Pubkey, i64>,
    /// Duty 6 (M6): the second of the last transition attempt per market.
    transition_tried: BTreeMap<Pubkey, i64>,
    /// Duties 6 and 13: the second of the last trader-side read, post-wipe attempt and cashback
    /// claim per market.
    sweep_checked: BTreeMap<Pubkey, i64>,
    post_wipe_tried: BTreeMap<Pubkey, i64>,
    cashback_claimed: BTreeMap<Pubkey, i64>,
    /// The next Jito tip account to use (in turn).
    tip_next: usize,
    /// O17: the `coverage_authority` key of the designated pauser (`[coverage_pause]`).
    coverage_authority: Option<Arc<Keypair>>,
    /// O17: the second of the last `set_coverage_pause` attempt per market (one a second).
    pause_tried: BTreeMap<Pubkey, i64>,
    /// O17: the flag value the keeper is bringing a market to (until the flag matches).
    pause_intent: BTreeMap<Pubkey, bool>,
    /// O17: coverage-pause alerts already raised, per (market, detail); a market's entries are
    /// cleared once its flag matches its coverage state.
    pause_alerted: HashSet<(Pubkey, String)>,
    /// T-CASCADE: (slot, compute units requested) of the keeper's exits per market.
    slot_cu: BTreeMap<Pubkey, (u64, u64)>,
    /// Duty 11: the second of the next read of the Pyth feed; the oracle alerts raised (cleared
    /// when a crank lands); the last aging alert: (the cache's `publish_time` it was raised for,
    /// the second it was raised) — one per stall (M7D-6), repeated hourly within one.
    oracle_next: i64,
    oracle_alerted: HashSet<String>,
    oracle_aging_alerted: Option<(i64, i64)>,
    /// The slot of the tick in progress.
    tick_slot: u64,
    /// Decision A (2026-10-01; ARCH §7.1 duty 8): the global keeper budget ([`crate::budget`]).
    budget: GlobalBudget,
    /// The charge class of each market's crank queued this tick ([`crate::budget::ChargeReq`]):
    /// exit-critical — the cranks exits depend on: a position near a threshold (duty 1: live CR
    /// below `near_cr_bps`·L, deadline within the window, armed, the post-change warm-up; without
    /// it `P_ema` stays where it was and no liquidation becomes possible), a liquidatable or
    /// expired position waiting for a fresh reference or its bound, the S3 chain, a market
    /// without a route: charged, never throttled ("exits continue") —, baseline or discretionary.
    crank_charges: BTreeMap<Pubkey, ChargeReq>,
    /// Markets whose crank this tick a graduation step forced (after the keeper's `migrate_v2` or
    /// straddle, a list-only market's re-detection): a graduation move, neither charged nor
    /// throttled.
    free_cranks: HashSet<Pubkey>,
    /// Markets whose crank the budget refused this tick.
    refused: HashSet<Pubkey>,
    /// Decision A: the second at which each below-floor market (tier 2) is read next — when its
    /// idle crank is due (not every tick: M7-SEC-8).
    idle_next: BTreeMap<Pubkey, i64>,
    /// M7-SEC-8: markets classified cold at their last full read (no open positions, nothing
    /// time-critical pending: read every `coverage.cold_interval_s`, staggered), and the second
    /// of each market's last full read.
    cold: HashSet<Pubkey>,
    last_read: BTreeMap<Pubkey, i64>,
    /// M7-SEC-3: positions already alerted as opened while the keeper held their market degraded.
    open_alerted: HashSet<Pubkey>,
    /// M7-SEC-8: this tick's trader-side facts of the markets whose periodic sweep check is due,
    /// read in one call before the markets ([`Self::prefetch_sweeps`]).
    sweep_prefetch: BTreeMap<Pubkey, Vec<Option<AccountData>>>,
    /// Counters (duty 8).
    pub metrics: Metrics,
}

impl<C: Chain> Keeper<C> {
    /// A keeper on `chain` paying with `payer` (a hot key with no protocol authority).
    pub fn new(chain: C, payer: Keypair, cfg: KeeperConfig) -> anyhow::Result<Self> {
        cfg.validate()?;
        Ok(Self {
            selection: cfg.markets.keys()?,
            chain,
            payer,
            cfg,
            markets: Vec::new(),
            last_discovery: None,
            tracks: BTreeMap::new(),
            v1: V1Support::Unknown,
            lookup_tables: Vec::new(),
            lookup_loaded: false,
            alerts: Vec::new(),
            pools: BTreeMap::new(),
            program_slots: BTreeMap::new(),
            upgrades_alerted: HashSet::new(),
            flag_failures_alerted: HashSet::new(),
            flag_tried: BTreeMap::new(),
            pool_tracks: BTreeMap::new(),
            discovery: None,
            migrate_tried: BTreeMap::new(),
            straddle_tried: BTreeMap::new(),
            graduation_alerted: HashSet::new(),
            curve_ready: HashSet::new(),
            curve_prepare_tried: BTreeMap::new(),
            pamm_ready: BTreeMap::new(),
            pamm_prepare_tried: BTreeMap::new(),
            transition_tried: BTreeMap::new(),
            sweep_checked: BTreeMap::new(),
            post_wipe_tried: BTreeMap::new(),
            cashback_claimed: BTreeMap::new(),
            tip_next: 0,
            coverage_authority: None,
            pause_tried: BTreeMap::new(),
            pause_intent: BTreeMap::new(),
            pause_alerted: HashSet::new(),
            slot_cu: BTreeMap::new(),
            oracle_next: 0,
            oracle_alerted: HashSet::new(),
            oracle_aging_alerted: None,
            tick_slot: 0,
            budget: GlobalBudget::default(),
            crank_charges: BTreeMap::new(),
            free_cranks: HashSet::new(),
            refused: HashSet::new(),
            idle_next: BTreeMap::new(),
            cold: HashSet::new(),
            last_read: BTreeMap::new(),
            open_alerted: HashSet::new(),
            sweep_prefetch: BTreeMap::new(),
            metrics: Metrics::default(),
        })
    }

    /// O17: this instance holds the `coverage_authority` key `kp` (a separate key from its fee
    /// payer). With `coverage_pause.enabled` it pauses opens on a degraded market with open
    /// positions and resumes them after `recovery_s` of restored coverage
    /// ([`crate::config::CoveragePauseConfig`]).
    pub fn with_coverage_authority(mut self, kp: Keypair) -> Self {
        self.coverage_authority = Some(Arc::new(kp));
        self
    }

    /// Replaces (or, with `None`, drops) the `coverage_authority` key between ticks (a key
    /// rotation; [`Self::with_coverage_authority`]).
    pub fn set_coverage_authority(&mut self, kp: Option<Keypair>) {
        self.coverage_authority = kp.map(Arc::new);
    }

    /// The `coverage_authority` key this instance holds, if any.
    pub fn coverage_authority(&self) -> Option<Pubkey> {
        self.coverage_authority.as_ref().map(|k| k.pubkey())
    }

    /// Sends the pool discovery's `getProgramAccounts` calls to `chain` instead of the
    /// keeper's own (`pool_select.discovery_rpc`, `--discovery-rpc`).
    pub fn with_discovery_chain(mut self, chain: C) -> Self {
        self.discovery = Some(chain);
        self
    }

    /// The chain the pool discovery uses.
    fn discovery_chain(&self) -> &C {
        self.discovery.as_ref().unwrap_or(&self.chain)
    }

    /// A market's pool-selection state (duties 5 and 12).
    pub fn pool_track(&self, market: &Pubkey) -> Option<&PoolTrack> {
        self.pool_tracks.get(market)
    }

    /// The chain backend.
    pub fn chain(&self) -> &C {
        &self.chain
    }

    /// The fee payer's key.
    pub fn payer(&self) -> Pubkey {
        self.payer.pubkey()
    }

    /// The configuration.
    pub fn config(&self) -> &KeeperConfig {
        &self.cfg
    }

    /// Mutable configuration (budgets, ceilings) between ticks.
    pub fn config_mut(&mut self) -> &mut KeeperConfig {
        &mut self.cfg
    }

    /// Whether v1 transactions are accepted, as far as the keeper knows.
    pub fn v1_support(&self) -> V1Support {
        self.v1
    }

    /// A market's coverage state.
    pub fn track(&self, market: &Pubkey) -> Option<&MarketTrack> {
        self.tracks.get(market)
    }

    /// The global keeper budget (decision A of 2026-10-01; [`crate::budget`]).
    pub fn budget(&self) -> &GlobalBudget {
        &self.budget
    }

    /// The budget's parameters this tick: `[budget]`, the idle cadence, the value-at-risk floor
    /// clamped to the deployment's `market_max_notional` (its bound), and (M7D-5) the per-crank
    /// charge estimated before the keeper has measured any: one signature shared by a full batch
    /// of the transaction format in use, `⌈5,000 / batch⌉` (834 at `batch_v1` = 6).
    fn budget_params(&self, config: &shortwin::state::GlobalConfig) -> BudgetParams {
        let b = &self.cfg.budget;
        let batch = match self.cfg.tx_format {
            TxFormatPolicy::V0 => self.cfg.batch_v0,
            _ => self.cfg.batch_v1,
        };
        let batch = u64::try_from(batch.max(1)).unwrap_or(1);
        BudgetParams {
            global: b.global_daily_lamports,
            var_floor: b.var_floor_lamports.min(config.params.market_max_notional),
            reserve_margin_bps: b.reserve_margin_bps,
            tail_share_bps: b.tail_share_bps,
            idle_interval_s: self.cfg.coverage.idle_interval_s,
            first_charge: budget::FIRST_CHARGE_LAMPORTS.div_ceil(batch),
        }
    }

    /// The covered markets as of the last discovery.
    pub fn markets(&self) -> &[Pubkey] {
        &self.markets
    }

    /// The lookup tables v0 transactions use (loaded at the first tick).
    pub fn lookup_tables(&self) -> &[AddressLookupTableAccount] {
        &self.lookup_tables
    }

    fn format(&self, now: i64) -> TxFormat {
        match self.cfg.tx_format {
            TxFormatPolicy::V1 => TxFormat::V1,
            TxFormatPolicy::V0 => TxFormat::V0,
            TxFormatPolicy::Auto => match self.v1 {
                V1Support::No { until } if now < until => TxFormat::V0,
                _ => TxFormat::V1,
            },
        }
    }

    /// A v1 refusal under the `auto` policy switches to v0 (returns `true`: retry).
    fn on_rejected(&mut self, format: TxFormat, reason: &str, now: i64) -> bool {
        if format != TxFormat::V1 || self.cfg.tx_format != TxFormatPolicy::Auto {
            return false;
        }
        self.v1 = V1Support::No {
            until: now.saturating_add(self.cfg.v1_reprobe_s),
        };
        self.metrics.v1_rejections += 1;
        self.alerts.push(Alert::V1Unsupported {
            reason: reason.to_string(),
        });
        true
    }

    async fn load_lookup_tables(&mut self) -> anyhow::Result<()> {
        if self.lookup_loaded {
            return Ok(());
        }
        let keys = self.cfg.lookup_table_keys()?;
        if !keys.is_empty() {
            let accs = self.chain.accounts(&keys).await?;
            self.lookup_tables = keys
                .iter()
                .zip(accs)
                .map(|(k, a)| {
                    let a = a.ok_or_else(|| anyhow!("lookup table {k} not found"))?;
                    tx::parse_lookup_table(*k, &a.owner, &a.data)
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            // A28: the protocol table should hold every static key (`shortwin-build-alt`); a
            // missing one only costs bytes, so this is a warning, not an error.
            let have: HashSet<Pubkey> = self
                .lookup_tables
                .iter()
                .flat_map(|t| t.addresses.iter().copied())
                .collect();
            let missing: Vec<String> = crate::alt::static_entries()
                .into_iter()
                .filter(|e| !have.contains(&e.key))
                .map(|e| format!("{} ({})", e.key, e.label))
                .collect();
            if !missing.is_empty() {
                tracing::warn!(
                    missing = ?missing,
                    "the lookup tables lack protocol ALT keys (scripts/build-alt.sh extend)"
                );
            }
        }
        self.lookup_loaded = true;
        Ok(())
    }

    async fn priority_price(&self, markets: &[Pubkey], exit: bool) -> u64 {
        let b = &self.cfg.budget;
        let recent = self
            .chain
            .recent_priority_fees(markets)
            .await
            .unwrap_or_default();
        let misses = markets
            .iter()
            .filter_map(|m| self.tracks.get(m))
            .map(|t| t.miss_streak)
            .max()
            .unwrap_or(0);
        let ceiling = if exit {
            b.exit_priority_ceiling
        } else {
            b.priority_ceiling
        };
        fees::priority_price(
            &recent,
            b.priority_percentile_bps,
            b.priority_floor,
            ceiling,
            b.escalate_bps,
            misses,
        )
    }

    /// Simulates `ixs` (compute ceiling, no priority price) in the current format, switching to
    /// v0 once on a v1 refusal.
    ///
    /// The simulation carries the priority price the send will use: a v0 transaction then has
    /// the same ComputeBudget instructions (each consumes compute units) as the one sent with
    /// `⌈units × 1.1⌉`.
    async fn simulate(
        &mut self,
        ixs: &[Instruction],
        now: i64,
        price: u64,
    ) -> anyhow::Result<(TxOutcome, TxFormat, solana_hash::Hash)> {
        self.simulate_signed(ixs, &[], now, price).await
    }

    /// [`Self::simulate`] with `signers` besides the fee payer (the `coverage_authority`).
    async fn simulate_signed(
        &mut self,
        ixs: &[Instruction],
        signers: &[&Keypair],
        now: i64,
        price: u64,
    ) -> anyhow::Result<(TxOutcome, TxFormat, solana_hash::Hash)> {
        for _ in 0..2 {
            let format = self.format(now);
            let alts = if format == TxFormat::V0 {
                self.lookup_tables.clone()
            } else {
                Vec::new()
            };
            let bh = self.chain.latest_blockhash().await?;
            let budget = ComputeBudget {
                unit_price: price,
                ..ComputeBudget::SIMULATION
            };
            let t = tx::build(format, &self.payer, signers, ixs, &budget, bh, &alts)?;
            if !tx::fits(format, &t) {
                return Ok((
                    TxOutcome {
                        err: Some(TxError::Rejected(format!(
                            "too large: {} B, {} accounts",
                            tx::size(&t),
                            tx::account_count(&t)
                        ))),
                        ..TxOutcome::default()
                    },
                    format,
                    bh,
                ));
            }
            self.metrics.simulations += 1;
            let sim = self.chain.simulate(&t).await?;
            if let Some(TxError::Rejected(r)) = &sim.err
                && self.on_rejected(format, r, now)
            {
                continue;
            }
            if sim.err.is_some() {
                self.metrics.simulation_failures += 1;
            }
            return Ok((sim, format, bh));
        }
        Err(anyhow!(
            "the cluster refused the transaction in both formats"
        ))
    }

    /// Simulates `ixs`, then sends them with the compute budget of the simulation
    /// (`⌈units × 1.1⌉`) and the priority price for `markets`.
    async fn send_ixs(
        &mut self,
        ixs: &[Instruction],
        markets: &[Pubkey],
        exit: bool,
        now: i64,
    ) -> anyhow::Result<Sent> {
        self.send_ixs_within(ixs, &[], markets, exit, now, None)
            .await
    }

    /// [`Self::send_ixs`] with `signers` besides the fee payer, and — with `room` — not sent
    /// ([`Sent::Deferred`]) when its compute-unit limit `⌈simulated × 1.1⌉` exceeds `room`
    /// (T-CASCADE's per-market, per-slot cap).
    async fn send_ixs_within(
        &mut self,
        ixs: &[Instruction],
        signers: &[&Keypair],
        markets: &[Pubkey],
        exit: bool,
        now: i64,
        room: Option<u64>,
    ) -> anyhow::Result<Sent> {
        for _ in 0..2 {
            let price = self.priority_price(markets, exit).await;
            let (sim, format, bh) = self.simulate_signed(ixs, signers, now, price).await?;
            if let Some(TxError::Rejected(r)) = &sim.err
                && r.starts_with("too large")
            {
                return Ok(Sent::TooLarge);
            }
            if sim.err.is_some() {
                return Ok(Sent::SimFailed { out: sim, format });
            }
            if self.cfg.dry_run {
                return Ok(Sent::DryRun { out: sim });
            }
            let budget = ComputeBudget {
                unit_limit: tx::cu_limit(sim.units_consumed),
                unit_price: price,
            };
            if let Some(room) = room
                && u64::from(budget.unit_limit) > room
            {
                return Ok(Sent::Deferred {
                    unit_limit: budget.unit_limit,
                });
            }
            let alts = if format == TxFormat::V0 {
                self.lookup_tables.clone()
            } else {
                Vec::new()
            };
            let t = tx::build(format, &self.payer, signers, ixs, &budget, bh, &alts)?;
            let (bytes, accounts) = (tx::size(&t), tx::account_count(&t));
            let out = self.chain.send(&t).await?;
            match &out.err {
                Some(TxError::Rejected(r)) => {
                    if self.on_rejected(format, &r.clone(), now) {
                        continue;
                    }
                    self.metrics.tx_failed += 1;
                    return Ok(Sent::Failed { out });
                }
                Some(_) => {
                    self.metrics.tx_failed += 1;
                    return Ok(Sent::Failed { out });
                }
                None => {}
            }
            self.metrics.tx_landed += 1;
            if format == TxFormat::V1 {
                self.v1 = V1Support::Yes;
            }
            return Ok(Sent::Landed {
                out,
                format,
                bytes,
                accounts,
                price,
                unit_limit: budget.unit_limit,
            });
        }
        Err(anyhow!(
            "the cluster refused the transaction in both formats"
        ))
    }

    async fn discover(&mut self, now: i64) -> anyhow::Result<()> {
        let due = self.last_discovery.is_none_or(|t| {
            now.saturating_sub(t) >= self.cfg.coverage.discovery_interval_s || now < t
        });
        if !due {
            return Ok(());
        }
        let all = view::discover_markets(&self.chain).await?;
        self.markets = match &self.selection {
            None => all,
            Some(sel) => all.into_iter().filter(|k| sel.contains(k)).collect(),
        };
        self.last_discovery = Some(now);
        Ok(())
    }

    /// One tick (module docs).
    pub async fn tick(&mut self) -> anyhow::Result<TickReport> {
        self.load_lookup_tables().await?;
        let clock = self.chain.clock().await?;
        self.tick_slot = clock.slot;
        self.discover(clock.unix_timestamp).await?;
        // M7-SEC-8: the open positions first (one `getProgramAccounts`), then in full only the
        // markets with positions or something time-critical pending, and the cold ones on their
        // staggered poll.
        let positions = view::open_positions(&self.chain).await?;
        let read = self.markets_to_read(clock.unix_timestamp, &positions);
        let mut view = view::load_with_positions(&self.chain, &read, &positions).await?;
        let now = view.clock.unix_timestamp;
        self.tick_slot = view.clock.slot;
        self.crank_charges.clear();
        self.free_cranks.clear();
        self.refused.clear();
        // Decision A (2026-10-01; ARCH §7.1 duty 8): the global budget's UTC day, and the class
        // of every market read this tick (its value at risk from the `Market` as read).
        let params = self.budget_params(&view.config);
        self.budget.roll(now, &params);
        let known: HashSet<Pubkey> = self.markets.iter().copied().collect();
        let gone: Vec<Pubkey> = self
            .budget
            .markets
            .keys()
            .filter(|k| !known.contains(k))
            .copied()
            .collect();
        for k in gone {
            self.budget.forget(&k);
            self.idle_next.remove(&k);
        }
        for mv in &view.markets {
            let open = !mv.positions.is_empty() || mv.market.open_positions > 0;
            let var = budget::value_at_risk(&mv.market);
            self.budget
                .classify(now, mv.key, open, var, mv.market.created_at, &params);
        }
        let mut report = TickReport {
            clock: Some(view.clock),
            skipped: view.skipped.clone(),
            ..TickReport::default()
        };
        self.metrics.ticks += 1;
        // Close the previous seconds' coverage accounting before this second's work.
        for (key, track) in &mut self.tracks {
            if let Some(true) = track.close_second(*key, now, &self.cfg.coverage, &mut self.alerts)
            {
                self.metrics.market(*key).misses += 1;
            }
            // Seconds that required nothing are met too (a market whose positions closed while
            // it was degraded; the seconds between idle observations); seconds the keeper did not
            // run on a market with open positions are not.
            let open = view
                .market(key)
                .is_some_and(|mv| !mv.positions.is_empty() || mv.market.open_positions > 0);
            track.advance(*key, now, open, &self.cfg.coverage, &mut self.alerts);
        }

        // Duty 10: the latch first (an upgrade is flagged in the slot it is seen), then the
        // markets' pools.
        if self.cfg.venue_watch.flag_upgrades {
            self.watch_programs(now, &mut report).await;
        }
        if self.cfg.venue_watch.pool_admin {
            self.watch_pools(&view);
        }
        // Duty 11: the SOL/USD cache the tiers read (before the markets: a crank this tick then
        // tiers with it).
        if self.cfg.oracle.enabled {
            self.oracle_duty(&view, &mut report).await;
        }

        self.prefetch_sweeps(&view).await;
        let mut crank_queue: Vec<(Pubkey, Due)> = Vec::new();
        for idx in 0..view.markets.len() {
            if let Err(e) = self
                .process_market(&mut view, idx, &mut report, &mut crank_queue)
                .await
            {
                report
                    .errors
                    .push(format!("{}: {e:#}", view.markets[idx].key));
            }
        }
        self.send_cranks(&view, crank_queue, &mut report, true)
            .await;
        // Pool selection after this second's observations and exits (M4-SEC-7): its RPC work —
        // discovery, ranking, the best candidates' accounts — never delays duties 1–4; the
        // cranks it asks for (a failing market's `pool_fail` chain) go out in a batch of their
        // own.
        if self.cfg.pool_select.enabled {
            let mut chain_cranks: Vec<(Pubkey, Due)> = Vec::new();
            self.select_pools(&mut view, &mut report, &mut chain_cranks)
                .await;
            if !chain_cranks.is_empty() {
                // Pool selection's chain cranks: neither charged nor throttled (ARCH §7.1 duty 8).
                self.send_cranks(&view, chain_cranks, &mut report, false)
                    .await;
            }
        }

        // O17: the designated pauser brings each market's `paused_coverage` flag in line with its
        // coverage state (after this second's observations, which may have ended a gap).
        self.coverage_pauses(&view, &mut report).await;

        for (key, track) in &self.tracks {
            self.metrics.market(*key).degraded = track.state.is_degraded();
        }
        self.schedule_idle_reads(&view, &report);
        self.budget_metrics(&view);
        self.classify_reads(&view, &report);
        for a in self.alerts.drain(..) {
            a.log();
            report.alerts.push(a);
        }
        Ok(report)
    }

    /// M7-SEC-8: the trader-side facts (`sweep::fact_keys`) of every market whose periodic sweep
    /// check is due this tick, in one read (one round trip; as built each market's check was a
    /// read of its own, so list-only junk markets added one sequential call a minute each). A
    /// post-wipe sweep, or a market an exit landed on this tick, reads its facts fresh.
    async fn prefetch_sweeps(&mut self, view: &ChainView) {
        self.sweep_prefetch.clear();
        let cfg = &self.cfg.sweep;
        if !cfg.sweep {
            return;
        }
        let now = view.clock.unix_timestamp;
        let due: Vec<&MarketView> =
            view.markets
                .iter()
                .filter(|mv| {
                    !(cfg.post_wipe && sweep::wipe_pending(&mv.market))
                        && self.sweep_checked.get(&mv.key).is_none_or(|t| {
                            now.saturating_sub(*t) >= cfg.check_interval_s || now < *t
                        })
                })
                .collect();
        if due.is_empty() {
            return;
        }
        let keys: Vec<Pubkey> = due
            .iter()
            .flat_map(|mv| sweep::fact_keys(&mv.market))
            .collect();
        let Ok(accs) = self.chain.accounts(&keys).await else {
            return;
        };
        for (mv, chunk) in due.iter().zip(accs.chunks(sweep::FACT_KEYS_LEN)) {
            self.sweep_prefetch.insert(mv.key, chunk.to_vec());
        }
    }

    /// M7-SEC-8: the covered markets this tick reads in full — every market with open positions
    /// (`positions`, read this tick), every market not classified cold at its last read (new
    /// markets included), and each cold market on its poll: every `cold_interval_s`, at a phase
    /// staggered by its key (and at once when a poll was skipped). Decision A (2026-10-01; ARCH
    /// §7.1 duty 8): a market with positions **below the value-at-risk floor** at its last read is
    /// read only when its idle crank is due ([`Self::schedule_idle_reads`]), in the tick its set
    /// of open positions changes (this tick's first read shows it), and while the keeper's own
    /// `set_coverage_pause` on it is pending — junk with positions does not grow the per-tick
    /// reads.
    fn markets_to_read(
        &mut self,
        now: i64,
        positions: &std::collections::HashMap<Pubkey, Vec<PositionView>>,
    ) -> Vec<Pubkey> {
        let every = self.cfg.coverage.cold_interval_s.max(1);
        let mut out = Vec::with_capacity(self.markets.len());
        let mut skipped = 0u64;
        let mut idle_skipped = 0u64;
        for k in &self.markets {
            if let Some(ps) = positions.get(k)
                && self.budget.tier(k) == Some(Tier::BelowFloor)
            {
                let changed = self
                    .tracks
                    .get(k)
                    .is_none_or(|t| t.positions_changed(ps.iter().map(|p| p.key)));
                let due = self.idle_next.get(k).is_none_or(|t| now >= *t);
                if changed || due || self.pause_intent.contains_key(k) {
                    out.push(*k);
                } else {
                    idle_skipped += 1;
                }
                continue;
            }
            let hot = positions.contains_key(k) || !self.cold.contains(k);
            let due = self.last_read.get(k).is_none_or(|t| {
                now < *t
                    || now.saturating_sub(*t) >= every
                    || (now > *t && now.saturating_add(cold_phase(k, every)).rem_euclid(every) == 0)
            });
            if hot || due {
                out.push(*k);
            } else {
                skipped += 1;
            }
        }
        self.metrics.markets_read = self
            .metrics
            .markets_read
            .saturating_add(u64::try_from(out.len()).unwrap_or(u64::MAX));
        self.metrics.markets_cold_skipped =
            self.metrics.markets_cold_skipped.saturating_add(skipped);
        self.metrics.markets_idle_skipped = self
            .metrics
            .markets_idle_skipped
            .saturating_add(idle_skipped);
        out
    }

    /// Decision A: after the tick, each below-floor market read gets the second of its next read
    /// — when its next idle crank is due (`idle_interval_s` after its last observation; a crank
    /// due and not landed is retried the next second), or `idle_interval_s` from now when the
    /// budget refused it (the tail spent).
    ///
    /// M7D-1: while a position is near an exit (near a threshold or its deadline, expired, armed,
    /// waiting only for a fresh reference, or with any plan but `Idle` / `Watch`), the market is
    /// read **every second the reference of the keeper's last observation is still fresh** (its
    /// positions' `ref_max_age_s`, 2 s by default): an exit needs a completed bucket at most that
    /// old, so the crank at `t` is followed by the read — and the exit the program accepts — at
    /// `t + 1` (and `t + 2`). Cranks stay at the idle cadence; a landed exit is an observation,
    /// so a cascade keeps going second by second. As reviewed, each read came 30 s after the
    /// last observation and so always found its own reference stale: no exit ever went.
    fn schedule_idle_reads(&mut self, view: &ChainView, report: &TickReport) {
        let now = view.clock.unix_timestamp;
        let idle = self.cfg.coverage.idle_interval_s.max(1);
        for mv in &view.markets {
            if self.budget.tier(&mv.key) != Some(Tier::BelowFloor) {
                self.idle_next.remove(&mv.key);
                continue;
            }
            let last = self.tracks.get(&mv.key).and_then(|t| t.last_obs);
            let next = if self.refused.contains(&mv.key) {
                now.saturating_add(idle)
            } else {
                let fresh_for = mv
                    .positions
                    .iter()
                    .map(|p| i64::from(p.pos.ref_max_age_s))
                    .max()
                    .unwrap_or(0)
                    .max(1);
                let near_exit = report.evaluations.get(&mv.key).is_some_and(|evs| {
                    evs.iter().any(|e| {
                        e.near
                            || e.expired
                            || e.armed
                            || e.stale_exit
                            || !matches!(e.plan, Plan::Idle | Plan::Watch)
                    })
                });
                match last {
                    Some((t, _)) if near_exit && now < t.saturating_add(fresh_for) => {
                        now.saturating_add(1)
                    }
                    Some((t, _)) => t.saturating_add(idle).max(now.saturating_add(1)),
                    None => now.saturating_add(1),
                }
            };
            self.idle_next.insert(mv.key, next);
        }
    }

    /// Decision A: the budget's per-market gauges of the markets read this tick, and its global
    /// balances.
    fn budget_metrics(&mut self, view: &ChainView) {
        let ranked = self.budget.ranked();
        for mv in &view.markets {
            let Some(b) = self.budget.markets.get(&mv.key) else {
                continue;
            };
            let rank = ranked
                .iter()
                .position(|k| *k == mv.key)
                .map_or(0, |i| i + 1);
            let m = self.metrics.market(mv.key);
            m.var_lamports = b.var;
            m.budget_tier = b.tier.number();
            m.budget_rank = u64::try_from(rank).unwrap_or(u64::MAX);
            m.budget_unspent_lamports = b.unspent;
            m.budget_spent_exit_lamports = b.spent[0];
            m.budget_spent_baseline_lamports = b.spent[1];
            m.budget_spent_discretionary_lamports = b.spent[2];
        }
        let g = &self.budget;
        self.metrics.budget_global_lamports = g.global;
        self.metrics.budget_free_lamports = g.free;
        self.metrics.budget_tail_lamports = g.tail;
        self.metrics.budget_overdraft_lamports = g.overdraft;
        self.metrics.budget_spent_lamports = g.spent;
        self.metrics.budget_excess_lamports = g.excess;
    }

    /// M7-SEC-8: after the tick, each market read is classified: **hot** (read every tick) while it
    /// is a PumpSwap or Raydium market, a curve near completion ([`Self::hot`]), or has open
    /// positions or anything time-critical pending — a per-second, no-route,
    /// listing-window or warm-up need, a guard, a burst or liquidity window, degraded mode or the
    /// coverage flag, a graduation step (a completed or migrating curve, a valid canonical pool,
    /// a re-detection), a post-wipe sweep, a pool-selection window, chain or pending ranking —,
    /// **cold** otherwise. A market whose needs this tick did not compute stays hot.
    fn classify_reads(&mut self, view: &ChainView, report: &TickReport) {
        let now = view.clock.unix_timestamp;
        for mv in &view.markets {
            self.last_read.insert(mv.key, now);
            if self.hot(mv, report.needs.get(&mv.key), now) {
                self.cold.remove(&mv.key);
            } else {
                self.cold.insert(mv.key);
            }
        }
        let known: HashSet<Pubkey> = self.markets.iter().copied().collect();
        self.cold.retain(|k| known.contains(k));
        self.last_read.retain(|k, _| known.contains(k));
    }

    /// Whether `mv` must be read every tick ([`Self::classify_reads`]). Only a list-only market,
    /// or a curve market that a buy of more than `coverage.hot_curve_completion_lamports` is still
    /// away from completing, can be cold — the markets anyone can list for a fee and rent
    /// (≈ 0.035–0.038 SOL at ARCH §3.2's rent — the M7 review counted ≈ 0.047; a pump coin
    /// ≈ 0.02 SOL more); a PumpSwap or Raydium market needs real liquidity to exist and stays hot,
    /// so its venue-drift alerts come at once.
    fn hot(&self, mv: &MarketView, needs: Option<&MarketNeeds>, now: i64) -> bool {
        let Some(n) = needs else { return true };
        let m = mv.market.as_ref();
        let key = mv.key;
        let cheap = match mv.kind {
            VenueKind::None => true,
            VenueKind::PumpCurve => graduation::completion_quote(mv)
                .is_some_and(|(_, cost)| cost > self.cfg.coverage.hot_curve_completion_lamports),
            _ => false,
        };
        if !cheap {
            return true;
        }
        let open = !mv.positions.is_empty() || m.open_positions > 0;
        let needs_now = n.per_second
            || n.no_route
            || n.start_chain
            || n.listing
            || n.warm_up
            || n.warming
            || n.guarded
            || n.s3_due_executable
            || n.liq_covered;
        let track = self.tracks.get(&key).is_some_and(|t| {
            t.in_burst(now)
                || t.liq_covered(now)
                || t.state.is_degraded()
                || t.flag_seen == Some(true)
        });
        let graduation = mv.kind == VenueKind::PumpMigrating
            || graduation::migration_due(mv)
            || graduation::transition_due(mv)
            || graduation::redetection_due(mv);
        let pool = m.fp_t0 != 0
            || m.pool_fail_since != 0
            || m.no_route_since != 0
            || self.pool_tracks.get(&key).is_some_and(|t| {
                t.pinned.is_some()
                    || !t.rank_queue.is_empty()
                    || t.keep_chain
                    || t.last_pool_fail == Some(true)
            });
        open || needs_now
            || track
            || self.pause_intent.contains_key(&key)
            || m.has_flag(MARKET_FLAG_PAUSED_COVERAGE)
            || graduation
            || sweep::wipe_pending(m)
            || pool
    }

    async fn process_market(
        &mut self,
        view: &mut ChainView,
        idx: usize,
        report: &mut TickReport,
        crank_queue: &mut Vec<(Pubkey, Due)>,
    ) -> anyhow::Result<()> {
        let clock = view.clock;
        let (now, slot) = (clock.unix_timestamp, clock.slot);
        let key = view.markets[idx].key;
        let cov = self.cfg.coverage.clone();
        {
            let mv = &view.markets[idx];
            let open = !mv.positions.is_empty() || mv.market.open_positions > 0;
            let track = self.tracks.entry(key).or_default();
            track.record_view(
                now,
                mv.state.price(),
                mv.state.liquidity(),
                (mv.market.venue, mv.market.pool_cache[0]),
            );
            let unpausing = self.pause_intent.get(&key) == Some(&false);
            let flag = mv.market.has_flag(MARKET_FLAG_PAUSED_COVERAGE);
            let covered = self.budget.tier(&key) == Some(Tier::Covered);
            let pauser = self.cfg.coverage_pause.enabled;
            // M7-SEC-3: while the keeper holds the market degraded, a flag found cleared that it did
            // not clear, and a position that appeared while the flag was last seen set, were
            // signed by the `coverage_authority` key outside this instance.
            if let Some(since) = track.state.degraded_since()
                && track.flag_seen == Some(true)
            {
                if !flag && !unpausing {
                    self.alerts
                        .push(Alert::CoveragePauseCleared { market: key });
                }
                for p in mv
                    .positions
                    .iter()
                    .filter(|p| !track.knows_position(&p.key))
                {
                    if self.open_alerted.insert(p.key) {
                        self.metrics.market(key).opens_while_degraded += 1;
                        self.alerts.push(Alert::OpenWhileDegraded {
                            market: key,
                            position: p.key,
                            degraded_since: since,
                        });
                    }
                }
                // ★OWNER-M7-3 (b): a pause this keeper only adopted (found set, or set by
                // someone else) that someone else cleared — the Emergency multisig — stays clear;
                // a market the keeper degraded itself stays degraded (the pauser sets it again).
                if !flag && !unpausing {
                    track.on_flag_cleared_by_other();
                }
            }
            track.set_positions(mv.positions.iter().map(|p| p.key));
            match track.sync_pause_flag(now, flag, unpausing) {
                // ★OWNER-M7-3 (b): the designated pauser saw the flag clear, then set, without
                // having sent the set: a second or compromised coverage key.
                Some(FlagSet::Foreign) if pauser => {
                    self.alerts
                        .push(Alert::CoveragePauseForeign { market: key });
                }
                Some(_) => self.alerts.push(Alert::CoveragePauseFound { market: key }),
                None => {}
            }
            if covered {
                let found = track.check_foreign(&mv.market, &cov, open);
                if let Some(sec) = found.price {
                    report.bursts.push((key, sec));
                    self.metrics.market(key).bursts += 1;
                }
                if found.genuine.is_some() {
                    self.metrics.market(key).foreign_moves += 1;
                }
                if let Some(sec) = found.liquidity {
                    report.liq_covers.push((key, sec));
                    if found.liq_started {
                        self.metrics.market(key).liq_covers += 1;
                    }
                }
            } else {
                // Decision A (ARCH §7.1 duties 2, 8): price bursts and liquidity-anomaly coverage
                // run on covered markets only.
                track.stop_bursts();
            }
        }

        // Duty 6 (M5, M6): a list-only market whose detection set shows a pump venue again is
        // cranked onto it (`crank_price`'s re-detection, ARCH §2.5 `None` row); a curve or
        // PumpSwap market's trader accounts, which the re-detection and the graduation do not
        // create, are created when missing.
        let mut force_crank = graduation::redetection_due(&view.markets[idx]);
        if view.markets[idx].kind == VenueKind::PumpCurve && !self.curve_ready.contains(&key) {
            self.prepare_curve_trader(view, idx, now, report).await?;
        }
        if view.markets[idx].kind == VenueKind::PumpSwap {
            self.prepare_pamm_trader(view, idx, now, report).await?;
        }

        // Duty 6: `migrate_v2` as soon as the curve is complete (before the exits: a migrating
        // market's exits pause until then).
        let mut observed = false;
        if self.cfg.graduation.migrate
            && graduation::migration_due(&view.markets[idx])
            && self.migrate(view, idx, now, report).await?
        {
            force_crank = true;
        }
        // Duty 6 (M6): the `→ PumpSwap` transition as soon as the canonical pool is valid (after
        // the keeper's own `migrate_v2` or anyone's): the pool's accounts, the crank onto the pool
        // and its first observation, which is the market's observation this second.
        if graduation::transition_due(&view.markets[idx])
            && self.transition(view, idx, now, slot, report).await?
        {
            observed = true;
        }

        let mut attempted: HashSet<(Pubkey, ExitKind)> = HashSet::new();
        let mut straddled = false;
        let mut due_before: Option<Option<Due>> = None;
        loop {
            let mv = view.markets[idx].clone();
            let proj = decide::project(&view.config, &mv, clock)?;
            let evals: Vec<PositionEval> = decide::evaluate_all(&mv, &proj, &cov)
                .into_iter()
                .filter_map(|r| match r {
                    Ok(e) => Some(e),
                    Err((k, e)) => {
                        report.errors.push(format!("{key}/{k}: {e}"));
                        None
                    }
                })
                .collect();
            let needs = decide::market_needs(
                &mv,
                &proj,
                &evals,
                &cov,
                view.config.params.pool_list_window_s,
                self.tracks[&key].liq_covered(now),
            );
            if due_before.is_none() {
                // U28: the positions waiting for their bound, once per tick.
                for e in &evals {
                    if matches!(e.plan, Plan::LiquidateWait | Plan::ExpireWait)
                        && let Some(bps) = b_ratio_bps(e)
                    {
                        self.metrics.b_ratio_wait.record(bps);
                    }
                }
                // Decision A (ARCH §7.1 duty 8): below the value-at-risk floor, the idle cadence
                // only — no per-second coverage of any kind; exits still go on these reads.
                let below = self.budget.tier(&key) == Some(Tier::BelowFloor);
                let due_needs = if below {
                    MarketNeeds {
                        open_positions: needs.open_positions,
                        ..MarketNeeds::default()
                    }
                } else {
                    needs
                };
                let due = self.tracks[&key].observation_due(&due_needs, now, slot, &cov);
                if let Some(t) = self.tracks.get_mut(&key)
                    && !below
                    && (needs.per_second || needs.warm_up || t.in_burst(now))
                {
                    // M7-SEC-1 (b): the seconds a slow tick skips after this one were required.
                    t.note_every_second(now);
                }
                if let Some(d) = due {
                    if let Some(t) = self.tracks.get_mut(&key) {
                        t.required(now);
                    }
                    report.due.push((key, d));
                }
                due_before = Some(due);
            }
            report.needs.insert(key, needs);
            report.evaluations.insert(key, evals.clone());

            // The next exit to attempt.
            let next = evals.iter().find_map(|e| {
                let kind = match e.plan {
                    Plan::Liquidate => ExitKind::Liquidate,
                    Plan::Expire => ExitKind::Expire,
                    // A straddle whose bundle does not go out falls back to `sol_settle` (the
                    // decision tree's last branch).
                    Plan::SolSettle | Plan::S3Probe | Plan::Straddle => ExitKind::SolSettle,
                    _ => return None,
                };
                (!attempted.contains(&(e.key, kind))).then_some((e.clone(), kind))
            });
            let Some((ev, kind)) = next else { break };
            attempted.insert((ev.key, kind));
            let Some(pv) = mv.positions.iter().find(|p| p.key == ev.key).cloned() else {
                continue;
            };
            // Duty 6: one straddle bundle per market and tick (it completes the curve for every
            // position of the market).
            if ev.plan == Plan::Straddle && !straddled {
                straddled = true;
                if let Some(with_tx3) = self.straddle(&mv, &pv, now, slot, report).await {
                    observed |= with_tx3;
                    force_crank = true;
                    let fresh = view::load(&self.chain, &[key]).await?;
                    if let Some(m) = fresh.markets.into_iter().next() {
                        view.markets[idx] = m;
                    }
                    // V1-9's tx1 + tx2: the keeper's own transition right after the bundle.
                    if !with_tx3
                        && graduation::transition_due(&view.markets[idx])
                        && self.transition(view, idx, now, slot, report).await?
                    {
                        observed = true;
                    }
                    continue;
                }
            }
            if kind == ExitKind::SolSettle {
                let track = self.tracks.entry(key).or_default();
                if track.settle_sent.get(&pv.key) == Some(&now) {
                    continue;
                }
                if ev.plan == Plan::S3Probe {
                    let last = track.s3_probe.get(&pv.key).copied();
                    let throttled =
                        last.is_some_and(|t| now.saturating_sub(t) < cov.s3_probe_interval_s);
                    if throttled && !ev.expired {
                        continue;
                    }
                    track.s3_probe.insert(pv.key, now);
                }
            }
            let result = self.attempt(&mv, &pv, kind, ev.plan, now).await;
            if matches!(result, ExitResult::Deferred { .. }) {
                // T-CASCADE: the market's exit budget for this slot is spent; the rest (most
                // under-collateralised first: `evaluate_all`'s order) goes out next slot.
                self.metrics.market(key).exits_deferred += 1;
                self.account_exit(&mv, &pv, kind, ev.plan, result, now, slot, report);
                break;
            }
            let mut chained: Option<ExitKind> = None;
            if let ExitResult::Refused { code, .. } = &result {
                if is(*code, ShortwinError::UseSolSettle) {
                    chained = Some(ExitKind::SolSettle);
                } else if is(*code, ShortwinError::UseExpire) {
                    chained = Some(ExitKind::Expire);
                }
            }
            let exit_landed =
                result.landed().is_some() && matches!(kind, ExitKind::Liquidate | ExitKind::Expire);
            let mut landed = self.account_exit(&mv, &pv, kind, ev.plan, result, now, slot, report);
            // U28: `B̂_live/B̂_ref` at the keeper's decision of a landed liquidation or expiry.
            if exit_landed && let Some(bps) = b_ratio_bps(&ev) {
                self.metrics.b_ratio_landed.record(bps);
            }
            // The program redirected the exit (`UseSolSettle`, `UseExpire`): follow it once.
            if !landed
                && let Some(next_kind) = chained
                && attempted.insert((pv.key, next_kind))
            {
                let r = self.attempt(&mv, &pv, next_kind, ev.plan, now).await;
                landed = self.account_exit(&mv, &pv, next_kind, ev.plan, r, now, slot, report);
            }
            if landed {
                observed = true;
                // Re-read the market: the exit moved it (a buyback, an arming write, a
                // settlement that removed a position).
                let fresh = view::load(&self.chain, &[key]).await?;
                if let Some(m) = fresh.markets.into_iter().next() {
                    view.markets[idx] = m;
                }
            }
        }

        let due = due_before.flatten();
        if observed {
            report.observed.push((key, due));
        } else if let Some(d) = due.or(force_crank.then_some(Due::Second)) {
            if force_crank {
                // A graduation step's crank (after the keeper's `migrate_v2` or straddle, a
                // list-only market's re-detection): a graduation move, never charged or throttled.
                self.free_cranks.insert(key);
            } else {
                let req = self.charge_of(&key, d, now, report);
                self.crank_charges.insert(key, req);
            }
            crank_queue.push((key, d));
        }

        // Duties 6 and 13: the trader side, after this second's exits (a settlement that wiped the
        // pool is followed by its post-wipe sweep in the same second).
        self.sweep_duties(view, idx, now, report).await?;
        Ok(())
    }

    /// Decision A (2026-10-01; ARCH §7.1 duty 8): the charge class of `key`'s crank due for `d`
    /// at `now`. On a covered market: **exit-critical** — the cranks exits depend on, never
    /// throttled (O17): a position near a threshold (the EMA leg must follow the price for a
    /// liquidation to become possible), a liquidatable or expired position waiting for a fresh
    /// reference (`Observe`) or its bound (`*Wait`), an armed position, the S3 chain, a market
    /// without a route — **at most one crank a second** (M7D-4: the per-second coverage these
    /// need; a burst's further per-slot cranks in a second the keeper already observed are
    /// discretionary) —; **discretionary** — a price burst (`Due::Slot`) or per-second coverage
    /// owed to the liquidity anomaly alone —; **baseline** otherwise (the idle cadence, the guard
    /// coverage after a switch, per-second coverage of a market off its EMA). Below the floor
    /// (M7D-2): the idle crank an exit waits on — a position that waits only for a fresh
    /// reference (`PositionEval::stale_exit`) or its bound, an armed position, the S3 chain, a
    /// market without a route — is exit-critical (the tail, then the overdraft: exits are never
    /// paused); any other idle crank baseline (the tail). Without positions: a warm-up or
    /// listing-window crank (may overdraw), or another baseline charge.
    fn charge_of(&self, key: &Pubkey, d: Due, now: i64, report: &TickReport) -> ChargeReq {
        let evals = report.evaluations.get(key);
        let needs = report.needs.get(key).copied().unwrap_or_default();
        let chain = needs.no_route || needs.start_chain || needs.s3_due_executable;
        match self.budget.tier(key).unwrap_or_default() {
            Tier::Covered => {
                let serves_exit = evals.is_some_and(|evs| {
                    evs.iter()
                        .any(|e| e.near || !matches!(e.plan, Plan::Idle | Plan::Watch))
                }) || chain;
                // M7D-4: an observation of the keeper's already landed this second (a crank in an
                // earlier slot, an exit): the per-second coverage an exit needs is met.
                let observed = self
                    .tracks
                    .get(key)
                    .and_then(|t| t.last_obs)
                    .is_some_and(|(t, _)| t >= now);
                let near =
                    evals.is_some_and(|evs| evs.iter().any(|e| e.near || e.plan != Plan::Idle));
                let liq_only = needs.liq_covered
                    && !(near
                        || needs.ema_deviates
                        || needs.warming
                        || needs.guarded
                        || needs.s3_due_executable);
                let class = if serves_exit && !(d == Due::Slot && observed) {
                    ChargeClass::ExitCritical
                } else if d == Due::Slot || (d == Due::Second && liq_only) {
                    ChargeClass::Discretionary
                } else {
                    ChargeClass::Baseline
                };
                ChargeReq::of(class)
            }
            Tier::BelowFloor => {
                let serves_exit = evals.is_some_and(|evs| {
                    evs.iter().any(|e| {
                        e.stale_exit || e.armed || !matches!(e.plan, Plan::Idle | Plan::Watch)
                    })
                }) || chain;
                ChargeReq::of(if serves_exit {
                    ChargeClass::ExitCritical
                } else {
                    ChargeClass::Baseline
                })
            }
            Tier::NoPositions => ChargeReq {
                class: ChargeClass::Baseline,
                warm_up: d == Due::Listing || needs.warm_up,
            },
        }
    }

    /// Duty 6 (M5; module docs): the trader's base ATA and pump UVA of `view.markets[idx]`'s curve
    /// market ([`graduation::trader_account_keys`]) are read once; the missing ones are created
    /// (at most one attempt a second), and the market is marked ready when both exist.
    async fn prepare_curve_trader(
        &mut self,
        view: &ChainView,
        idx: usize,
        now: i64,
        report: &mut TickReport,
    ) -> anyhow::Result<()> {
        let mv = &view.markets[idx];
        let key = mv.key;
        let keys = graduation::trader_account_keys(&mv.market);
        let accs = self.chain.accounts(&keys).await?;
        let missing = [
            accs.first().is_none_or(Option::is_none),
            accs.get(1).is_none_or(Option::is_none),
        ];
        if missing == [false, false] {
            self.curve_ready.insert(key);
            return Ok(());
        }
        if self.curve_prepare_tried.get(&key) == Some(&now) {
            return Ok(());
        }
        self.curve_prepare_tried.insert(key, now);
        let ixs = graduation::trader_account_ixs(&self.payer.pubkey(), &mv.market, missing);
        let result = self.send_exit(&ixs, key, ExtrasSel::NONE, now).await;
        if result.landed().is_some() {
            self.curve_ready.insert(key);
        }
        report.curve_accounts.push((key, result));
        Ok(())
    }

    /// The pool accounts of `accounts` (from [`graduation::pamm_accounts_of`]) that do not exist
    /// yet: a UVA not owned by PumpSwap (or empty), a WSOL ATA that is not an SPL Token account.
    async fn missing_pamm(
        &self,
        accounts: &[(Pubkey, PammAccount)],
    ) -> anyhow::Result<Vec<(Pubkey, PammAccount)>> {
        let keys: Vec<Pubkey> = accounts.iter().map(|(k, _)| *k).collect();
        let accs = self.chain.accounts(&keys).await?;
        Ok(accounts
            .iter()
            .zip(accs)
            .filter(|((_, what), acc)| {
                let owner = what.owner_program();
                acc.as_ref()
                    .is_none_or(|a| a.owner != owner || a.data.is_empty())
            })
            .map(|(a, _)| *a)
            .collect())
    }

    /// Duty 6 (M6): the pool accounts `view.markets[idx]`'s PumpSwap trades require (the trader's
    /// base ATA, and ARCH §3.12's existence requirement: the trader's pAMM UVA, the coin-creator
    /// vault's WSOL ATA, the two recipients' WSOL ATAs, on a cashback coin the trader's cashback
    /// ATA) are read once per list
    /// (a coin-creator change makes a new list); the missing ones are created (at most one attempt
    /// a second), and the list is marked ready when every account exists.
    async fn prepare_pamm_trader(
        &mut self,
        view: &ChainView,
        idx: usize,
        now: i64,
        report: &mut TickReport,
    ) -> anyhow::Result<()> {
        let mv = &view.markets[idx];
        let key = mv.key;
        // A pool that does not parse (a layout change, A63) has no trade to prepare for.
        let Ok(accounts) = graduation::pamm_accounts_of(mv) else {
            return Ok(());
        };
        let list: Vec<Pubkey> = accounts.iter().map(|(k, _)| *k).collect();
        if self.pamm_ready.get(&key) == Some(&list) {
            return Ok(());
        }
        let missing = self.missing_pamm(&accounts).await?;
        if missing.is_empty() {
            self.pamm_ready.insert(key, list);
            return Ok(());
        }
        if self.pamm_prepare_tried.get(&key) == Some(&now) {
            return Ok(());
        }
        self.pamm_prepare_tried.insert(key, now);
        let ixs = graduation::pamm_account_ixs(&self.payer.pubkey(), &mv.market, &missing);
        let result = self.send_exit(&ixs, key, ExtrasSel::NONE, now).await;
        if result.landed().is_some() {
            self.pamm_ready.insert(key, list);
        } else if !matches!(result, ExitResult::DryRun { .. }) {
            let detail = format!("pool accounts: {result:?}");
            self.graduation_alert(
                key,
                &detail,
                Alert::TransitionFailed {
                    market: key,
                    detail: detail.clone(),
                },
            );
        }
        report.pamm_accounts.push((key, result));
        Ok(())
    }

    /// Duty 6 (M6; [`graduation::transition_ixs`]): the `→ PumpSwap` transition of
    /// `view.markets[idx]` — its missing pool accounts, `crank_price` on the curve's ReadSet onto
    /// the pool, `crank_price` on the pool (its first observation) — at most once a second, at
    /// the exit priority; the market is re-read after it lands. Returns whether it landed (the
    /// market's observation this second).
    async fn transition(
        &mut self,
        view: &mut ChainView,
        idx: usize,
        now: i64,
        slot: u64,
        report: &mut TickReport,
    ) -> anyhow::Result<bool> {
        let mv = view.markets[idx].clone();
        let key = mv.key;
        if self.transition_tried.get(&key) == Some(&now) {
            return Ok(false);
        }
        self.transition_tried.insert(key, now);
        let missing = match graduation::pamm_accounts_of(&mv) {
            Ok(a) => self.missing_pamm(&a).await?,
            Err(e) => {
                report
                    .errors
                    .push(format!("{key}: transition accounts: {e:#}"));
                Vec::new()
            }
        };
        let ixs = graduation::transition_ixs(&self.payer.pubkey(), &mv, &missing);
        let result = self.send_exit(&ixs, key, ExtrasSel::NONE, now).await;
        let landed = result.landed().cloned();
        let changes = landed.as_ref().map(venue_changes).unwrap_or_default();
        match &landed {
            Some(out) => {
                self.on_landed(key, out, now, slot, false);
                self.metrics.market(key).transitions += 1;
            }
            None if !matches!(result, ExitResult::DryRun { .. }) => {
                let detail = format!("transition: {result:?}");
                self.graduation_alert(
                    key,
                    &detail,
                    Alert::TransitionFailed {
                        market: key,
                        detail: detail.clone(),
                    },
                );
            }
            None => {}
        }
        report.transitions.push(TransitionRecord {
            market: key,
            created: missing.len(),
            bundled: false,
            venue_changes: changes,
            result,
        });
        if landed.is_some() {
            let fresh = view::load(&self.chain, &[key]).await?;
            if let Some(m) = fresh.markets.into_iter().next() {
                view.markets[idx] = m;
            }
        }
        Ok(landed.is_some())
    }

    /// Duties 6 and 13 ([`crate::sweep`]): right after a full wipe (the program's post-wipe bits
    /// still clear) the venue cashback claims the trader needs and `sweep_trader`, at most once a
    /// second; every `sweep.check_interval_s` the trader side is read, and `sweep_trader` goes out
    /// when it holds anything, with the venue cashback claims once the claimable cashback reaches
    /// `sweep.cashback_min_lamports` (at most every `sweep.cashback_interval_s`).
    async fn sweep_duties(
        &mut self,
        view: &ChainView,
        idx: usize,
        now: i64,
        report: &mut TickReport,
    ) -> anyhow::Result<()> {
        let cfg = self.cfg.sweep.clone();
        let mv = &view.markets[idx];
        let key = mv.key;
        let m = mv.market.as_ref();
        let post_wipe =
            cfg.post_wipe && sweep::wipe_pending(m) && self.post_wipe_tried.get(&key) != Some(&now);
        let check = cfg.sweep
            && self
                .sweep_checked
                .get(&key)
                .is_none_or(|t| now.saturating_sub(*t) >= cfg.check_interval_s || now < *t);
        if !post_wipe && !check {
            return Ok(());
        }
        if check {
            self.sweep_checked.insert(key, now);
        }
        // M7-SEC-8: the tick's prefetched facts, unless this is a post-wipe sweep or an exit
        // landed on the market this tick (the trader side may have moved).
        let exited = report
            .exits
            .iter()
            .any(|e| e.market == key && e.result.landed().is_some());
        let prefetched = (!post_wipe && !exited)
            .then(|| self.sweep_prefetch.remove(&key))
            .flatten();
        let accs = match prefetched {
            Some(a) => a,
            None => self.chain.accounts(&sweep::fact_keys(m)).await?,
        };
        let facts = SweepFacts::of(m, &accs);
        let payer = self.payer.pubkey();
        let claim_due = facts.claimable() > 0
            && facts.claimable() >= cfg.cashback_min_lamports
            && self
                .cashback_claimed
                .get(&key)
                .is_none_or(|t| now.saturating_sub(*t) >= cfg.cashback_interval_s || now < *t);
        let (kind, ixs) = if post_wipe {
            self.post_wipe_tried.insert(key, now);
            (
                SweepKind::PostWipe,
                sweep::post_wipe_ixs(&payer, &key, m, &facts),
            )
        } else if check && claim_due {
            self.cashback_claimed.insert(key, now);
            (
                SweepKind::Cashback,
                sweep::post_wipe_ixs(&payer, &key, m, &facts),
            )
        } else if check && facts.sweep_due() {
            (SweepKind::Sweep, vec![sweep::sweep_trader_ix(&key, m)])
        } else {
            return Ok(());
        };
        let result = self.send_exit(&ixs, key, ExtrasSel::NONE, now).await;
        match &result {
            ExitResult::Landed { .. } => {
                let mm = self.metrics.market(key);
                mm.sweeps += 1;
                if facts.claimable() > 0 && kind != SweepKind::Sweep {
                    mm.cashback_claims += 1;
                }
            }
            ExitResult::DryRun { .. } | ExitResult::NoOp | ExitResult::Deferred { .. } => {}
            other => {
                let detail = match other {
                    ExitResult::Refused { outcome, .. } | ExitResult::Failed { outcome } => {
                        format!("{:?}", outcome.err)
                    }
                    ExitResult::Error(e) => e.clone(),
                    _ => String::new(),
                };
                self.graduation_alert(
                    key,
                    &format!("sweep {kind:?}: {detail}"),
                    Alert::SweepFailed {
                        market: key,
                        post_wipe: kind == SweepKind::PostWipe,
                        detail,
                    },
                );
            }
        }
        report.sweeps.push(SweepRecord {
            market: key,
            kind,
            facts,
            instructions: ixs.len(),
            result,
        });
        Ok(())
    }

    /// Raises a graduation alert once per (`key`, detail).
    fn graduation_alert(&mut self, key: Pubkey, detail: &str, alert: Alert) {
        if self.graduation_alerted.insert((key, detail.to_string())) {
            self.alerts.push(alert);
        }
    }

    /// Duty 6: pump `migrate_v2` of `view.markets[idx]`'s completed curve, top-level and alone
    /// ([`graduation::migrate_v2_ix`]), at most once a second; the market is re-read after it
    /// lands. Returns whether it landed.
    async fn migrate(
        &mut self,
        view: &mut ChainView,
        idx: usize,
        now: i64,
        report: &mut TickReport,
    ) -> anyhow::Result<bool> {
        let mv = view.markets[idx].clone();
        let key = mv.key;
        if self.migrate_tried.get(&key) == Some(&now) {
            return Ok(false);
        }
        self.migrate_tried.insert(key, now);
        let result =
            match graduation::migrate_v2_ix(&self.payer.pubkey(), &mv.market, &mv.read_accounts) {
                Ok(ix) => self.send_exit(&[ix], key, ExtrasSel::NONE, now).await,
                Err(e) => ExitResult::Error(format!("{e:#}")),
            };
        let landed = result.landed().is_some();
        match &result {
            ExitResult::Landed { .. } => self.metrics.market(key).migrations += 1,
            ExitResult::DryRun { .. } | ExitResult::NoOp | ExitResult::Deferred { .. } => {}
            other => {
                let detail = match other {
                    ExitResult::Refused { outcome, .. } | ExitResult::Failed { outcome } => {
                        format!("{:?}", outcome.err)
                    }
                    ExitResult::Error(e) => e.clone(),
                    _ => String::new(),
                };
                self.graduation_alert(
                    key,
                    &format!("migrate: {detail}"),
                    Alert::MigrateFailed {
                        market: key,
                        detail: detail.clone(),
                    },
                );
            }
        }
        report.migrations.push(MigrateRecord {
            market: key,
            result,
        });
        if landed {
            let fresh = view::load(&self.chain, &[key]).await?;
            if let Some(m) = fresh.markets.into_iter().next() {
                view.markets[idx] = m;
            }
        }
        Ok(landed)
    }

    /// Duty 6: the straddle bundle for `pv` ([`graduation::plan_straddle`]; ARCH §5.4), at most
    /// once a second per market. `Some(with_tx3)` when the bundle landed (`with_tx3`: it carried
    /// the transition onto the pool, which is the market's observation this second — booked
    /// here); `None` when it was not sent or did not land — the caller then calls `sol_settle`
    /// for the position.
    async fn straddle(
        &mut self,
        mv: &MarketView,
        pv: &PositionView,
        now: i64,
        slot: u64,
        report: &mut TickReport,
    ) -> Option<bool> {
        let g = self.cfg.graduation.clone();
        if !g.straddle || self.straddle_tried.get(&mv.key) == Some(&now) {
            return None;
        }
        self.straddle_tried.insert(mv.key, now);
        let keeper = self.payer.pubkey();
        // tx3's pool accounts: the ones missing now (the bundle creates the pool, not these).
        let missing = match graduation::pamm_accounts_of(mv) {
            Ok(a) => self.missing_pamm(&a).await.ok(),
            Err(_) => None,
        };
        let created = missing.as_ref().map_or(0, Vec::len);
        let (plan, result) = match graduation::plan_straddle(
            &keeper,
            mv,
            pv,
            g.complete_slippage_bps,
            missing.as_deref(),
        ) {
            Ok(plan) => {
                let result = self.run_straddle(mv, &plan).await;
                (Some(plan), result)
            }
            Err(e) => (None, StraddleResult::Error(format!("{e:#}"))),
        };
        let landed = match &result {
            StraddleResult::Landed { outcome, with_tx3 } => {
                self.metrics.market(mv.key).straddles += 1;
                if *with_tx3 && let Some(tx3) = outcome.txs.get(2) {
                    // tx3 carried the transition and the pool's first observation.
                    self.on_landed(mv.key, tx3, now, slot, false);
                    self.metrics.market(mv.key).transitions += 1;
                    report.transitions.push(TransitionRecord {
                        market: mv.key,
                        created,
                        bundled: true,
                        venue_changes: venue_changes(tx3),
                        result: ExitResult::Landed {
                            outcome: tx3.clone(),
                            format: shortwin_services_common::tx::TxFormat::V0,
                            extras: ExtrasSel::NONE,
                            unit_limit: 0,
                            bytes: 0,
                        },
                    });
                }
                Some(*with_tx3)
            }
            _ => None,
        };
        let reason = match &result {
            StraddleResult::Waiting { reason } => Some(reason.clone()),
            StraddleResult::Refused { tx, outcome } => Some(format!(
                "tx{} refused in pre-flight: {:?}",
                tx + 1,
                outcome.err
            )),
            StraddleResult::Failed { outcome } => {
                Some(format!("bundle did not land: {}", outcome.detail))
            }
            StraddleResult::Error(e) => Some(e.clone()),
            StraddleResult::Landed { .. } | StraddleResult::DryRun { .. } => None,
        };
        if let Some(reason) = reason {
            self.graduation_alert(
                pv.key,
                &reason,
                Alert::StraddleWaiting {
                    market: mv.key,
                    position: pv.key,
                    reason: reason.clone(),
                },
            );
        }
        report.straddles.push(StraddleRecord {
            market: mv.key,
            position: pv.key,
            rt_read: plan.as_ref().map_or(0, |p| p.rt_read),
            quote: plan.as_ref().map_or(0, |p| p.quote),
            max_sol_cost: plan.as_ref().map_or(0, |p| p.max_sol_cost),
            txs: match &result {
                StraddleResult::Landed { outcome, .. } | StraddleResult::Failed { outcome } => {
                    outcome.txs.len()
                }
                _ => 0,
            },
            result,
        });
        landed
    }

    /// Builds, pre-flights and sends `plan`'s bundle (v0 transactions, the keeper paying and
    /// signing; the tip on the last one): `simulate_bundle` where the chain can, else tx1 alone
    /// (tx2 cannot run before tx1 has completed the curve) with `migrate_v2`'s fixed limit and
    /// tx3's; when tx3 is missing or its pre-flight fails the bundle goes out as tx1 + tx2 if
    /// `send_without_exit` (V1-9's fallback: tx3 holds no exit, the next second's crank makes the
    /// transition), else it waits.
    async fn run_straddle(&mut self, mv: &MarketView, plan: &StraddlePlan) -> StraddleResult {
        let g = self.cfg.graduation.clone();
        if plan.quote > g.max_complete_lamports {
            return StraddleResult::Waiting {
                reason: format!(
                    "completing the curve costs {} lamports, above max_complete_lamports {}",
                    plan.quote, g.max_complete_lamports
                ),
            };
        }
        let mut with_tx3 = plan.tx3.is_some();
        if !with_tx3 && !g.send_without_exit {
            return StraddleResult::Waiting {
                reason: "no transition transaction (tx3) could be built; \
                         graduation.send_without_exit is off"
                    .into(),
            };
        }
        let tips = match g.tip_keys() {
            Ok(t) if !t.is_empty() => t,
            _ => return StraddleResult::Error("no tip account".into()),
        };
        let tip_to = tips[self.tip_next % tips.len()];
        self.tip_next = self.tip_next.wrapping_add(1);
        let price = self.priority_price(&[mv.key], true).await;
        loop {
            let mut txs: Vec<Vec<Instruction>> = vec![plan.tx1.clone(), plan.tx2.clone()];
            if with_tx3 && let Some(t3) = &plan.tx3 {
                txs.push(t3.clone());
            }
            if let Some(last) = txs.last_mut() {
                last.push(graduation::tip_ix(
                    &self.payer.pubkey(),
                    &tip_to,
                    g.tip_lamports,
                ));
            }
            let bh = match self.chain.latest_blockhash().await {
                Ok(h) => h,
                Err(e) => return StraddleResult::Error(format!("{e:#}")),
            };
            let build = |ixs: &[Instruction], limit: u32| {
                tx::build(
                    TxFormat::V0,
                    &self.payer,
                    &[],
                    ixs,
                    &ComputeBudget {
                        unit_limit: limit,
                        unit_price: price,
                    },
                    bh,
                    &self.lookup_tables,
                )
            };
            let sims = match txs
                .iter()
                .map(|ixs| build(ixs, tx::MAX_COMPUTE_UNITS))
                .collect::<anyhow::Result<Vec<_>>>()
            {
                Ok(v) => v,
                Err(e) => return StraddleResult::Error(format!("{e:#}")),
            };
            if let Some(i) = sims.iter().position(|t| !tx::fits(TxFormat::V0, t)) {
                return StraddleResult::Error(format!("bundle tx{} too large", i + 1));
            }
            // Pre-flight.
            self.metrics.simulations += 1;
            let mut pre: Vec<TxOutcome> = Vec::new();
            let limits: Vec<u32> = match self.chain.simulate_bundle(&sims).await {
                Ok(outs) => {
                    if let Some((i, o)) = outs.iter().enumerate().find(|(_, o)| !o.ok()) {
                        self.metrics.simulation_failures += 1;
                        if i == 2 && with_tx3 && g.send_without_exit {
                            // V1-9 (module docs of `graduation`): tx3 would fail; complete and
                            // migrate only, the next second's crank makes the transition.
                            with_tx3 = false;
                            continue;
                        }
                        return StraddleResult::Refused {
                            tx: i,
                            outcome: o.clone(),
                        };
                    }
                    if outs.len() < sims.len() {
                        return StraddleResult::Error("the bundle simulation stopped early".into());
                    }
                    let l = outs
                        .iter()
                        .map(|o| tx::cu_limit(o.units_consumed))
                        .collect();
                    pre = outs;
                    l
                }
                Err(_) => match self.chain.simulate(&sims[0]).await {
                    Ok(o) if o.ok() => {
                        let mut l = vec![tx::cu_limit(o.units_consumed), graduation::MIGRATE_CU];
                        pre.push(o);
                        if with_tx3 {
                            l.push(graduation::UNSIMULATED_CU);
                        }
                        l
                    }
                    Ok(o) => {
                        self.metrics.simulation_failures += 1;
                        return StraddleResult::Refused { tx: 0, outcome: o };
                    }
                    Err(e) => return StraddleResult::Error(format!("{e:#}")),
                },
            };
            if self.cfg.dry_run {
                return StraddleResult::DryRun { outcomes: pre };
            }
            let signed = match txs
                .iter()
                .zip(&limits)
                .map(|(ixs, l)| build(ixs, *l))
                .collect::<anyhow::Result<Vec<_>>>()
            {
                Ok(v) => v,
                Err(e) => return StraddleResult::Error(format!("{e:#}")),
            };
            return match self.chain.send_bundle(&signed).await {
                Ok(outcome) if outcome.landed => {
                    self.metrics.tx_landed += outcome.txs.len() as u64;
                    StraddleResult::Landed { outcome, with_tx3 }
                }
                Ok(outcome) => {
                    self.metrics.tx_failed += 1;
                    StraddleResult::Failed { outcome }
                }
                Err(e) => StraddleResult::Waiting {
                    reason: format!("{e:#}"),
                },
            };
        }
    }

    /// Books an exit attempt; returns whether it landed (an observation of the market).
    #[allow(clippy::too_many_arguments)]
    fn account_exit(
        &mut self,
        mv: &MarketView,
        pv: &PositionView,
        kind: ExitKind,
        plan: Plan,
        result: ExitResult,
        now: i64,
        slot: u64,
        report: &mut TickReport,
    ) -> bool {
        let key = mv.key;
        let mut landed = false;
        match &result {
            ExitResult::Landed {
                outcome,
                unit_limit,
                ..
            } => {
                landed = true;
                *report.exit_cu.entry(key).or_default() += u64::from(*unit_limit);
                let settled = !events::of::<SolSettle>(outcome).is_empty();
                self.on_landed(key, outcome, now, slot, false);
                let m = self.metrics.market(key);
                match kind {
                    ExitKind::Liquidate => m.liquidations += 1,
                    ExitKind::Expire => m.expiries += 1,
                    ExitKind::SolSettle => {
                        m.settle_calls += 1;
                        if settled {
                            m.settlements += 1;
                        }
                    }
                }
                if kind == ExitKind::SolSettle {
                    self.tracks
                        .entry(key)
                        .or_default()
                        .settle_sent
                        .insert(pv.key, now);
                }
            }
            ExitResult::Refused { code, outcome } => {
                if !code.is_some_and(benign) {
                    self.alerts.push(Alert::ExitRefused {
                        market: key,
                        position: pv.key,
                        code: *code,
                        detail: format!("{:?}", outcome.err),
                    });
                }
            }
            ExitResult::Failed { outcome } => {
                self.alerts.push(Alert::ExitRefused {
                    market: key,
                    position: pv.key,
                    code: outcome.err.as_ref().and_then(TxError::custom_code),
                    detail: format!("failed on chain: {:?}", outcome.err),
                });
            }
            ExitResult::Error(e) => report.errors.push(format!("{key}/{}: {e}", pv.key)),
            ExitResult::DryRun { .. } | ExitResult::NoOp | ExitResult::Deferred { .. } => {}
        }
        report.exits.push(ExitRecord {
            market: key,
            position: pv.key,
            kind,
            plan,
            result,
        });
        landed
    }

    /// Books a landed transaction on `market`: coverage, spend, `CoverageGap` events.
    fn on_landed(&mut self, market: Pubkey, out: &TxOutcome, now: i64, slot: u64, crank: bool) {
        let track = self.tracks.entry(market).or_default();
        track.landed(now, slot);
        if crank {
            track.spend(now, out.fee);
        }
        let m = self.metrics.market(market);
        m.cover(now);
        m.observations += 1;
        m.cu = m.cu.saturating_add(out.units_consumed);
        m.lamports = m.lamports.saturating_add(out.fee);
        self.note_gaps(out, now);
    }

    /// `CoverageGap` events in the keeper's own transaction (A69): degraded mode, unless the gap
    /// is landing jitter of the idle cadence — at most `idle_interval_s + gap_tolerance_s`
    /// (M7-SEC-1 (a); [`crate::config::CoverageConfig::gap_degrades_above`]).
    fn note_gaps(&mut self, out: &TxOutcome, now: i64) {
        let above = self.cfg.coverage.gap_degrades_above();
        for gap in events::logged::<CoverageGap>(out) {
            let m = self.metrics.market(gap.market);
            m.coverage_gaps += 1;
            if gap.gap_s <= above {
                m.coverage_gaps_tolerated += 1;
                continue;
            }
            let track = self.tracks.entry(gap.market).or_default();
            track.on_coverage_gap(
                gap.market,
                now,
                gap.gap_s,
                gap.open_positions,
                &mut self.alerts,
            );
        }
    }

    /// Simulates and sends one exit (module docs); trading exits try the extras sets in order.
    async fn attempt(
        &mut self,
        mv: &MarketView,
        pv: &PositionView,
        kind: ExitKind,
        plan: Plan,
        now: i64,
    ) -> ExitResult {
        let keeper = self.payer.pubkey();
        match kind {
            ExitKind::SolSettle => {
                let ix = ixs::sol_settle(&keeper, mv, pv);
                if plan == Plan::S3Probe {
                    // Probe: send only a settlement.
                    let price = self.priority_price(&[mv.key], true).await;
                    match self.simulate(std::slice::from_ref(&ix), now, price).await {
                        Ok((sim, _, _))
                            if sim.err.is_none() && events::of::<SolSettle>(&sim).is_empty() =>
                        {
                            return ExitResult::NoOp;
                        }
                        Ok((sim, _, _)) if sim.err.is_some() => {
                            return ExitResult::Refused {
                                code: sim.err.as_ref().and_then(TxError::custom_code),
                                outcome: sim,
                            };
                        }
                        Ok(_) => {}
                        Err(e) => return ExitResult::Error(format!("{e:#}")),
                    }
                }
                self.send_exit(&[ix], mv.key, ExtrasSel::NONE, now).await
            }
            ExitKind::Liquidate | ExitKind::Expire => {
                let choices = venue::extras_candidates(
                    mv.kind,
                    EXTRAS_IX_BUY,
                    &mv.market,
                    mv.extras.as_deref(),
                );
                let mut last = ExitResult::Error("no extras set to try".into());
                // M6-SEC-4: a coin creator changed since the market's last crank needs its vault
                // ATA (T11) to exist; the exit creates it idempotently in its own transaction.
                let prep =
                    venue::pamm_creator_vault_prep(&keeper, &mv.market, mv.kind, &mv.read_accounts);
                for c in choices {
                    let built = match kind {
                        ExitKind::Liquidate => ixs::liquidate(&keeper, mv, pv, &c),
                        _ => ixs::expire(&keeper, mv, pv, &c),
                    };
                    let ix = match built {
                        Ok(ix) => ix,
                        Err(e) => return ExitResult::Error(format!("{e:#}")),
                    };
                    let mut exit: Vec<Instruction> = prep.iter().cloned().collect();
                    exit.push(ix);
                    let r = self.send_exit(&exit, mv.key, c.sel, now).await;
                    // Another extras set is worth trying after a venue-side failure or a
                    // refusal of the set itself: `BadExtras`, `BadRemainingAccounts` and
                    // `DeniedAccount` (raised only by `venues::parse_extras`; since M10 batch 3
                    // also for a set naming the adopted pool's accounts or the borrower, so a
                    // hostile `current` must not stall the exit — M2-SEC-2). The program's other
                    // refusals do not depend on the set.
                    let retry = match &r {
                        ExitResult::Refused { outcome, .. } => {
                            let code = outcome.err.as_ref().and_then(TxError::custom_code);
                            events::failing_program(outcome).is_some_and(|p| p != shortwin::ID)
                                || is(code, ShortwinError::BadExtras)
                                || is(code, ShortwinError::BadRemainingAccounts)
                                || is(code, ShortwinError::DeniedAccount)
                        }
                        _ => false,
                    };
                    last = r;
                    if !retry {
                        break;
                    }
                }
                last
            }
        }
    }

    /// T-CASCADE: the compute units the keeper may still request on exits of `market` in
    /// `slot` — `budget.market_cu_per_slot` less its landed exits' limits, and
    /// `budget.payer_cu_per_slot` less all of the slot's exits' limits, whichever is less; `None`:
    /// no cap, or nothing sent yet this slot under either (the first exit always goes, whatever
    /// its size).
    fn exit_room(&self, market: &Pubkey, slot: u64) -> Option<u64> {
        let used = |k: &Pubkey| {
            self.slot_cu
                .get(k)
                .filter(|(s, _)| *s == slot)
                .map_or(0, |(_, c)| *c)
        };
        let payer = self.payer.pubkey();
        [
            (self.cfg.budget.market_cu_per_slot, used(market)),
            (self.cfg.budget.payer_cu_per_slot, used(&payer)),
        ]
        .into_iter()
        .filter(|(cap, used)| *cap > 0 && *used > 0)
        .map(|(cap, used)| cap.saturating_sub(used))
        .min()
    }

    /// Books `unit_limit` requested by a landed exit of `market` in `slot`, on the market and on
    /// the fee payer (T-CASCADE).
    fn book_exit_cu(&mut self, market: Pubkey, slot: u64, unit_limit: u32) {
        for k in [market, self.payer.pubkey()] {
            let e = self.slot_cu.entry(k).or_insert((slot, 0));
            if e.0 != slot {
                *e = (slot, 0);
            }
            e.1 = e.1.saturating_add(u64::from(unit_limit));
        }
    }

    async fn send_exit(
        &mut self,
        ixs: &[Instruction],
        market: Pubkey,
        extras: ExtrasSel,
        now: i64,
    ) -> ExitResult {
        let slot = self.tick_slot;
        let room = self.exit_room(&market, slot);
        match self
            .send_ixs_within(ixs, &[], &[market], true, now, room)
            .await
        {
            Ok(Sent::Landed {
                out,
                format,
                bytes,
                unit_limit,
                ..
            }) => {
                self.book_exit_cu(market, slot, unit_limit);
                ExitResult::Landed {
                    outcome: out,
                    format,
                    extras,
                    unit_limit,
                    bytes,
                }
            }
            Ok(Sent::Deferred { unit_limit }) => ExitResult::Deferred { unit_limit },
            Ok(Sent::SimFailed { out, .. }) => ExitResult::Refused {
                code: out.err.as_ref().and_then(TxError::custom_code),
                outcome: out,
            },
            Ok(Sent::Failed { out }) => ExitResult::Failed { outcome: out },
            Ok(Sent::DryRun { out }) => ExitResult::DryRun { outcome: out },
            Ok(Sent::TooLarge) => ExitResult::Error("transaction too large".into()),
            Err(e) => ExitResult::Error(format!("{e:#}")),
        }
    }

    /// How many of `ixs` (a prefix) fit one transaction of `format`, with the compute-budget
    /// and priority-price overhead a send carries (duty 7's size packing).
    fn fitting(&self, format: TxFormat, ixs: &[Instruction]) -> usize {
        let budget = ComputeBudget {
            unit_limit: tx::MAX_COMPUTE_UNITS,
            unit_price: 1,
        };
        let alts: &[AddressLookupTableAccount] = match format {
            TxFormat::V0 => &self.lookup_tables,
            TxFormat::V1 => &[],
        };
        let hash = solana_hash::Hash::default();
        (1..=ixs.len())
            .rev()
            .find(|&k| {
                tx::build(format, &self.payer, &[], &ixs[..k], &budget, hash, alts)
                    .is_ok_and(|t| tx::fits(format, &t))
            })
            .unwrap_or(0)
    }

    /// Duty 5's Raydium fast path and duty 12's pool monitor ([`crate::pool_monitor`]) on every
    /// covered Raydium and list-only market (module docs): after the exits, so a market whose
    /// last position the keeper just closed is proposed for in the same tick, and after the
    /// tick's cranks (M4-SEC-7: its RPC work never delays an observation or an exit); the
    /// observation that keeps a failing market's `pool_fail` chain goes out in a batch of its
    /// own right after, and a market adopted this tick is cranked on its new pool from the
    /// next tick on.
    async fn select_pools(
        &mut self,
        view: &mut ChainView,
        report: &mut TickReport,
        crank_queue: &mut Vec<(Pubkey, Due)>,
    ) {
        let covered: Vec<Pubkey> = view
            .markets
            .iter()
            .filter(|m| m.kind.is_raydium())
            .map(|m| m.key)
            .chain(view.list_only.iter().map(|l| l.key))
            .collect();
        // M7-SEC-8: at most `discoveries_per_tick` discoveries (four `getProgramAccounts` calls
        // each) this tick — the fast cadence first, then markets with open positions, then the
        // longest waiting; the others keep their candidates and are discovered in a later tick.
        let now = view.clock.unix_timestamp;
        let pcfg = self.cfg.pool_select.clone();
        let mut due: Vec<(u8, i64, Pubkey)> = Vec::new();
        for key in &covered {
            let stored = view
                .markets
                .iter()
                .find(|m| m.key == *key)
                .map(|m| (m.market.as_ref(), !m.positions.is_empty()))
                .or_else(|| {
                    view.list_only
                        .iter()
                        .find(|l| l.key == *key)
                        .map(|l| (l.market.as_ref(), false))
                });
            let Some((m, with_positions)) = stored else {
                continue;
            };
            let fast = m.pool_fail_since != 0 || m.fp_t0 != 0;
            let track = self.pool_tracks.get(key);
            if track.is_none_or(|t| t.discovery_due(now, fast, &pcfg)) {
                let rank = if fast {
                    0
                } else if with_positions || m.open_positions > 0 {
                    1
                } else {
                    2
                };
                let last = track.and_then(|t| t.last_discovery).unwrap_or(i64::MIN);
                due.push((rank, last, *key));
            }
        }
        due.sort();
        let may_discover: HashSet<Pubkey> = due
            .iter()
            .take(pcfg.discoveries_per_tick.max(1))
            .map(|(_, _, k)| *k)
            .collect();
        let mut latched: Option<Option<VenueGuard>> = None;
        for key in &covered {
            if let Err(e) = self
                .select_market(
                    view,
                    *key,
                    may_discover.contains(key),
                    &mut latched,
                    report,
                    crank_queue,
                )
                .await
            {
                report.errors.push(format!("{key}: pool selection: {e:#}"));
            }
        }
        // Keep the tracks of the markets not read this tick (cold, M7-SEC-8): their venue is
        // as last seen.
        let read: HashSet<Pubkey> = view.markets.iter().map(|m| m.key).collect();
        let keep: HashSet<Pubkey> = covered
            .into_iter()
            .chain(self.markets.iter().copied().filter(|k| !read.contains(k)))
            .collect();
        self.pool_tracks.retain(|k, _| keep.contains(k));
    }

    /// Whether the `VenueGuard` latch holds an upgrade of the program of a candidate on `kind`
    /// (`VenueUpgraded`: no proposal of it until `revalidate_venue`; M4-SEC-8: the candidate's
    /// own program only); the guard is read at most once a tick. An unreadable guard counts as
    /// clear (the simulation decides).
    async fn candidate_latched(
        &self,
        kind: VenueKind,
        cache: &mut Option<Option<VenueGuard>>,
    ) -> bool {
        if cache.is_none() {
            let g = self
                .chain
                .accounts(&[pda::venue_guard()])
                .await
                .ok()
                .and_then(|v| v.into_iter().next().flatten())
                .and_then(|a| view::decode_zc::<VenueGuard>(&a));
            *cache = Some(g);
        }
        cache
            .as_ref()
            .and_then(|g| g.as_ref())
            .is_some_and(|g| guard::candidate_latched(g, kind))
    }

    /// Reports what changed on `market` since the keeper last saw it and was not its own doing:
    /// another pool adopted, a collection window opened or closed (someone else's
    /// `propose_pool` / `execute_pool_switch`).
    fn note_pool_changes(&mut self, key: Pubkey, market: &Market, fast_window_s: u32) {
        let seen = Seen::of(market);
        let track = self.pool_tracks.entry(key).or_default();
        let Some(before) = track.seen.replace(seen) else {
            return;
        };
        if before.pool != seen.pool {
            self.metrics.market(key).pool_switches += 1;
            self.alerts.push(Alert::PoolSwitched {
                market: key,
                from: before.pool,
                to: seen.pool,
                venue: market.venue_kind().unwrap_or(VenueKind::None),
                kind: None,
                d_inc: before.pool_d,
                d_cand: seen.pool_d,
                by_keeper: false,
            });
        } else if before.fp_t0 == 0 && seen.fp_t0 != 0 {
            self.alerts.push(Alert::PoolWindow {
                market: key,
                candidate: seen.fp_pool,
                d_cand: market.fp_d.get(),
                window_ends: pool_select::window_ends(seen.fp_t0, fast_window_s).unwrap_or(0),
                improved: false,
                by_keeper: false,
            });
        } else if before.fp_t0 != 0 && seen.fp_t0 == 0 {
            self.alerts.push(Alert::PoolSwitchCancelled {
                market: key,
                candidate: before.fp_pool,
                by_keeper: false,
            });
        }
    }

    /// One market's pool selection this tick ([`Self::select_pools`]): discovery and evaluation
    /// on their cadence, the plan, and at most one action (two when a resolved window is
    /// followed by a new proposal in the same tick).
    async fn select_market(
        &mut self,
        view: &mut ChainView,
        key: Pubkey,
        may_discover: bool,
        latched: &mut Option<Option<VenueGuard>>,
        report: &mut TickReport,
        crank_queue: &mut Vec<(Pubkey, Due)>,
    ) -> anyhow::Result<()> {
        let clock = view.clock;
        let now = clock.unix_timestamp;
        let sp = SelectParams::of(&view.config.params);
        let pcfg = self.cfg.pool_select.clone();
        for round in 0..2 {
            // The market as the next instruction sees it: a Raydium market's prologue projected
            // to the clock (`pool_d` refreshed, the `pool_fail` chain stepped); a list-only
            // market as stored.
            let (market, s, mv): (Box<Market>, Situation, Option<MarketView>) =
                // (A list-only market is in `view.markets` too, with its detection set as ReadSet:
                // the list-only branch below is its pool-selection view.)
                match view
                    .markets
                    .iter()
                    .find(|m| m.key == key && m.kind != VenueKind::None)
                {
                    Some(mv) if mv.kind.is_raydium() => {
                        let proj = decide::project(&view.config, mv, clock)?;
                        let s = Situation::of(&proj.market, mv.kind, proj.now, proj.pool_fail);
                        (proj.market, s, Some(mv.clone()))
                    }
                    Some(_) => return Ok(()),
                    None => match view.list_only.iter().find(|l| l.key == key) {
                        Some(l) => {
                            let t = prologue::market_now(&l.market, now);
                            let s = Situation::of(&l.market, VenueKind::None, t, false);
                            (l.market.clone(), s, None)
                        }
                        None => return Ok(()),
                    },
                };
            if round == 0 {
                self.note_pool_changes(key, &market, sp.fast_window_s);
            }

            // Discovery (the mint's Raydium SOL pools), then their ranking within this tick's
            // key budget (M4-SEC-7): only the best `max_candidates` are evaluated.
            let fast = s.pool_fail || s.fp_t0 != 0;
            let discover = may_discover
                && self
                    .pool_tracks
                    .get(&key)
                    .is_none_or(|t| t.discovery_due(now, fast, &pcfg));
            if discover {
                let found = pool_monitor::discover(self.discovery_chain(), &market.mint).await;
                self.metrics.pool_discoveries += 1;
                let t = self.pool_tracks.entry(key).or_default();
                t.last_discovery = Some(now);
                match found {
                    Ok(c) => t.start_ranking(c),
                    Err(e) => report.errors.push(format!("{key}: pool discovery: {e:#}")),
                }
            }
            let t = self.pool_tracks.entry(key).or_default();
            if round == 0 && !t.rank_queue.is_empty() {
                let batch = t.next_rank_batch(pcfg.rank_keys_per_tick);
                let keys: Vec<Pubkey> = batch.iter().flat_map(|c| c.rank_keys()).collect();
                let accs = self.chain.accounts(&keys).await?;
                let scored: Vec<(u128, pool_monitor::CandidatePool)> = batch
                    .into_iter()
                    .zip(accs.chunks(2))
                    .map(|(c, a)| (pool_monitor::score(&c, a), c))
                    .collect();
                let t = self.pool_tracks.entry(key).or_default();
                if t.finish_rank_batch(scored, pcfg.max_candidates) {
                    t.last_eval = None;
                }
            }

            // Evaluation, on its cadence (every tick while it matters).
            let track = self.pool_tracks.entry(key).or_default();
            let urgent = pool_monitor::urgent(&s, &sp, track);
            let window_due = s.fp_t0 != 0
                && pool_select::window_ends(s.fp_t0, sp.fast_window_s).is_ok_and(|e| s.now >= e);
            if !track.eval_due(now, urgent, &pcfg) && !window_due {
                if track.keep_chain {
                    let every = self.cfg.coverage.no_route_interval_s;
                    keep_pool_fail_chain(key, mv.as_ref(), now, every, report, crank_queue);
                }
                return Ok(());
            }
            let cands = track.candidates.clone();
            let list_only = s.venue == VenueKind::None;
            let mut keys: Vec<Pubkey> = cands
                .iter()
                .flat_map(|c| c.read_keys.iter().copied())
                .collect();
            // D1's detection set, only when there is a candidate to check it for.
            if list_only && !cands.is_empty() {
                keys.extend(ixs::detection_set(&market));
            }
            let accs = if keys.is_empty() {
                Vec::new()
            } else {
                self.chain.accounts(&keys).await?
            };
            let mut at = 0usize;
            let mut evaluated = Vec::with_capacity(cands.len());
            // Switch only if all fit: the program's check on the market as the next instruction
            // sees it, with the cached oracle (the tier the market would have on a candidate).
            let fit = FitContext {
                params: &view.config.params,
                tiers: &view.config.tiers,
                oracle: view
                    .oracle
                    .as_ref()
                    .map_or(OraclePrice::default(), |o| OraclePrice {
                        price: o.price,
                        conf: o.conf,
                        expo: o.expo,
                        publish_time: o.publish_time,
                    }),
                now: s.now,
            };
            for c in &cands {
                let n = c.read_keys.len();
                let slice = accs.get(at..at.saturating_add(n)).unwrap_or(&[]);
                at = at.saturating_add(n);
                evaluated.push(pool_monitor::evaluate(&market, &fit, now, c, slice));
            }
            // M4-SEC-8 / M4V-4: the program refuses a proposal of a candidate whose own program
            // is latched, so the plan ranks only the others — after a latched window candidate is
            // cancelled, the best pool on the other Raydium program opens the next window.
            let mut any_latched = false;
            for e in &mut evaluated {
                if e.d().is_some() && self.candidate_latched(e.kind, latched).await {
                    any_latched = true;
                    e.verdict = pool_monitor::Verdict::Refused {
                        code: Some(u32::from(ShortwinError::VenueUpgraded)),
                        detail: "the candidate's program awaits the venue review".into(),
                    };
                }
            }
            let mut plan = pool_monitor::plan(&s, &sp, &market, &evaluated);
            if plan.action.is_none() && plan.blocked.is_none() && any_latched {
                plan.blocked = Some(u32::from(ShortwinError::VenueUpgraded));
            }
            if list_only && !cands.is_empty() {
                // D1: a pump coin keeps its pump venue (`crank_price`'s re-detection, M5).
                let det = accs.get(at..at.saturating_add(2)).unwrap_or(&[]);
                let refused = match pool_monitor::classify(&market, det) {
                    Ok(class) if class.pump_venue() => {
                        Some(Some(u32::from(ShortwinError::PumpVenuePrecedence)))
                    }
                    Ok(_) => None,
                    Err(code) => Some(code),
                };
                if let Some(code) = refused {
                    plan.action = None;
                    plan.blocked = code;
                }
            }
            // The latch of the candidate's own program (M4-SEC-8). An execution is sent anyway:
            // the program cancels a window whose candidate's program is latched (M4V-4), and a
            // proposal on the other program can follow in the same tick.
            if let Some(Action::Propose { kind, .. }) = plan.action
                && self.candidate_latched(kind, latched).await
            {
                plan.action = None;
                plan.blocked = Some(u32::from(ShortwinError::VenueUpgraded));
            }
            let track = self.pool_tracks.entry(key).or_default();
            track.evaluated = evaluated;
            track.last_eval = Some(now);
            track.last_pool_fail = Some(s.pool_fail);
            track.keep_chain = plan.keep_chain;
            let pinned = if plan.pinned {
                plan.best.map(|b| b.0)
            } else {
                None
            };
            if let Some(since) = track.note_pinned(pinned, now, &pcfg)
                && let Some((candidate, d_cand)) = plan.best
            {
                self.alerts.push(Alert::PoolPinned {
                    market: key,
                    incumbent: pool_monitor::incumbent_pool(&market),
                    candidate,
                    d_inc: s.pool_d,
                    d_cand,
                    open_positions: s.open_positions,
                    since,
                });
            }
            if plan.keep_chain {
                let every = self.cfg.coverage.no_route_interval_s;
                keep_pool_fail_chain(key, mv.as_ref(), now, every, report, crank_queue);
            }
            report.pool_plans.insert(key, plan.clone());

            // Act: at most once a second per pool; a refused proposal backs off.
            let Some(action) = plan.action else {
                return Ok(());
            };
            let track = self.pool_tracks.entry(key).or_default();
            if track.tried == Some((now, action.pool()))
                || (matches!(action, Action::Propose { .. }) && now < track.backoff_until)
            {
                return Ok(());
            }
            track.tried = Some((now, action.pool()));
            let ix = match &action {
                Action::Execute { candidate, .. } => match &mv {
                    Some(mv) => ixs::execute_pool_switch(mv, candidate),
                    // A list-only market's window (positions open, M4-SEC-4).
                    None => ixs::execute_pool_switch_with(
                        &key,
                        &market.mint,
                        &ixs::detection_set(&market),
                        candidate,
                    ),
                },
                Action::Propose {
                    kind, candidate, ..
                } => {
                    let prefix: Vec<Pubkey> = match &mv {
                        Some(mv) => mv.read_keys.clone(),
                        None => ixs::detection_set(&market).to_vec(),
                    };
                    ixs::propose_pool(&key, &market.mint, &prefix, *kind, candidate)
                }
            };
            let result = match self.send_ixs(&[ix], &[key], true, now).await {
                Ok(Sent::Landed { out, .. }) => {
                    let events = events::pool_events(&out);
                    PoolResult::Landed {
                        outcome: out,
                        events,
                    }
                }
                Ok(Sent::SimFailed { out, .. }) => PoolResult::Refused {
                    code: out.err.as_ref().and_then(TxError::custom_code),
                    outcome: out,
                },
                Ok(Sent::Failed { out }) => PoolResult::Failed { outcome: out },
                Ok(Sent::DryRun { out }) => PoolResult::DryRun { outcome: out },
                Ok(Sent::TooLarge) => PoolResult::Error("transaction too large".into()),
                Ok(Sent::Deferred { .. }) => PoolResult::Error("deferred".into()),
                Err(e) => PoolResult::Error(format!("{e:#}")),
            };
            let landed = self.account_pool_action(key, &market, &action, &result, now, report);
            report.pool_actions.push(PoolActionRecord {
                market: key,
                action: action.clone(),
                result,
            });
            if !landed {
                return Ok(());
            }
            // Re-read the market: the action moved it (an adoption, a window).
            let fresh = view::load(&self.chain, &[key]).await?;
            view.markets.retain(|m| m.key != key);
            view.list_only.retain(|l| l.key != key);
            let seen = fresh
                .markets
                .iter()
                .map(|m| Seen::of(&m.market))
                .chain(fresh.list_only.iter().map(|l| Seen::of(&l.market)))
                .next();
            view.markets.extend(fresh.markets);
            view.list_only.extend(fresh.list_only);
            let track = self.pool_tracks.entry(key).or_default();
            if seen.is_some() {
                track.seen = seen;
            }
            // A resolved window may be followed by a new proposal in the same tick.
            if !matches!(action, Action::Execute { .. }) {
                return Ok(());
            }
            track.last_eval = None;
        }
        Ok(())
    }

    /// Books a pool-selection attempt (metrics, alerts, back-off); returns whether it landed.
    fn account_pool_action(
        &mut self,
        key: Pubkey,
        before: &Market,
        action: &Action,
        result: &PoolResult,
        now: i64,
        report: &mut TickReport,
    ) -> bool {
        let pool = action.pool();
        let backoff = if action.fast() {
            1
        } else {
            self.cfg.pool_select.eval_interval_s
        };
        let kind = match action {
            Action::Execute { kind, .. } | Action::Propose { kind, .. } => *kind,
        };
        match result {
            PoolResult::Landed { events, .. } => {
                let m = self.metrics.market(key);
                match action {
                    Action::Execute { .. } => m.pool_executions += 1,
                    Action::Propose { .. } => m.pool_proposals += 1,
                }
                for e in events {
                    report.pool_events.push(*e);
                    match e.what {
                        PoolEventKind::Switched => {
                            self.metrics.market(key).pool_switches += 1;
                            self.alerts.push(Alert::PoolSwitched {
                                market: key,
                                from: pool_monitor::incumbent_pool(before),
                                to: e.candidate,
                                venue: kind,
                                kind: Some(e.kind),
                                d_inc: e.d_inc,
                                d_cand: e.d_cand,
                                by_keeper: true,
                            });
                        }
                        PoolEventKind::Proposed => self.alerts.push(Alert::PoolWindow {
                            market: key,
                            candidate: e.candidate,
                            d_cand: e.d_cand,
                            window_ends: e.window_ends,
                            improved: matches!(
                                action,
                                Action::Propose {
                                    expect: Proposal::ImproveWindow,
                                    ..
                                }
                            ),
                            by_keeper: true,
                        }),
                        PoolEventKind::Cancelled => self.alerts.push(Alert::PoolSwitchCancelled {
                            market: key,
                            candidate: e.candidate,
                            by_keeper: true,
                        }),
                    }
                }
                let t = self.pool_tracks.entry(key).or_default();
                t.refusals_alerted.clear();
                t.backoff_until = 0;
                true
            }
            PoolResult::Refused { code, outcome } => {
                self.pool_refused(key, pool, *code, format!("{:?}", outcome.err), now, backoff);
                false
            }
            PoolResult::Failed { outcome } => {
                let code = outcome.err.as_ref().and_then(TxError::custom_code);
                self.pool_refused(
                    key,
                    pool,
                    code,
                    format!("failed on chain: {:?}", outcome.err),
                    now,
                    backoff,
                );
                false
            }
            PoolResult::Error(e) => {
                report.errors.push(format!("{key}: pool selection: {e}"));
                false
            }
            PoolResult::DryRun { .. } => {
                // Simulated only: the same cadence as a refusal, without an alert.
                let t = self.pool_tracks.entry(key).or_default();
                t.backoff_until = now.saturating_add(backoff);
                false
            }
        }
    }

    /// A refused or failed pool-selection attempt: back off; alert once per pool and code unless
    /// the refusal is a race's normal answer ([`benign_pool`]).
    fn pool_refused(
        &mut self,
        key: Pubkey,
        pool: Pubkey,
        code: Option<u32>,
        detail: String,
        now: i64,
        backoff: i64,
    ) {
        let t = self.pool_tracks.entry(key).or_default();
        t.backoff_until = now.saturating_add(backoff);
        if benign_pool(code) || t.refusals_alerted.contains(&(pool, code)) {
            return;
        }
        t.refusals_alerted.push((pool, code));
        self.alerts.push(Alert::PoolSelectRefused {
            market: key,
            pool,
            code,
            detail,
        });
    }

    /// Duty 10, upgrades ([`crate::monitor`]): reads the latch and the watched ProgramData
    /// headers in one call; flags every upgrade the program would accept
    /// (`flag_venue_upgraded`, simulated first, at the exit priority ceiling) and alerts once
    /// per upgrade; Token-2022's changes are alerted only. The same upgrade is attempted at most
    /// once a second (an unflagged one is retried every second); a new deploy slot is attempted
    /// at once.
    async fn watch_programs(&mut self, now: i64, report: &mut TickReport) {
        let watched = monitor::watched_programs();
        let guard_key = pda::venue_guard();
        // One read: the latch whole, the ProgramData accounts sliced to (at least) their header.
        let mut keys = vec![guard_key];
        keys.extend(watched.iter().map(|w| w.program_data));
        let len = monitor::HEADER_LEN.max(VenueGuard::SPACE);
        let accs = match self.chain.account_headers(&keys, len).await {
            Ok(a) => a,
            Err(e) => {
                report
                    .errors
                    .push(format!("venue guard / ProgramData: {e:#}"));
                return;
            }
        };
        let Some(guard) = accs
            .first()
            .and_then(Option::as_ref)
            .and_then(view::decode_zc::<VenueGuard>)
        else {
            report.errors.push("VenueGuard not found".into());
            return;
        };
        let headers = accs.get(1..).unwrap_or(&[]);
        for (w, acc) in watched.iter().zip(headers.iter()) {
            let slot = monitor::deploy_slot(acc.as_ref());
            let prev = self.program_slots.insert(w.program_data, slot);
            let Some(slot) = slot else {
                match prev {
                    // It was readable: a program closed or re-owned is worth a page.
                    Some(Some(_)) => self
                        .alerts
                        .push(Alert::ProgramDataUnreadable { program: w.program }),
                    None => tracing::warn!(program = %w.program, "ProgramData not readable"),
                    Some(None) => {}
                }
                continue;
            };
            let Some(venue) = w.guard else {
                if let Some(Some(p)) = prev
                    && p != slot
                {
                    self.alerts.push(Alert::ProgramUpgraded {
                        program: w.program,
                        previous_slot: p,
                        deploy_slot: slot,
                    });
                }
                continue;
            };
            let action = monitor::latch_action(&guard, venue, slot);
            let i = usize::from(venue);
            let known = match action {
                LatchAction::Nothing => continue,
                LatchAction::Flag => guard.guard_slot.get(i).copied().unwrap_or(0),
                LatchAction::Refresh => guard.flagged_slot.get(i).copied().unwrap_or(0),
            };
            if self.upgrades_alerted.insert((venue, slot)) {
                self.alerts.push(Alert::VenueUpgraded {
                    venue,
                    program: w.program,
                    known_slot: known,
                    deploy_slot: slot,
                });
            }
            if self.flag_tried.get(&(venue, slot)) == Some(&now) {
                continue;
            }
            self.flag_tried.retain(|(v, _), _| *v != venue);
            self.flag_tried.insert((venue, slot), now);
            let ix = match ixs::flag_venue_upgraded(venue) {
                Ok(ix) => ix,
                Err(e) => {
                    report
                        .errors
                        .push(format!("flag_venue_upgraded({venue}): {e:#}"));
                    continue;
                }
            };
            let failure = match self.send_ixs(&[ix], &[guard_key], true, now).await {
                Ok(Sent::Landed { .. }) => {
                    self.metrics.venue_flags += 1;
                    report.flagged.push((venue, slot));
                    self.alerts.push(Alert::VenueUpgradeFlagged {
                        venue,
                        deploy_slot: slot,
                    });
                    None
                }
                // Someone flagged it first (the latch already records this slot).
                Ok(Sent::SimFailed { out, .. })
                    if is(
                        out.err.as_ref().and_then(TxError::custom_code),
                        ShortwinError::VenueNotUpgraded,
                    ) =>
                {
                    None
                }
                Ok(Sent::DryRun { .. }) => None,
                Ok(Sent::SimFailed { out, .. }) | Ok(Sent::Failed { out }) => {
                    Some(format!("{:?}", out.err))
                }
                Ok(Sent::TooLarge) => Some("transaction too large".into()),
                Ok(Sent::Deferred { .. }) => Some("deferred".into()),
                Err(e) => Some(format!("{e:#}")),
            };
            if let Some(detail) = failure
                && self.flag_failures_alerted.insert((venue, slot))
            {
                self.alerts.push(Alert::VenueFlagFailed { venue, detail });
            }
        }
    }

    /// Duty 10, pool admin actions ([`crate::monitor`]): every covered market's pool against
    /// the last tick; each change is an alert.
    fn watch_pools(&mut self, view: &ChainView) {
        let caps = FeeCaps::of(&view.config);
        for mv in &view.markets {
            // The pump curve has no pool admin: its fee tier moves with its market cap and its
            // end is graduation (duty 6), not a lost route. A list-only market has no pool.
            if matches!(
                mv.kind,
                VenueKind::PumpCurve | VenueKind::PumpMigrating | VenueKind::None
            ) {
                continue;
            }
            let snap = monitor::snapshot(mv);
            let pool = mv.read_keys.first().copied().unwrap_or_default();
            let watch = self.pools.entry(mv.key).or_default();
            if watch.pool != Some(pool) {
                // A new pool (an adoption): its first sight, not a change of the old one.
                *watch = PoolWatch {
                    pool: Some(pool),
                    ..PoolWatch::default()
                };
            }
            let changes = watch.observe(mv.kind, snap, caps);
            for change in changes {
                self.metrics.market(mv.key).venue_admin_changes += 1;
                self.alerts.push(Alert::VenueAdmin {
                    market: mv.key,
                    venue: mv.kind,
                    pool,
                    change,
                });
            }
        }
    }

    /// Duty 11 (ARCH §7.1, §4.7; C1-12): every `oracle.interval_s` (≤ 30 s) the keeper reads the
    /// configured Pyth `PriceUpdateV2`; an update newer than the `OracleCache` that the program's
    /// own rules accept (`pyth::check` on the keeper's view: verification, feed, age, confidence,
    /// step) is cranked at once — simulated first, at the observation ceiling, outside every
    /// market's budget (a global line) — and a failed crank is retried every second. A newer
    /// update the rules refuse raises `Alert::OracleRefused` (once per reason until a crank
    /// lands); a cache older than `oracle.aging_alert_s` raises `Alert::OracleAging` (once per
    /// stall — per cache `publish_time`, M7D-6 — and hourly while it lasts).
    async fn oracle_duty(&mut self, view: &ChainView, report: &mut TickReport) {
        use shortwin_venue_state::pyth;
        let now = view.clock.unix_timestamp;
        let feed = view.config.pyth_sol_usd;
        if feed == Pubkey::default() {
            return;
        }
        let p = &view.config.params;
        let cached = view
            .oracle
            .as_ref()
            .map_or_else(pyth::Cached::default, |c| pyth::Cached {
                price: c.price,
                expo: c.expo,
                publish_time: c.publish_time,
            });
        let age = now.saturating_sub(cached.publish_time);
        // M7D-6: once per stall — per cache `publish_time` —, and hourly while one lasts. As
        // reviewed the alert was rate-limited to once an hour across stalls, which held while
        // the cache went stale after 3,600 s (alert at 1,800 s); with `oracle_stale_s` 600
        // (decision C) a second stall within ~50 min of an alert reached Tier C unalerted.
        if cached.publish_time > 0
            && age > self.cfg.oracle.aging_alert_s
            && self.oracle_aging_alerted.is_none_or(|(published, t)| {
                published != cached.publish_time || now.saturating_sub(t) >= 3_600
            })
        {
            self.oracle_aging_alerted = Some((cached.publish_time, now));
            self.alerts.push(Alert::OracleAging {
                age_s: age,
                publish_time: cached.publish_time,
            });
        }
        if now < self.oracle_next {
            return;
        }
        self.oracle_next = now.saturating_add(self.cfg.oracle.interval_s);
        self.metrics.oracle_reads += 1;
        let acc = match self.chain.accounts(&[feed]).await {
            Ok(mut v) => v.pop().flatten(),
            Err(e) => {
                report.errors.push(format!("oracle feed {feed}: {e:#}"));
                self.oracle_next = now.saturating_add(1);
                return;
            }
        };
        let update = match acc
            .as_ref()
            .map(|a| pyth::read(&a.owner.to_bytes(), &a.data))
        {
            Some(Ok(u)) => u,
            other => {
                let detail = match other {
                    None => "missing".to_string(),
                    Some(Err(e)) => format!("{e:?}"),
                    Some(Ok(_)) => unreachable!(),
                };
                if self.oracle_alerted.insert(format!("unreadable {detail}")) {
                    self.alerts.push(Alert::OracleUnreadable {
                        account: feed,
                        detail,
                    });
                }
                return;
            }
        };
        if update.publish_time <= cached.publish_time {
            return;
        }
        let limits = pyth::Limits {
            max_age_s: p.oracle_max_age_s,
            stale_s: p.oracle_stale_s,
            max_conf_bps: p.oracle_max_conf_bps,
            max_step_bps: p.oracle_max_step_bps,
        };
        if let Err(r) = pyth::check(&update, &view.config.pyth_feed_id, &cached, now, &limits) {
            self.metrics.oracle_refusals += 1;
            if self.oracle_alerted.insert(format!("{r:?}")) {
                self.alerts.push(Alert::OracleRefused {
                    publish_time: update.publish_time,
                    code: None,
                    reason: format!("{r:?}"),
                });
            }
            return;
        }
        let ix = ixs::crank_sol_usd(&feed);
        let result = match self
            .send_ixs(&[ix], &[pda::oracle_cache()], false, now)
            .await
        {
            Ok(Sent::Landed {
                out,
                format,
                bytes,
                unit_limit,
                ..
            }) => {
                self.metrics.oracle_cranks += 1;
                self.metrics.oracle_lamports = self.metrics.oracle_lamports.saturating_add(out.fee);
                self.oracle_alerted.clear();
                ExitResult::Landed {
                    outcome: out,
                    format,
                    extras: ExtrasSel::NONE,
                    unit_limit,
                    bytes,
                }
            }
            Ok(Sent::SimFailed { out, .. }) => ExitResult::Refused {
                code: out.err.as_ref().and_then(TxError::custom_code),
                outcome: out,
            },
            Ok(Sent::Failed { out }) => ExitResult::Failed { outcome: out },
            Ok(Sent::DryRun { out }) => ExitResult::DryRun { outcome: out },
            Ok(Sent::TooLarge) => ExitResult::Error("transaction too large".into()),
            Ok(Sent::Deferred { unit_limit }) => ExitResult::Deferred { unit_limit },
            Err(e) => ExitResult::Error(format!("{e:#}")),
        };
        match &result {
            ExitResult::Refused { code, outcome } => {
                self.metrics.oracle_refusals += 1;
                // "Not newer" after a race with another cranker is benign.
                let benign = is(*code, ShortwinError::OracleStale);
                if !benign && self.oracle_alerted.insert(format!("code {code:?}")) {
                    self.alerts.push(Alert::OracleRefused {
                        publish_time: update.publish_time,
                        code: *code,
                        reason: format!("{:?}", outcome.err),
                    });
                }
                if !benign {
                    self.oracle_next = now.saturating_add(1);
                }
            }
            ExitResult::Failed { .. } | ExitResult::Error(_) => {
                self.oracle_next = now.saturating_add(1);
            }
            _ => {}
        }
        report.oracle = Some((update.publish_time, result));
    }

    /// O17's automatic degraded mode (module docs; [`crate::config::CoveragePauseConfig`]): on
    /// every covered market, the `paused_coverage` flag should be set while the market is
    /// degraded or recovering and has open positions (or the flag is already set: it lifts only
    /// with the recovery), and clear otherwise. The designated pauser sends
    /// `set_coverage_pause(market, want)` — simulated first, at the exit priority ceiling,
    /// outside the observation budget, at most once a second per market, retried every second
    /// until the flag matches; others only alert (the `Degraded` alert pages the Emergency
    /// multisig). A market without positions is never paused: its opens wait for its own
    /// warm-up, and nothing rests on its coverage. ★OWNER-M7-3 (b) (2026-10-01): a cleared flag
    /// is set again only on a market this keeper degraded itself
    /// ([`crate::coverage::MarketTrack::own_degraded`]) — a pause it only adopted that the
    /// Emergency multisig cleared stays clear.
    async fn coverage_pauses(&mut self, view: &ChainView, report: &mut TickReport) {
        let now = view.clock.unix_timestamp;
        for mv in &view.markets {
            let key = mv.key;
            let Some(track) = self.tracks.get(&key) else {
                continue;
            };
            let flag = mv.market.has_flag(MARKET_FLAG_PAUSED_COVERAGE);
            self.metrics.market(key).paused_coverage = flag;
            let open = !mv.positions.is_empty() || mv.market.open_positions > 0;
            let mut want = track.state.is_degraded() && (open || flag);
            if want && !flag && !track.own_degraded {
                // Adopted, then cleared by someone else (the Emergency): not set again.
                want = false;
            }
            if want == flag {
                self.pause_alerted.retain(|(k, _)| *k != key);
                self.pause_intent.remove(&key);
                continue;
            }
            if !self.cfg.coverage_pause.enabled {
                continue;
            }
            let unavailable = match &self.coverage_authority {
                None => Some(PauseUnavailable::NoKey),
                Some(_) if view.config.coverage_authority == Pubkey::default() => {
                    Some(PauseUnavailable::AuthorityUnset)
                }
                Some(kp) if kp.pubkey() != view.config.coverage_authority => {
                    Some(PauseUnavailable::WrongKey {
                        configured: view.config.coverage_authority,
                        held: kp.pubkey(),
                    })
                }
                Some(_) => None,
            };
            if let Some(reason) = unavailable {
                if self
                    .pause_alerted
                    .insert((key, format!("{want}: {reason:?}")))
                {
                    self.alerts.push(Alert::CoveragePauseUnavailable {
                        market: key,
                        paused: want,
                        reason,
                    });
                }
                continue;
            }
            if self.pause_tried.get(&key) == Some(&now) {
                continue;
            }
            self.pause_tried.insert(key, now);
            self.pause_intent.insert(key, want);
            let Some(kp) = self.coverage_authority.clone() else {
                continue;
            };
            let ix = ixs::set_coverage_pause(&kp.pubkey(), &key, want);
            let sent = self
                .send_ixs_within(&[ix], &[kp.as_ref()], &[key], true, now, None)
                .await;
            let result = match sent {
                Ok(Sent::Landed {
                    out,
                    format,
                    bytes,
                    unit_limit,
                    ..
                }) => {
                    let m = self.metrics.market(key);
                    m.lamports = m.lamports.saturating_add(out.fee);
                    if want {
                        m.coverage_pauses += 1;
                    } else {
                        m.coverage_unpauses += 1;
                    }
                    m.paused_coverage = want;
                    // The flag is now `want` (M7-SEC-3: a later read that finds it otherwise, or
                    // a position opened meanwhile, was someone else's doing).
                    if let Some(t) = self.tracks.get_mut(&key) {
                        t.flag_seen = Some(want);
                    }
                    self.pause_alerted.retain(|(k, _)| *k != key);
                    self.alerts.push(Alert::CoveragePauseSent {
                        market: key,
                        paused: want,
                    });
                    ExitResult::Landed {
                        outcome: out,
                        format,
                        extras: ExtrasSel::NONE,
                        unit_limit,
                        bytes,
                    }
                }
                Ok(Sent::SimFailed { out, .. }) => ExitResult::Refused {
                    code: out.err.as_ref().and_then(TxError::custom_code),
                    outcome: out,
                },
                Ok(Sent::Failed { out }) => ExitResult::Failed { outcome: out },
                Ok(Sent::DryRun { out }) => ExitResult::DryRun { outcome: out },
                Ok(Sent::TooLarge) => ExitResult::Error("transaction too large".into()),
                Ok(Sent::Deferred { unit_limit }) => ExitResult::Deferred { unit_limit },
                Err(e) => ExitResult::Error(format!("{e:#}")),
            };
            let failure = match &result {
                ExitResult::Refused { code, outcome } => {
                    Some((*code, format!("{:?}", outcome.err)))
                }
                ExitResult::Failed { outcome } => Some((
                    outcome.err.as_ref().and_then(TxError::custom_code),
                    format!("failed on chain: {:?}", outcome.err),
                )),
                ExitResult::Error(e) => Some((None, e.clone())),
                _ => None,
            };
            if let Some((code, detail)) = failure {
                self.metrics.coverage_pause_failures += 1;
                if self
                    .pause_alerted
                    .insert((key, format!("{want}: {code:?}")))
                {
                    self.alerts.push(Alert::CoveragePauseFailed {
                        market: key,
                        paused: want,
                        code,
                        detail,
                    });
                }
            }
            report.coverage_pauses.push(CoveragePauseRecord {
                market: key,
                paused: want,
                result,
            });
        }
    }

    /// Sends the queued cranks in batches (module docs). `charged`: the cranks go through the
    /// global budget (decision A, [`crate::budget`]) — presented exit-critical, baseline,
    /// discretionary (each by rank), then tier 2, then tier 3; a refused charge is not sent and,
    /// on a market with open positions, degrades it at once; an admitted one is booked at its
    /// landed share of the batch's fee, or returned when it does not land. A graduation step's
    /// crank, and pool selection's (`charged` false), are neither charged nor throttled.
    async fn send_cranks(
        &mut self,
        view: &ChainView,
        queue: Vec<(Pubkey, Due)>,
        report: &mut TickReport,
        charged: bool,
    ) {
        let now = view.clock.unix_timestamp;
        let slot = view.clock.slot;
        let params = self.budget_params(&view.config);
        let req_of = |charges: &BTreeMap<Pubkey, ChargeReq>, k: &Pubkey| {
            charges
                .get(k)
                .copied()
                .unwrap_or(ChargeReq::of(ChargeClass::Baseline))
        };
        let mut queue = queue;
        if charged {
            let ranks = self.budget.ranks();
            queue.sort_by_cached_key(|(k, _)| {
                self.budget.order(k, req_of(&self.crank_charges, k), &ranks)
            });
        }
        let mut ready: Vec<(Pubkey, Due)> = Vec::new();
        let mut held: BTreeMap<Pubkey, budget::Reservation> = BTreeMap::new();
        for (k, d) in queue {
            if !charged || self.free_cranks.contains(&k) {
                ready.push((k, d));
                continue;
            }
            let req = req_of(&self.crank_charges, &k);
            match self.budget.admit(now, k, req, &params, &mut self.alerts) {
                Admission::Admitted(r) => {
                    held.insert(k, r);
                    ready.push((k, d));
                }
                Admission::Refused { rank } => {
                    self.refused.insert(k);
                    self.metrics.market(k).budget_refused += 1;
                    // O17: a refused charge on a market with open positions is degraded mode at
                    // once (opens paused; exits, never budgeted, continue).
                    let open = view
                        .market(&k)
                        .is_some_and(|mv| !mv.positions.is_empty() || mv.market.open_positions > 0);
                    if open {
                        let spent = self.budget.spent;
                        let track = self.tracks.entry(k).or_default();
                        let was = track.state.is_degraded();
                        track.on_budget_exhausted(
                            k,
                            now,
                            spent,
                            params.global,
                            rank,
                            &mut self.alerts,
                        );
                        if !was {
                            self.metrics.market(k).budget_degraded += 1;
                        }
                    }
                    report.crank_failed.push((
                        k,
                        format!("refused by the global keeper budget ({req:?}, rank {rank:?})"),
                    ));
                }
            }
        }
        let why: BTreeMap<Pubkey, Due> = ready.iter().copied().collect();
        let mut pending: VecDeque<Pubkey> = ready.iter().map(|(k, _)| *k).collect();
        let mut alone: Vec<Pubkey> = Vec::new();
        // A cap below the format's batch size after a transaction came out too large.
        let mut cap = usize::MAX;
        loop {
            // The batch size of the format in use now (a v1 refusal switches to v0 mid-way).
            let size = match self.format(now) {
                TxFormat::V1 => self.cfg.batch_v1,
                TxFormat::V0 => self.cfg.batch_v0,
            }
            .min(cap)
            .max(1);
            let n = size.min(pending.len());
            if n == 0 {
                break;
            }
            // Cut the batch where its transaction would stop fitting the format (duty 7).
            let candidates: Vec<Instruction> = pending
                .iter()
                .take(n)
                .filter_map(|k| view.market(k).map(ixs::crank_price))
                .collect();
            let n = if candidates.len() == n {
                self.fitting(self.format(now), &candidates).max(1)
            } else {
                n
            };
            let batch: Vec<Pubkey> = pending.drain(..n).collect();
            let ixs: Vec<Instruction> = batch
                .iter()
                .filter_map(|k| view.market(k).map(ixs::crank_price))
                .collect();
            let requeue = |pending: &mut VecDeque<Pubkey>, keys: Vec<Pubkey>| {
                for k in keys.into_iter().rev() {
                    pending.push_front(k);
                }
            };
            match self.send_ixs(&ixs, &batch, false, now).await {
                Ok(Sent::Landed {
                    out,
                    format,
                    bytes,
                    accounts,
                    price,
                    unit_limit,
                }) => {
                    match format {
                        TxFormat::V1 => self.metrics.batches_v1 += 1,
                        TxFormat::V0 => self.metrics.batches_v0 += 1,
                    }
                    let n = u64::try_from(batch.len()).unwrap_or(1).max(1);
                    for k in &batch {
                        let share = TxOutcome {
                            fee: out.fee / n,
                            units_consumed: out.units_consumed / n,
                            ..TxOutcome::default()
                        };
                        if let Some(r) = held.remove(k) {
                            self.budget.settle(r, share.fee);
                        }
                        self.on_landed(*k, &share, now, slot, true);
                        self.metrics.market(*k).cranks += 1;
                        report.cranked.push(*k);
                        report.observed.push((*k, why.get(k).copied()));
                    }
                    self.note_gaps(&out, now);
                    report.batches.push(BatchRecord {
                        markets: batch,
                        format,
                        bytes,
                        accounts,
                        units: out.units_consumed,
                        fee: out.fee,
                        price,
                        unit_limit,
                    });
                }
                Ok(Sent::SimFailed { out, .. }) | Ok(Sent::Failed { out }) if batch.len() == 1 => {
                    report
                        .crank_failed
                        .push((batch[0], format!("{:?}", out.err)));
                }
                Ok(Sent::SimFailed { out, format }) => {
                    // Drop the failing market and crank it alone; batch the rest again.
                    // v0 prepends SetComputeUnitLimit and, with a price, SetComputeUnitPrice.
                    let offset = match format {
                        TxFormat::V1 => 0,
                        TxFormat::V0 => {
                            1 + usize::from(self.priority_price(&batch, false).await > 0)
                        }
                    };
                    match out
                        .err
                        .as_ref()
                        .and_then(TxError::instruction)
                        .and_then(|i| i.checked_sub(offset))
                    {
                        Some(i) if i < batch.len() => {
                            let mut rest = batch;
                            alone.push(rest.remove(i));
                            requeue(&mut pending, rest);
                        }
                        _ => alone.extend(batch),
                    }
                }
                Ok(Sent::Failed { out }) if matches!(out.err, Some(TxError::Dropped(_))) => {
                    // Not landed in time: a miss this second (the price escalates next tick);
                    // resending each market alone would only add load.
                    for k in batch {
                        report.crank_failed.push((k, format!("{:?}", out.err)));
                    }
                }
                Ok(Sent::Failed { .. }) => alone.extend(batch),
                Ok(Sent::TooLarge) => {
                    let size_now = match self.format(now) {
                        TxFormat::V1 => self.cfg.batch_v1,
                        TxFormat::V0 => self.cfg.batch_v0,
                    };
                    if batch.len() == 1 {
                        report
                            .crank_failed
                            .push((batch[0], "transaction too large".into()));
                    } else {
                        if size_now >= batch.len() {
                            // Same format, still too large: halve the batches from now on.
                            cap = batch.len() / 2;
                        }
                        requeue(&mut pending, batch);
                    }
                }
                Ok(Sent::DryRun { .. }) => {}
                Ok(Sent::Deferred { .. }) => {
                    for k in batch {
                        report.crank_failed.push((k, "deferred".into()));
                    }
                }
                Err(e) => {
                    for k in batch {
                        report.crank_failed.push((k, format!("{e:#}")));
                    }
                }
            }
        }
        for k in alone {
            let Some(mv) = view.market(&k) else { continue };
            let ix = ixs::crank_price(mv);
            match self.send_ixs(&[ix], &[k], false, now).await {
                Ok(Sent::Landed {
                    out,
                    format,
                    bytes,
                    accounts,
                    price,
                    unit_limit,
                }) => {
                    if let Some(r) = held.remove(&k) {
                        self.budget.settle(r, out.fee);
                    }
                    self.on_landed(k, &out, now, slot, true);
                    self.metrics.market(k).cranks += 1;
                    report.cranked.push(k);
                    report.observed.push((k, why.get(&k).copied()));
                    report.batches.push(BatchRecord {
                        markets: vec![k],
                        format,
                        bytes,
                        accounts,
                        units: out.units_consumed,
                        fee: out.fee,
                        price,
                        unit_limit,
                    });
                }
                Ok(Sent::SimFailed { out, .. }) | Ok(Sent::Failed { out }) => {
                    report.crank_failed.push((k, format!("{:?}", out.err)));
                }
                Ok(Sent::TooLarge) => report
                    .crank_failed
                    .push((k, "transaction too large".into())),
                Ok(Sent::Deferred { .. }) => report.crank_failed.push((k, "deferred".into())),
                Ok(Sent::DryRun { .. }) => {}
                Err(e) => report.crank_failed.push((k, format!("{e:#}"))),
            }
        }
        // Admitted cranks that did not land: nothing is booked.
        for (_, r) in held {
            self.budget.release(r);
        }
    }
}
