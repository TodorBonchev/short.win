//! `disable_venue(bit)` (ARCH §3.1): the Emergency vault clears a `venue_enabled` bit at once;
//! re-enabling is timelocked (`ConfigChange::EnableVenues`). Opens only.

use anchor_lang::prelude::*;

use crate::constants::{CONFIG_VERSION, VENUE_BITS_KNOWN};
use crate::errors::ShortwinError;
use crate::events::VenueDisabled;
use crate::state::GlobalConfig;

#[event_cpi]
#[derive(Accounts)]
pub struct DisableVenue<'info> {
    pub pause_authority: Signer<'info>,
    #[account(
        mut,
        seeds = [GlobalConfig::SEED],
        bump = config.load()?.bump,
        constraint = config.load()?.version == CONFIG_VERSION @ ShortwinError::ConfigVersion,
        constraint = config.load()?.pause_authority == pause_authority.key()
            @ ShortwinError::Unauthorized
    )]
    pub config: AccountLoader<'info, GlobalConfig>,
}

pub fn handler(ctx: Context<DisableVenue>, bit: u8) -> Result<()> {
    let mask = 1u32
        .checked_shl(u32::from(bit))
        .filter(|m| m & VENUE_BITS_KNOWN != 0)
        .ok_or_else(|| error!(ShortwinError::InvalidParams))?;
    let clock = Clock::get()?;
    let venue_enabled = {
        let mut c = ctx.accounts.config.load_mut()?;
        c.venue_enabled &= !mask;
        c.venue_enabled
    };
    emit_cpi!(VenueDisabled {
        market: Pubkey::default(),
        slot: clock.slot,
        ts: clock.unix_timestamp,
        bit,
        venue_enabled,
    });
    Ok(())
}
