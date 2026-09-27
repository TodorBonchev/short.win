//! Shared instruction machinery (ARCH §2.1 `engine/`): what several instructions run in the
//! same order, applying `shortwin-math` results to accounts. Pure arithmetic stays in the math
//! crate.
//!
//! M1 modules (ARCH §9 M1):
//!
//! | Module | Content |
//! |---|---|
//! | [`guard`] | mint guard (ARCH §3.0 step 3); M2 adds the open guards (pauses, latch masks, route flags) |
//! | [`prologue`] | market prologue (ARCH §3.0): `market_now`, params-version sync (step 2), accrue (step 4); M2 adds the observation roll (step 5) and steps 6–8 |
//! | [`lender`] | `ShareEpoch` record loading, new-position setup, the post-wipe reward rule, SOL pay-outs and the vault floor (ARCH §3.3) |
//!
//! M2+ add `observe`, `settle`, `bounds`, `route`, `trader`, `pool_select`, `caps`, `tier`,
//! `waterfall` (ARCH §2.1).

pub mod guard;
pub mod lender;
pub mod prologue;
