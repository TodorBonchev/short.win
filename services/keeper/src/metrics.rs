//! Keeper metrics (ARCHITECTURE §7.1 duty 8: per-market coverage seconds, CU and lamports
//! spent; "Keys and ops": missed liquidations, coverage SLO misses, budget burn), rendered in
//! the Prometheus text format by [`Metrics::render`].

use std::collections::BTreeMap;
use std::fmt::Write;

use anchor_lang::prelude::Pubkey;

/// One market's counters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarketMetrics {
    /// Seconds in which the keeper landed at least one observation.
    pub covered_seconds: u64,
    /// Observations landed (cranks and exits carrying the ReadSet).
    pub observations: u64,
    /// `crank_price` instructions landed.
    pub cranks: u64,
    pub liquidations: u64,
    pub expiries: u64,
    /// `sol_settle` calls landed (arming re-checks included).
    pub settle_calls: u64,
    /// Settlements executed (S1, S2, S3).
    pub settlements: u64,
    /// Compute units attributed to this market.
    pub cu: u64,
    /// Lamports spent on this market (fees, split evenly across a batch).
    pub lamports: u64,
    /// Seconds that needed an observation and got none.
    pub misses: u64,
    /// Deviation bursts started.
    pub bursts: u64,
    /// Liquidity-triggered per-second coverage windows started (FITS-2 (b)).
    pub liq_covers: u64,
    /// `CoverageGap` events seen.
    pub coverage_gaps: u64,
    /// Admin actions seen on the market's pool or CPMM config (duty 10).
    pub venue_admin_changes: u64,
    /// `propose_pool` calls landed (duties 5 and 12).
    pub pool_proposals: u64,
    /// `execute_pool_switch` calls landed (duty 5).
    pub pool_executions: u64,
    /// Pool adoptions seen (the keeper's or anyone's).
    pub pool_switches: u64,
    /// pump `migrate_v2` calls of the keeper landed (duty 6).
    pub migrations: u64,
    /// Straddle bundles of the keeper landed (duty 6; ARCH §5.4).
    pub straddles: u64,
    /// Degraded (1) or not (0).
    pub degraded: bool,
    /// The last second covered.
    last_covered: i64,
}

impl MarketMetrics {
    /// Counts `now` as covered (once per second).
    pub fn cover(&mut self, now: i64) {
        if self.last_covered != now {
            self.last_covered = now;
            self.covered_seconds = self.covered_seconds.saturating_add(1);
        }
    }
}

/// All counters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metrics {
    pub markets: BTreeMap<Pubkey, MarketMetrics>,
    pub ticks: u64,
    pub tx_landed: u64,
    pub tx_failed: u64,
    pub simulations: u64,
    pub simulation_failures: u64,
    pub batches_v1: u64,
    pub batches_v0: u64,
    pub v1_rejections: u64,
    /// `flag_venue_upgraded` calls landed (duty 10).
    pub venue_flags: u64,
    /// Raydium pool discoveries run (duty 12; four `getProgramAccounts` calls each).
    pub pool_discoveries: u64,
}

impl Metrics {
    /// The counters of `market`.
    pub fn market(&mut self, market: Pubkey) -> &mut MarketMetrics {
        self.markets.entry(market).or_default()
    }

    /// Prometheus text exposition.
    pub fn render(&self) -> String {
        let mut s = String::new();
        let g = |s: &mut String, name: &str, help: &str, v: u64| {
            let _ = writeln!(s, "# HELP {name} {help}\n# TYPE {name} counter\n{name} {v}");
        };
        g(
            &mut s,
            "shortwin_keeper_ticks_total",
            "Keeper ticks.",
            self.ticks,
        );
        g(
            &mut s,
            "shortwin_keeper_tx_landed_total",
            "Transactions landed.",
            self.tx_landed,
        );
        g(
            &mut s,
            "shortwin_keeper_tx_failed_total",
            "Transactions that failed on chain.",
            self.tx_failed,
        );
        g(
            &mut s,
            "shortwin_keeper_simulations_total",
            "Simulations run.",
            self.simulations,
        );
        g(
            &mut s,
            "shortwin_keeper_simulation_failures_total",
            "Simulations that failed.",
            self.simulation_failures,
        );
        g(
            &mut s,
            "shortwin_keeper_batches_v1_total",
            "Crank batches sent as v1.",
            self.batches_v1,
        );
        g(
            &mut s,
            "shortwin_keeper_batches_v0_total",
            "Crank batches sent as v0.",
            self.batches_v0,
        );
        g(
            &mut s,
            "shortwin_keeper_v1_rejections_total",
            "v1 transactions the cluster refused.",
            self.v1_rejections,
        );
        g(
            &mut s,
            "shortwin_keeper_venue_flags_total",
            "flag_venue_upgraded calls landed.",
            self.venue_flags,
        );
        g(
            &mut s,
            "shortwin_keeper_pool_discoveries_total",
            "Raydium pool discoveries run (four getProgramAccounts calls each).",
            self.pool_discoveries,
        );
        type Row = (&'static str, &'static str, fn(&MarketMetrics) -> u64);
        let rows: [Row; 20] = [
            (
                "covered_seconds",
                "Seconds with a keeper observation.",
                |m| m.covered_seconds,
            ),
            ("observations", "Observations landed.", |m| m.observations),
            ("cranks", "crank_price instructions landed.", |m| m.cranks),
            ("liquidations", "Liquidations landed.", |m| m.liquidations),
            ("expiries", "Expiries landed.", |m| m.expiries),
            ("settle_calls", "sol_settle calls landed.", |m| {
                m.settle_calls
            }),
            ("settlements", "SOL settlements executed.", |m| {
                m.settlements
            }),
            ("compute_units", "Compute units spent.", |m| m.cu),
            ("lamports", "Lamports spent.", |m| m.lamports),
            (
                "misses",
                "Seconds that needed an observation and got none.",
                |m| m.misses,
            ),
            ("bursts", "Deviation bursts started.", |m| m.bursts),
            (
                "liq_covers",
                "Liquidity-triggered per-second coverage windows started.",
                |m| m.liq_covers,
            ),
            ("coverage_gaps", "CoverageGap events seen.", |m| {
                m.coverage_gaps
            }),
            (
                "venue_admin_changes",
                "Admin actions seen on the market's pool or CPMM config.",
                |m| m.venue_admin_changes,
            ),
            ("pool_proposals", "propose_pool calls landed.", |m| {
                m.pool_proposals
            }),
            (
                "pool_executions",
                "execute_pool_switch calls landed.",
                |m| m.pool_executions,
            ),
            ("pool_switches", "Pool adoptions seen.", |m| m.pool_switches),
            ("migrations", "pump migrate_v2 calls landed.", |m| {
                m.migrations
            }),
            ("straddles", "Straddle bundles landed.", |m| m.straddles),
            ("degraded", "1 while the market is in degraded mode.", |m| {
                u64::from(m.degraded)
            }),
        ];
        for (name, help, get) in rows {
            let full = format!("shortwin_keeper_market_{name}");
            let kind = if name == "degraded" {
                "gauge"
            } else {
                "counter"
            };
            let _ = writeln!(s, "# HELP {full} {help}\n# TYPE {full} {kind}");
            for (k, m) in &self.markets {
                let _ = writeln!(s, "{full}{{market=\"{k}\"}} {}", get(m));
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_per_market_series() {
        let mut m = Metrics::default();
        let k = Pubkey::new_unique();
        m.market(k).cover(5);
        m.market(k).cover(5);
        m.market(k).cover(6);
        m.market(k).lamports = 5_000;
        let out = m.render();
        assert!(out.contains(&format!(
            "shortwin_keeper_market_covered_seconds{{market=\"{k}\"}} 2"
        )));
        assert!(out.contains(&format!(
            "shortwin_keeper_market_lamports{{market=\"{k}\"}} 5000"
        )));
        assert!(out.contains("# TYPE shortwin_keeper_market_degraded gauge"));
    }
}
