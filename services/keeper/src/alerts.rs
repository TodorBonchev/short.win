//! Alerts (ARCHITECTURE §7.1 "Keys and ops", duties 9 and 10): what the keeper pages about. The
//! keeper returns them in each [`crate::keeper::TickReport`] and logs them; the binary forwards
//! them to its log (a pager integration is an ops choice).

use anchor_lang::prelude::Pubkey;
use shortwin::state::VenueKind;

use crate::monitor::AdminChange;

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
    /// Duty 10: venue program `venue` (a `VenueGuard` index) was upgraded — its ProgramData
    /// deploy slot is no longer the one the latch validated (or flagged). The keeper flags it
    /// (`flag_venue_upgraded`); page the Emergency multisig to run the fork suite against the
    /// new binary, then `revalidate_venue(venue, deploy_slot)`, publish an extras set or (once
    /// O6 is signed) set the route-broken flag.
    VenueUpgraded {
        venue: u8,
        program: Pubkey,
        /// The latch's validated slot (`guard_slot`), or the flagged one for a second upgrade
        /// during review.
        known_slot: u64,
        deploy_slot: u64,
    },
    /// The keeper's `flag_venue_upgraded(venue)` landed: opens on that venue's markets are
    /// refused until `revalidate_venue`.
    VenueUpgradeFlagged { venue: u8, deploy_slot: u64 },
    /// `flag_venue_upgraded(venue)` could not be sent or was refused (other than "already
    /// flagged"); retried every second while the upgrade stands unflagged.
    VenueFlagFailed { venue: u8, detail: String },
    /// A watched upgradeable program without a latch index (Token-2022) was upgraded: alert
    /// only (ARCH §7.1 duty 10).
    ProgramUpgraded {
        program: Pubkey,
        previous_slot: u64,
        deploy_slot: u64,
    },
    /// A watched ProgramData account is missing or no longer a ProgramData account.
    ProgramDataUnreadable { program: Pubkey },
    /// Duty 10: an admin action changed a market's pool (a Raydium `SetParams`,
    /// `update_pool_status` or `update_amm_config`, seen as the account change it makes;
    /// [`crate::monitor`]).
    VenueAdmin {
        market: Pubkey,
        venue: VenueKind,
        pool: Pubkey,
        change: AdminChange,
    },
}

impl Alert {
    /// Logs the alert.
    pub fn log(&self) {
        match self {
            Self::Degraded { .. } | Self::BudgetExhausted { .. } => {
                tracing::error!(alert = ?self, "keeper coverage alert");
            }
            Self::VenueUpgraded { .. }
            | Self::VenueFlagFailed { .. }
            | Self::ProgramUpgraded { .. }
            | Self::ProgramDataUnreadable { .. } => {
                tracing::error!(alert = ?self, "venue program alert");
            }
            Self::VenueAdmin { change, .. } if change.is_severe() => {
                tracing::error!(alert = ?self, "venue admin action");
            }
            _ => tracing::warn!(alert = ?self, "keeper alert"),
        }
    }
}
