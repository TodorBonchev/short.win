//! Actual SBF report persistence, intentionally without market admission or financial wiring.
use crate::common::*;
use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{InstructionData, ToAccountMetas};
use shortwin::constants::{TOKEN_PROGRAM_ID, WSOL_MINT};
use shortwin::state::{
    InitializePriceReportArgs, PriceReport, PriceReportDomainArgs, PriceReportModelArgs,
    PriceReportPolicyArgs, PriceReportStatusArgs, PublishPriceReportArgs,
};
use solana_signer::Signer;

fn args(publisher: Pubkey) -> InitializePriceReportArgs {
    let mut publishers = [Pubkey::default(); 8];
    publishers[0] = publisher;
    InitializePriceReportArgs {
        genesis_hash: [7; 32],
        quantity: 1_000,
        model: PriceReportModelArgs {
            version: 1,
            epoch: 1,
            policy_hash: [8; 32],
        },
        policy: PriceReportPolicyArgs {
            max_age_slots: 100,
            max_age_seconds: 2,
            max_window_slots: 100,
            max_window_seconds: 1,
            max_update_gap_slots: 100,
            max_update_gap_seconds: 2,
            epoch_seconds: 1,
            minimum_completed_epochs: 1,
        },
        publisher_count: 1,
        publishers,
    }
}
fn report_key(mm: &MockMarket, args: &InitializePriceReportArgs) -> Pubkey {
    Pubkey::find_program_address(
        &[
            PriceReport::SEED,
            mm.market().as_ref(),
            &args.quantity.to_le_bytes(),
            &args.model.epoch.to_le_bytes(),
        ],
        &shortwin::ID,
    )
    .0
}
fn init(mm: &MockMarket, authority: Pubkey, args: InitializePriceReportArgs) -> Instruction {
    let data = shortwin::instruction::InitializePriceReport { args }.data();
    assert_eq!(
        data.len(),
        406,
        "398-byte initialization arguments plus discriminator"
    );
    Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::InitializePriceReport {
            authority,
            config: pda::config(),
            market: mm.market(),
            price_report: report_key(mm, &args),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data,
    }
}
fn publication(
    env: &Env,
    mm: &MockMarket,
    init: &InitializePriceReportArgs,
    sequence: u64,
) -> PublishPriceReportArgs {
    let c = env.clock();
    let m: shortwin::state::Market = read::zc(&env.svm, &mm.market()).unwrap();
    PublishPriceReportArgs {
        version: 1,
        domain: PriceReportDomainArgs {
            genesis_hash: init.genesis_hash,
            program: shortwin::ID,
            config: pda::config(),
            market: mm.market(),
            base_mint: m.mint,
            base_token_program: m.token_program,
            quote_mint: WSOL_MINT,
            quote_token_program: TOKEN_PROGRAM_ID,
            base_decimals: m.decimals,
            quote_decimals: 9,
        },
        quantity: init.quantity,
        model: init.model,
        configuration_epoch: 1,
        publisher: init.publishers[0],
        sequence,
        start_slot: c.slot,
        start_ts: c.unix_timestamp,
        end_slot: c.slot,
        end_ts: c.unix_timestamp,
        observation_digest: [9; 32],
        status: PriceReportStatusArgs::Finite {
            lower: 10,
            central: 11,
            upper: 12,
        },
    }
}
fn publish(
    mm: &MockMarket,
    report: Pubkey,
    publisher: Pubkey,
    args: PublishPriceReportArgs,
) -> Instruction {
    let data = shortwin::instruction::PublishPriceReport { args }.data();
    let expected = match args.status {
        PriceReportStatusArgs::Unknown => 433,
        PriceReportStatusArgs::Finite { .. } => 457,
    };
    assert_eq!(
        data.len(),
        expected,
        "fixed publication arguments plus discriminator"
    );
    Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::PublishPriceReport {
            publisher,
            config: pda::config(),
            market: mm.market(),
            price_report: report,
        }
        .to_account_metas(None),
        data,
    }
}
fn restored(env: &Env, mm: &MockMarket, report: Pubkey) -> PriceReport {
    let account: PriceReport = read::borsh(&env.svm, &report).unwrap();
    let market: shortwin::state::Market = read::zc(&env.svm, &mm.market()).unwrap();
    account
        .restore(&pda::config(), &mm.market(), &market)
        .unwrap();
    account
}
#[test]
fn only_governance_initializes_unused_immutable_report_pda() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let foreign = env.borrower();
    let a = args(gov.pubkey());
    let key = report_key(&mm, &a);
    assert!(
        env.send(&[init(&mm, foreign.key(), a)], &[&foreign.kp])
            .is_err()
    );
    assert!(env.svm.get_account(&key).is_none());
    let mut bad = a;
    bad.publishers[7] = Pubkey::new_unique();
    assert!(env.send(&[init(&mm, gov.pubkey(), bad)], &[&gov]).is_err());
    let meta = ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    assert!(meta.compute_units_consumed < 200_000);
    assert_eq!(
        env.svm.get_account(&key).unwrap().data.len(),
        PriceReport::SPACE
    );
    assert_eq!(restored(&env, &mm, key).quantity, 1_000);
    let before = env.svm.get_account(&key);
    assert!(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]).is_err());
    assert_eq!(env.svm.get_account(&key), before);
}
#[test]
fn publisher_authentication_full_binding_sequences_and_windows_are_atomic() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let foreign = env.borrower();
    let a = args(gov.pubkey());
    let key = report_key(&mm, &a);
    ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    let good = publication(&env, &mm, &a, 1);
    let before = env.svm.get_account(&key);
    let market_before = env.svm.get_account(&mm.market());
    assert!(
        env.send(&[publish(&mm, key, foreign.key(), good)], &[&foreign.kp])
            .is_err()
    );
    let mut no_signer = publish(&mm, key, foreign.key(), good);
    no_signer.accounts[0].is_signer = false;
    assert!(env.send(&[no_signer], &[]).is_err());
    let mutations: &[fn(&mut PublishPriceReportArgs)] = &[
        |a| a.domain.genesis_hash = [99; 32],
        |a| a.domain.market = Pubkey::new_unique(),
        |a| a.domain.base_mint = Pubkey::new_unique(),
        |a| a.quantity += 1,
        |a| a.model.policy_hash = [99; 32],
        |a| a.configuration_epoch += 1,
        |a| a.sequence += 1,
        |a| a.end_slot += 1,
        |a| {
            a.start_ts -= 3;
            a.end_ts -= 3;
        },
    ];
    for mutate in mutations {
        let mut bad = good;
        mutate(&mut bad);
        assert!(
            env.send(&[publish(&mm, key, gov.pubkey(), bad)], &[&gov])
                .is_err()
        );
        assert_eq!(env.svm.get_account(&key), before);
        assert_eq!(env.svm.get_account(&mm.market()), market_before);
    }
    let meta = ok(env.send(&[publish(&mm, key, gov.pubkey(), good)], &[&gov]));
    assert!(meta.compute_units_consumed < 200_000);
    let accepted = env.svm.get_account(&key);
    assert!(
        env.send(&[publish(&mm, key, gov.pubkey(), good)], &[&gov])
            .is_err()
    );
    assert_eq!(env.svm.get_account(&key), accepted);
    assert_eq!(env.svm.get_account(&mm.market()), market_before);
}
#[test]
fn persisted_completed_history_and_unknown_reset_have_no_financial_side_effect() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let mut a = args(gov.pubkey());
    a.publisher_count = 8;
    a.policy.minimum_completed_epochs = 8;
    for key in &mut a.publishers[1..] {
        *key = Pubkey::new_unique();
    }
    let key = report_key(&mm, &a);
    ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    let market_before = env.svm.get_account(&mm.market());
    let mut max_cu = 0;
    for sequence in 1..=11 {
        let mut p = publication(&env, &mm, &a, sequence);
        if sequence == 11 {
            p.status = PriceReportStatusArgs::Unknown;
        }
        let meta = ok(env.send(&[publish(&mm, key, gov.pubkey(), p)], &[&gov]));
        max_cu = max_cu.max(meta.compute_units_consumed);
        assert!(meta.compute_units_consumed < 200_000);
        let account = restored(&env, &mm, key);
        let m = read::zc(&env.svm, &mm.market()).unwrap();
        let state = account.restore(&pda::config(), &mm.market(), &m).unwrap();
        assert_eq!(state.last_sequence(), sequence);
        if sequence == 10 {
            assert_eq!(state.completed_history_len(), 8);
        }
        if sequence == 11 {
            assert_eq!(state.completed_history_len(), 0);
            assert!(
                state
                    .live_marks_at(state.latest_metadata().unwrap().window.end)
                    .is_err()
            );
        }
        env.warp(1);
    }
    eprintln!("report publish: maximum exercised CU = {max_cu}");
    assert_eq!(env.svm.get_account(&mm.market()), market_before);
}
#[test]
fn corrupt_noncanonical_or_appended_account_bytes_are_refused_without_writes() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let a = args(gov.pubkey());
    let key = report_key(&mm, &a);
    ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    let original = env.svm.get_account(&key).unwrap();
    let good = publication(&env, &mm, &a, 1);
    for append in [false, true] {
        let mut bad = original.clone();
        if append {
            bad.data.push(0);
        } else {
            bad.data[59] ^= 0x80;
        }
        env.svm.set_account(key, bad.clone()).unwrap();
        assert_err(
            &env.send(&[publish(&mm, key, gov.pubkey(), good)], &[&gov]),
            shortwin::errors::ShortwinError::AccountMismatch,
        );
        assert_eq!(env.svm.get_account(&key), Some(bad));
    }
    env.svm.set_account(key, original).unwrap();
    ok(env.send(&[publish(&mm, key, gov.pubkey(), good)], &[&gov]));
}

#[test]
fn actual_account_owner_pda_header_and_version_are_enforced() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let a = args(gov.pubkey());
    let key = report_key(&mm, &a);
    ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    let original = env.svm.get_account(&key).unwrap();
    let good = publication(&env, &mm, &a, 1);
    for field in 0..3 {
        let mut bad = original.clone();
        match field {
            0 => bad.owner = Pubkey::new_unique(),
            1 => bad.data[43] ^= 1, // quantity in the outer header participates in the PDA
            _ => bad.data[8] ^= 1,  // unsupported outer account version
        }
        env.svm.set_account(key, bad.clone()).unwrap();
        assert!(
            env.send(&[publish(&mm, key, gov.pubkey(), good)], &[&gov])
                .is_err()
        );
        assert_eq!(env.svm.get_account(&key), Some(bad));
    }
    env.svm.set_account(key, original).unwrap();
    ok(env.send(&[publish(&mm, key, gov.pubkey(), good)], &[&gov]));
}
