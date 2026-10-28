//! Generation-scoped lender accounting. Only measured new transfers mint principal claims.
//! Unsolicited token transfers are untracked: they mint no shares and remain in their
//! original generation vault. No rescue, adoption, or implicit allocation is promised.
use crate::errors::{CheckedExt, SimpleError};
use crate::events::{Deposit, SolClaimed, Withdraw};
use crate::route;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::AccountsClose;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};
use shortwin_pooled_core as pool_math;

pub fn accounting_error(error: pool_math::Error) -> anchor_lang::error::Error {
    match error {
        pool_math::Error::Overflow => error!(SimpleError::Arithmetic),
        pool_math::Error::InvalidState | pool_math::Error::WrongGeneration => {
            error!(SimpleError::AccountMismatch)
        }
        pool_math::Error::ZeroAmount => error!(SimpleError::ZeroAmount),
        pool_math::Error::NoShares | pool_math::Error::InsufficientShares => {
            error!(SimpleError::MinimumNotMet)
        }
        pool_math::Error::InsufficientLiquidity => error!(SimpleError::InsufficientLiquidity),
        pool_math::Error::LastSharesBorrowed => error!(SimpleError::LastSharesBorrowed),
        pool_math::Error::Retired => error!(SimpleError::RetiredPool),
        pool_math::Error::Wiped | pool_math::Error::NotWiped | pool_math::Error::NotRetired => {
            error!(SimpleError::GenerationNotEmpty)
        }
    }
}

/// Snapshot the stable token-account base. Transfer-fee extensions may update their
/// own accounting on an exit; only the base amount is allowed to change here.
fn token_snapshot(account: &AccountInfo<'_>) -> Result<([u8; 165], u64)> {
    let data = account.try_borrow_data()?;
    require!(data.len() >= 165, SimpleError::AccountMismatch);
    let mut base = [0u8; 165];
    base.copy_from_slice(&data[..165]);
    let amount = u64::from_le_bytes(
        base[64..72]
            .try_into()
            .map_err(|_| error!(SimpleError::AccountMismatch))?,
    );
    Ok((base, amount))
}
fn stable_token(before: &[u8; 165], after: &[u8; 165]) -> Result<()> {
    require!(
        before[..64] == after[..64] && before[72..] == after[72..],
        SimpleError::BalanceMismatch
    );
    Ok(())
}
fn validate_pool_account(pool: &PoolGeneration, market: &Market, market_key: Pubkey) -> Result<()> {
    require!(
        market.version == STATE_VERSION
            && pool.version == STATE_VERSION
            && pool.market == market_key
            && pool.mint == market.mint
            && pool.token_program == market.token_program,
        SimpleError::AccountMismatch
    );
    pool_math::validate_pool(&pool.core()).map_err(accounting_error)
}
fn validate_receipt(
    receipt: &LenderReceipt,
    pool: &PoolGeneration,
    pool_key: Pubkey,
    owner: Pubkey,
) -> Result<()> {
    require!(
        receipt.version == STATE_VERSION
            && receipt.pool == pool_key
            && receipt.owner == owner
            && receipt.generation == pool.generation,
        SimpleError::AccountMismatch
    );
    pool_math::validate_lender(&pool.core(), &receipt.core()).map_err(accounting_error)
}
/// Rent and tracked paid income cannot be spent as principal or confused with donations.
pub fn require_sol_reserve(pool: &AccountInfo<'_>, reserve: u64) -> Result<()> {
    let minimum = Rent::get()?
        .minimum_balance(pool.data_len())
        .checked_add(reserve)
        .checked()?;
    require_gte!(pool.lamports(), minimum, SimpleError::BalanceMismatch);
    Ok(())
}
pub fn pay_pool_sol(
    pool: &AccountInfo<'_>,
    recipient: &AccountInfo<'_>,
    amount: u64,
    remaining_reserve: u64,
) -> Result<()> {
    require_keys_eq!(*pool.owner, crate::ID, SimpleError::AccountMismatch);
    require_keys_neq!(*pool.key, *recipient.key, SimpleError::AccountMismatch);
    let source_after = pool.lamports().checked_sub(amount).checked()?;
    let minimum = Rent::get()?
        .minimum_balance(pool.data_len())
        .checked_add(remaining_reserve)
        .checked()?;
    require_gte!(source_after, minimum, SimpleError::BalanceMismatch);
    let recipient_after = recipient.lamports().checked_add(amount).checked()?;
    **pool.try_borrow_mut_lamports()? = source_after;
    **recipient.try_borrow_mut_lamports()? = recipient_after;
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct DepositTokens<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(seeds=[MARKET_SEED, mint.key().as_ref()], bump=market.bump,
        constraint=market.mint==mint.key() @ SimpleError::AccountMismatch,
        constraint=market.token_program==token_program.key() @ SimpleError::AccountMismatch)]
    pub market: Box<Account<'info, Market>>,
    #[account(mut,seeds=[POOL_SEED, market.key().as_ref(), &pool.generation.to_le_bytes()], bump=pool.bump,
        constraint=pool.generation==market.active_generation @ SimpleError::AccountMismatch)]
    pub pool: Box<Account<'info, PoolGeneration>>,
    #[account(init_if_needed,payer=owner,space=LenderReceipt::SPACE,
        seeds=[LENDER_SEED,pool.key().as_ref(),owner.key().as_ref()],bump)]
    pub receipt: Box<Account<'info, LenderReceipt>>,
    #[account(mint::token_program=token_program)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut,token::mint=mint,token::authority=owner,token::token_program=token_program,
        constraint=owner_tokens.key()!=token_vault.key() @ SimpleError::AccountMismatch)]
    pub owner_tokens: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut,seeds=[TOKEN_VAULT_SEED,pool.key().as_ref()],bump=pool.token_vault_bump,
        token::mint=mint,token::authority=pool,token::token_program=token_program)]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
pub fn deposit(
    ctx: Context<DepositTokens>,
    amount: u64,
    min_shares: u128,
    max_receipt_rent: u64,
) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    validate_pool_account(
        &ctx.accounts.pool,
        &ctx.accounts.market,
        ctx.accounts.market.key(),
    )?;
    route::supported_mint(
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.token_program.key(),
    )?;
    require_sol_reserve(
        &ctx.accounts.pool.to_account_info(),
        ctx.accounts.pool.sol_reserve,
    )?;
    let mut pool = ctx.accounts.pool.core();
    let mut lender = if ctx.accounts.receipt.version == 0 {
        require_gte!(
            max_receipt_rent,
            Rent::get()?.minimum_balance(LenderReceipt::SPACE),
            SimpleError::SetupRentAboveMax
        );
        pool_math::Lender::new(pool.generation)
    } else {
        validate_receipt(
            &ctx.accounts.receipt,
            &ctx.accounts.pool,
            ctx.accounts.pool.key(),
            ctx.accounts.owner.key(),
        )?;
        require_eq!(
            ctx.accounts.receipt.bump,
            ctx.bumps.receipt,
            SimpleError::AccountMismatch
        );
        ctx.accounts.receipt.core()
    };
    let shares = pool_math::deposit(&mut pool, &mut lender, amount).map_err(accounting_error)?;
    require_gte!(shares, min_shares, SimpleError::MinimumNotMet);
    let (vault_before, vault_amount) = token_snapshot(&ctx.accounts.token_vault.to_account_info())?;
    let (source_before, source_amount) =
        token_snapshot(&ctx.accounts.owner_tokens.to_account_info())?;
    require_gte!(
        vault_amount,
        ctx.accounts.pool.cash,
        SimpleError::BalanceMismatch
    );
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.owner_tokens.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.token_vault.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.market.decimals,
    )?;
    let (vault_after, vault_new) = token_snapshot(&ctx.accounts.token_vault.to_account_info())?;
    let (source_after, source_new) = token_snapshot(&ctx.accounts.owner_tokens.to_account_info())?;
    stable_token(&vault_before, &vault_after)?;
    stable_token(&source_before, &source_after)?;
    require!(
        vault_new.checked_sub(vault_amount) == Some(amount)
            && source_amount.checked_sub(source_new) == Some(amount),
        SimpleError::BalanceMismatch
    );
    ctx.accounts.pool.store(&pool)?;
    ctx.accounts.receipt.version = STATE_VERSION;
    ctx.accounts.receipt.bump = ctx.bumps.receipt;
    ctx.accounts.receipt.pool = ctx.accounts.pool.key();
    ctx.accounts.receipt.owner = ctx.accounts.owner.key();
    ctx.accounts.receipt.generation = pool.generation;
    ctx.accounts.receipt.store(&lender)?;
    emit_cpi!(Deposit {
        pool: ctx.accounts.pool.key(),
        owner: ctx.accounts.owner.key(),
        tokens: amount,
        shares,
        total_shares: pool.shares,
        cash: pool.cash,
        principal_out: pool.principal_out
    });
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct WithdrawTokens<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(seeds=[MARKET_SEED,mint.key().as_ref()],bump=market.bump,
        constraint=market.mint==mint.key() @ SimpleError::AccountMismatch,
        constraint=market.token_program==token_program.key() @ SimpleError::AccountMismatch)]
    pub market: Box<Account<'info, Market>>,
    #[account(mut,seeds=[POOL_SEED,market.key().as_ref(),&pool.generation.to_le_bytes()],bump=pool.bump)]
    pub pool: Box<Account<'info, PoolGeneration>>,
    #[account(mut,seeds=[LENDER_SEED,pool.key().as_ref(),owner.key().as_ref()],bump=receipt.bump)]
    pub receipt: Box<Account<'info, LenderReceipt>>,
    #[account(mint::token_program=token_program)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut,token::mint=mint,token::authority=owner,token::token_program=token_program,
        constraint=owner_tokens.key()!=token_vault.key() @ SimpleError::AccountMismatch)]
    pub owner_tokens: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut,seeds=[TOKEN_VAULT_SEED,pool.key().as_ref()],bump=pool.token_vault_bump,
        token::mint=mint,token::authority=pool,token::token_program=token_program)]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Interface<'info, TokenInterface>,
}
pub fn withdraw(
    ctx: Context<WithdrawTokens>,
    shares: u128,
    min_tokens: u64,
    min_sol: u64,
) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    validate_pool_account(
        &ctx.accounts.pool,
        &ctx.accounts.market,
        ctx.accounts.market.key(),
    )?;
    validate_receipt(
        &ctx.accounts.receipt,
        &ctx.accounts.pool,
        ctx.accounts.pool.key(),
        ctx.accounts.owner.key(),
    )?;
    require_sol_reserve(
        &ctx.accounts.pool.to_account_info(),
        ctx.accounts.pool.sol_reserve,
    )?;
    let (mut pool, mut lender) = (ctx.accounts.pool.core(), ctx.accounts.receipt.core());
    let paid = pool_math::withdraw(&mut pool, &mut lender, shares).map_err(accounting_error)?;
    require_gte!(paid.sol, min_sol, SimpleError::MinimumNotMet);
    let (vault_before, vault_amount) = token_snapshot(&ctx.accounts.token_vault.to_account_info())?;
    let (destination_before, destination_amount) =
        token_snapshot(&ctx.accounts.owner_tokens.to_account_info())?;
    require_gte!(
        vault_amount,
        ctx.accounts.pool.cash,
        SimpleError::BalanceMismatch
    );
    if paid.tokens > 0 {
        let market_key = ctx.accounts.market.key();
        let generation = ctx.accounts.pool.generation.to_le_bytes();
        let bump = [ctx.accounts.pool.bump];
        let seeds: &[&[u8]] = &[POOL_SEED, market_key.as_ref(), &generation, &bump];
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.token_vault.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.owner_tokens.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                &[seeds],
            ),
            paid.tokens,
            ctx.accounts.market.decimals,
        )?;
    }
    let (vault_after, vault_new) = token_snapshot(&ctx.accounts.token_vault.to_account_info())?;
    let (destination_after, destination_new) =
        token_snapshot(&ctx.accounts.owner_tokens.to_account_info())?;
    stable_token(&vault_before, &vault_after)?;
    stable_token(&destination_before, &destination_after)?;
    require!(
        vault_amount.checked_sub(vault_new) == Some(paid.tokens),
        SimpleError::BalanceMismatch
    );
    let net_received = destination_new.checked_sub(destination_amount).checked()?;
    require_gte!(net_received, min_tokens, SimpleError::MinimumNotMet);
    pay_pool_sol(
        &ctx.accounts.pool.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        paid.sol,
        pool.sol_reserve,
    )?;
    ctx.accounts.pool.store(&pool)?;
    ctx.accounts.receipt.store(&lender)?;
    emit_cpi!(Withdraw {
        pool: ctx.accounts.pool.key(),
        owner: ctx.accounts.owner.key(),
        tokens: paid.tokens,
        sol: paid.sol,
        shares,
        total_shares: pool.shares,
        cash: pool.cash,
        principal_out: pool.principal_out
    });
    if lender.shares == 0 && lender.pending_sol == 0 && lender.fraction == 0 {
        ctx.accounts
            .receipt
            .close(ctx.accounts.owner.to_account_info())?;
    }
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct ClaimSol<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut,seeds=[POOL_SEED,pool.market.as_ref(),&pool.generation.to_le_bytes()],bump=pool.bump)]
    pub pool: Box<Account<'info, PoolGeneration>>,
    #[account(mut,seeds=[LENDER_SEED,pool.key().as_ref(),owner.key().as_ref()],bump=receipt.bump)]
    pub receipt: Box<Account<'info, LenderReceipt>>,
}
pub fn claim_sol(ctx: Context<ClaimSol>, min_sol: u64) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    require_eq!(
        ctx.accounts.pool.version,
        STATE_VERSION,
        SimpleError::AccountMismatch
    );
    validate_receipt(
        &ctx.accounts.receipt,
        &ctx.accounts.pool,
        ctx.accounts.pool.key(),
        ctx.accounts.owner.key(),
    )?;
    require_sol_reserve(
        &ctx.accounts.pool.to_account_info(),
        ctx.accounts.pool.sol_reserve,
    )?;
    let (mut pool, mut lender) = (ctx.accounts.pool.core(), ctx.accounts.receipt.core());
    let paid = pool_math::claim(&mut pool, &mut lender).map_err(accounting_error)?;
    require_gte!(paid, min_sol, SimpleError::MinimumNotMet);
    pay_pool_sol(
        &ctx.accounts.pool.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        paid,
        pool.sol_reserve,
    )?;
    ctx.accounts.pool.store(&pool)?;
    ctx.accounts.receipt.store(&lender)?;
    emit_cpi!(SolClaimed {
        pool: ctx.accounts.pool.key(),
        owner: ctx.accounts.owner.key(),
        sol: paid
    });
    if lender.shares == 0 && lender.pending_sol == 0 && lender.fraction == 0 {
        ctx.accounts
            .receipt
            .close(ctx.accounts.owner.to_account_info())?;
    }
    Ok(())
}
