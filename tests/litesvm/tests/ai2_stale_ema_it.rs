//! Regression for PoC 02 (`shortwin|open-short|stale-ema-open-margin`): the audited 1.0.1
//! `open_short` did not check the age of `P_ema`.
//!
//! The finding: `open_short` reads `P_ema` for the D4 margin, the caps and the spot/EMA guard but
//! never checks how old it is (no line reads `market.ema_ts`). A market that holds lender tokens
//! and has no open position is never cranked — `services/keeper/src/decide.rs::market_needs` gives
//! a position-less market no per-second cadence — so its `P_ema` ages while the live price moves.
//! An attacker sells the idle pool back down to the stale-low `P_ema`, opens a short there (margin
//! priced at the stale-low price), and buys back; the position is under-collateralised and the
//! lenders bear the loss.
//!
//! The fix checks the pre-prologue EMA timestamp and the completed observation's timestamp
//! against `COVERAGE_GAP_S`. A preceding crank cannot make an old bucket's sample fresh.
//!
//! This regression test asserts that SAFE behaviour: the stale-EMA open must be refused with
//! `EmaWarmingUp`. On the audited 1.0.1 build there is no such check, so the open lands — the
//! final assertion fails and prints the lenders' shortfall. After the fix the open is refused and
//! the test passes.
//!
//! Harness: the mock venue (test-venue build; the step-4 open guard where the bug lives uses
//! the same implementation as the production build and is venue-independent — the Raydium /
//! PumpSwap `ema_warm` blocks are skipped for the mock, so the mock isolates the missing age
//! check). Every short.win account reaches its state through real instructions; the only
//! "written" state is the mock pool's reserves, moved by real whale/attacker swaps, and the
//! clock, advanced by `warp` — both stand for ordinary market activity and the passage of time.

#![allow(clippy::result_large_err)]

use shortwin::errors::ShortwinError as E;
use shortwin::events::Open;
use shortwin::venues::ExtrasSel;
use shortwin_litesvm_tests::engine::{MarketSpec, crank_price_ix, open_short_ix};
use shortwin_litesvm_tests::errors::{assert_err, custom_code, ok};
use shortwin_litesvm_tests::mock::{self, Dir};
use shortwin_litesvm_tests::{Env, events};
use shortwin_math::consts::COVERAGE_GAP_S;

const SOL: u64 = 1_000_000_000;

/// The live price rises to `F x` the stale `P_ema` while the market is idle.
const F: u128 = 9;

#[test]
fn first_post_upgrade_observation_invalidates_legacy_settlement_arms_once() {
    let mut env = Env::test_venue();
    let mm = env.mock_market(MarketSpec::default());
    let key = mm.market();
    let mut market = env.market(&key);
    let before = market.ema_count;
    assert!(before > 0);
    // Only the new reserved-field encoding is reset; observations and economics are reached
    // through normal instructions. Existing accounts have zero here at upgrade.
    market.restart_ema_count = 0;
    shortwin_litesvm_tests::fixtures::set_zc(&mut env.svm, &key, &market, 0);
    env.warp(1);
    ok(env.send(&[crank_price_ix(&mm)], &[]));
    let marker = env.market(&key).restart_ema_count;
    assert_eq!(marker, before + 1);
    env.warp(1);
    ok(env.send(&[crank_price_ix(&mm)], &[]));
    assert_eq!(env.market(&key).restart_ema_count, marker);
}

#[test]
fn open_short_refuses_a_stale_ema() {
    // ---- 1. an idle mock market: lender inventory, no position, never cranked ---------------
    // `create_market` seeds the observation with `Observation::new` (an empty bucket, `ema_ts =
    // now`); the lender deposit runs `roll_only` and records nothing. Nothing then cranks it.
    let spec = MarketSpec {
        base: 1_000_000_000_000_000, // 1e15 raw tokens
        quote: 1_000_000_000_000,    // 1,000 SOL
        fee_ppm: 2_500,              // 25 bps
        lend: 100_000_000_000_000,   // 1e14 raw (10 % of the pool) lent
        // > market_warmup_s (50, so the open clears the warm-up guard) and > COVERAGE_GAP_S (40,
        // so P_ema is already "old"): the moment an idle market is first openable, its EMA is stale.
        warmup: 60,
        ..MarketSpec::default()
    };
    let mut env = Env::test_venue();
    let mm = env.mock_market_unobserved(spec);
    let m0 = env.market(&mm.market());
    let p0 = m0.ema_price.get();
    println!(
        "[setup] P_ema(listing)={p0}  ema_ts={}  now={}  b_lo={}  open_positions={}",
        m0.ema_ts,
        env.now(),
        m0.b_lo.get(),
        m0.open_positions
    );
    assert_eq!(m0.open_positions, 0, "the market is idle: no position");
    assert_eq!(m0.b_lo.get(), 0, "no observation bucket is open");

    // ~3 SOL of notional at P_ema; fits every default cap (the engine tests' `Q`).
    let q: u64 = 3_000_000_000_000;

    // ---- 2. other traders push the live price up F x while the market is idle ---------------
    // A real whale buy on the pool; no crank follows (an idle, position-less market gets none).
    let whale = env.actor(&mm, 0, 200_000 * SOL);
    env.push_price(&mm, &whale, p0 * F);
    let true_spot = env.spot(&mm);
    // The honest replacement cost of the q borrowed tokens at the live price (what it costs to buy
    // them back once the attacker restores the pool).
    let b_true = mock::quote_exact_out(&env.svm, mm.mock(), Dir::QuoteIn, q)
        .expect("buy quote")
        .cost;
    println!(
        "[pushed] live spot={true_spot} (~{}x P_ema)  B_hat(q) at live price = {b_true} lamports (~{:.3} SOL)",
        true_spot / p0.max(1),
        b_true as f64 / 1e9
    );

    // ---- 3. the attacker sells the idle pool back down to ~P_ema ----------------------------
    // A real attacker sale (it holds token inventory, returned when it buys back after the open).
    let attacker = env.actor(&mm, 2_000_000_000_000_000, 1_000 * SOL);
    env.push_price(&mm, &attacker, p0);
    let spot_lo = env.spot(&mm);

    // ---- 4. the stale, idle market at the moment of the open --------------------------------
    let m = env.market(&mm.market());
    let p_ema = m.ema_price.get();
    let age = env.now() - m.ema_ts;
    let dev_bps = spot_lo.abs_diff(p_ema).saturating_mul(10_000) / p_ema.max(1);
    println!(
        "[attack] P_ema={p_ema} (stale)  spot after sell-down={spot_lo} ({dev_bps} bps off P_ema)  \
         ema_ts={}  now={}  age={age}s  COVERAGE_GAP_S={COVERAGE_GAP_S}  b_lo={}",
        m.ema_ts,
        env.now(),
        m.b_lo.get()
    );
    // Preconditions of a faithful stale-EMA open (they hold on 1.0.1 and after the fix alike):
    // the roll at open integrates nothing (empty bucket), so ema_ts stays old; the EMA is older
    // than COVERAGE_GAP_S; and the sold-down spot is within open_ema_dev_bps of the stale P_ema.
    assert_eq!(
        m.b_lo.get(),
        0,
        "the open's roll integrates nothing: ema_ts stays old"
    );
    assert!(
        age > COVERAGE_GAP_S,
        "P_ema is older than COVERAGE_GAP_S ({age} > {COVERAGE_GAP_S})"
    );
    assert!(
        dev_bps <= 1_000,
        "spot is sold back to within open_ema_dev_bps of P_ema ({dev_bps} bps)"
    );

    // ---- 5. the attacker opens the short (sent on a clone of the VM) -------------------------
    let b = env.borrower();
    let ix = open_short_ix(
        &mm,
        &b.key(),
        0,
        q,
        0,
        u64::MAX,
        u64::MAX,
        ExtrasSel::NONE,
        &[],
    );
    let tx = env.transaction(&[ix], &[&b.kp]);
    let res = {
        let mut svm = env.svm.clone();
        svm.send_transaction(tx)
    };
    let opened = res.is_ok();
    let code = custom_code(&res);

    match &res {
        Ok(meta) => {
            let ev = events::of::<Open>(meta)
                .into_iter()
                .next()
                .expect("Open event");
            let shortfall = b_true.saturating_sub(ev.v);
            println!(
                "[1.0.1: open LANDED] q={} s={} m={} V=S+M={} n_open(B_spot)={} b_ema={} tier={}",
                ev.q, ev.s, ev.m, ev.v, ev.n_open, ev.b_ema, ev.tier
            );
            println!(
                "  margin locks V={:.3} SOL, priced at the stale-low P_ema; the q borrowed tokens \
                 are worth B_hat={:.3} SOL at the live price",
                ev.v as f64 / 1e9,
                b_true as f64 / 1e9
            );
            println!(
                "  => the position is under-collateralised; the lenders cannot recover \
                 ~{:.3} SOL ({shortfall} lamports) once the price is restored",
                shortfall as f64 / 1e9
            );
            assert!(
                shortfall > 0,
                "harm: live replacement cost B_true={b_true} must exceed locked V={}",
                ev.v
            );
        }
        Err(_) => println!("[fixed: open REFUSED] code={code:?} (the stale-EMA open is refused)"),
    }

    // ---- 6. the regression assertion ---------------------------------------------------------
    // SAFE behaviour (the Fix): a stale-EMA open is refused with EmaWarmingUp. On 1.0.1 there is
    // no age check, so `code` is `None` (the open landed) and this fails, printing the harm.
    assert_eq!(
        code,
        Some(u32::from(E::EmaWarmingUp)),
        "open_short accepted a short against a P_ema {age}s old (> COVERAGE_GAP_S={COVERAGE_GAP_S}): \
         it locked the D4 margin at the stale-low P_ema while the {q} borrowed tokens are worth \
         ~{:.1} SOL at the live price, so the lenders bear the shortfall (open landed: {opened})",
        b_true as f64 / 1e9
    );
}

/// The maximum permitted age is inclusive. The open rolls the still-open bucket, but both
/// its original observation time and the EMA's prior integration time must satisfy the bound.
#[test]
fn open_freshness_accepts_the_boundary_and_refuses_the_next_second() {
    for age in [COVERAGE_GAP_S, COVERAGE_GAP_S + 1] {
        let mut env = Env::test_venue();
        let mm = env.mock_market(MarketSpec::default());
        let before = env.market(&mm.market());
        assert_eq!(before.ema_ts, env.now());
        assert_eq!(before.b_sec, env.now());
        env.warp(age);
        let mut borrower = env.borrower();
        let (res, _) = env.open(&mm, &mut borrower, 3_000_000_000_000);
        if age == COVERAGE_GAP_S {
            ok(res);
        } else {
            assert_err(&res, E::EmaWarmingUp);
        }
    }
}

/// Rolling an old bucket updates ema_ts, but does not make that bucket's sample current.
/// A preceding crank (including in the same transaction) must not bypass the freshness gate.
/// A sample collected now and integrated the next second restores ordinary opens.
#[test]
fn an_old_bucket_needs_a_fresh_observation_before_opens_resume() {
    let mut env = Env::test_venue();
    let mm = env.mock_market(MarketSpec::default());
    let old_sample_at = env.market(&mm.market()).b_sec;
    env.warp(COVERAGE_GAP_S + 1);
    let borrower = env.borrower();
    let open = open_short_ix(
        &mm,
        &borrower.key(),
        0,
        3_000_000_000_000,
        0,
        u64::MAX,
        u64::MAX,
        ExtrasSel::NONE,
        &[],
    );
    assert_err(
        &env.send(&[crank_price_ix(&mm), open.clone()], &[&borrower.kp]),
        E::EmaWarmingUp,
    );
    env.crank(&mm);
    let rolled = env.market(&mm.market());
    assert_eq!(
        rolled.ema_ts,
        env.now(),
        "the old bucket was integrated now"
    );
    assert_eq!(rolled.lb_sec, old_sample_at, "its observation is still old");
    assert_err(
        &env.send(std::slice::from_ref(&open), &[&borrower.kp]),
        E::EmaWarmingUp,
    );

    env.warp(1);
    env.crank(&mm);
    let fresh = env.market(&mm.market());
    assert_eq!(fresh.lb_sec, env.now() - 1);
    ok(env.send(&[open], &[&borrower.kp]));
}
