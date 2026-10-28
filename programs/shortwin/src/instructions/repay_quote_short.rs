//! Quote-native full-token borrower repayment. No price service or history needed.
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};
use shortwin_math::{quote_rent, waterfall};

use crate::constants::{CONFIG_VERSION, MARKET_VERSION};
use crate::engine::guard::{mint_guard, MintPath};
use crate::engine::{lender, quote_finance, trader, waterfall as engine_waterfall};
use crate::errors::{CheckedExt, MathResultExt, ShortwinError};
use crate::events::{MintSafetyTripped, Repay};
use crate::lamports;
use crate::state::{
    GlobalConfig, Market, MarketVault, QuoteFinanceExposure, QuoteFinanceMarket,
    QuoteFinancePosition, POOL_VAULT_SEED,
};

#[event_cpi]
#[derive(Accounts)]
pub struct RepayQuoteShort<'info> {
    #[account(mut)]
    pub borrower: Signer<'info>,
    #[account(seeds = [GlobalConfig::SEED], bump = config.load()?.bump,
        constraint = config.load()?.version == CONFIG_VERSION @ ShortwinError::ConfigVersion)]
    pub config: AccountLoader<'info, GlobalConfig>,
    #[account(mut, seeds = [Market::SEED, mint.key().as_ref()], bump = market.load()?.bump,
        constraint = market.load()?.version == MARKET_VERSION @ ShortwinError::ConfigVersion,
        has_one = mint @ ShortwinError::AccountMismatch,
        has_one = market_vault @ ShortwinError::AccountMismatch,
        has_one = pool_vault @ ShortwinError::AccountMismatch,
        has_one = token_program @ ShortwinError::AccountMismatch)]
    pub market: AccountLoader<'info, Market>,
    #[account(mut, seeds = [QuoteFinanceMarket::SEED, market.key().as_ref()], bump = finance_market.bump,
        has_one = market @ ShortwinError::AccountMismatch,
        has_one = config @ ShortwinError::AccountMismatch)]
    pub finance_market: Box<Account<'info, QuoteFinanceMarket>>,
    #[account(mut, close = borrower,
        has_one = borrower @ ShortwinError::AccountMismatch,
        has_one = market @ ShortwinError::AccountMismatch,
        has_one = finance_market @ ShortwinError::AccountMismatch,
        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds)]
    pub position: Box<Account<'info, QuoteFinancePosition>>,
    #[account(mut, seeds = [QuoteFinanceExposure::SEED, market.key().as_ref(), borrower.key().as_ref()],
        bump = exposure.bump, has_one = market @ ShortwinError::AccountMismatch,
        constraint = exposure.wallet == borrower.key() @ ShortwinError::AccountMismatch)]
    pub exposure: Box<Account<'info, QuoteFinanceExposure>>,
    #[account(mint::token_program = token_program)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, token::mint = mint, token::authority = borrower, token::token_program = token_program,
        constraint = borrower_token_account.key() != pool_vault.key() @ ShortwinError::AccountMismatch)]
    pub borrower_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, seeds = [POOL_VAULT_SEED, market.key().as_ref()], bump = market.load()?.pool_vault_bump,
        token::mint = mint, token::authority = market, token::token_program = token_program)]
    pub pool_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, seeds = [MarketVault::SEED, market.key().as_ref()], bump = market_vault.load()?.bump)]
    pub market_vault: AccountLoader<'info, MarketVault>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler<'info>(ctx: Context<'info, RepayQuoteShort<'info>>, max_rent: u64) -> Result<()> {
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
    let floor = lamports::rent_min(QuoteFinancePosition::SPACE)?
        .checked_add(pos.v)
        .or_math()?;
    require_gte!(
        ctx.accounts.position.to_account_info().lamports(),
        floor,
        ShortwinError::InsufficientVault
    );
    let (now, mut pool, mut vault, r_due, protocol_bps, mint_ok) = {
        let mut market = ctx.accounts.market.load_mut()?;
        let mint_ok = mint_guard(
            &market,
            &ctx.accounts.mint.to_account_info(),
            ctx.accounts.token_program.key,
            MintPath::Exit,
        )?;
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
        let checkpoint = ctx.accounts.finance_market.restore(
            &ctx.accounts.config.key(),
            &market_key,
            &market,
        )?;
        let rent_position = pos.restore(&finance_key, &ctx.accounts.finance_market, &checkpoint)?;
        let r_due = quote_rent::due(&rent_position, &checkpoint.basis()).math()?;
        require_gte!(max_rent, r_due, ShortwinError::RepaymentRentAboveMax);
        require_gte!(pos.v, r_due, ShortwinError::InsufficientVault);
        (
            now,
            pool,
            vault,
            r_due,
            rent_position.protocol_bps(),
            mint_ok,
        )
    };
    let before = trader::token_state(&ctx.accounts.pool_vault.to_account_info())?;
    let source_before =
        trader::token_state(&ctx.accounts.borrower_token_account.to_account_info())?;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.borrower_token_account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.pool_vault.to_account_info(),
                authority: ctx.accounts.borrower.to_account_info(),
            },
        ),
        pos.q,
        ctx.accounts.mint.decimals,
    )?;
    let after = trader::token_state(&ctx.accounts.pool_vault.to_account_info())?;
    let source_after = trader::token_state(&ctx.accounts.borrower_token_account.to_account_info())?;
    require_eq!(
        after.amount.checked_sub(before.amount).or_math()?,
        pos.q,
        ShortwinError::VenueResultMismatch
    );
    require_eq!(
        source_before
            .amount
            .checked_sub(source_after.amount)
            .or_math()?,
        pos.q,
        ShortwinError::VenueResultMismatch
    );
    let mut expected_pool = before;
    expected_pool.amount = after.amount;
    let mut expected_source = source_before;
    expected_source.amount = source_after.amount;
    require!(
        expected_pool == after && expected_source == source_after,
        ShortwinError::AuthorityTampered
    );
    let payout = waterfall::close(pos.v, 0, r_due, protocol_bps).math()?;
    engine_waterfall::pay_vault(
        &ctx.accounts.position.to_account_info(),
        &ctx.accounts.market_vault.to_account_info(),
        &payout,
    )?;
    let (tokens_free, tracked) = {
        let mut market = ctx.accounts.market.load_mut()?;
        quote_finance::book_token_repayment(
            &ctx.accounts.config.key(),
            &market_key,
            &finance_key,
            &mut market,
            &mut ctx.accounts.finance_market,
            &mut pool,
            &mut vault,
            now,
            &pos,
            &payout,
        )?;
        ctx.accounts.market_vault.load_mut()?.set_vault(&vault);
        (pool.tokens_free, lender::tracked(&vault)?)
    };
    require_gte!(after.amount, tokens_free, ShortwinError::AccountMismatch);
    lender::check_vault_floor(&ctx.accounts.market_vault.to_account_info(), tracked)?;
    quote_finance::remove_exposure(&mut ctx.accounts.exposure, pos.q, pos.n_open)?;
    if ctx.accounts.exposure.q_open == 0 {
        ctx.accounts
            .exposure
            .close(ctx.accounts.borrower.to_account_info())?;
    }
    if !mint_ok {
        emit!(MintSafetyTripped {
            market: market_key,
            slot: clock.slot,
            ts: clock.unix_timestamp,
            mint: ctx.accounts.mint.key()
        });
    }
    emit_cpi!(Repay {
        market: market_key,
        slot: clock.slot,
        ts: clock.unix_timestamp,
        position: position_key,
        borrower: ctx.accounts.borrower.key(),
        q: pos.q,
        rent: payout.rent,
        rent_protocol: payout.rent_protocol,
        borrower_payout: payout.borrower,
    });
    Ok(())
}
