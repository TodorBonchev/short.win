//! KI-65 (ECON §9.5, §9.4 R3B-2 / R4B-4; ARCH §2.3.6, §3.3): no rebase orphans, through the
//! program. A lender's conversion through a rebase record (`1/1,000^k`) mints
//! `⌊(conv_rem + s)/den⌋` and leaves the remainder in the record's `conv_rem`, which every lender
//! instruction that converts writes back (`engine::lender::store_records`), so once every lender
//! has converted they hold exactly the new `S`. Before, each lender was floored alone and up to
//! `n − 1` shares of `S` had no holder: once every real lender left, `S` stayed above 0, the
//! R3B-2 flush never fired and the vest (and every later credit) went to shares nobody could
//! claim (the 24 h fuzz run B: 19,131,900,419 lamports on the mock; the reproducers replay to
//! dust in `tests/fuzz/tests/smoke.rs`).
//!
//! Every lender instruction runs through the lender lab (`lender_it/common.rs`): checked
//! against the math crate on its pre-state — the stored records included — with the hooks
//! after every transaction (I1 is now `Σ shares = S` once every lender is touched on copies).
//! The tests follow the KI-65 spec: the last lender after a rebase is paid the vest and `S`
//! reaches 0; R4B-4's whales (W1–W3, `pool_model.whale_order_demo`); multi-record lazy
//! conversions in one `claim_sol` and in `sync_lender(1)` steps; dust conversions and the
//! V2-12 jump; the writable-record rule (`EpochRecordReadOnly`); a deficit lender converting
//! through `cover_deficit`; re-entry after the flush; `claim_sol ×3`; and the I1 hook against a
//! conversion the program did not store. The `order_*` tests pin the conversion-order residual
//! (security review SEC-1): with permissionless `sync_lender`, Sybil positions or a tuned
//! residue, one party collects up to one unit of a record per position it holds, and where that
//! leaves every other holder at 0 shares it is paid the whole vest — a JIT depositor too, with
//! one Sybil placed before the roll.
//!
//! Requires the SBF build: `anchor build --tools-version v1.57 --arch v3`.

#![allow(clippy::result_large_err)]

#[allow(dead_code)]
#[path = "lender_it/common.rs"]
mod common;

use anchor_lang::solana_program::instruction::AccountMeta;
use common::{Lab, code};
use shortwin::errors::ShortwinError;
use shortwin::events::ClaimSol;
use shortwin_litesvm_tests::errors::custom_code;
use shortwin_litesvm_tests::invariants::check_all;
use shortwin_litesvm_tests::invariants::lender::PoolState;
use shortwin_litesvm_tests::ix::LenderKeys;
use shortwin_litesvm_tests::tokens::TokenProgram;
use shortwin_litesvm_tests::{events, ix, pda};
use shortwin_math::pool::{self, Standing, VIRTUAL_ASSETS, VIRTUAL_SHARES};

/// One SOL.
const U: u64 = 1_000_000_000;
/// The lumps of these tests: 3 SOL.
const LUMP: u64 = 3 * U;
/// The rebase unit of the tests' rolls (`1/1,000`).
const DEN: u128 = 1_000;

/// `name` deposits `amount` tokens, then burns (for 0 tokens) what leaves its shares at `rem`
/// modulo 1,000: a dust residue below the next rebase's unit.
fn dust(lab: &mut Lab, name: &str, amount: u64, rem: u128) -> u128 {
    lab.deposit(name, amount).unwrap();
    let s = lab.position(name).unwrap().shares;
    let extra = (s % DEN + DEN - rem) % DEN;
    if extra > 0 {
        assert_eq!(
            lab.withdraw(name, extra).map(|(t, _)| t),
            Ok(0),
            "a 0-token burn"
        );
    }
    lab.position(name).unwrap().shares
}

/// A settlement of everything but one raw token: `sol` reaches the lenders, `lump` of it
/// vests, and the shares rebase (the share price falls below `R_MAX`'s bound).
fn rebase(lab: &mut Lab, pid: &str, sol: u64, lump: u64) {
    let e = lab.market().share_epoch;
    let t = lab.market().tokens_free;
    lab.borrow30(pid, t - 1).unwrap();
    lab.settle(pid, sol, lump);
    assert_eq!(lab.market().share_epoch, e + 1, "{pid}: the epoch rolled");
    let r = *lab.state().records.last().unwrap();
    assert_eq!(r.num, 1, "{pid}: a rebase, not a wipe");
}

/// The shares every lender holds once touched on copies, through one shared copy of the
/// records (as the program converts them, one transaction after another), and `S`.
fn held_and_s(lab: &Lab) -> (u128, u128) {
    let st = lab.state();
    let mut recs = st.records.clone();
    let mut v = st.vault.clone();
    let mut held = 0;
    for (_, lp) in &st.lenders {
        let mut l = lp.lender();
        let from = PoolState::records_from_mut(&mut recs, lp.epoch);
        pool::touch(&st.pool, &mut v, &mut l, from, st.now).unwrap();
        if l.epoch == st.pool.share_epoch {
            held += l.shares;
        }
    }
    (held, st.pool.total_shares)
}

fn assert_every_share_held(lab: &Lab, ctx: &str) {
    let (held, s) = held_and_s(lab);
    assert_eq!(held, s, "{ctx}: Σ shares once every lender is touched");
}

/// `pool_model.exit_all`: `name` withdraws every share in the largest burns it may (its
/// allowance or, while that is 0, the largest burn that pays 0 tokens). Returns `(tokens, SOL)`.
fn exit_all(lab: &mut Lab, name: &str) -> (u64, u64) {
    let (mut tokens, mut sol) = (0, 0);
    loop {
        let Some(l) = lab.touch(name) else {
            return (tokens, sol);
        };
        let p = lab.market().pool();
        if l.shares == 0 || l.epoch != p.share_epoch {
            return (tokens, sol);
        }
        let allow = u128::from(lab.free_allowance(name));
        let c = u128::from(p.tokens_free) + u128::from(p.borrowed_q) + VIRTUAL_ASSETS;
        let svs = p.total_shares + VIRTUAL_SHARES;
        let zero = svs.div_ceil(c) - 1;
        let fit = allow * svs / c;
        let s = l.shares.min(fit.max(zero).max(1));
        let (t, x) = lab.withdraw(name, s).unwrap();
        tokens += t;
        sol += x;
    }
}

fn conv_rem(lab: &Lab, epoch: u32) -> u128 {
    lab.state().records[epoch as usize].conv_rem
}

/// Two lenders with 600 old shares above a multiple of 1,000 each, H (1 token) and W (9,000
/// tokens), and a rebase that vests a 3 SOL lump: the record's rounding unit (1,200 / 1,000)
/// goes to whoever converts second.
fn rebased_pair(program: TokenProgram) -> Lab {
    let mut lab = Lab::with_cap(program, 10_000);
    assert_eq!(dust(&mut lab, "H", 1, 600), 600);
    assert_eq!(dust(&mut lab, "W", 9_000, 600), 7_199_600);
    rebase(&mut lab, "p", U + LUMP, LUMP);
    assert_eq!(lab.market().total_shares.get(), 7_200);
    assert_eq!(lab.market().vest_bal, LUMP);
    lab
}

/// Spec (2): the last real lender after a rebase, with a 6 h vest running, is paid the vest:
/// its burn takes `S` to 0 (R3B-2). Before KI-65 H's conversion floored to 0 and W's to 7,199,
/// leaving one orphan share in `S`; W's exit was never "the last", and the 3 SOL vest released
/// to the orphan.
#[test]
fn the_last_lender_after_a_rebase_is_paid_the_vest() {
    let mut lab = rebased_pair(TokenProgram::Spl);
    let lam = lab.lamports_of("W");
    // H converts first (claim_sol writes conv_rem 600) and holds nothing of the new epoch.
    lab.claim("H").unwrap();
    assert_eq!(conv_rem(&lab, 0), 600);
    let h = lab.position("H").unwrap();
    assert_eq!(
        (h.shares, h.epoch),
        (0, 1),
        "dust converts to 0 and jumps (V2-12)"
    );
    assert_every_share_held(&lab, "after H");
    // W converts second: 600 + 7,199,600 = 7,200,200 → all 7,200 shares; its exit is the last.
    let (tokens, _) = exit_all(&mut lab, "W");
    assert_eq!(tokens, 1, "the one raw token left");
    let m = lab.market();
    assert_eq!(
        (m.total_shares.get(), m.vest_bal),
        (0, 0),
        "S = 0, the vest flushed"
    );
    assert_eq!(conv_rem(&lab, 0), 200);
    // The 3 SOL vest, W's part of the 1 SOL settled (7,199,600 / 7,200,200 of it) and the
    // position's rent back (the harness payer pays the fees).
    let paid = lab.lamports_of("W") - lam;
    assert!(paid >= LUMP + 999_916_667, "W paid {paid}");
    lab.claim("H").unwrap();
    assert!(lab.vault().lender_sol <= 10, "{}", lab.vault().lender_sol);
}

/// Spec (3) W1: R4B-4's 199× JIT whale around a lump while an honest lender holds a share it
/// has not converted yet: `S` counts H's latent share, so the whale's exit never takes all of
/// `S`, never triggers the flush, and it realises nothing of the lump. Mirrors
/// `pool_model.whale_order_demo` (W1).
#[test]
fn w1_a_jit_whale_takes_nothing_while_a_share_is_latent() {
    let mut lab = Lab::with_cap(TokenProgram::Token2022, 10_000);
    assert_eq!(dust(&mut lab, "H", 1, 0), 1_000);
    lab.deposit("W", 1_000_000).unwrap();
    rebase(&mut lab, "p", U + LUMP, LUMP);
    exit_all(&mut lab, "W");
    lab.claim("W").unwrap();
    assert_eq!(lab.position("H").unwrap().epoch, 0, "H has not converted");
    assert_eq!(lab.market().total_shares.get(), 1, "H's latent share");
    let lam = lab.lamports_of("J");
    lab.deposit("J", 199 * U).unwrap();
    let t = lab.market().tokens_free;
    lab.borrow30("q", t / 2).unwrap();
    lab.repay("q", 0, LUMP);
    let (_, sol) = exit_all(&mut lab, "J");
    lab.claim("J").unwrap();
    assert_eq!(sol, 0);
    assert_eq!(
        lab.lamports_of("J"),
        lam,
        "the whale realises 0 (rent of its position back)"
    );
    assert!(lab.market().total_shares.get() >= 1 && lab.market().vest_bal > 0);
    lab.advance(i64::from(lab.lump_vest_s) + 1, 0);
    let h = lab.claim("H").unwrap();
    assert!(h >= 2 * LUMP, "H is paid both vests: {h}");
    assert_every_share_held(&lab, "end");
}

/// Spec (3) W2: the whale bundles [settlement (vest + rebase) → permissionless `sync_lender`
/// of both lenders in either order → exit]. The record's rounding unit goes to the second
/// converter; nobody gets less than its floor (H's is 0 shares). Order H then W: W holds all of
/// `S` and its exit is paid the 3 SOL vest that was stranded before KI-65; order W then H: H
/// holds the unit and is paid the vest after W leaves.
#[test]
fn w2_the_sync_order_moves_only_the_rounding_unit() {
    for (first, second) in [("H", "W"), ("W", "H")] {
        let mut lab = rebased_pair(TokenProgram::BOTH[usize::from(first == "W")]);
        assert_eq!(lab.sync(first, 1), Ok(Standing::Current));
        assert_every_share_held(&lab, first);
        assert_eq!(lab.sync(second, 1), Ok(Standing::Current));
        let (h, w) = (
            lab.position("H").map_or(0, |l| l.shares),
            lab.position("W").unwrap().shares,
        );
        // Nobody below its floor (H's, ⌊600/1,000⌋, is 0), nobody above it by more than one.
        assert!(
            h <= 1 && (7_199_600 / DEN..=7_199_600 / DEN + 1).contains(&w),
            "H {h}, W {w}"
        );
        assert_eq!(h + w, 7_200);
        assert_eq!(conv_rem(&lab, 0), 200);
        let lam_w = lab.lamports_of("W");
        exit_all(&mut lab, "W");
        lab.claim("W").unwrap();
        let w_sol = lab.lamports_of("W") - lam_w;
        lab.advance(i64::from(lab.lump_vest_s) + 1, 0);
        let h_sol = lab.claim("H").unwrap();
        if first == "H" {
            assert_eq!((h, w), (0, 7_200));
            assert!(w_sol >= LUMP, "W's exit is the last: {w_sol}");
        } else {
            assert_eq!((h, w), (1, 7_199));
            // W: its part of the 1 SOL settled and its position's rent back, not the vest.
            assert!(w_sol < LUMP && h_sol >= LUMP, "W {w_sol}, H {h_sol}");
        }
        assert!(lab.vault().lender_sol <= 10, "{first} first: stranded");
    }
}

/// Spec (3) W3: after the roll the other holder G leaves, so `S` is H's unconverted share; a
/// whale deposits, a 3 SOL sweep vests, the whale leaves: no flush (`S` > the whale's shares),
/// `S` stays at 1, and H, converting later, is paid the vest.
#[test]
fn w3_a_deposit_while_a_share_is_latent_gets_no_flush() {
    let mut lab = rebased_pair(TokenProgram::Spl);
    exit_all(&mut lab, "W");
    lab.claim("W").unwrap();
    assert_eq!(lab.market().total_shares.get(), 1, "H's latent share");
    let lam = lab.lamports_of("J");
    lab.deposit("J", 1_000_000).unwrap();
    lab.sweep_sol(LUMP);
    exit_all(&mut lab, "J");
    lab.claim("J").unwrap();
    assert_eq!(lab.lamports_of("J"), lam, "the whale realises 0");
    assert_eq!(lab.market().total_shares.get(), 1);
    lab.advance(i64::from(lab.lump_vest_s) + 1, 0);
    let h = lab.claim("H").unwrap();
    assert!(h >= 2 * LUMP, "H is paid both vests: {h}");
    assert_eq!(lab.position("H").unwrap().shares, 1);
    assert_every_share_held(&lab, "end");
}

/// Spec (4): a lender three rebase records behind converts through all three in one
/// `claim_sol` (each record's `conv_rem` written, as the math crate computes), another in
/// `sync_lender(1)` steps; `Σ shares = S` after every step.
#[test]
fn multi_record_lazy_conversions() {
    let mut lab = Lab::with_cap(TokenProgram::Token2022, 10_000);
    dust(&mut lab, "A", 1_000_000_000_000, 123);
    dust(&mut lab, "B", 777_777_777_777, 456);
    for i in 0..3 {
        if i > 0 {
            dust(&mut lab, "K", 2_000, 789);
        }
        rebase(&mut lab, &format!("p{i}"), U, 0);
        assert_every_share_held(&lab, &format!("roll {i}"));
    }
    assert_eq!(lab.market().share_epoch, 3);
    let rems =
        |lab: &Lab| -> Vec<u128> { lab.state().records.iter().map(|r| r.conv_rem).collect() };
    let before = rems(&lab);
    lab.claim("A").unwrap();
    let a = lab.position("A").unwrap();
    assert_eq!(a.epoch, 3);
    assert!(
        a.shares > 0,
        "A converted through all three records (no V2-12 jump)"
    );
    assert_ne!(rems(&lab), before, "A's conversion wrote the records");
    assert_every_share_held(&lab, "A converted");
    for step in 1..=3u32 {
        let want = if step == 3 {
            Standing::Current
        } else {
            Standing::Behind
        };
        assert_eq!(lab.sync("B", 1), Ok(want), "step {step}");
        assert_eq!(lab.position("B").unwrap().epoch, step);
        assert_every_share_held(&lab, &format!("B step {step}"));
    }
    lab.claim("K").unwrap();
    let st = lab.state();
    let held: u128 = st.lenders.iter().map(|(_, l)| l.shares).sum();
    assert_eq!(
        held, st.pool.total_shares,
        "everyone converted: no orphan share"
    );
}

/// Spec (5): a dust lender (below one new share) converts to 0 shares and still advances
/// `conv_rem`; the V2-12 jump follows; a later converter collects the unit.
#[test]
fn a_dust_conversion_advances_conv_rem_and_jumps() {
    let mut lab = rebased_pair(TokenProgram::Spl);
    assert_eq!(lab.sync("H", 1), Ok(Standing::Current));
    let h = lab.position("H").unwrap();
    assert_eq!((h.shares, h.epoch), (0, 1));
    assert_eq!(conv_rem(&lab, 0), 600);
    lab.sync("W", 1).unwrap();
    assert_eq!(
        lab.position("W").unwrap().shares,
        7_200,
        "W collects the unit"
    );
    assert_eq!(conv_rem(&lab, 0), 200);
    assert_every_share_held(&lab, "end");
}

/// Spec (6): a record the conversion changes must be writable (`EpochRecordReadOnly`), on every
/// lender instruction that converts (`claim_sol`, `cover_deficit`, `sync_lender`, `deposit`,
/// `withdraw`), and a refusal writes nothing; a read-only record the conversion leaves unchanged (a
/// holding that is a multiple of the unit, with `conv_rem` 0) is accepted.
#[test]
fn a_record_that_needs_writing_must_be_writable() {
    let mut lab = rebased_pair(TokenProgram::Token2022);
    let m = lab.k.market;
    let ro = vec![AccountMeta::new_readonly(pda::share_epoch(&m, 0).0, false)];
    let refused = code(ShortwinError::EpochRecordReadOnly);
    let h = lab.key("H");
    let kp = lab.users["H"].kp.insecure_clone();
    // W (7,199,600 old shares) deposits and withdraws: its conversion changes `conv_rem` too.
    let wk = LenderKeys {
        owner: lab.key("W"),
        mint: lab.k.mint,
        token_program: lab.k.token_program,
        owner_token_account: lab.users["W"].ata,
    };
    let wkp = lab.users["W"].kp.insecure_clone();
    let payer = lab.env.payer.insecure_clone();
    let (records, w) = (lab.state().records, lab.position("W").unwrap());
    for (what, ixn, signer) in [
        ("claim_sol", ix::claim_sol(&h, &m), &kp),
        ("cover_deficit", ix::cover_deficit(&h, &m, 1), &kp),
        ("sync_lender", ix::sync_lender(&h, &m, 1), &payer),
        ("deposit", ix::deposit(&wk, 1_000, 0), &wkp),
        ("withdraw", ix::withdraw(&wk, 1, 0), &wkp),
    ] {
        let res = lab.env.send(&[ix::with_remaining(ixn, &ro)], &[signer]);
        assert_eq!(custom_code(&res), Some(refused), "{what}");
    }
    assert_eq!(lab.state().records, records, "no record written");
    assert_eq!(conv_rem(&lab, 0), 0);
    assert_eq!(lab.position("H").unwrap().epoch, 0, "nothing written");
    assert_eq!(lab.position("W").unwrap(), w, "nothing written");
    // Writable: accepted.
    lab.claim("H").unwrap();
    assert_eq!(conv_rem(&lab, 0), 600);
    // A lender whose conversion leaves `conv_rem` alone (1,000 old shares → 1 new share,
    // remainder 0 → 0) may pass its record read-only.
    let mut lab = Lab::with_cap(TokenProgram::Spl, 10_000);
    assert_eq!(dust(&mut lab, "A", 1, 0), 1_000);
    lab.deposit("B", 1_000_000).unwrap();
    rebase(&mut lab, "p", U, 0);
    let m = lab.k.market;
    let a = lab.key("A");
    let kp = lab.users["A"].kp.insecure_clone();
    let ro = vec![AccountMeta::new_readonly(pda::share_epoch(&m, 0).0, false)];
    let res = lab
        .env
        .send(&[ix::with_remaining(ix::claim_sol(&a, &m), &ro)], &[&kp]);
    assert!(res.is_ok(), "an unchanged record may stay read-only");
    let l = lab.position("A").unwrap();
    assert_eq!((l.shares, l.epoch), (1, 1));
    assert_eq!(conv_rem(&lab, 0), 0);
    assert_every_share_held(&lab, "end");
}

/// Spec (7): a lender with a negative `pending` behind a rebase record: `withdraw` is refused
/// (`NegativePending`, C0-13) and writes nothing; the lender still counts in `S`, so another
/// lender's whole exit is not the last and does not take the vest; `cover_deficit` converts it
/// through the record (writing `conv_rem`), and its own exit is then the last and is paid the
/// vest.
#[test]
fn a_deficit_lender_converts_through_cover_and_blocks_the_flush() {
    let mut lab = Lab::with_cap(TokenProgram::Token2022, 10_000);
    lab.deposit("B", 1_000_000_000_000).unwrap();
    lab.borrow30("1", 1_000_000_000_000).unwrap();
    lab.advance(20 * common::D, 10u128.pow(17));
    dust(&mut lab, "A", 6_000_000_000_000, 321);
    lab.repay("1", 0, 0); // the whole credited rent is debited pro-rata, A included
    rebase(&mut lab, "2", LUMP, LUMP);
    let a = lab.position("A").unwrap();
    assert_eq!(a.epoch, 0);
    let rem0 = conv_rem(&lab, 0);
    let neg = code(ShortwinError::NegativePending);
    assert_eq!(lab.withdraw("A", 1), Err(neg));
    assert_eq!(conv_rem(&lab, 0), rem0, "a refusal writes no record");
    // B leaves whole: A's (latent) shares keep S above B's, so B is not paid the vest.
    let vest = lab.market().vest_bal;
    exit_all(&mut lab, "B");
    assert!(lab.market().total_shares.get() > 0 && lab.market().vest_bal == vest);
    // A covers its deficit (converting through the record), then leaves last with the vest.
    let st = lab.state();
    let mut l = a.lender();
    let mut v = st.vault.clone();
    let mut recs = st.records.clone();
    pool::touch(&st.pool, &mut v, &mut l, &mut recs, st.now).unwrap();
    assert!(l.pending < 0, "A's pending {}", l.pending);
    lab.cover("A", l.pending.unsigned_abs()).unwrap();
    assert_eq!(lab.position("A").unwrap().epoch, 1);
    assert_ne!(conv_rem(&lab, 0), rem0, "cover_deficit wrote the record");
    let lam = lab.lamports_of("A");
    exit_all(&mut lab, "A");
    let m = lab.market();
    assert_eq!((m.total_shares.get(), m.vest_bal), (0, 0));
    assert!(lab.lamports_of("A") - lam >= vest, "A is paid the vest");
}

/// Spec (8): after the last exit flushed the vest (`S = 0`), a new depositor is a first
/// depositor (virtual shares only, `vest_bal = 0`), with nothing of the old epoch's SOL.
#[test]
fn re_entry_after_the_flush_is_a_first_deposit() {
    let mut lab = rebased_pair(TokenProgram::Spl);
    lab.claim("H").unwrap();
    exit_all(&mut lab, "W");
    lab.claim("H").unwrap();
    let m = lab.market();
    assert_eq!((m.total_shares.get(), m.vest_bal), (0, 0));
    let p = m.pool();
    let want = pool::shares_for_tokens(&p, 1_000_000).unwrap();
    assert_eq!(lab.deposit("N", 1_000_000), Ok(want));
    assert_eq!(lab.market().vest_bal, 0);
    lab.advance(i64::from(lab.lump_vest_s) + 1, 0);
    assert_eq!(lab.claim("N"), Ok(0));
    assert_every_share_held(&lab, "end");
}

/// Spec (9): `claim_sol ×3` in one transaction: the first converts the lender and writes the
/// record; the others write nothing and pay 0 (V2-2).
#[test]
fn claim_sol_three_times_converts_once() {
    let mut lab = rebased_pair(TokenProgram::Token2022);
    lab.advance(i64::from(lab.lump_vest_s) + 1, 0);
    let m = lab.k.market;
    let h = lab.key("H");
    let kp = lab.users["H"].kp.insecure_clone();
    let metas = ix::epoch_records(&m, 0, 1);
    let one = ix::with_remaining(ix::claim_sol(&h, &m), &metas);
    let meta = lab
        .env
        .send(&[one.clone(), one.clone(), one], &[&kp])
        .expect("claim_sol ×3");
    let paid: Vec<u64> = events::of::<ClaimSol>(&meta)
        .iter()
        .map(|e| e.sol_paid)
        .collect();
    assert_eq!(paid.len(), 3);
    assert!(paid[0] > 0 && paid[1] == 0 && paid[2] == 0, "{paid:?}");
    assert_eq!(conv_rem(&lab, 0), 600, "converted once");
    let before = lab.state().records;
    assert_eq!(lab.claim("H"), Ok(0));
    assert_eq!(
        lab.state().records,
        before,
        "a current lender writes no record"
    );
}

/// Spec (10): the hook against the `store_records` mutant. A lender instruction that converted
/// a lender through a rebase record but did not store the record's `conv_rem` (simulated by
/// restoring the record after the conversion) leaves a share without a holder (or one too
/// many): I1 (`Σ shares = S` once every lender is touched) catches it.
#[test]
fn a_conversion_not_stored_is_caught_by_i1() {
    let mut lab = rebased_pair(TokenProgram::Spl);
    let unconverted = lab.state().records[0];
    lab.claim("H").unwrap();
    assert!(check_all(&lab.env.svm, &lab.k.market).is_ok());
    lab.write_record(&unconverted);
    let v = check_all(&lab.env.svm, &lab.k.market).unwrap_err();
    assert_eq!(v.invariant, "I1", "{v}");
    assert!(v.detail.contains("!= S"), "{v}");
    let (held, s) = held_and_s(&lab);
    assert_eq!((held, s), (7_199, 7_200));
}

// ---- Conversion order (security review SEC-1): the residual by design, pinned -----------------
//
// `sync_lender` is permissionless, so anyone may choose the order in which other lenders
// convert through a record, and a party may hold several positions or tune its own residue.
// It can then collect every rounding unit of the record that its positions can receive (one per
// position, at most the record's `n − 1`), never pushing another lender below its floor
// `⌊s/den⌋`. Where those units leave every other holder at 0 shares (every other holder had
// less than one post-rebase share), the party holds all of `S`, and its last burn is paid the
// whole unreleased vest and later lumps (R3B-2) — including a JIT depositor's, with one Sybil
// placed before the roll. Before KI-65 that SOL was stranded (claimable by nobody). These tests
// pin the behaviour; the alternative (the owner's signature on a `sync_lender` that converts
// through a rebase record) is an owner decision (docs/audit/known-issues.md KI-65).

fn shares(lab: &mut Lab, name: &str) -> u128 {
    lab.position(name).map_or(0, |l| l.shares)
}

/// Honest dust lenders H1, H2 (999 old shares each: 1.998 units together) and an attacker
/// with a whale W and two 1-share Sybils A1, A2 (0.002 units), then a rebase that vests a lump.
fn sybil_market() -> (Lab, u128) {
    let mut lab = Lab::with_cap(TokenProgram::Spl, 10_000);
    assert_eq!(dust(&mut lab, "H1", 1, 999), 999);
    assert_eq!(dust(&mut lab, "H2", 1, 999), 999);
    assert_eq!(dust(&mut lab, "A1", 1, 1), 1);
    assert_eq!(dust(&mut lab, "A2", 1, 1), 1);
    let w = dust(&mut lab, "W", 9_000, 0);
    rebase(&mut lab, "p", U + LUMP, LUMP);
    assert_eq!(
        lab.market().total_shares.get(),
        (999 + 999 + 1 + 1 + w) / DEN
    );
    assert_eq!(lab.market().vest_bal, LUMP);
    (lab, w)
}

/// The SOL `names` receive from now on, after each exits whole and claims.
fn exit_and_claim(lab: &mut Lab, names: &[&str]) -> u64 {
    let lam: Vec<u64> = names.iter().map(|n| lab.lamports_of(n)).collect();
    for n in names {
        exit_all(lab, n);
        lab.claim(n).unwrap();
    }
    names
        .iter()
        .zip(&lam)
        .map(|(n, l)| lab.lamports_of(n) - l)
        .sum()
}

/// The bound counts positions, not actors: interleaving permissionless `sync_lender` calls
/// (H1, A1, H2, A2) hands each of the attacker's Sybils a unit right after an honest lender
/// left the remainder at 999, so the attacker's 2 old shares receive both of the record's units
/// and the honest lenders' 1,998 none (their floor, 0). The attacker then holds all of `S` and
/// its last exit is paid the whole running vest. The honest-first order leaves one unit with
/// H2, whose stay blocks the flush and who is paid the vest.
#[test]
fn order_a_sybil_interleaving_collects_every_unit_of_the_record() {
    let (mut lab, w) = sybil_market();
    for n in ["H1", "A1", "H2", "A2", "W"] {
        assert_eq!(lab.sync(n, 1), Ok(Standing::Current), "{n}");
        assert_every_share_held(&lab, n);
    }
    let got: Vec<u128> = ["H1", "H2", "A1", "A2", "W"]
        .iter()
        .map(|n| shares(&mut lab, n))
        .collect();
    assert_eq!(got, vec![0, 0, 1, 1, w / DEN], "adversarial order");
    let gain = exit_and_claim(&mut lab, &["A1", "A2", "W"]);
    assert_eq!(
        lab.market().total_shares.get(),
        0,
        "the attacker's last burn took S to 0"
    );
    assert!(
        gain >= LUMP,
        "the attacker's exits are paid the vest: {gain}"
    );
    lab.advance(i64::from(lab.lump_vest_s) + 1, 0);
    let honest = lab.claim("H1").unwrap() + lab.claim("H2").unwrap();
    assert!(
        honest < U / 1_000,
        "the honest lenders get none of it: {honest}"
    );
    assert!(lab.vault().lender_sol <= 10, "nothing stranded");
    eprintln!("Sybil order: attacker {gain} lamports, honest {honest}");

    // Honest-first order: H2 keeps a unit and the attacker's exit is not the last.
    let (mut lab, w) = sybil_market();
    for n in ["H1", "H2", "A1", "A2", "W"] {
        assert_eq!(lab.sync(n, 1), Ok(Standing::Current), "{n}");
    }
    let got: Vec<u128> = ["H1", "H2", "A1", "A2", "W"]
        .iter()
        .map(|n| shares(&mut lab, n))
        .collect();
    assert_eq!(got, vec![0, 1, 0, 1, w / DEN], "honest-first order");
    let gain = exit_and_claim(&mut lab, &["A1", "A2", "W"]);
    assert_eq!(lab.market().total_shares.get(), 1, "H2's unit");
    lab.advance(i64::from(lab.lump_vest_s) + 1, 0);
    let h2 = lab.claim("H2").unwrap();
    assert!(gain < LUMP && h2 >= LUMP, "attacker {gain}, H2 {h2}");
    eprintln!("honest-first order: attacker {gain} lamports, H2 {h2}");
}

/// Without Sybils: a whale that tunes its own residue (`s_W ≡ 1 mod 1,000`) and syncs H1,
/// itself, then H2 collects the honest dust lenders' unit (1,998 old shares between them) and
/// holds all of `S`; its exit is the last and is paid the running vest.
#[test]
fn order_a_residue_tuned_whale_collects_the_unit_without_sybils() {
    let mut lab = Lab::with_cap(TokenProgram::Spl, 10_000);
    assert_eq!(dust(&mut lab, "H1", 1, 999), 999);
    assert_eq!(dust(&mut lab, "H2", 1, 999), 999);
    let w = dust(&mut lab, "W", 9_000, 1);
    rebase(&mut lab, "p", U + LUMP, LUMP);
    for n in ["H1", "W", "H2"] {
        assert_eq!(lab.sync(n, 1), Ok(Standing::Current), "{n}");
    }
    let got = (
        shares(&mut lab, "H1"),
        shares(&mut lab, "H2"),
        shares(&mut lab, "W"),
    );
    assert_eq!(got, (0, 0, w / DEN + 1));
    assert_eq!(shares(&mut lab, "W"), lab.market().total_shares.get());
    let gain = exit_and_claim(&mut lab, &["W"]);
    assert!(gain >= LUMP, "W's exit is the last: {gain}");
    lab.advance(i64::from(lab.lump_vest_s) + 1, 0);
    let honest = lab.claim("H1").unwrap() + lab.claim("H2").unwrap();
    assert!(
        honest < U / 1_000,
        "the honest lenders get none of the vest: {honest}"
    );
    assert!(lab.vault().lender_sol <= 10);
    eprintln!("residue-tuned whale: {gain} lamports, honest {honest}");
}

/// W3 with one Sybil placed before the roll: H (600 old shares) is synced first, the
/// attacker's dust position A (400) collects the unit, the honest whale G leaves. Then a JIT
/// depositor J and a 3 SOL sweep: A burns its share, and J's exit takes `S` to 0 with no time
/// passing and is paid the whole lump (R3B-2), while H keeps a position with 0 shares. With H
/// synced after A (or never), H holds the unit and J realises 0
/// (`w3_a_deposit_while_a_share_is_latent_gets_no_flush`).
#[test]
fn order_w3_with_a_sybil_the_jit_depositor_is_paid_the_lump() {
    let mut lab = Lab::with_cap(TokenProgram::Spl, 10_000);
    assert_eq!(dust(&mut lab, "H", 1, 600), 600);
    assert_eq!(dust(&mut lab, "A", 1, 400), 400);
    let g = dust(&mut lab, "G", 9_000, 0);
    rebase(&mut lab, "p", U + LUMP, LUMP);
    assert_eq!(lab.market().total_shares.get(), (600 + 400 + g) / DEN);
    for n in ["H", "A", "G"] {
        assert_eq!(lab.sync(n, 1), Ok(Standing::Current), "{n}");
    }
    assert_eq!((shares(&mut lab, "H"), shares(&mut lab, "A")), (0, 1));
    exit_and_claim(&mut lab, &["G"]);
    assert_eq!(lab.market().total_shares.get(), 1, "A's unit");
    // A collects the first vest as the only holder; then the JIT round, at one instant.
    lab.advance(i64::from(lab.lump_vest_s) + 1, 0);
    lab.claim("A").unwrap();
    let lam = lab.lamports_of("J");
    lab.deposit("J", 1_000_000).unwrap();
    lab.sweep_sol(LUMP);
    exit_and_claim(&mut lab, &["A"]);
    exit_and_claim(&mut lab, &["J"]);
    let gain = lab.lamports_of("J") - lam;
    assert_eq!(lab.market().total_shares.get(), 0);
    assert!(gain >= LUMP - 10, "J is paid the lump: {gain}");
    eprintln!("JIT depositor with a Sybil: {gain} lamports of a {LUMP} lump");
    assert_eq!(lab.position("H").unwrap().shares, 0);
}
