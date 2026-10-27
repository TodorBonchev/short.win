//! Accept an authenticated report into immutable unadmitted persistence; no financial action.
use crate::constants::{CONFIG_VERSION, MARKET_VERSION};
use crate::errors::ShortwinError;
use crate::state::{core_error, GlobalConfig, Market, PriceReport, PublishPriceReportArgs};
use anchor_lang::prelude::*;
use shortwin_price_report as core;

#[derive(Accounts)]
pub struct PublishPriceReport<'info> {
    pub publisher: Signer<'info>,
    #[account(seeds = [GlobalConfig::SEED], bump = config.load()?.bump,
        constraint = config.load()?.version == CONFIG_VERSION @ ShortwinError::ConfigVersion)]
    pub config: AccountLoader<'info, GlobalConfig>,
    #[account(constraint = market.load()?.is_pda(&market.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
        constraint = market.load()?.version == MARKET_VERSION @ ShortwinError::ConfigVersion)]
    pub market: AccountLoader<'info, Market>,
    #[account(mut, seeds = [PriceReport::SEED, market.key().as_ref(), &price_report.quantity.to_le_bytes(), &price_report.model_epoch.to_le_bytes()],
        bump = price_report.bump)]
    pub price_report: Box<Account<'info, PriceReport>>,
}
pub fn handler(ctx: Context<PublishPriceReport>, args: PublishPriceReportArgs) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        ShortwinError::AccountMismatch
    );
    // Exact account size: Anchor prefix decoding alone would tolerate an appended account tail.
    require_eq!(
        ctx.accounts.price_report.to_account_info().data_len(),
        PriceReport::SPACE,
        ShortwinError::AccountMismatch
    );
    let market = ctx.accounts.market.load()?;
    let mut state = ctx.accounts.price_report.restore(
        &ctx.accounts.config.key(),
        &ctx.accounts.market.key(),
        &market,
    )?;
    let report = args.core()?;
    let clock = Clock::get()?;
    let now = core::RuntimeClock {
        slot: clock.slot,
        unix_timestamp: clock.unix_timestamp,
    };
    // Signer<'info> enforces actual runtime transaction authorization, not a payload assertion.
    let signer = core::CallerAuthenticatedSigner::from_verified_runtime_signer(
        ctx.accounts.publisher.key().to_bytes(),
    );
    state.accept(signer, &report, now).map_err(|err| {
        msg!("Refused price report: {:?}", err);
        match err {
            core::Error::UnauthorizedPublisher | core::Error::PublisherMismatch => {
                error!(ShortwinError::Unauthorized)
            }
            _ => error!(ShortwinError::InvalidParams),
        }
    })?;
    core::codec::encode(&state, &mut ctx.accounts.price_report.state_bytes).map_err(core_error)?;
    Ok(())
}
