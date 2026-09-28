//! short.win keeper v0 (ARCHITECTURE §7.1; ECON §5.8), duties 1–4 with batching and degraded
//! mode:
//!
//! 1. **Observation coverage** ([`coverage`]): one observation per second on every market with
//!    a position near a threshold (live CR below 1.3·L, deadline within 60 s, armed) or with
//!    `|P_spot/P_ema − 1| > 2 %`; otherwise every 30 s for markets with open positions; every
//!    30 s without a route (the continuous S3 chain; duty 5).
//! 2. **Deviation bursts**: after a foreign observation deviating from the keeper's own view
//!    by more than 2 %, one observation every slot for 60 s.
//! 3. **Liquidate** at the first executable second (the fixed δ bound, retried each second on
//!    `ExitBoundNotMet`); **expire** when the expiry bound is met. Always simulated first.
//! 4. **`sol_settle`** every second for armed positions and positions without a normal
//!    buyback (it arms, keeps the clock, disarms on box evidence, or settles), and for
//!    positions whose S3 is due.
//!
//! Operations: several markets' cranks per transaction ([`keeper`]: v1 batches of 6, v0
//! batches of 4 where v1 is refused), `⌈simulated × 1.1⌉` compute units, a priority price
//! from recent fees capped at the O17 ceiling ([`fees`]), the O17 daily budget, degraded-mode
//! detection (misses, the program's `CoverageGap`; [`alerts`]), per-market metrics
//! ([`metrics`]).
//!
//! **Same code as the program** (ARCH §7.1 "Stack"): the keeper links the `shortwin` crate for
//! its account types, its instruction builders, its venue reader and its market prologue, and
//! `shortwin-math` for every leg, reference and bound, so a keeper decision is the program's
//! decision on the state the next instruction sees ([`decide`]).
//!
//! The chain is pluggable ([`shortwin_services_common::Chain`]): JSON-RPC for a real cluster
//! (the binary), an in-process LiteSVM backend for tests (`services/keeper-litesvm`).
//! Venue kinds: the TEST-ONLY mock (`test-venue` feature) in M2; the MVP venues' account lists
//! arrive with M3–M6 ([`venue`]).

pub mod alerts;
pub mod config;
pub mod coverage;
pub mod decide;
pub mod events;
pub mod fees;
pub mod ixs;
pub mod keeper;
pub mod metrics;
pub mod pda;
pub mod venue;
pub mod view;

pub use alerts::{Alert, DegradedReason};
pub use config::KeeperConfig;
pub use keeper::{ExitKind, ExitRecord, ExitResult, Keeper, TickReport, V1Support};
