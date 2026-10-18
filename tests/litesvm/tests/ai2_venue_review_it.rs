//! AI review 2 venue claims, exercised against the pinned venue ELFs.
#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]

#[path = "pamm_it/common.rs"]
#[allow(unused_imports, clippy::result_large_err)] // TxResult is LiteSVM's own large-error type.
mod common;

use common::*;
use shortwin::constants::{TOKEN_PROGRAM_ID, WSOL_MINT};
use shortwin::state::{MARKET_FLAG_WIPE_REWARDS_CLAIMED, MARKET_FLAG_WIPE_TRADER_SWEPT};
use shortwin_litesvm_tests::{fixtures, ix};

/// Reach a dust flush with surviving shares and real outstanding PumpSwap cashback.
fn dust_flushed() -> (Env, PammMarket, u64) {
    let (mut env, pm, q) = setup(PammCoin::Cashback);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    let payer = env.payer.insecure_clone();
    ok(env.send(
        &[anchor_lang::solana_program::system_instruction::transfer(
            &payer.pubkey(),
            &pm.trader(),
            SOL,
        )],
        &[&payer],
    ));
    ok(env.send(&[pamm::sweep_trader_pamm_ix(&env, &pm)], &[]));
    let m = env.market(&pm.market());
    let want = m.tokens_free - q / 2_000;
    let shares = env
        .lender(&pm.market(), &pm.lender.pubkey())
        .unwrap()
        .shares;
    let (mut lo, mut hi) = (0, shares);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if shortwin_math::pool::tokens_for_shares(&m.pool(), mid).unwrap() <= u128::from(want) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let lender = pm.lender.insecure_clone();
    ok(env.send(&[ix::withdraw(&pm.lender_keys(), lo, want)], &[&lender]));
    let whale = PammUser::new(&mut env, &pm.pool, 1_000 * SOL);
    ok(whale.push_price(&mut env, &pm.pool, 5, 1));
    let kp = keeper(&mut env);
    let mut flushed = false;
    for _ in 0..240 {
        env.warp(1);
        crank(&mut env, &pm);
        let meta = ok(settle(&mut env, &pm, &kp, &b.key(), nonce));
        if let Some(event) = events::of::<SolSettle>(&meta).pop() {
            assert!(event.vest_flushed > 0);
            flushed = true;
            break;
        }
    }
    assert!(flushed);
    let m = env.market(&pm.market());
    assert!(m.total_shares.get() > 0);
    assert_eq!(m.vest_bal, 0);
    assert!(tokens(&env, &pm.cashback_ata()) > 0);
    (env, pm, q)
}

#[test]
fn air2_05_split_cashback_collection_keeps_deposits_held_until_release() {
    let (mut env, pm, q) = dust_flushed();
    // The first empty trader sweep leaves the venue reward source pending.
    ok(env.send(&[pamm::sweep_trader_pamm_ix(&env, &pm)], &[]));
    let m = env.market(&pm.market());
    assert!(m.has_flag(MARKET_FLAG_WIPE_TRADER_SWEPT));
    assert!(!m.has_flag(MARKET_FLAG_WIPE_REWARDS_CLAIMED));
    let trader = pm.trader();
    let dest = wsol_ata(&trader);
    ok(env.send(
        &[
            fork::create_ata_idempotent_ix(
                &env.payer.pubkey(),
                &trader,
                &WSOL_MINT,
                &TOKEN_PROGRAM_ID,
            ),
            pamm::pamm_claim_cashback_ix(&trader, &dest),
            pamm::sweep_trader_pamm_ix(&env, &pm),
        ],
        &[],
    ));
    let m = env.market(&pm.market());
    assert!(
        m.vest_bal > 0,
        "a repeated Trader source can re-vest the later claim"
    );
    assert!(m.has_flag(MARKET_FLAG_WIPE_REWARDS_CLAIMED));
    let late = env.funded_keypair();
    let acc = fork::fund_token(
        &mut env,
        &late.pubkey(),
        &pm.keys.mint,
        pm.pool.token_program(),
        q,
    );
    let keys = ix::LenderKeys {
        owner: late.pubkey(),
        mint: pm.keys.mint,
        token_program: pm.keys.token_program,
        owner_token_account: acc,
    };
    let deposit = ix::with_remaining(
        ix::deposit(&keys, q, 0),
        &ix::epoch_records(&pm.market(), 0, m.share_epoch),
    );
    assert_err(
        &env.send(std::slice::from_ref(&deposit), &[&late]),
        E::PoolDrained,
    );
    env.warp(m.vest_end - env.now() + 1);
    ok(env.send(&[deposit], &[&late]));
    let meta = ok(env.send(&[ix::claim_sol(&late.pubkey(), &pm.market())], &[&late]));
    let paid: shortwin::events::ClaimSol = events::one(&meta);
    assert_eq!(
        paid.sol_paid, 0,
        "the late depositor receives none of the old cashback"
    );
}

#[test]
fn air2_05_legacy_dust_state_rechecks_sources_even_without_a_live_vest() {
    let (mut env, pm, q) = dust_flushed();
    // Restore only the pre-upgrade marker/flags on the organically reached dust state.
    let mut legacy = env.market(&pm.market());
    legacy.deposit_hold_until = 0;
    legacy.set_flag(MARKET_FLAG_WIPE_TRADER_SWEPT, true);
    legacy.set_flag(MARKET_FLAG_WIPE_REWARDS_CLAIMED, true);
    fixtures::set_zc(&mut env.svm, &pm.market(), &legacy, 0);
    let late = env.funded_keypair();
    let acc = fork::fund_token(
        &mut env,
        &late.pubkey(),
        &pm.keys.mint,
        pm.pool.token_program(),
        q,
    );
    let keys = ix::LenderKeys {
        owner: late.pubkey(),
        mint: pm.keys.mint,
        token_program: pm.keys.token_program,
        owner_token_account: acc,
    };
    let deposit = ix::with_remaining(
        ix::deposit(&keys, q, 0),
        &ix::epoch_records(&pm.market(), 0, legacy.share_epoch),
    );
    assert_err(&env.send(&[deposit], &[&late]), E::WipeSweepPending);
    ok(env.send(&[pamm::sweep_trader_pamm_ix(&env, &pm)], &[]));
    let migrated = env.market(&pm.market());
    assert!(migrated.deposit_hold_until > 0);
    assert!(!migrated.has_flag(MARKET_FLAG_WIPE_REWARDS_CLAIMED));
}

/// AIR2-12 characterization: the venue can pay an arbitrary trader-owned WSOL account, while
/// short.win only sweeps the canonical ATA. No trader signature is available or used here.
#[test]
fn air2_12_permissionless_cashback_can_be_stranded_outside_the_canonical_ata() {
    let (mut env, pm, q) = setup(PammCoin::Cashback);
    let mut borrower = env.borrower();
    open_ok(&mut env, &pm, &mut borrower, q);
    let amount = tokens(&env, &pm.cashback_ata());
    assert!(amount > 0);
    let trader = pm.trader();
    // Real Token InitializeAccount3, paid by an unrelated payer; no trader signer required.
    let other = fork::fund_wsol(&mut env, &trader, 0);
    assert_ne!(other, wsol_ata(&trader));
    let claim = pamm::pamm_claim_cashback_ix(&trader, &other);
    assert!(claim.accounts.iter().all(|a| !a.is_signer));
    ok(env.send(&[claim], &[]));
    assert_eq!(tokens(&env, &pm.cashback_ata()), 0);
    assert_eq!(tokens(&env, &other), amount);

    let sweep = pamm::sweep_trader_pamm_ix(&env, &pm);
    ok(env.send(std::slice::from_ref(&sweep), &[]));
    assert_eq!(
        tokens(&env, &other),
        amount,
        "normal sweep leaves the cashback stranded"
    );
    let mut recover = sweep;
    let canonical = wsol_ata(&trader);
    let destination = recover
        .accounts
        .iter_mut()
        .find(|a| a.pubkey == canonical)
        .unwrap();
    destination.pubkey = other;
    assert_err(&env.send(&[recover], &[]), E::AccountMismatch);
    eprintln!(
        "AIR2-12: {amount} cashback lamports claimed without trader signature into {other}; canonical sweep cannot recover them"
    );
}
