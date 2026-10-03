//! PumpSwap's fee buckets on the 2026-10-02 redeploy (M10 pump revalidation; `fork::PumpDeploy::
//! Oct2026`, the build every fork loads; ARCH U30; R-VEN Update 2026-09-30 H2, which ran the same
//! release's unreleased artifacts from `pump-rust-client` 0.2.0):
//!
//! - **the release is deployed:** a third party's `buy_v2` / `sell_v2` keep the protocol fee
//!   (less the buyback slice) and the creator fee in the quote vault, counted in
//!   `Pool.protocol_fees` @271 / `creator_fees` @279 and taken off `virtual_quote_reserves`, so
//!   the price `vault + VQR` excludes them; the 2026-09 build has no such instruction;
//! - **v1 sales are paid from `vault − buckets`:** a `sell` whose payout lies between the two
//!   fails 6063 on the redeploy and executes on the 2026-09 build;
//! - the permissionless sweeps pay the buckets out without moving the price;
//! - **short.win's trades are unchanged:** its v1 `sell` / `buy` on a pool carrying buckets open
//!   and close to the lamport of `quote::sol_leg` on `vault + VQR` (the hooks run), and leave the
//!   buckets as they were;
//! - **what short.win does not read yet** (`#[ignore]`d, the change spec in the M10 report):
//!   its real SOL `R` is `min(vault, vault + VQR)`, which credits the buckets as payable SOL —
//!   overstated by `min(buckets, boost)` in the sale cap, the tier's `R_now`, its `R_slow` and the
//!   burned-LP bound, which subtract `max(VQR, 0)` = `boost − buckets` instead of the boost.

use shortwin_litesvm_tests::fork::{PumpDeploy, PumpProgram};
use shortwin_litesvm_tests::pamm::{
    PAMM_ERR_INSUFFICIENT_REAL_QUOTE, PammTrade, coin_creator_vault_authority, pamm_sweep_ix,
    plant_buckets, pool_buckets, protocol_fee_recipient,
};

use crate::common::*;
use crate::negvqr::host_state;

fn vqr(env: &Env, p: &PammPool) -> i128 {
    p.state(&env.svm).virtual_quote_reserves
}

/// The largest sale whose `quote::sol_leg` payout (capped by the vault only, as the 2026-09
/// build pays) stays at or below `cap`, by bisection on the base amount.
fn sale_paying_at_most(env: &Env, p: &PammPool, cap: u64) -> (u64, u64) {
    let r = p.reserves(&env.svm).expect("reserves");
    let f = pool_fees(env, p);
    let pay = |q: u64| {
        sol_leg::sell_exact_in(q, u128::from(r.base), r.effective_quote, &f, r.real_quote)
            .map(|s| s.proceeds)
    };
    let (mut lo, mut hi) = (1u64, r.base.saturating_mul(1_000));
    while lo + 1 < hi {
        let mid = lo + (hi - lo) / 2;
        match pay(mid) {
            Some(s) if s <= cap => lo = mid,
            _ => hi = mid,
        }
    }
    (lo, pay(lo).expect("a payout"))
}

/// The fields of a pAMM `BuyEvent` the indexer reads (`services/indexer` `pump_amm_trade`): the
/// pre-trade quote vault, `quote_amount_in_with_lp_fee`, `ix_name` and the trailing
/// `virtual_quote_reserves`.
struct BuyEventView {
    pool_quote: u64,
    quote_net_lp: u64,
    ix_name: String,
    vqr: i128,
}

fn buy_event(meta: &TransactionMetadata) -> BuyEventView {
    let disc = &fork::sha256(b"event:BuyEvent")[..8];
    let raw = events::raw(meta);
    let b = raw
        .iter()
        .find_map(|p| p.strip_prefix(disc))
        .expect("a BuyEvent");
    let u64_at = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
    // timestamp, base_amount_out, max_quote_amount_in, user base / quote reserves, pool base /
    // quote reserves, quote_amount_in, lp_fee_bps, lp_fee, protocol_fee_bps, protocol_fee,
    // quote_amount_in_with_lp_fee (8 B each), user_quote_amount_in, 7 keys, coin_creator_fee_bps,
    // coin_creator_fee, track_volume, 5 × u64, ix_name, 4 × u64, virtual_quote_reserves.
    let pool_quote = u64_at(8 * 6);
    let quote_net_lp = u64_at(8 * 12);
    let mut o = 8 * 14 + 7 * 32 + 8 * 2 + 1 + 8 * 5;
    let n = u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
    let ix_name = String::from_utf8_lossy(&b[o + 4..o + 4 + n]).into_owned();
    o += 4 + n + 8 * 4;
    let vqr = i128::from_le_bytes(b[o..o + 16].try_into().unwrap());
    BuyEventView {
        pool_quote,
        quote_net_lp,
        ix_name,
        vqr,
    }
}

/// A third party's `buy_v2` and `sell_v2` on the mature pool leave the protocol fee (less the
/// buyback slice, paid in the trade) and the creator fee in the quote vault: the buckets grow by
/// exactly what `virtual_quote_reserves` falls, the trader pays and receives the v1 quote, and
/// the 2026-09 build has no `buy_v2` (Anchor's 101 `InstructionFallbackNotFound`).
#[test]
fn v2_trades_keep_their_fees_in_the_pool_buckets() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Mature);
    let u = PammUser::new(&mut env, &p, 1_000 * SOL);
    assert_eq!(pool_buckets(&env.svm, &p.pool), (0, 0), "none at the dump");
    let buyback = wsol_ata(&pamm::buyback_fee_recipient(&env.svm, 0));
    let q = p.reserves(&env.svm).unwrap().base / 1_000;

    // buy_v2: the cost is the v1 quote; the vault keeps all of it but the buyback slice.
    let (vqr0, vault0, bb0, w0) = (
        vqr(&env, &p),
        tokens(&env, &p.quote_vault),
        tokens(&env, &buyback),
        tokens(&env, &u.wsol),
    );
    let cost = quote_buy(&env, &p, q).expect("quote");
    let meta = ok(u.trade_v2(&mut env, &p, PammTrade::Buy, q, cost));
    let (pf, cf) = pool_buckets(&env.svm, &p.pool);
    assert!(pf > 0 && cf > 0, "both buckets filled: {pf} / {cf}");
    assert_eq!(w0 - tokens(&env, &u.wsol), cost, "v2 costs the v1 quote");
    let bb = tokens(&env, &buyback) - bb0;
    assert!(bb > 0, "the buyback slice is paid in the trade");
    assert_eq!(tokens(&env, &p.quote_vault) - vault0, cost - bb);
    assert_eq!(
        vqr(&env, &p),
        vqr0 - i128::from(pf + cf),
        "VQR = boost − buckets"
    );

    // The v2 `BuyEvent` keeps the v1 layout and carries the **pre-trade** VQR; the indexer's
    // reconstruction (vault before + `quote_amount_in_with_lp_fee` + the event's VQR) is then the
    // post-trade effective reserve exactly: the buckets the trade left in the vault are the VQR
    // it took off.
    let ev = buy_event(&meta);
    let r1 = p.reserves(&env.svm).unwrap();
    assert_eq!(ev.pool_quote, vault0);
    assert_eq!(ev.vqr, vqr0, "the pre-trade VQR");
    let indexer = i128::from(ev.pool_quote + ev.quote_net_lp) + ev.vqr;
    eprintln!(
        "BuyEvent ix_name {:?}: indexer's post-trade quote {indexer}, effective {}",
        ev.ix_name, r1.effective_quote
    );
    assert_eq!(
        indexer,
        i128::try_from(r1.effective_quote).unwrap(),
        "the indexer's reconstruction holds for v2"
    );

    // sell_v2: the buckets grow again by what VQR falls; the payout is the v1 quote.
    let (vqr1, w1) = (vqr(&env, &p), tokens(&env, &u.wsol));
    let s = quote_sell(&env, &p, q / 2).expect("quote");
    ok(u.trade_v2(&mut env, &p, PammTrade::Sell, q / 2, s));
    let (pf2, cf2) = pool_buckets(&env.svm, &p.pool);
    assert!(pf2 > pf && cf2 > cf);
    assert_eq!(tokens(&env, &u.wsol) - w1, s, "v2 pays the v1 quote");
    assert_eq!(vqr(&env, &p), vqr1 - i128::from(pf2 - pf + cf2 - cf));
    eprintln!(
        "buy_v2 {q}: cost {cost}, buyback {bb}, buckets {pf} + {cf}; sell_v2 {}: {s}, buckets \
         {pf2} + {cf2}; VQR {vqr0} -> {}",
        q / 2,
        vqr(&env, &p)
    );

    fork::load_pump_deploy(&mut env.svm, PumpProgram::Amm, PumpDeploy::Sep2026);
    env.warp(1);
    let res = u.trade_v2(&mut env, &p, PammTrade::Buy, q, cost * 2);
    assert_eq!(
        custom_code(&res),
        Some(101),
        "no buy_v2 in the 2026-09 build"
    );
}

/// On a pool carrying buckets (planted as v2 trades leave them: half the vault), a v1 `sell` whose
/// payout lies between `vault − buckets` and the vault fails 6063 `InsufficientRealQuoteReserves`
/// on the redeploy and executes on the 2026-09 build, which ignores bytes 271–287.
#[test]
fn a_v1_sale_is_paid_from_the_vault_less_the_buckets() {
    for deploy in [PumpDeploy::Sep2026, PumpDeploy::Oct2026] {
        let (mut env, p) = Env::fork_pamm_coin(PammCoin::Mature);
        fork::load_pump_deploy(&mut env.svm, PumpProgram::Amm, deploy);
        let vault = tokens(&env, &p.quote_vault);
        let bucket = vault / 2;
        plant_buckets(&mut env.svm, &p, bucket, 0);
        let (q, payout) = sale_paying_at_most(&env, &p, vault - bucket + 10 * SOL);
        assert!(payout > vault - bucket && payout <= vault, "{payout}");
        let u = PammUser::new(&mut env, &p, SOL);
        let res = u.sell(&mut env, &p, q, payout);
        match deploy {
            PumpDeploy::Sep2026 => {
                ok(res);
            }
            PumpDeploy::Oct2026 => {
                assert_eq!(custom_code(&res), Some(PAMM_ERR_INSUFFICIENT_REAL_QUOTE));
            }
        }
    }
}

/// `sweep_protocol_fee` (to a listed protocol fee recipient) and `sweep_creator_fee` (to the
/// coin creator's vault authority), paid by anyone, take the buckets out of the vault and add them
/// back to `virtual_quote_reserves`: the price `vault + VQR` does not move.
#[test]
fn the_sweeps_pay_the_buckets_out_without_moving_the_price() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Mature);
    let u = PammUser::new(&mut env, &p, 1_000 * SOL);
    let q = p.reserves(&env.svm).unwrap().base / 1_000;
    ok(u.trade_v2(&mut env, &p, PammTrade::Buy, q, 1_000 * SOL));
    let (pf, cf) = pool_buckets(&env.svm, &p.pool);
    let payer = env.funded_keypair();
    let creator =
        coin_creator_vault_authority(&Pubkey::new_from_array(p.state(&env.svm).coin_creator));
    for (protocol, recipient, amount) in [
        (true, protocol_fee_recipient(&env.svm, 0), pf),
        (false, creator, cf),
    ] {
        let r0 = p.reserves(&env.svm).unwrap();
        let to0 = read::token_amount(&env.svm, &wsol_ata(&recipient)).unwrap_or(0);
        let ix = pamm_sweep_ix(&p, protocol, &payer.pubkey(), &recipient);
        ok(env.send(&[ix], &[&payer]));
        let r1 = p.reserves(&env.svm).unwrap();
        assert_eq!(r1.real_quote, r0.real_quote - amount, "out of the vault");
        assert_eq!(r1.effective_quote, r0.effective_quote, "the price holds");
        assert_eq!(
            read::token_amount(&env.svm, &wsol_ata(&recipient)).unwrap() - to0,
            amount
        );
    }
    assert_eq!(pool_buckets(&env.svm, &p.pool), (0, 0));
}

/// short.win's own v1 trades on a pool that carries real buckets (a third party's `buy_v2` and
/// `sell_v2` after the listing): the open's sale and the close's buyback equal `quote::sol_leg` on
/// `vault + VQR` to the lamport (the hooks re-quote both), and the v1 trades pay their fees out,
/// leaving the buckets as they were.
#[test]
fn short_win_trades_a_pool_carrying_buckets() {
    let (mut env, pm, q) = setup(PammCoin::Mature);
    let p = pm.pool;
    let u = PammUser::new(&mut env, &p, 1_000 * SOL);
    let qb = p.reserves(&env.svm).unwrap().base / 1_000;
    ok(u.trade_v2(&mut env, &p, PammTrade::Buy, qb, 1_000 * SOL));
    ok(u.trade_v2(&mut env, &p, PammTrade::Sell, qb / 2, 0));
    let buckets = pool_buckets(&env.svm, &p.pool);
    assert!(buckets.0 > 0 && buckets.1 > 0);
    env.warp(1);
    crank(&mut env, &pm);

    let s_exp = quote_sell(&env, &p, q).expect("quote");
    let mut b = env.borrower();
    let (open, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    assert_eq!(open.s, s_exp);
    assert_eq!(
        pool_buckets(&env.svm, &p.pool),
        buckets,
        "v1 pays its fees out"
    );
    env.warp(1);
    crank(&mut env, &pm);
    let b_exp = quote_buy(&env, &p, q).expect("quote");
    let (close, _) = close_ok(&mut env, &pm, &b, nonce);
    assert_eq!(close.b_live, b_exp);
    assert_eq!(pool_buckets(&env.svm, &p.pool), buckets);
    assert_trader_clean(&env, &pm);
}

/// The change the redeploy needs (ARCH U30's "read `R = vault − buckets` before
/// `revalidate_venue`"): with buckets `B ≤ boost` on the pool, the program's real SOL must be
/// the payable `min(vault − B, vault + VQR)`, and the tier's virtual part the boost
/// `max(VQR + B, 0)` (the program subtracts `max(VQR, 0)` = `boost − B`). Today `R = vault`,
/// overstated by `B` in the sale cap and in every term of `R_eff`.
#[test]
#[ignore = "pump-2026-10: short.win credits PumpSwap's fee buckets (@271/@279) as real SOL; adapter change PUMP-2026-10-1 in the M10 pump revalidation report"]
fn the_program_reads_the_payable_sol_under_buckets() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Mature);
    let boost = u64::try_from(vqr(&env, &p)).expect("a boosted pool");
    plant_buckets(&mut env.svm, &p, boost / 4, boost / 4);
    let (pf, cf) = pool_buckets(&env.svm, &p.pool);
    let q = q_for(&env, &p);
    let pm = list_on(&mut env, &p, q * 40);
    let config = env.config();
    let m = market(&env, &pm);
    let st = host_state(&env, &pm);
    let r = p.reserves(&env.svm).unwrap();
    let effective = u64::try_from(r.effective_quote).unwrap();
    let payable = r.real_quote - pf - cf;
    assert_eq!(
        st.r,
        payable.min(effective),
        "R_now = min(vault − buckets, vault + VQR)"
    );

    let r_eff = shortwin::engine::tier::r_eff(&config, &m, &st, env.now()).expect("R_eff");
    let y_slow = shortwin_math::quote::reprice(m.liq_slow.get(), m.ema_price.get())
        .map(|(_, y)| y)
        .unwrap();
    let virt = u128::try_from((vqr(&env, &p) + i128::from(pf + cf)).max(0)).unwrap();
    assert_eq!(virt, u128::from(boost), "the virtual part is the boost");
    let (_, d) = pm.pool.locked_depth(&env.svm);
    let locked = (d * m.ema_price.get().isqrt() / 1_000_000_000).saturating_sub(virt);
    let want = u128::from(payable.min(effective))
        .min(y_slow.saturating_sub(virt))
        .min(locked);
    assert_eq!(r_eff, want);
}
