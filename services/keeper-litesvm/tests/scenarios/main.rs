//! Keeper v0 scenarios on LiteSVM with the mock venue (ARCHITECTURE §7.1 duties 1–4, §9 M2
//! "keeper v0 (duties 1–4, batching) against LiteSVM"). Every keeper transaction runs the
//! harness's invariant hooks after every instruction (`shortwin_keeper_litesvm`).
//!
//! Loads short.win's **test-venue** build (`scripts/build-test-venue.sh`, or
//! `SHORTWIN_TEST_VENUE_SO`) and the mock venue (`target/deploy/mock_cpamm.so`, or
//! `MOCK_CPAMM_SO`), as the program tests do.

#![allow(clippy::result_large_err)]

mod batching;
mod bursts;
mod cadence;
mod common;
mod degraded;
mod expire;
mod extras;
mod liquidate;
mod priority;
mod s3;
mod selection;
mod settle;
mod smoke;
