//! One isolated Jupiter Router V2 execution path for the new pooled program.
//! Lending decisions remain in the caller; only loan-local scratch enters Jupiter.
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    bpf_loader_upgradeable,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    system_instruction, system_program,
};
use anchor_spl::token_interface::{self, CloseAccount, InitializeAccount3, TransferChecked};
use header::isolation::{self, RuntimeAccount, ScratchBinding, ScratchState};
use shortwin_jupiter_route as header;

use crate::errors::SimpleError;

pub const JUPITER_ID: Pubkey = Pubkey::new_from_array(header::JUPITER_PROGRAM);
pub const JUPITER_PROGRAM_DATA: Pubkey = pubkey!("4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT");
pub const TOKEN_ID: Pubkey = Pubkey::new_from_array(header::SPL_TOKEN_PROGRAM);
pub const TOKEN_2022_ID: Pubkey = Pubkey::new_from_array(header::TOKEN_2022_PROGRAM);
pub const WSOL_MINT: Pubkey = Pubkey::new_from_array(isolation::WSOL_MINT);
pub const AUTHORITY_SEED: &[u8] = b"simple_exec";
pub const BASE_SEED: &[u8] = b"simple_base";
pub const WSOL_SEED: &[u8] = b"simple_wsol";
pub const TOKEN_ACCOUNT_LEN: usize = 165;
pub const MAX_ROUTE_ACCOUNTS: usize = 48;
pub const MAX_ROUTE_BYTES: usize = 768;
pub const MAX_ROUTE_STEPS: u32 = 8;
pub const MAX_ROUTE_SLIPPAGE_BPS: u16 = 1000;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct RouteArgs {
    pub input_amount: u64,
    pub data: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewedDeployment {
    pub deploy_slot: u64,
    pub programdata_len: u32,
    pub elf_sha256: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sale {
    pub proceeds: u64,
    /// Arbitrary prior System-account prefunding is not trade proceeds.
    pub recovered_prefund: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buyback {
    pub spent: u64,
    pub pool_credit: u64,
    pub recovered_prefund: u64,
}
/// The financial handler must bind these identities to its actual loan and pool.
pub struct RouteAccounts<'a, 'info> {
    pub loan: &'a AccountInfo<'info>,
    pub pool: &'a AccountInfo<'info>,
    pub token_vault: &'a AccountInfo<'info>,
    pub mint: &'a AccountInfo<'info>,
    pub base_program: &'a AccountInfo<'info>,
    pub execution_authority: &'a AccountInfo<'info>,
    pub execution_base: &'a AccountInfo<'info>,
    pub execution_wsol: &'a AccountInfo<'info>,
    pub wsol_mint: &'a AccountInfo<'info>,
    pub wsol_token_program: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
    pub jupiter: &'a AccountInfo<'info>,
    pub jupiter_program_data: &'a AccountInfo<'info>,
    /// Borrower, keeper, treasury and any caller-specific sensitive identities.
    pub forbidden: &'a [Pubkey],
}
fn amount_error() -> anchor_lang::error::Error {
    error!(SimpleError::RouteBalance)
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| error!(SimpleError::MathOverflow))
}
fn sub(a: u64, b: u64) -> Result<u64> {
    a.checked_sub(b).ok_or_else(amount_error)
}
fn runtime(a: &AccountInfo<'_>) -> RuntimeAccount {
    RuntimeAccount {
        key: a.key.to_bytes(),
        owner: a.owner.to_bytes(),
        is_signer: a.is_signer,
        is_writable: a.is_writable,
        executable: a.executable,
    }
}
fn derive(seed: &[u8], loan: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[seed, loan.as_ref()], &crate::ID)
}
/// A matching reviewed loader instance is checked each use. The digest is a
/// governance attestation, not a claim that the whole ELF is hashed on each trade.
pub fn validate_deployment(
    program: &AccountInfo<'_>,
    data: &AccountInfo<'_>,
    reviewed: ReviewedDeployment,
    current_slot: u64,
) -> Result<()> {
    require!(
        program.key == &JUPITER_ID
            && program.owner == &bpf_loader_upgradeable::ID
            && program.executable
            && !program.is_writable
            && data.key == &JUPITER_PROGRAM_DATA
            && data.owner == &bpf_loader_upgradeable::ID
            && !data.executable
            && !data.is_writable,
        SimpleError::RouteAccounts
    );
    let program_bytes = program.try_borrow_data()?;
    let data_bytes = data.try_borrow_data()?;
    require!(
        program_bytes.len() == 36
            && program_bytes[..4] == 2u32.to_le_bytes()
            && program_bytes[4..36] == JUPITER_PROGRAM_DATA.to_bytes()
            && data_bytes.len() >= 45
            && data_bytes[..4] == 3u32.to_le_bytes()
            && data_bytes[12] <= 1
            && reviewed.deploy_slot > 0
            && reviewed.elf_sha256 != [0; 32]
            && usize::try_from(reviewed.programdata_len).ok() == Some(data_bytes.len()),
        SimpleError::RouteAccounts
    );
    let mut encoded = [0; 8];
    encoded.copy_from_slice(&data_bytes[4..12]);
    require!(
        u64::from_le_bytes(encoded) == reviewed.deploy_slot && current_slot > reviewed.deploy_slot,
        SimpleError::RouteAccounts
    );
    Ok(())
}
/// Only plain initialized SPL/T22 mints in the first implementation. A quote or
/// token name is not evidence that extension-bearing/frozen behavior is supported.
pub fn supported_mint(mint: &AccountInfo<'_>, token_program: &Pubkey) -> Result<u8> {
    require!(
        [TOKEN_ID, TOKEN_2022_ID].contains(token_program)
            && mint.owner == token_program
            && !mint.executable
            && !mint.is_writable
            && mint.key != &WSOL_MINT,
        SimpleError::RouteMintUnsupported
    );
    let bytes = mint.try_borrow_data()?;
    require!(
        bytes.len() == 82 && bytes[45] == 1 && bytes[46..50] == [0; 4],
        SimpleError::RouteMintUnsupported
    );
    Ok(bytes[44])
}
fn plain_token(
    account: &AccountInfo<'_>,
    program: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
) -> Result<[u8; 165]> {
    require!(
        account.owner == program && !account.executable && account.is_writable,
        SimpleError::RouteAccounts
    );
    let bytes = account.try_borrow_data()?;
    require!(bytes.len() == TOKEN_ACCOUNT_LEN, SimpleError::RouteAccounts);
    let mut copy = [0; 165];
    copy.copy_from_slice(&bytes);
    require!(
        copy[..32] == mint.to_bytes()
            && copy[32..64] == owner.to_bytes()
            && copy[72..76] == [0; 4]
            && copy[108] == 1
            && copy[109..113] == [0; 4]
            && copy[121..129] == [0; 8]
            && copy[129..133] == [0; 4],
        SimpleError::RouteAccounts
    );
    Ok(copy)
}
fn token_amount(bytes: &[u8; 165]) -> u64 {
    let mut n = [0; 8];
    n.copy_from_slice(&bytes[64..72]);
    u64::from_le_bytes(n)
}
fn same_token_metadata(mut before: [u8; 165], after: [u8; 165]) -> bool {
    before[64..72].copy_from_slice(&after[64..72]);
    before == after
}
fn move_owned(from: &AccountInfo<'_>, to: &AccountInfo<'_>, amount: u64) -> Result<()> {
    require!(
        from.owner == &crate::ID && from.key != to.key && from.is_writable && to.is_writable,
        SimpleError::RouteAccounts
    );
    let debit = sub(from.lamports(), amount)?;
    let credit = add(to.lamports(), amount)?;
    **from.try_borrow_mut_lamports()? = debit;
    **to.try_borrow_mut_lamports()? = credit;
    Ok(())
}
impl RouteAccounts<'_, '_> {
    fn validate(&self, reviewed: ReviewedDeployment, clock: Clock) -> Result<[u8; 3]> {
        validate_deployment(
            self.jupiter,
            self.jupiter_program_data,
            reviewed,
            clock.slot,
        )?;
        require!(
            self.loan.owner == &crate::ID
                && self.loan.is_writable
                && !self.loan.executable
                && self.pool.owner == &crate::ID
                && !self.pool.executable
                && self.pool.key != self.loan.key,
            SimpleError::RouteAccounts
        );
        let (authority, ab) = derive(AUTHORITY_SEED, self.loan.key);
        let (base, bb) = derive(BASE_SEED, self.loan.key);
        let (wsol, wb) = derive(WSOL_SEED, self.loan.key);
        require!(
            self.execution_authority.key == &authority
                && !self.execution_authority.is_writable
                && !self.execution_authority.is_signer
                && self.execution_authority.owner == &system_program::ID
                && self.execution_authority.data_is_empty()
                && !self.execution_authority.executable
                && self.execution_base.key == &base
                && self.execution_wsol.key == &wsol
                && self.execution_base.is_writable
                && self.execution_wsol.is_writable,
            SimpleError::RouteAccounts
        );
        require!(
            self.wsol_mint.key == &WSOL_MINT
                && self.wsol_mint.owner == &TOKEN_ID
                && !self.wsol_mint.is_writable
                && !self.wsol_mint.executable
                && self.wsol_token_program.key == &TOKEN_ID
                && self.wsol_token_program.executable
                && self.base_program.executable
                && self.system_program.key == &system_program::ID
                && self.system_program.executable,
            SimpleError::RouteAccounts
        );
        supported_mint(self.mint, self.base_program.key)?;
        plain_token(
            self.token_vault,
            self.base_program.key,
            self.mint.key,
            self.pool.key,
        )?;
        Ok([ab, bb, wb])
    }
    fn binding(&self, native: bool, rent: u64) -> ScratchBinding {
        ScratchBinding {
            key: if native {
                self.execution_wsol.key.to_bytes()
            } else {
                self.execution_base.key.to_bytes()
            },
            token_program: if native {
                TOKEN_ID.to_bytes()
            } else {
                self.base_program.key.to_bytes()
            },
            mint: if native {
                WSOL_MINT.to_bytes()
            } else {
                self.mint.key.to_bytes()
            },
            authority: self.execution_authority.key.to_bytes(),
            rent_lamports: rent,
            native,
        }
    }
    fn scratch_state(&self, native: bool, rent: u64) -> Result<ScratchState> {
        let account = if native {
            self.execution_wsol
        } else {
            self.execution_base
        };
        isolation::read_plain_scratch(
            runtime(account),
            &account.try_borrow_data()?,
            account.lamports(),
            self.binding(native, rent),
        )
        .map_err(|_| amount_error())
    }
}
fn normalize_prefund<'info>(
    account: &AccountInfo<'info>,
    loan: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    seeds: &[&[u8]],
) -> Result<u64> {
    require!(
        account.owner == &system_program::ID && account.data_is_empty() && !account.executable,
        SimpleError::RouteAccounts
    );
    let amount = account.lamports();
    if amount > 0 {
        invoke_signed(
            &system_instruction::transfer(account.key, loan.key, amount),
            &[account.clone(), loan.clone(), system.clone()],
            &[seeds],
        )?;
    }
    Ok(amount)
}
#[allow(clippy::too_many_arguments)]
fn initialize_scratch<'info>(
    account: &AccountInfo<'info>,
    loan: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    seeds: &[&[u8]],
    rent: u64,
    amount: u64,
) -> Result<()> {
    move_owned(loan, account, add(rent, amount)?)?;
    // Include both sides of the direct loan-to-scratch rent move in this local
    // System CPI. The financial account is never forwarded to Jupiter.
    let mut allocate = system_instruction::allocate(account.key, TOKEN_ACCOUNT_LEN as u64);
    allocate.accounts.push(AccountMeta::new(*loan.key, false));
    invoke_signed(
        &allocate,
        &[account.clone(), loan.clone(), system.clone()],
        &[seeds],
    )?;
    invoke_signed(
        &system_instruction::assign(account.key, token_program.key),
        &[account.clone(), system.clone()],
        &[seeds],
    )?;
    token_interface::initialize_account3(CpiContext::new(
        token_program.key(),
        InitializeAccount3 {
            account: account.clone(),
            mint: mint.clone(),
            authority: authority.clone(),
        },
    ))?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn token_transfer<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    program: &AccountInfo<'info>,
    seeds: &[&[u8]],
    amount: u64,
    decimals: u8,
) -> Result<()> {
    token_interface::transfer_checked(
        CpiContext::new(
            program.key(),
            TransferChecked {
                from: from.clone(),
                mint: mint.clone(),
                to: to.clone(),
                authority: authority.clone(),
            },
        )
        .with_signer(&[seeds]),
        amount,
        decimals,
    )
}
fn close_scratch<'info>(a: &RouteAccounts<'_, 'info>, seeds: &[&[u8]]) -> Result<()> {
    for (account, program) in [
        (a.execution_base, a.base_program),
        (a.execution_wsol, a.wsol_token_program),
    ] {
        token_interface::close_account(
            CpiContext::new(
                program.key(),
                CloseAccount {
                    account: account.clone(),
                    destination: a.loan.clone(),
                    authority: a.execution_authority.clone(),
                },
            )
            .with_signer(&[seeds]),
        )?;
    }
    require!(
        a.execution_base.lamports() == 0
            && a.execution_base.data_is_empty()
            && a.execution_base.owner == &system_program::ID
            && a.execution_wsol.lamports() == 0
            && a.execution_wsol.data_is_empty()
            && a.execution_wsol.owner == &system_program::ID,
        SimpleError::RouteBalance
    );
    Ok(())
}
fn invoke_jupiter<'info>(
    a: &RouteAccounts<'_, 'info>,
    remaining: &[AccountInfo<'info>],
    args: &RouteArgs,
    seeds: &[&[u8]],
    buy: bool,
    minimum_output: u64,
) -> Result<()> {
    require!(
        args.input_amount > 0
            && minimum_output > 0
            && remaining.len() <= MAX_ROUTE_ACCOUNTS
            && args.data.len() <= MAX_ROUTE_BYTES,
        SimpleError::RouteInvalid
    );
    let (source, destination, source_mint, destination_mint, source_program, destination_program) =
        if buy {
            (
                a.execution_wsol,
                a.execution_base,
                a.wsol_mint,
                a.mint,
                a.wsol_token_program,
                a.base_program,
            )
        } else {
            (
                a.execution_base,
                a.execution_wsol,
                a.mint,
                a.wsol_mint,
                a.base_program,
                a.wsol_token_program,
            )
        };
    let expected = header::ExpectedAccounts {
        transfer_authority: a.execution_authority.key.to_bytes(),
        source_token_account: source.key.to_bytes(),
        destination_token_account: destination.key.to_bytes(),
        source_mint: source_mint.key.to_bytes(),
        destination_mint: destination_mint.key.to_bytes(),
        source_token_program: source_program.key.to_bytes(),
        destination_token_program: destination_program.key.to_bytes(),
        shared: None,
    };
    let mut proposed = Vec::with_capacity(remaining.len());
    let mut actual = Vec::with_capacity(remaining.len());
    let mut metas = Vec::with_capacity(remaining.len());
    for account in remaining {
        let signer = account.key == a.execution_authority.key;
        proposed.push(header::AccountMeta {
            key: account.key.to_bytes(),
            is_signer: signer,
            is_writable: account.is_writable,
        });
        actual.push(runtime(account));
        metas.push(AccountMeta {
            pubkey: *account.key,
            is_signer: signer,
            is_writable: account.is_writable,
        });
    }
    let parsed = header::validate_header(
        &JUPITER_ID.to_bytes(),
        &args.data,
        &proposed,
        &expected,
        header::AmountExpectation::ExactIn {
            input: header::AmountRange {
                minimum: args.input_amount,
                maximum: args.input_amount,
            },
            minimum_quoted_output: minimum_output,
        },
        header::HeaderLimits {
            maximum_data_length: MAX_ROUTE_BYTES,
            maximum_accounts: MAX_ROUTE_ACCOUNTS,
            maximum_route_steps: MAX_ROUTE_STEPS,
            maximum_slippage_bps: MAX_ROUTE_SLIPPAGE_BPS,
        },
    )
    .map_err(|_| error!(SimpleError::RouteInvalid))?;
    require!(
        parsed.parsed().kind() == header::RouteKind::ExactIn,
        SimpleError::RouteInvalid
    );
    let mut forbidden = Vec::with_capacity(a.forbidden.len() + 4);
    forbidden.extend(a.forbidden.iter().map(|key| key.to_bytes()));
    forbidden.extend([
        a.loan.key.to_bytes(),
        a.pool.key.to_bytes(),
        a.token_vault.key.to_bytes(),
        a.jupiter_program_data.key.to_bytes(),
    ]);
    isolation::check_surface(
        &proposed,
        &actual,
        &expected,
        crate::ID.to_bytes(),
        &forbidden,
        MAX_ROUTE_ACCOUNTS,
    )
    .map_err(|_| error!(SimpleError::RouteAccounts))?;
    let ix = Instruction {
        program_id: JUPITER_ID,
        accounts: metas,
        data: args.data.clone(),
    };
    let mut infos = remaining.to_vec();
    infos.push(a.jupiter.clone());
    invoke_signed(&ix, &infos, &[seeds])?;
    Ok(())
}

/// Required temporary peak rent. It is returned after every successful route.
pub fn temporary_rent() -> Result<u64> {
    Rent::get()?
        .minimum_balance(TOKEN_ACCOUNT_LEN)
        .checked_mul(2)
        .ok_or_else(|| error!(SimpleError::MathOverflow))
}
fn available(loan: &AccountInfo<'_>, floor: u64, need: u64) -> Result<()> {
    require!(
        loan.lamports() >= add(floor, need)?,
        SimpleError::InsufficientFunds
    );
    Ok(())
}

/// Sell exactly q newly transferred pool tokens. The caller applies principal/fee
/// booking only after this succeeds. No pool/loan signer reaches Jupiter.
#[allow(clippy::too_many_arguments)]
pub fn sell<'info>(
    a: &RouteAccounts<'_, 'info>,
    remaining: &[AccountInfo<'info>],
    args: &RouteArgs,
    pool_signer: &[&[u8]],
    quantity: u64,
    minimum_proceeds: u64,
    loan_floor: u64,
    reviewed: ReviewedDeployment,
) -> Result<Sale> {
    require!(
        quantity > 0 && args.input_amount == quantity,
        SimpleError::RouteInvalid
    );
    let bumps = a.validate(reviewed, Clock::get()?)?;
    let rent = Rent::get()?.minimum_balance(TOKEN_ACCOUNT_LEN);
    let start = a.loan.lamports();
    let token_lamports = a.token_vault.lamports();
    let before = plain_token(a.token_vault, a.base_program.key, a.mint.key, a.pool.key)?;
    let base_bump = [bumps[1]];
    let wsol_bump = [bumps[2]];
    let authority_bump = [bumps[0]];
    let base_seeds: &[&[u8]] = &[BASE_SEED, a.loan.key.as_ref(), &base_bump];
    let wsol_seeds: &[&[u8]] = &[WSOL_SEED, a.loan.key.as_ref(), &wsol_bump];
    let authority_seeds: &[&[u8]] = &[AUTHORITY_SEED, a.loan.key.as_ref(), &authority_bump];
    let prefund = add(
        normalize_prefund(a.execution_base, a.loan, a.system_program, base_seeds)?,
        normalize_prefund(a.execution_wsol, a.loan, a.system_program, wsol_seeds)?,
    )?;
    available(a.loan, loan_floor, add(rent, rent)?)?;
    initialize_scratch(
        a.execution_base,
        a.loan,
        a.mint,
        a.execution_authority,
        a.base_program,
        a.system_program,
        base_seeds,
        rent,
        0,
    )?;
    initialize_scratch(
        a.execution_wsol,
        a.loan,
        a.wsol_mint,
        a.execution_authority,
        a.wsol_token_program,
        a.system_program,
        wsol_seeds,
        rent,
        0,
    )?;
    let decimals = supported_mint(a.mint, a.base_program.key)?;
    token_transfer(
        a.token_vault,
        a.execution_base,
        a.mint,
        a.pool,
        a.base_program,
        pool_signer,
        quantity,
        decimals,
    )?;
    require_eq!(
        a.scratch_state(false, rent)?.amount(),
        quantity,
        SimpleError::RouteBalance
    );
    require_eq!(
        a.scratch_state(true, rent)?.amount(),
        0,
        SimpleError::RouteBalance
    );
    invoke_jupiter(a, remaining, args, authority_seeds, false, minimum_proceeds)?;
    require_eq!(
        a.scratch_state(false, rent)?.amount(),
        0,
        SimpleError::RouteBalance
    );
    let proceeds = a.scratch_state(true, rent)?.amount();
    require!(proceeds >= minimum_proceeds, SimpleError::RouteOutputShort);
    close_scratch(a, authority_seeds)?;
    let after = plain_token(a.token_vault, a.base_program.key, a.mint.key, a.pool.key)?;
    require!(
        same_token_metadata(before, after)
            && sub(token_amount(&before), token_amount(&after))? == quantity
            && a.token_vault.lamports() == token_lamports
            && a.loan.lamports() == add(add(start, prefund)?, proceeds)?,
        SimpleError::RouteBalance
    );
    Ok(Sale {
        proceeds,
        recovered_prefund: prefund,
    })
}

/// Spend bounded isolated WSOL, transfer every received base token into the pool,
/// and verify new net pool credit >= the whole debt before the caller releases it.
#[allow(clippy::too_many_arguments)]
pub fn buyback<'info>(
    a: &RouteAccounts<'_, 'info>,
    remaining: &[AccountInfo<'info>],
    args: &RouteArgs,
    maximum_spend: u64,
    required_quantity: u64,
    loan_floor: u64,
    reviewed: ReviewedDeployment,
) -> Result<Buyback> {
    require!(
        required_quantity > 0 && args.input_amount > 0 && args.input_amount <= maximum_spend,
        SimpleError::RouteInvalid
    );
    let bumps = a.validate(reviewed, Clock::get()?)?;
    let rent = Rent::get()?.minimum_balance(TOKEN_ACCOUNT_LEN);
    let start = a.loan.lamports();
    let token_lamports = a.token_vault.lamports();
    let before = plain_token(a.token_vault, a.base_program.key, a.mint.key, a.pool.key)?;
    let base_bump = [bumps[1]];
    let wsol_bump = [bumps[2]];
    let authority_bump = [bumps[0]];
    let base_seeds: &[&[u8]] = &[BASE_SEED, a.loan.key.as_ref(), &base_bump];
    let wsol_seeds: &[&[u8]] = &[WSOL_SEED, a.loan.key.as_ref(), &wsol_bump];
    let authority_seeds: &[&[u8]] = &[AUTHORITY_SEED, a.loan.key.as_ref(), &authority_bump];
    let prefund = add(
        normalize_prefund(a.execution_base, a.loan, a.system_program, base_seeds)?,
        normalize_prefund(a.execution_wsol, a.loan, a.system_program, wsol_seeds)?,
    )?;
    available(
        a.loan,
        loan_floor,
        add(add(rent, rent)?, args.input_amount)?,
    )?;
    initialize_scratch(
        a.execution_base,
        a.loan,
        a.mint,
        a.execution_authority,
        a.base_program,
        a.system_program,
        base_seeds,
        rent,
        0,
    )?;
    initialize_scratch(
        a.execution_wsol,
        a.loan,
        a.wsol_mint,
        a.execution_authority,
        a.wsol_token_program,
        a.system_program,
        wsol_seeds,
        rent,
        args.input_amount,
    )?;
    let source_before = a.scratch_state(true, rent)?;
    let destination_before = a.scratch_state(false, rent)?;
    invoke_jupiter(a, remaining, args, authority_seeds, true, required_quantity)?;
    let source_after = a.scratch_state(true, rent)?;
    let destination_after = a.scratch_state(false, rent)?;
    let deltas = isolation::measure_buyback(
        source_before,
        source_after,
        destination_before,
        destination_after,
        maximum_spend,
        required_quantity,
    )
    .map_err(|_| error!(SimpleError::RouteOutputShort))?;
    let decimals = supported_mint(a.mint, a.base_program.key)?;
    token_transfer(
        a.execution_base,
        a.token_vault,
        a.mint,
        a.execution_authority,
        a.base_program,
        authority_seeds,
        deltas.received_tokens,
        decimals,
    )?;
    require_eq!(
        a.scratch_state(false, rent)?.amount(),
        0,
        SimpleError::RouteBalance
    );
    close_scratch(a, authority_seeds)?;
    let after = plain_token(a.token_vault, a.base_program.key, a.mint.key, a.pool.key)?;
    let credit = sub(token_amount(&after), token_amount(&before))?;
    require!(
        same_token_metadata(before, after)
            && credit == deltas.received_tokens
            && credit >= required_quantity
            && a.token_vault.lamports() == token_lamports
            && a.loan.lamports() == sub(add(start, prefund)?, deltas.spent_lamports)?,
        SimpleError::RouteBalance
    );
    Ok(Buyback {
        spent: deltas.spent_lamports,
        pool_credit: credit,
        recovered_prefund: prefund,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn deployment(
        program_owner: Pubkey,
        data_key: Pubkey,
        slot: u64,
        length: usize,
        expected: ReviewedDeployment,
        current_slot: u64,
    ) -> bool {
        let mut program_bytes = vec![0u8; 36];
        program_bytes[..4].copy_from_slice(&2u32.to_le_bytes());
        program_bytes[4..].copy_from_slice(JUPITER_PROGRAM_DATA.as_ref());
        let mut data_bytes = vec![0u8; length];
        if data_bytes.len() >= 45 {
            data_bytes[..4].copy_from_slice(&3u32.to_le_bytes());
            data_bytes[4..12].copy_from_slice(&slot.to_le_bytes());
            data_bytes[12] = 1;
        }
        let mut pl = 1;
        let mut dl = 1;
        let program = AccountInfo::new(
            &JUPITER_ID,
            false,
            false,
            &mut pl,
            &mut program_bytes,
            &program_owner,
            true,
        );
        let data = AccountInfo::new(
            &data_key,
            false,
            false,
            &mut dl,
            &mut data_bytes,
            &bpf_loader_upgradeable::ID,
            false,
        );
        validate_deployment(&program, &data, expected, current_slot).is_ok()
    }
    #[test]
    fn deployment_tuple_binds_current_loader_instance_without_a_fresh_hash_claim() {
        let expected = ReviewedDeployment {
            deploy_slot: 42,
            programdata_len: 80,
            elf_sha256: [7; 32],
        };
        assert!(deployment(
            bpf_loader_upgradeable::ID,
            JUPITER_PROGRAM_DATA,
            42,
            80,
            expected,
            43
        ));
        assert!(!deployment(
            system_program::ID,
            JUPITER_PROGRAM_DATA,
            42,
            80,
            expected,
            43
        ));
        assert!(!deployment(
            bpf_loader_upgradeable::ID,
            Pubkey::new_unique(),
            42,
            80,
            expected,
            43
        ));
        assert!(!deployment(
            bpf_loader_upgradeable::ID,
            JUPITER_PROGRAM_DATA,
            43,
            80,
            expected,
            44
        ));
        assert!(!deployment(
            bpf_loader_upgradeable::ID,
            JUPITER_PROGRAM_DATA,
            42,
            81,
            expected,
            43
        ));
        assert!(!deployment(
            bpf_loader_upgradeable::ID,
            JUPITER_PROGRAM_DATA,
            42,
            80,
            expected,
            42
        ));
        let missing = ReviewedDeployment {
            elf_sha256: [0; 32],
            ..expected
        };
        assert!(!deployment(
            bpf_loader_upgradeable::ID,
            JUPITER_PROGRAM_DATA,
            42,
            80,
            missing,
            43
        ));
    }
    #[test]
    fn mint_scope_is_plain_initialized_unfrozen_spl_or_token2022() {
        for owner in [TOKEN_ID, TOKEN_2022_ID] {
            let key = Pubkey::new_unique();
            let mut lamports = 1;
            let mut bytes = vec![0; 82];
            bytes[44] = 6;
            bytes[45] = 1;
            {
                let account =
                    AccountInfo::new(&key, false, false, &mut lamports, &mut bytes, &owner, false);
                assert_eq!(supported_mint(&account, &owner).unwrap(), 6);
            }
            bytes.push(0);
            {
                let account =
                    AccountInfo::new(&key, false, false, &mut lamports, &mut bytes, &owner, false);
                assert!(supported_mint(&account, &owner).is_err());
            }
            bytes.truncate(82);
            bytes[46] = 1;
            let account =
                AccountInfo::new(&key, false, false, &mut lamports, &mut bytes, &owner, false);
            assert!(supported_mint(&account, &owner).is_err());
        }
    }
    #[test]
    fn token_metadata_comparison_allows_only_the_amount_to_change() {
        let mut before = [0u8; 165];
        before[108] = 1;
        let mut after = before;
        after[64..72].copy_from_slice(&99u64.to_le_bytes());
        assert!(same_token_metadata(before, after));
        for at in [0, 32, 72, 108, 109, 121, 129, 164] {
            let mut altered = after;
            altered[at] ^= 1;
            assert!(!same_token_metadata(before, altered), "metadata at {at}");
        }
    }
}
