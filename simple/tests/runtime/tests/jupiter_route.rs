//! Actual new-program exits through the pinned Router V2 ELF. All loans are first
//! opened by the real financial handler; historical state/test ALT are local only.
use shortwin_simple::{
    accounts, finance, instruction, route,
    state::{Exposure, Loan, Policy, PoolGeneration},
};
use shortwin_simple_runtime_tests::{
    assert_ok, deterministic_key, env as pda,
    exit_case::*,
    jupiter,
    position_case::{exposure, instruction as ix, loan, Q},
};
use solana_signer::Signer;
#[test]
fn real_borrower_buyback_refunds_both_rents_and_prefunds_and_credits_token_surplus() {
    let mut c = opened();
    let l: Loan = c.env.read(&key(&c));
    let p: PoolGeneration = c.env.read(&c.pool);
    let loan_balance = c.env.lamports(&key(&c));
    let exposure_balance = c.env.lamports(&exposure(c.pool, c.borrower.pubkey()));
    let borrower_before = c.env.lamports(&c.borrower.pubkey());
    for (seed, amount) in [(route::BASE_SEED, 11), (route::WSOL_SEED, 17)] {
        let address = scratch(&c, seed);
        c.env
            .svm
            .set_account(
                address,
                solana_account::Account {
                    lamports: amount,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let input = 3_000_000;
    let i = close(&c, input, Q);
    let meta = assert_ok(c.env.send_v0(&[i], &[&c.borrower]).0);
    assert!(meta
        .logs
        .iter()
        .any(|s| s == &format!("Program {} success", jupiter::CPMM_ID)));
    let after: PoolGeneration = c.env.read(&c.pool);
    assert_eq!(
        (
            after.principal_out,
            after.loan_count,
            after.booked_notional_out
        ),
        (0, 0, 0)
    );
    assert!(
        after.cash > p.cash + l.q,
        "new exact full-debt plus surplus tokens credited"
    );
    assert_eq!(after.cash, c.env.token_amount(&pda::token_vault(&c.pool)));
    assert_eq!(
        after.sol_reserve, p.sol_reserve,
        "paid upfront fee remains lender income"
    );
    assert_eq!(
        c.env.lamports(&c.borrower.pubkey()) - borrower_before,
        loan_balance + exposure_balance + 28 - input
    );
    assert!(c.env.svm.get_account(&key(&c)).is_none());
    assert!(c
        .env
        .svm
        .get_account(&exposure(c.pool, c.borrower.pubkey()))
        .is_none());
    for seed in [route::BASE_SEED, route::WSOL_SEED] {
        assert!(c.env.svm.get_account(&scratch(&c, seed)).is_none());
    }
}
#[test]
fn wrong_input_and_actual_router_underfill_revert_pool_loan_user_and_dex() {
    let mut c = opened();
    for mode in 0..2 {
        let mut i = close(&c, 3_000_000, if mode == 0 { Q } else { 100 * Q });
        if mode == 0 {
            let offset = 8 + 8 + 8; // discriminator + maxcost + temporarymax, then RouteArgs.input
            i.data[offset..offset + 8].copy_from_slice(&2_900_000u64.to_le_bytes());
        }
        let (keys, before) = snapshot(&c, &[]);
        let e = c
            .env
            .send_v0(&[i], &[&c.borrower])
            .0
            .expect_err("wrong input/underfill refuses");
        assert_eq!(
            c.snapshot(&keys),
            before,
            "full actual-state rollback mode{mode}"
        );
        if mode == 1 {
            assert!(
                e.meta
                    .logs
                    .iter()
                    .any(|l| l == &format!("Program {} success", jupiter::CPMM_ID)),
                "inner swap happened before underfill failure"
            );
        }
    }
}
#[test]
fn direct_full_repayment_works_during_price_hold_without_jupiter_and_preserves_fee() {
    let mut c = opened();
    let src = source(&mut c, 112);
    let l: Loan = c.env.read(&key(&c));
    let old: PoolGeneration = c.env.read(&c.pool);
    let governance = c.env.governance.insecure_clone();
    let a = accounts::RoleAuthority {
        authority: governance.pubkey(),
        config: pda::config(),
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    assert_ok(c.env.send(
        &[ix(
            a,
            instruction::SetPause {
                prices: true,
                paused: true,
            },
        )],
        &[&governance],
    ));
    let pd = c.env.svm.get_account(&route::JUPITER_PROGRAM_DATA).unwrap();
    let mut changed = pd;
    changed.data[4..12].copy_from_slice(&999u64.to_le_bytes());
    c.env
        .svm
        .set_account(route::JUPITER_PROGRAM_DATA, changed)
        .unwrap();
    let before = c.env.lamports(&c.borrower.pubkey());
    let total = c.env.lamports(&key(&c)) + c.env.lamports(&exposure(c.pool, c.borrower.pubkey()));
    let i = repay(&c, src);
    assert_ok(c.env.send(&[i], &[&c.borrower]));
    let after: PoolGeneration = c.env.read(&c.pool);
    assert_eq!(
        (after.cash, after.principal_out, after.loan_count),
        (old.cash + l.q, 0, 0)
    );
    assert_eq!(after.sol_reserve, old.sol_reserve);
    assert_eq!(c.env.token_amount(&src), Q);
    assert_eq!(c.env.lamports(&c.borrower.pubkey()) - before, total);
}
#[test]
fn exact_grace_boundary_is_default_only_and_recovers_collateral_without_price_token_or_router() {
    let mut c = opened();
    let k = keeper(&mut c);
    let src = source(&mut c, 113);
    let l: Loan = c.env.read(&key(&c));
    let p: PoolGeneration = c.env.read(&c.pool);
    clock(&mut c, l.default_at, jupiter::EXECUTION_SLOT + 10);
    for i in [
        repay(&c, src),
        close(&c, 3_000_000, Q),
        add(&c, 1),
        force(&c, &k, 2_500_000),
    ] {
        let (keys, before) = snapshot(&c, &[k.pubkey(), src]);
        let actor = if i
            .accounts
            .iter()
            .any(|m| m.is_signer && m.pubkey == k.pubkey())
        {
            &k
        } else {
            &c.borrower
        };
        error(c.env.send_v0(&[i], &[actor]).0, "DefaultRequired");
        assert_eq!(c.snapshot(&keys), before);
    }
    let borrower_before = c.env.lamports(&c.borrower.pubkey());
    let actor_before = c.env.lamports(&k.pubkey());
    let pool_before = c.env.lamports(&c.pool);
    let total = c.env.lamports(&key(&c)) + c.env.lamports(&exposure(c.pool, c.borrower.pubkey()));
    let policy: Policy = c.env.read(&pda::policy(1));
    let reward = finance::keeper_reward(l.collateral, l.collateral, &policy.terms).unwrap();
    let i = default_ix(&c, &k);
    assert_ok(c.env.send(&[i], &[&k]));
    let after: PoolGeneration = c.env.read(&c.pool);
    assert_eq!(
        (
            after.cash,
            after.principal_out,
            after.loan_count,
            after.booked_notional_out
        ),
        (p.cash, 0, 0, 0)
    );
    assert_eq!(after.sol_reserve, p.sol_reserve + l.collateral - reward);
    assert_eq!(c.env.lamports(&c.pool) - pool_before, l.collateral - reward);
    assert_eq!(c.env.lamports(&k.pubkey()) - actor_before, reward);
    assert_eq!(
        c.env.lamports(&c.borrower.pubkey()) - borrower_before,
        total - l.collateral,
        "borrower gets only account rent/untracked gifts, no economic collateral"
    );
}
#[test]
fn expiry_buyback_pays_capped_keeper_and_refunds_surplus_before_grace_end() {
    let mut c = opened();
    let k = keeper(&mut c);
    let l: Loan = c.env.read(&key(&c));
    let p: PoolGeneration = c.env.read(&c.pool);
    let ts = l.deadline;
    let slot = jupiter::EXECUTION_SLOT + 20;
    clock(&mut c, ts, slot);
    publish(&mut c, ts, slot, 2_200_000, 2_400_000);
    let before = c.env.lamports(&k.pubkey());
    let input = 2_500_000;
    let policy: Policy = c.env.read(&pda::policy(1));
    let expected = finance::keeper_reward(input, l.collateral - input, &policy.terms).unwrap();
    let i = force(&c, &k, input);
    let meta = assert_ok(c.env.send_v0(&[i], &[&k]).0);
    assert!(meta
        .logs
        .iter()
        .any(|l| l == &format!("Program {} success", jupiter::CPMM_ID)));
    assert_eq!(
        c.env.lamports(&k.pubkey()) - before,
        expected,
        "temporary rent advance refunded exactly"
    );
    let after: PoolGeneration = c.env.read(&c.pool);
    assert_eq!((after.principal_out, after.loan_count), (0, 0));
    assert!(after.cash >= p.cash + Q);
    assert_eq!(after.sol_reserve, p.sol_reserve);
}
#[test]
fn delayed_source_observations_health_clear_and_actual_fill_veto_are_atomic() {
    let mut c = opened();
    let k = keeper(&mut c);
    let ts = pda::START_TS;
    let slot = jupiter::EXECUTION_SLOT;
    clock(&mut c, ts + 1, slot + 1);
    publish(&mut c, ts + 1, slot + 1, 10_000_000, 10_100_000);
    let i = arm(&c, &k);
    assert_ok(c.env.send(&[i], &[&k]));
    let armed: Loan = c.env.read(&key(&c));
    clock(&mut c, ts + 6, slot + 6);
    publish(&mut c, ts + 5, slot + 5, 10_000_000, 10_100_000);
    let i = force(&c, &k, 3_000_000);
    let (keys, before) = snapshot(&c, &[k.pubkey()]);
    error(c.env.send_v0(&[i], &[&k]).0, "ConfirmationPending");
    assert_eq!(c.snapshot(&keys), before);
    clock(&mut c, ts + 7, slot + 7);
    publish(&mut c, ts + 7, slot + 7, 2_200_000, 2_400_000);
    let i = arm(&c, &k);
    assert_ok(c.env.send(&[i], &[&k]));
    let cleared: Loan = c.env.read(&key(&c));
    assert_eq!(
        (cleared.cr_arm_sequence, cleared.cr_arm_reporter_epoch),
        (0, 0)
    );
    clock(&mut c, ts + 8, slot + 8);
    publish(&mut c, ts + 8, slot + 8, 10_000_000, 10_100_000);
    let i = arm(&c, &k);
    assert_ok(c.env.send(&[i], &[&k]));
    let rearmed: Loan = c.env.read(&key(&c));
    assert!(rearmed.cr_arm_sequence > armed.cr_arm_sequence);
    let i = arm(&c, &k);
    assert_ok(c.env.send(&[i], &[&k]));
    let same: Loan = c.env.read(&key(&c));
    assert_eq!(same.cr_arm_timestamp, rearmed.cr_arm_timestamp);
    clock(&mut c, ts + 13, slot + 13);
    publish(&mut c, ts + 13, slot + 13, 10_000_000, 10_100_000);
    for input in [3_000_000, 3_500_000] {
        let i = force(&c, &k, input);
        let (keys, before) = snapshot(&c, &[k.pubkey()]);
        let e = error(c.env.send_v0(&[i], &[&k]).0, "HealthyFill");
        assert!(e
            .meta
            .logs
            .iter()
            .any(|l| l == &format!("Program {} success", jupiter::CPMM_ID)));
        assert_eq!(
            c.snapshot(&keys),
            before,
            "actual q-normalized fill veto rolls back DEX, including overbuy, and rent advance"
        );
    }
    let i = add(&c, 1000);
    assert_ok(c.env.send(&[i], &[&c.borrower]));
    let after: Loan = c.env.read(&key(&c));
    assert_eq!((after.cr_arm_sequence, after.cr_arm_reporter_epoch), (0, 0));
    assert_eq!(after.collateral, same.collateral + 1000);
}

#[test]
fn retiring_one_of_two_loans_preserves_other_debt_and_exposure_rent() {
    let mut c = opened();
    let mut args = c.args();
    args.nonce = 1;
    let i = c.ix(args);
    assert_ok(c.env.send_v0(&[i], &[&c.borrower]).0);
    let address = exposure(c.pool, c.borrower.pubkey());
    let e: Exposure = c.env.read(&address);
    assert_eq!(e.q, 2 * Q);
    let src = source(&mut c, 114);
    let borrower_before = c.env.lamports(&c.borrower.pubkey());
    let refund = c.env.lamports(&key(&c));
    let exposure_rent = c.env.lamports(&address);
    let i = repay(&c, src);
    assert_ok(c.env.send(&[i], &[&c.borrower]));
    let e: Exposure = c.env.read(&address);
    let p: PoolGeneration = c.env.read(&c.pool);
    let remaining: Loan = c.env.read(&loan(c.pool, c.borrower.pubkey(), 1));
    assert_eq!(
        (e.q, e.notional, p.principal_out, p.loan_count),
        (Q, remaining.opening_notional, Q, 1)
    );
    assert_eq!(c.env.lamports(&address), exposure_rent);
    assert_eq!(
        c.env.lamports(&c.borrower.pubkey()) - borrower_before,
        refund
    );
}

#[test]
fn confirmed_bad_cr_executes_full_debt_after_a_synthetic_market_price_rise() {
    let mut c = opened();
    let k = keeper(&mut c);
    let l: Loan = c.env.read(&key(&c));
    // Local market shock: increase actual native reserve and backing lamports by
    // 50%. The route still executes both genuine frozen SBF programs.
    let mut vault = c.env.svm.get_account(&c.route.wsol_vault).unwrap();
    let old = u64::from_le_bytes(vault.data[64..72].try_into().unwrap());
    vault.data[64..72].copy_from_slice(&(old + old / 2).to_le_bytes());
    vault.lamports += old / 2;
    c.env.svm.set_account(c.route.wsol_vault, vault).unwrap();
    let ts = pda::START_TS;
    let slot = jupiter::EXECUTION_SLOT;
    clock(&mut c, ts + 1, slot + 1);
    publish(&mut c, ts + 1, slot + 1, 3_300_000, 3_500_000);
    let i = arm(&c, &k);
    assert_ok(c.env.send(&[i], &[&k]));
    clock(&mut c, ts + 6, slot + 6);
    publish(&mut c, ts + 6, slot + 6, 3_300_000, 3_500_000);
    let before = c.env.lamports(&k.pubkey());
    let input = 3_600_000;
    let policy: Policy = c.env.read(&pda::policy(1));
    let expected = finance::keeper_reward(input, l.collateral - input, &policy.terms).unwrap();
    let i = force(&c, &k, input);
    let meta = assert_ok(c.env.send_v0(&[i], &[&k]).0);
    assert!(meta
        .logs
        .iter()
        .any(|v| v == &format!("Program {} success", jupiter::CPMM_ID)));
    let p: PoolGeneration = c.env.read(&c.pool);
    assert_eq!((p.principal_out, p.loan_count), (0, 0));
    assert!(p.cash >= 10 * Q);
    assert_eq!(c.env.lamports(&k.pubkey()) - before, expected);
    assert!(c.env.svm.get_account(&key(&c)).is_none());
}

#[test]
fn reporter_rotation_invalidates_old_cr_arm_until_a_new_epoch_is_armed() {
    let mut c = opened();
    let k = keeper(&mut c);
    let ts = pda::START_TS;
    let slot = jupiter::EXECUTION_SLOT;
    clock(&mut c, ts + 1, slot + 1);
    publish(&mut c, ts + 1, slot + 1, 10_000_000, 10_100_000);
    let i = arm(&c, &k);
    assert_ok(c.env.send(&[i], &[&k]));
    let first: Loan = c.env.read(&key(&c));
    let next = deterministic_key(115);
    c.env.fund(next.pubkey(), 1_000_000_000);
    let governance = c.env.governance.insecure_clone();
    let a = accounts::RoleAuthority {
        authority: governance.pubkey(),
        config: pda::config(),
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    assert_ok(c.env.send(
        &[ix(
            a,
            instruction::SetReporter {
                reporter: next.pubkey(),
            },
        )],
        &[&governance],
    ));
    c.env.reporter = next;
    clock(&mut c, ts + 6, slot + 6);
    publish(&mut c, ts + 6, slot + 6, 10_000_000, 10_100_000);
    let i = force(&c, &k, 3_000_000);
    error(c.env.send_v0(&[i], &[&k]).0, "ConfirmationPending");
    let i = arm(&c, &k);
    assert_ok(c.env.send(&[i], &[&k]));
    let refreshed: Loan = c.env.read(&key(&c));
    assert_eq!(
        refreshed.cr_arm_reporter_epoch,
        first.cr_arm_reporter_epoch + 1
    );
    assert_eq!(refreshed.cr_arm_timestamp, ts + 6);
    assert!(refreshed.cr_arm_sequence > first.cr_arm_sequence);
    clock(&mut c, ts + 11, slot + 11);
    publish(&mut c, ts + 11, slot + 11, 10_000_000, 10_100_000);
    let i = force(&c, &k, 3_000_000);
    error(c.env.send_v0(&[i], &[&k]).0, "HealthyFill");
}
