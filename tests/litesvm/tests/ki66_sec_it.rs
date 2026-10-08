//! KI-66 security review: adversarial tests of the pot-only touch (a third party's
//! `sync_lender` of a lender behind a rebase record). Every lender instruction runs through the
//! lender lab (`lender_it/common.rs`): checked against the math crate on its pre-state, with the
//! hooks after every transaction.
//!
//! - No double pay: the same market wound down with and without third-party pot-only touches
//!   (repeated, with several `max_epochs`, before and after other lenders' claims, with rent
//!   accruing to the held lender's latent shares after the record) pays every lender the same
//!   total and leaves the same `lender_sol`.
//! - Idempotence across `max_epochs`, read-only records, a slice that skips the lender's epoch,
//!   records out of order or of another epoch.
//! - Locks: a held lender keeps its (expired) lock until its owner converts.
//! - A deficit lender (negative `pending`) behind a rebase record: its pot-only touch moves its
//!   unclaimed realised SOL to the pot exactly as its owner's touch would.
//! - Two rebase records: each epoch is accrued once.
//!
//! Requires the SBF build: `anchor build --tools-version v1.57 --arch v3`.

#![allow(clippy::result_large_err)]

#[allow(dead_code)]
#[path = "lender_it/common.rs"]
mod common;

use anchor_lang::solana_program::instruction::AccountMeta;
use common::{Lab, code};
use shortwin::errors::ShortwinError;
use shortwin_litesvm_tests::errors::custom_code;
use shortwin_litesvm_tests::tokens::TokenProgram;
use shortwin_litesvm_tests::{ix, pda, read};
use shortwin_math::pool::{self, Standing, VIRTUAL_ASSETS, VIRTUAL_SHARES};

const U: u64 = 1_000_000_000;

fn rebase(lab: &mut Lab, pid: &str, sol: u64, lump: u64) {
    let e = lab.market().share_epoch;
    let t = lab.market().tokens_free;
    lab.borrow30(pid, t - 1).unwrap();
    lab.settle(pid, sol, lump);
    assert_eq!(lab.market().share_epoch, e + 1, "{pid}: the epoch rolled");
    let r = *lab.state().records.last().unwrap();
    assert_eq!(r.num, 1, "{pid}: a rebase, not a wipe");
}

fn exit_all(lab: &mut Lab, name: &str) {
    loop {
        let Some(l) = lab.touch(name) else { return };
        let p = lab.market().pool();
        if l.shares == 0 || l.epoch != p.share_epoch {
            return;
        }
        let allow = u128::from(lab.free_allowance(name));
        let c = u128::from(p.tokens_free) + u128::from(p.borrowed_q) + VIRTUAL_ASSETS;
        let svs = p.total_shares + VIRTUAL_SHARES;
        let zero = svs.div_ceil(c) - 1;
        let fit = allow * svs / c;
        let s = l.shares.min(fit.max(zero).max(1));
        lab.withdraw(name, s).unwrap();
    }
}

/// Every lender exits whole and claims, then two more claim passes (the pot reaches the
/// deficit lenders on a later pass). Returns each lender's lamports gained since `base`.
fn wind_down(lab: &mut Lab, names: &[&str], base: &[u64]) -> Vec<u64> {
    for n in names {
        exit_all(lab, n);
        lab.claim(n).unwrap();
    }
    for _ in 0..2 {
        for n in names {
            lab.claim(n).unwrap();
        }
    }
    names
        .iter()
        .zip(base)
        .map(|(n, b)| i128::from(wealth(lab, n)) - i128::from(*b))
        .map(|g| u64::try_from(g.max(0)).expect("gain"))
        .collect()
}

/// `lender_it`'s pot scenario (A leaves early, C joins late and holds the surplus), rolled by a
/// rebase before anyone touched C; A claimed once (short).
fn pot_market() -> Lab {
    let mut lab = Lab::with_cap(TokenProgram::Spl, 10_000);
    lab.deposit("A", 1_000_000_000_000).unwrap();
    lab.deposit("B", 1_000_000_000_000).unwrap();
    lab.borrow30("p", 900_000_000_000).unwrap();
    lab.advance(10 * common::D, 10u128.pow(16));
    let a = lab.free_allowance("A");
    let s = lab.shares_for("A", a);
    lab.withdraw("A", s).unwrap();
    lab.deposit("C", 2_000_000_000_000).unwrap();
    let owed = lab.owed("p");
    lab.repay("p", owed, 0);
    lab.claim("A").unwrap();
    rebase(&mut lab, "q", U, 0);
    assert_eq!(lab.position("C").unwrap().epoch, 0);
    lab
}

const ABC: [&str; 3] = ["A", "B", "C"];

/// `name`'s lamports plus its position account's (the rent `withdraw` refunds when it closes
/// the position, which depends on `pending` being 0 at the exit, not on what is paid).
fn wealth(lab: &mut Lab, name: &str) -> u64 {
    let owner = lab.key(name);
    let pos = pda::lender(&lab.k.market, &owner).0;
    lab.lamports_of(name) + read::lamports(&lab.env.svm, &pos)
}

fn lamports_of(lab: &mut Lab, names: &[&str]) -> Vec<u64> {
    names.iter().map(|n| wealth(lab, n)).collect()
}

/// No double pay: the market wound down by the owners alone, and the same market with a third
/// party's pot-only touches of C (three in a row with `max_epochs` 1, 2 and 0, then again after
/// A's claim) pay A, B and C the same totals and leave the same `lender_sol` and pot.
#[test]
fn pot_only_touches_pay_nobody_twice() {
    // Owners alone.
    let mut x = pot_market();
    let base = lamports_of(&mut x, &ABC);
    let gx = wind_down(&mut x, &ABC, &base);
    let vx = x.vault();
    // A third party touches C, repeatedly.
    let mut y = pot_market();
    let base = lamports_of(&mut y, &ABC);
    for n in [1, 2, 0, 1] {
        assert_eq!(y.sync("C", n), Ok(Standing::Behind), "max_epochs {n}");
    }
    let pot = y.vault().unmatched;
    assert!(pot > 0);
    y.claim("A").unwrap();
    assert_eq!(y.position("A").unwrap().pending, 0, "A paid without C");
    for n in [1, 1] {
        assert_eq!(y.sync("C", n), Ok(Standing::Behind));
    }
    let gy = wind_down(&mut y, &ABC, &base);
    let vy = y.vault();
    eprintln!(
        "owners alone {gx:?} (lender_sol {}, pot {}); with pot-only touches {gy:?} \
         (lender_sol {}, pot {}); pot after the touches {pot}",
        vx.lender_sol, vx.unmatched, vy.lender_sol, vy.unmatched
    );
    assert_eq!(gx, gy, "every lender is paid the same");
    assert_eq!((vx.lender_sol, vx.unmatched), (vy.lender_sol, vy.unmatched));
}

/// No double pay with rent accruing to C's latent (unconverted) shares after the record: a
/// second position borrows after the rebase, rent accrues for 5 days with C held (touched by a
/// third party before and during the accrual), A claims in between, the position repays. The
/// totals equal the owners-alone wind-down's to the lamport (only who waits for the pot may
/// differ in between).
#[test]
fn pot_only_touches_with_post_record_accrual_pay_nobody_twice() {
    let run = |third_party: bool| -> (Vec<u64>, u64, u64) {
        let mut lab = pot_market();
        let base = lamports_of(&mut lab, &ABC);
        if third_party {
            assert_eq!(lab.sync("C", 1), Ok(Standing::Behind));
        }
        lab.deposit("D", 1_000_000_000_000).unwrap();
        let t = lab.market().tokens_free;
        lab.borrow30("r", t / 2).unwrap();
        for _ in 0..5 {
            lab.advance(common::D, 10u128.pow(15));
            if third_party {
                assert_eq!(lab.sync("C", 1), Ok(Standing::Behind));
                lab.claim("A").unwrap();
            }
        }
        let owed = lab.owed("r");
        lab.repay("r", owed, 0);
        if third_party {
            assert_eq!(lab.sync("C", 1), Ok(Standing::Behind));
        }
        let g = wind_down(&mut lab, &ABC, &base);
        let v = lab.vault();
        (g, v.lender_sol, v.unmatched)
    };
    let x = run(false);
    let y = run(true);
    eprintln!("owners alone {x:?}; with pot-only touches {y:?}");
    assert_eq!(x.0.iter().sum::<u64>(), y.0.iter().sum::<u64>(), "Σ paid");
    assert_eq!(x, y, "every lender is paid the same, the same left");
}

/// Idempotence and the record slice: read-only records are accepted (the pot-only touch writes
/// none); a second touch with any `max_epochs` changes nothing; a slice that starts after the
/// lender's epoch passes no record of it (a no-op); records out of order are refused.
#[test]
fn pot_only_touch_record_slices() {
    let mut lab = pot_market();
    // A second rebase, so C is two records behind.
    lab.deposit("D", 1_000_000_000_000).unwrap();
    rebase(&mut lab, "q2", U, 0);
    let m = lab.k.market;
    let c = lab.key("C");
    let payer = lab.env.payer.insecure_clone();
    let ro: Vec<AccountMeta> = (0..2)
        .map(|e| AccountMeta::new_readonly(pda::share_epoch(&m, e).0, false))
        .collect();
    // Expected: the math crate's pot-only touch on the pre-state.
    let st = lab.state();
    let mut want = st.lender_of(&c).unwrap().lender();
    let mut v = *st.vault;
    let recs = st.records.clone();
    assert_eq!(
        pool::touch_pot(&st.pool, &mut v, &mut want, &recs, st.now),
        Ok(Standing::Behind)
    );
    let res = lab.env.send(
        &[ix::with_remaining(ix::sync_lender(&c, &m, 2), &ro)],
        &[&payer],
    );
    assert!(res.is_ok(), "read-only records: {:?}", res.err());
    assert_eq!(lab.position("C").unwrap().lender(), want);
    assert_eq!(lab.vault().unmatched, v.unmatched);
    let (records, held, pot) = (lab.state().records, lab.position("C").unwrap(), v.unmatched);
    for n in [0, 1, 2, 8] {
        assert_eq!(lab.sync("C", n), Ok(Standing::Behind), "max_epochs {n}");
        assert_eq!(lab.position("C").unwrap(), held, "max_epochs {n}");
        assert_eq!(lab.vault().unmatched, pot);
    }
    // Only the record after C's epoch: no record of C's epoch, nothing moves.
    let res = lab.env.send(
        &[ix::with_remaining(
            ix::sync_lender(&c, &m, 1),
            &[AccountMeta::new_readonly(pda::share_epoch(&m, 1).0, false)],
        )],
        &[&payer],
    );
    assert!(res.is_ok(), "{:?}", res.err());
    assert_eq!(lab.position("C").unwrap(), held);
    // Out of order.
    let res = lab.env.send(
        &[ix::with_remaining(
            ix::sync_lender(&c, &m, 2),
            &[ro[1].clone(), ro[0].clone()],
        )],
        &[&payer],
    );
    assert_eq!(
        custom_code(&res),
        Some(code(ShortwinError::EpochRecordMismatch))
    );
    assert_eq!(lab.state().records, records, "no record written");
    assert_eq!(lab.position("C").unwrap(), held);
}

/// A held lender keeps its lock, expired or not, until its owner converts it (the calendar
/// already dropped the term at `lock_until`); after the owner's conversion it binds nothing.
#[test]
fn a_held_lender_keeps_its_expired_lock_until_its_owner_converts() {
    let mut lab = Lab::with_cap(TokenProgram::Spl, 10_000);
    lab.deposit("A", 1_000_000_000_000).unwrap();
    lab.borrow30("p", 900_000_000_000).unwrap();
    lab.deposit("C", 2_000_000_000_000).unwrap();
    let a = lab.free_allowance("C");
    let s = lab.shares_for("C", a / 10);
    lab.withdraw("C", s).unwrap();
    let c0 = lab.position("C").unwrap();
    assert!(
        c0.lock > 0,
        "C withdrew while inventory was borrowed: a lock"
    );
    lab.advance(10 * common::D, 10u128.pow(16));
    let owed = lab.owed("p");
    lab.repay("p", owed, 0);
    rebase(&mut lab, "q", U, 0);
    let held = lab.position("C").unwrap();
    lab.advance(40 * common::D, 0);
    assert!(lab.now() > held.lock_until, "the lock expired");
    assert_eq!(lab.sync("C", 1), Ok(Standing::Behind));
    let after = lab.position("C").unwrap();
    assert_eq!(
        (
            after.lock,
            after.lock_term,
            after.lock_until,
            after.lock_epoch
        ),
        (held.lock, held.lock_term, held.lock_until, held.lock_epoch),
        "the pot-only touch keeps the lock"
    );
    assert_eq!(lab.sync_signed("C", 1), Ok(Standing::Current));
    let st = lab.state();
    let c = st.lender_of(&lab.key("C")).unwrap().lender();
    let den = 1_000u128.pow(u32::from(st.records[0].den_log1000));
    assert_eq!(c.lock, held.lock / den, "the owner's conversion scales it");
    assert_eq!(
        pool::lock_of(&st.vault, &c, st.now),
        Ok(0),
        "no longer binding (expired, or of an earlier Π epoch)"
    );
    exit_all(&mut lab, "C");
    exit_all(&mut lab, "A");
}

/// A deficit lender (negative `pending`) behind a rebase record: the pot-only touch moves its
/// unclaimed realised SOL to the pot (`max(pending, 0) = 0`), as its owner's own touch would;
/// with `cover_deficit` and the wind-down after, every lender is paid what the owners-alone
/// market pays.
#[test]
fn a_deficit_lender_behind_a_rebase_record() {
    let run = |third_party: bool| -> (Vec<u64>, u64, u64, i64) {
        let mut lab = Lab::with_cap(TokenProgram::Token2022, 10_000);
        lab.deposit("B", 1_000_000_000_000).unwrap();
        lab.borrow30("1", 1_000_000_000_000).unwrap();
        lab.advance(20 * common::D, 10u128.pow(17));
        lab.deposit("A", 6_000_000_000_000).unwrap();
        lab.repay("1", 0, 0);
        rebase(&mut lab, "2", 3 * U, 3 * U);
        let names = ["A", "B"];
        let pot0 = lab.vault().unmatched;
        if third_party {
            assert_eq!(lab.sync("A", 1), Ok(Standing::Behind));
            let a = lab.position("A").unwrap();
            assert!(a.pending < 0, "A's pending {}", a.pending);
            assert!(lab.vault().unmatched >= pot0);
        }
        let st = lab.state();
        let a = st.lender_of(&lab.key("A")).unwrap().lender();
        let mut l = a;
        let mut v = *st.vault;
        let mut recs = st.records.clone();
        pool::touch(&st.pool, &mut v, &mut l, &mut recs, st.now).unwrap();
        let neg = l.pending;
        assert!(neg < 0);
        lab.cover("A", neg.unsigned_abs()).unwrap();
        let base = lamports_of(&mut lab, &names);
        let g = wind_down(&mut lab, &names, &base);
        let v = lab.vault();
        (g, v.lender_sol, v.unmatched, neg)
    };
    let x = run(false);
    let y = run(true);
    eprintln!("deficit lender, owners alone {x:?}; with a pot-only touch {y:?}");
    assert_eq!(x, y);
}

/// Two rebase records: the owner converts one record, a third party's touch holds C at the
/// second (accruing C's epoch-1 earnings once), the owner converts the second; the result is the
/// owner's two-record conversion's.
#[test]
fn two_rebase_records_each_epoch_accrued_once() {
    let run = |third_party: bool| -> (Vec<u64>, u64, u64) {
        let mut lab = pot_market();
        lab.deposit("D", 1_000_000_000_000).unwrap();
        let t = lab.market().tokens_free;
        lab.borrow30("r", t / 2).unwrap();
        lab.advance(3 * common::D, 10u128.pow(15));
        let owed = lab.owed("r");
        lab.repay("r", owed, 0);
        lab.deposit("E", 1_000_000_000_000).unwrap();
        rebase(&mut lab, "q2", U, 0);
        let base = lamports_of(&mut lab, &ABC);
        if third_party {
            assert_eq!(lab.sync("C", 2), Ok(Standing::Behind));
            assert_eq!(lab.sync_signed("C", 1), Ok(Standing::Behind));
            assert_eq!(lab.position("C").unwrap().epoch, 1);
            assert_eq!(lab.sync("C", 1), Ok(Standing::Behind));
            assert_eq!(lab.sync("C", 1), Ok(Standing::Behind));
            lab.claim("A").unwrap();
            lab.claim("B").unwrap();
        }
        assert_eq!(lab.sync_signed("C", 2), Ok(Standing::Current));
        let g = wind_down(&mut lab, &ABC, &base);
        let v = lab.vault();
        (g, v.lender_sol, v.unmatched)
    };
    let x = run(false);
    let y = run(true);
    eprintln!("two rebase records, owners alone {x:?}; with pot-only touches {y:?}");
    assert_eq!(x, y);
}
