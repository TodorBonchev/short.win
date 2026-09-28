//! T-RAY-DISABLE and T-RAY-FEE (ARCH §5.3, §5.5, §4.3 "two fee views", §8.2; A43, A63, A64,
//! V2-1, V2-5, R3A-4) on the real programs, driven by the pool admin's **real** instructions
//! (`SetParams`, `update_pool_status`, `update_amm_config`, the admin's signature forged) and
//! by byte builders for what no admin instruction reaches (AMM v4 status 8, a CPMM open time).
//!
//! - **Disable**: the route predicate equals the venue's own (every AMM v4 status and open-time
//!   case, every CPMM status bit and open time): a halted pool is not executable — nothing is
//!   observed, the S3 chain and the `pool_fail` chain start, opens and closes fail with
//!   `NoRoute` — and an expired position on it settles S3 once the chain has run `s3_wait_s`,
//!   at FV on the frozen EMA point priced at the halted pool's own fee; re-enabling clears both
//!   chains and trading resumes.
//! - **Fee**: a hike above `venue_fee_cap_bps` refuses opens and is not S3 evidence; above the
//!   global `venue_fee_hard_cap_bps` it is (`s3_no_route`, `pool_fail`), closes still execute
//!   at the live fee, liquidations price risk at the position's locked cap (a 70 % fee cannot
//!   make a healthy position liquidatable), and an S3 pays FV at that cap, never at the live fee.

use shortwin::events::SolSettle;
use shortwin::state::ShortPosition;
use shortwin_litesvm_tests::engine::{liquidate_ix, sol_settle_ix};
use shortwin_math::fees::{CpmmFees, RayV4Fee};
use shortwin_math::quote::Pricing;
use shortwin_math::rent;
use shortwin_venue_state::raydium_v4;
use solana_keypair::Keypair;

use crate::common::*;

/// `K_s` of a position (its locked settlement-keeper terms).
fn k_s(p: &ShortPosition) -> u64 {
    let pct = u128::from(p.v) * u128::from(p.settle_keeper_bps) / 10_000;
    (u128::from(p.settle_keeper_fixed) + pct)
        .min(u128::from(p.settle_keeper_cap))
        .min(u128::from(p.v)) as u64
}

/// `FV(P, L) = B̂(Q; reprice(L, P); pricing) + ⌈p'·B̂/10⁴⌉ + R_due` (ARCH §4.5).
fn fair_value(pricing: Pricing, q: u64, p: u128, l: u128, pprime_bps: u16, r_due: u64) -> u64 {
    let b = pricing.buy_repriced(q, l, p).expect("FV quote").cost;
    b + (u128::from(b) * u128::from(pprime_bps)).div_ceil(10_000) as u64 + r_due
}

/// One `sol_settle` by `keeper`: `Some(event)` if it settled.
fn settle(
    env: &mut Env,
    rm: &RayMarket,
    keeper: &Keypair,
    b: &Borrower,
    nonce: u64,
) -> Option<SolSettle> {
    let ix = sol_settle_ix(env, rm, &keeper.pubkey(), &b.key(), nonce);
    let meta = ok(env.send(&[ix], &[keeper]));
    events::of::<SolSettle>(&meta).into_iter().next()
}

/// Cranks every `step` seconds, calling `sol_settle` after each crank, for at most `n` steps:
/// the settlement event if one executed.
fn run_to_s3(
    env: &mut Env,
    rm: &RayMarket,
    keeper: &Keypair,
    b: &Borrower,
    nonce: u64,
    step: i64,
    n: usize,
) -> Option<SolSettle> {
    for _ in 0..n {
        env.warp(step);
        env.crank(rm);
        if let Some(ev) = settle(env, rm, keeper, b, nonce) {
            return Some(ev);
        }
    }
    None
}

fn warp_to(env: &mut Env, t: i64) {
    let now = env.now();
    assert!(t >= now);
    env.warp(t - now);
}

/// The AMM v4 route predicate is the venue's own (R-RAY §5.3), statuses set by `SetParams`
/// where the admin can (1–7) and by bytes where it cannot (0, 8): executable exactly for 1, 6,
/// and 7 from its open time.
#[test]
fn amm_v4_disable_matrix() {
    for status in 0u64..=8 {
        let (mut env, rm, _) = setup(FART);
        if (1..=7).contains(&status) {
            ok(fork::raydium_admin_send(
                &mut env,
                fork::v4_set_params_ix(rm.v4(), 0, status),
            ));
        } else {
            fork::set_v4_status(&mut env.svm, rm.v4(), status);
        }
        env.warp(1);
        let t0 = env.market(&rm.market());
        env.crank(&rm);
        let m = env.market(&rm.market());
        let executable = matches!(status, 1 | 6 | 7);
        assert_eq!(
            m.no_route_since == 0,
            executable,
            "status {status}: the S3 chain"
        );
        assert_eq!(
            m.pool_fail_since == 0,
            executable,
            "status {status}: pool_fail"
        );
        if !executable {
            assert_eq!(m.no_route_since, env.now());
            assert_eq!(m.b_lo.get(), 0, "status {status}: nothing observed");
            assert_eq!(
                m.ema_count,
                t0.ema_count + 1,
                "the bucket before the halt integrates"
            );
        } else {
            assert!(m.b_lo.get() > 0, "status {status}: observed");
        }
    }
    // WaitingTrade with a future open time: not executable until then (SetParams param 3).
    let (mut env, rm, q) = setup(FART);
    let open_at = env.now() + 120;
    ok(fork::raydium_admin_send(
        &mut env,
        fork::v4_set_params_ix(rm.v4(), 3, open_at as u64),
    ));
    ok(fork::raydium_admin_send(
        &mut env,
        fork::v4_set_params_ix(rm.v4(), 0, 7),
    ));
    env.warp(1);
    env.crank(&rm);
    assert!(env.market(&rm.market()).no_route_since > 0);
    let mut b = env.borrower();
    assert_err(&env.open(&rm, &mut b, q).0, E::NoRoute);
    warp_to(&mut env, open_at);
    env.crank(&rm);
    assert_eq!(
        env.market(&rm.market()).no_route_since,
        0,
        "open from pool_open_time"
    );
    let (_, nonce, _) = open_ok(&mut env, &rm, &mut b, q);
    assert_eq!(
        rm.v4().info(&env.svm).status,
        raydium_v4::STATUS_SWAP_ONLY,
        "the first swap moves WaitingTrade to SwapOnly"
    );
    env.warp(1);
    close_ok(&mut env, &rm, &b, nonce);
}

/// CPMM: the swap bit (2) of `update_pool_status` halts swaps; the deposit and withdraw bits do
/// not; a future `open_time` (bytes: no instruction can change it) halts until then.
#[test]
fn cpmm_disable_matrix() {
    for status in 0u8..=7 {
        let (mut env, rm, _) = setup(USELESS);
        ok(fork::raydium_admin_send(
            &mut env,
            fork::cpmm_update_pool_status_ix(rm.cpmm(), status),
        ));
        env.warp(1);
        env.crank(&rm);
        let m = env.market(&rm.market());
        let executable = status & 4 == 0;
        assert_eq!(m.no_route_since == 0, executable, "status {status}");
        assert_eq!(m.pool_fail_since == 0, executable, "status {status}");
    }
    let (mut env, rm, q) = setup(USELESS);
    let open_at = env.now() + 90;
    fork::set_cpmm_open_time(&mut env.svm, rm.cpmm(), open_at as u64);
    env.warp(1);
    env.crank(&rm);
    assert!(env.market(&rm.market()).no_route_since > 0);
    warp_to(&mut env, open_at);
    env.crank(&rm);
    assert_eq!(env.market(&rm.market()).no_route_since, 0);
    let mut b = env.borrower();
    open_ok(&mut env, &rm, &mut b, q);
}

/// M3-SEC-1 (T-RAY-DISABLE with zero fees). `update_amm_config(0, 0)` zeroes config 0's trade
/// fee; USELESS (fee model 0, creator fee off) then cannot split a buy's fee and the venue
/// refuses every swap (`ZeroTradingTokens`): not executable — both chains start, nothing is
/// observed, opens and closes fail with `NoRoute`. A zero fee that only the sale cannot split
/// (the creator rate also 0 via `update_amm_config(7, 0)`, the pool's fee model byte-set to 2:
/// the creator fee on the token, the sale's input and the buy's output) keeps the route: the
/// chains clear and the position closes fee-free; opens are refused by the fee-scaled
/// per-position cap, which is `3·φ·X₀ = 0` at `φ = 0` (`CapPositionImpact`, ECON §6.1) before
/// the undefined sale is quoted. With the fees back, trading resumes.
#[test]
fn cpmm_zero_fee_disable_matrix() {
    let (mut env, rm, q) = setup(USELESS);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &rm, &mut b, q);
    let cfg = rm.cpmm().config_key(&env.svm);
    let c0 = rm.cpmm().config(&env.svm);
    set_fee(&mut env, &rm, 0);
    env.warp(1);
    env.crank(&rm);
    let m = env.market(&rm.market());
    assert_eq!(
        (m.no_route_since, m.pool_fail_since),
        (env.now(), env.now())
    );
    assert_eq!(m.b_lo.get(), 0, "nothing observed");
    assert_err(&env.close(&rm, &b, nonce), E::NoRoute);
    let mut b2 = env.borrower();
    assert_err(&env.open(&rm, &mut b2, q).0, E::NoRoute);

    // Sell-only: the creator fee on the token side at rate 0.
    ok(fork::raydium_admin_send(
        &mut env,
        fork::cpmm_update_amm_config_ix(&cfg, 7, 0),
    ));
    fork::set_cpmm_creator_fee(&mut env.svm, rm.cpmm(), true, 2);
    assert!(rm.cpmm().venue_executable(&env.svm, env.now()));
    env.warp(1);
    env.crank(&rm);
    let m = env.market(&rm.market());
    assert_eq!(
        (m.no_route_since, m.pool_fail_since),
        (0, 0),
        "the buy route is back"
    );
    assert!(m.b_lo.get() > 0, "observed");
    assert_err(&env.open(&rm, &mut b2, q).0, E::CapPositionImpact);
    env.warp(1);
    let quote = rm.quote_buy(&env.svm, q).unwrap();
    assert_eq!(quote.cost, quote.net, "fee-free");
    let (close, _) = close_ok(&mut env, &rm, &b, nonce);
    assert_eq!(close.buyback_cost, quote.cost);

    // Both rates and the fee model back: opens resume.
    fork::set_cpmm_creator_fee(&mut env.svm, rm.cpmm(), false, 0);
    ok(fork::raydium_admin_send(
        &mut env,
        fork::cpmm_update_amm_config_ix(&cfg, 7, c0.creator_fee_rate),
    ));
    ok(fork::raydium_admin_send(
        &mut env,
        fork::cpmm_update_amm_config_ix(&cfg, 0, c0.trade_fee_rate),
    ));
    env.warp(1);
    env.crank(&rm);
    let (_, n2, _) = open_ok(&mut env, &rm, &mut b2, q);
    env.warp(1);
    close_ok(&mut env, &rm, &b2, n2);
}

/// ARCH §5.3 "Liveness without a route" on both programs: an admin halt freezes the position
/// (no close, no S3 while healthy and not expired); at its deadline the continuous chain lets
/// `sol_settle` pay S3 at FV on the frozen EMA point, priced at the halted pool's own fee, with
/// the expiry fee (O15). Re-enabling the pool clears both chains.
#[test]
fn a_halted_pool_reaches_s3_at_the_deadline() {
    for fixture in [FART, USELESS] {
        let (mut env, rm, q) = setup(fixture);
        let mut b = env.borrower();
        let (_, nonce, _) = open_ok(&mut env, &rm, &mut b, q);
        let pos = env.position(&rm, &b.key(), nonce);
        env.tick(&rm, 3);
        let live_fee = match &rm.pool {
            AnyPool::V4(p) => {
                let i = p.info(&env.svm);
                Pricing::RayV4(RayV4Fee {
                    num: i.swap_fee_numerator,
                    den: i.swap_fee_denominator,
                })
            }
            AnyPool::Cpmm(p) => Pricing::Cpmm {
                buy: p
                    .fees(&env.svm, shortwin_math::fees::Direction::Buy)
                    .unwrap(),
                sell: p
                    .fees(&env.svm, shortwin_math::fees::Direction::Sell)
                    .unwrap(),
            },
        };
        match &rm.pool {
            AnyPool::V4(p) => {
                ok(fork::raydium_admin_send(
                    &mut env,
                    fork::v4_set_params_ix(p, 0, 2),
                ));
            }
            AnyPool::Cpmm(p) => {
                ok(fork::raydium_admin_send(
                    &mut env,
                    fork::cpmm_update_pool_status_ix(p, 4),
                ));
            }
        }
        env.warp(1);
        let keeper = env.funded_keypair();
        assert!(
            settle(&mut env, &rm, &keeper, &b, nonce).is_none(),
            "healthy, not expired"
        );
        let m0 = env.market(&rm.market());
        assert_eq!(m0.no_route_since, env.now());
        assert_eq!(m0.pool_fail_since, env.now());
        assert_err(&env.close(&rm, &b, nonce), E::NoRoute);
        // The chain stays continuous only with observations ≤ 60 s apart: a jump to the
        // deadline restarts it, and S3 waits s3_wait_s from there.
        let slot = env.clock().slot;
        env.set_clock(pos.deadline - 5, slot + 1_500_000);
        env.crank(&rm);
        let since = env.market(&rm.market()).no_route_since;
        assert_eq!(since, env.now());
        let ev = run_to_s3(&mut env, &rm, &keeper, &b, nonce, 30, 30).expect("S3");
        assert_eq!(ev.condition, 3);
        assert!(env.now() >= since + i64::from(pos.s3_wait_s));
        let m = env.market(&rm.market());
        let (p, l) = (m.ema_price.get(), m.ema_liq.get());
        assert_eq!(
            (p, l, m.ema_count),
            (m0.ema_price.get(), m0.ema_liq.get(), m0.ema_count),
            "the EMA is frozen while the pool is halted"
        );
        assert_eq!(
            (ev.p_lo, ev.p_hi, ev.l_lo, ev.l_hi),
            (p, p, l, l),
            "the EMA point"
        );
        assert_eq!(ev.pprime_lo, pos.expiry_fee_bps, "O15: an expired position");
        let r_due = rent::due(&pos.pos_rent(), &m.basis()).unwrap();
        let cap = u64::from(pos.venue_fee_hard_cap_bps);
        let fv = fair_value(live_fee.capped(cap), pos.q, p, l, ev.pprime_lo, r_due);
        assert_eq!(ev.fv, fv, "{fixture:?}: FV at the halted pool's own fee");
        assert_eq!(ev.lender_amount, fv.min(pos.v - k_s(&pos)));
        // Re-enabling clears both chains.
        match &rm.pool {
            AnyPool::V4(p) => {
                ok(fork::raydium_admin_send(
                    &mut env,
                    fork::v4_set_params_ix(p, 0, 6),
                ));
            }
            AnyPool::Cpmm(p) => {
                ok(fork::raydium_admin_send(
                    &mut env,
                    fork::cpmm_update_pool_status_ix(p, 0),
                ));
            }
        }
        env.warp(1);
        env.crank(&rm);
        let m = env.market(&rm.market());
        assert_eq!((m.no_route_since, m.pool_fail_since), (0, 0));
        assert!(m.b_lo.get() > 0, "observed again");
    }
}

/// Sets the pool's live fee by the admin's own instruction: AMM v4 swap fee `bps/10⁴` of this
/// pool, CPMM trade fee `bps·100` ppm of its whole config.
fn set_fee(env: &mut Env, rm: &RayMarket, bps: u64) {
    match &rm.pool {
        AnyPool::V4(p) => {
            let mut fees = fork::V4Fees::of(&env.svm, p);
            fees.swap_fee = (bps, 10_000);
            ok(fork::raydium_admin_send(
                env,
                fork::v4_set_fees_ix(p, &fees),
            ));
        }
        AnyPool::Cpmm(p) => {
            let cfg = p.config_key(&env.svm);
            ok(fork::raydium_admin_send(
                env,
                fork::cpmm_update_amm_config_ix(&cfg, 0, bps * 100),
            ));
        }
    }
}

/// A fee between `venue_fee_cap_bps` and the hard cap: opens refused, no S3 or `pool_fail`
/// evidence, closes at the live fee.
#[test]
fn fee_between_the_caps() {
    for fixture in [FART, USELESS] {
        let (mut env, rm, q) = setup(fixture);
        let mut b = env.borrower();
        let (_, nonce, _) = open_ok(&mut env, &rm, &mut b, q);
        set_fee(&mut env, &rm, 300);
        env.warp(1);
        env.crank(&rm);
        let m = env.market(&rm.market());
        assert_eq!((m.no_route_since, m.pool_fail_since), (0, 0), "{fixture:?}");
        let mut b2 = env.borrower();
        assert_err(&env.open(&rm, &mut b2, q).0, E::VenueFeeTooHigh);
        env.warp(1);
        let b_exp = rm.quote_buy(&env.svm, q).unwrap().cost;
        let (close, _) = close_ok(&mut env, &rm, &b, nonce);
        assert_eq!(close.buyback_cost, b_exp, "the live 3 % fee");
    }
}

/// A fee far above the hard cap (70 %): opens refused; the S3 and `pool_fail` chains run while
/// the pool is still observed; risk is priced at the position's locked cap (500 bps), so the
/// healthy position is not liquidatable — at the live 70 % it would be; the borrower can still
/// close at the live fee; at the deadline S3 pays FV at the cap.
#[test]
fn fee_above_the_hard_cap() {
    for fixture in [FART, USELESS] {
        let (mut env, rm, q) = setup(fixture);
        let mut b = env.borrower();
        let (open, nonce, _) = open_ok(&mut env, &rm, &mut b, q);
        let pos = env.position(&rm, &b.key(), nonce);
        env.tick(&rm, 3);
        set_fee(&mut env, &rm, 7_000);
        env.warp(1);
        env.crank(&rm);
        let m = env.market(&rm.market());
        assert!(m.no_route_since > 0 && m.pool_fail_since > 0, "{fixture:?}");
        assert!(
            m.b_lo.get() > 0,
            "a fee above the cap does not stop observation"
        );
        let mut b2 = env.borrower();
        assert_err(&env.open(&rm, &mut b2, q).0, E::VenueFeeTooHigh);
        // At the live fee the vault would not cover 1.5 × the buyback; at the cap it does.
        let live = rm.quote_buy(&env.svm, q).unwrap().cost;
        assert!(u128::from(open.v) * 10_000 < u128::from(live) * u128::from(pos.cr_liq_bps));
        env.tick(&rm, 4);
        let keeper = env.funded_keypair();
        let ix = liquidate_ix(&rm, &keeper.pubkey(), &b.key(), nonce);
        assert_err(&env.send(&[ix], &[&keeper]), E::NotLiquidatable);
        // S3 at the deadline, FV at the locked cap.
        let slot = env.clock().slot;
        env.set_clock(pos.deadline - 5, slot + 1_500_000);
        let ev = run_to_s3(&mut env, &rm, &keeper, &b, nonce, 30, 30).expect("S3");
        assert_eq!(ev.condition, 3);
        let m = env.market(&rm.market());
        let (p, l) = (m.ema_price.get(), m.ema_liq.get());
        let r_due = rent::due(&pos.pos_rent(), &m.basis()).unwrap();
        let cap = u64::from(pos.venue_fee_hard_cap_bps);
        let at_cap = match &rm.pool {
            AnyPool::V4(_) => Pricing::RayV4(RayV4Fee {
                num: cap,
                den: 10_000,
            }),
            AnyPool::Cpmm(p) => {
                let f = p
                    .fees(&env.svm, shortwin_math::fees::Direction::Buy)
                    .unwrap();
                Pricing::Cpmm {
                    buy: CpmmFees {
                        trade: cap * 100,
                        ..f
                    },
                    sell: CpmmFees {
                        trade: cap * 100,
                        ..f
                    },
                }
            }
        };
        assert_eq!(
            ev.fv,
            fair_value(at_cap, pos.q, p, l, ev.pprime_lo, r_due),
            "{fixture:?}: FV at the locked cap"
        );
    }
}

/// M3-SEC-1 (T-RAY-FEE with a zero fee): a fee of 0 is below every cap. AMM v4 (`SetParams` fees
/// 0/10,000) executes at it: no chain, the pool is observed, closes are fee-free and the risk
/// view equals the execution quote; new opens are refused by the fee-scaled per-position cap
/// (`3·φ·X₀ = 0` at `φ = 0`: `CapPositionImpact`, ECON §6.1), never by a fee-evidence path.
/// CPMM at `t + c_eff = 0` with the fee on the buy's input is no route
/// (`cpmm_zero_fee_disable_matrix`), never fee evidence (`VenueFeeTooHigh`), and its risk view
/// is fee-free, not `+∞`: a healthy position is neither liquidated nor settled, and at the
/// deadline S3 pays FV at that fee-free price (`ray_sec_it` `m3_sec_1*`).
#[test]
fn a_zero_fee_is_below_every_cap() {
    let (mut env, rm, q) = setup(FART);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &rm, &mut b, q);
    set_fee(&mut env, &rm, 0);
    env.warp(1);
    env.crank(&rm);
    let m = env.market(&rm.market());
    assert_eq!((m.no_route_since, m.pool_fail_since), (0, 0));
    assert!(m.b_lo.get() > 0, "observed");
    let mut b2 = env.borrower();
    assert_err(&env.open(&rm, &mut b2, q).0, E::CapPositionImpact);
    env.warp(1);
    let quote = rm.quote_buy(&env.svm, q).unwrap();
    assert_eq!(quote.cost, quote.net, "fee-free");
    let pos = env.position(&rm, &b.key(), nonce);
    let risk = Pricing::RayV4(RayV4Fee {
        num: 0,
        den: 10_000,
    })
    .capped(u64::from(pos.venue_fee_hard_cap_bps));
    let (x, y) = rm.reserves(&env.svm).unwrap();
    assert_eq!(
        risk.buy(q, u128::from(x), u128::from(y)).unwrap().cost,
        quote.cost,
        "risk = execution"
    );
    let (close, _) = close_ok(&mut env, &rm, &b, nonce);
    assert_eq!(close.buyback_cost, quote.cost);
}

/// The borrower's own close under a fee above the hard cap executes at the live fee (closes use
/// the live fee and `venue_executable`, never the cap or the flag; V2-1).
#[test]
fn close_under_a_fee_above_the_hard_cap() {
    for fixture in [FART, USELESS] {
        let (mut env, rm, q) = setup(fixture);
        let mut b = env.borrower();
        let (_, nonce, _) = open_ok(&mut env, &rm, &mut b, q);
        set_fee(&mut env, &rm, 800);
        env.warp(1);
        let b_exp = rm.quote_buy(&env.svm, q).unwrap().cost;
        let (close, _) = close_ok(&mut env, &rm, &b, nonce);
        assert_eq!(close.buyback_cost, b_exp, "{fixture:?}: the live 8 % fee");
    }
}
