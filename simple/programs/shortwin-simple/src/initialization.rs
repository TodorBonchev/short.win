use crate::errors::{CheckedExt, SimpleError};
use crate::events::{ConfigChanged, MarketRegistered, PolicyCreated, PoolRolled};
use crate::route::{self, ReviewedDeployment};
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;
use anchor_lang::system_program::{self, Transfer};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitializeArgs {
    pub governance: Pubkey,
    pub emergency: Pubkey,
    pub reporter: Pubkey,
    pub fee_recipient: Pubkey,
    pub genesis_hash: [u8; 32],
    pub policy_version: u64,
    pub terms: FinancialPolicy,
    pub opens_paused: bool,
    pub market_creation_fee: u64,
    pub jupiter_deploy_slot: u64,
    pub jupiter_program_data_len: u32,
    pub jupiter_elf_hash: [u8; 32],
    pub max_setup_rent: u64,
}
#[event_cpi]
#[derive(Accounts)]
#[instruction(args: InitializeArgs)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init,payer=authority,space=Config::SPACE,seeds=[CONFIG_SEED],bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(init,payer=authority,space=Policy::SPACE,
        seeds=[POLICY_SEED,config.key().as_ref(),&args.policy_version.to_le_bytes()],bump)]
    pub policy: Box<Account<'info, Policy>>,
    /// CHECK: executable self program and its canonical loader data are checked before init commits.
    #[account(address=crate::ID,executable)]
    pub program_account: UncheckedAccount<'info>,
    /// CHECK: canonical loader identity and authority checked in validate_initializer.
    pub program_data: UncheckedAccount<'info>,
    /// CHECK: Jupiter's canonical loader program instance, checked by route::validate_deployment.
    pub jupiter: UncheckedAccount<'info>,
    /// CHECK: Jupiter's canonical loader ProgramData, checked by route::validate_deployment.
    pub jupiter_program_data: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

fn validate_initializer(
    program: &AccountInfo<'_>,
    data: &AccountInfo<'_>,
    authority: &Pubkey,
) -> Result<()> {
    let expected =
        Pubkey::find_program_address(&[crate::ID.as_ref()], &bpf_loader_upgradeable::ID).0;
    require!(
        program.key == &crate::ID
            && program.executable
            && program.owner == &bpf_loader_upgradeable::ID
            && data.key == &expected
            && data.owner == &bpf_loader_upgradeable::ID
            && !data.executable,
        SimpleError::AccountMismatch
    );
    let p = program.try_borrow_data()?;
    require!(
        p.len() == 36 && p[..4] == [2, 0, 0, 0] && p[4..36] == expected.to_bytes(),
        SimpleError::AccountMismatch
    );
    let d = data.try_borrow_data()?;
    require!(
        d.len() >= 45 && d[..4] == [3, 0, 0, 0] && d[12] == 1 && d[13..45] == authority.to_bytes(),
        SimpleError::Unauthorized
    );
    Ok(())
}
pub fn initialize(ctx: Context<Initialize>, args: InitializeArgs) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    validate_initializer(
        &ctx.accounts.program_account,
        &ctx.accounts.program_data,
        &ctx.accounts.authority.key(),
    )?;
    args.terms.validate()?;
    require_eq!(args.policy_version, 1, SimpleError::InvalidPolicy);
    let rent = Rent::get()?;
    let required = rent
        .minimum_balance(Config::SPACE)
        .checked_add(rent.minimum_balance(Policy::SPACE))
        .checked()?;
    require_gte!(
        args.max_setup_rent,
        required,
        SimpleError::SetupRentAboveMax
    );
    route::validate_deployment(
        &ctx.accounts.jupiter,
        &ctx.accounts.jupiter_program_data,
        ReviewedDeployment {
            deploy_slot: args.jupiter_deploy_slot,
            programdata_len: args.jupiter_program_data_len,
            elf_sha256: args.jupiter_elf_hash,
        },
        Clock::get()?.slot,
    )?;
    let config = Config {
        version: STATE_VERSION,
        bump: ctx.bumps.config,
        governance: args.governance,
        emergency: args.emergency,
        reporter: args.reporter,
        fee_recipient: args.fee_recipient,
        genesis_hash: args.genesis_hash,
        current_policy: ctx.accounts.policy.key(),
        current_policy_version: args.policy_version,
        reporter_epoch: 1,
        opens_paused: args.opens_paused,
        prices_paused: false,
        market_creation_fee: args.market_creation_fee,
        jupiter_deploy_slot: args.jupiter_deploy_slot,
        jupiter_program_data_len: args.jupiter_program_data_len,
        jupiter_elf_hash: args.jupiter_elf_hash,
    };
    config.validate()?;
    ctx.accounts.config.set_inner(config);
    ctx.accounts.policy.set_inner(Policy {
        version: STATE_VERSION,
        bump: ctx.bumps.policy,
        config: ctx.accounts.config.key(),
        policy_version: args.policy_version,
        terms: args.terms,
    });
    emit_cpi!(PolicyCreated {
        policy: ctx.accounts.policy.key(),
        version: args.policy_version
    });
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct RegisterMarket<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(seeds=[CONFIG_SEED],bump=config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds=[POLICY_SEED,config.key().as_ref(),&policy.policy_version.to_le_bytes()],bump=policy.bump,
        constraint=policy.key()==config.current_policy @ SimpleError::AccountMismatch,
        constraint=policy.config==config.key() @ SimpleError::AccountMismatch)]
    pub policy: Box<Account<'info, Policy>>,
    #[account(init,payer=creator,space=Market::SPACE,seeds=[MARKET_SEED,mint.key().as_ref()],bump)]
    pub market: Box<Account<'info, Market>>,
    #[account(init,payer=creator,space=PoolGeneration::SPACE,
        seeds=[POOL_SEED,market.key().as_ref(),&0u64.to_le_bytes()],bump)]
    pub pool: Box<Account<'info, PoolGeneration>>,
    #[account(mint::token_program=token_program)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(init,payer=creator,seeds=[TOKEN_VAULT_SEED,pool.key().as_ref()],bump,
        token::mint=mint,token::authority=pool,token::token_program=token_program)]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    /// CHECK: exact configured fee recipient; it need not be a System-owned treasury.
    #[account(mut,address=config.fee_recipient)]
    pub fee_recipient: UncheckedAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
fn setup_pool(
    pool: &mut PoolGeneration,
    bump: u8,
    vault_bump: u8,
    market: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
    generation: u64,
) {
    pool.version = STATE_VERSION;
    pool.bump = bump;
    pool.token_vault_bump = vault_bump;
    pool.market = market;
    pool.mint = mint;
    pool.token_program = token_program;
    pool.generation = generation;
    pool.cash = 0;
    pool.principal_out = 0;
    pool.shares = 0;
    pool.loan_count = 0;
    pool.sol_reserve = 0;
    pool.sol_index = [0; 4];
    pool.retired = false;
    pool.booked_notional_out = 0;
}
pub fn register_market(
    ctx: Context<RegisterMarket>,
    max_creation_fee: u64,
    max_setup_rent: u64,
) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    ctx.accounts.config.validate()?;
    ctx.accounts.policy.validate(&ctx.accounts.config.key())?;
    require_keys_neq!(
        ctx.accounts.mint.key(),
        WSOL_MINT,
        SimpleError::MintUnsupported
    );
    let decimals = route::supported_mint(
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.token_program.key(),
    )?;
    let rent = Rent::get()?;
    let required = rent
        .minimum_balance(Market::SPACE)
        .checked_add(rent.minimum_balance(PoolGeneration::SPACE))
        .checked()?
        .checked_add(rent.minimum_balance(ctx.accounts.token_vault.to_account_info().data_len()))
        .checked()?;
    require_gte!(max_setup_rent, required, SimpleError::SetupRentAboveMax);
    let fee = ctx.accounts.config.market_creation_fee;
    require_gte!(max_creation_fee, fee, SimpleError::SetupFeeAboveMax);
    if fee > 0 {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.key(),
                Transfer {
                    from: ctx.accounts.creator.to_account_info(),
                    to: ctx.accounts.fee_recipient.to_account_info(),
                },
            ),
            fee,
        )?;
    }
    ctx.accounts.market.set_inner(Market {
        version: STATE_VERSION,
        bump: ctx.bumps.market,
        config: ctx.accounts.config.key(),
        mint: ctx.accounts.mint.key(),
        token_program: ctx.accounts.token_program.key(),
        decimals,
        active_generation: 0,
        next_loan_nonce: 0,
    });
    setup_pool(
        &mut ctx.accounts.pool,
        ctx.bumps.pool,
        ctx.bumps.token_vault,
        ctx.accounts.market.key(),
        ctx.accounts.mint.key(),
        ctx.accounts.token_program.key(),
        0,
    );
    let clock = Clock::get()?;
    emit_cpi!(MarketRegistered {
        market: ctx.accounts.market.key(),
        pool: ctx.accounts.pool.key(),
        mint: ctx.accounts.mint.key(),
        creator: ctx.accounts.creator.key(),
        token_program: ctx.accounts.token_program.key(),
        generation: 0,
        slot: clock.slot,
        ts: clock.unix_timestamp
    });
    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct NewPolicyArgs {
    pub version: u64,
    pub terms: FinancialPolicy,
    pub max_setup_rent: u64,
}
#[event_cpi]
#[derive(Accounts)]
#[instruction(args:NewPolicyArgs)]
pub struct NewPolicy<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut,seeds=[CONFIG_SEED],bump=config.bump,
        constraint=config.governance==authority.key() @ SimpleError::Unauthorized)]
    pub config: Box<Account<'info, Config>>,
    #[account(init,payer=authority,space=Policy::SPACE,
        seeds=[POLICY_SEED,config.key().as_ref(),&args.version.to_le_bytes()],bump)]
    pub policy: Box<Account<'info, Policy>>,
    pub system_program: Program<'info, System>,
}
pub fn new_policy(ctx: Context<NewPolicy>, args: NewPolicyArgs) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    ctx.accounts.config.validate()?;
    args.terms.validate()?;
    require_eq!(
        args.version,
        ctx.accounts
            .config
            .current_policy_version
            .checked_add(1)
            .checked()?,
        SimpleError::InvalidPolicy
    );
    require_gte!(
        args.max_setup_rent,
        Rent::get()?.minimum_balance(Policy::SPACE),
        SimpleError::SetupRentAboveMax
    );
    ctx.accounts.policy.set_inner(Policy {
        version: STATE_VERSION,
        bump: ctx.bumps.policy,
        config: ctx.accounts.config.key(),
        policy_version: args.version,
        terms: args.terms,
    });
    ctx.accounts.config.current_policy = ctx.accounts.policy.key();
    ctx.accounts.config.current_policy_version = args.version;
    emit_cpi!(PolicyCreated {
        policy: ctx.accounts.policy.key(),
        version: args.version
    });
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct RoleAuthority<'info> {
    pub authority: Signer<'info>,
    #[account(mut,seeds=[CONFIG_SEED],bump=config.bump)]
    pub config: Box<Account<'info, Config>>,
}
pub fn set_pause(ctx: Context<RoleAuthority>, prices: bool, paused: bool) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    ctx.accounts.config.validate()?;
    require!(
        ctx.accounts.authority.key() == ctx.accounts.config.governance
            || (paused && ctx.accounts.authority.key() == ctx.accounts.config.emergency),
        SimpleError::Unauthorized
    );
    if prices {
        ctx.accounts.config.prices_paused = paused;
    } else {
        ctx.accounts.config.opens_paused = paused;
    }
    emit_cpi!(ConfigChanged {
        kind: if prices { 2 } else { 1 }
    });
    Ok(())
}
pub fn set_reporter(ctx: Context<RoleAuthority>, reporter: Pubkey) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    ctx.accounts.config.validate()?;
    require_keys_eq!(
        ctx.accounts.authority.key(),
        ctx.accounts.config.governance,
        SimpleError::Unauthorized
    );
    require!(
        reporter != Pubkey::default()
            && reporter != ctx.accounts.config.governance
            && reporter != ctx.accounts.config.emergency,
        SimpleError::InvalidPolicy
    );
    ctx.accounts.config.reporter = reporter;
    ctx.accounts.config.reporter_epoch = ctx
        .accounts
        .config
        .reporter_epoch
        .checked_add(1)
        .checked()?;
    emit_cpi!(ConfigChanged { kind: 3 });
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct ReviewJupiter<'info> {
    pub authority: Signer<'info>,
    #[account(mut,seeds=[CONFIG_SEED],bump=config.bump,
        constraint=config.governance==authority.key() @ SimpleError::Unauthorized)]
    pub config: Box<Account<'info, Config>>,
    /// CHECK: canonical executable Jupiter identity checked against the reviewed tuple.
    pub jupiter: UncheckedAccount<'info>,
    /// CHECK: canonical loader state checked by route::validate_deployment.
    pub jupiter_program_data: UncheckedAccount<'info>,
}
pub fn review_jupiter(
    ctx: Context<ReviewJupiter>,
    deploy_slot: u64,
    programdata_len: u32,
    elf_sha256: [u8; 32],
) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    ctx.accounts.config.validate()?;
    route::validate_deployment(
        &ctx.accounts.jupiter,
        &ctx.accounts.jupiter_program_data,
        ReviewedDeployment {
            deploy_slot,
            programdata_len,
            elf_sha256,
        },
        Clock::get()?.slot,
    )?;
    ctx.accounts.config.jupiter_deploy_slot = deploy_slot;
    ctx.accounts.config.jupiter_program_data_len = programdata_len;
    ctx.accounts.config.jupiter_elf_hash = elf_sha256;
    emit_cpi!(ConfigChanged { kind: 4 });
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct RollPool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut,seeds=[MARKET_SEED,market.mint.as_ref()],bump=market.bump)]
    pub market: Box<Account<'info, Market>>,
    #[account(mut,seeds=[POOL_SEED,market.key().as_ref(),&old_pool.generation.to_le_bytes()],bump=old_pool.bump,
        constraint=old_pool.market==market.key() @ SimpleError::AccountMismatch,
        constraint=old_pool.generation==market.active_generation @ SimpleError::AccountMismatch)]
    pub old_pool: Box<Account<'info, PoolGeneration>>,
    #[account(init,payer=payer,space=PoolGeneration::SPACE,
        seeds=[POOL_SEED,market.key().as_ref(),&market.active_generation.checked_add(1).ok_or(SimpleError::Arithmetic)?.to_le_bytes()],bump)]
    pub new_pool: Box<Account<'info, PoolGeneration>>,
    #[account(address=market.mint,mint::token_program=token_program)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(init,payer=payer,seeds=[TOKEN_VAULT_SEED,new_pool.key().as_ref()],bump,
        token::mint=mint,token::authority=new_pool,token::token_program=token_program)]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(address=market.token_program)]
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
pub fn roll_pool(ctx: Context<RollPool>, max_setup_rent: u64) -> Result<()> {
    require!(
        ctx.remaining_accounts.is_empty(),
        SimpleError::AccountMismatch
    );
    require!(
        ctx.accounts.market.version == STATE_VERSION
            && ctx.accounts.old_pool.version == STATE_VERSION
            && ctx.accounts.old_pool.booked_notional_out == 0,
        SimpleError::GenerationNotEmpty
    );
    let rent = Rent::get()?;
    let required = rent
        .minimum_balance(PoolGeneration::SPACE)
        .checked_add(rent.minimum_balance(ctx.accounts.token_vault.to_account_info().data_len()))
        .checked()?;
    require_gte!(max_setup_rent, required, SimpleError::SetupRentAboveMax);
    let mut old = ctx.accounts.old_pool.core();
    shortwin_pooled_core::retire_wiped(&mut old)
        .map_err(|_| error!(SimpleError::GenerationNotEmpty))?;
    let next = shortwin_pooled_core::next_generation(&old)
        .map_err(|_| error!(SimpleError::GenerationNotEmpty))?;
    ctx.accounts.old_pool.store(&old)?;
    setup_pool(
        &mut ctx.accounts.new_pool,
        ctx.bumps.new_pool,
        ctx.bumps.token_vault,
        ctx.accounts.market.key(),
        ctx.accounts.mint.key(),
        ctx.accounts.token_program.key(),
        next.generation,
    );
    ctx.accounts.market.active_generation = next.generation;
    emit_cpi!(PoolRolled {
        market: ctx.accounts.market.key(),
        old_pool: ctx.accounts.old_pool.key(),
        new_pool: ctx.accounts.new_pool.key(),
        generation: next.generation
    });
    Ok(())
}
