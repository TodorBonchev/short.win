//! Duty 10 on Raydium (ARCH §7.1, §5.5; §9 M3 "admin-action monitoring in keeper v1"): the real
//! admin instructions of the dumped programs, signed as the pool admin `GThUX1…` (signature
//! checks off for that one transaction, `fork::raydium_admin_send`), and upgrades of the dumped
//! ProgramData.
//!
//! - AMM v4 `SetParams` status 2 halts a pool: the keeper reports it at once, starts the S3 chain
//!   the same second, keeps it continuous, settles the expired position in S3, leaves the
//!   healthy one frozen, and reports the resume.
//! - Fees: AMM v4 `SetParams` fees and CPMM `update_amm_config` between and above the caps,
//!   reported against the opens cap and the hard cap; above the hard cap the market runs its S3
//!   chain (`s3_no_route`) while still executable.
//! - CPMM `update_pool_status`: the swap bit is a lost route, the deposit / withdraw bits are
//!   only a status change.
//! - CPMM `update_amm_config(0, 0)` (M3-SEC-1): a zero trade fee on a pool whose creator fee is
//!   on the buy's input is a lost route (every buy reverts with `ZeroTradingTokens`), reported
//!   with why; the S3 chain runs and the healthy position stays open; the fee restored, it
//!   closes.
//! - Upgrades: a changed deploy slot of either Raydium program is flagged with
//!   `flag_venue_upgraded` in the slot the keeper sees it (opens on every Raydium market then
//!   refused), once; a second upgrade during review refreshes the flag; after
//!   `revalidate_venue` nothing is sent; a flag someone else sent first is not repeated;
//!   Token-2022's upgrade is alerted only.

use anchor_lang::prelude::Pubkey;
use shortwin::constants::{GUARD_RAYDIUM_CPMM, GUARD_RAYDIUM_V4, TOKEN_2022_PROGRAM_ID};
use shortwin::errors::ShortwinError;
use shortwin::events::SolSettle;
use shortwin_keeper::events;
use shortwin_keeper::monitor::{self, AdminChange};
use shortwin_litesvm_tests::engine::VenueMarket;
use shortwin_litesvm_tests::errors::custom_code;
use shortwin_litesvm_tests::fork::{self, FIXTURE_GUARD_SLOTS, RaydiumProgram, V4Fees};
use shortwin_litesvm_tests::{fixtures, ix};
use solana_signer::Signer;

use crate::common::*;
use crate::raydium::*;

/// The `VenueAdmin` changes of `market` in `r`.
fn changes(r: &TickReport, market: &Pubkey) -> Vec<AdminChange> {
    r.alerts
        .iter()
        .filter_map(|a| match a {
            Alert::VenueAdmin {
                market: m, change, ..
            } if m == market => Some(change.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_amm_v4_halt_is_reported_the_chain_runs_and_the_expired_position_settles_in_s3() {
    let (mut h, ms) = ray_setup(&[FART], KeeperConfig::default());
    let (rm, q) = &ms[0];
    let key = rm.market();
    let (b1, n1) = ray_open(&h, rm, *q);
    let pos1 = position_of(rm, &b1.key(), n1);
    let deadline = h.with_env(|env| env.position(rm, &b1.key(), n1).deadline);
    h.seconds(2);
    // A second position, opened an hour later: healthy and before its deadline at the end.
    h.idle(3_600);
    h.seconds(2);
    let (b2, n2) = ray_open(&h, rm, q / 3);
    h.seconds(2);
    let wait = deadline - 700 - h.now();
    h.idle(wait);
    h.seconds(5);
    // Fartcoin's pool runs as 6 (SwapOnly), as dumped.
    let status0 = h.with_env(|env| rm.v4().info(&env.svm).status);
    assert_eq!(status0, 6);
    // The pool admin disables swaps: AMM v4 SetParams (param 0) status 2.
    h.with_env(|env| {
        ok(fork::raydium_admin_send(
            env,
            fork::v4_set_params_ix(rm.v4(), 0, 2),
        ))
    });
    let halted_at = h.now();

    let mut since = None;
    let mut last_crank = halted_at;
    let mut settled = None;
    let mut reported = false;
    for s in 0..1_000 {
        let r = h.second().clone();
        let now = h.now();
        assert!(r.errors.is_empty(), "second {s}: {:?}", r.errors);
        let c = changes(&r, &key);
        if s == 0 {
            // Reported at the first tick after the halt: the route and the status.
            assert!(
                matches!(&c[0], AdminChange::RouteLost { detail } if detail.contains("2 Disabled")),
                "{c:?}"
            );
            assert!(
                matches!(
                    c[1],
                    AdminChange::StatusChanged {
                        from: Some(6),
                        to: Some(2),
                        ..
                    }
                ),
                "{c:?}"
            );
            assert_eq!(c.len(), 2, "{c:?}");
            assert!(r.needs[&key].no_route && r.needs[&key].start_chain);
            reported = true;
        } else {
            assert!(c.is_empty(), "second {s}: reported again: {c:?}");
        }
        if r.cranked.contains(&key) {
            assert!(now - last_crank <= 30, "crank gap {} s", now - last_crank);
            last_crank = now;
        }
        let m = h.with_env(|env| env.market(&key));
        if m.no_route_since != 0 {
            assert_eq!(*since.get_or_insert(m.no_route_since), m.no_route_since);
        }
        if let Some(out) = landed_for(&r, ExitKind::SolSettle, &pos1).next() {
            let ev: Vec<SolSettle> = events::of(out);
            if let Some(e) = ev.first() {
                assert_eq!(e.condition, 3, "S3");
                settled = Some(now);
                break;
            }
        }
    }
    assert!(reported);
    let since = since.expect("the S3 chain never started");
    assert_eq!(
        since,
        halted_at + 1,
        "the chain starts at the keeper's next tick"
    );
    let settled = settled.expect("no S3 settlement");
    println!(
        "AMM v4 halted at {halted_at}, chain since {since}, deadline {deadline}, S3 at {settled}"
    );
    assert!(settled >= deadline && settled >= since + 600, "{settled}");
    assert!(settled <= deadline.max(since + 600) + 1, "late: {settled}");
    assert!(!is_open(&h, rm, &b1.key(), n1));
    assert!(
        is_open(&h, rm, &b2.key(), n2),
        "the healthy position is frozen"
    );

    // The admin resumes swaps: reported, the market executes again, the position trades.
    h.with_env(|env| {
        ok(fork::raydium_admin_send(
            env,
            fork::v4_set_params_ix(rm.v4(), 0, 6),
        ))
    });
    let r = h.second().clone();
    let c = changes(&r, &key);
    assert!(
        matches!(&c[0], AdminChange::RouteRestored { detail } if detail.contains("6 SwapOnly")),
        "{c:?}"
    );
    assert!(matches!(
        c[1],
        AdminChange::StatusChanged {
            from: Some(2),
            to: Some(6),
            ..
        }
    ));
    assert!(r.needs.get(&key).is_some_and(|n| !n.no_route));
    h.seconds(2);
    ok(h.with_env(|env| env.close(rm, &b2, n2)));
}

#[test]
fn fee_changes_are_reported_against_the_caps_on_both_programs() {
    let (mut h, ms) = ray_setup(&[FART, USELESS], KeeperConfig::default());
    for (rm, q) in &ms {
        let _ = ray_open(&h, rm, *q);
    }
    let (fart, useless) = (&ms[0].0, &ms[1].0);
    let r = h.second().clone();
    assert!(venue_alerts(&r).is_empty(), "{:?}", r.alerts);
    let caps = monitor::FeeCaps::of(&h.with_env(|env| env.config()));
    assert_eq!((caps.open_ppm, caps.hard_ppm), (15_000, 50_000));
    let v4_fee = h.with_env(|env| fart.v4().info(&env.svm).fee_ppm().unwrap());
    let cpmm_fee = h.with_env(|env| {
        let p = useless.cpmm();
        p.state(&env.svm).fee_ppm(&p.config(&env.svm)).unwrap()
    });
    let set_v4 = |h: &Harness, num: u64| {
        h.with_env(|env| {
            let mut f = V4Fees::of(&env.svm, fart.v4());
            f.swap_fee = (num, 100);
            ok(fork::raydium_admin_send(
                env,
                fork::v4_set_fees_ix(fart.v4(), &f),
            ));
        })
    };
    let config = h.with_env(|env| useless.cpmm().config_key(&env.svm));
    let trade0 = h.with_env(|env| useless.cpmm().config(&env.svm).trade_fee_rate);
    let set_cpmm = |h: &Harness, trade_ppm: u64| {
        h.with_env(|env| {
            ok(fork::raydium_admin_send(
                env,
                fork::cpmm_update_amm_config_ix(&config, 0, trade_ppm),
            ));
        })
    };

    // AMM v4 SetParams fees: 3 % (between the caps): opens refused, no S3 evidence.
    set_v4(&h, 3);
    let r = h.second().clone();
    assert_eq!(
        changes(&r, &fart.market()),
        vec![AdminChange::FeeChanged {
            from_ppm: Some(v4_fee),
            to_ppm: 30_000,
            above_open_cap: true,
            above_hard_cap: false,
            via: "AMM v4 SetParams (param 2, fees)",
        }]
    );
    assert!(!r.needs[&fart.market()].no_route);
    let mut b = h.with_env(|env| env.borrower());
    let res = h.with_env(|env| env.open(fart, &mut b, ms[0].1 / 2).0);
    assert_eq!(
        custom_code(&res),
        Some(u32::from(ShortwinError::VenueFeeTooHigh))
    );

    // CPMM update_amm_config trade fee 6 % (above the hard cap), for every pool on the config:
    // s3_no_route while the venue still executes; the keeper starts the S3 chain at once.
    set_cpmm(&h, 60_000);
    let r = h.second().clone();
    let c = changes(&r, &useless.market());
    let AdminChange::FeeChanged {
        from_ppm,
        to_ppm,
        above_open_cap,
        above_hard_cap,
        via,
    } = &c[0]
    else {
        panic!("{c:?}");
    };
    assert_eq!(*from_ppm, Some(cpmm_fee));
    assert!(
        *to_ppm >= 60_000 && *above_open_cap && *above_hard_cap,
        "{c:?}"
    );
    assert!(via.contains("update_amm_config"));
    let needs = r.needs[&useless.market()];
    assert!(needs.no_route && needs.start_chain, "{needs:?}");
    let m = h.with_env(|env| env.market(&useless.market()));
    assert_eq!(m.no_route_since, h.now(), "observed the same second");

    // Both fees restored: reported, below both caps, the route back.
    h.with_env(|env| {
        let mut f = V4Fees::of(&env.svm, fart.v4());
        f.swap_fee = (25, 10_000);
        ok(fork::raydium_admin_send(
            env,
            fork::v4_set_fees_ix(fart.v4(), &f),
        ));
    });
    assert_eq!(v4_fee, 2_500, "the dumped pool's 25/10,000");
    set_cpmm(&h, trade0);
    let r = h.second().clone();
    for (key, want) in [(fart.market(), v4_fee), (useless.market(), cpmm_fee)] {
        let c = changes(&r, &key);
        assert!(
            matches!(
                c.as_slice(),
                [AdminChange::FeeChanged {
                    to_ppm,
                    above_open_cap: false,
                    above_hard_cap: false,
                    ..
                }] if *to_ppm == want
            ),
            "{c:?}"
        );
        assert!(!r.needs[&key].no_route);
    }
}

/// M3-SEC-1: CPMM `update_amm_config(0, 0)` sets config 0's trade fee to 0. USELESS has no
/// creator fee enabled and `creator_fee_on = 0`, so every buy reverts in the venue
/// (`ZeroTradingTokens`) although the status is clear: the keeper reports a lost route (with
/// why) besides the fee change, the S3 chain starts at its next tick and stays continuous, and
/// the healthy position stays open past `s3_wait_s` (no S1 at an unchanged price, no S3 while
/// healthy). With the fee back, the route is restored and the position closes.
#[test]
fn a_zero_cpmm_trade_fee_is_a_lost_route_and_healthy_positions_stay_open() {
    let (mut h, ms) = ray_setup(&[USELESS], KeeperConfig::default());
    let (rm, q) = &ms[0];
    let key = rm.market();
    let (b, n) = ray_open(&h, rm, *q);
    let pos = position_of(rm, &b.key(), n);
    h.seconds(3);
    let config = h.with_env(|env| rm.cpmm().config_key(&env.svm));
    let trade0 = h.with_env(|env| rm.cpmm().config(&env.svm).trade_fee_rate);
    let fee0 = h.with_env(|env| {
        let p = rm.cpmm();
        p.state(&env.svm).fee_ppm(&p.config(&env.svm)).unwrap()
    });
    h.with_env(|env| {
        ok(fork::raydium_admin_send(
            env,
            fork::cpmm_update_amm_config_ix(&config, 0, 0),
        ))
    });
    let at = h.now();
    let wait = h.with_env(|env| env.position(rm, &b.key(), n).s3_wait_s);
    let mut last_crank = at;
    for s in 0..(i64::from(wait) + 120) {
        let r = h.second().clone();
        let now = h.now();
        assert!(r.errors.is_empty(), "second {s}: {:?}", r.errors);
        let c = changes(&r, &key);
        if s == 0 {
            assert!(
                matches!(&c[0], AdminChange::RouteLost { detail }
                    if detail.contains("ZeroTradingTokens") && detail.contains("swap bit clear")),
                "{c:?}"
            );
            assert_eq!(
                c[1],
                AdminChange::FeeChanged {
                    from_ppm: Some(fee0),
                    to_ppm: 0,
                    above_open_cap: false,
                    above_hard_cap: false,
                    via: "CPMM update_amm_config (every pool on the config)",
                }
            );
            assert_eq!(c.len(), 2, "{c:?}");
            assert!(c[0].is_severe());
            assert!(r.needs[&key].no_route && r.needs[&key].start_chain);
            assert_eq!(
                h.with_env(|env| env.market(&key)).no_route_since,
                now,
                "the chain starts at the keeper's first tick"
            );
        } else {
            assert!(c.is_empty(), "second {s}: reported again: {c:?}");
        }
        if r.cranked.contains(&key) {
            assert!(now - last_crank <= 30, "crank gap {} s", now - last_crank);
            last_crank = now;
        }
        let m = h.with_env(|env| env.market(&key));
        assert_eq!(m.no_route_since, at + 1, "second {s}: one continuous chain");
        for out in landed_for(&r, ExitKind::SolSettle, &pos) {
            let ev: Vec<SolSettle> = events::of(out);
            assert!(
                ev.is_empty(),
                "second {s}: a healthy position settled (S{:?})",
                ev.first().map(|e| e.condition)
            );
        }
        assert!(is_open(&h, rm, &b.key(), n), "second {s}");
    }
    assert!(h.now() >= at + i64::from(wait) + 60);
    // The fee restored: the route is back, the position closes.
    h.with_env(|env| {
        ok(fork::raydium_admin_send(
            env,
            fork::cpmm_update_amm_config_ix(&config, 0, trade0),
        ))
    });
    let r = h.second().clone();
    let c = changes(&r, &key);
    assert!(matches!(c[0], AdminChange::RouteRestored { .. }), "{c:?}");
    assert!(r.needs.get(&key).is_some_and(|n| !n.no_route));
    h.seconds(2);
    ok(h.with_env(|env| env.close(rm, &b, n)));
}

#[test]
fn cpmm_status_bits_swap_is_a_lost_route_deposit_is_only_a_status_change() {
    let (mut h, ms) = ray_setup(&[USELESS], KeeperConfig::default());
    let (rm, q) = &ms[0];
    let key = rm.market();
    let _ = ray_open(&h, rm, *q);
    h.seconds(2);
    let status = |h: &Harness, s: u8| {
        h.with_env(|env| {
            ok(fork::raydium_admin_send(
                env,
                fork::cpmm_update_pool_status_ix(rm.cpmm(), s),
            ))
        })
    };
    status(&h, 4);
    let r = h.second().clone();
    let c = changes(&r, &key);
    assert!(
        matches!(&c[0], AdminChange::RouteLost { detail } if detail.contains("swaps disabled")),
        "{c:?}"
    );
    assert!(matches!(
        c[1],
        AdminChange::StatusChanged {
            from: Some(0),
            to: Some(4),
            via: "CPMM update_pool_status",
        }
    ));
    assert!(r.observed(&key), "the chain starts at once");
    assert_eq!(h.with_env(|env| env.market(&key)).no_route_since, h.now());
    // Deposit disabled (bit 0) only: swaps work again.
    status(&h, 1);
    let r = h.second().clone();
    let c = changes(&r, &key);
    assert!(matches!(c[0], AdminChange::RouteRestored { .. }), "{c:?}");
    assert!(matches!(
        c[1],
        AdminChange::StatusChanged {
            from: Some(4),
            to: Some(1),
            ..
        }
    ));
    status(&h, 0);
    let r = h.second().clone();
    assert_eq!(
        changes(&r, &key),
        vec![AdminChange::StatusChanged {
            from: Some(1),
            to: Some(0),
            via: "CPMM update_pool_status",
        }]
    );
}

#[test]
fn upgrades_are_flagged_in_the_slot_they_are_seen_once_and_refreshed_during_review() {
    let (mut h, ms) = ray_setup(&[FART, USELESS], KeeperConfig::default());
    let v4_slot = FIXTURE_GUARD_SLOTS[usize::from(GUARD_RAYDIUM_V4)];
    let cpmm_slot = FIXTURE_GUARD_SLOTS[usize::from(GUARD_RAYDIUM_CPMM)];
    // Token-2022's ProgramData (LiteSVM loads Token-2022 without one): watched, alert only.
    let t22_pd = monitor::program_data_address(&TOKEN_2022_PROGRAM_ID);
    h.with_env(|env| fixtures::set_program_data(&mut env.svm, &t22_pd, 400_000_000, None, 64));
    let r = h.slot().clone();
    assert!(r.flagged.is_empty() && venue_alerts(&r).is_empty(), "{r:?}");

    // An AMM v4 upgrade: flagged in the keeper's next tick (the next slot).
    h.with_env(|env| fork::set_deploy_slot(&mut env.svm, RaydiumProgram::AmmV4, v4_slot + 777));
    let r = h.slot().clone();
    assert_eq!(
        r.flagged,
        vec![(GUARD_RAYDIUM_V4, v4_slot + 777)],
        "{:?} {:?}",
        r.alerts,
        r.errors
    );
    let a = venue_alerts(&r);
    assert!(
        a.contains(&Alert::VenueUpgraded {
            venue: GUARD_RAYDIUM_V4,
            program: shortwin::constants::RAYDIUM_V4_PROGRAM_ID,
            known_slot: v4_slot,
            deploy_slot: v4_slot + 777,
        }),
        "{a:?}"
    );
    assert!(a.contains(&Alert::VenueUpgradeFlagged {
        venue: GUARD_RAYDIUM_V4,
        deploy_slot: v4_slot + 777
    }));
    let g = h.with_env(|env| env.venue_guard());
    assert!(g.is_flagged(GUARD_RAYDIUM_V4));
    assert_eq!(g.flagged_slot[usize::from(GUARD_RAYDIUM_V4)], v4_slot + 777);
    assert_eq!(h.keeper.metrics.venue_flags, 1);
    // Opens on both Raydium venues are refused (one mask); exits are not.
    for (rm, q) in &ms {
        let mut b = h.with_env(|env| env.borrower());
        let res = h.with_env(|env| env.open(rm, &mut b, *q).0);
        assert_eq!(
            custom_code(&res),
            Some(u32::from(ShortwinError::VenueUpgraded)),
            "{:?}",
            rm.fixture
        );
    }
    // Nothing more while the slot stands: no second flag, no second alert.
    let sent = h.chain().sent().len();
    for _ in 0..3 {
        let r = h.slot().clone();
        assert!(r.flagged.is_empty() && venue_alerts(&r).is_empty(), "{r:?}");
    }
    assert_eq!(h.chain().sent().len(), sent, "no transaction");

    // A second upgrade during review refreshes the flag.
    h.with_env(|env| fork::set_deploy_slot(&mut env.svm, RaydiumProgram::AmmV4, v4_slot + 999));
    let r = h.slot().clone();
    assert_eq!(r.flagged, vec![(GUARD_RAYDIUM_V4, v4_slot + 999)]);
    assert!(venue_alerts(&r).contains(&Alert::VenueUpgraded {
        venue: GUARD_RAYDIUM_V4,
        program: shortwin::constants::RAYDIUM_V4_PROGRAM_ID,
        known_slot: v4_slot + 777,
        deploy_slot: v4_slot + 999,
    }));
    let g = h.with_env(|env| env.venue_guard());
    assert_eq!(g.flagged_slot[usize::from(GUARD_RAYDIUM_V4)], v4_slot + 999);

    // The Emergency multisig revalidates the reviewed slot: nothing to flag afterwards.
    h.with_env(|env| {
        let ea = env.pause_authority.insecure_clone();
        ok(env.send(
            &[ix::revalidate_venue(
                &ea.pubkey(),
                GUARD_RAYDIUM_V4,
                v4_slot + 999,
            )],
            &[&ea],
        ))
    });
    let r = h.slot().clone();
    assert!(r.flagged.is_empty() && venue_alerts(&r).is_empty(), "{r:?}");
    assert!(
        !h.with_env(|env| env.venue_guard())
            .is_flagged(GUARD_RAYDIUM_V4)
    );
    let (rm, q) = &ms[1];
    let _ = ray_open(&h, rm, *q);

    // A CPMM upgrade someone else flags first: the keeper sees the latch already set.
    h.with_env(|env| {
        fork::set_deploy_slot(&mut env.svm, RaydiumProgram::Cpmm, cpmm_slot + 5);
        ok(env.send(&[ix::flag_venue_upgraded(GUARD_RAYDIUM_CPMM)], &[]));
    });
    let r = h.slot().clone();
    assert!(r.flagged.is_empty(), "{r:?}");
    // The latch already holds this slot: no upgrade to report, no attempt, no failure.
    assert!(venue_alerts(&r).is_empty(), "{:?}", r.alerts);
    assert_eq!(h.keeper.metrics.venue_flags, 2);

    // Token-2022 upgraded: an alert, nothing sent for it.
    h.with_env(|env| fixtures::set_program_data(&mut env.svm, &t22_pd, 400_000_001, None, 64));
    let r = h.slot().clone();
    assert!(
        venue_alerts(&r).contains(&Alert::ProgramUpgraded {
            program: TOKEN_2022_PROGRAM_ID,
            previous_slot: 400_000_000,
            deploy_slot: 400_000_001,
        }),
        "{:?}",
        r.alerts
    );
    assert!(r.flagged.is_empty());
}

#[test]
fn with_the_watch_off_nothing_is_flagged_or_reported() {
    let mut cfg = KeeperConfig::default();
    cfg.venue_watch.flag_upgrades = false;
    cfg.venue_watch.pool_admin = false;
    let (mut h, ms) = ray_setup(&[FART], cfg);
    let (rm, q) = &ms[0];
    let _ = ray_open(&h, rm, *q);
    h.second();
    h.with_env(|env| {
        fork::set_deploy_slot(&mut env.svm, RaydiumProgram::AmmV4, 1);
        ok(fork::raydium_admin_send(
            env,
            fork::v4_set_params_ix(rm.v4(), 0, 2),
        ));
    });
    let r = h.second().clone();
    assert!(r.flagged.is_empty() && venue_alerts(&r).is_empty(), "{r:?}");
    assert!(
        !h.with_env(|env| env.venue_guard())
            .is_flagged(GUARD_RAYDIUM_V4)
    );
    // The duties still see the halt through the program's own route predicate.
    assert!(r.needs[&rm.market()].no_route);
}
