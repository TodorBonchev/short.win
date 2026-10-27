//! Borrower-authorized whole-debt in-kind repayment, without a route or price read.
//!
//! All q tokens must actually reach the lender vault. Accrue under the existing
//! stored rent basis, charge full accrued rent within the borrower's explicit bound,
//! then use the ordinary close waterfall with zero protocol swap cost. Borrower
//! acquisition cost is unknown; emit Repay, not a zero-cost economic Close profit.
//! No price, venue, pause, oracle-cache or hosted-order account can authorize this path.
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};
use shortwin_math::{rent, waterfall};

use crate::constants::{CONFIG_VERSION, MARKET_VERSION};
use crate::engine::guard::{mint_guard, MintPath};
use crate::engine::{lender, prologue, trader, waterfall as engine_waterfall};
use crate::errors::{CheckedExt, MathResultExt, ShortwinError};
use crate::events::{MintSafetyTripped, Repay};
use crate::state::{
    GlobalConfig, Market, MarketVault, ShortPosition, WalletExposure, POOL_VAULT_SEED,
};

#[event_cpi]
#[derive(Accounts)]
pub struct RepayShort<'info> {
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
    #[account(mut, close = borrower,
        has_one = borrower @ ShortwinError::AccountMismatch,
        has_one = market @ ShortwinError::AccountMismatch,
        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds)]
    pub position: Box<Account<'info, ShortPosition>>,
    #[account(mut, seeds = [WalletExposure::SEED, market.key().as_ref(), borrower.key().as_ref()],
        bump = exposure.bump, has_one = market @ ShortwinError::AccountMismatch,
        constraint = exposure.wallet == borrower.key() @ ShortwinError::AccountMismatch)]
    pub exposure: Box<Account<'info, WalletExposure>>,
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

pub fn handler<'info>(ctx: Context<'info, RepayShort<'info>>, max_rent: u64) -> Result<()> {
    // No caller-supplied route, hooks, reference or source history enters this instruction.
    require!(
        ctx.remaining_accounts.is_empty(),
        ShortwinError::AccountMismatch
    );
    let clock = Clock::get()?;
    let market_key = ctx.accounts.market.key();
    let position_key = ctx.accounts.position.key();
    let pos = (**ctx.accounts.position).clone();
    let config = ctx.accounts.config.load()?;
    let (now, mut pool, mut vault, r_due, mint_ok) = {
        let mut market = ctx.accounts.market.load_mut()?;
        let mint_ok = mint_guard(
            &market,
            &ctx.accounts.mint.to_account_info(),
            ctx.accounts.token_program.key,
            MintPath::Exit,
        )?;
        let mut vault = ctx.accounts.market_vault.load()?.vault();
        // No fabricated venue state. Roll/accrue stored state, but record no quote or
        // route/no-route evidence. Repayment does not depend on new price history.
        let pro = prologue::run(
            &config,
            &market_key,
            &mut market,
            &mut vault,
            None,
            clock.unix_timestamp,
            clock.slot,
        )?;
        let r_due = rent::due(&pos.pos_rent(), &market.basis()).math()?;
        require_gte!(max_rent, r_due, ShortwinError::RepaymentRentAboveMax);
        require_gte!(pos.v, r_due, ShortwinError::InsufficientVault);
        (pro.now, pro.pool, vault, r_due, mint_ok)
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
    // Net deltas, not a successful token CPI or pre-existing donated vault balance.
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

    let payout = waterfall::close(pos.v, 0, r_due, u32::from(pos.rent_protocol_bps)).math()?;
    let position_info = ctx.accounts.position.to_account_info();
    engine_waterfall::pay_vault(
        &position_info,
        &ctx.accounts.market_vault.to_account_info(),
        &payout,
    )?;
    let (tokens_free, tracked) = {
        let mut market = ctx.accounts.market.load_mut()?;
        let basis = market.basis();
        engine_waterfall::book_repay(
            &mut pool,
            &mut vault,
            now,
            config.params.lump_vest_s,
            &pos,
            &basis,
            &payout,
        )?;
        let mut aggr = market.aggr();
        engine_waterfall::remove_position(&mut market, &mut aggr, &pos)?;
        market.set_aggr(&aggr);
        market.set_pool(&pool);
        ctx.accounts.market_vault.load_mut()?.set_vault(&vault);
        (pool.tokens_free, lender::tracked(&vault)?)
    };
    require_gte!(after.amount, tokens_free, ShortwinError::AccountMismatch);
    lender::check_vault_floor(&ctx.accounts.market_vault.to_account_info(), tracked)?;
    let exposure = &mut ctx.accounts.exposure;
    exposure.q_open = exposure.q_open.checked_sub(pos.q).or_math()?;
    if exposure.q_open == 0 {
        exposure.close(ctx.accounts.borrower.to_account_info())?;
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
        borrower_payout: payout.borrower
    });
    Ok(())
}
