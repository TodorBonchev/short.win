//! Keeper configuration (ARCHITECTURE §7.1 "Operations" 7–9, 14; owner item O17: the
//! per-market daily budget, the priority-price ceiling and the markets covered are the owner's
//! to set).
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

/// The keeper's configuration.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct KeeperConfig {
    /// Markets covered (O17).
    pub markets: MarketSelection,
    /// Batch format policy.
    pub tx_format: TxFormatPolicy,
    /// Cranks per v1 batch (ARCH §7.1 duty 7: 6).
    pub batch_v1: usize,
    /// Cranks per v0 batch (4 with the protocol ALT).
    pub batch_v0: usize,
    /// Seconds before a v1 rejection is re-probed.
    pub v1_reprobe_s: i64,
    /// Address lookup tables for v0 transactions (the protocol ALT of A28), base58.
    pub lookup_tables: Vec<String>,
    /// Simulate only; never send (the keeper still reports what it would do).
    pub dry_run: bool,
    pub coverage: CoverageConfig,
    pub budget: BudgetConfig,
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
        let b = &self.budget;
        if b.priority_floor > b.priority_ceiling || b.priority_floor > b.exit_priority_ceiling {
            bail!("priority_floor exceeds a ceiling");
        }
        if b.priority_percentile_bps > 10_000 {
            bail!("priority_percentile_bps above 10,000");
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
        ] {
            assert!(KeeperConfig::from_toml(bad).is_err(), "{bad}");
        }
    }
}
