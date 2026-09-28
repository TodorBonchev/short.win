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
            degraded_after_misses: 3,
            recovery_s: 600,
            s3_probe_interval_s: 10,
            discovery_interval_s: 10,
            listing_interval_s: 60,
        }
    }
}

/// Budget and priority policy (O17; ARCH §7.1 duties 8 and 14).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct BudgetConfig {
    /// Lamports the keeper may spend on observations per market per UTC day (`None`:
    /// unlimited). Exits (`liquidate`, `expire`, `sol_settle`) are paid by their rewards
    /// and are never throttled.
    pub per_market_daily_lamports: Option<u64>,
    /// The O17 priority-price ceiling for observations, micro-lamports per CU.
    pub priority_ceiling: u64,
    /// The priority-price ceiling for exits, micro-lamports per CU.
    pub exit_priority_ceiling: u64,
    /// The lowest priority price, micro-lamports per CU.
    pub priority_floor: u64,
    /// Percentile of the recent per-account fees the price starts from, bps (7,500 = p75).
    pub priority_percentile_bps: u32,
    /// Escalation per consecutive missed observation on a market, bps (5,000 = ×1.5).
    pub escalate_bps: u32,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            per_market_daily_lamports: None,
            priority_ceiling: 10_000,
            exit_priority_ceiling: 100_000,
            priority_floor: 0,
            priority_percentile_bps: 7_500,
            escalate_bps: 5_000,
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
    pub venue_watch: VenueWatchConfig,
    pub pool_select: PoolSelectConfig,
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
            venue_watch: VenueWatchConfig::default(),
            pool_select: PoolSelectConfig::default(),
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
        if c.burst_s < 0 || c.recovery_s < 0 || c.s3_probe_interval_s <= 0 {
            bail!("coverage intervals must be non-negative");
        }
        if c.listing_interval_s <= 0 {
            bail!("listing_interval_s must be positive");
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
            [venue_watch]
            pool_admin = false
            [pool_select]
            eval_interval_s = 5
            discovery_rpc = "https://gpa.example"
            "#
        ))
        .unwrap();
        assert_eq!(cfg.markets.keys().unwrap(), Some(vec![k]));
        assert_eq!(cfg.tx_format, TxFormatPolicy::V0);
        assert_eq!(cfg.batch_v0, 3);
        assert_eq!(cfg.coverage.idle_interval_s, 20);
        assert_eq!(cfg.coverage.burst_s, 60);
        assert_eq!(cfg.budget.per_market_daily_lamports, Some(30_000_000));
        assert_eq!(cfg.budget.priority_ceiling, 5_000);
        assert!(!cfg.venue_watch.pool_admin && cfg.venue_watch.flag_upgrades);
        assert_eq!(cfg.pool_select.eval_interval_s, 5);
        assert_eq!(cfg.pool_select.discovery_rpc(), Some("https://gpa.example"));
        assert!(cfg.pool_select.enabled);
        assert_eq!(KeeperConfig::default().pool_select.discovery_rpc(), None);
    }

    #[test]
    fn refuses_what_breaks_the_s3_chain_or_the_coverage_rule() {
        for bad in [
            "[coverage]\nno_route_interval_s = 60",
            "[coverage]\nidle_interval_s = 31",
            "batch_v1 = 0",
            "[budget]\npriority_floor = 20000",
            "unknown = 1",
            "markets = { only = [\"nope\"] }",
            "[pool_select]\neval_interval_s = 0",
            "[pool_select]\nfast_discovery_interval_s = 600",
            "[pool_select]\nunknown = 1",
        ] {
            assert!(KeeperConfig::from_toml(bad).is_err(), "{bad}");
        }
    }
}
