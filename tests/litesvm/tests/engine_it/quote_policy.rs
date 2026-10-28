//! Actual SBF immutable policy, authenticated reference preflight and scoped pause control.
use crate::common::*;
use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{AnchorDeserialize, InstructionData, ToAccountMetas};
use shortwin::instructions::initialize_quote_policy::QuoteReferenceView;
use shortwin::state::{
    AttestedPriceBucketArgs, InitializePriceSnapshotArgs, InitializeQuotePolicyArgs,
    MARKET_FLAG_QUOTE_NATIVE, MARKET_FLAG_QUOTE_PRICE_PAUSED, PriceReportModelArgs,
    PriceReportPolicyArgs, PriceReportStatusArgs, PriceSnapshot, PublishPriceSnapshotArgs,
    QUOTE_FINANCE_VERSION, QuoteFinanceMarket, QuoteFinancialTerms, QuoteHistoryPolicy,
    QuotePolicy, QuotePolicyDomain,
};
use shortwin_litesvm_tests::{fixtures, invariants, ix};
use shortwin_math::quote_rent::persistence;
use solana_signer::Signer;
fn terms() -> QuoteFinancialTerms {
    QuoteFinancialTerms {
        position_quantity_max: 2000,
        market_quantity_max: 10000,
        min_notional: 100_000_000,
        position_notional_max: 5_000_000_000,
        market_notional_max: 25_000_000_000,
        utilization_cap_bps: 9000,
        wallet_cap_bps: 5000,
        position_premium_bps: 1000,
        capacity_premium_bps: 1500,
        cr_open_bps: 40000,
        cr_liq_bps: 17500,
        open_fee_bps: 25,
        rent_protocol_bps: 1500,
        util_kink_bps: 8000,
        rate_base_bps: 10000,
        rate_slope1_bps: 37500,
        rate_slope2_bps: 550000,
        min_rent_seconds: 3600,
        max_duration_seconds: 86400,
        penalty_bps: 1000,
        penalty_lender_bps: 7500,
        penalty_protocol_bps: 1250,
        penalty_keeper_bps: 1250,
        expiry_fee_bps: 100,
        settle_keeper_fixed: 1_000_000,
        settle_keeper_bps: 50,
        settle_keeper_cap: 10_000_000,
        expiry_keeper_fixed: 100_000,
        expiry_keeper_bps: 10,
        expiry_keeper_cap: 5_000_000,
        liquidation_delta_bps: 300,
        expiry_delta_bps: 100,
        completed_liquidation_epochs: 2,
        cr_arm_delay_seconds: 2,
        cr_arm_delay_slots: 2,
        coherence_max_seconds: 4,
        coherence_max_slots: 16,
        lump_vest_seconds: 86400,
        default_grace_seconds: 86400,
    }
}
fn args(env: &Env, mm: &MockMarket) -> InitializeQuotePolicyArgs {
    let market = read::zc(&env.svm, &mm.market()).unwrap();
    let mut commitments = [[0; 32]; 10];
    for (i, c) in commitments.iter_mut().take(4).enumerate() {
        *c = [u8::try_from(i + 1).unwrap(); 32];
    }
    let mut a = InitializeQuotePolicyArgs {
        genesis_hash: [7; 32],
        model_version: 1,
        model_epoch: 1,
        expected_policy_hash: [0; 32],
        history: QuoteHistoryPolicy {
            max_age_slots: 16,
            max_age_seconds: 4,
            max_window_slots: 8,
            max_window_seconds: 2,
            max_update_gap_slots: 8,
            max_update_gap_seconds: 2,
            epoch_seconds: 10,
            minimum_completed_epochs: 2,
        },
        lot_count: 2,
        lots: [1000, 2000, 0, 0, 0, 0, 0, 0],
        small_lot_quantity: 100,
        capacity_quantity: 10000,
        producer_commitments: commitments,
        terms: terms(),
    };
    a.expected_policy_hash = a.financial_hash_for_domain(&QuotePolicyDomain::for_market(
        a.genesis_hash,
        pda::config(),
        mm.market(),
        &market,
    ));
    a
}
fn policy_key(mm: &MockMarket, a: &InitializeQuotePolicyArgs) -> Pubkey {
    Pubkey::find_program_address(
        &[
            QuotePolicy::SEED,
            mm.market().as_ref(),
            &a.model_epoch.to_le_bytes(),
        ],
        &shortwin::ID,
    )
    .0
}
fn init_policy(mm: &MockMarket, authority: Pubkey, args: InitializeQuotePolicyArgs) -> Instruction {
    Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::InitializeQuotePolicy {
            authority,
            config: pda::config(),
            market: mm.market(),
            quote_policy: policy_key(mm, &args),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: shortwin::instruction::InitializeQuotePolicy { args }.data(),
    }
}
fn snapshot_key(mm: &MockMarket, q: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            PriceSnapshot::SEED,
            mm.market().as_ref(),
            &q.to_le_bytes(),
            &1u64.to_le_bytes(),
        ],
        &shortwin::ID,
    )
    .0
}
fn init_snapshot(
    mm: &MockMarket,
    authority: Pubkey,
    publisher: Pubkey,
    a: &InitializeQuotePolicyArgs,
    q: u64,
    producer: [u8; 32],
) -> Instruction {
    let mut publishers = [Pubkey::default(); 8];
    publishers[0] = publisher;
    let p = a.history;
    let args = InitializePriceSnapshotArgs {
        genesis_hash: a.genesis_hash,
        producer_commitment: producer,
        quantity: q,
        model: PriceReportModelArgs {
            version: a.model_version,
            epoch: a.model_epoch,
            policy_hash: a.expected_policy_hash,
        },
        policy: PriceReportPolicyArgs {
            max_age_slots: p.max_age_slots,
            max_age_seconds: p.max_age_seconds,
            max_window_slots: p.max_window_slots,
            max_window_seconds: p.max_window_seconds,
            max_update_gap_slots: p.max_update_gap_slots,
            max_update_gap_seconds: p.max_update_gap_seconds,
            epoch_seconds: p.epoch_seconds,
            minimum_completed_epochs: p.minimum_completed_epochs,
        },
        publisher_count: 1,
        publishers,
    };
    Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::InitializePriceSnapshot {
            authority,
            config: pda::config(),
            market: mm.market(),
            price_snapshot: snapshot_key(mm, q),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: shortwin::instruction::InitializePriceSnapshot { args }.data(),
    }
}
fn publish(
    env: &Env,
    mm: &MockMarket,
    publisher: Pubkey,
    q: u64,
    sequence: u64,
    unknown: bool,
) -> Instruction {
    let c = env.clock();
    let epoch = c.unix_timestamp as u64 / 10;
    let mut history = [AttestedPriceBucketArgs::default(); 8];
    if !unknown {
        for (i, b) in history.iter_mut().take(2).enumerate() {
            let e = epoch - 2 + i as u64;
            let first_ts = (e * 10) as i64;
            let last_ts = first_ts + 9;
            *b = AttestedPriceBucketArgs {
                epoch: e,
                first_slot: c.slot - (c.unix_timestamp - first_ts) as u64 * 2,
                first_ts,
                last_slot: c.slot - (c.unix_timestamp - last_ts) as u64 * 2,
                last_ts,
                lower_min: 10,
                central_last: 11,
                upper_max: 12,
                observations: 10,
                largest_gap_slots: 2,
                largest_gap_seconds: 1,
            };
        }
    }
    let args = PublishPriceSnapshotArgs {
        version: 2,
        sequence,
        start_slot: c.slot,
        start_ts: c.unix_timestamp,
        end_slot: c.slot,
        end_ts: c.unix_timestamp,
        observation_digest: [9; 32],
        status: if unknown {
            PriceReportStatusArgs::Unknown
        } else {
            PriceReportStatusArgs::Finite {
                lower: 10,
                central: 11,
                upper: 12,
            }
        },
        history_count: if unknown { 0 } else { 2 },
        history,
    };
    Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::PublishPriceSnapshot {
            publisher,
            config: pda::config(),
            market: mm.market(),
            price_snapshot: snapshot_key(mm, q),
        }
        .to_account_metas(None),
        data: shortwin::instruction::PublishPriceSnapshot { args }.data(),
    }
}
fn check(mm: &MockMarket, policy: Pubkey, snapshot: Pubkey, q: u64, min: u64) -> Instruction {
    Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::CheckQuoteReference {
            config: pda::config(),
            market: mm.market(),
            quote_policy: policy,
            price_snapshot: snapshot,
        }
        .to_account_metas(None),
        data: shortwin::instruction::CheckQuoteReference {
            quantity: q,
            minimum_sequence: min,
        }
        .data(),
    }
}
fn pause(mm: &MockMarket, authority: Pubkey, paused: bool) -> Instruction {
    Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::SetQuotePricePause {
            authority,
            config: pda::config(),
            market: mm.market(),
        }
        .to_account_metas(None),
        data: shortwin::instruction::SetQuotePricePause { paused }.data(),
    }
}
fn with_compute(ix: Instruction) -> Vec<Instruction> {
    let id = "ComputeBudget111111111111111111111111111111"
        .parse()
        .unwrap();
    let mut limit = vec![2];
    limit.extend_from_slice(&200_000u32.to_le_bytes());
    let mut price = vec![3];
    price.extend_from_slice(&1u64.to_le_bytes());
    vec![
        Instruction {
            program_id: id,
            accounts: vec![],
            data: limit,
        },
        Instruction {
            program_id: id,
            accounts: vec![],
            data: price,
        },
        ix,
    ]
}
#[test]
fn governed_policy_initialization_fits_packet_preserves_market_and_has_no_adoption() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let outsider = env.funded_keypair();
    let a = args(&env, &mm);
    let key = policy_key(&mm, &a);
    let before = env.svm.get_account(&mm.market());
    assert!(
        env.send(&[init_policy(&mm, outsider.pubkey(), a)], &[&outsider])
            .is_err()
    );
    assert!(env.svm.get_account(&key).is_none());
    for variant in 0..4 {
        let mut bad = a;
        match variant {
            0 => bad.expected_policy_hash = [99; 32],
            1 => bad.producer_commitments[9] = [9; 32],
            2 => bad.terms.market_notional_max = 25_000_000_001,
            _ => bad.terms.default_grace_seconds = 1,
        };
        assert!(
            env.send(&[init_policy(&mm, gov.pubkey(), bad)], &[&gov])
                .is_err()
        );
        assert!(env.svm.get_account(&key).is_none());
        assert_eq!(env.svm.get_account(&mm.market()), before);
    }
    let ixs = with_compute(init_policy(&mm, gov.pubkey(), a));
    let tx = env.transaction(&ixs, &[&gov]);
    let packet = 1 + tx.signatures.len() * 64 + tx.message_data().len();
    assert_eq!(tx.signatures.len(), 2);
    assert!(packet <= 1232, "policy packet {packet}");
    let meta = ok(env.send(&ixs, &[&gov]));
    eprintln!(
        "QuotePolicy init {} bytes withCUlimit/price, {} CU",
        packet, meta.compute_units_consumed
    );
    assert!(meta.compute_units_consumed < 200000);
    let p: QuotePolicy = read::borsh(&env.svm, &key).unwrap();
    assert_eq!(p.policy_hash, a.expected_policy_hash);
    assert_eq!(env.svm.get_account(&key).unwrap().data.len(), 1341);
    assert_eq!(env.svm.get_account(&mm.market()), before);
    assert!(!env.market(&mm.market()).has_flag(MARKET_FLAG_QUOTE_NATIVE));
    let finance = Pubkey::find_program_address(
        &[QuoteFinanceMarket::SEED, mm.market().as_ref()],
        &shortwin::ID,
    )
    .0;
    assert!(env.svm.get_account(&finance).is_none());
}
#[test]
fn actual_clock_preflight_authenticates_each_exact_quantity_without_mutating_or_claiming_execution()
{
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let reporter = env.funded_keypair();
    let a = args(&env, &mm);
    let key = policy_key(&mm, &a);
    ok(env.send(&[init_policy(&mm, gov.pubkey(), a)], &[&gov]));
    for (q, producer) in [(100, [1; 32]), (1000, [2; 32]), (10000, [4; 32])] {
        ok(env.send(
            &[init_snapshot(
                &mm,
                gov.pubkey(),
                reporter.pubkey(),
                &a,
                q,
                producer,
            )],
            &[&gov],
        ));
    }
    env.warp(1000);
    for q in [100, 1000, 10000] {
        let ix = publish(&env, &mm, reporter.pubkey(), q, 1, false);
        ok(env.send(&[ix], &[&reporter]));
        let before = [
            env.svm.get_account(&mm.market()),
            env.svm.get_account(&key),
            env.svm.get_account(&snapshot_key(&mm, q)),
        ];
        let meta = ok(env.send(&[check(&mm, key, snapshot_key(&mm, q), q, 1)], &[]));
        assert!(meta.compute_units_consumed < 200000);
        let view = QuoteReferenceView::deserialize(&mut meta.return_data.data.as_slice()).unwrap();
        assert_eq!(
            view,
            QuoteReferenceView {
                quantity: q,
                source_epoch: env.now() as u64 / 10,
                opening_upper: 12,
                lower_base: 10,
                acquisition_reference: 12
            }
        );
        assert_eq!(
            meta.return_data.program_id.to_bytes(),
            shortwin::ID.to_bytes()
        );
        assert_eq!(
            before,
            [
                env.svm.get_account(&mm.market()),
                env.svm.get_account(&key),
                env.svm.get_account(&snapshot_key(&mm, q))
            ]
        );
    }
    assert!(
        env.send(&[check(&mm, key, snapshot_key(&mm, 1000), 1000, 2)], &[])
            .is_err()
    );
    assert!(
        env.send(&[check(&mm, key, snapshot_key(&mm, 1000), 2000, 1)], &[])
            .is_err()
    );
    env.warp(5);
    assert!(
        env.send(&[check(&mm, key, snapshot_key(&mm, 1000), 1000, 1)], &[])
            .is_err()
    );
}
#[test]
fn owner_pda_domain_producer_model_tail_and_unknown_refusals_fail_closed() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let a = args(&env, &mm);
    let key = policy_key(&mm, &a);
    let sk = snapshot_key(&mm, 1000);
    ok(env.send(&[init_policy(&mm, gov.pubkey(), a)], &[&gov]));
    ok(env.send(
        &[init_snapshot(
            &mm,
            gov.pubkey(),
            gov.pubkey(),
            &a,
            1000,
            [2; 32],
        )],
        &[&gov],
    ));
    env.warp(1000);
    let ix = publish(&env, &mm, gov.pubkey(), 1000, 1, false);
    ok(env.send(&[ix], &[&gov]));
    let original = env.svm.get_account(&sk).unwrap();
    for variant in 0..6 {
        let mut bad = original.clone();
        match variant {
            0 => bad.owner = Pubkey::new_unique(),
            1 => bad.data[59] ^= 1,
            2 => bad.data[91 + 288] ^= 1,
            3 => bad.data[91 + 10] ^= 1,
            4 => bad.data.push(0),
            _ => bad.data[0] ^= 1,
        };
        env.svm.set_account(sk, bad.clone()).unwrap();
        assert!(
            env.send(&[check(&mm, key, sk, 1000, 1)], &[]).is_err(),
            "variant {variant}"
        );
        assert_eq!(env.svm.get_account(&sk), Some(bad));
    }
    env.svm.set_account(sk, original).unwrap();
    assert!(
        env.send(&[check(&mm, Pubkey::new_unique(), sk, 1000, 1)], &[])
            .is_err()
    );
    let mut trailing = check(&mm, key, sk, 1000, 1);
    trailing.accounts.push(trailing.accounts[0].clone());
    assert!(env.send(&[trailing], &[]).is_err());
    env.warp(1);
    let ix = publish(&env, &mm, gov.pubkey(), 1000, 2, true);
    ok(env.send(&[ix], &[&gov]));
    assert!(env.send(&[check(&mm, key, sk, 1000, 2)], &[]).is_err());
}
#[test]
fn price_pause_roles_are_narrow_and_cannot_change_backend_or_financial_terms() {
    let (mut env, mm) = setup();
    let gov = env.governance.insecure_clone();
    let emergency = env.pause_authority.insecure_clone();
    let reporter = env.funded_keypair();
    let a = args(&env, &mm);
    let key = policy_key(&mm, &a);
    let sk = snapshot_key(&mm, 1000);
    ok(env.send(&[init_policy(&mm, gov.pubkey(), a)], &[&gov]));
    ok(env.send(
        &[init_snapshot(
            &mm,
            gov.pubkey(),
            reporter.pubkey(),
            &a,
            1000,
            [2; 32],
        )],
        &[&gov],
    ));
    env.warp(1000);
    let ix = publish(&env, &mm, reporter.pubkey(), 1000, 1, false);
    ok(env.send(&[ix], &[&reporter]));
    let before = env.market(&mm.market());
    let policy_before = env.svm.get_account(&key);
    let cache_before = env.svm.get_account(&sk);
    assert!(
        env.send(&[pause(&mm, reporter.pubkey(), true)], &[&reporter])
            .is_err()
    );
    ok(env.send(&[pause(&mm, emergency.pubkey(), true)], &[&emergency]));
    let mut expected = before;
    expected.set_flag(MARKET_FLAG_QUOTE_PRICE_PAUSED, true);
    assert_eq!(
        &env.svm.get_account(&mm.market()).unwrap().data[8..],
        bytemuck::bytes_of(&expected)
    );
    assert_err(
        &env.send(&[check(&mm, key, sk, 1000, 1)], &[]),
        shortwin::errors::ShortwinError::QuotePriceActionsPaused,
    );
    assert!(
        env.send(&[pause(&mm, emergency.pubkey(), false)], &[&emergency])
            .is_err()
    );
    assert!(
        env.send(&[pause(&mm, reporter.pubkey(), false)], &[&reporter])
            .is_err()
    );
    ok(env.send(&[pause(&mm, gov.pubkey(), false)], &[&gov]));
    ok(env.send(&[check(&mm, key, sk, 1000, 1)], &[]));
    assert_eq!(env.svm.get_account(&key), policy_before);
    assert_eq!(env.svm.get_account(&sk), cache_before);
    assert!(!env.market(&mm.market()).has_flag(MARKET_FLAG_QUOTE_NATIVE));
}
#[test]
fn legacy_crank_and_deposit_refuse_seeded_quote_backend_before_any_financial_write() {
    let (mut env, mm) = setup();
    let mut m = env.market(&mm.market());
    m.set_flag(MARKET_FLAG_QUOTE_NATIVE, true);
    let (finance_key, bump) = Pubkey::find_program_address(
        &[QuoteFinanceMarket::SEED, mm.market().as_ref()],
        &shortwin::ID,
    );
    let mut finance = QuoteFinanceMarket {
        version: QUOTE_FINANCE_VERSION,
        bump,
        market: mm.market(),
        config: pda::config(),
        model_version: 1,
        model_epoch: 1,
        policy_hash: [9; 32],
        rent_state: [0; persistence::MARKET_ENCODED_LEN],
    };
    finance
        .store(&persistence::MarketCheckpoint::empty(
            m.last_accrual_ts as u64,
        ))
        .unwrap();
    fixtures::set_zc(&mut env.svm, &mm.market(), &m, 0);
    fixtures::set_borsh(
        &mut env.svm,
        &finance_key,
        &finance,
        QuoteFinanceMarket::SPACE,
        0,
    );
    invariants::check_all(&env.svm, &mm.market()).unwrap();
    let before = [
        env.svm.get_account(&mm.market()),
        env.svm.get_account(&mm.keys.market_vault),
        env.svm.get_account(&mm.keys.pool_vault),
        env.svm.get_account(&mm.lender_account),
        env.svm.get_account(&finance_key),
    ];
    assert_err(
        &env.send(&[crank_price_ix(&mm)], &[]),
        shortwin::errors::ShortwinError::AccountMismatch,
    );
    let lender = mm.lender.insecure_clone();
    assert_err(
        &env.send(&[ix::deposit(&mm.lender_keys(), 1, 0)], &[&lender]),
        shortwin::errors::ShortwinError::AccountMismatch,
    );
    assert_eq!(
        before,
        [
            env.svm.get_account(&mm.market()),
            env.svm.get_account(&mm.keys.market_vault),
            env.svm.get_account(&mm.keys.pool_vault),
            env.svm.get_account(&mm.lender_account),
            env.svm.get_account(&finance_key)
        ]
    );
}
