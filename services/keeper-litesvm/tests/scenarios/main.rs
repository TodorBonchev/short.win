//! Keeper scenarios on LiteSVM (ARCHITECTURE §7.1): v0's duties 1–4 and batching on the mock
//! venue (§9 M2), v1 on Raydium (§9 M3: the `raydium*` modules — the duties on the dumped AMM
//! v4 and CPMM programs and pools, duty 10's admin-action monitor and upgrade latch, the
//! protocol ALT built on chain), and Raydium pool selection (§9 M4: `pool_select` — duty 12's
//! pool monitor and duty 5's fast path on planted sibling pools), and `freeze` (M4F-1 and its
//! follow-ups on the fork: the keeper's per-second coverage while the post-change guard is set;
//! duty 2's liquidity-triggered **per-second** coverage on markets with open positions, FITS-2
//! (b) — the per-slot liquidity bursts M4F-1 added stay reverted, M4G-2, so duty-2 bursts are
//! price-only — against LP-wrapped cranks after a switch and in steady state; the carried fit of
//! FIT-2 / FITS-1; FITV-2 / FITS-2's withdraw contest and FITX-1's pinned residual, a genuinely
//! reached low frozen by withdraw-wrapped cranks; the keeper's cost of an LP-wrapped crank), and
//! `graduation` (M5, duty 6 on the pump-curve fork: `migrate_v2` as soon as a curve completes, the
//! straddle bundle with the `complete-curve` helper, a curve trade between the keeper's read and its
//! bundle; a curve that graduated elsewhere settled in S3 on its detection set, duty 5; a
//! list-only market re-detected on its curve and its trader accounts created).
//! Every keeper transaction runs the harness's invariant hooks after every instruction
//! (`shortwin_keeper_litesvm`).
//!
//! Loads short.win's **test-venue** build (`scripts/build-test-venue.sh`, or
//! `SHORTWIN_TEST_VENUE_SO`) and the mock venue (`target/deploy/mock_cpamm.so`, or
//! `MOCK_CPAMM_SO`) for the mock scenarios, and the **release** build
//! (`target/deploy/shortwin.so`, or `SHORTWIN_SO`) with the committed mainnet fixtures for the
//! Raydium and pump ones, as the program tests do; `graduation` also loads the straddle helper
//! (`target/deploy/complete_curve.so`, or `COMPLETE_CURVE_SO`).

#![allow(clippy::result_large_err)]

mod batching;
mod bursts;
mod cadence;
mod common;
mod degraded;
mod expire;
mod extras;
mod freeze;
mod graduation;
mod liquidate;
mod pool_select;
mod priority;
mod raydium;
mod raydium_admin;
mod raydium_alt;
mod s3;
mod selection;
mod settle;
mod smoke;
