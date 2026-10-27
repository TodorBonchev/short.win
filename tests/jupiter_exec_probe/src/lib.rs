//! TEST-ONLY SBF executor and hostile CPI target in one artifact, loaded at TWO test IDs.
//! Never deployed or linked into short.win. Bounds here are test inputs, not financial authority.
//! The separate pinned branch executes ONE historical Jupiter/CPMM route; it does not
//! establish production route admission, pricing or financial authorization.
use anchor_lang::prelude::{AccountInfo, Pubkey, Rent, SolanaSysvar};
use anchor_lang::solana_program::{
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    system_instruction, system_program,
};
use anchor_spl::token::spl_token::{self, instruction::AuthorityType};
use shortwin_jupiter_route::{
    isolation::{
        check_surface, measure_buyback, read_plain_scratch, RuntimeAccount, ScratchBinding,
        ScratchState,
    },
    AccountMeta as SurfaceMeta, ExpectedAccounts,
};
pub mod wire;
use wire::*;

#[cfg(not(feature = "no-entrypoint"))]
anchor_lang::solana_program::entrypoint!(process_instruction);

fn err(code: u32) -> ProgramError {
    ProgramError::Custom(code)
}
fn require(test: bool, code: u32) -> ProgramResult {
    if test {
        Ok(())
    } else {
        Err(err(code))
    }
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
fn derive(seed: &[u8], escrow: &Pubkey, program: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[seed, escrow.as_ref()], program)
}
fn token_state(a: &AccountInfo<'_>, binding: ScratchBinding) -> Result<ScratchState, ProgramError> {
    read_plain_scratch(runtime(a), &a.try_borrow_data()?, a.lamports(), binding).map_err(|e| {
        msg!("scratch rejected {:?}", e);
        err(ERR_SCRATCH)
    })
}
fn binding(
    a: &AccountInfo<'_>,
    mint: &Pubkey,
    authority: &Pubkey,
    rent: u64,
    native: bool,
) -> ScratchBinding {
    ScratchBinding {
        key: a.key.to_bytes(),
        token_program: spl_token::ID.to_bytes(),
        mint: mint.to_bytes(),
        authority: authority.to_bytes(),
        rent_lamports: rent,
        native,
    }
}
fn plain_mint(mint: &AccountInfo<'_>, native: bool) -> ProgramResult {
    require(*mint.owner == spl_token::ID && !mint.executable, ERR_MINT)?;
    let d = mint.try_borrow_data()?;
    require(d.len() == 82 && d[45] == 1 && d[46..50] == [0; 4], ERR_MINT)?;
    require(
        (mint.key == &spl_token::native_mint::ID) == native,
        ERR_MINT,
    )
}
fn move_lamports(from: &AccountInfo<'_>, to: &AccountInfo<'_>, amount: u64) -> ProgramResult {
    let left = from
        .lamports()
        .checked_sub(amount)
        .ok_or_else(|| err(ERR_FUNDING))?;
    let right = to
        .lamports()
        .checked_add(amount)
        .ok_or_else(|| err(ERR_FUNDING))?;
    **from.try_borrow_mut_lamports()? = left;
    **to.try_borrow_mut_lamports()? = right;
    Ok(())
}
fn normalize<'info>(
    scratch: &AccountInfo<'info>,
    escrow: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    seed: &[u8],
    bump: u8,
) -> ProgramResult {
    require(
        *scratch.owner == system_program::ID && scratch.data_is_empty() && !scratch.executable,
        ERR_BINDINGS,
    )?;
    if scratch.lamports() != 0 {
        let bump = [bump];
        let signer: &[&[u8]] = &[seed, escrow.key.as_ref(), &bump];
        invoke_signed(
            &system_instruction::transfer(scratch.key, escrow.key, scratch.lamports()),
            &[scratch.clone(), escrow.clone(), system.clone()],
            &[signer],
        )?;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn create_scratch<'info>(
    scratch: &AccountInfo<'info>,
    escrow: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    seed: &[u8],
    bump: u8,
    amount: u64,
) -> ProgramResult {
    let rent = Rent::get()?.minimum_balance(165);
    let funding = rent.checked_add(amount).ok_or_else(|| err(ERR_FUNDING))?;
    let remaining = escrow
        .lamports()
        .checked_sub(funding)
        .ok_or_else(|| err(ERR_FUNDING))?;
    require(
        remaining >= Rent::get()?.minimum_balance(escrow.data_len()),
        ERR_FUNDING,
    )?;
    move_lamports(escrow, scratch, funding)?;
    let bump = [bump];
    let signer: &[&[u8]] = &[seed, escrow.key.as_ref(), &bump];
    // The escrow participates ONLY in this local System CPI so the runtime sees both sides
    // of our direct lamport move. It is never forwarded to the untrusted target.
    let mut allocate = system_instruction::allocate(scratch.key, 165);
    allocate.accounts.push(AccountMeta::new(*escrow.key, false));
    invoke_signed(
        &allocate,
        &[scratch.clone(), escrow.clone(), system.clone()],
        &[signer],
    )?;
    invoke_signed(
        &system_instruction::assign(scratch.key, &spl_token::ID),
        &[scratch.clone(), system.clone()],
        &[signer],
    )?;
    let ix = spl_token::instruction::initialize_account3(
        &spl_token::ID,
        scratch.key,
        mint.key,
        authority.key,
    )?;
    invoke(&ix, &[scratch.clone(), mint.clone(), token.clone()])
}
fn transfer<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token: &AccountInfo<'info>,
    amount: u64,
    signer: &[&[u8]],
) -> ProgramResult {
    let ix = spl_token::instruction::transfer(
        &spl_token::ID,
        from.key,
        to.key,
        authority.key,
        &[],
        amount,
    )?;
    invoke_signed(
        &ix,
        &[from.clone(), to.clone(), authority.clone(), token.clone()],
        &[signer],
    )
}
fn close<'info>(
    account: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token: &AccountInfo<'info>,
    signer: &[&[u8]],
) -> ProgramResult {
    let ix = spl_token::instruction::close_account(
        &spl_token::ID,
        account.key,
        to.key,
        authority.key,
        &[],
    )?;
    invoke_signed(
        &ix,
        &[
            account.clone(),
            to.clone(),
            authority.clone(),
            token.clone(),
        ],
        &[signer],
    )
}

pub fn process_instruction(
    program: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
) -> ProgramResult {
    msg!("shortwin:test-only:jupiter-isolation-probe");
    let request = Request::decode(data).ok_or_else(|| err(ERR_INSTRUCTION))?;
    if program.to_bytes() == EXECUTOR_ID {
        execute(program, accounts, request)
    } else if program.to_bytes() == HOSTILE_TARGET_ID {
        hostile(program, accounts, request)
    } else {
        Err(err(ERR_TARGET))
    }
}

fn execute(program: &Pubkey, a: &[AccountInfo<'_>], r: Request) -> ProgramResult {
    let pinned = r.flags & FLAG_PINNED_JUPITER != 0;
    require(
        if pinned {
            a.len() == PINNED_ROUTE_ACCOUNTS
        } else {
            (FIXED_ACCOUNTS..=18).contains(&a.len())
        },
        ERR_ACCOUNTS,
    )?;
    let (
        payer,
        escrow,
        authority,
        input,
        output,
        pool,
        mint,
        wsol,
        token,
        system,
        target,
        venue_in,
        venue_out,
        venue_authority,
    ) = (
        &a[0], &a[1], &a[2], &a[3], &a[4], &a[5], &a[6], &a[7], &a[8], &a[9], &a[10], &a[11],
        &a[12], &a[13],
    );
    require(
        payer.is_signer && payer.is_writable && escrow.is_writable,
        ERR_BINDINGS,
    )?;
    let (escrow_key, _) = Pubkey::find_program_address(&[ESCROW_SEED, payer.key.as_ref()], program);
    require(
        *escrow.key == escrow_key && escrow.owner == program && !escrow.executable,
        ERR_BINDINGS,
    )?;
    require(
        *token.key == spl_token::ID
            && token.executable
            && *system.key == system_program::ID
            && system.executable,
        ERR_BINDINGS,
    )?;
    require(
        target.key.to_bytes()
            == if pinned {
                shortwin_jupiter_route::JUPITER_PROGRAM
            } else {
                HOSTILE_TARGET_ID
            }
            && target.executable,
        ERR_TARGET,
    )?;
    require(
        r.quantity > 0 && r.input_amount > 0 && r.input_amount <= r.maximum_input,
        ERR_FUNDING,
    )?;
    let (authority_key, authority_bump) = derive(AUTHORITY_SEED, escrow.key, program);
    let (input_key, input_bump) = derive(INPUT_SEED, escrow.key, program);
    let (output_key, output_bump) = derive(OUTPUT_SEED, escrow.key, program);
    let (pool_key, _) = derive(POOL_SEED, escrow.key, program);
    let (pool_owner, _) = derive(POOL_OWNER_SEED, escrow.key, program);
    let (target_authority, _) = Pubkey::find_program_address(&[TARGET_AUTHORITY_SEED], target.key);
    require(
        *authority.key == authority_key
            && *input.key == input_key
            && *output.key == output_key
            && *pool.key == pool_key
            && (pinned || *venue_authority.key == target_authority),
        ERR_BINDINGS,
    )?;
    plain_mint(mint, false)?;
    plain_mint(wsol, true)?;
    let keys = [
        *payer.key,
        *escrow.key,
        *authority.key,
        *input.key,
        *output.key,
        *pool.key,
        *mint.key,
        *wsol.key,
        *token.key,
        *system.key,
        *target.key,
        *venue_in.key,
        *venue_out.key,
        *venue_authority.key,
    ];
    for (i, key) in keys.iter().enumerate() {
        require(!keys[..i].contains(key), ERR_BINDINGS)?;
    }
    require(
        authority.owner == &system_program::ID
            && authority.data_is_empty()
            && !authority.executable,
        ERR_BINDINGS,
    )?;
    let rent = Rent::get()?.minimum_balance(165);
    let pool_binding = binding(pool, mint.key, &pool_owner, rent, false);
    let pool_before = token_state(pool, pool_binding)?;
    if pinned {
        external_vault(venue_in, wsol.key, venue_authority.key, true)?;
        external_vault(venue_out, mint.key, venue_authority.key, false)?;
    } else {
        token_state(
            venue_in,
            binding(venue_in, wsol.key, venue_authority.key, rent, true),
        )?;
        token_state(
            venue_out,
            binding(venue_out, mint.key, venue_authority.key, rent, false),
        )?;
    }

    // Both branches construct their own CPI surface, never accepting arbitrary setup/cleanup.
    // The pinned branch is ONE hardcoded historical RaydiumCP exact-input route, not a general router.
    let (metas, mut infos, cpi_data) = if pinned {
        require(r.mode == MODE_GOOD && r.target_spend == 0, ERR_INSTRUCTION)?;
        pinned_cpmm_surface(a, r)?
    } else {
        let fixed = [
            authority,
            input,
            output,
            venue_in,
            venue_out,
            venue_authority,
            wsol,
            mint,
            token,
        ];
        let mut metas = vec![
            AccountMeta::new_readonly(*authority.key, true),
            AccountMeta::new(*input.key, false),
            AccountMeta::new(*output.key, false),
            AccountMeta::new(*venue_in.key, false),
            AccountMeta::new(*venue_out.key, false),
            AccountMeta::new_readonly(*venue_authority.key, false),
            AccountMeta::new_readonly(*wsol.key, false),
            AccountMeta::new_readonly(*mint.key, false),
            AccountMeta::new_readonly(*token.key, false),
        ];
        let mut infos: Vec<_> = fixed.iter().map(|x| (*x).clone()).collect();
        for extra in &a[FIXED_ACCOUNTS..] {
            metas.push(AccountMeta {
                pubkey: *extra.key,
                is_signer: extra.is_signer,
                is_writable: extra.is_writable,
            });
            infos.push(extra.clone());
        }
        (metas, infos, r.encode().to_vec())
    };
    let proposed: Vec<_> = metas
        .iter()
        .map(|m| SurfaceMeta {
            key: m.pubkey.to_bytes(),
            is_signer: m.is_signer,
            is_writable: m.is_writable,
        })
        .collect();
    let actual: Vec<_> = infos.iter().map(runtime).collect();
    let expected = ExpectedAccounts {
        transfer_authority: authority.key.to_bytes(),
        source_token_account: input.key.to_bytes(),
        destination_token_account: output.key.to_bytes(),
        source_mint: wsol.key.to_bytes(),
        destination_mint: mint.key.to_bytes(),
        source_token_program: token.key.to_bytes(),
        destination_token_program: token.key.to_bytes(),
        shared: None,
    };
    if pinned {
        shortwin_jupiter_route::validate_header(
            &target.key.to_bytes(),
            &cpi_data,
            &proposed,
            &expected,
            shortwin_jupiter_route::AmountExpectation::ExactIn {
                input: shortwin_jupiter_route::AmountRange {
                    minimum: r.input_amount,
                    maximum: r.input_amount,
                },
                minimum_quoted_output: r.quantity,
            },
            shortwin_jupiter_route::HeaderLimits {
                maximum_data_length: 39,
                maximum_accounts: 24,
                maximum_route_steps: 1,
                maximum_slippage_bps: 0,
            },
        )
        .map_err(|e| {
            msg!("header rejected {:?}", e);
            err(ERR_BINDINGS)
        })?;
    }
    check_surface(
        &proposed,
        &actual,
        &expected,
        program.to_bytes(),
        &[
            payer.key.to_bytes(),
            escrow.key.to_bytes(),
            pool.key.to_bytes(),
        ],
        MAXIMUM_ACCOUNTS,
    )
    .map_err(|e| {
        msg!("surface rejected {:?}", e);
        err(ERR_SURFACE)
    })?;
    normalize(input, escrow, system, INPUT_SEED, input_bump)?;
    normalize(output, escrow, system, OUTPUT_SEED, output_bump)?;
    let escrow_before = escrow.lamports();
    create_scratch(
        input,
        escrow,
        wsol,
        authority,
        token,
        system,
        INPUT_SEED,
        input_bump,
        r.input_amount,
    )?;
    create_scratch(
        output,
        escrow,
        mint,
        authority,
        token,
        system,
        OUTPUT_SEED,
        output_bump,
        0,
    )?;
    let input_binding = binding(input, wsol.key, authority.key, rent, true);
    let output_binding = binding(output, mint.key, authority.key, rent, false);
    let before_in = token_state(input, input_binding)?;
    let before_out = token_state(output, output_binding)?;
    let bump = [authority_bump];
    let signer: &[&[u8]] = &[AUTHORITY_SEED, escrow.key.as_ref(), &bump];
    infos.push(target.clone());
    invoke_signed(
        &Instruction {
            program_id: *target.key,
            accounts: metas,
            data: cpi_data,
        },
        &infos,
        &[signer],
    )?;
    let after_in = token_state(input, input_binding)?;
    let after_out = token_state(output, output_binding)?;
    let delta = measure_buyback(
        before_in,
        after_in,
        before_out,
        after_out,
        r.maximum_input,
        r.quantity,
    )
    .map_err(|e| {
        msg!("delivery rejected {:?}", e);
        err(ERR_DELIVERY)
    })?;
    // Every received token, including surplus, is donated to the protected pool. The pool
    // and its authority were absent from the target CPI, so the target cannot recycle credit.
    transfer(
        output,
        pool,
        authority,
        token,
        delta.received_tokens,
        signer,
    )?;
    let pool_after = token_state(pool, pool_binding)?;
    require(
        pool_after.amount().checked_sub(pool_before.amount()) == Some(delta.received_tokens)
            && delta.received_tokens >= r.quantity,
        ERR_POOL_CREDIT,
    )?;
    close(input, escrow, authority, token, signer)?;
    close(output, escrow, authority, token, signer)?;
    require(
        [input, output]
            .iter()
            .all(|x| x.lamports() == 0 && x.data_is_empty() && *x.owner == system_program::ID),
        ERR_REFUND,
    )?;
    require(
        escrow_before.checked_sub(delta.spent_lamports) == Some(escrow.lamports()),
        ERR_REFUND,
    )?;
    msg!(
        "probe spend={} pool_credit={} escrow_debit={}",
        delta.spent_lamports,
        delta.received_tokens,
        escrow_before.saturating_sub(escrow.lamports())
    );
    require(r.flags & FORCE_POST_FAILURE == 0, ERR_FORCED_POST)
}

fn hostile(program: &Pubkey, a: &[AccountInfo<'_>], r: Request) -> ProgramResult {
    require(a.len() >= 9, ERR_ACCOUNTS)?;
    let (authority, input, output, venue_in, venue_out, venue_authority, token) =
        (&a[0], &a[1], &a[2], &a[3], &a[4], &a[5], &a[8]);
    require(
        authority.is_signer && *token.key == spl_token::ID,
        ERR_BINDINGS,
    )?;
    let (target_authority, bump) = Pubkey::find_program_address(&[TARGET_AUTHORITY_SEED], program);
    require(*venue_authority.key == target_authority, ERR_BINDINGS)?;
    // The incoming authority is signed only by the executor's invoke_signed.
    let ix = spl_token::instruction::transfer(
        &spl_token::ID,
        input.key,
        venue_in.key,
        authority.key,
        &[],
        r.target_spend,
    )?;
    invoke(
        &ix,
        &[
            input.clone(),
            venue_in.clone(),
            authority.clone(),
            token.clone(),
        ],
    )?;
    let bump = [bump];
    let signer: &[&[u8]] = &[TARGET_AUTHORITY_SEED, &bump];
    transfer(
        venue_out,
        output,
        venue_authority,
        token,
        r.target_delivery,
        signer,
    )?;
    match r.mode {
        MODE_GOOD => Ok(()),
        MODE_DELEGATE => {
            let ix = spl_token::instruction::approve(
                &spl_token::ID,
                output.key,
                venue_authority.key,
                authority.key,
                &[],
                1,
            )?;
            invoke(
                &ix,
                &[
                    output.clone(),
                    venue_authority.clone(),
                    authority.clone(),
                    token.clone(),
                ],
            )
        }
        MODE_OWNER => {
            let ix = spl_token::instruction::set_authority(
                &spl_token::ID,
                output.key,
                Some(venue_authority.key),
                AuthorityType::AccountOwner,
                authority.key,
                &[],
            )?;
            invoke(&ix, &[output.clone(), authority.clone(), token.clone()])
        }
        MODE_EARLY_CLOSE => {
            let ix = spl_token::instruction::close_account(
                &spl_token::ID,
                output.key,
                venue_in.key,
                authority.key,
                &[],
            )?;
            invoke(
                &ix,
                &[
                    output.clone(),
                    venue_in.clone(),
                    authority.clone(),
                    token.clone(),
                ],
            )
        }
        MODE_STEAL_NATIVE => {
            let ix = spl_token::instruction::close_account(
                &spl_token::ID,
                input.key,
                venue_in.key,
                authority.key,
                &[],
            )?;
            invoke(
                &ix,
                &[
                    input.clone(),
                    venue_in.clone(),
                    authority.clone(),
                    token.clone(),
                ],
            )
        }
        MODE_REENTER => invoke(
            &Instruction {
                program_id: Pubkey::new_from_array(EXECUTOR_ID),
                accounts: vec![],
                data: r.encode().to_vec(),
            },
            &[],
        ),
        _ => Err(err(ERR_INSTRUCTION)),
    }
}

/// Exact one-hop public-IDL vector. Successful SBF execution is required separately.
type CpiSurface<'info> = (Vec<AccountMeta>, Vec<AccountInfo<'info>>, Vec<u8>);

fn pinned_cpmm_surface<'info>(
    a: &[AccountInfo<'info>],
    r: Request,
) -> Result<CpiSurface<'info>, ProgramError> {
    require(
        a[14].key.to_bytes() == PINNED_CPMM_PROGRAM && a[14].executable,
        ERR_TARGET,
    )?;
    require(
        a[18].key.to_bytes() == shortwin_jupiter_route::EVENT_AUTHORITY,
        ERR_BINDINGS,
    )?;
    require(
        a[15].owner == a[14].key && a[16].owner == a[14].key && a[17].owner == a[14].key,
        ERR_BINDINGS,
    )?;
    let roles = [
        (2, true, false),
        (3, false, true),
        (4, false, true),
        (7, false, false),
        (6, false, false),
        (8, false, false),
        (8, false, false),
        (10, false, false),
        (18, false, false),
        (10, false, false),
        (14, false, false),
        (2, true, false),
        (13, false, false),
        (15, false, false),
        (16, false, true),
        (3, false, true),
        (4, false, true),
        (11, false, true),
        (12, false, true),
        (8, false, false),
        (8, false, false),
        (7, false, false),
        (6, false, false),
        (17, false, true),
    ];
    let metas = roles
        .iter()
        .map(|(i, signer, writable)| AccountMeta {
            pubkey: *a[*i].key,
            is_signer: *signer,
            is_writable: *writable,
        })
        .collect();
    let infos = roles.iter().map(|(i, _, _)| a[*i].clone()).collect();
    let mut data = shortwin_jupiter_route::ROUTE_V2.to_vec();
    data.extend_from_slice(&r.input_amount.to_le_bytes());
    data.extend_from_slice(&r.target_delivery.to_le_bytes());
    data.extend_from_slice(&[0; 6]);
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&[46, 0x10, 0x27, 0, 1]);
    Ok((metas, infos, data))
}

/// External historical DEX vaults are NOT freshly allocated scratch. Their stored native
/// reserve can predate today's rent and they may hold harmless lamport donations. Never
/// rewrite those fixture bytes or apply their old reserve to protocol scratch funding.
fn external_vault(
    a: &AccountInfo<'_>,
    mint: &Pubkey,
    authority: &Pubkey,
    native: bool,
) -> ProgramResult {
    require(*a.owner == spl_token::ID && !a.executable, ERR_BINDINGS)?;
    let d = a.try_borrow_data()?;
    require(
        d.len() == 165
            && d[..32] == mint.to_bytes()
            && d[32..64] == authority.to_bytes()
            && d[108] == 1,
        ERR_BINDINGS,
    )?;
    require(
        d[72..76] == [0; 4] && d[121..129] == [0; 8] && d[129..133] == [0; 4],
        ERR_BINDINGS,
    )?;
    if native {
        require(d[109..113] == [1, 0, 0, 0], ERR_BINDINGS)?;
        let mut raw = [0; 8];
        raw.copy_from_slice(&d[113..121]);
        let reserve = u64::from_le_bytes(raw);
        raw.copy_from_slice(&d[64..72]);
        let amount = u64::from_le_bytes(raw);
        require(
            reserve > 0
                && reserve
                    .checked_add(amount)
                    .is_some_and(|minimum| a.lamports() >= minimum),
            ERR_BINDINGS,
        )
    } else {
        require(d[109..113] == [0; 4], ERR_BINDINGS)
    }
}
