//! Keeper configuration (ARCHITECTURE §7.1 "Operations" 7–9, 14; duties 10 and 12; owner item
//! O17: the per-market daily budget, the priority-price ceiling and the markets covered are the
//! owner's to set).
//!
//! Every field has the design default (ECON §5.8 duties, ARCH §7.1), so an empty file is a
//! valid configuration. `keeper.example.toml` documents each one.

use std::str::FromStr;

use anyhow::{Context, bail};
use serde::Deserialize;
use solana_pubkey::Pubkey;

/// Which markets the keeper covers (O17).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketSelection {
    /// Every `Market` account of the program.
    #[default]
    All,
    /// Only these markets (base58 keys).
    Only(Vec<String>),
}

impl MarketSelection {
    /// The selected keys (`None` = all).
    pub fn keys(&self) -> anyhow::Result<Option<Vec<Pubkey>>> {
        match self {
            Self::All => Ok(None),
            Self::Only(v) => v
                .iter()
                .map(|s| Pubkey::from_str(s).with_context(|| format!("market key {s}")))
                .collect::<anyhow::Result<Vec<_>>>()
                .map(Some),
        }
    }
}

/// Which transaction format batches use (ARCH §7.1 duty 7).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TxFormatPolicy {
    /// v1 while the cluster accepts it; v0 after a rejection (re-probed after
    /// `v1_reprobe_s`).
    #[default]
    Auto,
    /// Always v1.
    V1,
    /// Always v0.
    V0,
}

/// Observation coverage (ARCH §7.1 duties 1, 2, 5; ECON §5.8).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct CoverageConfig {
    /// Cadence for a market with open positions and nothing near a threshold (duty 1): 30 s.
    pub idle_interval_s: i64,
    /// Cadence for a market without a route (duty 5; keeps the S3 chain continuous, which
    /// restarts after `S3_OBS_GAP_S` = 60 s): 30 s.
    pub no_route_interval_s: i64,
    /// A position is "near" when its live CR is below this multiple of its `L`, in bps
    /// (1.3·L = 13,000).
    pub near_cr_bps: u32,
    /// A position is "near" when its deadline is within this many seconds: 60.
    pub deadline_window_s: i64,
    /// A market needs per-second coverage when `|P_spot/P_ema − 1|` exceeds this, bps: 200.
    pub ema_dev_bps: u32,
    /// A foreign observation deviating from the keeper's own view by more than this starts a
    /// burst (duty 2), bps: 200.
    pub burst_dev_bps: u32,
    /// Length of a deviation burst (every slot), seconds: 60.
    pub burst_s: i64,
    /// FITS-2 (b): on a market with open positions, a foreign bucket liquidity (`llo` / `lhi`)
    /// more than `burst_dev_bps` off the keeper's own `√(X·Y)` of that second starts this many
    /// seconds of **per-second** observations from that second, extended by every recurrence
    /// (an LP-wrapped crank buys at most this many keeper observations; while an attacker keeps
    /// wrapping, one per second: 1:1); 0 disables it: 10.
    pub liq_cover_s: i64,
    /// Consecutive seconds of missed required observations that make a market degraded
    /// (duty 9): 3.
    pub degraded_after_misses: u32,
    /// Seconds of restored coverage before the degraded state lifts (ARCH §7.1 duty 9:
    /// 10 minutes).
    pub recovery_s: i64,
    /// How often a position on a market whose S3 is due is probed with a simulated
    /// `sol_settle` when nothing says it is liquidatable or expired: 10 s.
    pub s3_probe_interval_s: i64,
    /// Seconds between `getProgramAccounts` discoveries of markets: 10.
    pub discovery_interval_s: i64,
    /// Cadence for a Raydium market with no open position inside its listing window (M4V-5):
    /// 60 s. Its last `EMA_WARMUP_BUCKETS + 2` seconds, and the seconds after it until the
    /// post-change warm-up is over (`Market.ema_warm == 0`, M4R), are observed every second, so
    /// the first open after the window finds an honest `P_ema` and a confirmed EMA.
    pub listing_interval_s: i64,
    /// M7-SEC-1 (a): a `CoverageGap{gap_s}` the program emits in the keeper's own transaction
    /// degrades the market only when `gap_s > idle_interval_s + gap_tolerance_s`. The program
    /// measures the gap between two integrations' **landing** seconds, the keeper schedules its
    /// idle crank from the second it **read** (a `confirmed` clock), and the two differ by the
    /// send latency (0–2 s on mainnet): at the idle cadence of `COVERAGE_GAP_S` itself a crank
    /// landing one second later relative to its read than the previous one is a 31-s gap, which
    /// is jitter, not an outage. A crank that does not land at all is a miss
    /// ([`crate::coverage::MarketTrack::close_second`]), and a longer gap still degrades. 4 (a
    /// gap of 35 s or more degrades); at most `COVERAGE_GAP_S`.
    pub gap_tolerance_s: i64,
    /// M7-SEC-1 (b): a tick that follows the previous one by more than `1 + tick_gap_tolerance_s`
    /// seconds on a market with open positions counts as "the keeper was down" (a degraded
    /// market's recovery restarts). A shorter skip — a tick that waited for its transactions'
    /// confirmation, which takes about a second on mainnet — misses only the seconds that
    /// **required** an observation (per-second coverage: they count toward
    /// `degraded_after_misses` like any missed required second); on the idle cadence the skipped
    /// seconds required nothing. 5.
    pub tick_gap_tolerance_s: i64,
    /// M7-SEC-8: a list-only market, or a curve market far from completion
    /// (`hot_curve_completion_lamports`), without open positions and with nothing time-critical
    /// pending (no re-detection, pool-selection window or chain, graduation step, post-wipe
    /// sweep, coverage pause or burst) is read every `cold_interval_s` instead of every tick —
    /// staggered over the interval, so the per-tick RPC work does not grow with the markets
    /// anyone can list for a fee and rent; a market that has open positions is read every tick
    /// (the set of markets with positions is known each tick from the `ShortPosition`
    /// accounts), and so is every PumpSwap or Raydium market. (0, 60]: 30.
    pub cold_interval_s: i64,
    /// M7-SEC-8: a curve market whose completion (buying its remaining real tokens) costs at most
    /// this many lamports is read every tick, so its graduation (duty 6: `migrate_v2`, the move
    /// onto the pool) follows at once; a curve farther away — a fresh or junk coin, ≈ 85 SOL
    /// from completion — is cold without positions (a buy that completes it is seen within
    /// `cold_interval_s`): 20,000,000,000 (20 SOL).
    pub hot_curve_completion_lamports: u64,
}

impl Default for CoverageConfig {
    fn default() -> Self {
        Self {
            idle_interval_s: 30,
            no_route_interval_s: 30,
            near_cr_bps: 13_000,
            deadline_window_s: 60,
            ema_dev_bps: 200,
            burst_dev_bps: 200,
            burst_s: 60,
            liq_cover_s: 10,
            degraded_after_misses: 3,
            recovery_s: 600,
            s3_probe_interval_s: 10,
            discovery_interval_s: 10,
            listing_interval_s: 60,
            gap_tolerance_s: 4,
            tick_gap_tolerance_s: 5,
            cold_interval_s: 30,
            hot_curve_completion_lamports: 20_000_000_000,
        }
    }
}

impl CoverageConfig {
    /// The largest `CoverageGap{gap_s}` in the keeper's own transaction that is landing jitter
    /// of the idle cadence, not an outage ([`Self::gap_tolerance_s`], M7-SEC-1 (a)):
    /// `max(idle_interval_s + gap_tolerance_s, COVERAGE_GAP_S)`.
    pub fn gap_degrades_above(&self) -> i64 {
        self.idle_interval_s
            .saturating_add(self.gap_tolerance_s)
            .max(shortwin_math::consts::COVERAGE_GAP_S)
    }
}

/// Budget and priority policy (O17; ARCH §7.1 duties 8 and 14).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct BudgetConfig {
    /// Lamports the keeper may spend on coverage observations (`crank_price`) per market per UTC
    /// day (0 or `None`: unlimited). O17: its exhaustion on a market with open positions is degraded
    /// mode (opens paused, exits continue). The cranks exits depend on (per-second coverage of a
    /// position near a threshold, a liquidatable or expired position waiting for a fresh
    /// reference or its bound, the S3 chain) are charged but never throttled; exits (`liquidate`,
    /// `expire`, `sol_settle`), pool selection, graduation moves and `set_coverage_pause` are
    /// neither charged nor throttled. An exhausted budget stops the rest: the idle cadence,
    /// deviation bursts, the liquidity-anomaly and post-switch guard coverage, warm-ups.
    /// PROPOSED default 30,000,000 (0.03 SOL: O17's upper figure for targeted coverage; measured,
    /// `tests/bench/m7_keeper`: a quiet market costs 0.0024–0.0263 SOL a day from batched at no
    /// priority to unbatched at the 100,000 ceiling) — the owner's to confirm.
    pub per_market_daily_lamports: Option<u64>,
    /// The O17 priority-price ceiling for observations, micro-lamports per CU: 100,000 (O17,
    /// owner decision of 2026-09-30).
    pub priority_ceiling: u64,
    /// The priority-price ceiling for exits, `set_coverage_pause`, pool selection and the
    /// upgrade latch, micro-lamports per CU: 100,000.
    pub exit_priority_ceiling: u64,
    /// The lowest priority price, micro-lamports per CU.
    pub priority_floor: u64,
    /// Percentile of the recent per-account fees the price starts from, bps (7,500 = p75).
    pub priority_percentile_bps: u32,
    /// Escalation per consecutive missed observation on a market, bps (5,000 = ×1.5).
    pub escalate_bps: u32,
    /// T-CASCADE (C0-17, ARCH §6.6): the most compute units the keeper requests (its
    /// `⌈simulated × 1.1⌉` limits) per market per slot on exits — every exit write-locks the
    /// `Market` and its `MarketVault`, whose per-block budget is 24,000,000 CU
    /// (`MAX_WRITABLE_ACCOUNT_UNITS`). The rest of a cascade goes out in the next slot, most
    /// under-collateralised first: 12,000,000 (half the account's block budget, so a cascade
    /// never starves other writers of the market in a slot). 0: no cap.
    pub market_cu_per_slot: u64,
    /// T-CASCADE: the same across all of the keeper's exits in a slot — its fee payer signs, and
    /// write-locks, every one of its transactions, so the payer account's 24,000,000 CU block
    /// budget bounds the keeper's whole slot (≈ 109 PumpSwap liquidations): 20,000,000. 0: no cap.
    pub payer_cu_per_slot: u64,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            per_market_daily_lamports: Some(30_000_000),
            priority_ceiling: 100_000,
            exit_priority_ceiling: 100_000,
            priority_floor: 0,
            priority_percentile_bps: 7_500,
            escalate_bps: 5_000,
            market_cu_per_slot: 12_000_000,
            payer_cu_per_slot: 20_000_000,
        }
    }
}

/// O17's automatic degraded mode (ARCH §3.1 `set_coverage_pause`, §7.1 duty 9): the keeper
/// instance designated as the pauser holds the `coverage_authority` key — a separate key from its
/// fee payer, which can only pause and unpause opens per market (no funds, never exits) — and
/// sends `set_coverage_pause(market, true)` when a market with open positions is degraded,
/// `false` after `recovery_s` of restored coverage. Exactly one instance (of the regions that run
/// concurrently) is the pauser; the others only alert, so they never flip-flop the flag.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct CoveragePauseConfig {
    /// This instance is the designated pauser: false (the `Degraded` alert pages the Emergency
    /// multisig to `set_pause` instead).
    pub enabled: bool,
    /// Path of the `coverage_authority` keypair file (the binary's `--coverage-keypair`
    /// overrides it); required when `enabled`. Empty: none.
    pub authority_keypair: String,
}

impl CoveragePauseConfig {
    /// The keypair path, if one is set.
    pub fn authority_keypair(&self) -> Option<&str> {
        let s = self.authority_keypair.trim();
        (!s.is_empty()).then_some(s)
    }
}

/// Duty 11 (ARCH §7.1, §4.7; C1-12): `crank_sol_usd` — the USD tiers read the cached Pyth SOL/USD
/// price, and a cache older than `oracle_stale_s` (3,600 s) puts every market in Tier C.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct OracleConfig {
    /// Crank the configured `PriceUpdateV2` into the `OracleCache` whenever it holds a newer
    /// update: true.
    pub enabled: bool,
    /// Seconds between reads of the `PriceUpdateV2` (the sponsored feed updates on a 55-s
    /// heartbeat or a 0.5 % move; a newer update found is cranked at once, a failed crank retried
    /// every second): 30.
    pub interval_s: i64,
    /// `Alert::OracleAging` when the cache's publish time is older than this (half of
    /// `oracle_stale_s`; at `oracle_stale_s` every market falls to Tier C), at most hourly: 1,800.
    pub aging_alert_s: i64,
}

impl Default for OracleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_s: 30,
            aging_alert_s: 1_800,
        }
    }
}

/// The venue drift monitor (ARCH §7.1 duty 10; [`crate::monitor`]).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct VenueWatchConfig {
    /// Watch the six venue ProgramData accounts (and Token-2022's, alert only) and call
    /// `flag_venue_upgraded` in the slot after an upgrade: true.
    pub flag_upgrades: bool,
    /// Report admin actions on every covered market's pool and CPMM config (Raydium
    /// `SetParams`, `update_pool_status`, `update_amm_config`): true.
    pub pool_admin: bool,
}

impl Default for VenueWatchConfig {
    fn default() -> Self {
        Self {
            flag_upgrades: true,
            pool_admin: true,
        }
    }
}

/// Raydium pool selection (ARCH §2.6; §7.1 duty 5's fast path and duty 12, the pool monitor;
/// [`crate::pool_monitor`]).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct PoolSelectConfig {
    /// Run the pool monitor and the fast path: true.
    pub enabled: bool,
    /// Seconds between `getProgramAccounts` discoveries of a covered mint's Raydium SOL pools
    /// (four filtered calls per mint: AMM v4 and CPMM, WSOL on either side): 300. A market is
    /// discovered at once the first time the keeper sees it.
    pub discovery_interval_s: i64,
    /// The same while the market's incumbent fails `pool_fail` or a fast-path collection window
    /// runs (a pool created or refilled for the fast path is seen within this): 10.
    pub fast_discovery_interval_s: i64,
    /// Seconds between re-reads of the candidates' accounts when nothing is pending (at once
    /// when the incumbent starts or stops failing `pool_fail`; every tick while the fast path is
    /// armed, a window runs, or a waiting candidate meets a market whose last position just
    /// closed): 30.
    pub eval_interval_s: i64,
    /// R4B-10: a market whose adoption of a dominant candidate (≥ `pool_switch_margin_bps` more
    /// locked depth) is blocked this long by open positions is reported as pinned
    /// (`Alert::PoolPinned`: page the Emergency multisig to pause opens there so it drains):
    /// 3,600 s.
    pub pinned_alert_s: i64,
    /// A separate JSON-RPC endpoint for the discovery's `getProgramAccounts` calls (many
    /// providers restrict or meter them); empty: the keeper's own `--rpc`.
    pub discovery_rpc: String,
    /// M4-SEC-7: the best candidates (by locked SOL) evaluated each tick; the rest of a mint's
    /// pools — anyone can create them — are never fetched per tick: 8.
    pub max_candidates: usize,
    /// M4-SEC-7: the accounts (two per pool: its WSOL vault and LP mint) a discovery's ranking
    /// reads per tick and market; a larger discovery is ranked over several ticks: 200.
    pub rank_keys_per_tick: usize,
    /// M7-SEC-8: the most markets whose pool discovery (four `getProgramAccounts` calls each) runs
    /// in one tick; the due ones go by urgency — the fast cadence (a failing incumbent, a
    /// window), then markets with open positions, then the longest waiting. Listing is
    /// permissionless, so without it a batch of junk list-only markets made the ticks that
    /// discovered them (at first sight, then every `discovery_interval_s`) issue four calls per
    /// market at once. Pool selection runs after the tick's exits and cranks (M4-SEC-7), so these
    /// calls delay the next tick, not this one's duties: 2.
    pub discoveries_per_tick: usize,
}

impl Default for PoolSelectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            discovery_interval_s: 300,
            fast_discovery_interval_s: 10,
            eval_interval_s: 30,
            pinned_alert_s: 3_600,
            discovery_rpc: String::new(),
            max_candidates: 8,
            rank_keys_per_tick: 200,
            discoveries_per_tick: 2,
        }
    }
}

impl PoolSelectConfig {
    /// The discovery endpoint, if one is set.
    pub fn discovery_rpc(&self) -> Option<&str> {
        let s = self.discovery_rpc.trim();
        (!s.is_empty()).then_some(s)
    }
}

/// Graduation duties (ARCH §2.5, §5.1, §5.4, §7.1 duty 6; [`crate::graduation`]).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct GraduationConfig {
    /// Call pump `migrate_v2` top-level as soon as a covered market's curve completes (ADOPTED
    /// Q16), then the `→ PumpSwap` transition in the same second: true (M6; M5 suspended it,
    /// M5-SEC-4, because a migrated curve's market could not reach its pool).
    pub migrate: bool,
    /// Send the straddle bundle (tx1 `complete_curve`, tx2 `migrate_v2`, tx3 the transition onto
    /// the new pool) for a liquidatable position whose `Q` exceeds the curve's `rT`: true. Needs a
    /// block engine (`--block-engine`).
    pub straddle: bool,
    /// Send tx1 + tx2 alone when tx3 cannot be built or fails its pre-flight (ARCH §5.4's keeper
    /// fallback, V1-9): true (M6: the keeper's own transition follows right after the bundle, and
    /// the position is an ordinary PumpSwap position from then on; M5 had no transition and kept
    /// it false).
    pub send_without_exit: bool,
    /// `max_sol_cost` of `complete_curve` above the keeper's quote of buying the `rT` it read, in
    /// bps (a curve sale between the read and the bundle raises the cost; beyond this pump
    /// reverts tx1 and the bundle): 500.
    pub complete_slippage_bps: u32,
    /// The most the keeper spends completing one curve, lamports (it keeps the `rT` tokens it
    /// buys; ECON §10.1): 20 SOL.
    pub max_complete_lamports: u64,
    /// The Jito tip the bundle's last transaction pays, lamports: 100,000.
    pub tip_lamports: u64,
    /// The Jito tip accounts (base58), used in turn: the eight mainnet ones.
    pub tip_accounts: Vec<String>,
    /// The Jito block engine bundles go to (e.g. `https://mainnet.block-engine.jito.wtf`); empty:
    /// none (the binary's `--block-engine` overrides it).
    pub block_engine: String,
}

impl Default for GraduationConfig {
    fn default() -> Self {
        Self {
            migrate: true,
            straddle: true,
            send_without_exit: true,
            complete_slippage_bps: 500,
            max_complete_lamports: 20_000_000_000,
            tip_lamports: 100_000,
            tip_accounts: crate::graduation::JITO_TIP_ACCOUNTS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            block_engine: String::new(),
        }
    }
}

impl GraduationConfig {
    /// The tip accounts.
    pub fn tip_keys(&self) -> anyhow::Result<Vec<Pubkey>> {
        self.tip_accounts
            .iter()
            .map(|s| Pubkey::from_str(s).with_context(|| format!("tip account {s}")))
            .collect()
    }

    /// The block engine, if one is set.
    pub fn block_engine(&self) -> Option<&str> {
        let s = self.block_engine.trim();
        (!s.is_empty()).then_some(s)
    }
}

/// The trader side's value and the venue cashback (ARCH §7.1 duty 6 "right after a full wipe",
/// duty 13; [`crate::sweep`]). PROPOSED ops knobs (ECON §15 names none).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SweepConfig {
    /// Right after a full wipe, the claims the trader's venue cashback needs and `sweep_trader`
    /// (a deposit waits for both post-wipe bits, R4A-7): true.
    pub post_wipe: bool,
    /// `sweep_trader` when the trader side holds anything (`ATA(trader, WSOL)`, lamports on the
    /// `trader` / `trader_wsol` PDAs, tokens on a pump venue's trader base ATA): true.
    pub sweep: bool,
    /// Seconds between the reads of a market's trader side (duty 13): 60.
    pub check_interval_s: i64,
    /// Seconds between two cashback claims on one market (ARCH duty 13: weekly): 604,800.
    pub cashback_interval_s: i64,
    /// The claimable cashback a claim waits for, lamports (a PumpSwap claim costs the keeper the
    /// rent of `ATA(trader, WSOL)`, 2,039,280 at the pre-2026 rent, which the sweep credits to the
    /// lenders): 10,000,000. The post-wipe bundle claims whatever is claimable.
    pub cashback_min_lamports: u64,
}

impl Default for SweepConfig {
    fn default() -> Self {
        Self {
            post_wipe: true,
            sweep: true,
            check_interval_s: 60,
            cashback_interval_s: 604_800,
            cashback_min_lamports: 10_000_000,
        }
    }
}

/// The keeper's configuration.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct KeeperConfig {
    /// Markets covered (O17).
    pub markets: MarketSelection,
    /// Batch format policy.
    pub tx_format: TxFormatPolicy,
    /// Cranks per v1 batch at most (ARCH §7.1 duty 7: 6); a batch is also cut where its
    /// transaction would exceed the format's size or account limit.
    pub batch_v1: usize,
    /// Cranks per v0 batch at most (4 with the protocol ALT; T-SIZE), cut the same way.
    pub batch_v0: usize,
    /// Seconds before a v1 rejection is re-probed.
    pub v1_reprobe_s: i64,
    /// Address lookup tables for v0 transactions (the protocol ALT of A28), base58.
    pub lookup_tables: Vec<String>,
    /// Simulate only; never send (the keeper still reports what it would do).
    pub dry_run: bool,
    pub coverage: CoverageConfig,
    pub budget: BudgetConfig,
    pub coverage_pause: CoveragePauseConfig,
    pub oracle: OracleConfig,
    pub venue_watch: VenueWatchConfig,
    pub pool_select: PoolSelectConfig,
    pub graduation: GraduationConfig,
    pub sweep: SweepConfig,
}

impl Default for KeeperConfig {
    fn default() -> Self {
        Self {
            markets: MarketSelection::All,
            tx_format: TxFormatPolicy::Auto,
            batch_v1: 6,
            batch_v0: 4,
            v1_reprobe_s: 3_600,
            lookup_tables: Vec::new(),
            dry_run: false,
            coverage: CoverageConfig::default(),
            budget: BudgetConfig::default(),
            coverage_pause: CoveragePauseConfig::default(),
            oracle: OracleConfig::default(),
            venue_watch: VenueWatchConfig::default(),
            pool_select: PoolSelectConfig::default(),
            graduation: GraduationConfig::default(),
            sweep: SweepConfig::default(),
        }
    }
}

impl KeeperConfig {
    /// Parses and checks a TOML configuration.
    pub fn from_toml(s: &str) -> anyhow::Result<Self> {
        let cfg: Self = toml::from_str(s).context("keeper configuration")?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Relational checks.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.batch_v1 == 0 || self.batch_v0 == 0 {
            bail!("batch sizes must be at least 1");
        }
        let c = &self.coverage;
        if c.no_route_interval_s <= 0
            || c.no_route_interval_s >= shortwin_math::consts::S3_OBS_GAP_S
        {
            bail!(
                "no_route_interval_s must be in (0, {}) so the S3 chain stays continuous",
                shortwin_math::consts::S3_OBS_GAP_S
            );
        }
        if c.idle_interval_s <= 0 || c.idle_interval_s > shortwin_math::consts::COVERAGE_GAP_S {
            bail!(
                "idle_interval_s must be in (0, {}] (COVERAGE_GAP_S)",
                shortwin_math::consts::COVERAGE_GAP_S
            );
        }
        if c.burst_s < 0 || c.liq_cover_s < 0 || c.recovery_s < 0 || c.s3_probe_interval_s <= 0 {
            bail!("coverage intervals must be non-negative");
        }
        if c.listing_interval_s <= 0 {
            bail!("listing_interval_s must be positive");
        }
        if c.gap_tolerance_s < 0 || c.gap_tolerance_s > shortwin_math::consts::COVERAGE_GAP_S {
            bail!(
                "gap_tolerance_s must be in [0, {}] (COVERAGE_GAP_S)",
                shortwin_math::consts::COVERAGE_GAP_S
            );
        }
        if c.tick_gap_tolerance_s < 0
            || c.tick_gap_tolerance_s > shortwin_math::consts::COVERAGE_GAP_S
        {
            bail!(
                "tick_gap_tolerance_s must be in [0, {}] (COVERAGE_GAP_S)",
                shortwin_math::consts::COVERAGE_GAP_S
            );
        }
        if c.cold_interval_s <= 0 || c.cold_interval_s > 60 {
            bail!("cold_interval_s must be in (0, 60]");
        }
        let b = &self.budget;
        if b.priority_floor > b.priority_ceiling || b.priority_floor > b.exit_priority_ceiling {
            bail!("priority_floor exceeds a ceiling");
        }
        if b.priority_percentile_bps > 10_000 {
            bail!("priority_percentile_bps above 10,000");
        }
        let p = &self.pool_select;
        if p.discovery_interval_s <= 0
            || p.fast_discovery_interval_s <= 0
            || p.eval_interval_s <= 0
            || p.pinned_alert_s < 0
        {
            bail!("pool_select intervals must be positive");
        }
        if p.fast_discovery_interval_s > p.discovery_interval_s {
            bail!("pool_select.fast_discovery_interval_s exceeds discovery_interval_s");
        }
        if p.max_candidates == 0 || p.rank_keys_per_tick < 2 {
            bail!("pool_select.max_candidates must be positive and rank_keys_per_tick at least 2");
        }
        if p.discoveries_per_tick == 0 {
            bail!("pool_select.discoveries_per_tick must be positive");
        }
        let g = &self.graduation;
        if g.complete_slippage_bps > 10_000 {
            bail!("graduation.complete_slippage_bps above 10,000");
        }
        if g.straddle && (g.tip_lamports == 0 || g.tip_keys()?.is_empty()) {
            bail!("graduation: a bundle needs a tip (tip_lamports > 0 and a tip account)");
        }
        let w = &self.sweep;
        if w.check_interval_s <= 0 || w.cashback_interval_s <= 0 {
            bail!("sweep intervals must be positive");
        }
        let o = &self.oracle;
        if o.interval_s <= 0 || o.interval_s > 30 || o.aging_alert_s <= 0 {
            bail!(
                "oracle.interval_s must be in (0, 30] (ARCH §7.1 duty 11) and aging_alert_s positive"
            );
        }
        self.markets.keys()?;
        for t in &self.lookup_tables {
            Pubkey::from_str(t).with_context(|| format!("lookup table {t}"))?;
        }
        Ok(())
    }

    /// The configured lookup tables.
    pub fn lookup_table_keys(&self) -> anyhow::Result<Vec<Pubkey>> {
        self.lookup_tables
            .iter()
            .map(|s| Pubkey::from_str(s).with_context(|| format!("lookup table {s}")))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_the_defaults() {
        assert_eq!(
            KeeperConfig::from_toml("").unwrap(),
            KeeperConfig::default()
        );
    }

    #[test]
    fn the_example_file_is_the_defaults() {
        let cfg = KeeperConfig::from_toml(include_str!("../keeper.example.toml")).unwrap();
        assert_eq!(cfg, KeeperConfig::default());
    }

    #[test]
    fn parses_every_section() {
        let k = Pubkey::new_unique();
        let cfg = KeeperConfig::from_toml(&format!(
            r#"
            markets = {{ only = ["{k}"] }}
            tx_format = "v0"
            batch_v0 = 3
            [coverage]
            idle_interval_s = 20
            [budget]
            per_market_daily_lamports = 30000000
            priority_ceiling = 5000
            market_cu_per_slot = 6000000
            [coverage_pause]
            enabled = true
            authority_keypair = "/etc/shortwin/coverage.json"
            [venue_watch]
            pool_admin = false
            [pool_select]
            eval_interval_s = 5
            discovery_rpc = "https://gpa.example"
            [graduation]
            migrate = false
            send_without_exit = false
            block_engine = "https://mainnet.block-engine.jito.wtf"
            [sweep]
            cashback_min_lamports = 1
            "#
        ))
        .unwrap();
        assert_eq!(cfg.markets.keys().unwrap(), Some(vec![k]));
        assert_eq!(cfg.tx_format, TxFormatPolicy::V0);
        assert_eq!(cfg.batch_v0, 3);
        assert_eq!(cfg.coverage.idle_interval_s, 20);
        assert_eq!(cfg.coverage.burst_s, 60);
        assert_eq!(cfg.coverage.liq_cover_s, 10);
        assert_eq!(cfg.budget.per_market_daily_lamports, Some(30_000_000));
        assert_eq!(cfg.budget.priority_ceiling, 5_000);
        assert_eq!(cfg.budget.market_cu_per_slot, 6_000_000);
        assert_eq!(cfg.budget.payer_cu_per_slot, 20_000_000);
        assert!(cfg.coverage_pause.enabled);
        assert_eq!(
            cfg.coverage_pause.authority_keypair(),
            Some("/etc/shortwin/coverage.json")
        );
        // O17 (owner decision of 2026-09-30): both ceilings 100,000 µlamports/CU; the pauser is
        // opt-in per instance.
        let d = KeeperConfig::default();
        assert_eq!(
            (d.budget.priority_ceiling, d.budget.exit_priority_ceiling),
            (100_000, 100_000)
        );
        assert!(!d.coverage_pause.enabled && d.coverage_pause.authority_keypair().is_none());
        assert_eq!(d.budget.per_market_daily_lamports, Some(30_000_000));
        assert!(!cfg.venue_watch.pool_admin && cfg.venue_watch.flag_upgrades);
        assert_eq!(cfg.pool_select.eval_interval_s, 5);
        assert_eq!(cfg.pool_select.discovery_rpc(), Some("https://gpa.example"));
        assert!(cfg.pool_select.enabled);
        assert_eq!(KeeperConfig::default().pool_select.discovery_rpc(), None);
        assert!(!cfg.graduation.send_without_exit && !cfg.graduation.migrate);
        // M6: the migrate_v2 duty (with the → PumpSwap transition) and ARCH §5.4's tx1 + tx2
        // fallback are on by default again (M5-SEC-4 suspended them).
        let d = KeeperConfig::default();
        assert!(d.graduation.migrate && d.graduation.send_without_exit);
        assert_eq!(cfg.sweep.cashback_min_lamports, 1);
        assert!(cfg.sweep.post_wipe && cfg.sweep.sweep);
        assert_eq!(d.sweep.cashback_interval_s, 7 * 86_400);
        assert_eq!(
            cfg.graduation.block_engine(),
            Some("https://mainnet.block-engine.jito.wtf")
        );
        assert_eq!(KeeperConfig::default().graduation.block_engine(), None);
        assert_eq!(
            KeeperConfig::default().graduation.tip_keys().unwrap().len(),
            8
        );
    }

    #[test]
    fn refuses_what_breaks_the_s3_chain_or_the_coverage_rule() {
        for bad in [
            "[coverage]\nno_route_interval_s = 60",
            "[coverage]\nidle_interval_s = 31",
            "[coverage]\nliq_cover_s = -1",
            "[coverage]\ngap_tolerance_s = -1",
            "[coverage]\ngap_tolerance_s = 31",
            "[coverage]\ntick_gap_tolerance_s = -1",
            "[coverage]\ncold_interval_s = 0",
            "[coverage]\ncold_interval_s = 61",
            "batch_v1 = 0",
            "[budget]\npriority_floor = 200000",
            "unknown = 1",
            "markets = { only = [\"nope\"] }",
            "[pool_select]\neval_interval_s = 0",
            "[pool_select]\nfast_discovery_interval_s = 600",
            "[pool_select]\ndiscoveries_per_tick = 0",
            "[pool_select]\nunknown = 1",
            "[graduation]\ncomplete_slippage_bps = 10001",
            "[graduation]\ntip_lamports = 0",
            "[graduation]\ntip_accounts = []",
            "[graduation]\ntip_accounts = [\"nope\"]",
            "[graduation]\nunknown = 1",
            "[sweep]\ncheck_interval_s = 0",
            "[sweep]\nunknown = 1",
            "[coverage_pause]\nunknown = 1",
            "[oracle]\ninterval_s = 31",
            "[oracle]\ninterval_s = 0",
            "[oracle]\nunknown = 1",
        ] {
            assert!(KeeperConfig::from_toml(bad).is_err(), "{bad}");
        }
    }
}
