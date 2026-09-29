//! Graduation on the curve (ARCH §2.5 `sync_venue`, §3.8 step 4, §5.3, §5.4; M5): the venue
//! state machine `PumpCurve → PumpMigrating` (a completing buyback, an external completion seen
//! by a crank), the migration pause (exits refuse with `VenueMigrating`, S3 only after
//! `max_migration_pause_s + s3_wait_s` — the stuck migration at 70 min, FV on the deterministic
//! post-migration pool), a real `migrate_v2` that leaves the market `PumpMigrating` until M6's
//! `→ PumpSwap`, a curve that graduated elsewhere (`→ None`, the `graduated_elsewhere` flag, the
//! detection set as ReadSet, S3 after the chain: M2-SEC-5 on a real transition), and the
//! `None → PumpCurve` re-detection by `crank_price` (the curve cache, the price start, the
//! collection window cleared). M4's two carry-overs (ARCH §9 M4 → M5): a curve that graduated
//! elsewhere **with a position open** adopts its Raydium pool only through M4's collection window
//! (switch only if all fit; the deepest candidate; the position carried and closed on the new
//! pool), and the `None →` pump re-detection clears a running window (`fp_*`) and the route chains.
//!
//! Every instruction goes through `Env::send`: the invariant hooks run after each one. The only
//! planted state is the venue's (pump's curve account: reserves zeroed as by a migration to
//! another venue; a curve account removed before a listing and restored after it; a completed
//! curve made live again, the one state a re-detection with a window open needs; Raydium CPMM
//! pools of the pump mint planted from the dumped USELESS pool, `pool_select::plant_cpmm`).

use shortwin::state::{MARKET_FLAG_GRADUATED_ELSEWHERE, Market};
use shortwin_litesvm_tests::engine::expire_ix;
use shortwin_math::fees::{self, FeeBps, PammFeeSchedule};
use shortwin_math::quote::{self as mq, MigrationParams, sol_leg};
use shortwin_math::rent;
use shortwin_venue_state::{pump as vpump, pump_amm as vamm};

use crate::common::*;

/// The venue kind stored on the market.
fn venue(env: &Env, pm: &PumpMarket) -> VenueKind {
    market(env, pm).venue_kind().expect("venue")
}

/// `B̂_FV(q; P)` of a `PumpMigrating` market (ARCH §4.5, `Quoter::fv_repriced`), recomputed from
/// the math crate and the dumped accounts: the deterministic pool `migrate_v2` builds from the
/// complete curve (`migrated_pool` with the standard 206.9 M tokens), repriced at `P` with its
/// own liquidity, at its PumpSwap tier with the position's hard cap (the risk view).
fn migrating_fv_quote(env: &Env, pm: &PumpMarket, q: u64, p: u128, cap_bps: u64) -> u64 {
    let live = pm.curve.curve_state(&env.svm);
    assert!(live.complete && live.vt > 0, "a stuck migration");
    let g = env
        .svm
        .get_account(&shortwin::constants::PUMP_GLOBAL)
        .unwrap();
    let global = vpump::read_global(&g.owner.to_bytes(), &g.data).unwrap();
    let pfc = env
        .svm
        .get_account(&shortwin::constants::PUMP_AMM_FEE_CONFIG)
        .unwrap();
    let tiers: Vec<fees::FeeTier> = vpump::read_fee_config(&pfc.owner.to_bytes(), &pfc.data)
        .unwrap()
        .fee_tiers
        .iter()
        .map(|t| fees::FeeTier {
            threshold: t.threshold,
            fees: FeeBps::new(t.fees.lp, t.fees.protocol, t.fees.creator),
        })
        .collect();
    let gc = env
        .svm
        .get_account(&shortwin::constants::PUMP_AMM_GLOBAL_CONFIG)
        .unwrap();
    let gcfg = vamm::read_global_config(&gc.owner.to_bytes(), &gc.data).unwrap();
    let (xp, yp) = mq::migrated_pool(
        &live,
        &MigrationParams {
            migration_fee: global.pool_migration_fee(),
            pool_tokens: shortwin::venues::STANDARD_POOL_TOKENS,
        },
    )
    .unwrap();
    let f = fees::pump_amm(
        yp,
        xp,
        &PammFeeSchedule {
            tiers: &tiers,
            base_supply: 1_000_000_000_000_000,
            creator_fee_configurable: gcfg.creator_fee_configurable(),
            pool_creator_fee_bps: 0,
            coin_creator_is_default: pm.curve.state(&env.svm).creator_is_default(),
            cap_bps: Some(cap_bps),
        },
    )
    .unwrap();
    let l_pool = mq::liquidity(xp, yp).unwrap();
    let (x, y) = mq::reprice(l_pool, p).unwrap();
    sol_leg::buy_exact_out(q, x, y, false, &f).unwrap().cost
}

/// A buyback of exactly `rT` completes the curve: the close succeeds (pump's charge, `q` back in
/// the lending vault) and the venue sync moves the market to `PumpMigrating` in the same
/// instruction (`VenueStateChanged` before `Close`, `migration_seen_at = now`); the next open
/// is refused with `VenueMigrating`.
#[test]
fn a_completing_buyback_moves_the_market_to_pump_migrating() {
    let (mut env, pm, q) = setup(PumpCoin::Near);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    env.warp(5);
    buy_down_to(&mut env, &pm, q);
    env.warp(1);
    let charge = pm
        .curve
        .charge_buy(&env.svm, q)
        .expect("the completing charge");
    let vault0 = tokens(&env, &pm.keys.pool_vault);
    let (close, meta) = close_ok(&mut env, &pm, &b, nonce);
    assert_eq!(close.buyback_cost, charge.cost, "B = pump's charge for rT");
    assert_eq!(tokens(&env, &pm.keys.pool_vault) - vault0, q);
    let c = pm.curve.state(&env.svm);
    assert!(
        c.complete && c.real_token_reserves == 0,
        "the curve completed"
    );
    let ev = venue_changed(&meta, VenueKind::PumpCurve, VenueKind::PumpMigrating);
    assert_eq!(ev.market, pm.market());
    assert_eq!(ev.pool, pda::canonical_pool(&pm.keys.mint));
    assert_eq!(ev.ts, env.now());
    let m = market(&env, &pm);
    assert_eq!(venue(&env, &pm), VenueKind::PumpMigrating);
    assert_eq!(m.migration_seen_at, env.now());
    assert!(!m.has_flag(MARKET_FLAG_GRADUATED_ELSEWHERE));
    assert_trader_clean(&env, &pm);
    // The event precedes `Close` in the trace.
    let raw = events::raw(&meta);
    let pos = |d: &[u8]| raw.iter().position(|p| p.starts_with(d)).unwrap();
    use anchor_lang::Discriminator;
    assert!(
        pos(shortwin::events::VenueStateChanged::DISCRIMINATOR) < pos(Close::DISCRIMINATOR),
        "VenueStateChanged is emitted before Close"
    );
    env.warp(1);
    let mut b2 = env.borrower();
    let (res, _) = env.open(&pm, &mut b2, q);
    assert_err(&res, E::VenueMigrating);
    // A crank on the migrating curve changes nothing more.
    env.warp(1);
    no_venue_change(&crank(&mut env, &pm));
}

/// A stuck migration (ARCH §5.3, §5.4; the M5 test "stuck-migration S3 at 70 min"): a third
/// party completes the curve 30 min before the position's deadline; the next crank moves the
/// market to `PumpMigrating`; opens, the close, the expiry and a liquidation are refused with
/// `VenueMigrating` (exits pause); the keeper's `sol_settle` does nothing until
/// `migration_seen_at + max_migration_pause_s + s3_wait_s` (70 min with the defaults) on a
/// continuous chain, then settles S3 with FV priced on the deterministic post-migration pool
/// at `P_ema` (the risk view: the position's hard cap), the expiry fee on top (O15).
#[test]
fn a_stuck_migration_pauses_exits_and_settles_s3_after_the_pause() {
    let (mut env, pm, q) = setup(PumpCoin::Near);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    let kp = keeper(&mut env);
    warp_to_deadline(&mut env, &pm, &b.key(), nonce, -1_800);
    crank(&mut env, &pm);
    env.warp(1);
    buy_down_to(&mut env, &pm, 0);
    // The completing trade ran outside short.win: the market is still `PumpCurve` until an
    // instruction reads the curve.
    assert_eq!(venue(&env, &pm), VenueKind::PumpCurve);
    env.warp(1);
    let meta = crank(&mut env, &pm);
    venue_changed(&meta, VenueKind::PumpCurve, VenueKind::PumpMigrating);
    let t0 = env.now();
    let m = market(&env, &pm);
    assert_eq!((m.migration_seen_at, m.no_route_since), (t0, t0));
    let pos = env.position(&pm, &b.key(), nonce);
    assert_eq!(
        (pos.max_migration_pause_s, pos.s3_wait_s),
        (3_600, 600),
        "the default timers"
    );
    // Exits pause; opens are refused.
    env.warp(1);
    assert_err(&env.close(&pm, &b, nonce), E::VenueMigrating);
    let mut b2 = env.borrower();
    assert_err(&env.open(&pm, &mut b2, q).0, E::VenueMigrating);
    let liq = shortwin_litesvm_tests::engine::liquidate_ix(&pm, &kp.pubkey(), &b.key(), nonce);
    assert_err(&env.send(&[liq], &[&kp]), E::VenueMigrating);
    // The keeper: a crank and a `sol_settle` every 30 s (ECON §5.8 duty 5).
    let due = t0 + i64::from(pos.max_migration_pause_s) + i64::from(pos.s3_wait_s);
    let mut expired_checked = false;
    let ev = loop {
        env.warp(30);
        crank(&mut env, &pm);
        if !expired_checked && env.now() > pos.deadline {
            let ix = expire_ix(&pm, &kp.pubkey(), &b.key(), nonce);
            assert_err(&env.send(&[ix], &[&kp]), E::VenueMigrating);
            expired_checked = true;
        }
        let meta = ok(settle(&mut env, &pm, &kp, &b.key(), nonce));
        if let Some(ev) = settled(&meta) {
            break ev;
        }
        assert!(env.now() < due, "S3 is due at migration_seen_at + 70 min");
    };
    assert!(expired_checked);
    assert!(env.now() >= due && env.now() < due + 30, "S3 at 70 min");
    assert_eq!(ev.condition, 3);
    assert_eq!(venue(&env, &pm), VenueKind::PumpMigrating);
    assert_eq!(
        (ev.pprime_lo, ev.pprime_hi),
        (pos.expiry_fee_bps, pos.expiry_fee_bps)
    );
    let m = market(&env, &pm);
    let b_fv = migrating_fv_quote(
        &env,
        &pm,
        pos.q,
        m.ema_price.get(),
        u64::from(pos.venue_fee_hard_cap_bps),
    );
    let r_due = rent::due(&pos.pos_rent(), &m.basis()).unwrap();
    let fv =
        b_fv + (u128::from(b_fv) * u128::from(pos.expiry_fee_bps)).div_ceil(10_000) as u64 + r_due;
    assert_eq!(ev.fv, fv, "FV on the post-migration pool at P_ema");
    let take = pos.v - k_s(&pos);
    assert_eq!(ev.lender_amount, fv.min(take));
    assert!(read::account(&env.svm, &pda::position(&pm.market(), &b.key(), nonce).0).is_none());
}

/// `migrate_v2` on the real pump ELF (anyone may call it) after the completion: the canonical
/// pool appears with exactly the deterministic reserves short.win prices on a complete curve
/// (`X_p` = the curve's 206.9 M pool tokens, effective quote `Y_p = rQ − pool_migration_fee`)
/// and the boost of `⌊0.2069·Y⌋`; the curve's reserves are zeroed. (Priced before the
/// completion, `Y_p` is one lamport above: `straddle::b_str_prices_the_executed_bundle`.) In M5 the market stays
/// `PumpMigrating` (the `→ PumpSwap` transition is M6's): cranks change nothing, exits still
/// refuse.
#[test]
fn migrate_v2_on_the_real_elf_keeps_the_market_migrating() {
    let (mut env, pm, q) = setup(PumpCoin::Near);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    env.warp(1);
    buy_down_to(&mut env, &pm, 0);
    env.warp(1);
    venue_changed(
        &crank(&mut env, &pm),
        VenueKind::PumpCurve,
        VenueKind::PumpMigrating,
    );
    let live = pm.curve.curve_state(&env.svm);
    let g = env
        .svm
        .get_account(&shortwin::constants::PUMP_GLOBAL)
        .unwrap();
    let fee = vpump::read_global(&g.owner.to_bytes(), &g.data)
        .unwrap()
        .pool_migration_fee();
    let pool_tokens = u128::from(tokens(&env, &pm.curve.curve_base_ata));
    let (xp, yp) = mq::migrated_pool(
        &live,
        &MigrationParams {
            migration_fee: fee,
            pool_tokens,
        },
    )
    .unwrap();
    let migrator = env.funded_keypair();
    let ix = pump::migrate_v2_ix(&env.svm, &migrator.pubkey(), &pm.curve);
    let meta = ok(env.send(&[pump::cu_limit_ix(600_000), ix], &[&migrator]));
    eprintln!(
        "migrate_v2: {} CU, {} inner instructions",
        meta.compute_units_consumed,
        meta.inner_instructions.iter().map(Vec::len).sum::<usize>()
    );
    let np = pump::NewPool::of(&pm.curve);
    let acc = env.svm.get_account(&np.pool).expect("the canonical pool");
    assert_eq!(acc.data.len(), 271, "a new pool is 271 bytes");
    let r = np.reserves(&env.svm).expect("reserves");
    assert_eq!(u128::from(r.base), xp, "X_p");
    assert_eq!(pool_tokens, shortwin::venues::STANDARD_POOL_TOKENS);
    assert_eq!(
        r.effective_quote, yp,
        "Y_p of the complete curve (rQ − migration fee) is the pool's effective quote"
    );
    let st = np.state(&env.svm).unwrap();
    assert_eq!(
        st.virtual_quote_reserves,
        i128::try_from(r.effective_quote * 2_069 / 10_000).unwrap(),
        "the boost"
    );
    let c = pm.curve.state(&env.svm);
    assert_eq!(
        (c.virtual_token_reserves, c.real_token_reserves),
        (0, 0),
        "migrated"
    );
    env.warp(1);
    no_venue_change(&crank(&mut env, &pm));
    assert_eq!(venue(&env, &pm), VenueKind::PumpMigrating, "M6 moves it on");
    env.warp(1);
    assert_err(&env.close(&pm, &b, nonce), E::VenueMigrating);
}

/// A curve that completed and then migrated **elsewhere** (reserves zeroed, no canonical pool;
/// planted on pump's account) with a position open: the next crank moves `PumpMigrating →
/// None` and sets `graduated_elsewhere`; from then on the market's ReadSet is the detection set
/// (the curve's own set is refused), exits have no route (`NoRoute`), and an expired position
/// settles S3 once the chain has run `s3_wait_s` (M2-SEC-5 on a real transition): FV at the
/// cap-only risk view at the frozen EMA point.
#[test]
fn a_curve_that_graduates_elsewhere_goes_list_only_and_settles_s3() {
    let (mut env, pm, q) = setup(PumpCoin::Near);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    let kp = keeper(&mut env);
    warp_to_deadline(&mut env, &pm, &b.key(), nonce, 1);
    crank(&mut env, &pm);
    env.warp(1);
    buy_down_to(&mut env, &pm, 0);
    env.warp(1);
    venue_changed(
        &crank(&mut env, &pm),
        VenueKind::PumpCurve,
        VenueKind::PumpMigrating,
    );
    // Pump migrates it to another venue: reserves zeroed, no canonical pool.
    fork::set_curve_reserves(&mut env.svm, &pm.curve.curve, 0, 0, 0, 0);
    env.warp(1);
    let meta = crank(&mut env, &pm);
    let ev = venue_changed(&meta, VenueKind::PumpMigrating, VenueKind::None);
    assert_eq!(ev.pool, pda::canonical_pool(&pm.keys.mint));
    let m = market(&env, &pm);
    assert_eq!(venue(&env, &pm), VenueKind::None);
    assert!(m.has_flag(MARKET_FLAG_GRADUATED_ELSEWHERE));
    // The detection set is the ReadSet now.
    env.warp(1);
    assert_err(
        &env.send(&[crank_price_ix(&pm)], &[]),
        E::VenueAccountMismatch,
    );
    let none = AsNone(&pm);
    crank(&mut env, &none);
    let pos = env.position(&pm, &b.key(), nonce);
    let ix = expire_ix(&pm, &kp.pubkey(), &b.key(), nonce);
    assert_err(&env.send(&[ix], &[&kp]), E::NoRoute);
    let m0 = market(&env, &pm);
    let since = m0.no_route_since;
    assert!(since > 0);
    let ev = loop {
        env.warp(30);
        crank(&mut env, &none);
        let meta = ok(settle(&mut env, &none, &kp, &b.key(), nonce));
        if let Some(ev) = settled(&meta) {
            break ev;
        }
        assert!(env.now() < since + i64::from(pos.s3_wait_s));
    };
    assert_eq!(ev.condition, 3);
    assert!(env.now() >= since + i64::from(pos.s3_wait_s));
    let m = market(&env, &pm);
    let (p, l) = (m.ema_price.get(), m.ema_liq.get());
    assert_eq!((p, l), (m0.ema_price.get(), m0.ema_liq.get()), "frozen");
    assert_eq!((ev.p_lo, ev.p_hi, ev.l_lo, ev.l_hi), (p, p, l, l));
    let cap = shortwin_math::quote::Pricing::PumpSwap(FeeBps::new(
        0,
        u64::from(pos.venue_fee_hard_cap_bps),
        0,
    ));
    let bq = cap.buy_repriced(pos.q, l, p).unwrap().cost;
    let r_due = rent::due(&pos.pos_rent(), &m.basis()).unwrap();
    let fv = bq + (u128::from(bq) * u128::from(pos.expiry_fee_bps)).div_ceil(10_000) as u64 + r_due;
    assert_eq!(ev.fv, fv, "FV at the cap-only risk view");
    assert_eq!(ev.lender_amount, fv.min(pos.v - k_s(&pos)));
}

/// A live curve whose completion and migration elsewhere both happened between two reads:
/// `PumpCurve → None` directly (complete, `vT = 0`, no canonical pool).
#[test]
fn a_live_curve_seen_migrated_elsewhere_goes_straight_to_none() {
    let (mut env, pm, _q) = setup(PumpCoin::T22_115);
    fork::set_curve_complete(&mut env.svm, &pm.curve.curve, true);
    fork::set_curve_reserves(&mut env.svm, &pm.curve.curve, 0, 0, 0, 0);
    env.warp(1);
    let meta = crank(&mut env, &pm);
    venue_changed(&meta, VenueKind::PumpCurve, VenueKind::None);
    assert!(market(&env, &pm).has_flag(MARKET_FLAG_GRADUATED_ELSEWHERE));
}

/// `None → PumpCurve` by `crank_price` (ARCH §2.5 `None` row): a mint listed before its curve
/// was readable (the curve account removed for the listing, restored after) is re-detected by
/// the next crank on its detection set: `VenueStateChanged(None → PumpCurve)`, the curve cache
/// written (as a listing does), the price state started at the curve's spot, Tier D, the S3
/// chain and the collection window cleared. The crank creates no account: the trader's base
/// ATA and pump UVA are pre-created by anyone (here the payer) before the first open, which is
/// then exact.
#[test]
fn a_list_only_market_is_redetected_as_a_curve() {
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::T22_115);
    let saved = env.svm.get_account(&c.curve).unwrap();
    env.svm
        .set_account(c.curve, solana_account::Account::default())
        .unwrap();
    // A list-only listing takes no init set.
    let creator = env.funded_keypair();
    let list = shortwin_litesvm_tests::ix::create_market(
        &creator.pubkey(),
        &c.mint,
        &c.token_program.id(),
        shortwin::instructions::create_market::VENUE_HINT_AUTO,
        &pda::canonical_pool(&c.mint),
    );
    ok(env.send(&[list], &[&creator]));
    let market_key = pda::market(&c.mint).0;
    assert_eq!(
        env.market(&market_key).venue_kind(),
        Some(VenueKind::None),
        "no curve: list-only"
    );
    env.svm.set_account(c.curve, saved).unwrap();
    let vt = c.state(&env.svm).virtual_token_reserves;
    let pm = env.pump_market_listed(PumpSpec::new(PumpCoin::T22_115, (vt / 200) * 40), c);
    let meta = crank(&mut env, &AsNone(&pm));
    let ev = venue_changed(&meta, VenueKind::None, VenueKind::PumpCurve);
    assert_eq!(ev.ts, env.now());
    let m: Market = market(&env, &pm);
    assert_eq!(venue(&env, &pm), VenueKind::PumpCurve);
    assert_eq!(m.tier, shortwin_math::tier::Tier::D as u8);
    assert_eq!(m.curve_base_ata, pm.curve.curve_base_ata);
    assert_eq!(m.cached_creator_vault, pm.creator_vault);
    let s = pm.curve.state(&env.svm);
    let (vt, vq) = (
        u128::from(s.virtual_token_reserves),
        u128::from(s.virtual_quote_reserves),
    );
    assert_eq!(m.ema_price.get(), vq * shortwin_math::num::WAD / vt);
    assert_eq!(m.fp_t0, 0, "no collection window");
    assert_eq!(m.fp_d.get(), 0);
    assert_eq!(m.no_route_since, 0);
    // The crank created nothing: the first open needs the trader's accounts.
    let trader = pm.trader();
    assert!(read::account(&env.svm, &pm.trader_base_ata).is_none());
    let payer = env.payer.pubkey();
    let program = pm.curve.token_program.id();
    let ixs = [
        fork::create_ata_idempotent_ix(&payer, &trader, &pm.keys.mint, &program),
        fork::pump_init_uva_ix(&payer, &trader),
    ];
    ok(env.send(&ixs, &[]));
    env.warp(i64::from(env.config().params.market_warmup_s));
    let q = q_for(&env, &pm);
    crate::flows::round_trip(&mut env, &pm, q);
}

// ---- M4's carry-overs (ARCH §2.5 `None` row, §9 M4 / M5) ------------------------------------------

/// A CPMM pool for `pm`'s mint planted at the curve's spot now (WSOL token 0, `λ` 100 %, the
/// USELESS template's config), `tokens` deep: a Raydium pool the coin could have graduated to.
fn plant_at_spot(env: &mut Env, pm: &PumpMarket, tokens: u64) -> fork::AnyPool {
    use shortwin_litesvm_tests::pool_select::{PlantSpec, plant_cpmm};
    let s = pm.curve.state(&env.svm);
    let sol = (u128::from(tokens) * u128::from(s.virtual_quote_reserves)
        / u128::from(s.virtual_token_reserves)) as u64;
    fork::AnyPool::Cpmm(plant_cpmm(
        env,
        fork::CpmmFixture::Useless,
        PlantSpec {
            mint: pm.keys.mint,
            program: pm.curve.token_program,
            sol_first: true,
            sol,
            tokens,
            lp_counter: 1_000_000_000_000,
            lambda_bps: 10_000,
        },
    ))
}

/// The curve completes with a position open, then pump migrates it **elsewhere** (reserves zeroed,
/// no canonical pool; planted on pump's account): `PumpCurve → PumpMigrating → None` with the
/// `graduated_elsewhere` flag. Returns the market, the borrower, the position's nonce and `q`.
fn graduated_elsewhere_with_a_position() -> (Env, PumpMarket, Borrower, u64, u64) {
    let (mut env, pm, q) = setup(PumpCoin::Near);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    env.warp(1);
    crank(&mut env, &pm);
    env.warp(1);
    buy_down_to(&mut env, &pm, 0);
    env.warp(1);
    venue_changed(
        &crank(&mut env, &pm),
        VenueKind::PumpCurve,
        VenueKind::PumpMigrating,
    );
    // Returned before pump zeroes the curve: a test plants its pools at the completed spot first.
    (env, pm, b, nonce, q)
}

/// Pump migrates the completed curve elsewhere (reserves zeroed, no canonical pool) and a crank
/// sees it: `PumpMigrating → None`, `graduated_elsewhere`.
fn migrate_elsewhere(env: &mut Env, pm: &PumpMarket) {
    fork::set_curve_reserves(&mut env.svm, &pm.curve.curve, 0, 0, 0, 0);
    env.warp(1);
    venue_changed(&crank(env, pm), VenueKind::PumpMigrating, VenueKind::None);
    let m = market(env, pm);
    assert_eq!(m.venue_kind(), Some(VenueKind::None));
    assert!(m.has_flag(MARKET_FLAG_GRADUATED_ELSEWHERE));
    assert_eq!(m.open_positions, 1);
}

/// **M4 carry-over** (ARCH §2.5 `None` row and footnote *, M4-SEC-4; §9 M5): a curve market whose
/// curve graduates **elsewhere** with a position open becomes list-only (`→ None`) with the
/// position open, and its Raydium pool is adopted only through M4's collection window, on the real
/// transition (M4's `m4_sec_4_*` simulated it by rewriting the venue byte): a pool the position
/// does not fit is refused outright (switch only if all fit, `CarriedPositionsTooLarge`); a fitting
/// one opens the window (`PoolProposed`, kind fast — no first-come adoption); a deeper one replaces
/// it inside the window and the first can no longer come back (`PoolNotDominant`); the execution
/// before the window ends is refused (`PoolSwitchNotDue`); after `pool_fast_window_s` the deepest
/// is adopted (`PoolSwitched`, kind fast) with the position carried (its locked terms unchanged),
/// the market leaves the no-route chain, and the borrower closes on the new pool. The pools are
/// planted CPMM pools of the pump mint at the completed curve's price (the dumped ELF trades them).
#[test]
fn a_curve_that_graduated_elsewhere_adopts_its_raydium_pool_through_the_window() {
    use shortwin::events::{POOL_KIND_FAST, PoolProposed, PoolSwitched};
    use shortwin_litesvm_tests::pool_select::{carried_caps, fixture_of, pool_key};
    use shortwin_litesvm_tests::raydium::RayMarket;

    let (mut env, pm, b, nonce, q) = graduated_elsewhere_with_a_position();
    // Candidates priced at the completed curve's spot (where the EMA stands).
    let thin = plant_at_spot(&mut env, &pm, 100 * q);
    let fits = plant_at_spot(&mut env, &pm, 400 * q);
    let deep = plant_at_spot(&mut env, &pm, 1_200 * q);
    migrate_elsewhere(&mut env, &pm);
    let pos_key = pda::position(&pm.market(), &b.key(), nonce).0;
    let pos0 = env.svm.get_account(&pos_key).unwrap().data;
    let since = market(&env, &pm).no_route_since;
    assert!(since > 0, "no route: the S3 chain runs");
    let (cap_thin, _) = carried_caps(&env, &thin);
    let (cap_fits, _) = carried_caps(&env, &fits);
    assert!(
        cap_thin < q && q <= cap_fits,
        "the thin pool's cap {cap_thin} < q {q} ≤ the fitting pool's {cap_fits}"
    );

    // Switch only if all fit: the thin pool cannot open the window.
    assert_err(&env.propose(None, &thin), E::CarriedPositionsTooLarge);
    assert_eq!(market(&env, &pm).fp_t0, 0, "no window");
    // A fitting pool opens the window; nothing is adopted first-come.
    let meta = ok(env.propose(None, &fits));
    assert!(
        events::of::<PoolSwitched>(&meta).is_empty(),
        "no first-come adoption"
    );
    let w: PoolProposed = events::one(&meta);
    assert_eq!((w.kind, w.candidate), (POOL_KIND_FAST, pool_key(&fits)));
    assert_eq!(market(&env, &pm).venue_kind(), Some(VenueKind::None));
    // A deeper pool replaces the window's candidate; the shallower cannot come back.
    env.warp(30);
    let w2: PoolProposed = events::one(&ok(env.propose(None, &deep)));
    assert_eq!(w2.candidate, pool_key(&deep));
    assert_eq!(w2.window_ends, w.window_ends, "the window is not extended");
    assert_err(&env.propose(None, &fits), E::PoolNotDominant);
    assert_err(&env.execute_switch_list_only(&deep), E::PoolSwitchNotDue);
    // After the window: the deepest is adopted with the position carried.
    let now = env.now();
    env.warp(w.window_ends - now);
    let meta = ok(env.execute_switch_list_only(&deep));
    let e: PoolSwitched = events::one(&meta);
    assert_eq!((e.kind, e.candidate), (POOL_KIND_FAST, pool_key(&deep)));
    let m = market(&env, &pm);
    assert_eq!(m.venue_kind(), Some(VenueKind::RaydiumCpmm));
    assert_eq!(m.open_positions, 1);
    assert_eq!(
        (m.fp_t0, m.no_route_since),
        (0, 0),
        "window and chain cleared"
    );
    assert_eq!(
        env.svm.get_account(&pos_key).unwrap().data,
        pos0,
        "the position (its locked terms) is carried byte for byte"
    );
    // The position closes on the new pool.
    let amm_config = match &deep {
        fork::AnyPool::Cpmm(p) => Some(p.amm_config),
        fork::AnyPool::V4(_) => None,
    };
    let rm = RayMarket {
        fixture: fixture_of(&deep),
        pool: deep,
        keys: pm.keys,
        trader_wsol: pda::trader_wsol(&pm.market()).0,
        lender: pm.lender.insecure_clone(),
        lender_account: pm.lender_account,
        amm_config,
    };
    env.warp(1);
    crank(&mut env, &rm);
    let meta = ok(env.close(&rm, &b, nonce));
    let c: Close = events::one(&meta);
    assert_eq!(c.q, q);
    assert_eq!(market(&env, &pm).open_positions, 0);
    println!(
        "graduated elsewhere → None with a position (q {q}): a pool of 100·q tokens (above the window floor) refused (its cap {cap_thin} < q), one of 400·q (cap {cap_fits}) opened the window, one of 1,200·q replaced it and was adopted {} s after the transition; the position closed on it (buyback {} lamports)",
        w.window_ends - since,
        c.buyback_cost,
    );
}

/// **M4 carry-over** (ARCH §2.5 "Precedence", D1; §9 M4 → M5): `crank_price`'s `None →` pump
/// re-detection **clears a collection window**. A list-only market with a position open (a curve
/// that graduated elsewhere) has a Raydium window running; its detection set then shows a live
/// pump curve again (planted on pump's account: a state mainnet cannot reach — a migration is
/// final — and the only way to reach this branch with a window open, as M4's `pump_appears`
/// variant planted it) and the next crank moves the market onto the curve (`VenueStateChanged
/// None → PumpCurve`), clearing `fp_*` and the no-route chain; the window's execution then has
/// nothing to resolve, and the position closes on the curve.
#[test]
fn a_none_to_pump_redetection_clears_the_collection_window() {
    use shortwin::events::PoolProposed;

    let (mut env, pm, b, nonce, q) = graduated_elsewhere_with_a_position();
    let saved = env.svm.get_account(&pm.curve.curve).unwrap();
    let fits = plant_at_spot(&mut env, &pm, 1_200 * q);
    migrate_elsewhere(&mut env, &pm);
    let _: PoolProposed = events::one(&ok(env.propose(None, &fits)));
    let m = market(&env, &pm);
    assert!(m.fp_t0 != 0 && m.fp_d.get() > 0, "a window runs");
    // The detection set shows a live curve again (the saved completed curve, un-completed, with a
    // third of its tokens back: a live SOL curve).
    env.svm.set_account(pm.curve.curve, saved).unwrap();
    fork::set_curve_complete(&mut env.svm, &pm.curve.curve, false);
    let s = pm.curve.state(&env.svm);
    fork::set_curve_reserves(
        &mut env.svm,
        &pm.curve.curve,
        s.virtual_token_reserves + 3 * q,
        s.virtual_quote_reserves,
        3 * q,
        s.real_quote_reserves,
    );
    let base_ata = pm.curve.curve_base_ata;
    let held = tokens(&env, &base_ata);
    fork::set_token_amount(&mut env.svm, &base_ata, held + 3 * q);
    env.warp(1);
    let meta = crank(&mut env, &AsNone(&pm));
    venue_changed(&meta, VenueKind::None, VenueKind::PumpCurve);
    let m = market(&env, &pm);
    assert_eq!(m.venue_kind(), Some(VenueKind::PumpCurve));
    assert_eq!(
        (m.fp_t0, m.fp_d.get(), m.fp_keys, m.no_route_since),
        (0, 0, [Pubkey::default(); 6], 0),
        "the collection window and the no-route chain are cleared"
    );
    // The window has nothing left to resolve; the position closes on the curve.
    assert_err(&env.execute_switch_list_only(&fits), E::NoPendingSwitch);
    env.warp(1);
    crank(&mut env, &pm);
    let (c, _) = close_ok(&mut env, &pm, &b, nonce);
    assert_eq!(c.q, q);
}
