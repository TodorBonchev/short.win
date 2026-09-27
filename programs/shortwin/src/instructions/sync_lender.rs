//! `sync_lender(max_epochs)` (ARCH §3.3; C0-6): **anyone** advances a lender through up to
//! `max_epochs` share epochs and touches it.
//!
//! `pool::convert` with the first `max_epochs` `ShareEpoch` records of `remaining_accounts`
//! (key-checked): value moves only into that lender's own `pending` / `real`; its shares, lock
//! and lock term scale with each record, and a lender left with nothing jumps to the current
//! epoch (V2-12). If that brought the lender to the current epoch (or parked it on a live
//! full-wipe record), `pool::touch` follows: its surplus of realised SOL over entitlement
//! moves to the pot (R3A-2) and its own expired lock is cleared (the calendar already dropped
//! the lock from the aggregate, v0.4.1). Otherwise the partial conversion is kept and the
//! next call continues it. `sync_lender(0)` is a plain touch. Does not write `Market`
//! (C0-17). Event `LenderSynced`.
//!
//! Remaining accounts: up to `max_epochs` `ShareEpoch` records, in epoch order, starting at or
//! before the lender's epoch; extra accounts are ignored.

use anchor_lang::prelude::*;
use shortwin_math::pool::{self, Standing};

use crate::constants::MARKET_VERSION;
use crate::engine::lender;
use crate::errors::{MathResultExt, ShortwinError};
use crate::events::LenderSynced;
use crate::state::{LenderPosition, Market, MarketVault};

#[event_cpi]
#[derive(Accounts)]
pub struct SyncLender<'info> {
    /// Read-only (C0-17). Bound to its PDA `["market", mint]` with the stored bump
    /// (`Market::is_pda`; ARCH §2.3; M1-SEC-6 / M1-FID-6).
    #[account(
        constraint = market.load()?.is_pda(&market.key())
            @ anchor_lang::error::ErrorCode::ConstraintSeeds,
        constraint = market.load()?.version == MARKET_VERSION @ ShortwinError::ConfigVersion,
        has_one = market_vault @ ShortwinError::AccountMismatch
    )]
    pub market: AccountLoader<'info, Market>,
    #[account(
        mut,
        seeds = [MarketVault::SEED, market.key().as_ref()],
        bump = market_vault.load()?.bump
    )]
    pub market_vault: AccountLoader<'info, MarketVault>,
    #[account(
        mut,
        seeds = [LenderPosition::SEED, market.key().as_ref(), lender.owner.as_ref()],
        bump = lender.bump,
        has_one = market @ ShortwinError::AccountMismatch
    )]
    pub lender: Box<Account<'info, LenderPosition>>,
}

pub fn handler<'info>(ctx: Context<'info, SyncLender<'info>>, max_epochs: u8) -> Result<()> {
    let clock = Clock::get()?;
    let now = clock.unix_timestamp;
    let market_key = ctx.accounts.market.key();

    let (owner, t_after, s_after, after) = {
        let market = ctx.accounts.market.load()?;
        let mut vault_acc = ctx.accounts.market_vault.load_mut()?;
        let pool_state = market.pool();
        let mut vault = vault_acc.vault();
        let records =
            lender::load_records(&market_key, ctx.remaining_accounts, usize::from(max_epochs))?;
        let position = &mut ctx.accounts.lender;
        let mut l = position.lender();
        let standing = pool::convert(&pool_state, &mut l, &records).math()?;
        if standing != Standing::Behind {
            pool::touch(&pool_state, &mut vault, &mut l, &records, now).math()?;
        }

        vault_acc.set_vault(&vault);
        position.set_lender(&l);
        (
            position.owner,
            pool_state.tokens_free,
            pool_state.total_shares,
            l,
        )
    };

    emit_cpi!(LenderSynced {
        market: market_key,
        slot: clock.slot,
        ts: now,
        owner,
        amount: 0,
        shares: 0,
        t_after,
        s_after,
        sol_paid: 0,
        pot_draw: 0,
        lock: after.lock,
        lock_until: after.lock_until,
        epoch: after.epoch,
    });
    Ok(())
}
