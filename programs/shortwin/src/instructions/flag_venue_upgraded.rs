//! `flag_venue_upgraded(venue)` (ARCH §3.1, §3.9, A7; R4B-7): **anyone** records a venue
//! program upgrade in the `VenueGuard` latch.
//!
//! `venue` is a `VenueGuard` index (pump, pAMM, pfee, AMM v4, CPMM, Jupiter). The ProgramData
//! account must be the compile-time constant for that venue and owned by the upgradeable
//! loader, with tag 3. If the bit is clear and the deploy slot differs from `guard_slot`: set
//! the bit, record `flagged_slot` / `flagged_at`. If the bit is set and the deploy slot differs
//! from `flagged_slot` (another upgrade during review): refresh them. Either way emit
//! `VenueUpgradeFlagged`. Otherwise fail with `VenueNotUpgraded`, writing nothing.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;

use crate::constants::{venue_program_data, VENUE_GUARD_VERSION, VENUE_PROGRAMS};
use crate::errors::ShortwinError;
use crate::events::VenueUpgradeFlagged;
use crate::state::VenueGuard;

#[event_cpi]
#[derive(Accounts)]
#[instruction(venue: u8)]
pub struct FlagVenueUpgraded<'info> {
    #[account(
        mut,
        seeds = [VenueGuard::SEED],
        bump = venue_guard.load()?.bump,
        constraint = venue_guard.load()?.version == VENUE_GUARD_VERSION
            @ ShortwinError::ConfigVersion,
        constraint = usize::from(venue) < VENUE_PROGRAMS @ ShortwinError::InvalidParams
    )]
    pub venue_guard: AccountLoader<'info, VenueGuard>,
    /// CHECK: the venue program's ProgramData: key == the compile-time constant, owner == the
    /// upgradeable loader; only its 12-byte header is read (`shortwin_venue_state`).
    #[account(
        address = venue_program_data(venue).unwrap_or_default()
            @ ShortwinError::VenueAccountMismatch,
        owner = bpf_loader_upgradeable::ID @ ShortwinError::VenueAccountMismatch
    )]
    pub program_data: UncheckedAccount<'info>,
}

/// The deploy slot of a venue ProgramData (`VenueAccountMismatch` if the header does not
/// parse: wrong owner, too short, not tag 3).
pub fn read_deploy_slot(program_data: &AccountInfo) -> Result<u64> {
    let data = program_data.try_borrow_data()?;
    shortwin_venue_state::program_data::deploy_slot(&program_data.owner.to_bytes(), &data)
        .map_err(|_| error!(ShortwinError::VenueAccountMismatch))
}

pub fn handler(ctx: Context<FlagVenueUpgraded>, venue: u8) -> Result<()> {
    let slot = read_deploy_slot(&ctx.accounts.program_data)?;
    let clock = Clock::get()?;
    let i = usize::from(venue);
    let bit = 1u32
        .checked_shl(u32::from(venue))
        .ok_or_else(|| error!(ShortwinError::InvalidParams))?;
    let guard_slot = {
        let mut g = ctx.accounts.venue_guard.load_mut()?;
        let guard_slot = *g
            .guard_slot
            .get(i)
            .ok_or_else(|| error!(ShortwinError::InvalidParams))?;
        let flagged_slot = *g
            .flagged_slot
            .get(i)
            .ok_or_else(|| error!(ShortwinError::InvalidParams))?;
        if g.upgraded & bit == 0 {
            require!(slot != guard_slot, ShortwinError::VenueNotUpgraded);
            g.upgraded |= bit;
        } else {
            require!(slot != flagged_slot, ShortwinError::VenueNotUpgraded);
        }
        if let Some(s) = g.flagged_slot.get_mut(i) {
            *s = slot;
        }
        if let Some(t) = g.flagged_at.get_mut(i) {
            *t = clock.unix_timestamp;
        }
        guard_slot
    };
    emit_cpi!(VenueUpgradeFlagged {
        market: Pubkey::default(),
        slot: clock.slot,
        ts: clock.unix_timestamp,
        venue,
        guard_slot,
        programdata_slot: slot,
        flagged_at: clock.unix_timestamp,
    });
    Ok(())
}
