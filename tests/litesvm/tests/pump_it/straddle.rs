//! The straddle (ARCH §4.3 `B̂_str`, §5.4; ECON §3.1, §10.2; V1-9): a buyback above the curve's
//! `rT` cannot run on the curve (pump reverts with 6021), so the close refuses it with
//! `CurveStraddle` and the position waits for the curve (or the bundle of §5.4) — and
//! short.win's risk quote `B̂_str(q)` prices exactly the path that bundle executes: a third party
//! completes the curve, `migrate_v2` seeds the canonical pool, and the position buys all of `q`
//! there. Checked here on the real pump and PumpSwap ELFs against the program's own quote (the
//! host build of `venues::read_state` on the same accounts).
//!
//! Every instruction goes through `Env::send`: the invariant hooks run after each one.

use shortwin::constants::PUMP_GLOBAL;
use shortwin_math::fees::{self, FeeBps, PammFeeSchedule};
use shortwin_math::quote::{self as mq, MigrationParams, sol_leg};
use shortwin_venue_state::{pump as vpump, pump_amm as vamm};

use crate::common::*;

/// `q > rT`: the close is refused with `CurveStraddle` (never a pump revert, never a partial
/// buy); a later sale on the curve that brings `rT` back to `q` lets the same close through.
#[test]
fn a_buyback_above_rt_is_refused_with_curve_straddle() {
    let (mut env, pm, q) = setup(PumpCoin::Near);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    env.warp(2);
    let u = buy_down_to(&mut env, &pm, q - 1);
    env.warp(1);
    assert_err(&env.close(&pm, &b, nonce), E::CurveStraddle);
    // The curve is live: a crank observes it, nothing changes venue.
    no_venue_change(&crank(&mut env, &pm));
    // A holder sells back: rT ≥ q again.
    env.warp(1);
    ok(u.sell(&mut env, &pm.curve, 1, 0));
    assert_eq!(pm.curve.state(&env.svm).real_token_reserves, q);
    env.warp(1);
    let (close, meta) = close_ok(&mut env, &pm, &b, nonce);
    assert!(close.buyback_cost > 0);
    // Buying exactly rT completed the curve.
    venue_changed(&meta, VenueKind::PumpCurve, VenueKind::PumpMigrating);
}

/// The PumpSwap `FeeConfig` tiers of the dump, as `shortwin-math` tiers.
fn pool_tiers(env: &Env) -> Vec<fees::FeeTier> {
    let pfc = env
        .svm
        .get_account(&shortwin::constants::PUMP_AMM_FEE_CONFIG)
        .unwrap();
    vpump::read_fee_config(&pfc.owner.to_bytes(), &pfc.data)
        .unwrap()
        .fee_tiers
        .iter()
        .map(|t| fees::FeeTier {
            threshold: t.threshold,
            fees: FeeBps::new(t.fees.lp, t.fees.protocol, t.fees.creator),
        })
        .collect()
}

/// `B̂_str(q)` against the executed bundle (ARCH §5.4; R-PUMP U8): with `rT = q/2`, short.win's
/// live quote (the program's `VenueState::buy` on the pre-bundle accounts) is `B̂_pool(q; X_p,
/// Y_p)` on the deterministic post-migration pool — `X_p = curve_base_ata − rT`, `Y_p = rQ +
/// n(rT) − pool_migration_fee`, at the PumpSwap tier of its market cap. Then, on the real ELFs:
/// a third party completes the curve (`buy_v2(rT)`), anyone calls `migrate_v2` (the pool it
/// creates holds exactly `X_p` and `Y_p − 1`: the quote's intended `+1`), `extend_account`
/// takes the new pool (287 bytes since the 2026-10-02 redeploy, 271 before) to 301 bytes, and
/// PumpSwap `buy(q, B̂_str)` delivers `q` for at most `B̂_str` — the quote covers the execution,
/// within a few lamports.
#[test]
fn b_str_prices_the_executed_bundle() {
    let (mut env, pm, q) = setup(PumpCoin::Near);
    let mut b = env.borrower();
    open_ok(&mut env, &pm, &mut b, q);
    env.warp(2);
    buy_down_to(&mut env, &pm, q / 2);
    env.warp(1);
    crank(&mut env, &pm);

    // ---- the program's quote on the pre-bundle state ----
    let st = host_state(&env, &pm);
    assert!(st.executable, "the curve is live");
    let live = pm.curve.curve_state(&env.svm);
    assert!(live.rt < u128::from(q), "a straddle: q > rT");
    let quote = st.buy(q).expect("B̂_str");
    // The same from the math crate (module docs).
    let g = env.svm.get_account(&PUMP_GLOBAL).unwrap();
    let migration_fee = vpump::read_global(&g.owner.to_bytes(), &g.data)
        .unwrap()
        .pool_migration_fee();
    let base_ata = u128::from(tokens(&env, &pm.curve.curve_base_ata));
    let (xp, yp) = mq::migrated_pool(
        &live,
        &MigrationParams {
            migration_fee,
            pool_tokens: base_ata - live.rt,
        },
    )
    .unwrap();
    let tiers = pool_tiers(&env);
    let gc = env
        .svm
        .get_account(&shortwin::constants::PUMP_AMM_GLOBAL_CONFIG)
        .unwrap();
    let gcfg = vamm::read_global_config(&gc.owner.to_bytes(), &gc.data).unwrap();
    let pool_fees = fees::pump_amm(
        yp,
        xp,
        &PammFeeSchedule {
            tiers: &tiers,
            base_supply: 1_000_000_000_000_000,
            creator_fee_configurable: gcfg.creator_fee_configurable(),
            pool_creator_fee_bps: 0,
            coin_creator_is_default: pm.curve.state(&env.svm).creator_is_default(),
            cap_bps: None,
        },
    )
    .unwrap();
    let expect = sol_leg::buy_exact_out(q, xp, yp, false, &pool_fees).unwrap();
    assert_eq!(quote.cost, expect.cost, "B̂_str = B̂_pool(q; X_p, Y_p)");
    let cap = u64::from(env.config().params.venue_fee_hard_cap_bps);
    let risk = st.buy_risk(q, cap).expect("risk quote");
    assert!(
        risk.cost >= quote.cost,
        "the risk view never quotes below the live one"
    );

    // ---- the bundle on the real ELFs ----
    let keeper = buy_down_to(&mut env, &pm, 0);
    drop(keeper);
    let migrator = env.funded_keypair();
    let mig = pump::migrate_v2_ix(&env.svm, &migrator.pubkey(), &pm.curve);
    ok(env.send(&[pump::cu_limit_ix(600_000), mig], &[&migrator]));
    let np = pump::NewPool::of(&pm.curve);
    let r = np.reserves(&env.svm).expect("the canonical pool");
    assert_eq!(u128::from(r.base), xp, "X_p = the tokens migrate_v2 seeded");
    assert_eq!(
        r.effective_quote + 1,
        yp,
        "Y_p is one lamport above the pool"
    );
    let buyer = env.funded_keypair();
    let payer = env.payer.pubkey();
    let program = pm.curve.token_program.id();
    ok(env.send(
        &[fork::create_ata_idempotent_ix(
            &payer,
            &buyer.pubkey(),
            &pm.keys.mint,
            &program,
        )],
        &[],
    ));
    let buyer_base = pda::ata(&buyer.pubkey(), &pm.keys.mint, &program);
    let wsol = fork::fund_wsol(&mut env, &buyer.pubkey(), quote.cost + 1_000_000_000);
    let wsol0 = tokens(&env, &wsol);
    let ixs = [
        pump::pamm_extend_account_ix(&buyer.pubkey(), &np.pool),
        pump::pamm_buy_ix(
            &env.svm,
            &np,
            &buyer.pubkey(),
            &buyer_base,
            &wsol,
            q,
            quote.cost,
        ),
    ];
    let meta = ok(env.send(&ixs, &[&buyer]));
    assert_eq!(
        env.svm.get_account(&np.pool).unwrap().data.len(),
        301,
        "extended (as every boosted mainnet pool, R-VEN Update 2026-09-29)"
    );
    let spent = wsol0 - tokens(&env, &wsol);
    assert_eq!(tokens(&env, &buyer_base), q, "the pool delivered q");
    eprintln!(
        "B̂_str {} vs executed {} ({} CU)",
        quote.cost, spent, meta.compute_units_consumed
    );
    assert!(spent <= quote.cost, "B̂_str covers the execution");
    assert!(quote.cost - spent <= 4, "within a few lamports of it");
}

/// `B̂_str` on the **EMA leg** (ARCH §4.3, §4.4): a ring entry's cached reprojection `(x, y)` is
/// the curve at that price with the live offsets (`curve_at`), so its `rT(x) = x − (vT − rT)`
/// and a `q` above it is priced on the **deterministic** post-migration pool of the standard
/// curve — `X_p` = 206.9 M tokens (not the live curve's `curve_base_ata − rT`), `Y_p` from that
/// state — at the pool's tier; below it on the curve at the live fees (a donation to the
/// curve's base ATA moves the live leg's `X_p` only). Checked on every
/// entry of the market's ring against `shortwin-math` (`curve_straddle`), with the program's own
/// quoter (host build) on the live accounts.
#[test]
fn b_str_on_the_ema_leg_prices_the_standard_migration() {
    let (mut env, pm, q) = setup(PumpCoin::Near);
    let mut b = env.borrower();
    open_ok(&mut env, &pm, &mut b, q);
    for _ in 0..8 {
        env.warp(1);
        crank(&mut env, &pm);
    }
    buy_down_to(&mut env, &pm, q / 2);
    // A donation to the curve's base ATA: `migrate_v2` would seed it too, so the live leg's
    // `X_p` grows by it — the EMA leg keeps the standard 206.9 M.
    let donation = 1_000_000_000_000;
    donate_tokens(&mut env, &pm, &pm.curve.curve_base_ata, donation);
    // The keeper observes every second until the EMA (the ring's reprojections) has followed
    // the curve above `rT = q/2`: the newest entry's `rT(x)` falls below `q`.
    let offset = {
        let c = pm.curve.state(&env.svm);
        u128::from(c.virtual_token_reserves - c.real_token_reserves)
    };
    for _ in 0..1_800 {
        env.warp(1);
        crank(&mut env, &pm);
        let m = market(&env, &pm);
        let newest = m.ring[usize::from(m.ring_head)];
        if newest.x.get() < offset + u128::from(q) {
            break;
        }
    }
    let st = host_state(&env, &pm);
    let quoter = st.quoter().expect("curve quoter");
    let shortwin::venues::Quoter::Curve(cq) = quoter else {
        panic!("a curve quoter");
    };
    assert_eq!(
        cq.live_pool_tokens,
        shortwin::venues::STANDARD_POOL_TOKENS + u128::from(donation),
        "the live leg's X_p = curve_base_ata − rT"
    );
    let m = market(&env, &pm);
    let mut straddled = 0;
    for e in m.ring.iter().take(usize::from(m.ring_count)) {
        let (x, y) = (e.x.get(), e.y.get());
        let got = quoter.buy_xy(q, x, y).map(|bq| bq.cost);
        let state = shortwin::venues::curve_at(x, y, cq.pricing.offsets);
        let want = mq::curve_straddle(
            q,
            &state,
            &cq.pricing.fees,
            &MigrationParams {
                migration_fee: cq.pricing.migration.migration_fee,
                pool_tokens: shortwin::venues::STANDARD_POOL_TOKENS,
            },
            &cq.pricing.pool,
        )
        .map(|bq| bq.cost);
        assert_eq!(got, want, "entry at {}", e.ts);
        if state.rt < u128::from(q) {
            straddled += 1;
        }
    }
    assert!(straddled > 0, "the newest entry is above rT");
}
