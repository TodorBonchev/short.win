//! PoC `04_sol_settle` (rust-auditor `--poc`): "A deposit after a halted-venue S3 takes the old
//! lenders' vested SOL".
//!
//! On a venue that cannot execute, `sol_settle` prices the extinguished tokens at 0 for the
//! immediate part (`sol_settle.rs:285`, AIR-09): all of the S3 take above `R_due` — the SOL that
//! pays the lenders for the tokens the settlement extinguished — vests over `lump_vest_s`
//! (`pool::settle`). The settlement leaves far more than 1/1,000 of the token claims, so the vest
//! is not flushed to the shares that held the tokens, and `deposit` (no venue account; its AIR-07
//! gate needs `ΣB_Q > 0`) lets anyone mint shares after the S3. The vest then releases pro rata to
//! every share that exists while it runs, the new depositor's included.
//!
//! The test runs `target/deploy/shortwin.so` on the
//! Raydium AMM v4 fork (Fartcoin's dumped pool and program), through the scenario of
//! `raydium_v4_e2e::admin_disable_reaches_s3_after_the_timer`: a lender H, a position that real
//! buys make liquidatable, the Raydium admin's real `SetParams` halt, the keeper's 30 s cranks and
//! a checked `sol_settle` (S3). From the state just before the S3:
//!
//! - control: the S3 alone; after `lump_vest_s + 1` s H claims;
//! - attack: the same S3, then X (an outside token holder) deposits as many tokens as H lent, in
//!   the same second; after `lump_vest_s + 1` s X claims and withdraws, then H claims.
//!
//! The regression asserts the safe behaviour the fix restores: deposits are held until the vest
//! releases, the depositor who came after the S3 takes none of it, and H receives the same amount
//! with or without the late deposit. It also checks that a later vested sweep cannot bypass the
//! hold by extending the common vest. The initial scenario failed on the audited 1.0.1 binary.

#![allow(clippy::result_large_err)]

use anchor_lang::solana_program::system_instruction;

#[path = "raydium_v4_support.rs"]
mod support;

use shortwin::events::{ClaimSol as ClaimSolEv, Withdraw as WithdrawEv};
use shortwin_math::pool;
use shortwin_venue_state::raydium_v4;
use support::*;

/// H's deposit: 10⁶ Fartcoin (≈ 1,600 SOL of value), as `raydium_v4_e2e`.
const LEND: u64 = 1_000_000_000_000;

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

/// The most of `owner`'s shares whose tokens are within its allowance now (the program's
/// `pool::allowance`, the lender touched alone).
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

/// One `crank_price` (the keeper's observation; its prologue accrues the vest release that
/// `claim_sol` pays against).
fn crank(f: &mut Fart) {
    let ix = crank_price_ix(&f.rm);
    ok(f.env.send(&[ix], &[]));
}

#[test]
fn a_deposit_after_a_halted_venue_s3_takes_none_of_the_old_lenders_vest() {
    // H lends; a borrower opens 5 SOL of Fartcoin; real buys (×2.3) make it liquidatable.
    let mut f = Fart::new(fork_env(), LEND);
    let market = f.rm.market();
    let q = q_for_sol(&f.env, &f.rm, 5 * SOL);
    let (b, nonce, _) = f.open(q);
    f.tick(5);
    f.pump_until_liquidatable(&b.key(), nonce, 2.3, 5, 60);
    let pos = f.pos(&b, nonce);
    // A third party, the Raydium pool admin, halts the pool (SetParams status 4) before any
    // keeper liquidates: the venue cannot execute.
    f.env.warp(1);
    set_status(&mut f.env, &f.rm, raydium_v4::STATUS_LIQUIDITY_ONLY);
    f.second();
    // X: an outside holder of Fartcoin with no lender position (as many tokens as H lent).
    let x = f.env.funded_keypair();
    let d = LEND;
    let x_acc = fork::fund_token(
        &mut f.env,
        &x.pubkey(),
        &f.rm.keys.mint,
        TokenProgram::Spl,
        d,
    );
    let h = f.lender.kp.insecure_clone();
    let vest_s = i64::from(f.env.config().params.lump_vest_s);

    // The keeper cranks every 30 s and calls sol_settle (checked) until the S3 executes.
    let mut found = None;
    for _ in 0..40 {
        f.env.warp(30);
        crank_checked(&mut f.env, &f.rm);
        let saved = f.env.svm.clone();
        let s = f.settle(&b.key(), nonce);
        if let Some(ev) = s.ev {
            found = Some((saved, ev, s.pred.inp.r_due, s.pred.inp.executable));
            break;
        }
    }
    let (ready, ctl_ev, r_due, executable) = found.expect("the S3 executes after s3_wait_s");
    assert_eq!(ctl_ev.condition, 3, "S3");
    assert!(!executable, "the venue cannot execute");
    assert_eq!(
        ctl_ev.lump,
        ctl_ev.lender_amount - r_due,
        "AIR-09: all of the take above R_due vests"
    );
    assert_eq!(
        ctl_ev.vest_flushed, 0,
        "the settlement does not flush the vest"
    );
    let m = f.market();
    assert_eq!((m.open_positions, m.borrowed_q), (0, 0), "no position open");
    let vest = m.vest_bal;
    assert!(vest >= ctl_ev.lump && vest > 0, "the take vests");
    let t_after = m.tokens_free;
    let s_after = m.total_shares.get();

    // ---- control: the S3 alone; H claims after the vest ------------------------------------
    f.env.warp(vest_s + 1);
    crank(&mut f);
    assert_eq!(f.market().vest_bal, 0, "the vest has released");
    let h_control = claim_twice(&mut f.env, &market, &h);
    assert!(
        h_control >= vest,
        "without the late deposit H is paid the whole vest: {h_control} < {vest}"
    );

    // ---- attack: the same S3, then X deposits in the same second ---------------------------
    f.env.svm = ready.clone();
    let s = f.settle(&b.key(), nonce);
    let ev = s.ev.expect("the same S3 executes");
    assert_eq!(
        (ev.condition, ev.lender_amount, ev.lump),
        (ctl_ev.condition, ctl_ev.lender_amount, ctl_ev.lump),
        "the same settlement"
    );
    assert_eq!(f.market().vest_bal, vest, "the same vest");
    let keys = lender_keys(&f.rm, &x.pubkey(), &x_acc);
    let dep = f.env.send(&[ix::deposit(&keys, d, 0)], &[&x]);
    let dep_refused = dep.is_err();
    if dep_refused {
        assert_err(&dep, E::PoolDrained);
    }
    let (x_sol, x_back, x_frac) = match dep {
        Err(e) => {
            // The Fix's behaviour: the deposit waits until the vest ends; X takes nothing.
            println!("X's deposit after the S3 is refused: {:?}", e.err);
            f.env.warp(vest_s + 1);
            crank(&mut f);
            assert_eq!(f.market().vest_bal, 0, "the vest has released");
            (0u64, d, 0f64)
        }
        Ok(meta) => {
            let dev: Deposit = events::one(&meta);
            assert_eq!(dev.owner, x.pubkey());
            let frac = dev.shares as f64 / (dev.s_after + pool::VIRTUAL_SHARES) as f64;
            println!(
                "X deposits {d} tokens after the S3 at T = {t_after}, S = {s_after}: {} shares, \
                 {:.4} of the pool's shares",
                dev.shares, frac
            );
            // X holds through the vest; the keeper's crank; X claims and withdraws.
            f.env.warp(vest_s + 1);
            crank(&mut f);
            assert_eq!(f.market().vest_bal, 0, "the vest has released");
            let shares = withdrawable_shares(&f.env, &market, &x.pubkey());
            assert!(shares > 0, "X may withdraw");
            let meta = ok(f.env.send(
                &[
                    ix::claim_sol(&x.pubkey(), &market),
                    ix::withdraw(&keys, shares, 0),
                ],
                &[&x],
            ));
            let mut x_sol = sol_paid(&meta, &x.pubkey());
            let x_back = tokens_withdrawn(&meta, &x.pubkey());
            x_sol += claim_twice(&mut f.env, &market, &x);
            assert!(
                x_back + 2 >= d,
                "X's tokens come back: {x_back} of {d} (no position was open to lend them to)"
            );
            (x_sol, x_back, frac)
        }
    };
    let h_attack = claim_twice(&mut f.env, &market, &h);
    if dep_refused {
        // Once the old shares have received their vest, a new lender may enter again.
        let meta = ok(f.env.send(&[ix::deposit(&keys, d, 0)], &[&x]));
        let dev: Deposit = events::one(&meta);
        assert_eq!(dev.owner, x.pubkey());
        assert!(claim_twice(&mut f.env, &market, &x) <= 2);
    }

    println!(
        "S3 on the halted pool: q {} tokens extinguished, V {} lamports, lenders' take {}, \
         R_due {r_due}, lump {}, vest {vest} lamports (lump_vest_s {vest_s} s)",
        pos.q, pos.v, ctl_ev.lender_amount, ctl_ev.lump
    );
    println!("control (no late deposit): H is paid {h_control} lamports after the vest");
    println!(
        "attack: X (deposit {d} tokens after the S3, share {x_frac:.4}) is paid {x_sol} lamports \
         and gets {x_back} of {d} tokens back; H is paid {h_attack} lamports"
    );
    println!(
        "H loses {} lamports; X gains {x_sol} lamports ({:.2} % of the vest)",
        h_control.saturating_sub(h_attack),
        x_sol as f64 * 100.0 / vest as f64
    );
    assert!(
        x_sol <= 2 && h_attack + 2 >= h_control,
        "harm: X, who deposited {d} tokens after the halted-venue S3 and got {x_back} of them back, \
         takes {x_sol} lamports of the {vest}-lamport vest that pays for H's {} extinguished tokens; \
         H is paid {h_attack} instead of {h_control} lamports (loses {})",
        pos.q,
        h_control.saturating_sub(h_attack)
    );

    // Pre-upgrade markets have zero in the newly assigned reserved bytes. Preserve the
    // economically reached post-S3 state and reset only this marker to that legacy encoding.
    f.env.svm = ready.clone();
    assert!(f.settle(&b.key(), nonce).ev.is_some());
    let mut account = f.env.svm.get_account(&market).expect("market account");
    let offset = 8 + core::mem::offset_of!(shortwin::state::Market, deposit_hold_until);
    account.data[offset..offset + 8].copy_from_slice(&0i64.to_le_bytes());
    f.env
        .svm
        .set_account(market, account)
        .expect("legacy marker");
    assert_eq!(f.market().deposit_hold_until, 0);
    assert_err(
        &f.env.send(&[ix::deposit(&keys, d, 0)], &[&x]),
        E::PoolDrained,
    );
    f.env.warp(vest_s + 1);
    crank(&mut f);
    assert_err(
        &f.env.send(&[ix::deposit(&keys, d, 0)], &[&x]),
        E::WipeSweepPending,
    );
    let sweep = shortwin_litesvm_tests::engine::sweep_trader_ix_for(&f.env, &market);
    ok(f.env.send(&[sweep], &[]));
    ok(f.env.send(&[ix::deposit(&keys, d, 0)], &[&x]));
    assert_eq!(f.market().deposit_hold_until, -1);
    assert!(claim_twice(&mut f.env, &market, &x) <= 2);

    // A later vested sweep moves the common vest end. The stored timestamp alone must not
    // admit X while any of the S3 payment remains unreleased.
    f.env.svm = ready;
    assert!(f.settle(&b.key(), nonce).ev.is_some());
    let first_end = f.market().deposit_hold_until;
    assert_eq!(
        first_end,
        f.market().vest_end,
        "the hold initially records the unreleased vest's end"
    );
    f.env.warp(3_600);
    crank(&mut f);
    let payer = f.env.payer.pubkey();
    let trader = f.market().trader;
    ok(f.env.send(
        &[system_instruction::transfer(&payer, &trader, 10 * SOL)],
        &[],
    ));
    let sweep = shortwin_litesvm_tests::engine::sweep_trader_ix_for(&f.env, &market);
    ok(f.env.send(&[sweep], &[]));
    let extended_end = f.market().vest_end;
    assert!(
        extended_end > first_end,
        "the later sweep extends the common vest"
    );

    f.env.warp(first_end - f.env.now());
    crank(&mut f);
    assert!(f.market().vest_bal > 0, "the extended vest still holds SOL");
    let dep = f.env.send(&[ix::deposit(&keys, d, 0)], &[&x]);
    assert_err(&dep, E::PoolDrained);

    f.env.warp(extended_end - f.env.now() + 1);
    crank(&mut f);
    assert_eq!(f.market().vest_bal, 0);
    ok(f.env.send(&[ix::deposit(&keys, d, 0)], &[&x]));
}
