//! Authenticate one operator-attested current snapshot and private-history assertion.
use crate::constants::{CONFIG_VERSION, MARKET_VERSION};
use crate::errors::ShortwinError;
use crate::state::{
    snapshot_codec_error, GlobalConfig, Market, PriceSnapshot, PublishPriceSnapshotArgs,
};
use anchor_lang::prelude::*;
use shortwin_price_report::{self as core, trusted_snapshot as snapshot};

#[derive(Accounts)]
pub struct PublishPriceSnapshot<'info> {
    pub publisher: Signer<'info>,
    #[account(seeds=[GlobalConfig::SEED],bump=config.load()?.bump,
        constraint=config.load()?.version==CONFIG_VERSION @ ShortwinError::ConfigVersion)]
    pub config: AccountLoader<'info, GlobalConfig>,
    #[account(constraint=market.load()?.is_pda(&market.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
        constraint=market.load()?.version==MARKET_VERSION @ ShortwinError::ConfigVersion)]
    pub market: AccountLoader<'info, Market>,
    #[account(mut,seeds=[PriceSnapshot::SEED,market.key().as_ref(),&price_snapshot.quantity.to_le_bytes(),&price_snapshot.model_epoch.to_le_bytes()],bump=price_snapshot.bump)]
    pub price_snapshot: Box<Account<'info, PriceSnapshot>>,
}
pub fn handler(ctx: Context<PublishPriceSnapshot>, args: PublishPriceSnapshotArgs) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        ShortwinError::AccountMismatch
    );
    require_eq!(
        ctx.accounts.price_snapshot.to_account_info().data_len(),
        PriceSnapshot::SPACE,
        ShortwinError::AccountMismatch
    );
    let market = ctx.accounts.market.load()?;
    let mut state = ctx.accounts.price_snapshot.restore(
        &ctx.accounts.config.key(),
        &ctx.accounts.market.key(),
        &market,
    )?;
    let report = args.core(&state, ctx.accounts.publisher.key())?;
    let clock = Clock::get()?;
    let now = core::RuntimeClock {
        slot: clock.slot,
        unix_timestamp: clock.unix_timestamp,
    };
    let signer = core::CallerAuthenticatedSigner::from_verified_runtime_signer(
        ctx.accounts.publisher.key().to_bytes(),
    );
    state.accept(signer, &report, now).map_err(|err| {
        msg!("Refused trusted price snapshot: {:?}", err);
        match err {
            core::Error::UnauthorizedPublisher | core::Error::PublisherMismatch => {
                error!(ShortwinError::Unauthorized)
            }
            _ => error!(ShortwinError::InvalidParams),
        }
    })?;
    snapshot::codec::encode(&state, &mut ctx.accounts.price_snapshot.state_bytes)
        .map_err(snapshot_codec_error)?;
    Ok(())
}
