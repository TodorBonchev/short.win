//! `cancel_config()` (ARCH §3.1): Governance or the Emergency vault clears the pending change.
//!
//! **Owner decision pending (M1 review, M1-SEC-1).** This is ARCH §3.1 as written: the
//! Emergency vault (no timelock) may cancel *any* pending change, as often as it likes,
//! including an `Authorities` change that would rotate it out. SPEC §11 lists the Emergency's
//! powers (pause opens; O6's route-broken flag and venue extras) and not a veto over
//! governance, so a rogue Emergency key can freeze every parameter change and key rotation
//! until a 48 h program upgrade. The options (Emergency may not cancel `Authorities` /
//! `Timelock` changes; cancelling those needs governance; governance marks a proposal
//! non-cancellable) change the power split between the two multisigs and are the owner's to
//! record in SPEC §11 and ARCH §3.1; the program keeps ARCH's rule until then
//! (`tests/litesvm/tests/security.rs::sec_1_*` pins the current behaviour).

use anchor_lang::prelude::*;

use crate::constants::CONFIG_VERSION;
use crate::errors::ShortwinError;
use crate::events::ConfigCancelled;
use crate::state::GlobalConfig;

#[event_cpi]
#[derive(Accounts)]
pub struct CancelConfig<'info> {
    /// `governance` or `pause_authority`.
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [GlobalConfig::SEED],
        bump = config.load()?.bump,
        constraint = config.load()?.version == CONFIG_VERSION @ ShortwinError::ConfigVersion,
        constraint = config.load()?.is_pause_or_governance(authority.key)
            @ ShortwinError::Unauthorized
    )]
    pub config: AccountLoader<'info, GlobalConfig>,
}

pub fn handler(ctx: Context<CancelConfig>) -> Result<()> {
    let clock = Clock::get()?;
    let (kind, hash) = {
        let mut c = ctx.accounts.config.load_mut()?;
        require!(c.pending.is_some(), ShortwinError::NoPendingChange);
        let cancelled = (c.pending.kind, c.pending.hash());
        c.pending.clear();
        cancelled
    };
    emit_cpi!(ConfigCancelled {
        market: Pubkey::default(),
        slot: clock.slot,
        ts: clock.unix_timestamp,
        kind,
        hash,
        by: ctx.accounts.authority.key(),
    });
    Ok(())
}
