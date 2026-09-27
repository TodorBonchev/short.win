//! short.win program-test harness on LiteSVM 0.16 (ARCHITECTURE §8.2, §8.3).
//!
//! - [`Env`]: LiteSVM with `target/deploy/shortwin.so` (build it first with `anchor build
//!   --tools-version v1.57 --arch v3`), the admin keys (short.win's upgrade authority recorded
//!   in its ProgramData, Governance, Emergency, fee recipient), the clock under test control
//!   ([`Env::warp`], [`Env::set_clock`]) and the invariant hooks on every [`Env::send`].
//! - [`ix`]: instruction builders for every M1 instruction (anchor-lang's generated types).
//! - [`tokens`]: SPL Token and Token-2022 mints, accounts and holders.
//! - [`fixtures`]: mock venue ProgramData (T-LATCH), raw account writers, an installed
//!   list-only market (as `create_market` writes it), mint fixtures.
//! - [`invariants`]: `check_all(&svm, market)` (I1, I2, I8) after every instruction, and
//!   `check_ix` (I4, I17, the lender instruction bounds) for each instruction against the
//!   state just before it; [`Env::send`] runs both on every market a transaction names, after
//!   each instruction of a multi-instruction transaction too.
//! - [`events`], [`errors`], [`read`], [`pda`]: event decoding, error assertions, account
//!   readers, PDAs.
//!
//! Agave line: LiteSVM 0.16.0 on the 4.2.2 runtime crates, pinned in the workspace lockfile
//! (C1-14; `Cargo.toml` of this crate).

// `TxResult` is LiteSVM's own `TransactionResult` (its `FailedTransactionMetadata` carries the
// logs); boxing it would only make every call site unwrap one more layer.
#![allow(clippy::result_large_err)]

pub mod env;
pub mod errors;
pub mod events;
pub mod fixtures;
pub mod invariants;
pub mod ix;
pub mod pda;
pub mod read;
pub mod tokens;

pub use env::{Env, START_SLOT, START_TS, TxResult};
