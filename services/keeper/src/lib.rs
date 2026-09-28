//! short.win keeper v1 (ARCHITECTURE §7.1; ECON §5.8): duties 1–5 with batching, degraded mode,
//! duty 10's venue drift monitor and duty 12's Raydium pool monitor (with duty 5's fast path,
//! M4), on the Raydium venues and the TEST-ONLY mock:
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
//! 5. **No route**: a halted venue (a Raydium admin halt, a fee above the hard cap) starts the
//!    S3 chain at the keeper's next tick and is observed every 30 s after. **Raydium fast
//!    path** ([`pool_monitor`]): once the incumbent has failed `pool_fail` for
//!    `pool_fast_route_s`, propose the deepest eligible candidate (opening the collection
//!    window, or improving someone else's), and `execute_pool_switch` at the first second after
//!    the window; a failing market without positions is observed to keep its chain.
//! 10. **Venue drift monitor** ([`monitor`]): the six venue ProgramData deploy slots against the
//!     `VenueGuard` latch — an upgrade is flagged with `flag_venue_upgraded` in the slot it is
//!     seen, once, and refreshed when a second upgrade lands during review; Token-2022's upgrade
//!     is alerted — and every covered market's pool and CPMM config (Raydium `SetParams`,
//!     `update_pool_status`, `update_amm_config`, seen as the account changes they make):
//!     route lost / restored, status, open time and fee against the opens and hard caps.
//! 12. **Raydium pool monitor** ([`pool_monitor`]): every Raydium SOL pool of each covered mint
//!     (`getProgramAccounts` on both programs, optionally through a separate endpoint), each
//!     verified and ranked by locked depth `D` with the program's own verifier; `propose_pool`
//!     exactly when the program's own `pool_select::decide` accepts — a list-only market's
//!     first pool (after D1's `classify_pump`), a listing-window replacement (5 %), a
//!     zero-position adoption (25 %, in the tick the last position closes); a market pinned by
//!     open positions to a much thinner pool is alerted (R4B-10); pool events are reported.
//!
//! Operations: several markets' cranks per transaction ([`keeper`]: v1 batches of up to 6, v0
//! batches of up to 4 where v1 is refused, each cut where its transaction would exceed the
//! format's limits), `⌈simulated × 1.1⌉` compute units, a priority price from recent fees capped
//! at the O17 ceiling ([`fees`]), the O17 daily budget, degraded-mode detection (misses, the
//! program's `CoverageGap`; [`alerts`]), per-market metrics ([`metrics`]). The protocol address
//! lookup table of A28 is built and checked by `shortwin-build-alt` ([`alt`]).
//!
//! **Same code as the program** (ARCH §7.1 "Stack"): the keeper links the `shortwin` crate for
//! its account types, its instruction builders, its venue reader and account lists (the Raydium
//! adapters' own key functions) and its market prologue, `shortwin-venue-state` for the venue
//! and ProgramData readers, and `shortwin-math` for every leg, reference and bound, so a keeper
//! decision is the program's decision on the state the next instruction sees ([`decide`]).
//!
//! The chain is pluggable ([`shortwin_services_common::Chain`]): JSON-RPC for a real cluster
//! (the binary), an in-process LiteSVM backend for tests (`services/keeper-litesvm`).
//! Venue kinds ([`venue`]): Raydium AMM v4 and CPMM (M3); the TEST-ONLY mock (`test-venue`
//! feature); the pump venues arrive with M5–M6.

pub mod alerts;
pub mod alt;
pub mod config;
pub mod coverage;
pub mod decide;
pub mod events;
pub mod fees;
pub mod ixs;
pub mod keeper;
pub mod metrics;
pub mod monitor;
pub mod pda;
pub mod pool_monitor;
pub mod venue;
pub mod view;

pub use alerts::{Alert, DegradedReason};
pub use config::KeeperConfig;
pub use keeper::{
    ExitKind, ExitRecord, ExitResult, Keeper, PoolActionRecord, PoolResult, TickReport, V1Support,
};
