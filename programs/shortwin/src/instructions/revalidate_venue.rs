//! `revalidate_venue(venue, expected_slot)` (ARCH §3.1, A7; R4B-7): the Emergency vault clears
//! the latch for a venue program after reviewing the upgrade.
//!
//! Requires the ProgramData's current deploy slot to equal `expected_slot` (the slot the
//! Emergency signers reviewed), else `VenueSlotMismatch`, so an upgrade that lands while the
//! Squads proposal collects signatures cannot be validated silently. Then `guard_slot[venue] =
//! expected_slot` and bit `venue` is cleared. Same address and owner checks as
//! `flag_venue_upgraded`.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;

use crate::constants::{venue_program_data, CONFIG_VERSION, VENUE_GUARD_VERSION, VENUE_PROGRAMS};
use crate::errors::ShortwinError;
use crate::events::VenueRevalidated;
use crate::instructions::flag_venue_upgraded::read_deploy_slot;
use crate::state::{GlobalConfig, VenueGuard};

#[event_cpi]
#[derive(Accounts)]
#[instruction(venue: u8)]
pub struct RevalidateVenue<'info> {
    pub pause_authority: Signer<'info>,
    #[account(
        seeds = [GlobalConfig::SEED],
        bump = config.load()?.bump,
        constraint = config.load()?.version == CONFIG_VERSION @ ShortwinError::ConfigVersion,
        constraint = config.load()?.pause_authority == pause_authority.key()
            @ ShortwinError::Unauthorized
    )]
    pub config: AccountLoader<'info, GlobalConfig>,
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
    /// upgradeable loader; only its header is read.
    #[account(
        address = venue_program_data(venue).unwrap_or_default()
            @ ShortwinError::VenueAccountMismatch,
        owner = bpf_loader_upgradeable::ID @ ShortwinError::VenueAccountMismatch
    )]
    pub program_data: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<RevalidateVenue>, venue: u8, expected_slot: u64) -> Result<()> {
    let slot = read_deploy_slot(&ctx.accounts.program_data)?;
    require_eq!(slot, expected_slot, ShortwinError::VenueSlotMismatch);
    let clock = Clock::get()?;
    let i = usize::from(venue);
    let bit = 1u32
        .checked_shl(u32::from(venue))
        .ok_or_else(|| error!(ShortwinError::InvalidParams))?;
    let (old_slot, flagged_at) = {
        let mut g = ctx.accounts.venue_guard.load_mut()?;
        let was_flagged = g.upgraded & bit != 0;
        let flagged_at = if was_flagged {
            g.flagged_at.get(i).copied().unwrap_or_default()
        } else {
            0
        };
        let slot_ref = g
            .guard_slot
            .get_mut(i)
            .ok_or_else(|| error!(ShortwinError::InvalidParams))?;
        let old = *slot_ref;
        *slot_ref = expected_slot;
        g.upgraded &= !bit;
        (old, flagged_at)
    };
    emit_cpi!(VenueRevalidated {
        market: Pubkey::default(),
        slot: clock.slot,
        ts: clock.unix_timestamp,
        venue,
        guard_slot: old_slot,
        programdata_slot: slot,
        flagged_at,
        expected_slot,
    });
    Ok(())
}
