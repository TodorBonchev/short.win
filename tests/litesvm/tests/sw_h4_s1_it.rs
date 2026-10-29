//! Scenario contributed by Defriza Crypto (defrizacrypto@gmail.com), 2026-10-29, kept as a
//! regression (SW-H4/S1).
//!
//! SW-H4 measurement, executable-venue S1 variant (external pre-beta audit,
//! HEAD 3776f9e; scenario authored 2026-10-29 for this audit, not part of
//! the repo's suite).
//!
//! Hypothesis timing not covered by the repo's JIT regressions (which bundle
//! `[deposit, settle, claim, withdraw]` atomically at `h = 0`, or deposit
//! after a settlement): a depositor enters AFTER a genuine gap has armed
//! the S1 clock and the settlement is one keeper second away — its amount
//! already determined — then holds through the vest like an honest lender.
//!
//! Measured against a no-deposit control from the identical snapshot: the
//! settlement's amounts are unchanged; the depositor's SOL take and token
//! return, valued at the post-gap spot (the price its tokens actually cost
//! at entry); and the honest lender's loss.

#![allow(clippy::result_large_err)]

#[path = "settle_support.rs"]
mod support;

use anchor_lang::prelude::Pubkey;
use litesvm::types::TransactionMetadata;
use shortwin::events::{ClaimSol as ClaimSolEv, Deposit as DepositEv, Withdraw as WithdrawEv};
use shortwin_litesvm_tests::ix::{self, LenderKeys};
use shortwin_math::num::WAD;
use shortwin_math::pool;
use solana_keypair::Keypair;
use solana_signer::Signer;
use support::*;

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

/// Tokens paid to `owner` by the `withdraw` instructions of a transaction.
fn tokens_withdrawn(meta: &TransactionMetadata, owner: &Pubkey) -> u64 {
    events::of::<WithdrawEv>(meta)
        .iter()
        .filter(|e| e.owner == *owner)
        .map(|e| e.amount)
        .sum()
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

/// The most of `owner`'s shares whose tokens are within its allowance now.
fn withdrawable_shares(env: &Env, market: &Pubkey, owner: &Pubkey) -> u128 {
    let pool = env.market(market).pool();
    let v = *env.market_vault(market).vault();
    let l = env
        .lender(market, owner)
        .expect("a lender position")
        .lender();
    let allow = pool::allowance_view(&pool, &v, &l, &[], env.now()).expect("allowance");
    let (mut lo, mut hi) = (0u128, l.shares);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if pool::tokens_for_shares(&pool, mid).unwrap() <= u128::from(allow) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

fn keys_of(mm: &MockMarket, owner: &Pubkey, token_account: &Pubkey) -> LenderKeys {
    LenderKeys {
        owner: *owner,
        mint: mm.mint(),
        token_program: mm.keys.token_program,
        owner_token_account: *token_account,
    }
}

#[test]
fn sw_h4_jit_deposit_one_second_before_an_s1_settle() {
    // The attack scripts' S1 set-up: RC market, 120 s of honest cranks, a
    // genuine gap to 1.05x the S1 boundary.
    let mut sc = Sc::rc();
    sc.env.tick(&sc.mm, 120);
    let kb = sc.s1_boundary();
    let p0 = sc.spot();
    sc.move_to(scale(p0, kb * 1.05));
    // Keeper seconds until the settling one; snapshot the state just before it.
    let mut snap = None;
    let mut honest_ev = None;
    for _ in 0..600 {
        let saved = sc.env.svm.clone();
        let out = sc.second();
        if let Some(ev) = out.settled {
            snap = Some(saved);
            honest_ev = Some(ev);
            break;
        }
    }
    let (snap, honest_ev) = (snap.expect("settles"), honest_ev.expect("settles"));
    assert_eq!(honest_ev.condition, 1, "S1");
    let market_key = sc.mm.market();
    let vest_s = i64::from(sc.env.config().params.lump_vest_s);
    let h = sc.mm.lender.insecure_clone();
    println!(
        "SW-H4/S1 honest settlement: lender_amount {}, lump {}, vest_flushed {}",
        honest_ev.lender_amount, honest_ev.lump, honest_ev.vest_flushed
    );

    // ---- control: settle from the snapshot; H claims after the vest --------
    sc.env.svm = snap.clone();
    let out = sc.second();
    let ev = out.settled.expect("the control settles");
    assert_eq!(
        (ev.lender_amount, ev.lump),
        (honest_ev.lender_amount, honest_ev.lump),
        "the same settlement"
    );
    let vest = sc.market().vest_bal;
    sc.env.warp(vest_s + 1);
    sc.crank();
    assert_eq!(sc.market().vest_bal, 0, "the vest has released");
    let h_control = claim_twice(&mut sc.env, &market_key, &h);
    println!("SW-H4/S1 control: vest {vest}; H claims {h_control}");

    // ---- attack: X deposits as much as H lent, one second before the settle -
    sc.env.svm = snap;
    let d = sc.mm.spec.lend;
    let x = sc.env.actor(&sc.mm, d, 1_000_000_000);
    let xk = keys_of(&sc.mm, &x.key(), &x.base);
    let xkp = x.kp.insecure_clone();
    let dep = ok(sc.env.send(&[ix::deposit(&xk, d, 0)], &[&xkp]));
    let dev: DepositEv = events::one(&dep);
    let frac = dev.shares as f64 / (dev.s_after + pool::VIRTUAL_SHARES) as f64;
    println!(
        "SW-H4/S1 attack: X deposits {d} tokens one second before the settle: {} shares, \
         {frac:.4} of the pool",
        dev.shares
    );
    let out = sc.second();
    let ev = out.settled.expect("the attack run settles");
    assert_eq!(
        (ev.lender_amount, ev.lump),
        (honest_ev.lender_amount, honest_ev.lump),
        "the deposit does not change the settlement"
    );
    println!(
        "SW-H4/S1 attack vest {} (control {vest})",
        sc.market().vest_bal
    );
    sc.env.warp(vest_s + 1);
    sc.crank();
    assert_eq!(sc.market().vest_bal, 0, "the vest has released");
    let mut x_sol = claim_twice(&mut sc.env, &market_key, &xkp);
    let shares = withdrawable_shares(&sc.env, &market_key, &x.key());
    assert!(shares > 0, "X may withdraw");
    let meta = ok(sc.env.send(
        &[
            ix::claim_sol(&x.key(), &market_key),
            ix::withdraw(&xk, shares, 0),
        ],
        &[&xkp],
    ));
    x_sol += sol_paid(&meta, &x.key());
    let x_back = tokens_withdrawn(&meta, &x.key());
    x_sol += claim_twice(&mut sc.env, &market_key, &xkp);
    let h_attack = claim_twice(&mut sc.env, &market_key, &h);
    // X's net in lamports, its token delta valued at the post-gap spot (what
    // its tokens cost at entry and what the returned ones are worth).
    let spot = sc.spot();
    let x_net =
        i128::from(x_sol) + (i128::from(x_back) - i128::from(d)) * (spot as i128) / (WAD as i128);
    println!(
        "SW-H4/S1 attack: X is paid {x_sol} lamports and gets {x_back} of {d} tokens back; \
         net at the post-gap spot {x_net} lamports; H is paid {h_attack} instead of \
         {h_control} (loses {})",
        h_control.saturating_sub(h_attack)
    );
}
