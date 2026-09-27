//! Each lender instruction on both token programs: the happy paths, dust and zero amounts,
//! slippage bounds, refusals, account and record checks, the M1 prologue (params sync, rent
//! accrual), `claim_sol` / `cover_deficit` / `sync_lender` not writing `Market` (C0-17),
//! `sweep_protocol_fees`, the mint guard on both paths, and the CU of each instruction.

use anchor_lang::error::ErrorCode as AnchorErrorCode;
use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::AccountMeta;
use shortwin::errors::ShortwinError;
use shortwin::state::{ConfigChange, MARKET_FLAG_MINT_UNSAFE, Market, ShareEpoch, VenueKind};
use shortwin_litesvm_tests::errors::{assert_anchor_err, assert_err, ok};
use shortwin_litesvm_tests::ix::{self, LenderKeys};
use shortwin_litesvm_tests::tokens::TokenProgram;
use shortwin_litesvm_tests::{events, pda, read};
use shortwin_math::num::WAD;
use shortwin_math::pool::{self, Standing};
use shortwin_math::rent;
use solana_signer::Signer;

use crate::common::{D, Lab, code, propose_and_apply, write_zc};

fn lender_keys(lab: &mut Lab, name: &str) -> LenderKeys {
    let k = lab.k;
    let u = lab.user(name);
    LenderKeys {
        owner: u.kp.pubkey(),
        mint: k.mint,
        token_program: k.token_program,
        owner_token_account: u.ata,
    }
}

#[test]
fn deposit_and_withdraw_round_trip_on_both_token_programs() {
    for program in TokenProgram::BOTH {
        let mut lab = Lab::new(program);
        // First deposit: ⌊a·(0 + 1,000)/(0 + 1)⌋ (VS = 1,000, VA = 1; ECON §9.2).
        assert_eq!(lab.deposit("A", 1_000_000), Ok(1_000_000_000));
        let b = lab.deposit("B", 3_000_000).unwrap();
        assert_eq!(b, 3_000_000u128 * (1_000_000_000 + 1_000) / (1_000_000 + 1));
        let m = lab.market();
        assert_eq!(
            (m.tokens_free, m.total_shares.get()),
            (4_000_000, 1_000_000_000 + b)
        );
        assert_eq!(lab.pool_vault_amount(), 4_000_000);
        let a = lab.position("A").unwrap();
        assert_eq!(
            (a.shares, a.epoch, a.lock, a.pending),
            (1_000_000_000, 0, 0, 0)
        );
        assert_eq!((a.lock_pi, a.lock_epoch), (WAD, pool::NO_LOCK_EPOCH));

        // Nothing borrowed: everyone may take everything, pro rata or not (SPEC §2.4).
        let a_shares = lab.position("A").unwrap().shares;
        assert_eq!(lab.withdraw("A", a_shares), Ok((1_000_000, 0)));
        // shares, pending and lock all 0: the position is closed and its rent refunded.
        assert!(lab.position("A").is_none());
        let b_shares = lab.position("B").unwrap().shares;
        let (tok, sol) = lab.withdraw("B", b_shares).unwrap();
        assert_eq!((tok, sol), (3_000_000, 0));
        let m = lab.market();
        assert_eq!((m.tokens_free, m.total_shares.get()), (0, 0));
        // A new deposit into the empty pool works again and the position is re-created.
        assert!(lab.deposit("A", 10).is_ok());
        assert!(lab.position("A").is_some());
    }
}

#[test]
fn zero_dust_and_slippage_bounds() {
    for program in TokenProgram::BOTH {
        let mut lab = Lab::new(program);
        let zero = code(ShortwinError::ZeroAmount);
        assert_eq!(lab.deposit("A", 0), Err(zero));
        // One raw unit mints VS shares.
        assert_eq!(lab.deposit("A", 1), Ok(1_000));
        assert_eq!(lab.deposit("B", 5), Ok(5 * 2_000 / 2));
        // min_shares above what the deposit mints (5·7,000/7).
        assert_eq!(
            lab.deposit_min("B", 5, 5_001),
            Err(code(ShortwinError::SharesBelowMin))
        );
        // A dust burn: 1 share is worth ⌊1·(T + VA)/(S + VS)⌋ = 0 tokens; allowed (it is within
        // the allowance), and the owner's min_tokens protects against it.
        assert_eq!(
            lab.withdraw_min("A", 1, 1),
            Err(code(ShortwinError::TokensBelowMin))
        );
        assert_eq!(lab.withdraw("A", 1), Ok((0, 0)));
        // Zero shares, and more shares than held.
        assert_eq!(
            lab.withdraw("A", 0),
            Err(code(ShortwinError::InsufficientShares))
        );
        let held = lab.position("A").unwrap().shares;
        assert_eq!(
            lab.withdraw("A", held + 1),
            Err(code(ShortwinError::InsufficientShares))
        );
        // Nothing to claim pays 0; a zero cover is refused; a one-lamport cover goes in and
        // comes back out as the lender's own SOL.
        assert_eq!(lab.claim("A"), Ok(0));
        assert_eq!(lab.cover("A", 0), Err(zero));
        assert_eq!(lab.cover("A", 1), Ok(()));
        assert_eq!(lab.claim("A"), Ok(1));
        assert_eq!(lab.claim("A"), Ok(0));
    }
}

#[test]
fn only_the_owner_moves_its_position() {
    let mut lab = Lab::new(TokenProgram::Spl);
    lab.deposit("A", 1_000_000).unwrap();
    let thief = lab.env.funded_keypair();
    let mut lk = lender_keys(&mut lab, "A");
    let a = lk.owner;
    // The thief signs with A's position in place of its own: the seeds bind the signer.
    lk.owner = thief.pubkey();
    let mut ix = ix::withdraw(&lk, 1, 0);
    ix.accounts[4].pubkey = pda::lender(&lab.k.market, &a).0;
    let res = lab.env.send(&[ix], &[&thief]);
    assert_anchor_err(&res, AnchorErrorCode::ConstraintSeeds);
    let mut ix = ix::cover_deficit(&thief.pubkey(), &lab.k.market, 1);
    ix.accounts[2].pubkey = pda::lender(&lab.k.market, &a).0;
    assert_anchor_err(
        &lab.env.send(&[ix], &[&thief]),
        AnchorErrorCode::ConstraintSeeds,
    );
    // A user without a position cannot withdraw or claim.
    let res = lab.env.send(&[ix::withdraw(&lk, 1, 0)], &[&thief]);
    assert_anchor_err(&res, AnchorErrorCode::AccountNotInitialized);
    let res = lab
        .env
        .send(&[ix::claim_sol(&thief.pubkey(), &lab.k.market)], &[&thief]);
    assert_anchor_err(&res, AnchorErrorCode::AccountNotInitialized);
    // The pool vault is the market's own; another token account of the mint is refused.
    let mut lk = lender_keys(&mut lab, "A");
    lk.owner_token_account = lab.user("B").ata;
    let owner = lab.users["A"].kp.insecure_clone();
    // (B's account as the source: the token program refuses A's signature.)
    assert!(lab.env.send(&[ix::deposit(&lk, 10, 0)], &[&owner]).is_err());
    // Another token account in place of the pool vault.
    let mut ix = ix::withdraw(&lender_keys(&mut lab, "A"), 1, 0);
    ix.accounts[7].pubkey = lab.user("B").ata;
    assert!(lab.env.send(&[ix], &[&owner]).is_err());
}

/// Rolls one share epoch with a full wipe (an injected SOL settlement of all borrowed
/// tokens), leaving `A` parked on the live wipe record and `B` (who deposits after the
/// post-wipe sweep) on epoch 1.
fn wiped_lab() -> Lab {
    let mut lab = Lab::with_cap(TokenProgram::Spl, 10_000);
    lab.deposit("A", 1_000_000_000).unwrap();
    lab.borrow30("p", 1_000_000_000).unwrap();
    lab.settle("p", 5_000_000_000, 0);
    assert_eq!(lab.market().share_epoch, 1);
    lab
}

#[test]
fn share_epoch_records_are_key_checked() {
    let mut lab = wiped_lab();
    let m = lab.k.market;
    let a = lab.key("A");
    let owner = lab.users["A"].kp.insecure_clone();
    let good = pda::share_epoch(&m, 0).0;
    let record_ix = |extra: Vec<AccountMeta>| ix::with_remaining(ix::claim_sol(&a, &m), &extra);

    // Missing: the lender is behind.
    let res = lab.env.send(&[ix::claim_sol(&a, &m)], &[&owner]);
    assert_err(&res, ShortwinError::LenderBehindEpochs);

    // A copy of the record at another address (program-owned, right discriminator).
    let mut acc = lab.env.svm.get_account(&good).unwrap();
    let fake = Pubkey::new_unique();
    lab.env.svm.set_account(fake, acc.clone()).unwrap();
    let res = lab.env.send(
        &[record_ix(vec![AccountMeta::new_readonly(fake, false)])],
        &[&owner],
    );
    assert_err(&res, ShortwinError::EpochRecordMismatch);
    // The same bytes owned by another program.
    acc.owner = Pubkey::new_unique();
    let foreign = Pubkey::new_unique();
    lab.env.svm.set_account(foreign, acc).unwrap();
    let res = lab.env.send(
        &[record_ix(vec![AccountMeta::new_readonly(foreign, false)])],
        &[&owner],
    );
    assert_err(&res, ShortwinError::EpochRecordMismatch);
    // Another market's record at its own address.
    let other = wiped_lab();
    let theirs = pda::share_epoch(&other.k.market, 0).0;
    let acc = other.env.svm.get_account(&theirs).unwrap();
    lab.env.svm.set_account(theirs, acc).unwrap();
    let res = lab.env.send(
        &[record_ix(vec![AccountMeta::new_readonly(theirs, false)])],
        &[&owner],
    );
    assert_err(&res, ShortwinError::EpochRecordMismatch);
    // A record of this market whose epoch field was changed (its address no longer matches).
    let mut rec: ShareEpoch = read::borsh(&lab.env.svm, &good).unwrap();
    rec.epoch = 5;
    let bad = Pubkey::new_unique();
    shortwin_litesvm_tests::fixtures::set_borsh(&mut lab.env.svm, &bad, &rec, ShareEpoch::SPACE, 0);
    let res = lab.env.send(
        &[record_ix(vec![AccountMeta::new_readonly(bad, false)])],
        &[&owner],
    );
    assert_err(&res, ShortwinError::EpochRecordMismatch);
    // A non-record account of this program (the market itself).
    let res = lab.env.send(
        &[record_ix(vec![AccountMeta::new_readonly(m, false)])],
        &[&owner],
    );
    assert_err(&res, ShortwinError::EpochRecordMismatch);

    // The right record: the lender is parked on the live wipe record and claims its SOL.
    assert_eq!(lab.claim("A"), Ok(5_000_000_000));
}

#[test]
fn records_must_be_consecutive() {
    // Two epochs: a wipe, then (after the post-wipe sweep) a new lender and a rebase-free wipe.
    let mut lab = wiped_lab();
    lab.sweep_sol(0);
    lab.deposit("B", 1_000_000_000).unwrap();
    lab.borrow30("q", 1_000_000_000).unwrap();
    lab.settle("q", 7, 0);
    assert_eq!(lab.market().share_epoch, 2);
    let m = lab.k.market;
    let a = lab.key("A");
    let owner = lab.users["A"].kp.insecure_clone();
    let r0 = AccountMeta::new_readonly(pda::share_epoch(&m, 0).0, false);
    let r1 = AccountMeta::new_readonly(pda::share_epoch(&m, 1).0, false);
    let res = lab.env.send(
        &[ix::with_remaining(
            ix::claim_sol(&a, &m),
            &[r1.clone(), r0.clone()],
        )],
        &[&owner],
    );
    assert_err(&res, ShortwinError::EpochRecordMismatch);
    let res = lab.env.send(
        &[ix::with_remaining(ix::claim_sol(&a, &m), &[r0.clone(), r0])],
        &[&owner],
    );
    assert_err(&res, ShortwinError::EpochRecordMismatch);
    // In order they work (A converts through the wipe and jumps: it holds nothing).
    assert_eq!(lab.claim("A"), Ok(5_000_000_000));
    assert_eq!(lab.position("A").unwrap().epoch, 2);
}

#[test]
fn sync_lender_is_permissionless_and_goes_step_by_step() {
    let mut lab = Lab::with_cap(TokenProgram::Token2022, 10_000);
    // Three settlements that extinguish all but one token: each time S + VS > R_MAX·(T + ΣB_Q +
    // VA) forces a rebase (C0-6). A deposits once and is never touched in between; K deposits
    // again before each round.
    lab.deposit("A", 1_000_000_000_000).unwrap();
    for i in 0..3 {
        if i > 0 {
            lab.deposit("K", 1_000_000_000_000).unwrap();
        }
        let pid = format!("p{i}");
        let t = lab.market().tokens_free;
        lab.borrow30(&pid, t - 1).unwrap();
        lab.settle(&pid, 1_000_000, 0);
        assert_eq!(lab.market().share_epoch, i + 1, "a rebase each round");
    }
    let e = lab.market().share_epoch;
    let before = lab.position("A").unwrap();
    assert_eq!(before.epoch, 0);
    // One record at a time: Behind until the last, then touched (sync_lender needs no owner
    // signature; the lab signs with the fee payer only).
    let mut steps = 0;
    loop {
        let st = lab.sync("A", 1).unwrap();
        steps += 1;
        if st != Standing::Behind {
            break;
        }
        assert!(steps < 10);
    }
    assert!(steps >= 2, "at least one partial step");
    let after = lab.position("A").unwrap();
    assert_eq!(after.epoch, e);
    assert!(after.shares < before.shares);
    assert!(after.pending > 0);
    // sync_lender(0) is a plain touch.
    assert_eq!(lab.sync("A", 0), Ok(Standing::Current));
}

#[test]
fn claim_cover_and_sync_never_write_the_market() {
    let mut lab = Lab::new(TokenProgram::Spl);
    lab.deposit("A", 1_000_000_000).unwrap();
    lab.deposit("B", 1_000_000_000).unwrap();
    lab.borrow30("p", 500_000_000).unwrap();
    lab.advance(3_600, 10u128.pow(15));
    lab.repay("p", lab.owed("p"), 1_000_000);
    let market_bytes = lab.env.svm.get_account(&lab.k.market).unwrap();
    lab.claim("A").unwrap();
    lab.cover("B", 5).unwrap();
    lab.sync("A", 0).unwrap();
    assert_eq!(
        lab.env.svm.get_account(&lab.k.market).unwrap(),
        market_bytes,
        "C0-17"
    );
}

#[test]
fn mint_guard_entry_and_exit() {
    let mut lab = Lab::new(TokenProgram::Token2022);
    lab.deposit("A", 1_000_000).unwrap();
    // A market flagged unsafe refuses deposits; withdrawals and claims go on (exit path).
    let mut m = lab.market();
    m.set_flag(MARKET_FLAG_MINT_UNSAFE, true);
    write_zc(&mut lab.env, &lab.k.market, &m, 0);
    assert_eq!(
        lab.deposit("A", 10),
        Err(code(ShortwinError::MintUnsafeFlagged))
    );
    assert_eq!(lab.withdraw("A", 1_000), Ok((1, 0)));

    // A mint that fails the filter after listing (a freeze authority written into it): the
    // entry path refuses, the exit path never does and reports MintSafetyTripped.
    let mut lab = Lab::new(TokenProgram::Spl);
    lab.deposit("A", 1_000_000).unwrap();
    let mut mint = lab.env.svm.get_account(&lab.k.mint).unwrap();
    mint.data[46..50].copy_from_slice(&1u32.to_le_bytes());
    mint.data[50..82].copy_from_slice(&Pubkey::new_unique().to_bytes());
    lab.env.svm.set_account(lab.k.mint, mint).unwrap();
    assert_eq!(
        lab.deposit("A", 10),
        Err(code(ShortwinError::MintFreezeAuthority))
    );
    let lk = lender_keys(&mut lab, "A");
    let owner = lab.users["A"].kp.insecure_clone();
    let meta = ok(lab.env.send(&[ix::withdraw(&lk, 1_000, 0)], &[&owner]));
    // One `emit_cpi!` per instruction (ARCH §3.0 step 11): `Withdraw`; the tripwire is a log
    // event (`emit!`, no instruction-trace entry; M1-SEC-11 / M1-FID-10).
    let tripped = events::logged::<shortwin::events::MintSafetyTripped>(&meta);
    assert_eq!(tripped.len(), 1);
    assert_eq!(tripped[0].mint, lab.k.mint);
    assert_eq!(events::raw(&meta).len(), 1, "one event CPI");
    assert!(events::of::<shortwin::events::MintSafetyTripped>(&meta).is_empty());
    assert_eq!(events::one::<shortwin::events::Withdraw>(&meta).amount, 1);
    // A passing mint logs no tripwire.
    let mut lab = Lab::new(TokenProgram::Spl);
    lab.deposit("A", 1_000_000).unwrap();
    let lk = lender_keys(&mut lab, "A");
    let owner = lab.users["A"].kp.insecure_clone();
    let meta = ok(lab.env.send(&[ix::withdraw(&lk, 1_000, 0)], &[&owner]));
    assert!(events::logged::<shortwin::events::MintSafetyTripped>(&meta).is_empty());
    assert_eq!(events::raw(&meta).len(), 1, "one event CPI");
}

#[test]
fn prologue_adopts_the_kink_only_without_open_positions() {
    let mut lab = Lab::new(TokenProgram::Spl);
    lab.deposit("A", 1_000_000_000).unwrap();
    lab.borrow30("p", 100_000_000).unwrap();
    propose_and_apply(
        &mut lab.env,
        ConfigChange::Kink {
            util_kink_bps: 7_000,
        },
    );
    let cfg = lab.env.config();
    assert_eq!(cfg.kink.version, 2);
    // A position is open: deposits and withdrawals keep the old kink (R3A-9).
    lab.deposit("A", 10).unwrap();
    assert_eq!(
        (lab.market().kink_version, lab.market().util_kink_bps),
        (1, 8_000)
    );
    lab.repay("p", 0, 0);
    lab.withdraw("A", 1_000).unwrap();
    assert_eq!(
        (lab.market().kink_version, lab.market().util_kink_bps),
        (2, 7_000)
    );
}

#[test]
fn prologue_forces_ema_params_after_force_at_only() {
    let mut lab = Lab::new(TokenProgram::Spl);
    lab.deposit("A", 1_000_000_000).unwrap();
    lab.borrow30("p", 100_000_000).unwrap();
    let mut ema = shortwin::state::EmaParamsArgs::defaults();
    ema.ema_dtmax_s = 1;
    propose_and_apply(&mut lab.env, ConfigChange::EmaParams { ema });
    let cfg = lab.env.config();
    lab.deposit("A", 10).unwrap();
    assert_eq!(
        lab.market().ema_params_version,
        1,
        "positions open, before force_at"
    );
    let wait = cfg.ema_params.force_at - lab.now();
    lab.advance(wait, 0);
    lab.deposit("A", 10).unwrap();
    let m = lab.market();
    assert_eq!(
        (m.ema_params_version, m.ema_dtmax_s),
        (cfg.ema_params.version, 1)
    );
    // A forced EMA adoption never touches the kink.
    assert_eq!(m.kink_version, cfg.kink.version);
}

#[test]
fn prologue_accrues_rent_from_the_aggregate() {
    // The M1 prologue's step 4 with a price and an open position written into the market (as
    // M2 will leave them): the basis indices advance on P_ema and the lenders' credit
    // ⌊A·δJ/(10⁴·10⁴·31,536,000·WAD)⌋ goes to acc and receivable.
    let mut lab = Lab::new(TokenProgram::Spl);
    lab.deposit("A", 2_000_000_000).unwrap();
    lab.borrow30("p", 1_000_000_000).unwrap();
    let terms = rent::RentTerms {
        base_bps: 5_000,
        slope1_bps: 18_750,
        slope2_bps: 300_000,
    };
    let mut m: Market = lab.market();
    m.ema_price.set(3 * WAD);
    let mut aggr = rent::Aggr::default();
    aggr.add(1_000_000_000, &terms, 1_500).unwrap();
    m.set_aggr(&aggr);
    write_zc(&mut lab.env, &lab.k.market, &m, 0);
    lab.env.warp(D);

    let m0 = lab.market();
    let p0 = m0.pool();
    let now = lab.now();
    let util = rent::utilization_bps(p0.tokens_free, p0.borrowed_q).unwrap();
    let mut basis = m0.basis();
    let delta = rent::accrue(
        &mut basis,
        3 * WAD,
        util,
        m0.util_kink_bps,
        u64::try_from(now - m0.last_accrual_ts).unwrap(),
    )
    .unwrap();
    let credit = rent::lender_credit(&m0.aggr(), &delta).unwrap();
    assert!(credit > 0);
    let lk = lender_keys(&mut lab, "A");
    let owner = lab.users["A"].kp.insecure_clone();
    ok(lab.env.send(&[ix::deposit(&lk, 1, 0)], &[&owner]));
    let m1 = lab.market();
    assert_eq!(m1.basis(), basis);
    assert_eq!(m1.last_accrual_ts, now);
    assert_eq!(m1.receivable, p0.receivable + credit);
    let mut p = p0;
    let mut v = lab.state().vault;
    pool::accrue(&mut p, &mut v, now, credit).unwrap();
    assert_eq!((m1.acc.get(), m1.carry.get()), (p.acc, p.carry));
}

#[test]
fn sweep_protocol_fees_pays_the_recipient_only() {
    let mut lab = Lab::new(TokenProgram::Spl);
    lab.deposit("A", 1_000_000_000).unwrap();
    lab.borrow30("p", 500_000_000).unwrap();
    lab.advance(D, 10u128.pow(16));
    let owed = lab.owed("p");
    lab.repay("p", owed, 0);
    lab.add_protocol_fees(5_000_000);
    let v0 = lab.vault();
    assert!(v0.protocol_fees > 5_000_000 && v0.lender_sol > 0);
    let recipient = lab.env.fee_recipient.pubkey();
    let r0 = read::lamports(&lab.env.svm, &recipient);
    let vl0 = read::lamports(&lab.env.svm, &lab.k.market_vault);

    // Only config.fee_recipient.
    let res = lab.env.send(
        &[ix::sweep_protocol_fees(
            &lab.k.market,
            &Pubkey::new_unique(),
        )],
        &[],
    );
    assert_err(&res, ShortwinError::AccountMismatch);
    let meta = ok(lab
        .env
        .send(&[ix::sweep_protocol_fees(&lab.k.market, &recipient)], &[]));
    let ev = events::one::<shortwin::events::ProtocolFeesSwept>(&meta);
    assert_eq!((ev.amount, ev.recipient), (v0.protocol_fees, recipient));
    assert_eq!(
        read::lamports(&lab.env.svm, &recipient),
        r0 + v0.protocol_fees
    );
    assert_eq!(
        read::lamports(&lab.env.svm, &lab.k.market_vault),
        vl0 - v0.protocol_fees
    );
    let v1 = lab.vault();
    assert_eq!((v1.protocol_fees, v1.lender_sol), (0, v0.lender_sol));
    // Again: nothing to move.
    let meta = ok(lab
        .env
        .send(&[ix::sweep_protocol_fees(&lab.k.market, &recipient)], &[]));
    assert_eq!(
        events::one::<shortwin::events::ProtocolFeesSwept>(&meta).amount,
        0
    );
    // The lenders' SOL is still claimable in full.
    let paid = lab.claim("A").unwrap();
    assert!(paid > 0);

    // Protocol fees recorded without the lamports behind them (a broken I2): the sweep
    // refuses rather than take lender SOL.
    lab.env.check_invariants = false;
    let mut v = lab.vault();
    v.protocol_fees = 1_000;
    write_zc(&mut lab.env, &lab.k.market_vault, &v, 0);
    let res = lab
        .env
        .send(&[ix::sweep_protocol_fees(&lab.k.market, &recipient)], &[]);
    assert_err(&res, ShortwinError::MathOverflow);
}

#[test]
fn deposit_waits_for_the_post_wipe_sweeps() {
    // R4A-7: a list-only (or Raydium) market needs the trader sweep; a pump venue also the
    // venue-reward claim.
    for venue in [VenueKind::None, VenueKind::PumpSwap] {
        let mut lab = wiped_lab();
        lab.set_venue(venue);
        let pending = code(ShortwinError::WipeSweepPending);
        assert_eq!(lab.deposit("X", 2), Err(pending));
        lab.sweep_sol(0);
        if venue == VenueKind::PumpSwap {
            assert_eq!(lab.deposit("X", 2), Err(pending));
            lab.sweep_from(
                1_000_000_000,
                shortwin_math::pool::SweepSource::VenueRewards,
            );
        }
        assert_eq!(lab.deposit("X", 2), Ok(2_000));
        assert!(
            !lab.market()
                .has_flag(shortwin::state::MARKET_FLAG_WIPE_LIVE)
        );
        // The wiped lender got the settlement and the post-wipe cashback, the depositor none.
        let old = if venue == VenueKind::PumpSwap {
            6_000_000_000
        } else {
            5_000_000_000
        };
        assert_eq!(lab.claim("A"), Ok(old));
        lab.advance(i64::from(lab.lump_vest_s) + 1, 0);
        assert_eq!(lab.claim("X"), Ok(0));
    }
}

#[test]
fn cu_of_each_lender_instruction() {
    let mut lab = Lab::new(TokenProgram::Token2022);
    for u in ["A", "B", "C"] {
        lab.deposit(u, 1_000_000_000_000).unwrap();
    }
    lab.borrow30("p", 1_500_000_000_000).unwrap();
    lab.advance(D, 10u128.pow(16));
    let a = lab.free_allowance("A");
    let s = lab.shares_for("A", a);
    lab.withdraw("A", s).unwrap();
    lab.repay("p", lab.owed("p"), 50_000_000);
    lab.advance(3_600, 0);
    lab.claim("B").unwrap();
    lab.cover("C", 1).unwrap();
    lab.deposit("C", 1).unwrap();
    // A withdrawal after a year: the calendar advance visits all 128 day buckets.
    lab.advance(365 * D, 0);
    lab.withdraw("B", 1_000).unwrap();
    lab.sync("A", 0).unwrap();
    let recipient = lab.env.fee_recipient.pubkey();
    let meta = ok(lab
        .env
        .send(&[ix::sweep_protocol_fees(&lab.k.market, &recipient)], &[]));
    lab.cu
        .insert("sweep_protocol_fees", meta.compute_units_consumed);
    println!("lender CU (Token-2022): {}", lab.cu_report());
    for (name, cu) in &lab.cu {
        assert!(*cu < 200_000, "{name} {cu}");
    }
}

/// The lender instructions on a market listed by `create_market` (the market track's
/// instruction) instead of the installed fixture: deposit, withdraw, claim on both token
/// programs, and the creation fee (booked as `protocol_fees`) swept to the fee recipient.
#[test]
fn lender_flow_on_a_created_market() {
    use shortwin_litesvm_tests::{Env, tokens};
    use solana_keypair::Keypair;
    for program in TokenProgram::BOTH {
        let mut env = Env::initialized();
        let authority = Keypair::new();
        let payer = env.payer.insecure_clone();
        let mint = tokens::create_mint(&mut env.svm, &payer, &authority.pubkey(), 6, program);
        let creator = env.funded_keypair();
        let create = ix::create_market(
            &creator.pubkey(),
            &mint,
            &program.id(),
            0,
            &pda::canonical_pool(&mint),
        );
        ok(env.send(&[create], &[&creator]));
        let market = pda::market(&mint).0;
        env.track_market(market);
        let h = tokens::holder(&mut env.svm, &payer, &mint, &authority, 5_000_000, program);
        let lk = LenderKeys {
            owner: h.owner.pubkey(),
            mint,
            token_program: program.id(),
            owner_token_account: h.token_account,
        };
        let meta = ok(env.send(&[ix::deposit(&lk, 5_000_000, 0)], &[&h.owner]));
        let d = events::one::<shortwin::events::Deposit>(&meta);
        assert_eq!((d.shares, d.t_after), (5_000_000_000, 5_000_000));
        ok(env.send(&[ix::claim_sol(&lk.owner, &market)], &[&h.owner]));
        let meta = ok(env.send(&[ix::withdraw(&lk, 5_000_000_000, 5_000_000)], &[&h.owner]));
        assert_eq!(
            events::one::<shortwin::events::Withdraw>(&meta).amount,
            5_000_000
        );
        assert!(env.lender(&market, &lk.owner).is_none());
        let fee = env.config().params.market_creation_fee;
        let recipient = env.fee_recipient.pubkey();
        let before = read::lamports(&env.svm, &recipient);
        ok(env.send(&[ix::sweep_protocol_fees(&market, &recipient)], &[]));
        assert_eq!(read::lamports(&env.svm, &recipient), before + fee);
    }
}
