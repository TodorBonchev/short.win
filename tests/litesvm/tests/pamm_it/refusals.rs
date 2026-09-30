//! Every `open_short` refusal particular to PumpSwap (ARCH §3.4 steps 3–4, §3.12; A13, A24, C0-10,
//! C0-14; M6): the post-change warm-up (`EmaWarmingUp`), `disable_flags` (`VenueFlagged` on any
//! non-zero value, `NoRoute` under the T-DISABLE buy bit), the boost gate (`BoostInProgress`), the
//! latch on pAMM and pump-fees (`VenueUpgraded`, exits unaffected), the existence requirement and
//! account substitution in the TradeExtras (`VenueAccountMismatch`). The venue-generic refusals are
//! `engine_it`'s and `pump_it`'s.

use shortwin::constants::{GUARD_PUMP_AMM, GUARD_PUMP_FEES};
use shortwin_litesvm_tests::ix;

use crate::common::*;

/// An open that must fail with `e`.
fn refused(env: &mut Env, pm: &PammMarket, q: u64, e: E) {
    let mut b = env.borrower();
    let (res, _) = env.open(pm, &mut b, q);
    assert_err(&res, e);
}

/// A listed market whose price has just started (one PumpSwap crank): the warm-up refuses opens
/// (after the static guards, before the listing's own `market_warmup_s`), and so it does after
/// every integration until the eighth; an open's own reverted roll integrates nothing, so the
/// warm-up counts cranked seconds, not attempts.
#[test]
fn an_open_waits_for_the_post_change_warm_up() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Legacy261);
    let q = q_for(&env, &p);
    ok(env.list_pamm(&p.mint, &p.base_program));
    let pm = env.pamm_market_listed(&p.mint, q * 40, 0);
    env.warp(60);
    crank(&mut env, &pm);
    assert_eq!(market(&env, &pm).ema_warm, 8);
    // Refused opens alone: the counter does not move.
    for _ in 0..3 {
        env.warp(1);
        refused(&mut env, &pm, q, E::EmaWarmingUp);
    }
    assert_eq!(market(&env, &pm).ema_warm, 8);
    for warm in (1..8).rev() {
        env.warp(1);
        crank(&mut env, &pm);
        assert_eq!(market(&env, &pm).ema_warm, warm);
        refused(&mut env, &pm, q, E::EmaWarmingUp);
    }
    // The open's own roll integrates the eighth bucket: confirmed, and it opens.
    env.warp(1);
    let mut b = env.borrower();
    let (res, _) = env.open(&pm, &mut b, q);
    ok(res);
}

/// `disable_flags` (T-DISABLE, A24): any non-zero value refuses opens with `VenueFlagged` — the
/// sell bit, the halts of `create_pool` / `deposit` / `withdraw`, and bits 5–7, which pAMM ignores —
/// except the buy bit, which makes the venue not executable (`NoRoute`).
#[test]
fn disable_flags_refuse_opens() {
    let (mut env, pm, q) = setup(PammCoin::Mature);
    for bit in 0..8u8 {
        pamm::set_disable_flags(&mut env.svm, 1 << bit);
        env.warp(1);
        let want = if bit == 3 {
            E::NoRoute
        } else {
            E::VenueFlagged
        };
        refused(&mut env, &pm, q, want);
    }
    pamm::set_disable_flags(&mut env.svm, 0);
    env.warp(1);
    let mut b = env.borrower();
    let (res, _) = env.open(&pm, &mut b, q);
    ok(res);
}

/// The boost gate (A13, C1-2): on the fresh pool (its boost vault still holding 13.98 SOL) opens
/// are refused `BoostInProgress` inside `boost_window_s` of `graduated_at` (the listing); a vault
/// at exactly `boost_dust_lamports` no longer blocks, one lamport more does; after the window
/// the gate is off whatever the vault holds.
#[test]
fn the_boost_gate() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Fresh);
    let q = q_for(&env, &p);
    let pm = list_on(&mut env, &p, q * 40);
    let dust = env.config().params.boost_dust_lamports;
    assert!(
        pm.pool.boost_amount(&env.svm) > dust,
        "the fixture's boost is running"
    );
    refused(&mut env, &pm, q, E::BoostInProgress);
    fork::set_token_amount(&mut env.svm, &pm.pool.boost_vault_ata, dust + 1);
    env.warp(1);
    refused(&mut env, &pm, q, E::BoostInProgress);
    fork::set_token_amount(&mut env.svm, &pm.pool.boost_vault_ata, dust);
    env.warp(1);
    let mut b = env.borrower();
    let (res, n) = env.open(&pm, &mut b, q);
    ok(res);
    close_ok(&mut env, &pm, &b, n);
    // The window: with the boost vault full again, only its end lifts the gate.
    fork::set_token_amount(&mut env.svm, &pm.pool.boost_vault_ata, 13 * SOL);
    env.warp(1);
    refused(&mut env, &pm, q, E::BoostInProgress);
    wait_out_boost(&mut env, &pm);
    let (res, _) = env.open(&pm, &mut b, q);
    ok(res);
}

/// The latch (A7; masks v0.4.1): an upgrade of pAMM or pump-fees refuses PumpSwap opens
/// (`VenueUpgraded`) until the Emergency key re-validates it; a keeper crank, `sol_settle` and
/// the borrower's close keep working (I10).
#[test]
fn the_latch_blocks_pumpswap_opens_only() {
    for (program, guard) in [
        (fork::PumpProgram::Amm, GUARD_PUMP_AMM),
        (fork::PumpProgram::Fees, GUARD_PUMP_FEES),
    ] {
        let (mut env, pm, q) = setup(PammCoin::Legacy261);
        let mut b = env.borrower();
        let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
        let slot = shortwin::constants::VENUE_GUARD_SLOTS_2026_09_27[usize::from(guard)];
        fork::set_pump_deploy_slot(&mut env.svm, program, slot + 7);
        ok(env.send(&[ix::flag_venue_upgraded(guard)], &[]));
        env.warp(1);
        let mut b2 = env.borrower();
        let (res, _) = env.open(&pm, &mut b2, q);
        assert_err(&res, E::VenueUpgraded);
        crank(&mut env, &pm);
        let kp = keeper(&mut env);
        ok(settle(&mut env, &pm, &kp, &b.key(), nonce));
        env.warp(1);
        close_ok(&mut env, &pm, &b, nonce);
        let ea = env.pause_authority.insecure_clone();
        ok(env.send(
            &[ix::revalidate_venue(&ea.pubkey(), guard, slot + 7)],
            &[&ea],
        ));
        env.warp(1);
        open_ok(&mut env, &pm, &mut b2, q);
    }
}

/// The existence requirement (C0-14, C1-8, C1-9): a trade whose trader pAMM UVA, coin-creator
/// vault ATA or recipient ATAs do not exist is refused before PumpSwap runs
/// (`VenueAccountMismatch`), and executes once anyone created them.
#[test]
fn the_existence_requirement() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Legacy261);
    let q = q_for(&env, &p);
    ok(env.list_pamm(&p.mint, &p.base_program));
    let mut pm = env.pamm_market_listed(&p.mint, q * 40, 60);
    // The trader's pAMM UVA created at listing; a protocol recipient whose ATA does not exist.
    let fresh = anchor_lang::prelude::Pubkey::new_unique();
    let mut data = env
        .svm
        .get_account(&shortwin::constants::PUMP_AMM_GLOBAL_CONFIG)
        .unwrap();
    data.data[57..89].copy_from_slice(fresh.as_ref());
    env.svm
        .set_account(shortwin::constants::PUMP_AMM_GLOBAL_CONFIG, data)
        .unwrap();
    pm.protocol_recipient = fresh;
    env.warp(1);
    refused(&mut env, &pm, q, E::VenueAccountMismatch);
    let payer = env.payer.pubkey();
    ok(env.send(
        &[fork::create_ata_idempotent_ix(
            &payer,
            &fresh,
            &shortwin::constants::WSOL_MINT,
            &shortwin::constants::TOKEN_PROGRAM_ID,
        )],
        &[],
    ));
    env.warp(1);
    let mut b = env.borrower();
    let (res, _) = env.open(&pm, &mut b, q);
    ok(res);
}

/// Account substitution (I14's PumpSwap part): every TradeExtras slot is key-checked — a
/// recipient not in the live lists, another WSOL account at a recipient's ATA slot, another
/// coin-creator vault, another UVA, another `trader_wsol` — `VenueAccountMismatch`; the ReadSet
/// likewise (another pool's vault).
#[test]
fn every_slot_is_key_checked() {
    let (mut env, pm, q) = setup(PammCoin::Legacy261);
    let other = Pubkey::new_unique();
    for slot in [
        shortwin::venues::pump_amm::T_PROGRAM,
        shortwin::venues::pump_amm::T_TRADER_WSOL,
        shortwin::venues::pump_amm::T_PROTOCOL_FEE_RECIPIENT,
        shortwin::venues::pump_amm::T_PROTOCOL_FEE_RECIPIENT_ATA,
        shortwin::venues::pump_amm::T_COIN_CREATOR_VAULT_ATA,
        shortwin::venues::pump_amm::T_COIN_CREATOR_VAULT_AUTHORITY,
        shortwin::venues::pump_amm::T_UVA,
        shortwin::venues::pump_amm::T_POOL_V2,
        shortwin::venues::pump_amm::T_BUYBACK_RECIPIENT,
        shortwin::venues::pump_amm::T_CASHBACK_ATA,
    ] {
        let b = env.borrower();
        let mut ix = open_short_ix(
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
        let n = ix.accounts.len();
        let rem_start =
            n - shortwin::venues::pump_amm::READ_LEN - shortwin::venues::pump_amm::TRADE_LEN;
        ix.accounts[rem_start + shortwin::venues::pump_amm::READ_LEN + slot].pubkey = other;
        let kp = b.kp.insecure_clone();
        let res = env.send(&[cu_limit_ix(pamm::PAMM_TRADE_CU), ix], &[&kp]);
        assert_err(&res, E::VenueAccountMismatch);
    }
    let mut b = env.borrower();
    let (res, _) = env.open(&pm, &mut b, q);
    ok(res);
}
