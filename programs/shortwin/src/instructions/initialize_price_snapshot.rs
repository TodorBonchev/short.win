//! Governance initializes an immutable trusted-history cache. This does not enable a market.
use crate::constants::{CONFIG_VERSION, MARKET_VERSION, TOKEN_PROGRAM_ID, WSOL_MINT};
use crate::errors::ShortwinError;
use crate::state::{
    snapshot_codec_error, GlobalConfig, InitializePriceSnapshotArgs, Market, PriceSnapshot,
    PRICE_SNAPSHOT_VERSION,
};
use anchor_lang::prelude::*;
use shortwin_price_report::{self as core, trusted_snapshot as snapshot};

#[derive(Accounts)]
#[instruction(args: InitializePriceSnapshotArgs)]
pub struct InitializePriceSnapshot<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds=[GlobalConfig::SEED], bump=config.load()?.bump,
        constraint=config.load()?.version==CONFIG_VERSION @ ShortwinError::ConfigVersion,
        constraint=config.load()?.governance==authority.key() @ ShortwinError::Unauthorized)]
    pub config: AccountLoader<'info, GlobalConfig>,
    #[account(constraint=market.load()?.is_pda(&market.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
        constraint=market.load()?.version==MARKET_VERSION @ ShortwinError::ConfigVersion)]
    pub market: AccountLoader<'info, Market>,
    #[account(init,payer=authority,space=PriceSnapshot::SPACE,
        seeds=[PriceSnapshot::SEED,market.key().as_ref(),&args.quantity.to_le_bytes(),&args.model.epoch.to_le_bytes()],bump)]
    pub price_snapshot: Box<Account<'info, PriceSnapshot>>,
    pub system_program: Program<'info, System>,
}
pub fn handler(
    ctx: Context<InitializePriceSnapshot>,
    args: InitializePriceSnapshotArgs,
) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        ShortwinError::AccountMismatch
    );
    require!(
        args.producer_commitment != [0; 32],
        ShortwinError::InvalidParams
    );
    let count = usize::from(args.publisher_count);
    require!(
        count > 0 && count <= core::MAX_PUBLISHERS,
        ShortwinError::InvalidParams
    );
    let mut publishers = [[0; 32]; core::MAX_PUBLISHERS];
    for (index, key) in args.publishers.iter().enumerate() {
        if index < count {
            publishers[index] = key.to_bytes();
        } else {
            require_keys_eq!(*key, Pubkey::default(), ShortwinError::InvalidParams);
        }
    }
    let market = ctx.accounts.market.load()?;
    let configuration = core::Configuration::new(
        core::Domain {
            genesis_hash: args.genesis_hash,
            program: crate::ID.to_bytes(),
            config: ctx.accounts.config.key().to_bytes(),
            market: ctx.accounts.market.key().to_bytes(),
            base_mint: market.mint.to_bytes(),
            base_token_program: market.token_program.to_bytes(),
            base_decimals: market.decimals,
            quote_mint: WSOL_MINT.to_bytes(),
            quote_token_program: TOKEN_PROGRAM_ID.to_bytes(),
            quote_decimals: 9,
        },
        args.quantity,
        args.model.core(),
        1,
        &publishers[..count],
        args.policy.core(),
    )
    .map_err(|_| error!(ShortwinError::InvalidParams))?;
    let clock = Clock::get()?;
    let state = Box::new(
        snapshot::SnapshotState::new(
            configuration,
            core::RuntimeClock {
                slot: clock.slot,
                unix_timestamp: clock.unix_timestamp,
            },
        )
        .map_err(|_| error!(ShortwinError::InvalidParams))?,
    );
    let account = &mut ctx.accounts.price_snapshot;
    account.version = PRICE_SNAPSHOT_VERSION;
    account.bump = ctx.bumps.price_snapshot;
    account.market = ctx.accounts.market.key();
    account.quantity = args.quantity;
    account.model_epoch = args.model.epoch;
    account.producer_commitment = args.producer_commitment;
    snapshot::codec::encode(&state, &mut account.state_bytes).map_err(snapshot_codec_error)?;
    Ok(())
}
