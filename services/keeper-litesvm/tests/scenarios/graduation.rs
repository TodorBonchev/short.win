//! Keeper duty 6 on the pump curve (M5; ARCH §5.1, §5.4, §7.1 duty 6; ADOPTED Q14, Q16; A66):
//! the **release** short.win, the **dumped** pump, pump-fees and PumpSwap ELFs and the
//! `complete-curve` helper's own SBF build on the mainnet fork of the program tests
//! (`shortwin_litesvm_tests::pump`), driven by the keeper. Every keeper transaction — each
//! transaction of a bundle too — runs the invariant hooks after every instruction.
//!
//! - **`migrate_v2` as soon as a curve completes**: an outside buy completes the curve of a
//!   covered market; at its next tick the keeper migrates it (its account list is the harness's
//!   own derivation's) and observes the market in the same second (M5: `→ PumpMigrating`), once.
//! - **The straddle, M5 default**: an expired position with `Q > rT` on a live curve is
//!   `Plan::Straddle`; there is no exit transaction before M6 and `send_without_exit` is off, so
//!   the keeper sends no bundle, alerts once, and calls `sol_settle` for the position each second.
//! - **The straddle bundle** (`send_without_exit`): tx1 (the keeper's base ATA, `complete_curve`)
//!   and tx2 (`migrate_v2`, the Jito tip) land atomically; the keeper holds the curve's last
//!   tokens, the market records the migration in the same second, and its exits pause
//!   (`VenueMigrating`) — the exit itself is M6's tx3. Trace, CU and size of each transaction.
//! - ★ **A curve trade between the keeper's read and its bundle**: a buy lands after the keeper's
//!   pre-flight — the bundle still lands (the helper reads `rT` at execution); a large sale
//!   pushes the completion above the keeper's `max_sol_cost` — pump reverts tx1, nothing of the
//!   bundle is committed, and the keeper's bundle of the next second lands.
//! - **No block engine**: the bundle cannot go out; the keeper alerts and settles meanwhile.
//! - **Below the straddle** (`Q ≤ rT`): the keeper expires a curve position on the curve with the
//!   adapter's own account lists (the live recipients, the live creator's vault).
//! - **Graduated elsewhere** (duty 5 on a list-only market, M2-SEC-5): a curve migrated elsewhere
//!   with an expired position open goes `→ None` at the keeper's first crank; the keeper keeps
//!   the S3 chain on the no-route cadence with the detection set as ReadSet and settles the
//!   position in S3 once the chain has run `s3_wait_s`.
//! - **`None → PumpCurve` re-detection**: a list-only market whose detection set shows a live
//!   curve is cranked onto it, and the keeper then creates the trader's base ATA and pump UVA the
//!   re-detection does not create, once; an open lands on the curve.

use anchor_lang::prelude::Pubkey;
use shortwin::errors::ShortwinError;
use shortwin::state::VenueKind;
use shortwin_keeper::config::GraduationConfig;
use shortwin_keeper::decide::Plan;
use shortwin_keeper::graduation::{self, StraddleResult};
use shortwin_keeper_litesvm::load_complete_curve;
use shortwin_litesvm_tests::engine::{Borrower, VenueMarket};
use shortwin_litesvm_tests::errors::assert_err;
use shortwin_litesvm_tests::fork::{self, PumpCoin, PumpCurve};
use shortwin_litesvm_tests::pump::{self, CurveUser, PumpMarket, PumpSpec};
use shortwin_litesvm_tests::{pda, read};
use shortwin_services_common::AccountData;

use crate::common::*;

/// A funded Jito tip account for every one the keeper may pay (a tip below the rent-exempt
/// minimum cannot create one).
fn fund_tip_accounts(env: &mut Env) {
    for k in graduation::JITO_TIP_ACCOUNTS {
        env.fund(&k.parse().unwrap(), 1_000_000_000);
    }
}

/// The fork on the near-complete curve with the helper, a listed market with `40·q` lent
/// (`q = vT/200`), and a keeper with `cfg`.
fn pump_setup(cfg: KeeperConfig) -> (Harness, PumpMarket, u64) {
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::Near);
    load_complete_curve(&mut env);
    fund_tip_accounts(&mut env);
    ok(env.list_pump(&c));
    let q = c.state(&env.svm).virtual_token_reserves / 200;
    let pm = env.pump_market_listed(PumpSpec::new(PumpCoin::Near, q * 40), c);
    let h = Harness::new(env, cfg);
    // A priority price: every v0 transaction carries both ComputeBudget instructions (ARCH §5.4's
    // "ComputeBudget ×2").
    h.chain().set_priority_fees(vec![5_000]);
    (h, pm, q)
}

/// A keeper configuration with the `migrate_v2` duty on (ADOPTED Q16; M6's default — suspended
/// in M5 by M5-SEC-4).
fn with_migrate() -> KeeperConfig {
    KeeperConfig {
        graduation: GraduationConfig {
            migrate: true,
            ..GraduationConfig::default()
        },
        ..KeeperConfig::default()
    }
}

/// A keeper configuration that sends the two-transaction bundle (ARCH §5.4's fallback).
fn without_exit() -> KeeperConfig {
    KeeperConfig {
        graduation: GraduationConfig {
            send_without_exit: true,
            ..GraduationConfig::default()
        },
        ..KeeperConfig::default()
    }
}

/// An outside user buys until `rt_left` real tokens remain.
fn buy_down_to(env: &mut Env, c: &PumpCurve, rt_left: u64) {
    let u = CurveUser::new(env, c);
    let rt = c.state(&env.svm).real_token_reserves;
    ok(u.buy(env, c, rt - rt_left, u64::MAX));
    assert_eq!(c.state(&env.svm).real_token_reserves, rt_left);
}

/// Opens `q` for a new borrower, pushes the curve to `rT = q/2` (a straddle: `q > rT`) and warps
/// past the position's deadline (expired: liquidatable).
fn straddled_position(h: &Harness, pm: &PumpMarket, q: u64) -> (Borrower, u64) {
    h.with_env(|env| {
        env.crank(pm);
        env.warp(1);
        let mut b = env.borrower();
        let (res, nonce) = env.open(pm, &mut b, q);
        ok(res);
        env.warp(2);
        buy_down_to(env, &pm.curve, q / 2);
        let deadline = env.position(pm, &b.key(), nonce).deadline;
        let now = env.now();
        env.warp(deadline - now + 1);
        (b, nonce)
    })
}

fn market_kind(h: &Harness, pm: &PumpMarket) -> Option<VenueKind> {
    h.with_env(|env| env.market(&pm.market()).venue_kind())
}

fn curve(h: &Harness, pm: &PumpMarket) -> shortwin_venue_state::pump::BondingCurve {
    h.with_env(|env| pm.curve.state(&env.svm))
}

fn tokens(h: &Harness, key: &Pubkey) -> u64 {
    h.with_env(|env| read::token_amount(&env.svm, key).unwrap_or(0))
}

/// The ReadSet accounts of `pm` as the VM holds them (what the keeper reads).
fn read_accounts(h: &Harness, pm: &PumpMarket) -> Vec<Option<AccountData>> {
    h.with_env(|env| {
        pm.read_set(false)
            .iter()
            .map(|a| {
                env.svm.get_account(&a.pubkey).map(|acc| AccountData {
                    lamports: acc.lamports,
                    owner: acc.owner,
                    executable: acc.executable,
                    data: acc.data,
                })
            })
            .collect()
    })
}

/// The keeper's base ATA of the market's mint.
fn keeper_ata(h: &Harness, pm: &PumpMarket) -> Pubkey {
    pda::ata(&h.keeper.payer(), &pm.keys.mint, &pm.keys.token_program)
}

/// Duty 6 (ADOPTED Q16), with `graduation.migrate` on (M6's default; M5 suspends it,
/// M5-SEC-4): an outside buy completes a covered market's curve; at its next tick the keeper
/// sends `migrate_v2` — top-level, alone, with exactly the accounts the harness derives
/// (`pump::migrate_v2_ix`: the 27 IDL accounts and the two boost accounts) — the canonical pool
/// appears, and the market is observed in the same second (M5: `PumpCurve → PumpMigrating`).
/// Once: the next ticks send nothing more.
#[test]
fn the_keeper_migrates_a_completed_curve() {
    let (mut h, pm, _q) = pump_setup(with_migrate());
    h.second();
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpCurve));
    h.with_env(|env| buy_down_to(env, &pm.curve, 0));
    let c = curve(&h, &pm);
    assert!(c.complete && c.virtual_token_reserves > 0, "migrating");

    // The keeper's instruction is the harness's own derivation.
    let payer = h.keeper.payer();
    let market = h.with_env(|env| env.market(&pm.market()));
    let mine = graduation::migrate_v2_ix(&payer, &market, &read_accounts(&h, &pm)).unwrap();
    let theirs = h.with_env(|env| pump::migrate_v2_ix(&env.svm, &payer, &pm.curve));
    assert_eq!(mine.program_id, theirs.program_id);
    assert_eq!(mine.data, theirs.data);
    assert_eq!(mine.accounts, theirs.accounts);

    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.migrations.len(), 1, "{:?}", r.migrations);
    let out = r.migrations[0].result.landed().expect("migrate_v2 landed");
    let pump_top: Vec<_> = out.inner.iter().map(Vec::len).collect();
    eprintln!("migrate_v2: {} CU, inner {pump_top:?}", out.units_consumed);
    assert_eq!(curve(&h, &pm).virtual_token_reserves, 0, "migrated");
    let np = pump::NewPool::of(&pm.curve);
    assert!(
        h.with_env(|env| np.state(&env.svm)).is_some(),
        "the canonical pool"
    );
    assert!(
        r.cranked.contains(&pm.market()),
        "observed in the same second"
    );
    assert_eq!(
        market_kind(&h, &pm),
        Some(VenueKind::PumpMigrating),
        "M5 records the migration (M6: → PumpSwap)"
    );
    for _ in 0..3 {
        let r = h.second().clone();
        assert!(r.migrations.is_empty(), "once");
        assert!(r.errors.is_empty(), "{:?}", r.errors);
    }
}

/// M5-SEC-4: M5's default keeper leaves a completed curve unmigrated. Without M6's `→ PumpSwap`
/// transition a migrated curve's market is no longer observed (its EMA freezes at the last curve
/// price and its exits pause for good), while the completed, unmigrated curve is still observed
/// and its stuck-migration S3 prices the post-migration pool: the keeper sends no `migrate_v2`.
#[test]
fn m5_sec_4_the_keeper_does_not_migrate_by_default() {
    let (mut h, pm, _q) = pump_setup(KeeperConfig::default());
    assert!(!KeeperConfig::default().graduation.migrate);
    h.second();
    h.with_env(|env| buy_down_to(env, &pm.curve, 0));
    for _ in 0..3 {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.migrations.is_empty(), "no migrate_v2 in M5");
    }
    let c = curve(&h, &pm);
    assert!(
        c.complete && c.virtual_token_reserves > 0,
        "still migrating"
    );
}

/// M5's default (ARCH §5.4, keeper fallback with no exit transaction before M6): an expired
/// position with `Q > rT` on a live curve is `Plan::Straddle`; the keeper sends no bundle — tx3
/// (the exit on the new pool) is M6's and `send_without_exit` is off —, alerts once, and calls
/// `sol_settle` for the position (the tree's last branch) every second meanwhile. The curve is
/// left live.
#[test]
fn m5_waits_for_the_exit_transaction_and_settles_meanwhile() {
    let (mut h, pm, q) = pump_setup(KeeperConfig::default());
    let (b, nonce) = straddled_position(&h, &pm, q);
    let position = pda::position(&pm.market(), &b.key(), nonce).0;
    let mut alerts = 0;
    for s in 0..3 {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let ev = r.evaluations[&pm.market()]
            .iter()
            .find(|e| e.key == position)
            .unwrap();
        assert_eq!(ev.plan, Plan::Straddle, "second {s}: {ev:?}");
        assert!(ev.straddle && ev.expired);
        assert_eq!(r.straddles.len(), 1);
        let StraddleResult::Waiting { reason } = &r.straddles[0].result else {
            panic!("{:?}", r.straddles[0].result);
        };
        assert!(reason.contains("M6"), "{reason}");
        assert_eq!(r.straddles[0].rt_read, q / 2);
        assert!(
            r.exits.iter().any(|e| e.position == position
                && e.kind == ExitKind::SolSettle
                && e.plan == Plan::Straddle
                && e.result.landed().is_some()),
            "sol_settle meanwhile: {:?}",
            r.exits
        );
        alerts += r
            .alerts
            .iter()
            .filter(|a| matches!(a, Alert::StraddleWaiting { position: p, .. } if *p == position))
            .count();
    }
    assert_eq!(alerts, 1, "alerted once");
    assert!(h.chain().bundles_sent().is_empty());
    let c = curve(&h, &pm);
    assert!(
        !c.complete && c.real_token_reserves == q / 2,
        "the curve is untouched"
    );
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpCurve));
}

/// With `send_without_exit` (ARCH §5.4's two-transaction fallback): the keeper's bundle — tx1
/// (ComputeBudget ×2, its base ATA, `complete_curve`), tx2 (ComputeBudget ×2, `migrate_v2`, the
/// Jito tip) — lands atomically: the keeper holds the curve's last `rT` tokens and paid the tip,
/// the curve is complete and migrated, the market is observed in the same second (M5:
/// `PumpMigrating`) and the position's exits pause (`VenueMigrating`: its exit is M6's tx3).
/// The per-transaction trace, CU and size: tx2 is 2 + 51 + 1 = 54 entries.
#[test]
fn the_bundle_completes_and_migrates_the_curve() {
    let (mut h, pm, q) = pump_setup(without_exit());
    let (b, nonce) = straddled_position(&h, &pm, q);
    let tip0: u64 = graduation::JITO_TIP_ACCOUNTS
        .iter()
        .map(|k| h.with_env(|env| read::lamports(&env.svm, &k.parse().unwrap())))
        .sum();
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.straddles.len(), 1);
    let rec = &r.straddles[0];
    assert!(
        matches!(
            rec.result,
            StraddleResult::Landed {
                with_exit: false,
                ..
            }
        ),
        "{:?}",
        rec.result
    );
    assert_eq!(rec.rt_read, q / 2);
    assert!(rec.max_sol_cost > rec.quote);
    let c = curve(&h, &pm);
    assert!(
        c.complete && c.virtual_token_reserves == 0,
        "completed and migrated"
    );
    assert_eq!(
        tokens(&h, &keeper_ata(&h, &pm)),
        q / 2,
        "the keeper holds rT"
    );
    let tip1: u64 = graduation::JITO_TIP_ACCOUNTS
        .iter()
        .map(|k| h.with_env(|env| read::lamports(&env.svm, &k.parse().unwrap())))
        .sum();
    assert_eq!(tip1 - tip0, GraduationConfig::default().tip_lamports);
    assert!(
        r.cranked.contains(&pm.market()),
        "observed in the same second"
    );
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpMigrating));
    h.with_env(|env| assert_err(&env.close(&pm, &b, nonce), ShortwinError::VenueMigrating));

    let sent = h.chain().bundles_sent();
    assert_eq!(sent.len(), 1);
    let bundle = &sent[0];
    assert!(bundle.landed);
    eprintln!("| tx | top-level | trace | CU | bytes | accounts |");
    for (i, t) in bundle.txs.iter().enumerate() {
        eprintln!(
            "| tx{} | {} | {} | {} | {} | {} |",
            i + 1,
            bundle.top[i],
            bundle.trace[i],
            bundle.units[i],
            t.bytes,
            t.accounts
        );
        assert!(t.bytes <= 1_232 && t.accounts <= 64, "v0, T-SIZE: {t:?}");
        assert!(bundle.trace[i] <= 64);
    }
    // tx1: ComputeBudget ×2, the keeper's ATA (created: 1 + 4 CPIs), complete_curve (1 + the
    // completing buy_v2's 9, + 1 when pump creates the keeper's UVA).
    assert_eq!(bundle.top[0], 4);
    assert!(bundle.trace[0] <= 4 + 4 + 9 + 1, "{:?}", bundle.trace);
    // tx2: ComputeBudget ×2, migrate_v2 (1 + 50 CPIs), the tip.
    assert_eq!(bundle.top[1], 4);
    assert_eq!(
        bundle.trace[1],
        2 + 51 + 1,
        "migrate_v2 alone with the budget and the tip"
    );
    // The next seconds: nothing more to complete or migrate.
    for _ in 0..3 {
        let r = h.second().clone();
        assert!(r.straddles.is_empty() && r.migrations.is_empty());
        assert!(r.errors.is_empty(), "{:?}", r.errors);
    }
}

/// ★ T-STRADDLE at the keeper: a curve trade lands between the keeper's read (and pre-flight)
/// and its bundle. A buy of a third of `rT`: the bundle still lands — `complete_curve` buys the
/// `rT` live when it runs, which is what the keeper ends up holding. A sale large enough to lift
/// the completion above the keeper's `max_sol_cost`: pump reverts tx1 (6002), the bundle commits
/// nothing (the curve is as the sale left it, the keeper holds nothing), the keeper alerts, and its
/// bundle of the next second — built on a fresh read — lands.
#[test]
fn a_curve_trade_between_the_keepers_read_and_its_bundle() {
    // A buy.
    let (mut h, pm, q) = pump_setup(without_exit());
    straddled_position(&h, &pm, q);
    let c = pm.curve;
    h.chain().before_next_bundle(move |env| {
        let u = CurveUser::new(env, &c);
        ok(u.buy(env, &c, q / 6, u64::MAX));
    });
    let r = h.second().clone();
    assert!(
        r.straddles[0].result.landed(),
        "{:?}",
        r.straddles[0].result
    );
    assert_eq!(r.straddles[0].rt_read, q / 2, "read before the buy");
    assert_eq!(
        tokens(&h, &keeper_ata(&h, &pm)),
        q / 2 - q / 6,
        "bought the rT live at execution"
    );
    assert_eq!(curve(&h, &pm).virtual_token_reserves, 0);

    // A sale beyond the bound (and still a straddle: rT = 3q/4 < q).
    let (mut h, pm, q) = pump_setup(without_exit());
    straddled_position(&h, &pm, q);
    let c = pm.curve;
    let sale = q / 4;
    h.chain().before_next_bundle(move |env| {
        let u = CurveUser::new(env, &c);
        ok(u.sell(env, &c, sale, 0));
    });
    let r = h.second().clone();
    let StraddleResult::Failed { outcome } = &r.straddles[0].result else {
        panic!("{:?}", r.straddles[0].result);
    };
    assert!(!outcome.landed);
    assert_eq!(
        outcome.txs[0].err.as_ref().and_then(|e| e.custom_code()),
        Some(fork::PUMP_ERR_TOO_MUCH_SOL_REQUIRED),
        "{outcome:?}"
    );
    let after_sale = curve(&h, &pm);
    assert!(!after_sale.complete);
    assert_eq!(
        after_sale.real_token_reserves,
        q / 2 + sale,
        "only the sale landed"
    );
    assert_eq!(tokens(&h, &keeper_ata(&h, &pm)), 0, "nothing of the bundle");
    assert!(r.alerts.iter().any(
        |a| matches!(a, Alert::StraddleWaiting { reason, .. } if reason.contains("did not land"))
    ));
    let r = h.second().clone();
    assert!(
        r.straddles[0].result.landed(),
        "{:?}",
        r.straddles[0].result
    );
    assert_eq!(r.straddles[0].rt_read, q / 2 + sale, "a fresh read");
    assert_eq!(tokens(&h, &keeper_ata(&h, &pm)), q / 2 + sale);
    assert_eq!(curve(&h, &pm).virtual_token_reserves, 0);
}

/// Below the straddle (`Q ≤ rT`), a curve position exits **on the curve**: the keeper's venue
/// lists for a `PumpCurve` exit (the adapter's ReadSet, TradeExtras T0–T19 with the live
/// recipients and creator vault) are the ones the program key-checks, and an expired position is
/// expired at its deadline (`buy_v2` by the trader PDA), with no straddle and no migration.
#[test]
fn below_rt_the_keeper_expires_on_the_curve() {
    let (mut h, pm, q) = pump_setup(KeeperConfig::default());
    let (b, nonce) = h.with_env(|env| {
        env.crank(&pm);
        env.warp(1);
        let mut b = env.borrower();
        let (res, nonce) = env.open(&pm, &mut b, q);
        ok(res);
        let deadline = env.position(&pm, &b.key(), nonce).deadline;
        let now = env.now();
        env.warp(deadline - now + 1);
        (b, nonce)
    });
    let position = pda::position(&pm.market(), &b.key(), nonce).0;
    assert!(u128::from(q) <= u128::from(curve(&h, &pm).real_token_reserves));
    let mut expired = false;
    for _ in 0..5 {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.straddles.is_empty() && r.migrations.is_empty());
        if r.exits.iter().any(|e| {
            e.position == position && e.kind == ExitKind::Expire && e.result.landed().is_some()
        }) {
            expired = true;
            break;
        }
    }
    assert!(expired, "expired on the curve");
    assert!(
        h.with_env(|env| read::borsh::<shortwin::state::ShortPosition>(&env.svm, &position))
            .is_none()
    );
    assert!(!curve(&h, &pm).complete);
}

/// A chain without a block engine cannot take the bundle: the keeper alerts once and calls
/// `sol_settle` for the position meanwhile; nothing touches the curve.
#[test]
fn without_a_block_engine_the_straddle_waits() {
    let (mut h, pm, q) = pump_setup(without_exit());
    h.chain().set_bundles(false);
    let (b, nonce) = straddled_position(&h, &pm, q);
    let position = pda::position(&pm.market(), &b.key(), nonce).0;
    let r = h.second().clone();
    let StraddleResult::Waiting { reason } = &r.straddles[0].result else {
        panic!("{:?}", r.straddles[0].result);
    };
    assert!(reason.contains("block engine"), "{reason}");
    assert!(
        r.exits
            .iter()
            .any(|e| e.position == position && e.kind == ExitKind::SolSettle)
    );
    let c = curve(&h, &pm);
    assert!(!c.complete && c.real_token_reserves == q / 2);
}

/// Duty 5 on a list-only market (M5, M2-SEC-5): a covered curve market's curve completes and pump
/// migrates it **elsewhere** (reserves zeroed, no canonical pool; planted on pump's account) with
/// an expired position open. The keeper covers the market with its detection set as ReadSet: its
/// first crank moves it `PumpCurve → None` (graduated elsewhere; the S3 chain starts), it keeps
/// the chain on the no-route cadence (an observation at least every `no_route_interval_s`), sends
/// no migration, no straddle and no trading exit, and settles the position in S3 on the detection
/// set as soon as the chain has run `s3_wait_s`.
#[test]
fn a_curve_that_graduated_elsewhere_is_settled_in_s3_by_the_keeper() {
    let (mut h, pm, q) = pump_setup(KeeperConfig::default());
    let (b, nonce) = h.with_env(|env| {
        env.crank(&pm);
        env.warp(1);
        let mut b = env.borrower();
        let (res, nonce) = env.open(&pm, &mut b, q);
        ok(res);
        env.warp(1);
        // The curve completes and pump migrates it elsewhere before the keeper's next tick.
        buy_down_to(env, &pm.curve, 0);
        fork::set_curve_reserves(&mut env.svm, &pm.curve.curve, 0, 0, 0, 0);
        let deadline = env.position(&pm, &b.key(), nonce).deadline;
        let now = env.now();
        env.warp(deadline - now + 1);
        (b, nonce)
    });
    let position = pda::position(&pm.market(), &b.key(), nonce).0;
    let s3_wait = h.with_env(|env| env.position(&pm, &b.key(), nonce).s3_wait_s);
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::None));
    let since = h.with_env(|env| env.market(&pm.market()).no_route_since);
    assert!(since > 0, "the S3 chain runs");
    let mut last_obs = h.now();
    let mut settled_at = None;
    for _ in 0..(i64::from(s3_wait) + 120) {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.migrations.is_empty() && r.straddles.is_empty());
        assert!(r.exits.iter().all(|e| e.kind == ExitKind::SolSettle));
        let now = h.now();
        if r.cranked.contains(&pm.market()) || r.observed(&pm.market()) {
            last_obs = now;
        }
        assert!(
            now - last_obs <= KeeperConfig::default().coverage.no_route_interval_s,
            "the no-route cadence at {now} (last observation {last_obs})"
        );
        if r.exits.iter().any(|e| {
            e.position == position && e.kind == ExitKind::SolSettle && e.result.landed().is_some()
        }) && h
            .with_env(|env| read::borsh::<shortwin::state::ShortPosition>(&env.svm, &position))
            .is_none()
        {
            settled_at = Some(now);
            break;
        }
    }
    let at = settled_at.expect("settled in S3");
    assert!(
        at >= since + i64::from(s3_wait),
        "not before the chain ran s3_wait_s"
    );
    assert!(
        at <= since + i64::from(s3_wait) + KeeperConfig::default().coverage.s3_probe_interval_s + 1,
        "at the first probe after it ({at} vs {since} + {s3_wait})"
    );
    println!(
        "graduated elsewhere: the keeper kept the S3 chain from {since} and settled at +{} s",
        at - since
    );
}

/// `crank_price`'s `None → PumpCurve` re-detection by the keeper (M5; ARCH §2.5 `None` row): a mint
/// listed before its curve was readable (the curve account removed for the listing, restored
/// after) is list-only; the keeper sees a live curve in its detection set (the program's
/// `classify_pump`) and cranks it onto the curve, then creates the trader's base ATA and pump UVA
/// the curve legs require (the crank has no payer and creates neither), once; a borrower's open
/// then lands on the curve.
#[test]
fn a_list_only_market_is_redetected_and_prepared_by_the_keeper() {
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::T22_115);
    let saved = env.svm.get_account(&c.curve).unwrap();
    env.svm
        .set_account(c.curve, solana_account::Account::default())
        .unwrap();
    let creator = env.funded_keypair();
    let list = shortwin_litesvm_tests::ix::create_market(
        &solana_signer::Signer::pubkey(&creator),
        &c.mint,
        &c.token_program.id(),
        shortwin::instructions::create_market::VENUE_HINT_AUTO,
        &pda::canonical_pool(&c.mint),
    );
    ok(env.send(&[list], &[&creator]));
    env.svm.set_account(c.curve, saved).unwrap();
    let q = c.state(&env.svm).virtual_token_reserves / 200;
    let pm = env.pump_market_listed(PumpSpec::new(PumpCoin::T22_115, q * 40), c);
    assert_eq!(
        env.market(&pm.market()).venue_kind(),
        Some(VenueKind::None),
        "listed without a curve: list-only"
    );
    let mut h = Harness::new(env, KeeperConfig::default());
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(r.cranked.contains(&pm.market()), "the keeper cranks it");
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpCurve));
    let [base, uva] = h.with_env(|env| graduation::trader_account_keys(&env.market(&pm.market())));
    assert_eq!(base, pm.trader_base_ata);
    let missing = |h: &Harness| {
        h.with_env(|env| {
            (
                read::account(&env.svm, &base).is_none(),
                read::account(&env.svm, &uva).is_none(),
            )
        })
    };
    assert_eq!(missing(&h), (true, true), "the crank created nothing");
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.curve_accounts.len(), 1);
    assert!(
        r.curve_accounts[0].1.landed().is_some(),
        "{:?}",
        r.curve_accounts
    );
    assert_eq!(missing(&h), (false, false), "the keeper created both");
    for _ in 0..3 {
        let r = h.second().clone();
        assert!(r.curve_accounts.is_empty(), "once");
    }
    // A borrower's open lands on the curve.
    h.with_env(|env| {
        env.warp(i64::from(env.config().params.market_warmup_s));
        env.crank(&pm);
        env.warp(1);
        let mut b = env.borrower();
        let (res, _) = env.open(&pm, &mut b, q);
        ok(res);
    });
}
