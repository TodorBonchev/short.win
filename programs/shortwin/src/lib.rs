//! short.win on-chain program (Anchor 1.2.0; ARCHITECTURE §2, §3).
//!
//! Module map (ARCH §1.2, §2.1): `constants` (compile-time venue ids, static PDAs, WSOL,
//! `T_CLAMP`, re-exported protocol constants and compiled bounds), `state` (every account of
//! ARCH §2.3, `PodU128`/`PodI128`, T-LAYOUT asserts), `instructions` (one module per
//! instruction), `engine` (shared instruction machinery), `events` (ARCH §3.10), `errors`
//! (ARCH §3.11), `lamports` (checked direct lamport moves), `mint_filter` (the allowlist
//! filter). All arithmetic is `shortwin-math`'s; the program never re-implements a formula.
//!
//! M1 (ARCH §9): the instruction set below. `set_coverage_pause` waits for O17;
//! `set_route_broken` and `set_venue_extras` (O6) and every position instruction are M2.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::unwrap_used
)]

use anchor_lang::prelude::*;

pub mod constants;
pub mod engine;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod lamports;
pub mod mint_filter;
pub mod state;

pub use instructions::*;
pub use state::*;

declare_id!("5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK");

#[program]
pub mod shortwin {
    use super::*;

    // ---- initialization and admin (ARCH §3.1) ----

    /// Creates `GlobalConfig`, `VenueGuard`, the `VenueExtras` and `OracleCache` (upgrade
    /// authority, once).
    pub fn initialize_config(ctx: Context<InitializeConfig>, args: InitConfigArgs) -> Result<()> {
        instructions::initialize_config::handler(ctx, args)
    }

    /// Governance proposes a timelocked config change.
    pub fn propose_config(ctx: Context<ProposeConfig>, change: ConfigChange) -> Result<()> {
        instructions::propose_config::handler(ctx, change)
    }

    /// Anyone applies the pending change once its timelock has run.
    pub fn apply_config(ctx: Context<ApplyConfig>) -> Result<()> {
        instructions::apply_config::handler(ctx)
    }

    /// Governance or the Emergency vault cancels the pending change.
    pub fn cancel_config(ctx: Context<CancelConfig>) -> Result<()> {
        instructions::cancel_config::handler(ctx)
    }

    /// Pauses or resumes new opens, globally or on one market.
    pub fn set_pause(ctx: Context<SetPause>, scope: PauseScope, paused: bool) -> Result<()> {
        instructions::set_pause::handler(ctx, scope, paused)
    }

    /// The Emergency vault disables a venue for opens.
    pub fn disable_venue(ctx: Context<DisableVenue>, bit: u8) -> Result<()> {
        instructions::disable_venue::handler(ctx, bit)
    }

    /// Anyone records a venue program upgrade in the latch.
    pub fn flag_venue_upgraded(ctx: Context<FlagVenueUpgraded>, venue: u8) -> Result<()> {
        instructions::flag_venue_upgraded::handler(ctx, venue)
    }

    /// The Emergency vault re-validates an upgraded venue program at `expected_slot`.
    pub fn revalidate_venue(
        ctx: Context<RevalidateVenue>,
        venue: u8,
        expected_slot: u64,
    ) -> Result<()> {
        instructions::revalidate_venue::handler(ctx, venue, expected_slot)
    }

    /// Anyone moves a market's protocol fees to the configured recipient.
    pub fn sweep_protocol_fees(ctx: Context<SweepProtocolFees>) -> Result<()> {
        instructions::sweep_protocol_fees::handler(ctx)
    }

    // ---- markets (ARCH §3.2, §3.9) ----

    /// Anyone lists a mint, paying at most `max_creation_fee`.
    pub fn create_market(
        ctx: Context<CreateMarket>,
        venue_hint: u8,
        max_creation_fee: u64,
    ) -> Result<()> {
        instructions::create_market::handler(ctx, venue_hint, max_creation_fee)
    }

    /// Anyone flags a market whose mint fails the safety filter.
    pub fn flag_unsafe_mint(ctx: Context<FlagUnsafeMint>) -> Result<()> {
        instructions::flag_unsafe_mint::handler(ctx)
    }

    // ---- lenders (ARCH §3.3) ----

    /// A lender deposits tokens.
    pub fn deposit<'info>(
        ctx: Context<'info, Deposit<'info>>,
        amount: u64,
        min_shares: u128,
    ) -> Result<()> {
        instructions::deposit::handler(ctx, amount, min_shares)
    }

    /// A lender withdraws its pro-rata part (tokens and SOL), bounded below by `min_tokens`
    /// and `min_sol`.
    pub fn withdraw<'info>(
        ctx: Context<'info, Withdraw<'info>>,
        shares: u128,
        min_tokens: u64,
        min_sol: u64,
    ) -> Result<()> {
        instructions::withdraw::handler(ctx, shares, min_tokens, min_sol)
    }

    /// A lender claims its realised SOL.
    pub fn claim_sol<'info>(ctx: Context<'info, ClaimSol<'info>>) -> Result<()> {
        instructions::claim_sol::handler(ctx)
    }

    /// A lender covers a negative SOL balance.
    pub fn cover_deficit<'info>(
        ctx: Context<'info, CoverDeficit<'info>>,
        lamports: u64,
    ) -> Result<()> {
        instructions::cover_deficit::handler(ctx, lamports)
    }

    /// Anyone advances a lender through share epochs.
    pub fn sync_lender<'info>(
        ctx: Context<'info, SyncLender<'info>>,
        max_epochs: u8,
    ) -> Result<()> {
        instructions::sync_lender::handler(ctx, max_epochs)
    }

    // ---- oracle (ARCH §3.9) ----

    /// Anyone refreshes the cached SOL/USD price.
    pub fn crank_sol_usd(ctx: Context<CrankSolUsd>) -> Result<()> {
        instructions::crank_sol_usd::handler(ctx)
    }
}
