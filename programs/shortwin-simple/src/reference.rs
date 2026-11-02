//! Authenticated compact assertions from the trusted in-house reporter.
//! These bounds and completed history summaries are not on-chain proofs of Jupiter
//! API responses or of private observations. The program enforces identity, exact
//! quantity, sequence, freshness, and the declared summary's numerical domain.
use crate::errors::{CheckedExt, SimpleError};
use crate::events::ReferencePublished;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PublishReferenceArgs {
    pub quantity: u64,
    pub sequence: u64,
    pub source_slot: u64,
    pub source_ts: i64,
    pub finite: bool,
    pub live_bid_lower: u64,
    pub live_ask_upper: u64,
    pub history_ask_upper_max: u64,
    pub history_bid_lower_max: u64,
    pub acquisition_reference: u64,
    pub completed_epochs: u8,
    pub history_start_ts: i64,
    pub history_end_ts: i64,
    pub evidence_hash: [u8; 32],
    pub max_reference_rent: u64,
}
#[event_cpi]
#[derive(Accounts)]
#[instruction(args:PublishReferenceArgs)]
pub struct PublishReference<'info> {
    #[account(mut)]
    pub reporter: Signer<'info>,
    #[account(seeds=[CONFIG_SEED],bump=config.bump,
        constraint=config.reporter==reporter.key() @ SimpleError::Unauthorized)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds=[MARKET_SEED,market.mint.as_ref()],bump=market.bump,
        constraint=market.config==config.key() @ SimpleError::AccountMismatch)]
    pub market: Box<Account<'info, Market>>,
    #[account(seeds=[POLICY_SEED,config.key().as_ref(),&policy.policy_version.to_le_bytes()],bump=policy.bump,
        constraint=policy.config==config.key() @ SimpleError::AccountMismatch)]
    pub policy: Box<Account<'info, Policy>>,
    #[account(init_if_needed,payer=reporter,space=PriceReference::SPACE,
        seeds=[REFERENCE_SEED,market.key().as_ref(),policy.key().as_ref(),&args.quantity.to_le_bytes()],bump)]
    pub reference: Box<Account<'info, PriceReference>>,
    pub system_program: Program<'info, System>,
}

fn check_clock(
    source_slot: u64,
    source_ts: i64,
    terms: &FinancialPolicy,
    clock: &Clock,
) -> Result<()> {
    require!(
        source_slot > 0
            && source_ts > 0
            && source_slot <= clock.slot
            && source_ts <= clock.unix_timestamp,
        SimpleError::BadClock
    );
    let age_seconds = clock.unix_timestamp.checked_sub(source_ts).checked()?;
    let age_slots = clock.slot.checked_sub(source_slot).checked()?;
    require!(
        age_seconds <= i64::from(terms.reference_max_age_seconds)
            && age_slots <= terms.reference_max_age_slots,
        SimpleError::StaleReference
    );
    Ok(())
}
fn check_finite(reference: &PriceReference, terms: &FinancialPolicy) -> Result<()> {
    require!(reference.finite, SimpleError::UnknownReference);
    require!(
        reference.quantity > 0
            && reference.sequence > 0
            && reference.live_bid_lower > 0
            && reference.live_ask_upper >= reference.live_bid_lower
            && reference.history_bid_lower_max > 0
            && reference.history_ask_upper_max >= reference.history_bid_lower_max
            && reference.acquisition_reference > 0
            && reference.acquisition_reference <= reference.live_ask_upper
            && reference.acquisition_reference <= reference.history_ask_upper_max
            && reference.completed_epochs == terms.minimum_history_epochs
            && reference.evidence_hash != [0; 32],
        SimpleError::BadReference
    );
    // Conservative bid lower bound is the denominator of the relative live spread.
    let spread = u128::from(reference.live_ask_upper - reference.live_bid_lower) * 10_000;
    let limit = u128::from(reference.live_bid_lower) * u128::from(terms.max_spread_bps);
    require!(spread <= limit, SimpleError::BadReference);
    let epoch = i64::from(terms.history_epoch_seconds);
    let current_epoch_start = reference
        .source_ts
        .checked_div(epoch)
        .checked()?
        .checked_mul(epoch)
        .checked()?;
    let history_seconds = epoch
        .checked_mul(i64::from(reference.completed_epochs))
        .checked()?;
    let expected_start = current_epoch_start.checked_sub(history_seconds).checked()?;
    let expected_end = current_epoch_start.checked_sub(1).checked()?;
    require!(
        expected_start >= 0
            && reference.history_start_ts == expected_start
            && reference.history_end_ts == expected_end,
        SimpleError::BadReference
    );
    Ok(())
}

/// Accept old immutable policies for old loans, but only the current reporter epoch.
/// Publication is allowed during a price hold so an operator can restore fresh
/// references before governance clears the hold; priced economic actions stay held.
pub fn publish_reference(ctx: Context<PublishReference>, args: PublishReferenceArgs) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    ctx.accounts.config.validate()?;
    ctx.accounts.policy.validate(&ctx.accounts.config.key())?;
    require_eq!(
        ctx.accounts.market.version,
        STATE_VERSION,
        SimpleError::AccountMismatch
    );
    require!(
        args.quantity > 0 && args.sequence > 0,
        SimpleError::BadReference
    );
    let clock = Clock::get()?;
    check_clock(
        args.source_slot,
        args.source_ts,
        &ctx.accounts.policy.terms,
        &clock,
    )?;
    if ctx.accounts.reference.version == 0 {
        require_eq!(args.sequence, 1, SimpleError::WrongSequence);
        require_gte!(
            args.max_reference_rent,
            Rent::get()?.minimum_balance(PriceReference::SPACE),
            SimpleError::SetupRentAboveMax
        );
    } else {
        require!(
            ctx.accounts.reference.version == STATE_VERSION
                && ctx.accounts.reference.market == ctx.accounts.market.key()
                && ctx.accounts.reference.policy == ctx.accounts.policy.key()
                && ctx.accounts.reference.quantity == args.quantity
                && ctx.accounts.reference.bump == ctx.bumps.reference,
            SimpleError::AccountMismatch
        );
        require_eq!(
            args.sequence,
            ctx.accounts.reference.sequence.checked_add(1).checked()?,
            SimpleError::WrongSequence
        );
        require!(
            args.source_slot > ctx.accounts.reference.source_slot
                && args.source_ts >= ctx.accounts.reference.source_ts,
            SimpleError::WrongSequence
        );
    }
    let mut next = PriceReference {
        version: STATE_VERSION,
        bump: ctx.bumps.reference,
        market: ctx.accounts.market.key(),
        policy: ctx.accounts.policy.key(),
        quantity: args.quantity,
        reporter_epoch: ctx.accounts.config.reporter_epoch,
        sequence: args.sequence,
        finite: args.finite,
        source_slot: args.source_slot,
        source_ts: args.source_ts,
        live_bid_lower: args.live_bid_lower,
        live_ask_upper: args.live_ask_upper,
        history_ask_upper_max: args.history_ask_upper_max,
        history_bid_lower_max: args.history_bid_lower_max,
        acquisition_reference: args.acquisition_reference,
        completed_epochs: args.completed_epochs,
        history_start_ts: args.history_start_ts,
        history_end_ts: args.history_end_ts,
        evidence_hash: args.evidence_hash,
    };
    if next.finite {
        check_finite(&next, &ctx.accounts.policy.terms)?;
    } else {
        next.live_bid_lower = 0;
        next.live_ask_upper = 0;
        next.history_ask_upper_max = 0;
        next.history_bid_lower_max = 0;
        next.acquisition_reference = 0;
        next.completed_epochs = 0;
        next.history_start_ts = 0;
        next.history_end_ts = 0;
        next.evidence_hash = [0; 32];
    }
    ctx.accounts.reference.set_inner(next);
    emit_cpi!(ReferencePublished {
        reference: ctx.accounts.reference.key(),
        market: ctx.accounts.market.key(),
        policy: ctx.accounts.policy.key(),
        quantity: args.quantity,
        sequence: args.sequence,
        source_slot: args.source_slot,
        source_ts: args.source_ts
    });
    Ok(())
}

pub fn checked_reference(
    reference: &Account<'_, PriceReference>,
    market: &Pubkey,
    policy: &Account<'_, Policy>,
    config: &Account<'_, Config>,
    q: u64,
    minseq: u64,
    clock: &Clock,
) -> Result<()> {
    config.validate()?;
    policy.validate(&config.key())?;
    require!(!config.prices_paused, SimpleError::PricePaused);
    let (config_key, config_bump) = Pubkey::find_program_address(&[CONFIG_SEED], &crate::ID);
    require!(
        config.key() == config_key && config.bump == config_bump,
        SimpleError::AccountMismatch
    );
    let (policy_key, policy_bump) = Pubkey::find_program_address(
        &[
            POLICY_SEED,
            config.key().as_ref(),
            &policy.policy_version.to_le_bytes(),
        ],
        &crate::ID,
    );
    require!(
        policy.key() == policy_key && policy.bump == policy_bump,
        SimpleError::AccountMismatch
    );
    let (reference_key, reference_bump) = Pubkey::find_program_address(
        &[
            REFERENCE_SEED,
            market.as_ref(),
            policy.key().as_ref(),
            &q.to_le_bytes(),
        ],
        &crate::ID,
    );
    require!(
        reference.key() == reference_key
            && reference.bump == reference_bump
            && reference.version == STATE_VERSION
            && reference.market == *market
            && reference.policy == policy.key()
            && reference.quantity == q
            && q > 0,
        SimpleError::BadReference
    );
    require_eq!(
        reference.reporter_epoch,
        config.reporter_epoch,
        SimpleError::StaleReference
    );
    require_gte!(reference.sequence, minseq, SimpleError::WrongSequence);
    check_clock(
        reference.source_slot,
        reference.source_ts,
        &policy.terms,
        clock,
    )?;
    check_finite(reference, &policy.terms)
}
