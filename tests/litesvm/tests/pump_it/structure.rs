//! I14 on the curve (ARCH §3.0 A6, §3.12 PumpCurve, §8.3; C0-2, C0-14): every account of the
//! curve's ReadSet (C0–C6) and TradeExtras (T0–T19) is present and key-checked in `open_short`,
//! `close_short`, `crank_price` and `sol_settle` — each slot omitted, replaced by a foreign key,
//! a trailing extra and the first two slots swapped make the instruction fail with
//! `BadRemainingAccounts` / `VenueAccountMismatch`, never "no route" or a silent default; then
//! the recipient slots take any key of the live `Global` lists, and the creator-vault slots
//! follow the **live** curve creator (a creator change after the listing: the stale cached vault
//! is refused, the re-derived one accepted).
//!
//! Every instruction goes through `Env::send`: the invariant hooks run after each one.

use shortwin_litesvm_tests::engine::with_cu_limit;

use crate::common::*;

fn variants(ix: &Instruction, remaining: usize) -> Vec<(String, Instruction)> {
    let start = ix.accounts.len() - remaining;
    let mut out = Vec::new();
    for i in 0..remaining {
        let mut omit = ix.clone();
        omit.accounts.remove(start + i);
        out.push((format!("omit slot {i}"), omit));
        let mut swap = ix.clone();
        let m = &mut swap.accounts[start + i];
        *m = AccountMeta {
            pubkey: Pubkey::new_unique(),
            is_signer: false,
            is_writable: m.is_writable,
        };
        out.push((format!("replace slot {i}"), swap));
    }
    let mut extra = ix.clone();
    extra
        .accounts
        .push(AccountMeta::new_readonly(Pubkey::new_unique(), false));
    out.push(("extra trailing account".to_string(), extra));
    let mut order = ix.clone();
    order.accounts.swap(start, start + 1);
    out.push(("slots 0 and 1 swapped".to_string(), order));
    out
}

fn assert_all_fail(
    env: &mut Env,
    ix: &Instruction,
    remaining: usize,
    signers: &[&solana_keypair::Keypair],
) {
    for (what, bad) in variants(ix, remaining) {
        let res = env.send(&[bad], signers);
        let got = custom_code(&res);
        assert!(
            matches!(
                got,
                Some(c) if c == code(E::BadRemainingAccounts) || c == code(E::VenueAccountMismatch)
            ),
            "{what}: expected BadRemainingAccounts / VenueAccountMismatch, got {got:?}"
        );
    }
}

/// Every slot of the four instructions (module docs).
#[test]
fn open_close_crank_and_settle_check_every_curve_slot() {
    let (mut env, pm, q) = setup(PumpCoin::T22_115);
    let b = env.borrower();
    let open = open_short_ix(
        &pm,
        &b.key(),
        0,
        q,
        0,
        u64::MAX,
        u64::MAX,
        ExtrasSel::NONE,
        &[],
    );
    assert_all_fail(&mut env, &open, 27, &[&b.kp]);
    ok(env.send(&with_cu_limit(&pm, open), &[&b.kp]));
    env.warp(1);
    let crank_ix = crank_price_ix(&pm);
    assert_all_fail(&mut env, &crank_ix, 7, &[]);
    let kp = keeper(&mut env);
    let settle_ix = sol_settle_ix(&env, &pm, &kp.pubkey(), &b.key(), 0);
    assert_all_fail(&mut env, &settle_ix, 7, &[&kp]);
    let close = close_short_ix(&pm, &b.key(), 0, u64::MAX, ExtrasSel::NONE, &[]);
    assert_all_fail(&mut env, &close, 27, &[&b.kp]);
    ok(env.send(&[crank_ix], &[]));
    ok(env.send(&[settle_ix], &[&kp]));
    ok(env.send(&with_cu_limit(&pm, close), &[&b.kp]));
}

/// The recipient slots (T6, T8 and their derived WSOL ATAs T7, T9) take any key of the live
/// `Global` lists: an open with the last fee recipient and the third buyback recipient, a close
/// with others, both exact.
#[test]
fn any_listed_recipient_is_accepted() {
    let (mut env, mut pm, q) = setup(PumpCoin::Legacy115);
    pm.use_recipients(&env.svm, 7, 2);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    pm.use_recipients(&env.svm, 4, 6);
    env.warp(1);
    close_ok(&mut env, &pm, &b, nonce);
    assert_trader_clean(&env, &pm);
}

/// A curve creator change after the listing (the creator vault moves): the market's cached
/// vault (T12, T13) is refused, the vault re-derived from the live creator is accepted — on
/// the open and on the close (pump itself checks the vault against the live creator).
#[test]
fn the_creator_vault_follows_the_live_creator() {
    let (mut env, mut pm, q) = setup(PumpCoin::T22_115);
    let stale = pm.creator_vault;
    let new_creator = Pubkey::new_unique();
    fork::set_curve_creator(&mut env.svm, &pm.curve.curve, &new_creator);
    let mut b = env.borrower();
    let (res, _) = env.open(&pm, &mut b, q);
    assert_err(&res, E::VenueAccountMismatch);
    pm.refresh_creator(&env.svm);
    assert_ne!(pm.creator_vault, stale);
    assert_eq!(pm.creator_vault, fork::pump_creator_vault(&new_creator));
    // The new creator's vault holds its rent-exempt minimum (an empty one costs the sale pump's
    // top-up: `rent`).
    let min0 = env.svm.minimum_balance_for_rent_exemption(0);
    env.fund(&pm.creator_vault, min0);
    // Re-deriving both vault keys lifts the open just above the 200 k default: within ARCH
    // §6.1's PROPOSED 250 k limit for curve opens and closes.
    let open = open_short_ix(
        &pm,
        &b.key(),
        0,
        q,
        0,
        u64::MAX,
        u64::MAX,
        ExtrasSel::NONE,
        &[],
    );
    let meta = ok(env.send(&[pump::cu_limit_ix(pump::CURVE_TRADE_CU), open], &[&b.kp]));
    eprintln!(
        "open after a creator change: {} CU",
        meta.compute_units_consumed
    );
    env.warp(1);
    let close = close_short_ix(&pm, &b.key(), 0, u64::MAX, ExtrasSel::NONE, &[]);
    let meta = ok(env.send(&[pump::cu_limit_ix(pump::CURVE_TRADE_CU), close], &[&b.kp]));
    eprintln!(
        "close after a creator change: {} CU",
        meta.compute_units_consumed
    );
    assert_trader_clean(&env, &pm);
}
