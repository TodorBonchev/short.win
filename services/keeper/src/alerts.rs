//! Alerts (ARCHITECTURE §7.1 "Keys and ops", duties 5, 6, 9, 10 and 12): what the keeper pages
//! about. The keeper returns them in each [`crate::keeper::TickReport`] and logs them; the
//! binary forwards them to its log (a pager integration is an ops choice).

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
    /// Duty 5 / 12 (ARCH §2.6): `market` adopted another pool — by the keeper's own
    /// `propose_pool` / `execute_pool_switch` (`kind`: its `POOL_KIND_*`), or seen on chain
    /// (`kind` `None`: someone else's transaction). A list-only market's `from` is the default
    /// key.
    PoolSwitched {
        market: Pubkey,
        from: Pubkey,
        to: Pubkey,
        venue: VenueKind,
        kind: Option<u8>,
        d_inc: u128,
        d_cand: u128,
        by_keeper: bool,
    },
    /// Duty 5: a fast-path collection window opened (or took a deeper candidate) on
    /// `candidate`; it resolves at `window_ends`.
    PoolWindow {
        market: Pubkey,
        candidate: Pubkey,
        d_cand: u128,
        window_ends: i64,
        improved: bool,
        by_keeper: bool,
    },
    /// Duty 5: a collection window closed without an adoption (the best candidate no longer
    /// eligible, or the incumbent no longer failing).
    PoolSwitchCancelled {
        market: Pubkey,
        candidate: Pubkey,
        by_keeper: bool,
    },
    /// Duty 12, R4B-10: a candidate with at least `pool_switch_margin_bps` more locked depth has
    /// been waiting since `since` while open positions pin the market to its thinner incumbent
    /// — page the Emergency multisig to pause opens on the market so it drains; the keeper
    /// adopts the candidate in the slot the last position closes.
    PoolPinned {
        market: Pubkey,
        incumbent: Pubkey,
        candidate: Pubkey,
        d_inc: u128,
        d_cand: u128,
        open_positions: u32,
        since: i64,
    },
    /// A pool-selection instruction the keeper's projection expected to pass was refused by the
    /// program (custom error `code`) or failed on chain; alerted once per pool and code.
    PoolSelectRefused {
        market: Pubkey,
        pool: Pubkey,
        code: Option<u32>,
        detail: String,
    },
    /// Duty 6: the keeper's `migrate_v2` of `market`'s completed curve failed in simulation or
    /// on chain (retried every second while the curve stays unmigrated); alerted once per
    /// market and detail. Positions pause (`VenueMigrating`) until someone migrates.
    MigrateFailed { market: Pubkey, detail: String },
    /// Duty 6: a liquidatable position's `Q` exceeds its live curve's `rT` (ARCH §5.4) and the
    /// straddle bundle is not sent (`reason`: no exit transaction before M6, no block engine,
    /// the completion above `max_complete_lamports`, a failed pre-flight); the keeper calls
    /// `sol_settle` for it instead. Alerted once per position and reason.
    StraddleWaiting {
        market: Pubkey,
        position: Pubkey,
        reason: String,
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
            Self::PoolPinned { .. } => tracing::error!(alert = ?self, "pool selection alert"),
            Self::MigrateFailed { .. } | Self::StraddleWaiting { .. } => {
                tracing::error!(alert = ?self, "graduation alert");
            }
            Self::PoolSwitched { .. }
            | Self::PoolWindow { .. }
            | Self::PoolSwitchCancelled { .. }
            | Self::PoolSelectRefused { .. } => {
                tracing::warn!(alert = ?self, "pool selection");
            }
            _ => tracing::warn!(alert = ?self, "keeper alert"),
        }
    }
}
