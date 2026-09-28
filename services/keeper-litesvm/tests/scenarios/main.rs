//! Keeper scenarios on LiteSVM (ARCHITECTURE §7.1): v0's duties 1–4 and batching on the mock
//! venue (§9 M2), and v1 on Raydium (§9 M3: the `raydium*` modules — the duties on the dumped AMM
//! v4 and CPMM programs and pools, duty 10's admin-action monitor and upgrade latch, the
//! protocol ALT built on chain). Every keeper transaction runs the harness's invariant hooks
//! after every instruction (`shortwin_keeper_litesvm`).
//!
//! Loads short.win's **test-venue** build (`scripts/build-test-venue.sh`, or
//! `SHORTWIN_TEST_VENUE_SO`) and the mock venue (`target/deploy/mock_cpamm.so`, or
//! `MOCK_CPAMM_SO`) for the mock scenarios, and the **release** build
//! (`target/deploy/shortwin.so`, or `SHORTWIN_SO`) with the committed mainnet fixtures for the
//! Raydium ones, as the program tests do.

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
mod raydium;
mod raydium_admin;
mod raydium_alt;
mod s3;
mod selection;
mod settle;
mod smoke;
