//! Current pump curve exits: retain the simulation deployment floor and report actual parser
//! failures directly, without probing retired short.win 1.0.0 account lists.
use anchor_lang::prelude::Pubkey;
use shortwin::errors::ShortwinError;
use shortwin_litesvm_tests::env::program_so_path;
use shortwin_litesvm_tests::fork::PumpCoin;
use shortwin_litesvm_tests::pump::{PumpMarket, PumpSpec};
use shortwin_litesvm_tests::{pda, read};

use crate::common::*;

/// A current-program curve position whose expiry can execute now. Cranks keep the observation
/// chain intact while advancing to its deadline.
fn expired_curve() -> (Harness, PumpMarket, Pubkey) {
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::Near166);
    ok(env.list_pump(&c));
    let q = c.state(&env.svm).virtual_token_reserves / 200;
    let pm = env.pump_market_listed(PumpSpec::new(PumpCoin::Near166, q * 40), c);
    env.crank(&pm);
    env.warp(1);
    let mut borrower = env.borrower();
    let (res, nonce) = env.open(&pm, &mut borrower, q);
    ok(res);
    let position = pda::position(&pm.market(), &borrower.key(), nonce).0;
    let deadline = env.position(&pm, &borrower.key(), nonce).deadline;
    while env.now() < deadline + 1 {
        env.warp((deadline + 1 - env.now()).min(600));
        env.crank(&pm);
    }
    let h = Harness::new(env, covering_all(KeeperConfig::default()));
    h.chain().set_priority_fees(vec![5_000]);
    (h, pm, position)
}

fn deploy_slot(h: &Harness) -> u64 {
    h.with_env(|env| {
        let account = env
            .svm
            .get_account(&pda::program_data())
            .expect("ProgramData");
        shortwin_venue_state::program_data::deploy_slot(&account.owner.to_bytes(), &account.data)
            .expect("deploy slot")
    })
}

/// Re-deploy the same current binary through the real loader. Even an otherwise successful
/// simulation of the same account format is too old when its bank predates the observed deploy.
#[test]
fn a_curve_exit_simulated_before_the_observed_deploy_is_not_sent() {
    let (mut h, _pm, position) = expired_curve();
    let stale = h.with_env(|env| env.clock().slot);
    h.chain().lag_exit_simulations(2);
    let program = std::fs::read(program_so_path()).expect("current release binary");
    let upgraded_at = h.with_env(|env| {
        env.warp(1);
        env.upgrade_shortwin(&program)
    });
    assert_eq!(deploy_slot(&h), upgraded_at);
    assert!(upgraded_at > stale);
    h.with_env(|env| env.warp(1));

    let behind = format!("before short.win's deploy slot {upgraded_at}");
    let mut held = 0;
    let mut landed = false;
    for _ in 0..10 {
        let report = h.second().clone();
        assert!(
            !report
                .alerts
                .iter()
                .any(|a| matches!(a, Alert::ExitRefused { .. })),
            "{:?}",
            report.alerts
        );
        let attempts: Vec<_> = report
            .exits
            .iter()
            .filter(|e| e.position == position)
            .collect();
        match attempts.as_slice() {
            [] => assert!(report.errors.is_empty(), "{:?}", report.errors),
            [attempt] if matches!(&attempt.result, ExitResult::Error(e) if e.contains(&behind)) => {
                assert_eq!(attempt.kind, ExitKind::Expire);
                assert!(report.errors.len() == 1 && report.errors[0].contains(&behind));
                assert!(
                    read::borsh::<shortwin::state::ShortPosition>(&h.env.borrow().svm, &position)
                        .is_some(),
                    "stale simulation must not close the position"
                );
                held += 1;
            }
            [attempt] if attempt.result.landed().is_some() => {
                assert_eq!(attempt.kind, ExitKind::Expire);
                assert!(report.errors.is_empty(), "{:?}", report.errors);
                landed = true;
                break;
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(held, 2, "both stale-bank exit simulations were held");
    assert!(
        landed,
        "current-bank expiry recovers without a format probe"
    );
    assert!(
        read::borsh::<shortwin::state::ShortPosition>(&h.env.borrow().svm, &position).is_none()
    );
    assert!(
        h.chain().sent().iter().all(|sent| sent.ok),
        "no failed send"
    );
}

/// A malformed current list or unexpected CurveStraddle is an ordinary actionable refusal.
/// Neither a legacy-list retry nor the retired wait-for-upgrade suppression may hide it.
#[test]
fn current_curve_parse_refusals_are_reported_and_recover_when_corrected() {
    for error in [
        ShortwinError::BadRemainingAccounts,
        ShortwinError::VenueAccountMismatch,
        ShortwinError::CurveStraddle,
    ] {
        let code = u32::from(error);
        let (mut h, _pm, position) = expired_curve();
        h.chain().refuse_parsed_exit_simulations(Some(code));
        let mut refused = 0;
        for _ in 0..6 {
            let report = h.second().clone();
            assert!(report.errors.is_empty(), "{:?}", report.errors);
            for attempt in report.exits.iter().filter(|e| e.position == position) {
                assert_eq!(attempt.kind, ExitKind::Expire);
                assert!(
                    matches!(&attempt.result, ExitResult::Refused { code: Some(c), .. } if *c == code),
                    "{:?}",
                    attempt.result
                );
                let alerts: Vec<_> = report
                    .alerts
                    .iter()
                    .filter_map(|a| match a {
                        Alert::ExitRefused {
                            position: p,
                            code: Some(c),
                            detail,
                            ..
                        } if *p == position && *c == code => Some(detail),
                        _ => None,
                    })
                    .collect();
                assert_eq!(alerts.len(), 1, "the current refusal is paged");
                assert!(alerts[0].contains("Error Code: Injected"), "{}", alerts[0]);
                refused += 1;
            }
        }
        assert!(refused >= 4, "persistent current refusal must stay visible");
        assert!(
            read::borsh::<shortwin::state::ShortPosition>(&h.env.borrow().svm, &position).is_some()
        );
        h.chain().refuse_exit_simulations(None);
        let mut landed = false;
        for _ in 0..5 {
            let report = h.second().clone();
            assert!(report.errors.is_empty(), "{:?}", report.errors);
            if report
                .exits
                .iter()
                .any(|e| e.position == position && e.result.landed().is_some())
            {
                landed = true;
                break;
            }
        }
        assert!(landed, "the current list recovers after refusal {code}");
        assert!(
            read::borsh::<shortwin::state::ShortPosition>(&h.env.borrow().svm, &position).is_none()
        );
        assert!(
            h.chain().sent().iter().all(|sent| sent.ok),
            "no failed send"
        );
    }
}
