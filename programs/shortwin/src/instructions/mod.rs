//! One module per instruction (ARCH §3): the `#[derive(Accounts)]` struct with every
//! constraint, and `handler`. `lib.rs` stays thin: each entrypoint calls
//! `instructions::<name>::handler`.
//!
//! M1 status: every M1 instruction (ARCH §9 M1) is implemented — admin, config and latch;
//! `create_market` and `flag_unsafe_mint`; the six lender-pool instructions; `crank_sol_usd`.

pub mod apply_config;
pub mod cancel_config;
pub mod claim_sol;
pub mod cover_deficit;
pub mod crank_sol_usd;
pub mod create_market;
pub mod deposit;
pub mod disable_venue;
pub mod flag_unsafe_mint;
pub mod flag_venue_upgraded;
pub mod initialize_config;
pub mod propose_config;
pub mod revalidate_venue;
pub mod set_pause;
pub mod sweep_protocol_fees;
pub mod sync_lender;
pub mod withdraw;

// The `#[program]` macro needs each Accounts struct (and the `__client_accounts_*` /
// `__cpi_client_accounts_*` modules its derive generates) at the crate root. Every module also
// has a `handler`; `lib.rs` always calls it by its full path, so the glob re-exports' clash on
// that one name is harmless.
#[allow(ambiguous_glob_reexports)]
pub use apply_config::*;
pub use cancel_config::*;
pub use claim_sol::*;
pub use cover_deficit::*;
pub use crank_sol_usd::*;
pub use create_market::*;
pub use deposit::*;
pub use disable_venue::*;
pub use flag_unsafe_mint::*;
pub use flag_venue_upgraded::*;
pub use initialize_config::*;
pub use propose_config::*;
pub use revalidate_venue::*;
pub use set_pause::*;
pub use sweep_protocol_fees::*;
pub use sync_lender::*;
pub use withdraw::*;
