//! Governance creates a prospective immutable policy. Does not activate a backend or open debt.
use crate::constants::{CONFIG_VERSION, MARKET_VERSION};
use crate::errors::ShortwinError;
use crate::state::{GlobalConfig, InitializeQuotePolicyArgs, Market, QuotePolicy};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(args:InitializeQuotePolicyArgs)]
pub struct InitializeQuotePolicy<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds=[GlobalConfig::SEED],bump=config.load()?.bump,
        constraint=config.load()?.version==CONFIG_VERSION @ ShortwinError::ConfigVersion,
        constraint=config.load()?.governance==authority.key() @ ShortwinError::Unauthorized)]
    pub config: AccountLoader<'info, GlobalConfig>,
    #[account(constraint=market.load()?.is_pda(&market.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
        constraint=market.load()?.version==MARKET_VERSION @ ShortwinError::ConfigVersion)]
    pub market: AccountLoader<'info, Market>,
    #[account(init,payer=authority,space=QuotePolicy::SPACE,
        seeds=[QuotePolicy::SEED,market.key().as_ref(),&args.model_epoch.to_le_bytes()],bump)]
    pub quote_policy: Box<Account<'info, QuotePolicy>>,
    pub system_program: Program<'info, System>,
}
pub fn handler(ctx: Context<InitializeQuotePolicy>, args: InitializeQuotePolicyArgs) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        ShortwinError::AccountMismatch
    );
    let market = ctx.accounts.market.load()?;
    let policy = QuotePolicy::from_args(
        ctx.accounts.config.key(),
        ctx.accounts.market.key(),
        &market,
        &args,
        ctx.bumps.quote_policy,
    )?;
    ctx.accounts.quote_policy.set_inner(*policy);
    // Deliberately no market mutation, finance adoption, source activation or debt creation.
    Ok(())
}

/// Read-only authenticated reference preflight. This is neither open admission nor
/// liquidation eligibility, and no field promises executable buyback liquidity.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteReferenceView {
    pub quantity: u64,
    pub source_epoch: u64,
    pub opening_upper: u64,
    pub lower_base: u64,
    pub acquisition_reference: u64,
}
#[derive(Accounts)]
pub struct CheckQuoteReference<'info> {
    #[account(seeds=[GlobalConfig::SEED],bump=config.load()?.bump,
        constraint=config.load()?.version==CONFIG_VERSION @ ShortwinError::ConfigVersion)]
    pub config: AccountLoader<'info, GlobalConfig>,
    #[account(constraint=market.load()?.is_pda(&market.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
        constraint=market.load()?.version==MARKET_VERSION @ ShortwinError::ConfigVersion)]
    pub market: AccountLoader<'info, Market>,
    /// CHECK: actual owner, exact size, discriminator, PDA and immutable bindings checked by bridge.
    pub quote_policy: UncheckedAccount<'info>,
    /// CHECK: canonical feed PDA, actual owner/size/codec, producer/global hash and freshness checked by bridge.
    pub price_snapshot: UncheckedAccount<'info>,
}
pub fn check_handler(
    ctx: Context<CheckQuoteReference>,
    quantity: u64,
    minimum_sequence: u64,
) -> Result<QuoteReferenceView> {
    require!(
        ctx.remaining_accounts.is_empty(),
        ShortwinError::AccountMismatch
    );
    let summary = crate::engine::quote_reference::summary_from_snapshot(
        &ctx.accounts.quote_policy.to_account_info(),
        &ctx.accounts.config,
        &ctx.accounts.market,
        quantity,
        &ctx.accounts.price_snapshot.to_account_info(),
        minimum_sequence,
    )?;
    msg!("Trusted operator reference only; no financial action authorized");
    Ok(QuoteReferenceView {
        quantity: summary.key().quantity(),
        source_epoch: summary.epoch(),
        opening_upper: summary.opening_upper(),
        lower_base: summary.lower_base(),
        acquisition_reference: summary.acquisition_reference(),
    })
}

/// A compromised reporter cannot pause or unpause this control. Governance and
/// emergency may pause; only governance may resume. No economic policy is changed.
#[derive(Accounts)]
pub struct SetQuotePricePause<'info> {
    pub authority: Signer<'info>,
    #[account(seeds=[GlobalConfig::SEED],bump=config.load()?.bump,
        constraint=config.load()?.version==CONFIG_VERSION @ ShortwinError::ConfigVersion)]
    pub config: AccountLoader<'info, GlobalConfig>,
    #[account(mut,constraint=market.load()?.is_pda(&market.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
        constraint=market.load()?.version==MARKET_VERSION @ ShortwinError::ConfigVersion)]
    pub market: AccountLoader<'info, Market>,
}
pub fn set_price_pause_handler(ctx: Context<SetQuotePricePause>, paused: bool) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        ShortwinError::AccountMismatch
    );
    let config = ctx.accounts.config.load()?;
    require!(
        ctx.accounts.authority.key() == config.governance
            || (paused && ctx.accounts.authority.key() == config.pause_authority),
        ShortwinError::Unauthorized
    );
    ctx.accounts
        .market
        .load_mut()?
        .set_flag(crate::state::MARKET_FLAG_QUOTE_PRICE_PAUSED, paused);
    msg!("Quote price action pause set to {}", paused);
    Ok(())
}
