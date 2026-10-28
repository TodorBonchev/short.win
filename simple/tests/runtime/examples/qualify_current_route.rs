//! Opt-in, read-only-snapshot replay through the actual new financial program.
//! This does not send an RPC transaction. Candidate snapshots never replace the
//! historical regression baseline. Lender inventory, collateral, reference
//! history and test signers are synthetic; mint/DEX/program bytes are untouched.
use anchor_lang::{prelude::Pubkey, solana_program::clock::Clock};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use shortwin_simple::{
    accounts,
    exits::CloseArgs,
    instruction,
    open::OpenArgs,
    reference::PublishReferenceArgs,
    route::{self, RouteArgs},
    state::{Exposure, Loan, Market, PoolGeneration},
};
use shortwin_simple_runtime_tests::{
    assert_ok, deterministic_key, env as pda,
    position_case::{exposure, instruction as ix, loan},
    tokens::{self, TokenProgram},
    Env,
};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::{AddressLookupTableAccount, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    str::FromStr,
};

type Result<T> = std::result::Result<T, String>;
fn key(v: &Value) -> Result<Pubkey> {
    Pubkey::from_str(v.as_str().ok_or("pubkey must be string")?).map_err(|e| e.to_string())
}
fn n(v: &Value) -> Result<u64> {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .ok_or("integer missing".into())
}
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn checked(root: &Path, file: &Value, expected: &Value) -> Result<Vec<u8>> {
    let relative = Path::new(file.as_str().ok_or("file missing")?);
    if relative.is_absolute()
        || relative
            .components()
            .any(|p| p == std::path::Component::ParentDir)
    {
        return Err("evidence paths must remain within the manifest directory".into());
    }
    let bytes = std::fs::read(root.join(relative)).map_err(|e| e.to_string())?;
    if Some(sha(&bytes).as_str()) != expected.as_str() {
        return Err(format!("hash mismatch: {}", relative.display()));
    }
    Ok(bytes)
}
fn read_json(root: &Path, receipt: &Value) -> Result<Value> {
    serde_json::from_slice(&checked(root, &receipt["file"], &receipt["sha256"])?)
        .map_err(|e| e.to_string())
}
fn ata(owner: Pubkey, mint: Pubkey, token: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token.as_ref(), mint.as_ref()],
        &anchor_lang::prelude::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
    )
    .0
}

struct Build {
    input: u64,
    minimum: u64,
    setup: u64,
    data: Vec<u8>,
    metas: Vec<AccountMeta>,
}
#[derive(Clone, Copy)]
struct Binding {
    mint: Pubkey,
    token: Pubkey,
    q: u64,
    authority: Pubkey,
    base: Pubkey,
    wsol: Pubkey,
}
fn build(v: &Value, binding: Binding, buy: bool) -> Result<Build> {
    let Binding {
        mint,
        token,
        q,
        authority,
        base,
        wsol,
    } = binding;
    let (input_mint, output_mint, input_account, output_account, input_program, output_program) =
        if buy {
            (route::WSOL_MINT, mint, wsol, base, route::TOKEN_ID, token)
        } else {
            (mint, route::WSOL_MINT, base, wsol, token, route::TOKEN_ID)
        };
    if key(&v["inputMint"])? != input_mint
        || key(&v["outputMint"])? != output_mint
        || v["swapMode"] != "ExactIn"
    {
        return Err("build is not the requested ExactIn side".into());
    }
    let input = n(&v["inAmount"])?;
    let minimum = n(&v["otherAmountThreshold"])?;
    if input == 0
        || minimum == 0
        || n(&v["outAmount"])? < minimum
        || (!buy && input != q)
        || (buy && minimum < q)
    {
        return Err("build does not guarantee the requested whole-q output".into());
    }
    let swap = &v["swapInstruction"];
    if key(&swap["programId"])? != route::JUPITER_ID {
        return Err("unexpected swap program".into());
    }
    let data = STANDARD
        .decode(swap["data"].as_str().ok_or("swap data missing")?)
        .map_err(|e| e.to_string())?;
    if data.len() > route::MAX_ROUTE_BYTES {
        return Err("route data exceeds on-chain bound".into());
    }
    let source_ata = ata(authority, input_mint, input_program);
    let destination_ata = ata(authority, output_mint, output_program);
    let mut metas = vec![];
    for a in swap["accounts"].as_array().ok_or("swap metas missing")? {
        let original = key(&a["pubkey"])?;
        if a["isSigner"].as_bool().ok_or("signer flag missing")? && original != authority {
            return Err("unexpected route signer".into());
        }
        let address = if original == source_ata {
            input_account
        } else if original == destination_ata {
            output_account
        } else {
            original
        };
        let writable = a["isWritable"].as_bool().ok_or("writable flag missing")?;
        metas.push(if writable {
            AccountMeta::new(address, false)
        } else {
            AccountMeta::new_readonly(address, false)
        });
    }
    if metas.len() > route::MAX_ROUTE_ACCOUNTS
        || ![authority, input_account, output_account]
            .iter()
            .all(|k| metas.iter().any(|m| &m.pubkey == k))
    {
        return Err("missing endpoint or oversized route".into());
    }
    let setup = if metas.iter().any(|a| a.pubkey == authority && a.is_writable) {
        5_000_000
    } else {
        0
    };
    Ok(Build {
        input,
        minimum,
        setup,
        data,
        metas,
    })
}
fn send_tables(
    env: &mut Env,
    instruction: Instruction,
    signer: &Keypair,
    tables: &[AddressLookupTableAccount],
) -> Result<(shortwin_simple_runtime_tests::TxResult, usize)> {
    let limit = Instruction {
        program_id: anchor_lang::prelude::pubkey!("ComputeBudget111111111111111111111111111111"),
        accounts: vec![],
        data: [vec![2], 1_400_000u32.to_le_bytes().to_vec()].concat(),
    };
    let message = VersionedMessage::V0(
        solana_message::v0::Message::try_compile(
            &env.payer.pubkey(),
            &[limit, instruction],
            tables,
            env.svm.latest_blockhash(),
        )
        .map_err(|e| e.to_string())?,
    );
    let size =
        1 + 64 * usize::from(message.header().num_required_signatures) + message.serialize().len();
    if size > 1232 {
        return Err(format!(
            "actual lookup tables produce {size} bytes, exceeding 1232"
        ));
    }
    let tx =
        VersionedTransaction::try_new(message, &[&env.payer, signer]).map_err(|e| e.to_string())?;
    let result = env.svm.send_transaction(tx);
    env.svm.expire_blockhash();
    Ok((result, size))
}
fn submit(
    env: &mut Env,
    instruction: Instruction,
    signer: &Keypair,
    tables: &[AddressLookupTableAccount],
    stage: &str,
    report: &mut Value,
    out: &Path,
) -> Result<()> {
    // Try only actual mainnet table contents first. If packet fit fails, a
    // separately labelled synthetic ALT may still isolate CPI compatibility.
    let (result, size, mode) = match send_tables(env, instruction.clone(), signer, tables) {
        Ok((r, size)) => (r, size, "actual_snapshot_lookup_tables"),
        Err(reason) => {
            report["packet_fit_failures"][stage] = json!(reason);
            let (r, size) = env.send_v0(&[instruction], &[signer]);
            (r, size, "synthetic_test_alt_only")
        }
    };
    let (metadata, error) = match result {
        Ok(m) => (m, None),
        Err(e) => (e.meta, Some(format!("{:?}", e.err))),
    };
    let log_file = format!("{stage}.log");
    let logs = metadata.logs.join("\n") + "\n";
    std::fs::write(out.join(&log_file), logs.as_bytes()).map_err(|e| e.to_string())?;
    report["transactions"][stage] = json!({"wire_bytes":size,"lookup_mode":mode,"compute_units":metadata.compute_units_consumed,"error":error,"log_file":log_file,"log_sha256":sha(logs.as_bytes())});
    match error {
        Some(e) => Err(format!("{stage}: {e}; see exact runtime log")),
        None => Ok(()),
    }
}
fn run(manifest: &Path, out: &Path, report: &mut Value) -> Result<()> {
    let bytes = std::fs::read(manifest).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if v["schema"] != 1 {
        return Err("unsupported candidate schema".into());
    }
    report["manifest_sha256"] = json!(sha(&bytes));
    report["name"] = v["name"].clone();
    let root = manifest.parent().ok_or("manifest directory missing")?;
    let own = std::fs::read(pda::program_so_path()).map_err(|e| e.to_string())?;
    if Some(sha(&own).as_str()) != v["own_program_sha256"].as_str() {
        return Err("own program hash mismatch".into());
    }
    report["own_program_sha256"] = json!(sha(&own));
    let mint = key(&v["base_mint"])?;
    let q = n(&v["quantity"])?;
    if q == 0 || q.checked_mul(10).is_none() {
        return Err("invalid quantity".into());
    }
    let mut env = Env::new();
    let mut raw = BTreeMap::new();
    for a in v["accounts"].as_array().ok_or("accounts missing")? {
        let address = key(&a["address"])?;
        let account = Account {
            data: checked(root, &a["file"], &a["sha256"])?,
            owner: key(&a["owner"])?,
            lamports: n(&a["lamports"])?,
            executable: a["executable"].as_bool().ok_or("executable missing")?,
            rent_epoch: n(&a["rent_epoch"])?,
        };
        if raw.insert(address, account).is_some() {
            return Err("duplicate account snapshot".into());
        }
    }
    let mint_account = raw.get(&mint).ok_or("base mint snapshot missing")?;
    let token_program = if mint_account.owner == route::TOKEN_ID {
        TokenProgram::Spl
    } else if mint_account.owner == route::TOKEN_2022_ID {
        TokenProgram::Token2022
    } else {
        return Err("unsupported mint owner".into());
    };
    if mint_account.data.len() < 82
        || mint_account.data[45] != 1
        || mint_account.data[46..50] != [0; 4]
    {
        return Err("current mint header is not initialized and no-freeze".into());
    }
    let pd = raw
        .get(&route::JUPITER_PROGRAM_DATA)
        .ok_or("Jupiter ProgramData missing")?;
    if pd.owner != pda::LOADER || pd.data.len() < 45 || pd.data[..4] != 3u32.to_le_bytes() {
        return Err("invalid Jupiter ProgramData".into());
    }
    let deploy_slot = u64::from_le_bytes(pd.data[4..12].try_into().unwrap());
    let end = pd
        .data
        .iter()
        .rposition(|b| *b != 0)
        .ok_or("empty Jupiter ELF")?
        + 1;
    let digest = Sha256::digest(&pd.data[45..end]);
    let pd_len = pd.data.len() as u32;
    report["jupiter"] = json!({"deploy_slot":deploy_slot,"programdata_len":pd_len,"programdata_sha256":sha(&pd.data),"trimmed_elf_sha256":sha(&pd.data[45..end])});
    let slot = n(&v["context_slot"])?;
    let timestamp = v["block_time"].as_i64().ok_or("block time missing")?;
    if slot <= deploy_slot {
        return Err("execution slot precedes reviewed deployment".into());
    }
    let mut clock = env.svm.get_sysvar::<Clock>();
    clock.slot = slot;
    clock.unix_timestamp = timestamp;
    env.svm.set_sysvar(&clock);
    // Dynamic runtime sysvars are generated by LiteSVM. Their raw observations
    // remain retained, but must not overwrite the current transaction context.
    let dynamic = [
        anchor_lang::prelude::pubkey!("SysvarC1ock11111111111111111111111111111111"),
        anchor_lang::prelude::pubkey!("Sysvar1nstructions1111111111111111111111111"),
    ];
    for executable in [false, true] {
        for (address, account) in &raw {
            if account.executable == executable && !dynamic.contains(address) {
                env.svm
                    .set_account(*address, account.clone())
                    .map_err(|e| format!("install {address}: {e}"))?;
            }
        }
    }
    let mut tables = vec![];
    for k in v["lookup_tables"]
        .as_array()
        .ok_or("lookup tables missing")?
    {
        let k = key(k)?;
        let a = raw.get(&k).ok_or("lookup account missing")?;
        if a.owner != anchor_lang::prelude::pubkey!("AddressLookupTab1e1111111111111111111111111")
            || a.data.len() < 56
            || (a.data.len() - 56) % 32 != 0
            || a.data[..4] != 1u32.to_le_bytes()
        {
            return Err("invalid actual lookup state".into());
        }
        tables.push(AddressLookupTableAccount {
            key: k,
            addresses: a.data[56..]
                .as_chunks::<32>()
                .0
                .iter()
                .map(|b| Pubkey::new_from_array(*b))
                .collect(),
        });
    }
    let mut args = env.initialize_args();
    args.terms.max_spread_bps = 500; // Match the proposed production spread bound; other terms/history stay functional fixtures.
    args.jupiter_deploy_slot = deploy_slot;
    args.jupiter_program_data_len = pd_len;
    args.jupiter_elf_hash = digest.into();
    assert_ok(env.initialize(args));
    let borrower = deterministic_key(100);
    let lender = deterministic_key(101);
    env.fund(borrower.pubkey(), 100_000_000_000);
    env.fund(lender.pubkey(), 1_000_000_000);
    assert_ok(env.register_market(&lender, mint, token_program.id(), 10_000, 100_000_000));
    let market = pda::market(&mint);
    let pool = pda::pool(&market, 0);
    let loan = loan(pool, borrower.pubkey(), 0);
    let scratch =
        |seed: &[u8]| Pubkey::find_program_address(&[seed, loan.as_ref()], &shortwin_simple::ID).0;
    let authority = scratch(route::AUTHORITY_SEED);
    let base = scratch(route::BASE_SEED);
    let wsol = scratch(route::WSOL_SEED);
    let binding = Binding {
        mint,
        token: token_program.id(),
        q,
        authority,
        base,
        wsol,
    };
    let sell = build(&read_json(root, &v["sell"])?, binding, false)?;
    let mut buy = build(&read_json(root, &v["buy"])?, binding, true)?;
    // Prove the proposed keeper default after borrower-funded opening setup.
    // No native setup subsidy is silently assumed on the full buyback path.
    if buy.metas.iter().any(|a| {
        [
            anchor_lang::prelude::pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"),
            anchor_lang::prelude::pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"),
        ]
        .contains(&a.pubkey)
    }) {
        buy.setup = 0;
        report["closing_setup_policy"] =
            json!("zero allowance after actual borrower-funded opening setup");
    }
    report["synthetic_reference"] = json!({
        "live_bid_lower":sell.minimum,"live_ask_upper":buy.input,
        "acquisition_reference":buy.input,"completed_epochs":2,
        "spread_bps_ceiling":u128::from(buy.input.saturating_sub(sell.minimum)).saturating_mul(10_000).div_ceil(u128::from(sell.minimum)),
        "functional_policy_max_spread_bps":500
    });
    report["binding"] = json!({"mint":mint.to_string(),"token_program":token_program.id().to_string(),"q":q,"loan":loan.to_string(),"execution_authority":authority.to_string(),"execution_base":base.to_string(),"execution_wsol":wsol.to_string()});
    let source = tokens::create_account(
        &mut env,
        &deterministic_key(102),
        &mint,
        &lender.pubkey(),
        token_program,
    );
    let mut a = env.svm.get_account(&source).unwrap();
    a.data[64..72].copy_from_slice(&(10 * q).to_le_bytes());
    env.svm.set_account(source, a).unwrap();
    assert_ok(env.send(
        &[ix(
            accounts::DepositTokens {
                owner: lender.pubkey(),
                market,
                pool,
                receipt: pda::lender(&pool, &lender.pubkey()),
                mint,
                owner_tokens: source,
                token_vault: pda::token_vault(&pool),
                token_program: token_program.id(),
                system_program: anchor_lang::system_program::ID,
                event_authority: pda::event_authority(),
                program: shortwin_simple::ID,
            },
            instruction::Deposit {
                amount: 10 * q,
                min_shares: u128::from(10 * q),
                max_receipt_rent: 100_000_000,
            },
        )],
        &[&lender],
    ));
    let reference = Pubkey::find_program_address(
        &[
            b"reference",
            market.as_ref(),
            pda::policy(1).as_ref(),
            &q.to_le_bytes(),
        ],
        &shortwin_simple::ID,
    )
    .0;
    let epoch = timestamp / 60 * 60;
    let reporter = env.reporter.insecure_clone();
    let reference_args = PublishReferenceArgs {
        quantity: q,
        sequence: 1,
        source_slot: slot,
        source_ts: timestamp,
        finite: true,
        live_bid_lower: sell.minimum,
        live_ask_upper: buy.input,
        history_ask_upper_max: buy.input,
        history_bid_lower_max: sell.minimum,
        acquisition_reference: buy.input,
        completed_epochs: 2,
        history_start_ts: epoch - 120,
        history_end_ts: epoch - 1,
        evidence_hash: Sha256::digest(&bytes).into(),
        max_reference_rent: 100_000_000,
    };
    assert_ok(env.send(
        &[ix(
            accounts::PublishReference {
                reporter: reporter.pubkey(),
                config: pda::config(),
                market,
                policy: pda::policy(1),
                reference,
                system_program: anchor_lang::system_program::ID,
                event_authority: pda::event_authority(),
                program: shortwin_simple::ID,
            },
            instruction::PublishReference {
                args: reference_args,
            },
        )],
        &[&reporter],
    ));
    let opening = |max_fee, setup_lamports, temporary_funding| {
        let args = OpenArgs {
            nonce: 0,
            q,
            min_proceeds: sell.minimum,
            max_margin: 10_000_000_000,
            max_term_fee: max_fee,
            temporary_funding,
            max_account_rent: 100_000_000,
            minimum_reference_sequence: 1,
            route: RouteArgs {
                input_amount: sell.input,
                setup_lamports,
                data: sell.data.clone(),
            },
        };
        let mut i = ix(
            accounts::OpenPosition {
                borrower: borrower.pubkey(),
                config: pda::config(),
                policy: pda::policy(1),
                market,
                pool,
                loan,
                exposure: exposure(pool, borrower.pubkey()),
                reference,
                mint,
                token_vault: pda::token_vault(&pool),
                token_program: token_program.id(),
                fee_recipient: env.fee_recipient.pubkey(),
                execution_authority: authority,
                execution_base: base,
                execution_wsol: wsol,
                wsol_mint: route::WSOL_MINT,
                wsol_token_program: route::TOKEN_ID,
                jupiter: route::JUPITER_ID,
                jupiter_program_data: route::JUPITER_PROGRAM_DATA,
                system_program: anchor_lang::system_program::ID,
                event_authority: pda::event_authority(),
                program: shortwin_simple::ID,
            },
            instruction::OpenPosition { args },
        );
        i.accounts.extend(sell.metas.clone());
        i
    };
    let open_ix = opening(u64::MAX, sell.setup, 100_000_000);
    let rollback_ix = opening(0, sell.setup, 100_000_000);
    let pump = [
        anchor_lang::prelude::pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"),
        anchor_lang::prelude::pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"),
    ];
    let budget_negatives = if sell.metas.iter().any(|a| pump.contains(&a.pubkey)) {
        vec![
            ("setup_zero_budget", opening(u64::MAX, 0, 100_000_000)),
            (
                "setup_insufficient_budget",
                opening(u64::MAX, 1, 100_000_000),
            ),
            (
                "setup_over_program_maximum",
                opening(u64::MAX, 5_000_001, 100_000_000),
            ),
            (
                "setup_signed_funding_below_advance",
                opening(u64::MAX, sell.setup, 0),
            ),
        ]
    } else {
        vec![]
    };
    // Include absent route PDAs too: an inner program may create volume/rent
    // accounts, and those exact outer keys must be included in conservation.
    let mut external: Vec<_> = raw.keys().copied().collect();
    external.extend(sell.metas.iter().chain(&buy.metas).map(|a| a.pubkey));
    let protocol = [
        market,
        pool,
        pda::config(),
        pda::policy(1),
        pda::token_vault(&pool),
        borrower.pubkey(),
        lender.pubkey(),
        env.payer.pubkey(),
        env.fee_recipient.pubkey(),
        loan,
        exposure(pool, borrower.pubkey()),
        reference,
        authority,
        base,
        wsol,
        source,
    ];
    external.retain(|k| !protocol.contains(k) && !dynamic.contains(k));
    external.sort();
    external.dedup();
    let external_liquidity =
        |env: &Env| -> u128 { external.iter().map(|k| u128::from(env.lamports(k))).sum() };
    let external_liquidity_before = external_liquidity(&env);
    let mut protected: Vec<_> = raw.keys().copied().collect();
    protected.extend(sell.metas.iter().chain(&buy.metas).map(|a| a.pubkey));
    protected.retain(|k| !dynamic.contains(k));
    protected.extend([
        market,
        pool,
        pda::token_vault(&pool),
        borrower.pubkey(),
        lender.pubkey(),
        env.fee_recipient.pubkey(),
        loan,
        exposure(pool, borrower.pubkey()),
        reference,
        authority,
        base,
        wsol,
        source,
    ]);
    protected.sort();
    protected.dedup();
    let token_total = |env: &Env, mint: Pubkey| -> u128 {
        protected
            .iter()
            .filter_map(|k| env.svm.get_account(k))
            .filter(|a| {
                [route::TOKEN_ID, route::TOKEN_2022_ID].contains(&a.owner)
                    && a.data.len() >= 165
                    && a.data[..32] == mint.to_bytes()
            })
            .map(|a| u128::from(u64::from_le_bytes(a.data[64..72].try_into().unwrap())))
            .sum()
    };
    let base_total_before = token_total(&env, mint);
    let wsol_total_before = token_total(&env, route::WSOL_MINT);
    let before: Vec<_> = protected.iter().map(|k| env.svm.get_account(k)).collect();
    for (stage, instruction) in budget_negatives {
        let failure = submit(
            &mut env,
            instruction,
            &borrower,
            &tables,
            stage,
            report,
            out,
        );
        if failure.is_ok() {
            return Err(format!("{stage} unexpectedly accepted"));
        }
        if protected
            .iter()
            .map(|k| env.svm.get_account(k))
            .collect::<Vec<_>>()
            != before
        {
            return Err(format!(
                "{stage} failed to roll back every protected DEX/protocol account"
            ));
        }
        if std::fs::read_to_string(out.join(format!("{stage}.log")))
            .map_err(|e| e.to_string())?
            .contains("FeeAboveMax")
        {
            return Err(format!("{stage} failed only at unrelated max-fee bound"));
        }
        report["setup_budget_rejections"][stage] = json!(true);
    }
    let rejection = submit(
        &mut env,
        rollback_ix,
        &borrower,
        &tables,
        "post_swap_fee_bound_rollback",
        report,
        out,
    );
    if rejection.is_ok() {
        return Err("zero max fee unexpectedly accepted".into());
    }
    if protected
        .iter()
        .map(|k| env.svm.get_account(k))
        .collect::<Vec<_>>()
        != before
    {
        return Err("rejected transaction did not restore protected protocol/DEX state".into());
    }
    let rollback_log = std::fs::read_to_string(out.join("post_swap_fee_bound_rollback.log"))
        .map_err(|e| e.to_string())?;
    if !rollback_log
        .lines()
        .any(|l| l == format!("Program {} success", route::JUPITER_ID))
        || !rollback_log.contains("FeeAboveMax")
    {
        return Err(format!(
            "rollback did not reach successful real Jupiter CPI and signed-bound refusal: {}",
            rejection.unwrap_err()
        ));
    }
    report["post_cpi_rollback"] = json!(true);
    let treasury = env.fee_recipient.pubkey();
    let borrower_before = env.lamports(&borrower.pubkey());
    let treasury_before = env.lamports(&treasury);
    let pool_before = env.lamports(&pool);
    submit(
        &mut env,
        open_ix,
        &borrower,
        &tables,
        "open_sell",
        report,
        out,
    )?;
    let l: Loan = env.read(&loan);
    let p: PoolGeneration = env.read(&pool);
    let e: Exposure = env.read(&exposure(pool, borrower.pubkey()));
    let m: Market = env.read(&market);
    if l.q != q
        || p.cash != 9 * q
        || p.principal_out != q
        || p.loan_count != 1
        || env.token_amount(&pda::token_vault(&pool)) != 9 * q
        || e.q != q
        || e.notional != l.opening_notional
        || m.next_loan_nonce != 1
    {
        return Err("opening principal/exposure/vault invariant failed".into());
    }
    let loan_rent = env.svm.minimum_balance_for_rent_exemption(Loan::SPACE);
    let exposure_rent = env.svm.minimum_balance_for_rent_exemption(Exposure::SPACE);
    let open_fixed = loan_rent + exposure_rent + l.margin + l.term_fee;
    let open_debit = borrower_before
        .checked_sub(env.lamports(&borrower.pubkey()))
        .ok_or("opening unexpectedly credited borrower")?;
    let setup_open = open_debit
        .checked_sub(open_fixed)
        .ok_or("opening debit does not cover fixed financial charges")?;
    if setup_open > sell.setup
        || env.lamports(&loan) != loan_rent + l.collateral
        || env.lamports(&pool) != pool_before + l.lender_fee
        || env.lamports(&treasury) != treasury_before + l.protocol_fee
    {
        return Err("opening collateral/fee/rent invariant failed".into());
    }
    for k in [base, wsol] {
        if env.svm.get_account(&k).is_some() {
            return Err("scratch account survived sale".into());
        }
    }
    let close_ix = {
        let mut i = ix(
            accounts::ClosePosition {
                actor: borrower.pubkey(),
                position: accounts::PositionAccounts {
                    config: pda::config(),
                    policy: pda::policy(1),
                    market,
                    pool,
                    loan,
                    exposure: exposure(pool, borrower.pubkey()),
                    borrower: borrower.pubkey(),
                },
                token: accounts::LoanTokenAccounts {
                    mint,
                    token_vault: pda::token_vault(&pool),
                    token_program: token_program.id(),
                },
                execution: accounts::RouteExecution {
                    execution_authority: authority,
                    execution_base: base,
                    execution_wsol: wsol,
                    wsol_mint: route::WSOL_MINT,
                    wsol_token_program: route::TOKEN_ID,
                    jupiter: route::JUPITER_ID,
                    jupiter_program_data: route::JUPITER_PROGRAM_DATA,
                    system_program: anchor_lang::system_program::ID,
                },
                event_authority: pda::event_authority(),
                program: shortwin_simple::ID,
            },
            instruction::ClosePosition {
                args: CloseArgs {
                    max_buyback_cost: buy.input,
                    temporary_funding: 100_000_000,
                    route: RouteArgs {
                        input_amount: buy.input,
                        setup_lamports: buy.setup,
                        data: buy.data,
                    },
                },
            },
        );
        i.accounts.extend(buy.metas);
        i
    };
    let borrower_before = env.lamports(&borrower.pubkey());
    submit(
        &mut env,
        close_ix,
        &borrower,
        &tables,
        "full_buyback",
        report,
        out,
    )?;
    let p: PoolGeneration = env.read(&pool);
    if p.cash < 10 * q
        || p.principal_out != 0
        || p.loan_count != 0
        || p.booked_notional_out != 0
        || env
            .svm
            .get_account(&exposure(pool, borrower.pubkey()))
            .is_some()
        || env.token_amount(&pda::token_vault(&pool)) != p.cash
        || env.svm.get_account(&loan).is_some()
    {
        return Err("full buyback net-q/principal/exposure invariant failed".into());
    }
    let close_fixed_refund = loan_rent + exposure_rent + l.collateral - buy.input;
    let close_refund = env
        .lamports(&borrower.pubkey())
        .checked_sub(borrower_before)
        .ok_or("closing actor refund unexpectedly negative")?;
    let setup_close = close_fixed_refund
        .checked_sub(close_refund)
        .ok_or("closing refund exceeds fixed financial refund")?;
    if setup_close > buy.setup
        || env.lamports(&pool) != pool_before + l.lender_fee
        || env.lamports(&treasury) != treasury_before + l.protocol_fee
    {
        return Err("close SOL/collateral/rent invariant failed".into());
    }
    for k in [base, wsol] {
        if env.svm.get_account(&k).is_some() {
            return Err("scratch account survived buyback".into());
        }
    }
    for k in [mint, route::WSOL_MINT] {
        if env.svm.get_account(&k) != raw.get(&k).cloned() {
            return Err("real mint metadata changed".into());
        }
    }
    for (k, before) in &raw {
        if [route::TOKEN_ID, route::TOKEN_2022_ID].contains(&before.owner)
            && before.data.len() >= 165
        {
            let after = env
                .svm
                .get_account(k)
                .ok_or("real DEX token account unexpectedly closed")?;
            if after.owner != before.owner
                || after.data.len() != before.data.len()
                || after.data[..64] != before.data[..64]
                || after.data[72..] != before.data[72..]
            {
                return Err(format!("real DEX token metadata changed: {k}"));
            }
        }
    }
    if token_total(&env, mint) != base_total_before {
        return Err("base token conservation across real DEX/protocol accounts failed".into());
    }
    let wsol_delta = token_total(&env, route::WSOL_MINT) as i128 - wsol_total_before as i128;
    // Token amount alone is insufficient for Pump native-SOL curves. Sum all
    // captured external account lamports, including WSOL vault lamports, native
    // curves, fee recipients and newly initialized volume/rent accounts. Never
    // double-count WSOL data amounts on top of their already-backed lamports.
    let liquidity_delta = external_liquidity(&env) as i128 - external_liquidity_before as i128;
    if liquidity_delta
        != i128::from(buy.input) - i128::from(l.sale_proceeds)
            + i128::from(setup_open)
            + i128::from(setup_close)
    {
        return Err("captured external native-SOL plus WSOL liquidity/rent flow differs from actual spend minus proceeds".into());
    }
    report["setup_allowance"] = json!({"opening_signed_maximum":sell.setup,"opening_actual_actor_cost":setup_open,"opening_unused_refunded":sell.setup-setup_open,"closing_signed_maximum":buy.setup,"closing_actual_actor_cost":setup_close,"closing_unused_refunded":buy.setup-setup_close,"cost_excluded_from_WSOL_notional_collateral_and_protocol_fees":true});
    if env.svm.get_account(&authority).is_some_and(|a| {
        a.owner != anchor_lang::system_program::ID
            || !a.data.is_empty()
            || a.lamports != 0
            || a.executable
    }) {
        return Err("execution authority retains native funds/data or foreign ownership".into());
    }
    report["external_native_and_wrapped_lamports_delta"] = json!(liquidity_delta.to_string());
    report["external_liquidity_account_count"] = json!(external.len());
    report["liquidity_conservation_scope"] = json!("All captured raw and both-route outer account keys, including initially absent PDAs; native curve/fee/rent plus WSOL vault lamports, excluding protocol financial actors/accounts and scratch; WSOL data delta retained separately without double counting.");
    report["base_token_conservation"] = json!(true);
    report["wsol_delta_lamports"] = json!(wsol_delta.to_string());
    report["actual_snapshot_packets_fit"] = json!(report["packet_fit_failures"]
        .as_object()
        .unwrap()
        .is_empty());
    report["functional_execution_passed"] = json!(true);
    report["full_buyback_net_credit"] = json!(p.cash - 9 * q);
    report["sell_proceeds"] = json!(l.sale_proceeds);
    report["buyback_spend"] = json!(buy.input);
    report["invariants"] = json!("principal/exposure, whole-q net vault credit, actual paid fees, collateral, rent/scratch closure, immutable real mint bytes, full post-CPI rollback");
    Ok(())
}
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: qualify_current_route CANDIDATE.json NEW_OUTPUT_DIRECTORY");
        std::process::exit(2);
    }
    let manifest = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);
    if let Err(e) = std::fs::create_dir(&out) {
        eprintln!("refuse existing/unavailable output: {e}");
        std::process::exit(2);
    }
    let mut report = json!({"schema":1,"kind":"actual_SBF_candidate_snapshot_replay","status":"running","live_transactions_sent":0,"synthetic_inputs":["local signers and funded SOL","initial lender token allocation from existing supply","two-epoch reference history attestation","financial policy for functional testing","synthetic ALT fallback only when explicitly reported"],"limitations":["read-only snapshots can span slots; no atomic mainnet landing is claimed","buy response predates simulated sell and may fail after real DEX mutation","route-family/token scope is exactly this candidate; no universal Jupiter qualification"],"packet_fit_failures":{},"transactions":{},"setup_budget_rejections":{}});
    let runner =
        std::fs::read(std::env::current_exe().expect("runner path")).expect("runner bytes");
    report["native_runner_sha256"] = json!(sha(&runner));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(&manifest, &out, &mut report)
    }))
    .unwrap_or_else(|p| {
        let message = p
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| p.downcast_ref::<&str>().copied())
            .unwrap_or("unknown setup panic");
        Err(format!("candidate setup/replay panic: {message}"))
    });
    report["status"] = json!(if result.is_ok() { "passed" } else { "failed" });
    report["error"] = json!(result.as_ref().err());
    if let Err(e) = std::fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    ) {
        eprintln!("cannot retain summary: {e}");
        std::process::exit(1);
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    if result.is_err() {
        std::process::exit(1);
    }
}
