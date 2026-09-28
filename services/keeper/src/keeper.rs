//! The keeper loop (ARCHITECTURE §7.1): one [`Keeper::tick`] per slot.
//!
//! Each tick:
//!
//! 1. reads the clock, re-discovers markets every `discovery_interval_s`, and loads every
//!    covered market ([`crate::view::load`]);
//! 2. per market: records its own spot view and checks the buckets for a foreign deviating
//!    observation (duty 2); projects the prologue at the clock and evaluates every open
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
//! Before the markets, the venue drift monitor (duty 10, [`crate::monitor`]): the venue
//! ProgramData headers against the `VenueGuard` latch — an upgrade is flagged with
//! `flag_venue_upgraded` in the slot the keeper sees it, at the exit priority ceiling — and
//! every covered market's pool and CPMM config against the last tick (Raydium admin actions:
//! route lost or restored, status, open time, fee against the opens and hard caps), as alerts.
//!
//! Batches are **packed by size**: a batch takes up to `batch_v1` / `batch_v0` cranks and is cut
//! where its transaction would exceed the format's size or account limit, so any mix of venue
//! account sets (the mock's 3 ReadSet keys, AMM v4's 4, CPMM's 5) is batched as densely as fits.
//!
//! Every transaction: simulated first, compute-unit limit `⌈simulated × 1.1⌉`, priority price
//! from recent per-account fees, escalated after misses, capped at the O17 ceiling (duty 14).
//! The Jito bundle path of duty 3 is not built yet (see the crate README).

use std::collections::{BTreeMap, HashSet, VecDeque};

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use anyhow::anyhow;
use shortwin::errors::ShortwinError;
use shortwin::events::{CoverageGap, SolSettle};
use shortwin::state::{EXTRAS_IX_BUY, VenueGuard};
use shortwin::venues::ExtrasSel;
use shortwin_services_common::tx::{self, ComputeBudget, TxFormat};
use shortwin_services_common::{Chain, ChainClock, TxError, TxOutcome};
use solana_keypair::Keypair;
use solana_message::AddressLookupTableAccount;
use solana_signer::Signer;

use crate::alerts::Alert;
use crate::config::{KeeperConfig, TxFormatPolicy};
use crate::coverage::{Due, MarketTrack};
use crate::decide::{self, MarketNeeds, Plan, PositionEval};
use crate::events;
use crate::fees;
use crate::ixs;
use crate::metrics::Metrics;
use crate::monitor::{self, FeeCaps, LatchAction, PoolWatch};
use crate::pda;
use crate::venue;
use crate::view::{self, ChainView, MarketView, PositionView};

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
    /// Landed (the transaction's outcome, its format and the extras set used).
    Landed {
        outcome: TxOutcome,
        format: TxFormat,
        extras: ExtrasSel,
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
    /// Foreign deviating observations detected: (market, second).
    pub bursts: Vec<(Pubkey, i64)>,
    /// `flag_venue_upgraded` calls that landed this tick: (venue, deploy slot) (duty 10).
    pub flagged: Vec<(u8, u64)>,
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
            metrics: Metrics::default(),
        })
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
            let t = tx::build(format, &self.payer, &[], ixs, &budget, bh, &alts)?;
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
        for _ in 0..2 {
            let price = self.priority_price(markets, exit).await;
            let (sim, format, bh) = self.simulate(ixs, now, price).await?;
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
            let alts = if format == TxFormat::V0 {
                self.lookup_tables.clone()
            } else {
                Vec::new()
            };
            let t = tx::build(format, &self.payer, &[], ixs, &budget, bh, &alts)?;
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
        self.discover(clock.unix_timestamp).await?;
        let mut view = view::load(&self.chain, &self.markets).await?;
        let now = view.clock.unix_timestamp;
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
        }

        // Duty 10: the latch first (an upgrade is flagged in the slot it is seen), then the
        // markets' pools.
        if self.cfg.venue_watch.flag_upgrades {
            self.watch_programs(now, &mut report).await;
        }
        if self.cfg.venue_watch.pool_admin {
            self.watch_pools(&view);
        }

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
        self.send_cranks(&view, crank_queue, &mut report).await;

        for (key, track) in &self.tracks {
            self.metrics.market(*key).degraded = track.state.is_degraded();
        }
        for a in self.alerts.drain(..) {
            a.log();
            report.alerts.push(a);
        }
        Ok(report)
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
            let track = self.tracks.entry(key).or_default();
            track.record_view(now, mv.state.price());
            if let Some(sec) = track.check_foreign(&mv.market, &cov) {
                report.bursts.push((key, sec));
                self.metrics.market(key).bursts += 1;
            }
        }

        let mut attempted: HashSet<(Pubkey, ExitKind)> = HashSet::new();
        let mut observed = false;
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
            let needs = decide::market_needs(&mv, &proj, &evals, &cov);
            if due_before.is_none() {
                let due = self.tracks[&key].observation_due(&needs, now, slot, &cov);
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
                    Plan::SolSettle | Plan::S3Probe => ExitKind::SolSettle,
                    _ => return None,
                };
                (!attempted.contains(&(e.key, kind))).then_some((e.clone(), kind))
            });
            let Some((ev, kind)) = next else { break };
            attempted.insert((ev.key, kind));
            let Some(pv) = mv.positions.iter().find(|p| p.key == ev.key).cloned() else {
                continue;
            };
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
            let mut chained: Option<ExitKind> = None;
            if let ExitResult::Refused { code, .. } = &result {
                if is(*code, ShortwinError::UseSolSettle) {
                    chained = Some(ExitKind::SolSettle);
                } else if is(*code, ShortwinError::UseExpire) {
                    chained = Some(ExitKind::Expire);
                }
            }
            let mut landed = self.account_exit(&mv, &pv, kind, ev.plan, result, now, slot, report);
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
        } else if let Some(d) = due {
            crank_queue.push((key, d));
        }
        Ok(())
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
            ExitResult::Landed { outcome, .. } => {
                landed = true;
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
            ExitResult::DryRun { .. } | ExitResult::NoOp => {}
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

    fn note_gaps(&mut self, out: &TxOutcome, now: i64) {
        for gap in events::logged::<CoverageGap>(out) {
            self.metrics.market(gap.market).coverage_gaps += 1;
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
                for c in choices {
                    let built = match kind {
                        ExitKind::Liquidate => ixs::liquidate(&keeper, mv, pv, &c),
                        _ => ixs::expire(&keeper, mv, pv, &c),
                    };
                    let ix = match built {
                        Ok(ix) => ix,
                        Err(e) => return ExitResult::Error(format!("{e:#}")),
                    };
                    let r = self.send_exit(&[ix], mv.key, c.sel, now).await;
                    // Only a venue-side failure makes another extras set worth trying; the
                    // program's own refusals do not depend on the set.
                    let retry = match &r {
                        ExitResult::Refused { outcome, .. } => {
                            events::failing_program(outcome).is_some_and(|p| p != shortwin::ID)
                                || is(
                                    outcome.err.as_ref().and_then(TxError::custom_code),
                                    ShortwinError::BadExtras,
                                )
                                || is(
                                    outcome.err.as_ref().and_then(TxError::custom_code),
                                    ShortwinError::BadRemainingAccounts,
                                )
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

    async fn send_exit(
        &mut self,
        ixs: &[Instruction],
        market: Pubkey,
        extras: ExtrasSel,
        now: i64,
    ) -> ExitResult {
        match self.send_ixs(ixs, &[market], true, now).await {
            Ok(Sent::Landed { out, format, .. }) => ExitResult::Landed {
                outcome: out,
                format,
                extras,
            },
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
            let snap = monitor::snapshot(mv);
            let changes = self
                .pools
                .entry(mv.key)
                .or_default()
                .observe(mv.kind, snap, caps);
            let pool = mv.read_keys.first().copied().unwrap_or_default();
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

    /// Sends the queued cranks in batches (module docs).
    async fn send_cranks(
        &mut self,
        view: &ChainView,
        queue: Vec<(Pubkey, Due)>,
        report: &mut TickReport,
    ) {
        let now = view.clock.unix_timestamp;
        let slot = view.clock.slot;
        let budget = self.cfg.budget.per_market_daily_lamports;
        let mut ready: Vec<(Pubkey, Due)> = Vec::new();
        for (k, d) in queue {
            let track = self.tracks.entry(k).or_default();
            if track.can_spend(k, now, budget, &mut self.alerts) {
                ready.push((k, d));
            } else {
                report
                    .crank_failed
                    .push((k, "daily observation budget spent".into()));
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
                Ok(Sent::DryRun { .. }) => {}
                Err(e) => report.crank_failed.push((k, format!("{e:#}"))),
            }
        }
    }
}
