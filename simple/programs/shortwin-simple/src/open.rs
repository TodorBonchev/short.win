//! Fixed-term, fully collateralized sale from the shared token pool.
use crate::{
    errors::{CheckedExt, SimpleError},
    events::Opened,
    finance, reference,
    route::{self, ReviewedDeployment, RouteAccounts, RouteArgs},
    state::*,
};
use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct OpenArgs {
    pub nonce: u64,
    pub q: u64,
    pub min_proceeds: u64,
    pub max_margin: u64,
    pub max_term_fee: u64,
    /// Signed maximum for the separately refunded scratch-account rent advance.
    pub temporary_funding: u64,
    pub max_account_rent: u64,
    pub minimum_reference_sequence: u64,
    pub route: RouteArgs,
}
#[event_cpi]
#[derive(Accounts)]
#[instruction(args:OpenArgs)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub borrower: Signer<'info>,
    #[account(seeds=[CONFIG_SEED],bump=config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds=[POLICY_SEED,config.key().as_ref(),&policy.policy_version.to_le_bytes()],bump=policy.bump,
  constraint=policy.key()==config.current_policy @ SimpleError::AccountMismatch,
  constraint=policy.config==config.key() @ SimpleError::AccountMismatch)]
    pub policy: Box<Account<'info, Policy>>,
    #[account(mut,seeds=[MARKET_SEED,mint.key().as_ref()],bump=market.bump,
  constraint=market.config==config.key() @ SimpleError::AccountMismatch,
  constraint=market.mint==mint.key() @ SimpleError::AccountMismatch,
  constraint=market.token_program==token_program.key() @ SimpleError::AccountMismatch)]
    pub market: Box<Account<'info, Market>>,
    #[account(mut,seeds=[POOL_SEED,market.key().as_ref(),&pool.generation.to_le_bytes()],bump=pool.bump,
  constraint=pool.market==market.key() @ SimpleError::AccountMismatch,
  constraint=pool.generation==market.active_generation @ SimpleError::AccountMismatch,
  constraint=pool.mint==mint.key() @ SimpleError::AccountMismatch,
  constraint=pool.token_program==token_program.key() @ SimpleError::AccountMismatch)]
    pub pool: Box<Account<'info, PoolGeneration>>,
    #[account(init,payer=borrower,space=Loan::SPACE,
  seeds=[LOAN_SEED,pool.key().as_ref(),borrower.key().as_ref(),&args.nonce.to_le_bytes()],bump)]
    pub loan: Box<Account<'info, Loan>>,
    #[account(init_if_needed,payer=borrower,space=Exposure::SPACE,
  seeds=[EXPOSURE_SEED,pool.key().as_ref(),borrower.key().as_ref()],bump)]
    pub exposure: Box<Account<'info, Exposure>>,
    pub reference: Box<Account<'info, PriceReference>>,
    #[account(mint::token_program=token_program)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut,seeds=[TOKEN_VAULT_SEED,pool.key().as_ref()],bump=pool.token_vault_bump,
  token::mint=mint,token::authority=pool,token::token_program=token_program)]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Interface<'info, TokenInterface>,
    /// CHECK: governed destination, excluded from Jupiter's account surface.
    #[account(mut,address=config.fee_recipient)]
    pub fee_recipient: UncheckedAccount<'info>,
    /// CHECK: route validates exact readonly loan-local authority PDA.
    pub execution_authority: UncheckedAccount<'info>,
    /// CHECK: route validates, initializes and closes exact loan-local scratch PDA.
    #[account(mut)]
    pub execution_base: UncheckedAccount<'info>,
    /// CHECK: route validates, initializes and closes exact loan-local scratch PDA.
    #[account(mut)]
    pub execution_wsol: UncheckedAccount<'info>,
    /// CHECK: route validates canonical readonly WSOL mint.
    pub wsol_mint: UncheckedAccount<'info>,
    pub wsol_token_program: Program<'info, Token>,
    /// CHECK: canonical executable Jupiter loader instance checked every trade.
    pub jupiter: UncheckedAccount<'info>,
    /// CHECK: canonical reviewed Jupiter ProgramData checked every trade.
    pub jupiter_program_data: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

fn pay<'info>(
    from: &Signer<'info>,
    to: AccountInfo<'info>,
    system: &Program<'info, System>,
    amount: u64,
) -> Result<()> {
    if amount > 0 {
        system_program::transfer(
            CpiContext::new(
                system.key(),
                Transfer {
                    from: from.to_account_info(),
                    to,
                },
            ),
            amount,
        )?;
    }
    Ok(())
}
pub fn open_position<'info>(
    ctx: Context<'info, OpenPosition<'info>>,
    args: OpenArgs,
) -> Result<()> {
    let a = &mut *ctx.accounts;
    a.config.validate()?;
    a.policy.validate(&a.config.key())?;
    require!(!a.config.opens_paused, SimpleError::Paused);
    require!(
        a.market.version == STATE_VERSION && a.pool.version == STATE_VERSION && !a.pool.retired,
        SimpleError::AccountMismatch
    );
    require_eq!(
        args.nonce,
        a.market.next_loan_nonce,
        SimpleError::InvalidNonce
    );
    require!(args.q > 0 && args.min_proceeds > 0, SimpleError::ZeroAmount);
    require_gte!(a.pool.cash, args.q, SimpleError::InsufficientLiquidity);
    require_gte!(
        a.token_vault.amount,
        a.pool.cash,
        SimpleError::BalanceMismatch
    );
    let clock = Clock::get()?;
    reference::checked_reference(
        &a.reference,
        &a.market.key(),
        &a.policy,
        &a.config,
        args.q,
        args.minimum_reference_sequence,
        &clock,
    )?;
    let mut core = a.pool.core();
    shortwin_pooled_core::validate_pool(&core).map_err(crate::lender::accounting_error)?;
    let rent = Rent::get()?;
    let mut setup = rent.minimum_balance(Loan::SPACE);
    if a.exposure.version == 0 {
        setup = setup
            .checked_add(rent.minimum_balance(Exposure::SPACE))
            .checked()?;
        a.exposure.set_inner(Exposure {
            version: STATE_VERSION,
            bump: ctx.bumps.exposure,
            pool: a.pool.key(),
            borrower: a.borrower.key(),
            q: 0,
            notional: 0,
        });
    }
    require_gte!(args.max_account_rent, setup, SimpleError::SetupRentAboveMax);
    require!(
        a.exposure.version == STATE_VERSION
            && a.exposure.pool == a.pool.key()
            && a.exposure.borrower == a.borrower.key(),
        SimpleError::AccountMismatch
    );
    require_keys_neq!(
        a.fee_recipient.key(),
        a.loan.key(),
        SimpleError::AccountMismatch
    );
    require_keys_neq!(
        a.fee_recipient.key(),
        a.pool.key(),
        SimpleError::AccountMismatch
    );
    let temporary = route::temporary_rent()?;
    require_gte!(
        args.temporary_funding,
        temporary,
        SimpleError::FundingAboveMax
    );
    pay(
        &a.borrower,
        a.loan.to_account_info(),
        &a.system_program,
        temporary,
    )?;
    // Own AccountInfo clones keep route borrows separate from subsequent ledger writes.
    let loan_info = a.loan.to_account_info();
    let pool_info = a.pool.to_account_info();
    let vault_info = a.token_vault.to_account_info();
    let mint_info = a.mint.to_account_info();
    let token_info = a.token_program.to_account_info();
    let auth_info = a.execution_authority.to_account_info();
    let base_info = a.execution_base.to_account_info();
    let wsol_info = a.execution_wsol.to_account_info();
    let wm_info = a.wsol_mint.to_account_info();
    let wt_info = a.wsol_token_program.to_account_info();
    let sys_info = a.system_program.to_account_info();
    let jup_info = a.jupiter.to_account_info();
    let pd_info = a.jupiter_program_data.to_account_info();
    let forbidden = [
        a.borrower.key(),
        a.config.key(),
        a.policy.key(),
        a.market.key(),
        a.exposure.key(),
        a.reference.key(),
        a.fee_recipient.key(),
        a.event_authority.key(),
        a.program.key(),
    ];
    let routes = RouteAccounts {
        loan: &loan_info,
        pool: &pool_info,
        token_vault: &vault_info,
        mint: &mint_info,
        base_program: &token_info,
        execution_authority: &auth_info,
        execution_base: &base_info,
        execution_wsol: &wsol_info,
        wsol_mint: &wm_info,
        wsol_token_program: &wt_info,
        system_program: &sys_info,
        jupiter: &jup_info,
        jupiter_program_data: &pd_info,
        forbidden: &forbidden,
    };
    let market_key = a.market.key();
    let generation = a.pool.generation.to_le_bytes();
    let bump = [a.pool.bump];
    let pool_signer: &[&[u8]] = &[POOL_SEED, market_key.as_ref(), &generation, &bump];
    let sale = route::sell(
        &routes,
        ctx.remaining_accounts,
        &args.route,
        pool_signer,
        args.q,
        args.min_proceeds,
        rent.minimum_balance(Loan::SPACE),
        ReviewedDeployment {
            deploy_slot: a.config.jupiter_deploy_slot,
            programdata_len: a.config.jupiter_program_data_len,
            elf_sha256: a.config.jupiter_elf_hash,
        },
    )?;
    let economics = finance::opening(
        sale.proceeds,
        a.reference.live_ask_upper,
        a.reference.history_ask_upper_max,
        &a.policy.terms,
    )?;
    finance::check_caps(
        args.q,
        core.cash,
        core.principal_out,
        a.pool.booked_notional_out,
        a.exposure.notional,
        economics.notional,
        &a.policy.terms,
    )?;
    require_gte!(
        args.max_margin,
        economics.margin,
        SimpleError::MarginAboveMax
    );
    require_gte!(args.max_term_fee, economics.fee, SimpleError::FeeAboveMax);
    pay(
        &a.borrower,
        loan_info.clone(),
        &a.system_program,
        economics.margin,
    )?;
    pay(
        &a.borrower,
        pool_info.clone(),
        &a.system_program,
        economics.lender_fee,
    )?;
    pay(
        &a.borrower,
        a.fee_recipient.to_account_info(),
        &a.system_program,
        economics.protocol_fee,
    )?;
    shortwin_pooled_core::borrow(&mut core, args.q).map_err(crate::lender::accounting_error)?;
    if economics.lender_fee > 0 {
        shortwin_pooled_core::credit_income(&mut core, economics.lender_fee)
            .map_err(crate::lender::accounting_error)?;
    }
    a.pool.store(&core)?;
    a.pool.booked_notional_out = a
        .pool
        .booked_notional_out
        .checked_add(economics.notional)
        .checked()?;
    a.exposure.q = a.exposure.q.checked_add(args.q).checked()?;
    a.exposure.notional = a
        .exposure
        .notional
        .checked_add(economics.notional)
        .checked()?;
    a.market.next_loan_nonce = a.market.next_loan_nonce.checked_add(1).checked()?;
    let deadline = clock
        .unix_timestamp
        .checked_add(i64::from(a.policy.terms.term_seconds))
        .checked()?;
    let default_at = deadline
        .checked_add(i64::from(a.policy.terms.default_grace_seconds))
        .checked()?;
    a.loan.set_inner(Loan {
        version: STATE_VERSION,
        bump: ctx.bumps.loan,
        market: a.market.key(),
        pool: a.pool.key(),
        borrower: a.borrower.key(),
        policy: a.policy.key(),
        nonce: args.nonce,
        q: args.q,
        sale_proceeds: sale.proceeds,
        margin: economics.margin,
        collateral: economics.collateral,
        opening_notional: economics.notional,
        term_fee: economics.fee,
        lender_fee: economics.lender_fee,
        protocol_fee: economics.protocol_fee,
        opened_at: clock.unix_timestamp,
        deadline,
        default_at,
        cr_arm_timestamp: 0,
        cr_arm_slot: 0,
        cr_arm_sequence: 0,
        cr_arm_reporter_epoch: 0,
    });
    finance::move_sol(&loan_info, &a.borrower.to_account_info(), temporary)?;
    require_gte!(
        loan_info.lamports(),
        rent.minimum_balance(Loan::SPACE)
            .checked_add(economics.collateral)
            .checked()?,
        SimpleError::BalanceMismatch
    );
    crate::lender::require_sol_reserve(&pool_info, core.sol_reserve)?;
    let event = Opened {
        market: a.market.key(),
        pool: a.pool.key(),
        loan: a.loan.key(),
        borrower: a.borrower.key(),
        policy: a.policy.key(),
        nonce: args.nonce,
        quantity: args.q,
        sale_proceeds: sale.proceeds,
        margin: economics.margin,
        collateral: economics.collateral,
        opening_notional: economics.notional,
        term_fee: economics.fee,
        lender_fee: economics.lender_fee,
        protocol_fee: economics.protocol_fee,
        deadline,
        default_at,
        slot: clock.slot,
        ts: clock.unix_timestamp,
    };
    emit_cpi!(event);
    Ok(())
}
