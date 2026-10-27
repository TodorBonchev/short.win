//! KI-12 launch-cap fix (erratum): a sale-wrapped open that starts above the EMA
//! understated `N_open`, so the 25 SOL `market_max_notional` could be exceeded on
//! honest / EMA notional. The open now gates the cap on `max(N_open, B̂_ema)`.
//!
//! Credit: Chaplin (ahsanimtoken). Adapted from their CAND-1 PoC, which showed
//! that KI-12's ≈ 11 % bound held only when the pre-sale spot started at the EMA
//! (`s0 = 1.00` → ratio ≈ 0.905; `s0 = 1.05` → ≈ 0.862; `s0 = 1.10` → ≈ 0.823).
//!
//! Shape (mock venue, mirroring `m7_sec_4`): establish EMA by cranking, push spot
//! up to `s0 × EMA` without cranking, then in one tx `[sell δ to 0.905 × EMA,
//! open_short(q)]`. Assert that an open whose EMA notional would exceed the
//! launch cap is refused `MarketNotionalCap` at `s0 = 1.05` and `s0 = 1.10`,
//! while a normal open at the EMA still succeeds.

#![allow(clippy::result_large_err)]

#[allow(unused_imports)]
#[path = "engine_it/common.rs"]
mod ecommon;

use crate::ecommon::*;
use shortwin::errors::ShortwinError as E;
use shortwin::events::Open;
use shortwin::venues::ExtrasSel;

const SOL: u64 = 1_000_000_000;

/// `N_open(q)` on the mock pool now: sale of `q`, then buyback of `q` on the
/// post-sale pricing reserves (honest notional of an open of `q` here).
fn n_open(env: &Env, mm: &MockMarket, q: u64) -> Option<u64> {
    use shortwin_math::quote::cpmm;
    let r = mock::reserves(&env.svm, mm.mock());
    let fee = mock::state(&env.svm, &mm.mock().pool).fee_ppm;
    let fees = mock::cpmm_fees(fee);
    let s = cpmm::sell_exact_in(q, u128::from(r.base), u128::from(r.quote), &fees)?;
    cpmm::buy_exact_out(q, s.sol_after, s.token_after, &fees).map(|b| b.cost)
}

fn least_q(f: impl Fn(u64) -> u64, target: u64, hi: u64) -> u64 {
    let (mut lo, mut hi) = (0u64, hi);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if f(mid) >= target {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

fn deep_spec() -> MarketSpec {
    MarketSpec {
        fee_ppm: 10_000,
        base: 10_000_000_000_000_000,
        quote: 10_000_000_000_000,
        lend: 1_000_000_000_000_000,
        ..MarketSpec::default()
    }
}

/// Crank to pin the EMA, optionally push spot to `s0 × EMA`, size `q` for
/// `target_honest` on those reserves, then attempt a sale-wrapped open dumping
/// to 0.905 × EMA in the same transaction.
fn sale_wrap_at(
    s0_num: u128,
    s0_den: u128,
    target_honest: u64,
) -> (u64, Result<Open, Option<u32>>) {
    let (mut env, mm) = setup_with(deep_spec());
    let key = mm.market();
    let attacker = env.actor(&mm, 5_000_000_000_000_000, 1_000_000_000_000_000);
    let b = env.borrower();
    for _ in 0..30 {
        env.warp(1);
        env.crank(&mm);
    }
    let p_ema = env.market(&key).ema_price.get();
    if s0_num != s0_den {
        env.push_price(&mm, &attacker, p_ema * s0_num / s0_den);
    }
    let q = least_q(
        |q| n_open(&env, &mm, q).unwrap_or(0),
        target_honest,
        100_000_000_000_000,
    );
    let honest = n_open(&env, &mm, q).expect("honest notional");
    let (_, _, sell) = env
        .push_price_ix(&mm, &attacker, p_ema * 905 / 1000)
        .expect("a sale");
    let open = open_short_ix(
        &mm,
        &b.key(),
        b.next_nonce,
        q,
        0,
        u64::MAX,
        u64::MAX,
        ExtrasSel::NONE,
        &[],
    );
    let (akp, bkp) = (attacker.kp.insecure_clone(), b.kp.insecure_clone());
    let res = env.send(&[sell, open], &[&akp, &bkp]);
    let out = match &res {
        Ok(meta) => Ok(events::one::<Open>(meta)),
        Err(_) => Err(custom_code(&res)),
    };
    (honest, out)
}

/// At s0 = 1.05 and 1.10, an open sized so the honest pre-wrap notional exceeds
/// the 25 SOL launch cap must be refused `MarketNotionalCap` (before the fix it
/// would land with recorded `N_open` under the cap).
#[test]
fn sale_wrap_above_ema_exceeding_cap_on_honest_notional_is_refused() {
    let max = 25 * SOL;
    // Target honest ≈ 28 SOL on the elevated spot: recorded N_open at the dump
    // edge is ~0.82–0.86 of that (under 25), but B_ema ≈ quote at EMA exceeds 25.
    let target_honest = 28 * SOL;
    for (s0_num, s0_den, label) in [(105u128, 100u128, "1.05"), (110, 100, "1.10")] {
        let (honest, out) = sale_wrap_at(s0_num, s0_den, target_honest);
        assert!(
            honest > max,
            "s0={label}: honest {honest} should exceed the {max} launch cap"
        );
        match out {
            Err(Some(c)) if c == code(E::MarketNotionalCap) => {}
            Ok(_) => panic!(
                "s0={label}: expected MarketNotionalCap for honest {honest} (~{:.2} SOL), open landed",
                honest as f64 / 1e9
            ),
            Err(other) => panic!(
                "s0={label}: expected MarketNotionalCap for honest {honest} (~{:.2} SOL), got err {other:?}",
                honest as f64 / 1e9
            ),
        }
        println!(
            "KI-12 fix s0={label}: honest {:.3} SOL refused MarketNotionalCap (cap {:.0} SOL)",
            honest as f64 / 1e9,
            max as f64 / 1e9
        );
    }
}

/// A normal open at the EMA whose notional fits under the launch cap still lands.
#[test]
fn normal_open_at_ema_under_cap_succeeds() {
    let (mut env, mm) = setup_with(deep_spec());
    for _ in 0..30 {
        env.warp(1);
        env.crank(&mm);
    }
    let target = 5 * SOL;
    let q = least_q(
        |q| n_open(&env, &mm, q).unwrap_or(0),
        target,
        100_000_000_000_000,
    );
    let mut b = env.borrower();
    let (res, _) = env.open(&mm, &mut b, q);
    let o: Open = events::one(&ok(res));
    assert!(o.n_open >= target);
    assert!(o.n_open < 25 * SOL);
    println!(
        "KI-12 fix: honest EMA open N_open {:.3} SOL succeeded",
        o.n_open as f64 / 1e9
    );
}

/// Small sale-wrapped opens (Chaplin's original PoC size) still succeed: the fix
/// only refuses when `max(N_open, B_ema)` would breach the launch cap. Also pins
/// that at s0 = 1.05 / 1.10 the recorded/honest ratio still falls below KI-12's
/// old 90 % one-sided figure (fee / stored `N_open` unchanged).
#[test]
fn small_sale_wrap_above_ema_still_succeeds() {
    let target = SOL / 2;
    let cases = [
        (100u128, 100u128, "1.00", 8800u128..=9200),
        (105, 100, "1.05", 8400..=8800),
        (110, 100, "1.10", 8000..=8400),
    ];
    for (s0_num, s0_den, label, ratio_range) in cases {
        let (honest, out) = sale_wrap_at(s0_num, s0_den, target);
        let o = out.unwrap_or_else(|e| panic!("s0={label}: small wrap should land, got {e:?}"));
        let ratio = u128::from(o.n_open) * 10_000 / u128::from(honest);
        assert!(
            ratio_range.contains(&ratio),
            "s0={label}: ratio {}.{}% outside expected {ratio_range:?}",
            ratio / 100,
            ratio % 100
        );
        assert!(o.n_open < 25 * SOL);
        println!(
            "KI-12 fix s0={label}: small wrap landed recorded {:.3} SOL (honest {:.3} SOL, ratio {}.{}%)",
            o.n_open as f64 / 1e9,
            honest as f64 / 1e9,
            ratio / 100,
            ratio % 100
        );
    }
}
