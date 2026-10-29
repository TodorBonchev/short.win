//! Regression for the I-09 S3-wait extension. Scenario contributed by Defriza Crypto
//! (defrizacrypto@gmail.com), 2026-10-29; adapted to assert the fix: `deposit` is refused with
//! `DepositDuringNoRoute` while the no-route chain runs with a position open, and the honest
//! lender H is paid exactly the no-deposit control.
//!
//! SW-H4 measurement, None-market variant (external pre-beta audit, HEAD
//! 3776f9e; scenario authored 2026-10-29 for this audit, not part of the
//! repo's suite).
//!
//! Hypothesis (the timing the repo's JIT regressions do not cover): the
//! repo's JIT tests bundle `[deposit, settle, claim, withdraw]` atomically at
//! `h = 0`, or deposit AFTER a settlement. Here a depositor enters BEFORE
//! the settlement, during a window in which the settlement's outcome is
//! already fully determined, and holds through the vest.
//!
//! The sharpest such window is a `None` market's no-route chain: the coin
//! graduated elsewhere, the price state is FROZEN at the pre-migration EMA
//! point, and an open position will settle S3 at that frozen point once
//! `s3_wait_s` runs out — its `lender_amount` is computable in advance by
//! anyone. A holder of the (elsewhere crashed, cheap) mint can deposit into
//! the pool during the wait, mint shares at token parity against claims
//! priced at the frozen EMA, and share the SOL compensation pro rata.
//!
//! Measured (program-side quantities, exact): the settlement is identical
//! with and without the deposit; the depositor's SOL take and token return;
//! the honest lender's loss against the no-deposit control. The depositor's
//! tokens are planted (`fund_token`), standing for tokens acquired on the
//! venue the coin migrated to, at its post-migration price — the external
//! precondition this scenario cannot price in-fork.

use shortwin::errors::ShortwinError;
use shortwin::events::{ClaimSol as ClaimSolEv, Withdraw as WithdrawEv};
use shortwin::state::MARKET_FLAG_GRADUATED_ELSEWHERE;
use shortwin_litesvm_tests::ix::{self, LenderKeys};
use solana_keypair::Keypair;

use crate::common::*;
use crate::graduation::{graduated_elsewhere_with_a_position, migrate_elsewhere};

/// SOL paid to `owner` by the `withdraw` and `claim_sol` instructions of a transaction.
fn sol_paid(meta: &TransactionMetadata, owner: &Pubkey) -> u64 {
    let w: u64 = events::of::<WithdrawEv>(meta)
        .iter()
        .filter(|e| e.owner == *owner)
        .map(|e| e.sol_paid)
        .sum();
    let c: u64 = events::of::<ClaimSolEv>(meta)
        .iter()
        .filter(|e| e.owner == *owner)
        .map(|e| e.sol_paid)
        .sum();
    w + c
}

/// `claim_sol` by `owner` with the records it is behind on (must land); 0 without a position.
fn claim(env: &mut Env, market: &Pubkey, owner: &Keypair) -> u64 {
    let Some(l) = env.lender(market, &owner.pubkey()) else {
        return 0;
    };
    let m = env.market(market);
    let recs = ix::epoch_records(market, l.epoch, m.share_epoch);
    let ix = ix::with_remaining(ix::claim_sol(&owner.pubkey(), market), &recs);
    let meta = ok(env.send(&[ix], &[owner]));
    sol_paid(&meta, &owner.pubkey())
}

/// Two rounds of [`claim`] (a surplus reaches the pot at its owner's touch).
fn claim_twice(env: &mut Env, market: &Pubkey, owner: &Keypair) -> u64 {
    claim(env, market, owner) + claim(env, market, owner)
}

/// Warps to the first moment the S3 is due and settles it; returns the
/// settlement event. S3 requires the position expired (or liquidatable) and
/// `now >= max(no_route_since, graduated_at + grad_grace_s) + s3_wait_s` on
/// the position's locked timers (`shortwin_math::settle`'s chain rule).
fn settle_s3(
    env: &mut Env,
    pm: &PumpMarket,
    b: &Pubkey,
    nonce: u64,
) -> shortwin::events::SolSettle {
    let kp = keeper(env);
    let none = AsNone(pm);
    let pos = env.position(pm, b, nonce);
    let m = market(env, pm);
    assert!(m.no_route_since > 0, "the no-route chain runs");
    let start = m
        .no_route_since
        .max(m.graduated_at + i64::from(pos.grad_grace_s));
    let due = start + i64::from(pos.s3_wait_s);
    let target = due.max(pos.deadline);
    let now = env.now();
    if target > now {
        env.warp(target - now);
    }
    crank(env, &none);
    for _ in 0..40 {
        let meta = ok(settle(env, &none, &kp, b, nonce));
        if let Some(ev) = settled(&meta) {
            assert_eq!(ev.condition, 3, "S3");
            return ev;
        }
        env.warp(30);
        crank(env, &none);
    }
    panic!("the S3 did not execute once due");
}

#[test]
fn sw_h4_jit_deposit_during_the_none_markets_s3_wait() {
    // None with one position open (graduated elsewhere); the S3 chain runs.
    let (mut env, pm, b, nonce, q) = graduated_elsewhere_with_a_position();
    migrate_elsewhere(&mut env, &pm);
    let market_key = pm.market();
    let m0 = market(&env, &pm);
    assert_eq!(m0.venue_kind(), Some(VenueKind::None));
    assert!(m0.has_flag(MARKET_FLAG_GRADUATED_ELSEWHERE));
    let vest_s = i64::from(env.config().params.lump_vest_s);
    let h = pm.lender.insecure_clone();
    let ready = env.svm.clone();

    // ---- control: the S3 alone; H claims after the vest ---------------------
    let ctl = settle_s3(&mut env, &pm, &b.key(), nonce);
    let m = market(&env, &pm);
    let vest = m.vest_bal;
    assert!(vest > 0, "the S3 take vests on a non-executable market");
    env.warp(vest_s + 1);
    crank(&mut env, &AsNone(&pm));
    assert_eq!(market(&env, &pm).vest_bal, 0, "the vest has released");
    let h_control = claim_twice(&mut env, &market_key, &h);
    println!(
        "SW-H4 control: S3 lender_amount {}, lump {}, vest {vest}; H claims {h_control}",
        ctl.lender_amount, ctl.lump
    );

    // ---- attack: X deposits during the wait, then the same S3 --------------
    env.svm = ready;
    let x = env.funded_keypair();
    let d = 10 * q;
    let x_acc = fork::fund_token(
        &mut env,
        &x.pubkey(),
        &pm.keys.mint,
        pm.curve.token_program,
        d,
    );
    let keys = LenderKeys {
        owner: x.pubkey(),
        mint: pm.keys.mint,
        token_program: pm.keys.token_program,
        owner_token_account: x_acc,
    };
    let dep = env.send(&[ix::deposit(&keys, d, 0)], &[&x]);
    assert!(
        market(&env, &pm).no_route_since > 0,
        "the deposit lands inside the S3 wait"
    );
    assert_err(&dep, ShortwinError::DepositDuringNoRoute);
    println!("SW-H4 attack: X's deposit of {d} tokens during the S3 wait is refused");
    let ev = settle_s3(&mut env, &pm, &b.key(), nonce);
    assert_eq!(
        ev.lender_amount, ctl.lender_amount,
        "the settlement is unchanged"
    );
    assert_eq!(ev.lump, ctl.lump);
    assert_eq!(market(&env, &pm).vest_bal, vest);
    env.warp(vest_s + 1);
    crank(&mut env, &AsNone(&pm));
    assert_eq!(market(&env, &pm).vest_bal, 0, "the vest has released");
    assert!(
        env.lender(&market_key, &x.pubkey()).is_none(),
        "X holds no shares"
    );
    let h_attack = claim_twice(&mut env, &market_key, &h);
    println!("SW-H4 attack: H is paid {h_attack} (control {h_control})");
    assert_eq!(h_attack, h_control, "H is made whole");
}
