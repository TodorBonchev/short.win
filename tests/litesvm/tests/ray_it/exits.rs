//! The keeper exits on the fork (ARCH §3.6, §3.7; M3 exit criteria "fork E2E on Fartcoin (AMM
//! v4) and USELESS (CPMM) through open → liquidation"): a whale's real buy on the real pool
//! doubles the price, the keeper cranks while the EMA follows, and `liquidate` buys the position
//! back on the venue within the fixed bound — the buy leg delivering straight into the lending
//! vault; an expired healthy position is closed by `expire`. (S3 on a halted pool is
//! `route.rs`; the S1 / S2 arithmetic is venue-independent and runs on the mock,
//! `tests/settle_it.rs`.)

use shortwin::events::{Expire, Liquidate};
use shortwin_litesvm_tests::engine::{expire_ix, liquidate_ix};
use shortwin_math::fees::Direction;

use crate::common::*;

/// A whale buy of `sol` lamports of the token on `rm`'s pool (a real swap).
fn whale_buy(env: &mut Env, rm: &RayMarket, sol: u64) {
    let (tok, program) = rm.pool.token();
    let whale = fork::trader(env, &tok, program, sol + 1_000_000_000, 0);
    ok(fork::swap(
        env,
        &rm.pool,
        &whale,
        Direction::Buy,
        fork::Swap::ExactIn(sol, 0),
    ));
}

#[test]
fn liquidation_after_a_pump() {
    for fixture in [FART, USELESS] {
        let (mut env, rm, q) = setup(fixture);
        let mut b = env.borrower();
        let (open, nonce, _) = open_ok(&mut env, &rm, &mut b, q);
        let pos = env.position(&rm, &b.key(), nonce);
        env.tick(&rm, 3);
        // √2.2 − 1 of the SOL reserve lifts the price ≈ 2.2× (constant product).
        let (_, y) = rm.reserves(&env.svm).unwrap();
        whale_buy(&mut env, &rm, y / 1_000 * 484);
        let keeper = env.funded_keypair();
        let mut ev = None;
        for _ in 0..30 {
            env.tick(&rm, 1);
            let pool_vault0 = tokens(&env, &rm.keys.pool_vault);
            let b_exp = rm.quote_buy(&env.svm, q).unwrap().cost;
            let ix = liquidate_ix(&rm, &keeper.pubkey(), &b.key(), nonce);
            let res = env.send(&[ix], &[&keeper]);
            match custom_code(&res) {
                None => {
                    let meta = ok(res);
                    let e: Liquidate = events::one(&meta);
                    assert_eq!(
                        e.buyback_cost, b_exp,
                        "{fixture:?}: B = B̂_live at the live fee"
                    );
                    assert_eq!(e.b_live, b_exp);
                    assert_eq!(tokens(&env, &rm.keys.pool_vault) - pool_vault0, q);
                    ev = Some(e);
                    break;
                }
                Some(c) if c == code(E::NotLiquidatable) || c == code(E::ExitBoundNotMet) => {}
                Some(c) => panic!("{fixture:?}: liquidate failed with {c}"),
            }
        }
        let ev = ev.unwrap_or_else(|| panic!("{fixture:?}: never liquidated"));
        assert!(ev.cr_spot_bps < u64::from(pos.cr_liq_bps));
        assert!(!ev.expired);
        assert!(
            ev.buyback_cost + ev.rent + ev.penalty + ev.keeper_topup + ev.borrower_payout == open.v,
            "the waterfall conserves V"
        );
        assert!(read::account(&env.svm, &pda::position(&rm.market(), &b.key(), nonce).0).is_none());
        assert_eq!(read::token_delegate(&env.svm, &rm.keys.pool_vault), None);
    }
}

#[test]
fn expiry_of_a_healthy_position() {
    for fixture in [FART, USELESS] {
        let (mut env, rm, q) = setup(fixture);
        let mut b = env.borrower();
        let (_, nonce, _) = open_ok(&mut env, &rm, &mut b, q);
        let pos = env.position(&rm, &b.key(), nonce);
        let slot = env.clock().slot;
        env.set_clock(pos.deadline + 1, slot + 1_600_000);
        env.tick(&rm, 3);
        let keeper = env.funded_keypair();
        let pool_vault0 = tokens(&env, &rm.keys.pool_vault);
        let b_exp = rm.quote_buy(&env.svm, q).unwrap().cost;
        let meta = ok(env.send(
            &[expire_ix(&rm, &keeper.pubkey(), &b.key(), nonce)],
            &[&keeper],
        ));
        let e: Expire = events::one(&meta);
        assert_eq!(e.buyback_cost, b_exp, "{fixture:?}");
        assert_eq!(tokens(&env, &rm.keys.pool_vault) - pool_vault0, q);
        assert!(e.expiry_fee > 0 && e.keeper_reward > 0);
    }
}

/// The zero-lamport borrower on AMM v4 (M10 G-H10; `engine::zero_lamport_exits`): with the wallet
/// emptied after the open (A) or inside the exit's transaction (B), the borrower's close and a
/// keeper's expiry land and credit at least the position's and the exposure record's rent.
#[test]
fn a_zero_lamport_borrower_exits_on_amm_v4() {
    shortwin_litesvm_tests::engine::zero_lamport_exits(|| setup(FART), "AMM v4");
}
