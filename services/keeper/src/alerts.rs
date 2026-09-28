//! Alerts (ARCHITECTURE §7.1 "Keys and ops", duty 9): what the keeper pages about. The keeper
//! returns them in each [`crate::keeper::TickReport`] and logs them; the binary forwards them to
//! its log (a pager integration is an ops choice).

use anchor_lang::prelude::Pubkey;

/// Why a market is degraded (duty 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DegradedReason {
    /// This many consecutive seconds needed an observation and got none (budget, outage,
    /// contested blocks).
    Misses(u32),
    /// The program reported `CoverageGap{gap_s, open_positions}` (A69).
    CoverageGap { gap_s: i64, open_positions: u32 },
}

/// An alert.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Alert {
    /// The keeper cannot meet duties 1–2 on `market`: page the Emergency multisig to
    /// `set_pause(market)` with reason `KeeperCoverage` (or, once O17 enables the
    /// `coverage_authority`, `set_coverage_pause(true)`).
    Degraded {
        market: Pubkey,
        reason: DegradedReason,
    },
    /// Coverage has been met for `recovery_s` again: the `KeeperCoverage` pause may lift.
    CoverageRestored { market: Pubkey, degraded_s: i64 },
    /// The market's O17 daily observation budget is spent.
    BudgetExhausted {
        market: Pubkey,
        spent: u64,
        budget: u64,
    },
    /// The cluster refused a v1 transaction: batches fall back to v0.
    V1Unsupported { reason: String },
    /// An exit the keeper's projection expected was refused by the program in simulation
    /// (custom error `code`), or failed on a venue with every extras set.
    ExitRefused {
        market: Pubkey,
        position: Pubkey,
        code: Option<u32>,
        detail: String,
    },
}

impl Alert {
    /// Logs the alert.
    pub fn log(&self) {
        match self {
            Self::Degraded { .. } | Self::BudgetExhausted { .. } => {
                tracing::error!(alert = ?self, "keeper coverage alert");
            }
            _ => tracing::warn!(alert = ?self, "keeper alert"),
        }
    }
}
