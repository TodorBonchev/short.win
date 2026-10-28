//! Whole-position exits. The fixed fee is paid at open and is never refunded.
use crate::{
    errors::{CheckedExt, SimpleError},
    events, finance, reference,
    route::{self, ReviewedDeployment, RouteAccounts, RouteArgs},
    state::*,
};
use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};
use anchor_spl::{
    token::Token,
    token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CloseArgs {
    pub max_buyback_cost: u64,
    pub temporary_funding: u64,
    pub route: RouteArgs,
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ForcedExitArgs {
    pub max_buyback_cost: u64,
    pub temporary_funding: u64,
    pub minimum_reference_sequence: u64,
    pub route: RouteArgs,
}

/// State only: deterministic default never needs a mint, token vault, price or Jupiter account.
#[derive(Accounts)]
pub struct PositionAccounts<'info> {
    #[account(seeds=[CONFIG_SEED],bump=config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds=[POLICY_SEED,config.key().as_ref(),&policy.policy_version.to_le_bytes()],bump=policy.bump,
        constraint=policy.config==config.key() @ SimpleError::AccountMismatch)]
    pub policy: Box<Account<'info, Policy>>,
    #[account(seeds=[MARKET_SEED,market.mint.as_ref()],bump=market.bump,
        constraint=market.config==config.key() @ SimpleError::AccountMismatch)]
    pub market: Box<Account<'info, Market>>,
    #[account(mut,seeds=[POOL_SEED,market.key().as_ref(),&pool.generation.to_le_bytes()],bump=pool.bump,
        constraint=pool.market==market.key() @ SimpleError::AccountMismatch,
        constraint=pool.mint==market.mint @ SimpleError::AccountMismatch,
        constraint=pool.token_program==market.token_program @ SimpleError::AccountMismatch)]
    pub pool: Box<Account<'info, PoolGeneration>>,
    #[account(mut,seeds=[LOAN_SEED,pool.key().as_ref(),borrower.key().as_ref(),&loan.nonce.to_le_bytes()],bump=loan.bump,
        constraint=loan.pool==pool.key() @ SimpleError::AccountMismatch,
        constraint=loan.market==market.key() @ SimpleError::AccountMismatch,
        constraint=loan.policy==policy.key() @ SimpleError::AccountMismatch,
        constraint=loan.borrower==borrower.key() @ SimpleError::AccountMismatch)]
    pub loan: Box<Account<'info, Loan>>,
    #[account(mut,seeds=[EXPOSURE_SEED,pool.key().as_ref(),borrower.key().as_ref()],bump=exposure.bump,
        constraint=exposure.pool==pool.key() @ SimpleError::AccountMismatch,
        constraint=exposure.borrower==borrower.key() @ SimpleError::AccountMismatch)]
    pub exposure: Box<Account<'info, Exposure>>,
    /// CHECK: exact stored borrower receives all account rent and untracked donations; forced exits need no signature.
    #[account(mut)]
    pub borrower: UncheckedAccount<'info>,
}
#[derive(Accounts)]
pub struct LoanTokenAccounts<'info> {
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Interface<'info, TokenInterface>,
}
#[derive(Accounts)]
pub struct RouteExecution<'info> {
    /// CHECK: route binds the isolated loan-local authority and exact native setup budget.
    pub execution_authority: UncheckedAccount<'info>,
    /// CHECK: route creates, checks and closes the exact loan-local scratch account.
    #[account(mut)]
    pub execution_base: UncheckedAccount<'info>,
    /// CHECK: route creates, checks and closes the exact loan-local scratch account.
    #[account(mut)]
    pub execution_wsol: UncheckedAccount<'info>,
    /// CHECK: route checks the canonical readonly native mint.
    pub wsol_mint: UncheckedAccount<'info>,
    pub wsol_token_program: Program<'info, Token>,
    /// CHECK: route checks the canonical current reviewed loader instance.
    pub jupiter: UncheckedAccount<'info>,
    /// CHECK: route checks the canonical current reviewed loader instance.
    pub jupiter_program_data: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
#[event_cpi]
#[derive(Accounts)]
pub struct Repay<'info> {
    pub actor: Signer<'info>,
    pub position: PositionAccounts<'info>,
    pub token: LoanTokenAccounts<'info>,
    #[account(mut)]
    pub source: Box<InterfaceAccount<'info, TokenAccount>>,
}
#[event_cpi]
#[derive(Accounts)]
pub struct ClosePosition<'info> {
    #[account(mut)]
    pub actor: Signer<'info>,
    pub position: PositionAccounts<'info>,
    pub token: LoanTokenAccounts<'info>,
    pub execution: RouteExecution<'info>,
}
#[event_cpi]
#[derive(Accounts)]
pub struct AddMargin<'info> {
    #[account(mut)]
    pub actor: Signer<'info>,
    pub position: PositionAccounts<'info>,
    pub system_program: Program<'info, System>,
}
#[event_cpi]
#[derive(Accounts)]
pub struct ArmLiquidation<'info> {
    pub actor: Signer<'info>,
    pub position: PositionAccounts<'info>,
    pub reference: Box<Account<'info, PriceReference>>,
}
#[event_cpi]
#[derive(Accounts)]
pub struct ForceExit<'info> {
    #[account(mut)]
    pub actor: Signer<'info>,
    pub position: PositionAccounts<'info>,
    pub token: LoanTokenAccounts<'info>,
    pub execution: RouteExecution<'info>,
    pub reference: Box<Account<'info, PriceReference>>,
}
#[event_cpi]
#[derive(Accounts)]
pub struct DefaultLoan<'info> {
    #[account(mut)]
    pub actor: Signer<'info>,
    pub position: PositionAccounts<'info>,
}
fn validate_position(p: &PositionAccounts<'_>, clock: &Clock) -> Result<()> {
    p.config.validate()?;
    p.policy.validate(&p.config.key())?;
    require!(
        p.market.version == STATE_VERSION
            && p.pool.version == STATE_VERSION
            && p.loan.version == STATE_VERSION
            && p.exposure.version == STATE_VERSION,
        SimpleError::AccountMismatch
    );
    require!(
        p.loan.q > 0
            && p.loan.collateral > 0
            && p.loan.opening_notional > 0
            && p.loan.opened_at <= clock.unix_timestamp
            && p.loan.deadline > p.loan.opened_at
            && p.loan.default_at > p.loan.deadline
            && !p.pool.retired,
        SimpleError::AccountMismatch
    );
    require_gte!(p.exposure.q, p.loan.q, SimpleError::AccountMismatch);
    require_gte!(
        p.exposure.notional,
        p.loan.opening_notional,
        SimpleError::AccountMismatch
    );
    require_gte!(
        p.pool.booked_notional_out,
        p.loan.opening_notional,
        SimpleError::AccountMismatch
    );
    shortwin_pooled_core::validate_pool(&p.pool.core()).map_err(crate::lender::accounting_error)?;
    require_gte!(
        p.loan.to_account_info().lamports(),
        Rent::get()?
            .minimum_balance(Loan::SPACE)
            .checked_add(p.loan.collateral)
            .checked()?,
        SimpleError::BalanceMismatch
    );
    crate::lender::require_sol_reserve(&p.pool.to_account_info(), p.pool.sol_reserve)?;
    for key in [
        p.config.key(),
        p.policy.key(),
        p.market.key(),
        p.pool.key(),
        p.loan.key(),
        p.exposure.key(),
    ] {
        require_keys_neq!(p.borrower.key(), key, SimpleError::AccountMismatch);
    }
    Ok(())
}
fn before_default(p: &PositionAccounts<'_>, clock: &Clock) -> Result<()> {
    require!(
        clock.unix_timestamp < p.loan.default_at,
        SimpleError::DefaultRequired
    );
    Ok(())
}
fn validate_tokens(p: &PositionAccounts<'_>, t: &LoanTokenAccounts<'_>) -> Result<()> {
    require_keys_eq!(t.mint.key(), p.pool.mint, SimpleError::AccountMismatch);
    require_keys_eq!(
        t.token_program.key(),
        p.pool.token_program,
        SimpleError::AccountMismatch
    );
    let expected =
        Pubkey::find_program_address(&[TOKEN_VAULT_SEED, p.pool.key().as_ref()], &crate::ID).0;
    require_keys_eq!(t.token_vault.key(), expected, SimpleError::AccountMismatch);
    require!(
        t.mint.to_account_info().owner == &t.token_program.key()
            && t.token_vault.to_account_info().owner == &t.token_program.key()
            && t.token_vault.mint == t.mint.key()
            && t.token_vault.owner == p.pool.key(),
        SimpleError::AccountMismatch
    );
    require_gte!(
        t.token_vault.amount,
        p.pool.cash,
        SimpleError::BalanceMismatch
    );
    Ok(())
}
fn keeper(p: &PositionAccounts<'_>, actor: &Pubkey) -> Result<()> {
    for key in [
        p.borrower.key(),
        p.config.key(),
        p.policy.key(),
        p.market.key(),
        p.pool.key(),
        p.loan.key(),
        p.exposure.key(),
    ] {
        require_keys_neq!(*actor, key, SimpleError::AccountMismatch);
    }
    Ok(())
}
fn clear_arm(loan: &mut Loan) {
    loan.cr_arm_timestamp = 0;
    loan.cr_arm_slot = 0;
    loan.cr_arm_sequence = 0;
    loan.cr_arm_reporter_epoch = 0;
}
fn retire(p: &mut PositionAccounts<'_>, surplus: u64, lender_recovery: Option<u64>) -> Result<()> {
    let mut core = p.pool.core();
    if let Some(sol) = lender_recovery {
        shortwin_pooled_core::default(&mut core, p.loan.q, sol)
    } else {
        shortwin_pooled_core::repay(&mut core, p.loan.q)
    }
    .map_err(crate::lender::accounting_error)?;
    if surplus > 0 {
        shortwin_pooled_core::credit_tokens(&mut core, surplus)
            .map_err(crate::lender::accounting_error)?;
    }
    p.pool.store(&core)?;
    p.pool.booked_notional_out = p
        .pool
        .booked_notional_out
        .checked_sub(p.loan.opening_notional)
        .checked()?;
    p.exposure.q = p.exposure.q.checked_sub(p.loan.q).checked()?;
    p.exposure.notional = p
        .exposure
        .notional
        .checked_sub(p.loan.opening_notional)
        .checked()?;
    require!(
        (p.exposure.q == 0) == (p.exposure.notional == 0),
        SimpleError::BalanceMismatch
    );
    Ok(())
}
fn close_resolved(p: &mut PositionAccounts<'_>) -> Result<()> {
    if p.exposure.q == 0 {
        p.exposure.close(p.borrower.to_account_info())?;
    }
    p.loan.close(p.borrower.to_account_info())
}
fn normalized_debt_cost(spent: u64, quantity: u64, received: u64) -> Result<u64> {
    require!(
        quantity > 0 && received >= quantity,
        SimpleError::RouteOutputShort
    );
    let numerator = u128::from(spent)
        .checked_mul(u128::from(quantity))
        .checked()?;
    u64::try_from(numerator / u128::from(received)).map_err(|_| error!(SimpleError::Arithmetic))
}
fn bad_reference(p: &PositionAccounts<'_>, r: &PriceReference) -> Result<bool> {
    Ok(finance::unhealthy(
        p.loan.collateral,
        r.live_bid_lower,
        p.policy.terms.cr_liq_bps,
    )? && finance::unhealthy(
        p.loan.collateral,
        r.history_bid_lower_max,
        p.policy.terms.cr_liq_bps,
    )?)
}
#[derive(Clone, Copy)]
struct Observation {
    timestamp: i64,
    slot: u64,
    sequence: u64,
    reporter_epoch: u64,
}
fn confirmation_ready(
    arm: Observation,
    observed: Observation,
    clock: &Clock,
    seconds: u32,
    slots: u64,
) -> Result<bool> {
    let earliest_ts = arm.timestamp.checked_add(i64::from(seconds)).checked()?;
    let earliest_slot = arm.slot.checked_add(slots).checked()?;
    Ok(arm.sequence > 0
        && arm.reporter_epoch == observed.reporter_epoch
        && observed.sequence > arm.sequence
        && observed.timestamp >= earliest_ts
        && observed.slot >= earliest_slot
        && clock.unix_timestamp >= earliest_ts
        && clock.slot >= earliest_slot)
}
fn confirmed(p: &PositionAccounts<'_>, r: &PriceReference, clock: &Clock) -> Result<()> {
    let arm = Observation {
        timestamp: p.loan.cr_arm_timestamp,
        slot: p.loan.cr_arm_slot,
        sequence: p.loan.cr_arm_sequence,
        reporter_epoch: p.loan.cr_arm_reporter_epoch,
    };
    let observed = Observation {
        timestamp: r.source_ts,
        slot: r.source_slot,
        sequence: r.sequence,
        reporter_epoch: p.config.reporter_epoch,
    };
    require!(
        confirmation_ready(
            arm,
            observed,
            clock,
            p.policy.terms.cr_confirmation_delay_seconds,
            p.policy.terms.cr_confirmation_delay_slots
        )?,
        SimpleError::ConfirmationPending
    );
    Ok(())
}
fn pay<'info>(
    actor: &Signer<'info>,
    loan: AccountInfo<'info>,
    system: &Program<'info, System>,
    amount: u64,
) -> Result<()> {
    if amount > 0 {
        system_program::transfer(
            CpiContext::new(
                system.key(),
                Transfer {
                    from: actor.to_account_info(),
                    to: loan,
                },
            ),
            amount,
        )?;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn buy<'info>(
    p: &PositionAccounts<'info>,
    t: &LoanTokenAccounts<'info>,
    e: &RouteExecution<'info>,
    actor: &Signer<'info>,
    remaining: &[AccountInfo<'info>],
    args: &RouteArgs,
    maximum: u64,
    temporary_max: u64,
) -> Result<route::Buyback> {
    let temporary = route::temporary_advance(args)?;
    require_gte!(temporary_max, temporary, SimpleError::FundingAboveMax);
    pay(
        actor,
        p.loan.to_account_info(),
        &e.system_program,
        temporary,
    )?;
    let loan = p.loan.to_account_info();
    let pool = p.pool.to_account_info();
    let vault = t.token_vault.to_account_info();
    let mint = t.mint.to_account_info();
    let token = t.token_program.to_account_info();
    let auth = e.execution_authority.to_account_info();
    let base = e.execution_base.to_account_info();
    let wsol = e.execution_wsol.to_account_info();
    let wm = e.wsol_mint.to_account_info();
    let wt = e.wsol_token_program.to_account_info();
    let system = e.system_program.to_account_info();
    let jup = e.jupiter.to_account_info();
    let pd = e.jupiter_program_data.to_account_info();
    let forbidden = [
        p.borrower.key(),
        actor.key(),
        p.config.key(),
        p.policy.key(),
        p.market.key(),
        p.exposure.key(),
        p.config.fee_recipient,
    ];
    let accounts = RouteAccounts {
        loan: &loan,
        pool: &pool,
        token_vault: &vault,
        mint: &mint,
        base_program: &token,
        execution_authority: &auth,
        execution_base: &base,
        execution_wsol: &wsol,
        wsol_mint: &wm,
        wsol_token_program: &wt,
        system_program: &system,
        jupiter: &jup,
        jupiter_program_data: &pd,
        forbidden: &forbidden,
    };
    let result = route::buyback(
        &accounts,
        remaining,
        args,
        maximum,
        p.loan.q,
        Rent::get()?.minimum_balance(Loan::SPACE),
        ReviewedDeployment {
            deploy_slot: p.config.jupiter_deploy_slot,
            programdata_len: p.config.jupiter_program_data_len,
            elf_sha256: p.config.jupiter_elf_hash,
        },
    )?;
    finance::move_sol(
        &loan,
        &actor.to_account_info(),
        temporary.checked_sub(result.setup_spent).checked()?,
    )?;
    if args.setup_lamports > 0 {
        msg!(
            "RouteSetup actor={} allowance={} spent={}",
            actor.key(),
            args.setup_lamports,
            result.setup_spent
        );
    }
    Ok(result)
}

pub fn repay(ctx: Context<Repay>) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    let a = &mut *ctx.accounts;
    let clock = Clock::get()?;
    validate_position(&a.position, &clock)?;
    before_default(&a.position, &clock)?;
    validate_tokens(&a.position, &a.token)?;
    require_keys_eq!(
        a.actor.key(),
        a.position.borrower.key(),
        SimpleError::Unauthorized
    );
    require!(
        a.source.to_account_info().owner == &a.token.token_program.key()
            && a.source.mint == a.token.mint.key()
            && a.source.owner == a.actor.key()
            && a.source.key() != a.token.token_vault.key(),
        SimpleError::AccountMismatch
    );
    let before = a.token.token_vault.amount;
    let source_before = a.source.amount;
    token_interface::transfer_checked(
        CpiContext::new(
            a.token.token_program.key(),
            TransferChecked {
                from: a.source.to_account_info(),
                mint: a.token.mint.to_account_info(),
                to: a.token.token_vault.to_account_info(),
                authority: a.actor.to_account_info(),
            },
        ),
        a.position.loan.q,
        a.token.mint.decimals,
    )?;
    a.token.token_vault.reload()?;
    a.source.reload()?;
    require_eq!(
        a.token.token_vault.amount.checked_sub(before).checked()?,
        a.position.loan.q,
        SimpleError::BalanceMismatch
    );
    require_eq!(
        source_before.checked_sub(a.source.amount).checked()?,
        a.position.loan.q,
        SimpleError::BalanceMismatch
    );
    retire(&mut a.position, 0, None)?;
    let event = events::Repaid {
        pool: a.position.pool.key(),
        loan: a.position.loan.key(),
        borrower: a.position.borrower.key(),
        quantity: a.position.loan.q,
        collateral_refund: a.position.loan.collateral,
        slot: clock.slot,
        ts: clock.unix_timestamp,
    };
    close_resolved(&mut a.position)?;
    emit_cpi!(event);
    Ok(())
}
pub fn close_position<'info>(
    ctx: Context<'info, ClosePosition<'info>>,
    args: CloseArgs,
) -> Result<()> {
    let a = &mut *ctx.accounts;
    let clock = Clock::get()?;
    validate_position(&a.position, &clock)?;
    before_default(&a.position, &clock)?;
    validate_tokens(&a.position, &a.token)?;
    require_keys_eq!(
        a.actor.key(),
        a.position.borrower.key(),
        SimpleError::Unauthorized
    );
    let maximum = args.max_buyback_cost.min(a.position.loan.collateral);
    let result = buy(
        &a.position,
        &a.token,
        &a.execution,
        &a.actor,
        ctx.remaining_accounts,
        &args.route,
        maximum,
        args.temporary_funding,
    )?;
    let refund = a
        .position
        .loan
        .collateral
        .checked_sub(result.spent)
        .checked()?;
    let surplus = result
        .pool_credit
        .checked_sub(a.position.loan.q)
        .checked()?;
    retire(&mut a.position, surplus, None)?;
    let event = events::Closed {
        pool: a.position.pool.key(),
        loan: a.position.loan.key(),
        borrower: a.position.borrower.key(),
        quantity: a.position.loan.q,
        buyback_cost: result.spent,
        token_surplus: surplus,
        collateral_refund: refund,
        slot: clock.slot,
        ts: clock.unix_timestamp,
    };
    close_resolved(&mut a.position)?;
    emit_cpi!(event);
    Ok(())
}
pub fn add_margin(ctx: Context<AddMargin>, amount: u64) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    let a = &mut *ctx.accounts;
    let clock = Clock::get()?;
    validate_position(&a.position, &clock)?;
    before_default(&a.position, &clock)?;
    require_keys_eq!(
        a.actor.key(),
        a.position.borrower.key(),
        SimpleError::Unauthorized
    );
    require!(amount > 0, SimpleError::ZeroAmount);
    let before = a.position.loan.to_account_info().lamports();
    pay(
        &a.actor,
        a.position.loan.to_account_info(),
        &a.system_program,
        amount,
    )?;
    require_eq!(
        a.position
            .loan
            .to_account_info()
            .lamports()
            .checked_sub(before)
            .checked()?,
        amount,
        SimpleError::BalanceMismatch
    );
    a.position.loan.collateral = a.position.loan.collateral.checked_add(amount).checked()?;
    a.position.loan.margin = a.position.loan.margin.checked_add(amount).checked()?;
    clear_arm(&mut a.position.loan);
    let event = events::MarginAdded {
        loan: a.position.loan.key(),
        borrower: a.position.borrower.key(),
        amount,
        collateral: a.position.loan.collateral,
    };
    emit_cpi!(event);
    Ok(())
}
pub fn arm_liquidation(
    ctx: Context<ArmLiquidation>,
    minimum_reference_sequence: u64,
) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    let a = &mut *ctx.accounts;
    let clock = Clock::get()?;
    validate_position(&a.position, &clock)?;
    before_default(&a.position, &clock)?;
    require!(
        clock.unix_timestamp < a.position.loan.deadline,
        SimpleError::NotLiquidatable
    );
    reference::checked_reference(
        &a.reference,
        &a.position.market.key(),
        &a.position.policy,
        &a.position.config,
        a.position.loan.q,
        minimum_reference_sequence,
        &clock,
    )?;
    let bad = bad_reference(&a.position, &a.reference)?;
    let old = a.position.loan.cr_arm_sequence;
    let mut changed = false;
    if !bad {
        if old > 0 {
            clear_arm(&mut a.position.loan);
            changed = true;
        }
    } else if old == 0 || a.position.loan.cr_arm_reporter_epoch != a.position.config.reporter_epoch
    {
        // Two delayed, fresh bad observations are required. This does not claim
        // that every intermediate off-chain price was observed or unhealthy.
        a.position.loan.cr_arm_timestamp = clock.unix_timestamp;
        a.position.loan.cr_arm_slot = clock.slot;
        a.position.loan.cr_arm_sequence = a.reference.sequence;
        a.position.loan.cr_arm_reporter_epoch = a.position.config.reporter_epoch;
        changed = true;
    }
    // A repeated bad report does not let a third party reset the timer.
    if changed {
        let event = events::LiquidationArmed {
            loan: a.position.loan.key(),
            sequence: a.reference.sequence,
            reporter_epoch: a.position.config.reporter_epoch,
            timestamp: clock.unix_timestamp,
            slot: clock.slot,
            armed: bad,
        };
        emit_cpi!(event);
    }
    Ok(())
}
pub fn force_exit<'info>(
    ctx: Context<'info, ForceExit<'info>>,
    args: ForcedExitArgs,
) -> Result<()> {
    let a = &mut *ctx.accounts;
    let clock = Clock::get()?;
    validate_position(&a.position, &clock)?;
    before_default(&a.position, &clock)?;
    validate_tokens(&a.position, &a.token)?;
    keeper(&a.position, &a.actor.key())?;
    reference::checked_reference(
        &a.reference,
        &a.position.market.key(),
        &a.position.policy,
        &a.position.config,
        a.position.loan.q,
        args.minimum_reference_sequence,
        &clock,
    )?;
    let expired = clock.unix_timestamp >= a.position.loan.deadline;
    if !expired {
        require!(
            bad_reference(&a.position, &a.reference)?,
            SimpleError::NotLiquidatable
        );
        confirmed(&a.position, &a.reference, &clock)?;
    }
    let cap = finance::buyback_cap(
        a.reference.acquisition_reference,
        a.position.loan.collateral,
        &a.position.policy.terms,
    )?;
    let result = buy(
        &a.position,
        &a.token,
        &a.execution,
        &a.actor,
        ctx.remaining_accounts,
        &args.route,
        cap.min(args.max_buyback_cost),
        args.temporary_funding,
    )?;
    if !expired {
        require!(
            finance::unhealthy(
                a.position.loan.collateral,
                normalized_debt_cost(result.spent, a.position.loan.q, result.pool_credit)?,
                a.position.policy.terms.cr_liq_bps
            )?,
            SimpleError::HealthyFill
        );
    }
    let remaining = a
        .position
        .loan
        .collateral
        .checked_sub(result.spent)
        .checked()?;
    let reward = finance::keeper_reward(result.spent, remaining, &a.position.policy.terms)?;
    finance::move_sol(
        &a.position.loan.to_account_info(),
        &a.actor.to_account_info(),
        reward,
    )?;
    let refund = remaining.checked_sub(reward).checked()?;
    let surplus = result
        .pool_credit
        .checked_sub(a.position.loan.q)
        .checked()?;
    retire(&mut a.position, surplus, None)?;
    let event = events::ForcedExit {
        pool: a.position.pool.key(),
        loan: a.position.loan.key(),
        borrower: a.position.borrower.key(),
        keeper: a.actor.key(),
        quantity: a.position.loan.q,
        kind: if expired { 2 } else { 1 },
        buyback_cost: result.spent,
        token_surplus: surplus,
        keeper_reward: reward,
        collateral_refund: refund,
        slot: clock.slot,
        ts: clock.unix_timestamp,
    };
    close_resolved(&mut a.position)?;
    emit_cpi!(event);
    Ok(())
}
pub fn default_loan(ctx: Context<DefaultLoan>) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    let a = &mut *ctx.accounts;
    let clock = Clock::get()?;
    validate_position(&a.position, &clock)?;
    keeper(&a.position, &a.actor.key())?;
    require!(
        clock.unix_timestamp >= a.position.loan.default_at,
        SimpleError::NotOverdue
    );
    let recovery = a.position.loan.collateral;
    let reward = finance::keeper_reward(recovery, recovery, &a.position.policy.terms)?;
    let lender_sol = recovery.checked_sub(reward).checked()?;
    finance::move_sol(
        &a.position.loan.to_account_info(),
        &a.position.pool.to_account_info(),
        lender_sol,
    )?;
    finance::move_sol(
        &a.position.loan.to_account_info(),
        &a.actor.to_account_info(),
        reward,
    )?;
    retire(&mut a.position, 0, Some(lender_sol))?;
    crate::lender::require_sol_reserve(
        &a.position.pool.to_account_info(),
        a.position.pool.sol_reserve,
    )?;
    let event = events::Defaulted {
        pool: a.position.pool.key(),
        loan: a.position.loan.key(),
        borrower: a.position.borrower.key(),
        keeper: a.actor.key(),
        quantity_lost: a.position.loan.q,
        lender_sol,
        keeper_reward: reward,
        borrower_collateral_refund: 0,
        slot: clock.slot,
        ts: clock.unix_timestamp,
    };
    close_resolved(&mut a.position)?;
    emit_cpi!(event);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surplus_output_cannot_make_an_otherwise_healthy_loan_liquidatable() {
        let normalized = normalized_debt_cost(35, 10, 15).unwrap();
        assert_eq!(normalized, 23);
        assert!(finance::unhealthy(48, 35, 15_000).unwrap());
        assert!(!finance::unhealthy(48, normalized, 15_000).unwrap());
        assert_eq!(
            normalized_debt_cost(u64::MAX, u64::MAX, u64::MAX).unwrap(),
            u64::MAX
        );
        assert!(normalized_debt_cost(1, 2, 1).is_err());
    }
    #[test]
    fn second_quote_must_itself_postdate_both_confirmation_delays() {
        let arm = Observation {
            timestamp: 100,
            slot: 200,
            sequence: 5,
            reporter_epoch: 3,
        };
        let observed = Observation {
            timestamp: 105,
            slot: 207,
            sequence: 6,
            reporter_epoch: 3,
        };
        let clock = Clock {
            unix_timestamp: 110,
            slot: 220,
            ..Clock::default()
        };
        assert!(confirmation_ready(arm, observed, &clock, 5, 7).unwrap());
        assert!(!confirmation_ready(
            arm,
            Observation {
                timestamp: 104,
                ..observed
            },
            &clock,
            5,
            7
        )
        .unwrap());
        assert!(!confirmation_ready(
            arm,
            Observation {
                slot: 206,
                ..observed
            },
            &clock,
            5,
            7
        )
        .unwrap());
        assert!(!confirmation_ready(
            arm,
            Observation {
                sequence: 5,
                ..observed
            },
            &clock,
            5,
            7
        )
        .unwrap());
        assert!(!confirmation_ready(
            arm,
            Observation {
                reporter_epoch: 4,
                ..observed
            },
            &clock,
            5,
            7
        )
        .unwrap());
        assert!(!confirmation_ready(
            arm,
            observed,
            &Clock {
                unix_timestamp: 104,
                ..clock
            },
            5,
            7
        )
        .unwrap());
        assert!(!confirmation_ready(arm, observed, &Clock { slot: 206, ..clock }, 5, 7).unwrap());
        assert!(confirmation_ready(
            Observation {
                timestamp: i64::MAX,
                ..arm
            },
            observed,
            &clock,
            5,
            7
        )
        .is_err());
    }
}
