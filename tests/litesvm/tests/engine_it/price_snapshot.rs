//! Actual SBF on-demand snapshots; historical prices are explicitly trusted private assertions.
use crate::common::*;
use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{InstructionData, ToAccountMetas};
use shortwin::state::{
    AttestedPriceBucketArgs, InitializePriceSnapshotArgs, PriceReportModelArgs,
    PriceReportPolicyArgs, PriceReportStatusArgs, PriceSnapshot, PublishPriceSnapshotArgs,
};
use solana_signer::Signer;

fn args(publisher: Pubkey) -> InitializePriceSnapshotArgs {
    let mut publishers = [Pubkey::default(); 8];
    publishers[0] = publisher;
    InitializePriceSnapshotArgs {
        genesis_hash: [7; 32],
        producer_commitment: [6; 32],
        quantity: 1_000,
        model: PriceReportModelArgs {
            version: 1,
            epoch: 1,
            policy_hash: [8; 32],
        },
        policy: PriceReportPolicyArgs {
            max_age_slots: 100,
            max_age_seconds: 4,
            max_window_slots: 100,
            max_window_seconds: 1,
            max_update_gap_slots: 10,
            max_update_gap_seconds: 3,
            epoch_seconds: 10,
            minimum_completed_epochs: 8,
        },
        publisher_count: 1,
        publishers,
    }
}
fn key(mm: &MockMarket, a: &InitializePriceSnapshotArgs) -> Pubkey {
    Pubkey::find_program_address(
        &[
            PriceSnapshot::SEED,
            mm.market().as_ref(),
            &a.quantity.to_le_bytes(),
            &a.model.epoch.to_le_bytes(),
        ],
        &shortwin::ID,
    )
    .0
}
fn init(mm: &MockMarket, authority: Pubkey, a: InitializePriceSnapshotArgs) -> Instruction {
    Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::InitializePriceSnapshot {
            authority,
            config: pda::config(),
            market: mm.market(),
            price_snapshot: key(mm, &a),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: shortwin::instruction::InitializePriceSnapshot { args: a }.data(),
    }
}
fn publication(env: &Env, sequence: u64) -> PublishPriceSnapshotArgs {
    let c = env.clock();
    let epoch = c.unix_timestamp as u64 / 10;
    let mut history = [AttestedPriceBucketArgs::default(); 8];
    for (i, b) in history.iter_mut().enumerate() {
        let e = epoch - 8 + i as u64;
        let first_ts = (e * 10) as i64;
        let last_ts = first_ts + 9;
        *b = AttestedPriceBucketArgs {
            epoch: e,
            first_slot: c.slot - ((c.unix_timestamp - first_ts) as u64 * 2),
            first_ts,
            last_slot: c.slot - ((c.unix_timestamp - last_ts) as u64 * 2),
            last_ts,
            lower_min: 10,
            central_last: 11,
            upper_max: 12,
            observations: 10,
            largest_gap_slots: 2,
            largest_gap_seconds: 1,
        };
    }
    PublishPriceSnapshotArgs {
        version: 2,
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
        history_count: 8,
        history,
    }
}
fn publish(
    mm: &MockMarket,
    key: Pubkey,
    publisher: Pubkey,
    args: PublishPriceSnapshotArgs,
) -> Instruction {
    let data = shortwin::instruction::PublishPriceSnapshot { args }.data();
    assert_eq!(
        data.len(),
        match args.status {
            PriceReportStatusArgs::Finite { .. } => 812,
            PriceReportStatusArgs::Unknown => 788,
        }
    );
    Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::PublishPriceSnapshot {
            publisher,
            config: pda::config(),
            market: mm.market(),
            price_snapshot: key,
        }
        .to_account_metas(None),
        data,
    }
}
fn read_snapshot(env: &Env, mm: &MockMarket, key: Pubkey) -> PriceSnapshot {
    let a: PriceSnapshot = read::borsh(&env.svm, &key).unwrap();
    let m = read::zc(&env.svm, &mm.market()).unwrap();
    a.restore(&pda::config(), &mm.market(), &m).unwrap();
    a
}

#[test]
fn governance_alone_initializes_versioned_immutable_snapshot() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let outsider = env.funded_keypair();
    let a = args(gov.pubkey());
    let key = key(&mm, &a);
    assert!(
        env.send(&[init(&mm, outsider.pubkey(), a)], &[&outsider])
            .is_err()
    );
    assert!(env.svm.get_account(&key).is_none());
    let mut bad = a;
    bad.producer_commitment = [0; 32];
    assert!(env.send(&[init(&mm, gov.pubkey(), bad)], &[&gov]).is_err());
    let mut bad = a;
    bad.publishers[7] = Pubkey::new_unique();
    assert!(env.send(&[init(&mm, gov.pubkey(), bad)], &[&gov]).is_err());
    let meta = ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    assert!(meta.compute_units_consumed < 200_000);
    assert_eq!(env.svm.get_account(&key).unwrap().data.len(), 1624);
    let before = env.svm.get_account(&key);
    assert!(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]).is_err());
    assert_eq!(env.svm.get_account(&key), before);
}
#[test]
fn one_full_h8_after_one_hundred_idle_epochs_fits_packet_and_uses_no_heartbeat() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let reporter = env.funded_keypair();
    assert_ne!(reporter.pubkey(), env.payer.pubkey());
    let a = args(reporter.pubkey());
    let key = key(&mm, &a);
    ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    let initial = env.svm.get_account(&key);
    let market_before = env.svm.get_account(&mm.market());
    env.warp(1_000);
    assert_eq!(env.svm.get_account(&key), initial);
    let ix = publish(&mm, key, reporter.pubkey(), publication(&env, 1));
    let tx = env.transaction(std::slice::from_ref(&ix), &[&reporter]);
    assert_eq!(tx.signatures.len(), 2);
    let packet = 1 + 64 * tx.signatures.len() + tx.message_data().len();
    assert!(packet <= 1232, "H8 packet: {packet}");
    let compute_program = "ComputeBudget111111111111111111111111111111"
        .parse()
        .unwrap();
    let mut limit = vec![2];
    limit.extend_from_slice(&200_000u32.to_le_bytes());
    let mut price = vec![3];
    price.extend_from_slice(&1u64.to_le_bytes());
    let priority_ixs = [
        Instruction {
            program_id: compute_program,
            accounts: vec![],
            data: limit,
        },
        Instruction {
            program_id: compute_program,
            accounts: vec![],
            data: price,
        },
        ix,
    ];
    let priority_tx = env.transaction(&priority_ixs, &[&reporter]);
    let priority_packet = 1 + 64 * priority_tx.signatures.len() + priority_tx.message_data().len();
    assert_eq!(packet, 1178);
    assert_eq!(priority_packet, 1230);
    assert!(priority_packet <= 1232);
    let meta = ok(env.send(&priority_ixs, &[&reporter]));
    eprintln!(
        "trusted H8 snapshot: {packet} bytes ordinary, {priority_packet} bytes with actual CU limit+price, {} CU",
        meta.compute_units_consumed
    );
    assert!(meta.compute_units_consumed < 200_000);
    let account = read_snapshot(&env, &mm, key);
    let m = read::zc(&env.svm, &mm.market()).unwrap();
    let state = account.restore(&pda::config(), &mm.market(), &m).unwrap();
    assert_eq!(state.completed_history_len(), 8);
    let now = state.latest_metadata().unwrap().window.end;
    assert!(state.evidence_at(now, 1).is_ok());
    assert!(state.evidence_at(now, 2).is_err());
    assert_eq!(state.outstanding_positions(), 0);
    assert_eq!(env.svm.get_account(&mm.market()), market_before);
}
#[test]
fn a_fresh_source_snapshot_can_land_and_be_read_across_an_epoch_boundary() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let a = args(gov.pubkey());
    let key = key(&mm, &a);
    ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    env.warp(1_009);
    assert_eq!(env.now() % 10, 9);
    let p = publication(&env, 1);
    env.warp(1);
    let meta = ok(env.send(&[publish(&mm, key, gov.pubkey(), p)], &[&gov]));
    assert!(meta.compute_units_consumed < 200_000);
    let account = read_snapshot(&env, &mm, key);
    let m = read::zc(&env.svm, &mm.market()).unwrap();
    let state = account.restore(&pda::config(), &mm.market(), &m).unwrap();
    let mut now = state.latest_metadata().unwrap().window.end;
    now.slot = env.clock().slot;
    now.unix_timestamp = env.now();
    assert!(state.evidence_at(now, 1).is_ok());
    now.unix_timestamp += 4;
    now.slot += 10;
    assert!(state.evidence_at(now, 1).is_err());
}
#[test]
fn signer_sequence_policy_window_and_private_history_refusals_are_atomic() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let outsider = env.funded_keypair();
    let a = args(gov.pubkey());
    let key = key(&mm, &a);
    ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    env.warp(1_000);
    let good = publication(&env, 1);
    let before = env.svm.get_account(&key);
    let market_before = env.svm.get_account(&mm.market());
    assert!(
        env.send(&[publish(&mm, key, outsider.pubkey(), good)], &[&outsider])
            .is_err()
    );
    let mut unsigned = publish(&mm, key, outsider.pubkey(), good);
    unsigned.accounts[0].is_signer = false;
    assert!(env.send(&[unsigned], &[]).is_err());
    let mut trailing = publish(&mm, key, gov.pubkey(), good);
    trailing.accounts.push(trailing.accounts[1].clone());
    assert!(env.send(&[trailing], &[&gov]).is_err());
    assert_eq!(env.svm.get_account(&key), before);
    let mutations: &[fn(&mut PublishPriceSnapshotArgs)] = &[
        |a| a.version = 1,
        |a| a.sequence += 1,
        |a| a.end_slot += 1,
        |a| {
            a.start_ts -= 5;
            a.end_ts -= 5;
        },
        |a| a.history[0].largest_gap_slots = 11,
        |a| a.history[0].epoch += 1,
        |a| a.history.swap(0, 1),
        |a| a.history_count = 7,
        |a| a.observation_digest = [0; 32],
        |a| {
            a.status = PriceReportStatusArgs::Finite {
                lower: 12,
                central: 11,
                upper: 10,
            }
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
    ok(env.send(&[publish(&mm, key, gov.pubkey(), good)], &[&gov]));
    let accepted = env.svm.get_account(&key);
    assert!(
        env.send(&[publish(&mm, key, gov.pubkey(), good)], &[&gov])
            .is_err()
    );
    assert_eq!(env.svm.get_account(&key), accepted);
}
#[test]
fn unknown_erases_history_and_finite_warming_does_not_reuse_old_buckets() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let a = args(gov.pubkey());
    let key = key(&mm, &a);
    ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    env.warp(1_000);
    ok(env.send(
        &[publish(&mm, key, gov.pubkey(), publication(&env, 1))],
        &[&gov],
    ));
    env.warp(1);
    let mut unknown = publication(&env, 2);
    unknown.status = PriceReportStatusArgs::Unknown;
    let prior = env.svm.get_account(&key);
    assert!(
        env.send(&[publish(&mm, key, gov.pubkey(), unknown)], &[&gov])
            .is_err()
    );
    assert_eq!(env.svm.get_account(&key), prior);
    unknown.history_count = 0;
    unknown.history = [AttestedPriceBucketArgs::default(); 8];
    ok(env.send(&[publish(&mm, key, gov.pubkey(), unknown)], &[&gov]));
    let account = read_snapshot(&env, &mm, key);
    let m = read::zc(&env.svm, &mm.market()).unwrap();
    let state = account.restore(&pda::config(), &mm.market(), &m).unwrap();
    assert_eq!(state.completed_history_len(), 0);
    assert!(
        state
            .live_marks_at(state.latest_metadata().unwrap().window.end, 2)
            .is_err()
    );
    env.warp(1);
    let mut warming = publication(&env, 3);
    warming.history_count = 0;
    warming.history = [AttestedPriceBucketArgs::default(); 8];
    ok(env.send(&[publish(&mm, key, gov.pubkey(), warming)], &[&gov]));
    let account = read_snapshot(&env, &mm, key);
    let state = account.restore(&pda::config(), &mm.market(), &m).unwrap();
    let now = state.latest_metadata().unwrap().window.end;
    assert!(state.live_marks_at(now, 3).is_ok());
    assert!(state.evidence_at(now, 3).is_err());
}
#[test]
fn runtime_owner_pda_version_codec_and_exact_account_length_are_enforced() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let a = args(gov.pubkey());
    let key = key(&mm, &a);
    ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    env.warp(1_000);
    let good = publication(&env, 1);
    let original = env.svm.get_account(&key).unwrap();
    for field in 0..8 {
        let mut bad = original.clone();
        match field {
            0 => bad.owner = Pubkey::new_unique(),
            1 => bad.data[43] ^= 1,
            2 => bad.data[8] ^= 1,
            3 => bad.data[91] ^= 0x80,
            4 => bad.data.push(0),
            5 => bad.data[11] ^= 1, // outer market must match the actual market account
            6 => bad.data[91 + 10 + 64] ^= 1, // decoded domain.config must match actual config
            _ => bad.data[59..91].fill(0), // missing immutable producer commitment
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

#[test]
fn single_interval_observation_per_completed_epoch_has_no_internal_gap() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let mut a = args(gov.pubkey());
    a.policy.epoch_seconds = 2;
    let key = key(&mm, &a);
    ok(env.send(&[init(&mm, gov.pubkey(), a)], &[&gov]));
    env.warp(1_000);
    let c = env.clock();
    let epoch = c.unix_timestamp as u64 / 2;
    let mut p = publication(&env, 1);
    for (i, b) in p.history.iter_mut().enumerate() {
        let e = epoch - 8 + i as u64;
        let first_ts = (e * 2) as i64;
        let last_ts = first_ts + 1;
        *b = AttestedPriceBucketArgs {
            epoch: e,
            first_slot: c.slot - (c.unix_timestamp - first_ts) as u64 * 2,
            first_ts,
            last_slot: c.slot - (c.unix_timestamp - last_ts) as u64 * 2,
            last_ts,
            lower_min: 10,
            central_last: 11,
            upper_max: 12,
            observations: 1,
            largest_gap_slots: 0,
            largest_gap_seconds: 0,
        };
    }
    let meta = ok(env.send(&[publish(&mm, key, gov.pubkey(), p)], &[&gov]));
    assert!(meta.compute_units_consumed < 200_000);
    let account = read_snapshot(&env, &mm, key);
    let m = read::zc(&env.svm, &mm.market()).unwrap();
    let state = account.restore(&pda::config(), &mm.market(), &m).unwrap();
    assert!(
        state
            .evidence_at(state.latest_metadata().unwrap().window.end, 1)
            .is_ok()
    );
}
