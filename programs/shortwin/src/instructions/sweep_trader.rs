//! `sweep_trader()` (ARCH §3.3, §3.9; A33, A44, R3A-10, R4A-7): **anyone** recovers value
//! left on a market's trader side between instructions — lamports donated to the `trader` PDA
//! or pre-funded to the `trader_wsol` address, and `ATA(trader, WSOL)` (anyone can create and
//! fund it; no leg uses it) — and credits it to lenders.
//!
//! **Why it exists in M2 (M2-SEC-4).** ARCH schedules it for M6, but M2's `sol_settle` makes a
//! full wipe reachable (lenders withdraw every free token, the last borrowed tokens settle S1),
//! and after a full wipe `deposit` waits for `wipe_trader_swept` (R4A-7), which only this
//! instruction sets unconditionally: the trading legs' normalisation sets it only when it moves
//! lamports, and no leg can run once `S = 0` (opens refuse a shareless pool, no position is
//! left to exit). Without it the market — the only one its mint can have — would take no
//! deposit, and hence no open, until a program upgrade.
//!
//! **Accounts** (ARCH §3.9, less the pump venues' token leg): `config` (read), `market` (mut,
//! bound to its PDA with the stored bump), `market_vault` (mut), `trader` (mut) and
//! `trader_wsol` (mut) at the market's stored keys, `trader_wsol_ata` (mut) =
//! `ATA(trader, WSOL)` under SPL Token (passed whether or not it exists, ARCH §3.0), SPL Token,
//! System, event accounts. **Remaining:** the latest `ShareEpoch` record
//! (`["share_epoch", market, share_epoch − 1]`, key-checked with its stored bump) exactly while
//! a full-wipe record is live (`S = 0 ∧ wipe_live`), nothing otherwise
//! (`BadRemainingAccounts`). The pump venues' `trader_base_ata` → `pool_vault` token sweep
//! arrives with those venues (M5/M6); the mock and Raydium markets have no trader base ATA.
//!
//! **Effects, in order:**
//! 1. `CloseAccount(ATA(trader, WSOL) → trader)` if it is an initialised token account
//!    (`engine::trader::close_trader_ata`), then the §2.4 normalisation: every lamport of
//!    `trader` and of the `trader_wsol` address moves to `market_vault`
//!    (`engine::trader::normalise_pdas`; `TraderStateInvalid` if either is not an empty System
//!    account).
//! 2. The market prologue of a lender instruction (ARCH §3.0 steps 1–5, A16: params-version
//!    sync, accrue with the vest release, roll a completed bucket; no ReadSet, so no record).
//! 3. `pool::sweep(…, SweepSource::Trader)`: `lender_sol += swept`; while a full-wipe record is
//!    live `wipe_trader_swept` is set **even when nothing moved** and the SOL tops up the
//!    record; with `S > 0` it vests over `lump_vest_s`; with `S = 0` and no live wipe it goes
//!    to `protocol_fees` (R3A-10).
//! 4. Post-conditions (step 10): `trader` and `trader_wsol` hold 0 lamports; `market_vault`
//!    holds rent + `lender_sol + protocol_fees` (I2).
//! 5. Event `TraderSwept{lamports, tokens: 0, to, wipe_bits}` (`wipe_bits`: the math crate's
//!    `wipe_swept` bits, `WIPE_TRADER_SWEPT` = 1, `WIPE_REWARDS_CLAIMED` = 2, plus 0x80 while
//!    `wipe_live`).
//!
//! Sweeping nothing succeeds, so a keeper can always call it (ECON §5.8 duty 6: right after a
//! full wipe, before any deposit). It moves no principal and no position's SOL.

use anchor_lang::prelude::*;
use anchor_lang::AccountDeserialize;
use anchor_spl::associated_token::get_associated_token_address;
use shortwin_math::pool::{self, SweepSource};

use crate::constants::{CONFIG_VERSION, MARKET_VERSION, TOKEN_PROGRAM_ID, WSOL_MINT};
use crate::engine::trader::{close_trader_ata, normalise_pdas, TraderPdas};
use crate::engine::{lender, prologue};
use crate::errors::{MathResultExt, ShortwinError};
use crate::events::{TraderSwept, SWEEP_TO_PROTOCOL, SWEEP_TO_VEST, SWEEP_TO_WIPE_RECORD};
use crate::state::{GlobalConfig, Market, MarketVault, ShareEpoch};

/// `TraderSwept.wipe_bits` flag: a full-wipe record is live after the sweep.
pub const WIPE_BITS_LIVE: u8 = 0x80;

#[event_cpi]
#[derive(Accounts)]
pub struct SweepTrader<'info> {
    #[account(
        seeds = [GlobalConfig::SEED],
        bump = config.load()?.bump,
        constraint = config.load()?.version == CONFIG_VERSION @ ShortwinError::ConfigVersion
    )]
    pub config: AccountLoader<'info, GlobalConfig>,
    /// Bound to its PDA `["market", mint]` with the stored bump (`Market::is_pda`; no mint is
    /// passed).
    #[account(
        mut,
        constraint = market.load()?.is_pda(&market.key())
            @ anchor_lang::error::ErrorCode::ConstraintSeeds,
        constraint = market.load()?.version == MARKET_VERSION @ ShortwinError::ConfigVersion,
        has_one = market_vault @ ShortwinError::AccountMismatch,
        has_one = trader @ ShortwinError::AccountMismatch,
        has_one = trader_wsol @ ShortwinError::AccountMismatch
    )]
    pub market: AccountLoader<'info, Market>,
    #[account(
        mut,
        seeds = [MarketVault::SEED, market.key().as_ref()],
        bump = market_vault.load()?.bump
    )]
    pub market_vault: AccountLoader<'info, MarketVault>,
    /// CHECK: `== market.trader` (`has_one`); a System account with no data (checked by the
    /// normalisation).
    #[account(mut)]
    pub trader: UncheckedAccount<'info>,
    /// CHECK: `== market.trader_wsol` (`has_one`); a System account with no data between
    /// instructions (checked by the normalisation).
    #[account(mut)]
    pub trader_wsol: UncheckedAccount<'info>,
    /// CHECK: `ATA(trader, WSOL)` under SPL Token (key-checked by the handler); may not exist.
    #[account(mut)]
    pub trader_wsol_ata: UncheckedAccount<'info>,
    /// CHECK: SPL Token (the WSOL side's program).
    #[account(address = TOKEN_PROGRAM_ID @ ShortwinError::AccountMismatch)]
    pub token_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler<'info>(ctx: Context<'info, SweepTrader<'info>>) -> Result<()> {
    let clock = Clock::get()?;
    let market_key = ctx.accounts.market.key();
    let (trader_bump, trader_wsol_bump) = {
        let m = ctx.accounts.market.load()?;
        (m.trader_bump, m.trader_wsol_bump)
    };
    require_keys_eq!(
        ctx.accounts.trader_wsol_ata.key(),
        get_associated_token_address(&ctx.accounts.trader.key(), &WSOL_MINT),
        ShortwinError::AccountMismatch
    );

    // ---- 1. the trader side's lamports into `market_vault` (untracked until booked) --------
    let accs = &ctx.accounts;
    let from_ata = close_trader_ata(
        &market_key,
        &accs.trader.to_account_info(),
        trader_bump,
        &accs.trader_wsol_ata.to_account_info(),
        &accs.token_program.to_account_info(),
    )?;
    let swept = normalise_pdas(&TraderPdas {
        market: market_key,
        trader: accs.trader.to_account_info(),
        trader_bump,
        trader_wsol: accs.trader_wsol.to_account_info(),
        trader_wsol_bump,
        market_vault: accs.market_vault.to_account_info(),
        system_program: accs.system_program.to_account_info(),
    })?;
    require_gte!(swept, from_ata, ShortwinError::TraderStateInvalid);

    // ---- 2.–3. the prologue and the booking ------------------------------------------------
    let (to, wipe_bits, tracked) = {
        let config = ctx.accounts.config.load()?;
        let mut market = ctx.accounts.market.load_mut()?;
        let mut vault_acc = ctx.accounts.market_vault.load_mut()?;
        let now = prologue::market_now(&market, clock.unix_timestamp);
        prologue::sync_market_params(&config, &mut market, now);
        let mut pool = market.pool();
        let mut vault = vault_acc.vault();
        prologue::accrue(&mut market, &mut pool, &mut vault, now)?;
        prologue::roll_only(&market_key, &mut market, now, clock.slot)?;

        let wipe_live = pool.total_shares == 0 && pool.is_wipe_live();
        let to = if pool.total_shares > 0 {
            SWEEP_TO_VEST
        } else if wipe_live {
            SWEEP_TO_WIPE_RECORD
        } else {
            SWEEP_TO_PROTOCOL
        };
        let rem = ctx.remaining_accounts;
        if wipe_live {
            // The live full-wipe record: the latest one (the epoch the wipe closed).
            require!(rem.len() == 1, ShortwinError::BadRemainingAccounts);
            let recs = lender::load_records(&market_key, rem, 1)?;
            let mut rec = *recs.first().ok_or(ShortwinError::EpochRecordMismatch)?;
            require!(
                rec.epoch.checked_add(1) == Some(pool.share_epoch) && rec.is_full_wipe(),
                ShortwinError::EpochRecordMismatch
            );
            pool::sweep(
                &mut pool,
                &mut vault,
                Some(&mut rec),
                now,
                config.params.lump_vest_s,
                swept,
                SweepSource::Trader,
            )
            .math()?;
            let info = rem.first().ok_or(ShortwinError::EpochRecordMismatch)?;
            require!(info.is_writable, ShortwinError::EpochRecordMismatch);
            let mut account = {
                let data = info.try_borrow_data()?;
                ShareEpoch::try_deserialize(&mut &data[..])
                    .map_err(|_| error!(ShortwinError::EpochRecordMismatch))?
            };
            account.set_record(&rec);
            let mut data = info.try_borrow_mut_data()?;
            let mut w: &mut [u8] = &mut data;
            account.try_serialize(&mut w)?;
        } else {
            require!(rem.is_empty(), ShortwinError::BadRemainingAccounts);
            pool::sweep(
                &mut pool,
                &mut vault,
                None,
                now,
                config.params.lump_vest_s,
                swept,
                SweepSource::Trader,
            )
            .math()?;
        }
        market.set_pool(&pool);
        vault_acc.set_vault(&vault);
        let live_bit = if pool.is_wipe_live() {
            WIPE_BITS_LIVE
        } else {
            0
        };
        (to, pool.wipe_swept | live_bit, lender::tracked(&vault)?)
    };

    // ---- 4. post-conditions ------------------------------------------------------------------
    require!(
        ctx.accounts.trader.lamports() == 0 && ctx.accounts.trader_wsol.lamports() == 0,
        ShortwinError::TraderStateInvalid
    );
    lender::check_vault_floor(&ctx.accounts.market_vault.to_account_info(), tracked)?;

    emit_cpi!(TraderSwept {
        market: market_key,
        slot: clock.slot,
        ts: clock.unix_timestamp,
        lamports: swept,
        tokens: 0,
        to,
        wipe_bits,
    });
    Ok(())
}
