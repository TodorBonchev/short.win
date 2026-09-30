//! The keeper on PumpSwap (M6; ARCH §2.4.1 PumpSwap row, §3.12 PumpSwap account sets, §5.3, §5.5,
//! §7.1 duties 1, 3, 6 and 10): the release short.win with the dumped PumpSwap, pump and pump-fees
//! ELFs and the `pamm_state` pools of the fork. Every keeper transaction runs the invariant hooks
//! after every instruction.
//!
//! - **A PumpSwap listing** (the `mature` pool): nobody observes it after the listing, so it has
//!   no price; the keeper starts the price (its projection of the next instruction does) and keeps
//!   the warm-up observed every second until the next instruction would end it (M6's keeper
//!   warm-up, eight buckets); the first open is then warm. A ×3 pump makes the position
//!   liquidatable and the keeper liquidates it in the first second the program accepts it, with
//!   the keeper's own PumpSwap ReadSet and TradeExtras (the adapter's key functions, the live
//!   `GlobalConfig`'s recipients: the same list as the harness's for those recipients).
//! - **`disable_flags`** (A24; T-DISABLE): any non-zero value other than the buy bit is reported
//!   (severe: opens refuse `VenueFlagged`) with the route intact; the buy bit is a lost route as
//!   well; clearing both restores the route — each once.
//! - **The existence requirement** (ARCH §3.12): on a cashback coin listed without the trader's
//!   cashback ATA (T18), the keeper creates it once; trades need it.
//! - **`None → PumpSwap` re-detection**: a graduated coin listed list-only (its pool unreadable at
//!   the listing) is cranked onto its pool; the keeper then creates the trader's base ATA and pAMM
//!   UVA (neither a list-only listing nor the crank creates them), once; an open lands.

use anchor_lang::prelude::Pubkey;
use shortwin::errors::ShortwinError;
use shortwin::state::VenueKind;
use shortwin_keeper::monitor::AdminChange;
use shortwin_keeper::venue;
use shortwin_litesvm_tests::engine::VenueMarket;
use shortwin_litesvm_tests::errors::assert_err;
use shortwin_litesvm_tests::pamm::{self, PammCoin, PammMarket, PammUser};
use shortwin_litesvm_tests::pump::cu_limit_ix;
use shortwin_litesvm_tests::{fork, ix, pda, read};
use shortwin_services_common::AccountData;
use solana_signer::Signer;
use solana_transaction::Transaction;

use crate::common::*;

const SOL: u64 = 1_000_000_000;

/// A position size inside every default cap on `p` now (1 % of the base reserve, at most ≈ 10 SOL
/// of notional).
fn q_for(env: &Env, p: &pamm::PammPool) -> u64 {
    let r = p.reserves(&env.svm).expect("reserves");
    let base = u128::from(r.base);
    let by_notional = base * u128::from(10 * SOL) / r.effective_quote.max(1);
    u64::try_from((base / 100).min(by_notional)).expect("u64")
}

/// Whether `ix` (signed by `kp`, with a compute budget) is accepted by the program now.
fn accepts(
    h: &Harness,
    kp: &solana_keypair::Keypair,
    ix: anchor_lang::solana_program::instruction::Instruction,
) -> Result<(), Option<u32>> {
    h.with_env(|env| {
        let tx = Transaction::new_signed_with_payer(
            &[cu_limit_ix(600_000), ix],
            Some(&kp.pubkey()),
            &[kp],
            env.svm.latest_blockhash(),
        );
        match env.svm.simulate_transaction(tx) {
            Ok(_) => Ok(()),
            Err(f) => Err(shortwin_litesvm_tests::errors::custom_code(&Err(f))),
        }
    })
}

/// The accounts of `keys` as the VM holds them.
fn accounts(h: &Harness, keys: &[Pubkey]) -> Vec<Option<AccountData>> {
    h.with_env(|env| {
        keys.iter()
            .map(|k| {
                env.svm.get_account(k).map(|a| AccountData {
                    lamports: a.lamports,
                    owner: a.owner,
                    executable: a.executable,
                    data: a.data,
                })
            })
            .collect()
    })
}

/// A PumpSwap listing observed by nobody but the keeper: the keeper starts its price and observes
/// its warm-up every second (eight buckets; the eighth integrated by the next instruction), then
/// leaves it alone; after the listing warm-up an open lands; a ×3 pump is liquidated on the pool in
/// the first second the program accepts it, with the keeper's PumpSwap lists.
#[test]
fn a_pumpswap_listing_is_warmed_up_and_a_pump_is_liquidated_on_the_pool() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Mature);
    let q = q_for(&env, &p);
    ok(env.list_pamm(&p.mint, &p.base_program));
    let mut pm = env.pamm_market_listed(&p.mint, q * 40, 0);
    let market = pm.market();
    assert_eq!(env.market(&market).venue_kind(), Some(VenueKind::PumpSwap));
    assert_eq!(
        env.market(&market).ema_price.get(),
        0,
        "no price at listing"
    );
    let listed = env.now();
    let mut h = Harness::new(env, KeeperConfig::default());

    // The keeper's price start and warm-up.
    let mut observed = Vec::new();
    for s in 0..12 {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.pamm_accounts.is_empty(), "the listing created the UVA");
        if r.observed(&market) {
            observed.push(s);
        }
    }
    assert_eq!(
        observed,
        (0..8).collect::<Vec<_>>(),
        "the start, then seven warm-up seconds"
    );
    let m = h.with_env(|env| env.market(&market));
    assert!(m.ema_price.get() > 0, "priced");
    assert_eq!(m.ema_warm, 1, "the eighth bucket pending");

    // The keeper's lists are the adapter's, with the live recipients it picks.
    let mk = h.with_env(|env| env.market(&market));
    let read_keys = venue::read_set(VenueKind::PumpSwap, &mk).unwrap();
    assert_eq!(
        read_keys,
        pm.read_set(false)
            .iter()
            .map(|a| a.pubkey)
            .collect::<Vec<_>>()
    );
    let extras = venue::trade_extras(VenueKind::PumpSwap, &mk, &accounts(&h, &read_keys)).unwrap();
    let pick = usize::from(p.mint.to_bytes()[0]) % 8;
    h.with_env(|env| pm.use_recipients(&env.svm, pick, pick));
    assert_eq!(
        extras.iter().map(|a| a.pubkey).collect::<Vec<_>>(),
        pm.trade_keys(),
        "the harness's T0–T18 for the same recipients"
    );
    assert_eq!(
        extras.iter().map(|a| a.is_writable).collect::<Vec<_>>(),
        pm.trade_extras()
            .iter()
            .map(|a| a.is_writable)
            .collect::<Vec<_>>()
    );

    // The listing warm-up, then an open.
    let warmup = h.with_env(|env| i64::from(env.config().params.market_warmup_s));
    let wait = (listed + warmup - h.now()).max(0);
    h.idle(wait);
    let (b, nonce) = h.with_env(|env| {
        env.crank(&pm);
        env.warp(1);
        let q = q_for(env, &pm.pool);
        let mut b = env.borrower();
        let (res, nonce) = env.open(&pm, &mut b, q);
        ok(res);
        (b, nonce)
    });
    let position = pda::position(&market, &b.key(), nonce).0;
    h.with_env(|env| {
        let whale = PammUser::new(env, &pm.pool, 50_000 * SOL);
        ok(whale.push_price(env, &pm.pool, 3, 1));
    });
    let outsider = h.with_env(|env| env.funded_keypair());
    let mut liquidated = false;
    for _ in 0..300 {
        h.idle(1);
        let ix =
            shortwin_litesvm_tests::engine::liquidate_ix(&pm, &outsider.pubkey(), &b.key(), nonce);
        let before = accepts(&h, &outsider, ix);
        let r = h.tick().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        if r.exits.iter().any(|e| {
            e.position == position && e.kind == ExitKind::Liquidate && e.result.landed().is_some()
        }) {
            assert_eq!(before, Ok(()), "the program accepted it this second");
            liquidated = true;
            break;
        }
        assert_ne!(
            before,
            Ok(()),
            "the keeper missed a second the program accepted"
        );
    }
    assert!(liquidated, "liquidated on the pool");
    assert!(
        h.with_env(|env| read::borsh::<shortwin::state::ShortPosition>(&env.svm, &position))
            .is_none()
    );
}

/// The PumpSwap admin changes of one tick (duty 10).
fn pamm_changes(r: &TickReport, market: &Pubkey) -> Vec<AdminChange> {
    r.alerts
        .iter()
        .filter_map(|a| match a {
            Alert::VenueAdmin {
                market: m, change, ..
            } if m == market => Some(change.clone()),
            _ => None,
        })
        .collect()
}

/// Duty 10 on PumpSwap (A24; ARCH §5.3, §5.5): the admin's `disable_flags`. Bit 0 set: reported
/// (severe: opens refuse `VenueFlagged`), the route intact; the buy bit (T-DISABLE bit 3): the
/// route lost as well (no buy, S3 evidence); cleared: the route restored — each once.
#[test]
fn pumpswap_disable_flags_are_reported_and_the_buy_bit_is_a_lost_route() {
    let (mut env, pm) = Env::pamm_market(PammCoin::Mature, 1_000_000_000_000);
    let market = pm.market();
    env.warp(1);
    env.crank(&pm);
    let mut h = Harness::new(env, KeeperConfig::default());
    let r = h.second().clone();
    assert!(pamm_changes(&r, &market).is_empty(), "first sight, flags 0");

    h.with_env(|env| pamm::set_disable_flags(&mut env.svm, 1));
    let r = h.second().clone();
    let c = pamm_changes(&r, &market);
    assert_eq!(
        c,
        [AdminChange::PammFlags {
            from: Some(0),
            to: 1,
            buys_halted: false
        }]
    );
    assert!(c[0].is_severe());
    h.with_env(|env| {
        env.crank(&pm);
        let mut b = env.borrower();
        let (res, _) = env.open(&pm, &mut b, 1_000_000_000);
        assert_err(&res, ShortwinError::VenueFlagged);
    });
    let r = h.second().clone();
    assert!(pamm_changes(&r, &market).is_empty(), "once");

    h.with_env(|env| pamm::set_disable_flags(&mut env.svm, 1 << 3));
    let r = h.second().clone();
    let c = pamm_changes(&r, &market);
    assert_eq!(c.len(), 2, "{c:?}");
    assert!(matches!(&c[0], AdminChange::RouteLost { detail } if detail.contains("bit 3")));
    assert_eq!(
        c[1],
        AdminChange::PammFlags {
            from: Some(1),
            to: 8,
            buys_halted: true
        }
    );

    h.with_env(|env| pamm::set_disable_flags(&mut env.svm, 0));
    let r = h.second().clone();
    let c = pamm_changes(&r, &market);
    assert!(matches!(c[0], AdminChange::RouteRestored { .. }), "{c:?}");
    assert_eq!(
        c[1],
        AdminChange::PammFlags {
            from: Some(8),
            to: 0,
            buys_halted: false
        }
    );
    assert!(!c[1].is_severe());
}

/// The existence requirement (ARCH §3.12; duty 6): a cashback coin listed without the trader's
/// cashback ATA `ATA(pAMM UVA(trader), WSOL)` (T18) cannot trade (`VenueAccountMismatch`); the
/// keeper creates it at its first tick (paying), once.
#[test]
fn the_keeper_creates_a_pumpswap_markets_missing_pool_accounts() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Cashback);
    let q = q_for(&env, &p);
    ok(env.list_pamm(&p.mint, &p.base_program));
    // A lender, without the harness's venue accounts.
    let lender = env.funded_keypair();
    let program = p.token_program();
    let acc = fork::fund_token(&mut env, &lender.pubkey(), &p.mint, program, q * 40);
    let pm = PammMarket::new(&env.svm, &p.mint, lender, acc);
    let kp = pm.lender.insecure_clone();
    ok(env.send(&[ix::deposit(&pm.lender_keys(), q * 40, 0)], &[&kp]));
    let cashback_ata = pm.cashback_ata();
    assert!(env.svm.get_account(&cashback_ata).is_none(), "T18 missing");
    let mut h = Harness::new(env, KeeperConfig::default());
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.pamm_accounts.len(), 1, "{:?}", r.pamm_accounts);
    assert!(r.pamm_accounts[0].1.landed().is_some());
    assert!(h.with_env(|env| env.svm.get_account(&cashback_ata).is_some()));
    for _ in 0..3 {
        let r = h.second().clone();
        assert!(r.pamm_accounts.is_empty(), "once");
    }
}

/// `crank_price`'s `None → PumpSwap` re-detection by the keeper (M6; ARCH §2.5 `None` row): a
/// graduated coin listed while its canonical pool was unreadable (the pool account removed for the
/// listing, restored after) is list-only, with neither the trader's base ATA nor its pAMM UVA (a
/// list-only listing creates neither). The keeper sees the valid pool in the detection set (the
/// program's `classify_pump`) and cranks the market onto it; at its next tick it creates the
/// trader accounts a PumpSwap leg requires (the base ATA, the pAMM UVA; `crank_price` creates
/// nothing), once, and warms the price up; a borrower's open then lands on the pool.
#[test]
fn a_list_only_market_is_redetected_onto_its_pool_and_prepared_by_the_keeper() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Mature);
    let saved = env.svm.get_account(&p.pool).unwrap();
    env.svm
        .set_account(p.pool, solana_account::Account::default())
        .unwrap();
    let creator = env.funded_keypair();
    let list = ix::create_market(
        &creator.pubkey(),
        &p.mint,
        &p.base_program,
        shortwin::instructions::create_market::VENUE_HINT_AUTO,
        &pda::canonical_pool(&p.mint),
    );
    ok(env.send(&[list], &[&creator]));
    env.svm.set_account(p.pool, saved).unwrap();
    let q = q_for(&env, &p);
    let lender = env.funded_keypair();
    let acc = fork::fund_token(
        &mut env,
        &lender.pubkey(),
        &p.mint,
        p.token_program(),
        q * 40,
    );
    let pm = PammMarket::new(&env.svm, &p.mint, lender, acc);
    let kp = pm.lender.insecure_clone();
    ok(env.send(&[ix::deposit(&pm.lender_keys(), q * 40, 0)], &[&kp]));
    let market = pm.market();
    assert_eq!(env.market(&market).venue_kind(), Some(VenueKind::None));
    let missing = |env: &Env| {
        [pm.trader_base_ata, pm.trader_uva()]
            .map(|k| env.svm.get_account(&k).is_none_or(|a| a.data.is_empty()))
    };
    assert_eq!(
        missing(&env),
        [true, true],
        "a list-only listing creates neither"
    );
    let listed = env.now();
    let mut h = Harness::new(env, KeeperConfig::default());
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(r.cranked.contains(&market), "the keeper cranks it");
    assert_eq!(
        h.with_env(|env| env.market(&market).venue_kind()),
        Some(VenueKind::PumpSwap)
    );
    assert_eq!(
        h.with_env(|env| missing(env)),
        [true, true],
        "the crank created nothing"
    );
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.pamm_accounts.len(), 1, "{:?}", r.pamm_accounts);
    assert!(r.pamm_accounts[0].1.landed().is_some());
    assert_eq!(
        h.with_env(|env| missing(env)),
        [false, false],
        "the keeper created both"
    );
    for _ in 0..10 {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.pamm_accounts.is_empty(), "once");
    }
    // The listing warm-up, then an open on the pool.
    let warmup = h.with_env(|env| i64::from(env.config().params.market_warmup_s));
    h.idle((listed + warmup - h.now()).max(0));
    h.with_env(|env| {
        env.crank(&pm);
        env.warp(1);
        let q = q_for(env, &pm.pool);
        let mut b = env.borrower();
        let (res, _) = env.open(&pm, &mut b, q);
        ok(res);
    });
}
