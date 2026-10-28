//! Actual new-program SBF publication and authority transitions. The history
//! values are explicit trusted fixtures; these tests do not prove API history.
#![allow(clippy::result_large_err)] // Retain complete LiteSVM failure metadata and logs for evidence.
use anchor_lang::{
    prelude::{Clock, Pubkey},
    InstructionData, ToAccountMetas,
};
use shortwin_simple::{
    accounts,
    initialization::NewPolicyArgs,
    instruction,
    reference::PublishReferenceArgs,
    state::{Config, Policy, PriceReference},
};
use shortwin_simple_runtime_tests::{assert_ok, deterministic_key, Env, TxResult};
use shortwin_simple_runtime_tests::{
    env as pda,
    tokens::{self, TokenProgram},
};
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_signer::Signer;

fn ix(accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
    Instruction {
        program_id: shortwin_simple::ID,
        accounts: accounts.to_account_metas(None),
        data: data.data(),
    }
}
fn reference(market: Pubkey, policy: Pubkey, q: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"reference",
            market.as_ref(),
            policy.as_ref(),
            &q.to_le_bytes(),
        ],
        &shortwin_simple::ID,
    )
    .0
}
fn fixture() -> (Env, Pubkey) {
    let mut env = Env::initialized();
    let authority = deterministic_key(90);
    let mint = deterministic_key(91);
    let mint = tokens::create_mint(
        &mut env,
        &mint,
        &authority.pubkey(),
        None,
        6,
        TokenProgram::Spl,
    );
    let creator = env.payer.insecure_clone();
    assert_ok(env.register_market(&creator, mint, TokenProgram::Spl.id(), 10_000, 100_000_000));
    (env, pda::market(&mint))
}
fn args(env: &Env, q: u64, sequence: u64) -> PublishReferenceArgs {
    let c = env.svm.get_sysvar::<Clock>();
    let epoch_start = c.unix_timestamp / 60 * 60;
    PublishReferenceArgs {
        quantity: q,
        sequence,
        source_slot: c.slot,
        source_ts: c.unix_timestamp,
        finite: true,
        live_bid_lower: 1_000_000,
        live_ask_upper: 1_010_000,
        history_ask_upper_max: 1_020_000,
        history_bid_lower_max: 1_000_000,
        acquisition_reference: 1_010_000,
        completed_epochs: 2,
        history_start_ts: epoch_start - 120,
        history_end_ts: epoch_start - 1,
        evidence_hash: [55; 32],
        max_reference_rent: 100_000_000,
    }
}
fn publish(
    env: &mut Env,
    market: Pubkey,
    version: u64,
    args: PublishReferenceArgs,
    signer: &Keypair,
) -> TxResult {
    let policy = pda::policy(version);
    let account = reference(market, policy, args.quantity);
    let accounts = accounts::PublishReference {
        reporter: signer.pubkey(),
        config: pda::config(),
        market,
        policy,
        reference: account,
        system_program: anchor_lang::system_program::ID,
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    env.send(
        &[ix(accounts, instruction::PublishReference { args })],
        &[signer],
    )
}
fn tick(env: &mut Env) {
    let c = env.svm.get_sysvar::<Clock>();
    env.set_clock(c.unix_timestamp + 1, c.slot + 1);
}
fn pause(env: &mut Env, signer: &Keypair, paused: bool) -> TxResult {
    let accounts = accounts::RoleAuthority {
        authority: signer.pubkey(),
        config: pda::config(),
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    env.send(
        &[ix(
            accounts,
            instruction::SetPause {
                prices: true,
                paused,
            },
        )],
        &[signer],
    )
}

#[test]
fn only_reporter_can_publish_and_creation_rent_is_explicitly_bounded() {
    let (mut env, market) = fixture();
    let reporter = env.reporter.insecure_clone();
    let stranger = deterministic_key(92);
    env.fund(stranger.pubkey(), 100_000_000);
    let key = reference(market, pda::policy(1), 7);
    let a = args(&env, 7, 1);
    let before = env.lamports(&stranger.pubkey());
    assert!(publish(&mut env, market, 1, a.clone(), &stranger).is_err());
    assert!(env.svm.get_account(&key).is_none());
    assert_eq!(env.lamports(&stranger.pubkey()), before);
    let mut bounded = a.clone();
    bounded.max_reference_rent = 0;
    let before = env.lamports(&reporter.pubkey());
    assert!(publish(&mut env, market, 1, bounded, &reporter).is_err());
    assert!(env.svm.get_account(&key).is_none());
    assert_eq!(env.lamports(&reporter.pubkey()), before);
    assert_ok(publish(&mut env, market, 1, a, &reporter));
    let r: PriceReference = env.read(&key);
    assert_eq!(
        (r.quantity, r.sequence, r.reporter_epoch, r.finite),
        (7, 1, 1, true)
    );
}

#[test]
fn reference_sequence_exact_history_spread_and_dual_clock_checks_are_atomic() {
    let (mut env, market) = fixture();
    let reporter = env.reporter.insecure_clone();
    let key = reference(market, pda::policy(1), 7);
    for mutation in 0..8 {
        let mut a = args(&env, 7, 1);
        match mutation {
            0 => a.source_slot += 1,
            1 => a.source_ts += 1,
            2 => a.source_ts -= 31,
            3 => a.source_slot -= 101,
            4 => a.evidence_hash = [0; 32],
            5 => a.history_start_ts += 1,
            6 => a.completed_epochs = 3,
            7 => a.live_ask_upper = 2_000_000,
            _ => unreachable!(),
        }
        assert!(
            publish(&mut env, market, 1, a, &reporter).is_err(),
            "mutation {mutation}"
        );
        assert!(env.svm.get_account(&key).is_none());
    }
    let first = args(&env, 7, 1);
    assert_ok(publish(&mut env, market, 1, first.clone(), &reporter));
    let original = env.svm.get_account(&key).unwrap();
    assert!(publish(&mut env, market, 1, first.clone(), &reporter).is_err());
    assert_eq!(env.svm.get_account(&key).unwrap(), original);
    let mut second = first.clone();
    second.sequence = 2;
    assert!(publish(&mut env, market, 1, second, &reporter).is_err());
    assert_eq!(env.svm.get_account(&key).unwrap(), original);
    tick(&mut env);
    let mut skipped = args(&env, 7, 3);
    assert!(publish(&mut env, market, 1, skipped.clone(), &reporter).is_err());
    skipped.sequence = 2;
    assert_ok(publish(&mut env, market, 1, skipped, &reporter));
    let r: PriceReference = env.read(&key);
    assert_eq!(r.sequence, 2);
    assert!(r.source_slot > first.source_slot);
    // A current live jump above completed history is permissible; it is not
    // concealed by forcing history to include an uncompleted current epoch.
    tick(&mut env);
    let mut jump = args(&env, 7, 3);
    jump.live_bid_lower = 1_990_000;
    jump.live_ask_upper = 2_000_000;
    assert_ok(publish(&mut env, market, 1, jump, &reporter));
}

#[test]
fn unknown_clears_eligibility_and_reporter_rotation_keeps_monotonic_identity() {
    let (mut env, market) = fixture();
    let reporter = env.reporter.insecure_clone();
    let governance = env.governance.insecure_clone();
    let key = reference(market, pda::policy(1), 7);
    let first = args(&env, 7, 1);
    assert_ok(publish(&mut env, market, 1, first, &reporter));
    tick(&mut env);
    let mut unknown = args(&env, 7, 2);
    unknown.finite = false;
    assert_ok(publish(&mut env, market, 1, unknown, &reporter));
    let r: PriceReference = env.read(&key);
    assert!(!r.finite);
    assert_eq!(
        (
            r.live_bid_lower,
            r.live_ask_upper,
            r.history_ask_upper_max,
            r.history_bid_lower_max,
            r.acquisition_reference
        ),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(
        (
            r.completed_epochs,
            r.history_start_ts,
            r.history_end_ts,
            r.evidence_hash
        ),
        (0, 0, 0, [0; 32])
    );
    let replacement = deterministic_key(93);
    env.fund(replacement.pubkey(), 100_000_000);
    let accounts = accounts::RoleAuthority {
        authority: governance.pubkey(),
        config: pda::config(),
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    assert_ok(env.send(
        &[ix(
            accounts,
            instruction::SetReporter {
                reporter: replacement.pubkey(),
            },
        )],
        &[&governance],
    ));
    let c: Config = env.read(&pda::config());
    assert_eq!(c.reporter_epoch, 2);
    tick(&mut env);
    let next = args(&env, 7, 3);
    let before = env.svm.get_account(&key).unwrap();
    assert!(publish(&mut env, market, 1, next.clone(), &reporter).is_err());
    assert_eq!(env.svm.get_account(&key).unwrap(), before);
    let mut reset = next.clone();
    reset.sequence = 1;
    assert!(publish(&mut env, market, 1, reset, &replacement).is_err());
    assert_ok(publish(&mut env, market, 1, next, &replacement));
    let r: PriceReference = env.read(&key);
    assert_eq!((r.sequence, r.reporter_epoch, r.finite), (3, 2, true));
}

#[test]
fn old_immutable_policy_remains_publishable_and_price_hold_can_be_refreshed() {
    let (mut env, market) = fixture();
    let reporter = env.reporter.insecure_clone();
    let governance = env.governance.insecure_clone();
    let emergency = env.emergency.insecure_clone();
    let old: Policy = env.read(&pda::policy(1));
    let old_bytes = env.svm.get_account(&pda::policy(1)).unwrap().data;
    let mut terms = old.terms;
    terms.term_fee_bps += 1;
    let accounts = accounts::NewPolicy {
        authority: governance.pubkey(),
        config: pda::config(),
        policy: pda::policy(2),
        system_program: anchor_lang::system_program::ID,
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    assert_ok(env.send(
        &[ix(
            accounts,
            instruction::NewPolicy {
                args: NewPolicyArgs {
                    version: 2,
                    terms,
                    max_setup_rent: 100_000_000,
                },
            },
        )],
        &[&governance],
    ));
    assert_eq!(
        env.svm.get_account(&pda::policy(1)).unwrap().data,
        old_bytes
    );
    let c: Config = env.read(&pda::config());
    assert_eq!(c.current_policy, pda::policy(2));
    let a = args(&env, 7, 1);
    assert_ok(publish(&mut env, market, 1, a, &reporter));
    assert_ok(pause(&mut env, &emergency, true));
    let a = args(&env, 7, 1);
    assert_ok(publish(&mut env, market, 2, a, &reporter));
    assert!(pause(&mut env, &emergency, false).is_err());
    let c: Config = env.read(&pda::config());
    assert!(c.prices_paused);
    assert_ok(pause(&mut env, &governance, false));
    let c: Config = env.read(&pda::config());
    assert!(!c.prices_paused);
}
