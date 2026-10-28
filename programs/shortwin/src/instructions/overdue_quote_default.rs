//! Owner-selected fixed-clock collateral foreclosure for unpaid quote-native debt.
//!
//! This is NOT token repayment, fair-value settlement or a no-route assertion.
//! Actual Clock>=deadline+locked24h authorizes the separate terminal action even
//! when prices would be healthy. All tracked V goes to lenders less locked K_s(V).
//! The borrower's escrow/storage rent and untracked donations remain refundable.
use anchor_lang::prelude::*;
use shortwin_math::pool;

use crate::constants::{CONFIG_VERSION, MARKET_VERSION};
use crate::engine::{arming, lender, quote_finance};
use crate::errors::{CheckedExt, MathResultExt, ShortwinError};
use crate::events::{EpochRolled, OverdueCollateralDefault};
use crate::lamports;
use crate::state::{
    GlobalConfig, Market, MarketVault, QuoteFinanceExposure, QuoteFinanceMarket,
    QuoteFinancePosition, ShareEpoch,
};

#[event_cpi]
#[derive(Accounts)]
pub struct OverdueQuoteDefault<'info> {
    /// Permissionless executor and reward recipient; funds a roll record if needed.
    #[account(mut)]
    pub keeper: Signer<'info>,
    #[account(seeds = [GlobalConfig::SEED], bump = config.load()?.bump,
        constraint = config.load()?.version == CONFIG_VERSION @ ShortwinError::ConfigVersion)]
    pub config: AccountLoader<'info, GlobalConfig>,
    #[account(mut,
        constraint = market.load()?.is_pda(&market.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
        constraint = market.load()?.version == MARKET_VERSION @ ShortwinError::ConfigVersion,
        has_one = market_vault @ ShortwinError::AccountMismatch)]
    pub market: AccountLoader<'info, Market>,
    #[account(mut, seeds = [QuoteFinanceMarket::SEED, market.key().as_ref()], bump = finance_market.bump,
        has_one = market @ ShortwinError::AccountMismatch,
        has_one = config @ ShortwinError::AccountMismatch)]
    pub finance_market: Box<Account<'info, QuoteFinanceMarket>>,
    #[account(mut,
        has_one = borrower @ ShortwinError::AccountMismatch,
        has_one = market @ ShortwinError::AccountMismatch,
        has_one = finance_market @ ShortwinError::AccountMismatch,
        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds)]
    pub position: Box<Account<'info, QuoteFinancePosition>>,
    /// CHECK: Bound to position.borrower; receives only account rent/untracked refund.
    #[account(mut)]
    pub borrower: UncheckedAccount<'info>,
    #[account(mut, seeds = [QuoteFinanceExposure::SEED, market.key().as_ref(), borrower.key().as_ref()],
        bump = exposure.bump, has_one = market @ ShortwinError::AccountMismatch,
        constraint = exposure.wallet == borrower.key() @ ShortwinError::AccountMismatch)]
    pub exposure: Box<Account<'info, QuoteFinanceExposure>>,
    #[account(mut, seeds = [MarketVault::SEED, market.key().as_ref()], bump = market_vault.load()?.bump)]
    pub market_vault: AccountLoader<'info, MarketVault>,
    /// CHECK: Exact current share-epoch PDA checked before booking. Created only on roll.
    #[account(mut)]
    pub share_epoch_rec: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler<'info>(
    ctx: Context<'info, OverdueQuoteDefault<'info>>,
    max_epoch_record_rent: u64,
) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        ShortwinError::AccountMismatch
    );
    require_eq!(
        ctx.accounts.finance_market.to_account_info().data_len(),
        QuoteFinanceMarket::SPACE,
        ShortwinError::AccountMismatch
    );
    require_eq!(
        ctx.accounts.position.to_account_info().data_len(),
        QuoteFinancePosition::SPACE,
        ShortwinError::AccountMismatch
    );
    require_eq!(
        ctx.accounts.exposure.to_account_info().data_len(),
        QuoteFinanceExposure::SPACE,
        ShortwinError::AccountMismatch
    );
    let clock = Clock::get()?;
    let market_key = ctx.accounts.market.key();
    let finance_key = ctx.accounts.finance_market.key();
    let position_key = ctx.accounts.position.key();
    let pos = (**ctx.accounts.position).clone();
    // Never authorize with market_now's monotonic accrual clamp or a report clock.
    require_gte!(
        clock.unix_timestamp,
        pos.default_at,
        ShortwinError::OverdueDefaultNotReady
    );
    let account_balance = ctx.accounts.position.to_account_info().lamports();
    let floor = lamports::rent_min(QuoteFinancePosition::SPACE)?
        .checked_add(pos.v)
        .or_math()?;
    require_gte!(account_balance, floor, ShortwinError::InsufficientVault);
    let position_account_refund = account_balance.checked_sub(pos.v).or_math()?;
    let (mut pool, mut vault, booked, now, epoch_bump) = {
        let mut market = ctx.accounts.market.load_mut()?;
        let epoch_le = market.share_epoch.to_le_bytes();
        let (expected, epoch_bump) = Pubkey::find_program_address(
            &[ShareEpoch::SEED, market_key.as_ref(), &epoch_le],
            &crate::ID,
        );
        require_keys_eq!(
            ctx.accounts.share_epoch_rec.key(),
            expected,
            ShortwinError::EpochRecordMismatch
        );
        let mut pool = market.pool();
        let mut vault = ctx.accounts.market_vault.load()?.vault();
        let now = quote_finance::accrue(
            &ctx.accounts.config.key(),
            &market_key,
            &mut market,
            &mut ctx.accounts.finance_market,
            &mut pool,
            &mut vault,
            clock.unix_timestamp,
        )?;
        let booked = quote_finance::book_overdue_default(
            &ctx.accounts.config.key(),
            &market_key,
            &finance_key,
            &mut market,
            &mut ctx.accounts.finance_market,
            &mut pool,
            &mut vault,
            clock.unix_timestamp,
            &pos,
        )?;
        (pool, vault, booked, now, epoch_bump)
    };
    let mut rolled = None;
    if let Some(mut rec) = booked.record {
        // The grief-proof creator sweeps any pre-funding to the old lenders,
        // then the keeper supplies the full actual Rent minimum. Its debit is
        // never reduced by a donation or allowed above the signed caller bound.
        let record_rent = lamports::rent_min(ShareEpoch::SPACE)?;
        require_gte!(
            max_epoch_record_rent,
            record_rent,
            ShortwinError::EpochRecordRentAboveMax
        );
        let epoch_le = rec.epoch.to_le_bytes();
        let bump_seed = [epoch_bump];
        let seeds: &[&[u8]] = &[ShareEpoch::SEED, market_key.as_ref(), &epoch_le, &bump_seed];
        let rec_info = ctx.accounts.share_epoch_rec.to_account_info();
        require!(
            *rec_info.owner == System::id() && rec_info.data_is_empty(),
            ShortwinError::EpochRecordMismatch
        );
        let swept = arming::create_pda_account(
            &ctx.accounts.keeper.to_account_info(),
            &rec_info,
            &ctx.accounts.market_vault.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            seeds,
            ShareEpoch::SPACE,
            &crate::ID,
        )?;
        if swept > 0 {
            vault.lender_sol = vault.lender_sol.checked_add(swept).or_math()?;
            let wipe = (pool.total_shares == 0 && pool.wipe_live != 0).then_some(&mut rec);
            pool::vest(
                &mut pool,
                &mut vault,
                wipe,
                now,
                pos.lump_vest_seconds,
                swept,
            )
            .math()?;
            ctx.accounts.market.load_mut()?.set_pool(&pool);
        }
        let record = ShareEpoch {
            market: market_key,
            epoch: rec.epoch,
            acc_end: rec.acc_end,
            acc_r_end: rec.acc_r_end,
            s_old: rec.s_old,
            carry: rec.carry,
            carry_r: rec.carry_r,
            conv_rem: rec.conv_rem,
            num: rec.num,
            den_log1000: rec.den_log1000,
            bump: epoch_bump,
        };
        record.try_serialize(&mut &mut rec_info.try_borrow_mut_data()?[..])?;
        rolled = Some(EpochRolled {
            market: market_key,
            slot: clock.slot,
            ts: clock.unix_timestamp,
            epoch: rec.epoch,
            acc_end: rec.acc_end,
            num: rec.num,
            den: rec.den().math()?,
            vest_flushed: booked.vest_flushed,
        });
    }
    // Every account-creation CPI precedes direct debit of program-owned escrow.
    let position_info = ctx.accounts.position.to_account_info();
    let vault_info = ctx.accounts.market_vault.to_account_info();
    lamports::move_lamports(
        &position_info,
        &ctx.accounts.keeper.to_account_info(),
        booked.keeper_payment,
    )?;
    lamports::move_lamports(&position_info, &vault_info, booked.lender_amount)?;
    ctx.accounts.market_vault.load_mut()?.set_vault(&vault);
    lender::check_vault_floor(&vault_info, lender::tracked(&vault)?)?;
    quote_finance::remove_exposure(&mut ctx.accounts.exposure, pos.q, pos.n_open)?;
    if ctx.accounts.exposure.q_open == 0 {
        ctx.accounts
            .exposure
            .close(ctx.accounts.borrower.to_account_info())?;
    }
    ctx.accounts
        .position
        .close(ctx.accounts.borrower.to_account_info())?;
    emit_cpi!(OverdueCollateralDefault {
        market: market_key,
        slot: clock.slot,
        ts: clock.unix_timestamp,
        position: position_key,
        borrower: ctx.accounts.borrower.key(),
        keeper: ctx.accounts.keeper.key(),
        q_lost: pos.q,
        default_at: pos.default_at,
        collateral: pos.v,
        keeper_payment: booked.keeper_payment,
        lender_amount: booked.lender_amount,
        borrower_amount: 0,
        rent_due: booked.rent_due,
        credited_retired: booked.credited_retired,
        vested: booked.vested,
        vest_flushed: booked.vest_flushed,
        share_epoch_after: pool.share_epoch,
        position_account_refund,
    });
    if let Some(event) = rolled {
        emit_cpi!(event);
    }
    Ok(())
}
