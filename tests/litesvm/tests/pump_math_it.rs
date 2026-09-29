//! T-PUMP-MATH, execution half (M5; ARCH §4.2, §4.3 curve rows, §8.2; R-PUMP §3.5, §4, §7.2–
//! §7.3; Update 2026-09-29 U4, U6, U7): a **normal user's** `buy_v2`, `sell_v2` and
//! `buy_exact_quote_in_v2` on the **real** pump program dumped from mainnet (deploy slot
//! 449,734,335, with the real pump-fees program it CPIs for the fee tier;
//! `tests/fixtures/`, loaded by `shortwin_litesvm_tests::fork`) charge exactly what
//! `shortwin-math` computes on the `shortwin-venue-state` readers' view of the curve, the live
//! `FeeConfig` and `Global`:
//!
//! - **buys** (`buy_v2`, exact tokens out): `quote::sol_leg::curve_charge_exact_out` —
//!   `n = ⌊q·vQ/(vT − q)⌋ + 1`, each fee `⌈n·bps/10⁴⌉` — with `fees::pump_curve`'s tier; and
//!   short.win's own conservative quote (`sol_leg::buy_exact_out`, ε₁ = 1) is never below the
//!   charge and at most `1 + #fees` above it (ECON §3.1, §3.2);
//! - **sells** (`sell_v2`, exact tokens in): `quote::sol_leg::sell_exact_in` with the real-reserve
//!   cap `rQ`;
//! - **exact quote in** (`buy_exact_quote_in_v2`): R-PUMP §4's rule with the fees on the
//!   pre-adjustment net (Update 2026-09-29 U4; `fork::pump_exact_quote_in`, a test-side
//!   reference because short.win never trades exact-in), and its tokens bracketed by
//!   `curve_charge_exact_out`;
//! - over amount grids from 1 raw unit to the whole curve, and seeded random sequences, on every
//!   live SOL curve of the dump: 125 bytes (`create_v2` today), 151 (extended), 115 (Token-2022
//!   and legacy SPL-Token) and the 2024 49-byte layout (the first v2 trade grows a curve under
//!   125 bytes to 125, the user topping up its rent reserve); at the live fee tier and under
//!   rewritten `FeeConfig`s (PumpSwap's 25 market-cap tiers, thresholds at a curve's own market
//!   cap, the per-coin creator override, a default creator). One deviation is pinned as a
//!   tripwire: the deployed pump resolves the tier at the mint's **live** supply, `fees::
//!   pump_curve` at a fixed 10¹⁵ (no quote differs under today's single tier);
//! - the exact bound passes and one unit tighter fails (6002 / 6003 / 6042); every lamport and
//!   token delta (user, curve, fee and buyback recipients, creator vault) and the curve's
//!   post-trade reserves (reader) equal the quote, and so does the venue's own `TradeEvent`;
//! - a buy beyond `rT` reverts with 6021 on every buy instruction (the undocumented trailing
//!   cap byte of U7 fills exactly `rT` instead), the completing buy of exactly `rT` completes the
//!   curve, and every trade on a complete curve — the one just completed and the three migrated
//!   curves of the dump — reverts with 6005;
//! - venue-side rent (T-RENT's curve part): a creator vault that does not exist is topped up to
//!   `Rent.minimum_balance(0)` from the user on buys and sells alike, with mainnet's Rent.
//!
//! Every transaction goes through `Env::send` (the invariant hooks run after every
//! instruction). `PUMP_MATH_SEQ` sets the random sequence length per curve (default 30) and
//! `PUMP_MATH_SEED` its seed.

// `TxResult` is LiteSVM's own result type (see the harness's lib.rs).
#![allow(clippy::result_large_err)]

use anchor_lang::prelude::Pubkey;
use litesvm::LiteSVM;
use shortwin::constants::{
    PUMP_AMM_FEE_CONFIG, PUMP_AMM_GLOBAL_CONFIG, PUMP_AMM_PROGRAM_ID, PUMP_FEE_CONFIG,
    PUMP_FEES_PROGRAM_ID, PUMP_GLOBAL, PUMP_PROGRAM_ID, VENUE_GUARD_SLOTS_2026_09_27,
    VENUE_PROGRAM_DATA,
};
use shortwin_litesvm_tests::errors::custom_code;
use shortwin_litesvm_tests::fork::{
    self, PumpCoin, PumpCurve, PumpPool, PumpProgram, PumpTradeEvent, PumpV2,
};
use shortwin_litesvm_tests::{Env, TxResult, pda, read};
use shortwin_math::fees::{self as mfees, FeeBps, FeeTier as MathFeeTier};
use shortwin_math::quote::sol_leg;
use shortwin_venue_state::{program_data, pump, pump_amm};
use solana_keypair::Keypair;
use solana_signer::Signer;

// ---- users and trades ----------------------------------------------------------------------------

/// A normal user: a funded keypair with its base-token ATA and its pump UVA, both created before
/// any trade (the ATA program and pump's own `init_user_volume_accumulator`, through `Env::send`),
/// so a trade's deltas carry no account creation.
struct User {
    kp: Keypair,
    ata: Pubkey,
}

fn user(env: &mut Env, c: &PumpCurve) -> User {
    let kp = env.funded_keypair();
    let payer = env.payer.pubkey();
    let program = c.token_program.id();
    let ixs = [
        fork::create_ata_idempotent_ix(&payer, &kp.pubkey(), &c.mint, &program),
        fork::pump_init_uva_ix(&payer, &kp.pubkey()),
    ];
    ok(env.send(&ixs, &[]), "user setup");
    let uva = fork::pump_user_volume_accumulator(&kp.pubkey());
    let acc = env.svm.get_account(&uva).expect("UVA");
    let u = pump::read_user_volume_accumulator(&acc.owner.to_bytes(), &acc.data).expect("UVA");
    assert_eq!(u.user, kp.pubkey().to_bytes());
    assert_eq!(acc.data.len(), pump::USER_VOLUME_ACCUMULATOR_LEN);
    User {
        ata: pda::ata(&kp.pubkey(), &c.mint, &program),
        kp,
    }
}

#[track_caller]
fn ok(res: TxResult, what: &str) -> litesvm::types::TransactionMetadata {
    match res {
        Ok(m) => m,
        Err(f) => panic!("{what}: failed with {:?}\n{}", f.err, f.meta.pretty_logs()),
    }
}

fn describe(res: &TxResult) -> String {
    match res {
        Ok(m) => format!("succeeded\n{}", m.pretty_logs()),
        Err(f) => format!("failed with {:?}\n{}", f.err, f.meta.pretty_logs()),
    }
}

/// Fee recipient `k % 8` and buyback recipient `(k / 8) % 8` of the live `Global` (every listed
/// recipient is accepted; spread like the keeper spreads them).
fn recipients(svm: &LiteSVM, k: u64) -> (Pubkey, Pubkey) {
    let i = usize::try_from(k % 8).unwrap();
    let j = usize::try_from((k / 8) % 8).unwrap();
    (
        fork::pump_fee_recipient(svm, i),
        fork::pump_buyback_fee_recipient(svm, j),
    )
}

/// One v2 trade by `u`, with recipients chosen by `k`.
fn trade(env: &mut Env, c: &PumpCurve, u: &User, kind: PumpV2, a: u64, b: u64, k: u64) -> TxResult {
    let (fr, bb) = recipients(&env.svm, k);
    let ix = c.v2_ix(&env.svm, kind, &u.kp.pubkey(), &u.ata, &fr, &bb, a, b);
    let signer = u.kp.insecure_clone();
    env.send(&[ix], &[&signer])
}

/// Everything a trade moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Balances {
    user: u64,
    user_tokens: u64,
    curve: u64,
    curve_tokens: u64,
    fee_recipient: u64,
    buyback: u64,
    creator_vault: u64,
}

fn balances(svm: &LiteSVM, c: &PumpCurve, u: &User, k: u64) -> Balances {
    let (fr, bb) = recipients(svm, k);
    Balances {
        user: read::lamports(svm, &u.kp.pubkey()),
        user_tokens: read::token_amount(svm, &u.ata).unwrap_or(0),
        curve: read::lamports(svm, &c.curve),
        curve_tokens: read::token_amount(svm, &c.curve_base_ata).unwrap_or(0),
        fee_recipient: read::lamports(svm, &fr),
        buyback: read::lamports(svm, &bb),
        creator_vault: read::lamports(svm, &c.creator_vault(svm)),
    }
}

fn fee_up(x: u64, bps: u64) -> u64 {
    (u128::from(x) * u128::from(bps))
        .div_ceil(10_000)
        .try_into()
        .unwrap()
}

/// The buyback share of a protocol fee: `⌊protocol·buyback_bps/10⁴⌋` (R-PUMP §4 "Where the fees
/// go"; the fee recipient gets the rest).
fn buyback_share(svm: &LiteSVM, protocol: u64) -> u64 {
    (u128::from(protocol) * u128::from(fork::pump_buyback_bps(svm)) / 10_000)
        .try_into()
        .unwrap()
}

/// The venue-side rent pump takes from the user for the creator vault (Update 2026-09-29 U6): a
/// vault holding less than `Rent.minimum_balance(0)` is topped up to it, on top of the fee.
fn creator_top_up(svm: &LiteSVM, vault_before: u64, creator_fee: u64) -> u64 {
    let min0 = svm.minimum_balance_for_rent_exemption(0);
    if creator_fee > 0 && vault_before < min0 {
        min0 - vault_before
    } else {
        0
    }
}

/// The single `TradeEvent` of a transaction.
fn one_trade(meta: &litesvm::types::TransactionMetadata) -> PumpTradeEvent {
    let mut ev = fork::pump_trade_events(meta);
    assert_eq!(ev.len(), 1, "one TradeEvent\n{}", meta.pretty_logs());
    ev.remove(0)
}

/// The curve's growth on a v2 trade, as the deployed ELF does it: a curve shorter than the
/// current layout grows to [`pump::CURVE_LEN_V2`] (125 bytes — not the docs' 115), and the user
/// tops its rent reserve (`lamports − rQ`) up to `Rent.minimum_balance(125)`; a longer curve is
/// left alone. Returns the expected top-up.
#[track_caller]
fn curve_growth(
    svm: &LiteSVM,
    ctx: &str,
    len_before: usize,
    len_after: usize,
    lamports_before: u64,
    rq_before: u64,
) -> u64 {
    assert_eq!(
        len_after,
        len_before.max(pump::CURVE_LEN_V2),
        "{ctx}: a v2 trade leaves the curve at max(len, 125)"
    );
    if len_before >= pump::CURVE_LEN_V2 {
        return 0;
    }
    svm.minimum_balance_for_rent_exemption(pump::CURVE_LEN_V2)
        .saturating_sub(lamports_before - rq_before)
}

/// The reader's curve state after a trade equals the quote's and the event's post-state.
#[track_caller]
fn check_post_state(
    ctx: &str,
    after: &pump::BondingCurve,
    before: &pump::BondingCurve,
    ev: &PumpTradeEvent,
    (vt, vq, rt, rq): (u64, u64, u64, u64),
) {
    assert_eq!(
        (
            after.virtual_token_reserves,
            after.virtual_quote_reserves,
            after.real_token_reserves,
            after.real_quote_reserves
        ),
        (vt, vq, rt, rq),
        "{ctx}: the curve after the trade"
    );
    assert_eq!(
        (
            ev.virtual_token_reserves,
            ev.virtual_sol_reserves,
            ev.real_token_reserves,
            ev.real_sol_reserves
        ),
        (vt, vq, rt, rq),
        "{ctx}: the TradeEvent's post-state"
    );
    assert_eq!(after.complete, rt == 0, "{ctx}: complete iff rT = 0");
    assert_eq!(
        (after.creator, after.quote_mint, after.token_total_supply),
        (before.creator, before.quote_mint, before.token_total_supply),
        "{ctx}: nothing else changes"
    );
}

/// The expected failure of a trade the quote says is undefined.
fn expected_failure(before: &pump::BondingCurve, kind: PumpV2, amount: u64) -> Option<u32> {
    if before.complete {
        return Some(fork::PUMP_ERR_BONDING_CURVE_COMPLETE);
    }
    match kind {
        PumpV2::Buy if amount == 0 => Some(fork::PUMP_ERR_BUY_ZERO_AMOUNT),
        PumpV2::Buy if amount > before.real_token_reserves => {
            Some(fork::PUMP_ERR_NOT_ENOUGH_TOKENS_TO_BUY)
        }
        _ => None,
    }
}

/// An exact-out buy of `q` checked end to end (module docs). Returns whether it executed.
#[track_caller]
fn exact_buy(env: &mut Env, c: &PumpCurve, u: &User, q: u64, k: u64) -> bool {
    let ctx = format!("{:?} buy_v2 q {q}", c.coin);
    let before = c.state(&env.svm);
    let Some(ch) = c.charge_buy(&env.svm, q) else {
        let saved = env.svm.clone();
        let res = trade(env, c, u, PumpV2::Buy, q, u64::MAX, k);
        assert!(
            res.is_err(),
            "{ctx}: no charge, but the venue {}",
            describe(&res)
        );
        if let Some(want) = expected_failure(&before, PumpV2::Buy, q) {
            assert_eq!(custom_code(&res), Some(want), "{ctx}: {}", describe(&res));
        }
        env.svm = saved;
        return false;
    };
    let fees = c.fees(&env.svm).expect("fees");
    // short.win's own quote (ECON §3.1) covers the charge, by at most 1 + #fees lamports.
    let quote = c.quote_buy(&env.svm, q).expect("quote");
    let n_fees = [fees.protocol, fees.creator]
        .iter()
        .filter(|&&b| b > 0)
        .count() as u64;
    assert!(
        quote.cost >= ch.cost && quote.cost <= ch.cost + 1 + n_fees,
        "{ctx}: quote {} vs charge {}",
        quote.cost,
        ch.cost
    );
    let protocol = fee_up(ch.net, fees.protocol);
    let creator = fee_up(ch.net, fees.creator);
    assert_eq!(ch.cost, ch.net + protocol + creator);

    // One lamport under the charge: TooMuchSolRequired, nothing moves.
    let res = trade(env, c, u, PumpV2::Buy, q, ch.cost - 1, k);
    assert_eq!(
        custom_code(&res),
        Some(fork::PUMP_ERR_TOO_MUCH_SOL_REQUIRED),
        "{ctx}: one lamport under the charge; {}",
        describe(&res)
    );

    let len_before = c.data_len(&env.svm);
    let b0 = balances(&env.svm, c, u, k);
    let top_up = creator_top_up(&env.svm, b0.creator_vault, creator);
    let meta = ok(trade(env, c, u, PumpV2::Buy, q, ch.cost, k), &ctx);
    let b1 = balances(&env.svm, c, u, k);
    let after = c.state(&env.svm);
    let ev = one_trade(&meta);
    let len_after = c.data_len(&env.svm);
    let curve_top_up = curve_growth(
        &env.svm,
        &ctx,
        len_before,
        len_after,
        b0.curve,
        before.real_quote_reserves,
    );
    assert_eq!(
        b1.curve - b0.curve,
        ch.net + curve_top_up,
        "{ctx}: curve lamports"
    );
    let buyback = buyback_share(&env.svm, protocol);
    assert_eq!(
        b0.user - b1.user,
        ch.cost + top_up + curve_top_up,
        "{ctx}: user paid"
    );
    assert_eq!(b1.user_tokens - b0.user_tokens, q, "{ctx}: user received");
    assert_eq!(b0.curve_tokens - b1.curve_tokens, q, "{ctx}: curve tokens");
    assert_eq!(
        b1.fee_recipient - b0.fee_recipient,
        protocol - buyback,
        "{ctx}: fee recipient"
    );
    assert_eq!(b1.buyback - b0.buyback, buyback, "{ctx}: buyback recipient");
    assert_eq!(
        b1.creator_vault - b0.creator_vault,
        creator + top_up,
        "{ctx}: creator vault"
    );
    assert_eq!(
        (
            ev.sol_amount,
            ev.token_amount,
            ev.fee,
            ev.creator_fee,
            ev.buyback_fee
        ),
        (ch.net, q, protocol, creator, buyback),
        "{ctx}: TradeEvent"
    );
    assert!(ev.is_buy && ev.ix_name == "buy" && ev.mint == c.mint && ev.user == u.kp.pubkey());
    assert_eq!(
        ev.fee_basis_points, fees.protocol,
        "{ctx}: event protocol bps"
    );
    check_post_state(
        &ctx,
        &after,
        &before,
        &ev,
        (
            u64::try_from(ch.token_after).unwrap(),
            u64::try_from(ch.sol_after).unwrap(),
            before.real_token_reserves - q,
            before.real_quote_reserves + ch.net,
        ),
    );
    true
}

/// An exact-in sale of `q` checked end to end. The user's balance is planted if short (these
/// mints have no mint authority; pump reads only the account). Returns whether it executed.
#[track_caller]
fn exact_sell(env: &mut Env, c: &PumpCurve, u: &User, q: u64, k: u64) -> bool {
    let ctx = format!("{:?} sell_v2 q {q}", c.coin);
    let have = read::token_amount(&env.svm, &u.ata).unwrap_or(0);
    if have < q {
        fork::set_token_amount(&mut env.svm, &u.ata, q);
    }
    let before = c.state(&env.svm);
    let Some(sq) = c.quote_sell(&env.svm, q) else {
        let saved = env.svm.clone();
        let res = trade(env, c, u, PumpV2::Sell, q, 0, k);
        assert!(
            res.is_err(),
            "{ctx}: no quote, but the venue {}",
            describe(&res)
        );
        if let Some(want) = expected_failure(&before, PumpV2::Sell, q) {
            assert_eq!(custom_code(&res), Some(want), "{ctx}: {}", describe(&res));
        }
        env.svm = saved;
        return false;
    };
    let fees = c.fees(&env.svm).expect("fees");
    let g = before.virtual_quote_reserves - u64::try_from(sq.sol_after).unwrap();
    let protocol = fee_up(g, fees.protocol);
    let creator = fee_up(g, fees.creator);
    assert_eq!(sq.proceeds, g - protocol - creator);

    let res = trade(env, c, u, PumpV2::Sell, q, sq.proceeds + 1, k);
    assert_eq!(
        custom_code(&res),
        Some(fork::PUMP_ERR_TOO_LITTLE_SOL_RECEIVED),
        "{ctx}: one lamport over the proceeds; {}",
        describe(&res)
    );

    let len_before = c.data_len(&env.svm);
    let b0 = balances(&env.svm, c, u, k);
    let top_up = creator_top_up(&env.svm, b0.creator_vault, creator);
    let meta = ok(trade(env, c, u, PumpV2::Sell, q, sq.proceeds, k), &ctx);
    let b1 = balances(&env.svm, c, u, k);
    let after = c.state(&env.svm);
    let ev = one_trade(&meta);
    let curve_top_up = curve_growth(
        &env.svm,
        &ctx,
        len_before,
        c.data_len(&env.svm),
        b0.curve,
        before.real_quote_reserves,
    );
    assert_eq!(
        b1.curve + g,
        b0.curve + curve_top_up,
        "{ctx}: curve lamports"
    );
    let buyback = buyback_share(&env.svm, protocol);
    assert_eq!(
        i128::from(b1.user) - i128::from(b0.user),
        i128::from(sq.proceeds) - i128::from(top_up) - i128::from(curve_top_up),
        "{ctx}: user received"
    );
    assert_eq!(b0.user_tokens - b1.user_tokens, q, "{ctx}: user sold");
    assert_eq!(b1.curve_tokens - b0.curve_tokens, q, "{ctx}: curve tokens");
    assert_eq!(
        b1.fee_recipient - b0.fee_recipient,
        protocol - buyback,
        "{ctx}: fee recipient"
    );
    assert_eq!(b1.buyback - b0.buyback, buyback, "{ctx}: buyback recipient");
    assert_eq!(
        b1.creator_vault - b0.creator_vault,
        creator + top_up,
        "{ctx}: creator vault"
    );
    assert_eq!(
        (
            ev.sol_amount,
            ev.token_amount,
            ev.fee,
            ev.creator_fee,
            ev.buyback_fee
        ),
        (g, q, protocol, creator, buyback),
        "{ctx}: TradeEvent"
    );
    assert!(!ev.is_buy && ev.ix_name == "sell");
    check_post_state(
        &ctx,
        &after,
        &before,
        &ev,
        (
            u64::try_from(sq.token_after).unwrap(),
            u64::try_from(sq.sol_after).unwrap(),
            before.real_token_reserves + q,
            before.real_quote_reserves - g,
        ),
    );
    true
}

/// A `buy_exact_quote_in_v2(spendable, min_tokens_out)` checked end to end. Returns whether it
/// executed.
#[track_caller]
fn exact_quote_in(env: &mut Env, c: &PumpCurve, u: &User, spendable: u64, k: u64) -> bool {
    let ctx = format!("{:?} buy_exact_quote_in_v2 spendable {spendable}", c.coin);
    let before = c.state(&env.svm);
    let Some(r) = c.quote_exact_quote_in(&env.svm, spendable) else {
        let saved = env.svm.clone();
        let res = trade(env, c, u, PumpV2::BuyExactQuoteIn, spendable, 0, k);
        assert!(
            res.is_err(),
            "{ctx}: no quote, but the venue {}",
            describe(&res)
        );
        if before.complete {
            assert_eq!(
                custom_code(&res),
                Some(fork::PUMP_ERR_BONDING_CURVE_COMPLETE),
                "{ctx}"
            );
        }
        env.svm = saved;
        return false;
    };
    assert!(r.cost() <= spendable, "{ctx}: never more than spendable");
    // shortwin-math's exact-out charge brackets the tokens: n(t) ≤ net ≤ n(t + 1).
    let fees = c.fees(&env.svm).expect("fees");
    let (vt, vq) = (
        u128::from(before.virtual_token_reserves),
        u128::from(before.virtual_quote_reserves),
    );
    if r.tokens > 0 {
        let lo = sol_leg::curve_charge_exact_out(r.tokens, vt, vq, &fees).expect("n(t)");
        assert!(lo.net <= r.net, "{ctx}: n(t) {} > net {}", lo.net, r.net);
    }
    if let Some(hi) = sol_leg::curve_charge_exact_out(r.tokens + 1, vt, vq, &fees) {
        assert!(
            hi.net >= r.net,
            "{ctx}: n(t + 1) {} < net {}",
            hi.net,
            r.net
        );
    }

    let res = trade(
        env,
        c,
        u,
        PumpV2::BuyExactQuoteIn,
        spendable,
        r.tokens + 1,
        k,
    );
    assert_eq!(
        custom_code(&res),
        Some(fork::PUMP_ERR_BUY_SLIPPAGE_BELOW_MIN_TOKENS_OUT),
        "{ctx}: one token over the output; {}",
        describe(&res)
    );

    let len_before = c.data_len(&env.svm);
    let b0 = balances(&env.svm, c, u, k);
    let top_up = creator_top_up(&env.svm, b0.creator_vault, r.creator_fee);
    let meta = ok(
        trade(env, c, u, PumpV2::BuyExactQuoteIn, spendable, r.tokens, k),
        &ctx,
    );
    let b1 = balances(&env.svm, c, u, k);
    let after = c.state(&env.svm);
    let ev = one_trade(&meta);
    let curve_top_up = curve_growth(
        &env.svm,
        &ctx,
        len_before,
        c.data_len(&env.svm),
        b0.curve,
        before.real_quote_reserves,
    );
    assert_eq!(
        b1.curve - b0.curve,
        r.net + curve_top_up,
        "{ctx}: curve lamports"
    );
    let buyback = buyback_share(&env.svm, r.protocol_fee);
    assert_eq!(
        b0.user - b1.user,
        r.cost() + top_up + curve_top_up,
        "{ctx}: user paid"
    );
    assert_eq!(
        b1.user_tokens - b0.user_tokens,
        r.tokens,
        "{ctx}: user received"
    );
    assert_eq!(
        b1.fee_recipient - b0.fee_recipient,
        r.protocol_fee - buyback,
        "{ctx}: fee recipient"
    );
    assert_eq!(b1.buyback - b0.buyback, buyback, "{ctx}: buyback recipient");
    assert_eq!(
        b1.creator_vault - b0.creator_vault,
        r.creator_fee + top_up,
        "{ctx}: creator vault"
    );
    assert_eq!(
        (
            ev.sol_amount,
            ev.token_amount,
            ev.fee,
            ev.creator_fee,
            ev.buyback_fee
        ),
        (r.net, r.tokens, r.protocol_fee, r.creator_fee, buyback),
        "{ctx}: TradeEvent"
    );
    assert!(ev.is_buy && ev.ix_name == "buy_exact_quote_in");
    check_post_state(
        &ctx,
        &after,
        &before,
        &ev,
        (
            before.virtual_token_reserves - r.tokens,
            before.virtual_quote_reserves + r.net,
            before.real_token_reserves - r.tokens,
            before.real_quote_reserves + r.net,
        ),
    );
    true
}

// ---- grids ----------------------------------------------------------------------------------------

const BUYS: [u64; 14] = [
    1,
    2,
    10,
    1_000,
    1_000_000,
    1_000_000_000,
    10_000_000_000,
    100_000_000_000,
    1_000_000_000_000,
    5_000_000_000_000,
    10_000_000_000_000,
    30_000_000_000_000,
    100_000_000_000_000,
    300_000_000_000_000,
];

const SELLS: [u64; 12] = [
    1,
    10,
    1_000,
    1_000_000,
    1_000_000_000,
    100_000_000_000,
    1_000_000_000_000,
    5_000_000_000_000,
    10_000_000_000_000,
    30_000_000_000_000,
    100_000_000_000_000,
    300_000_000_000_000,
];

const SPENDS: [u64; 12] = [
    1,
    2,
    3,
    10,
    100,
    10_000,
    1_000_000,
    100_000_000,
    1_000_000_000,
    5_000_000_000,
    20_000_000_000,
    60_000_000_000,
];

/// Buys, then sells, then exact-quote-in buys over the grids on `coin`'s live curve; returns
/// how many executed.
fn grid(coin: PumpCoin) -> usize {
    let (mut env, c) = Env::fork_pump_coin(coin);
    let u = user(&mut env, &c);
    let mut executed = 0;
    for (k, q) in BUYS.iter().enumerate() {
        executed += usize::from(exact_buy(&mut env, &c, &u, *q, k as u64));
    }
    for (k, q) in SELLS.iter().enumerate() {
        executed += usize::from(exact_sell(&mut env, &c, &u, *q, 3 + k as u64));
    }
    for (k, s) in SPENDS.iter().enumerate() {
        executed += usize::from(exact_quote_in(&mut env, &c, &u, *s, 7 + k as u64));
    }
    executed
}

#[test]
fn fresh_125_byte_curve() {
    assert!(grid(PumpCoin::Fresh) >= 30);
}

#[test]
fn near_completion_151_byte_curve() {
    assert!(grid(PumpCoin::Near) >= 25);
}

#[test]
fn token_2022_115_byte_curve() {
    assert!(grid(PumpCoin::T22_115) >= 30);
}

#[test]
fn legacy_spl_115_byte_curve_without_a_creator() {
    let (env, c) = Env::fork_pump_coin(PumpCoin::Legacy115);
    assert_eq!(c.token_program.id(), shortwin::constants::TOKEN_PROGRAM_ID);
    assert!(c.state(&env.svm).creator_is_default());
    assert_eq!(c.fees(&env.svm).unwrap().creator, 0);
    drop(env);
    assert!(grid(PumpCoin::Legacy115) >= 30);
}

/// The 2024 layout: the first v2 trade grows the curve to 125 bytes (the fields past the old end
/// zero), the user paying its rent reserve up to `minimum_balance(125)` — 53,320 lamports here:
/// the reserve was the old rent of 49 bytes —, and every trade after it is exact.
#[test]
fn legacy_spl_49_byte_curve_grows_to_125() {
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::Legacy49);
    assert_eq!(c.data_len(&env.svm), pump::CURVE_LEN_ORIGINAL);
    let s = c.state(&env.svm);
    let reserve = read::lamports(&env.svm, &c.curve) - s.real_quote_reserves;
    assert_eq!(
        reserve, 1_231_920,
        "the rent of 49 bytes at the old 6,960 lamports per byte"
    );
    let u = user(&mut env, &c);
    let u0 = read::lamports(&env.svm, &u.kp.pubkey());
    let ch = c.charge_buy(&env.svm, 1_000_000_000_000).unwrap();
    assert!(exact_buy(&mut env, &c, &u, 1_000_000_000_000, 0));
    assert_eq!(c.data_len(&env.svm), pump::CURVE_LEN_V2);
    assert_eq!(
        u0 - read::lamports(&env.svm, &u.kp.pubkey()),
        ch.cost + 53_320
    );
    let acc = env.svm.get_account(&c.curve).unwrap();
    assert!(acc.data[pump::CURVE_LEN_ORIGINAL..].iter().all(|&b| b == 0));
    assert_eq!(
        acc.lamports - c.state(&env.svm).real_quote_reserves,
        env.svm
            .minimum_balance_for_rent_exemption(pump::CURVE_LEN_V2)
    );
    drop(env);
    assert!(grid(PumpCoin::Legacy49) >= 30);
}

// ---- beyond rT, completion, complete curves ----------------------------------------------------

/// A buy beyond `rT` reverts (6021, no cap, no partial fill) with `buy_v2` and with
/// `buy_exact_quote_in_v2`; the buy of exactly `rT` completes the curve at the exact-out price;
/// then every trade reverts with 6005 (R-PUMP §7.2–§7.3).
#[test]
fn beyond_rt_reverts_and_the_completing_buy_completes() {
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::Near);
    let u = user(&mut env, &c);
    let rt = c.state(&env.svm).real_token_reserves;
    for q in [rt + 1, rt + 1_000_000_000_000, 2 * rt] {
        assert!(!exact_buy(&mut env, &c, &u, q, 1), "q {q} > rT {rt}");
    }
    // Enough SOL for twice the curve: the exact-in buy asks for more than rT.
    let need = c.charge_buy(&env.svm, rt).unwrap().cost;
    let res = trade(&mut env, &c, &u, PumpV2::BuyExactQuoteIn, 3 * need, 1, 2);
    assert_eq!(
        custom_code(&res),
        Some(fork::PUMP_ERR_NOT_ENOUGH_TOKENS_TO_BUY),
        "{}",
        describe(&res)
    );
    assert!(c.quote_exact_quote_in(&env.svm, 3 * need).is_none());
    // The completing buy.
    assert!(exact_buy(&mut env, &c, &u, rt, 3));
    let s = c.state(&env.svm);
    assert!(s.complete && s.is_migrating() && s.real_token_reserves == 0);
    assert_eq!(
        read::token_amount(&env.svm, &c.curve_base_ata),
        Some(206_900_000_000_000)
    );
    // Complete: every trade reverts with 6005, whatever its size.
    for q in [1, 1_000_000, rt] {
        assert!(!exact_buy(&mut env, &c, &u, q, 4));
        assert!(!exact_sell(&mut env, &c, &u, q, 5));
    }
    assert!(!exact_quote_in(&mut env, &c, &u, 1_000_000_000, 6));
    for kind in [PumpV2::Buy, PumpV2::Sell, PumpV2::BuyExactQuoteIn] {
        let res = trade(
            &mut env,
            &c,
            &u,
            kind,
            1_000,
            if kind == PumpV2::Buy { u64::MAX } else { 0 },
            7,
        );
        assert_eq!(
            custom_code(&res),
            Some(fork::PUMP_ERR_BONDING_CURVE_COMPLETE),
            "{kind:?}: {}",
            describe(&res)
        );
    }
}

/// The dump's migrated curves (PumpSwap graduates old and new, and Fartcoin, which graduated to
/// Raydium) refuse every trade with 6005.
#[test]
fn migrated_curves_refuse_every_trade() {
    for coin in [PumpCoin::Graduated, PumpCoin::Legacy150, PumpCoin::Fartcoin] {
        let (mut env, c) = Env::fork_pump_coin(coin);
        assert!(c.state(&env.svm).is_migrated());
        assert!(c.charge_buy(&env.svm, 1).is_none() && c.quote_sell(&env.svm, 1).is_none());
        let u = user(&mut env, &c);
        fork::set_token_amount(&mut env.svm, &u.ata, 1_000_000);
        for kind in [PumpV2::Buy, PumpV2::Sell, PumpV2::BuyExactQuoteIn] {
            let bound = if kind == PumpV2::Buy { u64::MAX } else { 0 };
            let res = trade(&mut env, &c, &u, kind, 1_000, bound, 0);
            assert_eq!(
                custom_code(&res),
                Some(fork::PUMP_ERR_BONDING_CURVE_COMPLETE),
                "{coin:?} {kind:?}: {}",
                describe(&res)
            );
        }
    }
}

/// The undocumented trailing byte of R-PUMP Update 2026-09-29 U7 on `buy_v2`: absent or 0 keeps
/// the 6021 revert; 1 fills exactly `rT` at the exact-out charge of `rT` and completes the
/// curve; 2 does not deserialise. short.win never sets it (the protocol owes exactly `Q`).
#[test]
fn the_trailing_cap_byte_fills_exactly_rt() {
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::Near);
    let u = user(&mut env, &c);
    let rt = c.state(&env.svm).real_token_reserves;
    let ch = c.charge_buy(&env.svm, rt).unwrap();
    let q = rt + 1_000_000_000_000;
    let (fr, bb) = recipients(&env.svm, 0);
    let base = c.v2_ix(
        &env.svm,
        PumpV2::Buy,
        &u.kp.pubkey(),
        &u.ata,
        &fr,
        &bb,
        q,
        u64::MAX,
    );
    let ix = |flag: Option<u8>| {
        let mut ix = base.clone();
        ix.data.extend(flag);
        ix
    };
    let signer = u.kp.insecure_clone();
    for flag in [None, Some(0u8)] {
        let res = env.send(&[ix(flag)], &[&signer]);
        assert_eq!(
            custom_code(&res),
            Some(fork::PUMP_ERR_NOT_ENOUGH_TOKENS_TO_BUY),
            "{flag:?}: {}",
            describe(&res)
        );
    }
    let res = env.send(&[ix(Some(2))], &[&signer]);
    assert_eq!(
        custom_code(&res),
        Some(102),
        "InstructionDidNotDeserialize: {}",
        describe(&res)
    );
    let before = c.state(&env.svm);
    let u0 = read::lamports(&env.svm, &u.kp.pubkey());
    let meta = ok(env.send(&[ix(Some(1))], &[&signer]), "capped buy");
    let ev = one_trade(&meta);
    let after = c.state(&env.svm);
    assert_eq!((ev.token_amount, ev.sol_amount), (rt, ch.net));
    assert_eq!(u0 - read::lamports(&env.svm, &u.kp.pubkey()), ch.cost);
    assert!(after.complete && after.real_token_reserves == 0);
    assert_eq!(
        after.virtual_quote_reserves,
        before.virtual_quote_reserves + ch.net
    );
}

// ---- fee tiers through the real pump-fees program ------------------------------------------------

/// The market cap pump-fees selects a curve's tier by: `⌊vQ·10¹⁵/vT⌋` (R-PUMP §4 step 1).
fn mcap(c: &pump::BondingCurve) -> u128 {
    u128::from(c.virtual_quote_reserves) * mfees::PUMP_CURVE_MCAP_SUPPLY
        / u128::from(c.virtual_token_reserves)
}

/// The market cap the **deployed** pump-fees resolves a curve's tier at: `⌊vQ·supply/vT⌋` with the
/// mint's **live** supply (burns lower it), not the fixed 10¹⁵ of R-PUMP §4 step 1 / ARCH §4.2 /
/// `fees::pump_curve` (see `tripwire_the_curve_tier_market_cap_uses_the_live_mint_supply`).
fn mcap_live(svm: &LiteSVM, c: &PumpCurve) -> u128 {
    let s = c.state(svm);
    let mint = svm.get_account(&c.mint).unwrap();
    let supply = shortwin_venue_state::mint::read_base(&mint.owner.to_bytes(), &mint.data)
        .unwrap()
        .supply;
    u128::from(s.virtual_quote_reserves) * u128::from(supply) / u128::from(s.virtual_token_reserves)
}

/// A buy, a sale and an exact-in buy on each live curve, under `FeeConfig` tiers rewritten as the
/// admin's `update_fee_config` would leave them, are charged exactly the tier
/// `fees::pump_curve` selects: PumpSwap's 25 live market-cap tiers (tier 0 carries an LP fee the
/// curve never charges), thresholds at the curve's own market cap and one above it, an unsorted
/// list ("the last tier whose threshold is ≤ mcap"), a tier above the market cap only (tier 0
/// applies), and a 3.82 % schedule. On a coin whose supply was burned the thresholds bracket both
/// market caps (fixed-supply and live-supply), the band between them being the tripwire's.
#[test]
fn fee_tiers_as_the_fee_program_resolves_them() {
    let pamm_tiers = {
        let (env, _) = Env::fork_pump_coin(PumpCoin::Fresh);
        fork::fee_config_specs(&env.svm, &PUMP_AMM_FEE_CONFIG).1
    };
    for coin in [
        PumpCoin::Fresh,
        PumpCoin::Near,
        PumpCoin::T22_115,
        PumpCoin::Legacy115,
    ] {
        let schedules: Vec<Vec<fork::TierSpec>> = {
            let (env, c) = Env::fork_pump_coin(coin);
            let (a, b) = (mcap(&c.state(&env.svm)), mcap_live(&env.svm, &c));
            let (lo, hi) = (a.min(b), a.max(b));
            vec![
                pamm_tiers.clone(),
                vec![(0, [0, 95, 30]), (lo, [0, 40, 10]), (hi + 1, [0, 10, 3])],
                vec![(0, [0, 95, 30]), (hi + 1, [0, 40, 10])],
                vec![
                    (lo / 2, [0, 70, 20]),
                    (lo - 1, [0, 50, 0]),
                    (u128::MAX, [0, 1, 1]),
                ],
                vec![(hi * 2, [0, 120, 45])],
                vec![(0, [7, 250, 125])],
            ]
        };
        for tiers in schedules {
            let (mut env, c) = Env::fork_pump_coin(coin);
            fork::set_fee_config(
                &mut env.svm,
                &PUMP_FEE_CONFIG,
                [0, 95, 30],
                &tiers,
                &tiers,
                [0, 95, 30],
            );
            let fees = c.fees(&env.svm).unwrap();
            let s = c.state(&env.svm);
            let math_tiers: Vec<MathFeeTier> = tiers
                .iter()
                .map(|(t, f)| MathFeeTier {
                    threshold: *t,
                    fees: FeeBps::new(f[0], f[1], f[2]),
                })
                .collect();
            let tier = mfees::fee_tier(&math_tiers, mcap(&s)).unwrap();
            let creator = if s.creator_is_default() {
                0
            } else {
                tier.creator
            };
            assert_eq!(
                fees,
                FeeBps::new(0, tier.protocol, creator),
                "{coin:?} {tiers:?}"
            );
            // The reader's streamed selection is the same tier.
            let fc = env.svm.get_account(&PUMP_FEE_CONFIG).unwrap();
            let rc = pump::read_fee_config(&fc.owner.to_bytes(), &fc.data).unwrap();
            let sel = rc.fee_tiers.select(mcap(&s)).unwrap();
            assert_eq!(
                (sel.lp, sel.protocol, sel.creator),
                (tier.lp, tier.protocol, tier.creator)
            );
            let u = user(&mut env, &c);
            assert!(
                exact_buy(&mut env, &c, &u, 2_000_000_000_000, 1),
                "{coin:?} {tiers:?}"
            );
            assert!(
                exact_sell(&mut env, &c, &u, 1_000_000_000_000, 2),
                "{coin:?} {tiers:?}"
            );
            assert!(
                exact_quote_in(&mut env, &c, &u, 300_000_000, 3),
                "{coin:?} {tiers:?}"
            );
        }
    }
}

/// **Tripwire (a shortwin-math / ARCH §4.2 deviation found here; M5 design note).** The deployed
/// pump resolves the curve's fee tier at `⌊vQ·mint.supply/vT⌋` with the mint's live supply, while
/// `fees::pump_curve` (ARCH §4.2, R-PUMP §4 step 1) uses a fixed 10¹⁵. The two differ only on a
/// coin whose supply was burned (the near-completion coin: 999,978,034,493,483), inside the band
/// `[mcap_live, mcap_fixed)` of a tier threshold. With the single live tier (threshold 0) no
/// quote differs today. A threshold in the band: the ELF charges the lower tier, the math the
/// upper. When the math takes the live supply, the second assertion flips.
#[test]
fn tripwire_the_curve_tier_market_cap_uses_the_live_mint_supply() {
    for coin in [PumpCoin::Near, PumpCoin::Legacy115] {
        let (mut env, c) = Env::fork_pump_coin(coin);
        let (fixed, live) = (mcap(&c.state(&env.svm)), mcap_live(&env.svm, &c));
        assert!(live < fixed, "{coin:?}: a burned supply");
        let lower = [0, 95, 30];
        let upper = [0, 40, 10];
        let tiers = [(0, lower), (fixed, upper)];
        fork::set_fee_config(&mut env.svm, &PUMP_FEE_CONFIG, lower, &tiers, &tiers, lower);
        let creator = |bps: u64| {
            if c.state(&env.svm).creator_is_default() {
                0
            } else {
                bps
            }
        };
        // shortwin-math: the upper tier (fixed supply).
        assert_eq!(
            c.fees(&env.svm).unwrap(),
            FeeBps::new(0, 40, creator(10)),
            "{coin:?}"
        );
        // The ELF: the lower tier (live supply), charged exactly.
        let elf = FeeBps::new(0, 95, creator(30));
        let s = c.curve_state(&env.svm);
        let q = 1_000_000_000_000;
        let ch = sol_leg::curve_charge_exact_out(q, s.vt, s.vq, &elf).unwrap();
        let u = user(&mut env, &c);
        let res = trade(&mut env, &c, &u, PumpV2::Buy, q, ch.cost - 1, 0);
        assert_eq!(
            custom_code(&res),
            Some(fork::PUMP_ERR_TOO_MUCH_SOL_REQUIRED)
        );
        let u0 = read::lamports(&env.svm, &u.kp.pubkey());
        let meta = ok(trade(&mut env, &c, &u, PumpV2::Buy, q, ch.cost, 0), "buy");
        assert_eq!(u0 - read::lamports(&env.svm, &u.kp.pubkey()), ch.cost);
        let ev = one_trade(&meta);
        assert_eq!((ev.fee_basis_points, ev.sol_amount), (95, ch.net));
        // At a threshold ≤ the live cap both agree again.
        let (mut env, c) = Env::fork_pump_coin(coin);
        let tiers = [(0, lower), (live, upper)];
        fork::set_fee_config(&mut env.svm, &PUMP_FEE_CONFIG, lower, &tiers, &tiers, lower);
        let u = user(&mut env, &c);
        assert!(exact_buy(&mut env, &c, &u, q, 0), "{coin:?}");
    }
}

/// `creator = default` charges no creator fee whatever the tier says; `Global.creator_fee_
/// configurable ∧ curve.creator_fee_bps ≠ 0` replaces the tier's creator bps; with the flag off
/// the tier's applies (R-PUMP §4 steps 4–5; `fees::pump_curve`).
#[test]
fn creator_fee_rules() {
    // Default creator (the legacy coins): no creator fee even at a 125-bps tier.
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::Legacy115);
    fork::set_fee_config(
        &mut env.svm,
        &PUMP_FEE_CONFIG,
        [0, 95, 30],
        &[(0, [0, 95, 125])],
        &[],
        [0, 95, 30],
    );
    assert_eq!(c.fees(&env.svm).unwrap(), FeeBps::new(0, 95, 0));
    let u = user(&mut env, &c);
    assert!(exact_buy(&mut env, &c, &u, 3_000_000_000_000, 0));
    assert!(exact_sell(&mut env, &c, &u, 1_000_000_000_000, 1));
    assert!(exact_quote_in(&mut env, &c, &u, 700_000_000, 2));

    // The per-coin override on a SOL curve (a byte-level state; create_v2 sets it only for
    // custom pairs), with the flag on and off.
    for configurable in [true, false] {
        let (mut env, c) = Env::fork_pump_coin(PumpCoin::Fresh);
        fork::set_curve_creator_fee_bps(&mut env.svm, &c.curve, 200);
        fork::set_global_creator_fee_configurable(&mut env.svm, configurable);
        let want = if configurable { 200 } else { 30 };
        assert_eq!(c.fees(&env.svm).unwrap(), FeeBps::new(0, 95, want));
        let u = user(&mut env, &c);
        assert!(
            exact_buy(&mut env, &c, &u, 3_000_000_000_000, 0),
            "configurable {configurable}"
        );
        assert!(
            exact_sell(&mut env, &c, &u, 1_000_000_000_000, 1),
            "configurable {configurable}"
        );
        assert!(
            exact_quote_in(&mut env, &c, &u, 700_000_000, 2),
            "configurable {configurable}"
        );
    }
}

// ---- venue-side rent -----------------------------------------------------------------------------

/// A creator vault that does not exist is topped up to `Rent.minimum_balance(0)` (650,240 under
/// mainnet's Rent) from the user, on top of the creator fee — on a buy, and on a sale before
/// the proceeds are credited (R-PUMP Update 2026-09-29 U6; T-RENT, curve part). A funded vault
/// takes only the fee.
#[test]
fn an_empty_creator_vault_is_topped_up_by_the_user() {
    for kind in [PumpV2::Buy, PumpV2::Sell, PumpV2::BuyExactQuoteIn] {
        let (mut env, c) = Env::fork_pump_coin(PumpCoin::Fresh);
        assert_eq!(
            env.svm.minimum_balance_for_rent_exemption(0),
            650_240,
            "mainnet Rent"
        );
        let u = user(&mut env, &c);
        // Some SOL in the curve for the sale.
        assert!(exact_buy(&mut env, &c, &u, 50_000_000_000_000, 0));
        let vault = c.creator_vault(&env.svm);
        env.svm
            .set_account(vault, solana_account::Account::default())
            .unwrap();
        assert_eq!(read::lamports(&env.svm, &vault), 0);
        let executed = match kind {
            PumpV2::Buy => exact_buy(&mut env, &c, &u, 10_000_000_000_000, 1),
            PumpV2::Sell => exact_sell(&mut env, &c, &u, 10_000_000_000_000, 1),
            PumpV2::BuyExactQuoteIn => exact_quote_in(&mut env, &c, &u, 400_000_000, 1),
        };
        assert!(executed, "{kind:?}");
        assert!(
            read::lamports(&env.svm, &vault) >= 650_240,
            "{kind:?}: the vault is rent-exempt"
        );
    }
}

// ---- recipients --------------------------------------------------------------------------------

/// pump enforces the live `Global` lists that short.win checks as well (C0-14): a fee recipient
/// outside the 8 normal ones (a reserved one, a random key) is `NotAuthorized` (6000), a buyback
/// recipient outside its list 6057; every listed recipient is accepted (the grids rotate them).
#[test]
fn recipients_outside_the_live_lists_are_refused() {
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::Fresh);
    let u = user(&mut env, &c);
    let ch = c.charge_buy(&env.svm, 1_000_000_000).unwrap();
    let g = env.svm.get_account(&PUMP_GLOBAL).unwrap().data;
    let global = pump::read_global(&pump::PROGRAM, &g).unwrap();
    let reserved = Pubkey::new_from_array(global.reserved_fee_recipients()[1]);
    let normal = fork::pump_fee_recipient(&env.svm, 0);
    let buyback = fork::pump_buyback_fee_recipient(&env.svm, 0);
    let signer = u.kp.insecure_clone();
    let cases = [
        (reserved, buyback, fork::PUMP_ERR_NOT_AUTHORIZED),
        (Pubkey::new_unique(), buyback, fork::PUMP_ERR_NOT_AUTHORIZED),
        (
            normal,
            normal,
            fork::PUMP_ERR_BUYBACK_FEE_RECIPIENT_NOT_AUTHORIZED,
        ),
        (
            normal,
            Pubkey::new_unique(),
            fork::PUMP_ERR_BUYBACK_FEE_RECIPIENT_NOT_AUTHORIZED,
        ),
    ];
    for (fr, bb, want) in cases {
        for (kind, a, b) in [
            (PumpV2::Buy, 1_000_000_000, ch.cost),
            (PumpV2::Sell, 1_000_000, 0),
        ] {
            if kind == PumpV2::Sell {
                fork::set_token_amount(&mut env.svm, &u.ata, 1_000_000);
            }
            let ix = c.v2_ix(&env.svm, kind, &u.kp.pubkey(), &u.ata, &fr, &bb, a, b);
            let res = env.send(&[ix], &[&signer]);
            assert_eq!(
                custom_code(&res),
                Some(want),
                "{kind:?} {fr} {bb}: {}",
                describe(&res)
            );
        }
    }
    for k in 0..64 {
        assert!(exact_buy(&mut env, &c, &u, 1_000_000_000 + k, k));
    }
}

// ---- random sequences ----------------------------------------------------------------------------

/// A seeded xorshift.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Log-uniform in `[1, max]`.
    fn amount(&mut self, max: u64) -> u64 {
        let bits = 1 + self.next() % u64::from(64 - max.leading_zeros());
        let v = self.next() & (u64::MAX >> (64 - bits));
        v.clamp(1, max)
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Seeded random buys, sells and exact-in buys on the evolving state of every live curve, each
/// checked end to end.
#[test]
fn random_sequences_on_every_live_curve() {
    let n = env_u64("PUMP_MATH_SEQ", 30);
    let seed = env_u64("PUMP_MATH_SEED", 0x5eed_c0de_0005);
    for (i, coin) in PumpCoin::LIVE_SOL.into_iter().enumerate() {
        let (mut env, c) = Env::fork_pump_coin(coin);
        let u = user(&mut env, &c);
        let mut rng = Rng(seed ^ (i as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut executed = 0;
        for k in 0..n {
            let s = c.state(&env.svm);
            if s.complete {
                break;
            }
            executed += usize::from(match rng.next() % 3 {
                0 => {
                    let q = rng.amount(s.real_token_reserves.clamp(1, 400_000_000_000_000));
                    exact_buy(&mut env, &c, &u, q, k)
                }
                1 => exact_sell(&mut env, &c, &u, rng.amount(400_000_000_000_000), k),
                _ => exact_quote_in(&mut env, &c, &u, rng.amount(40_000_000_000), k),
            });
        }
        println!("{coin:?}: {executed} of {n} random trades executed");
        assert!(
            executed * 2 >= usize::try_from(n).unwrap(),
            "{coin:?}: {executed} of {n}"
        );
    }
}

// ---- fixtures, programs, readers ---------------------------------------------------------------------

/// The dumped programs are the ones ARCH §8.2 names (deploy slots = `VENUE_GUARD_SLOTS_2026_09_27`,
/// the ProgramData rebuilt to the mainnet sha256 when loaded), every pump-state account is one
/// snapshot, the reader constants equal the program's, the statics sit at their PDAs, and every
/// role holds what the dump script expects of it.
#[test]
fn fixtures_programs_and_readers() {
    for p in PumpProgram::ALL {
        let pd = p.program_data();
        let slot = program_data::deploy_slot(&pd.account.owner.to_bytes(), &pd.account.data)
            .expect("ProgramData header");
        let guard = usize::from(p.guard());
        assert_eq!(pd.address, VENUE_PROGRAM_DATA[guard], "{p:?}");
        assert_eq!(
            slot, VENUE_GUARD_SLOTS_2026_09_27[guard],
            "{p:?}: ARCH §8.2 deploy slot"
        );
        assert!(
            pd.account.data[program_data::HEADER_LEN + p.elf().len()..]
                .iter()
                .all(|&b| b == 0)
        );
        assert_eq!(&p.elf()[..4], b"\x7fELF");
        let prog = p.program();
        assert_eq!(prog.address, p.id());
        assert!(prog.account.executable);
        assert_eq!(prog.account.data[4..36], pd.address.to_bytes());
    }
    let s = fork::pump_snapshot();
    for f in fork::all_pump_fixtures().filter(|f| f.group == "pump_state") {
        assert_eq!(f.slot, s.slot, "{}", f.name);
    }
    assert!(s.block_time > 1_790_600_000);
    assert_eq!(pump::PROGRAM, PUMP_PROGRAM_ID.to_bytes());
    assert_eq!(pump::FEE_PROGRAM, PUMP_FEES_PROGRAM_ID.to_bytes());
    assert_eq!(pump_amm::PROGRAM, PUMP_AMM_PROGRAM_ID.to_bytes());
    let pda = |seeds: &[&[u8]], p: &Pubkey| Pubkey::find_program_address(seeds, p).0;
    assert_eq!(pda(&[pump::GLOBAL_SEED], &PUMP_PROGRAM_ID), PUMP_GLOBAL);
    assert_eq!(
        pda(
            &[pump::FEE_CONFIG_SEED, PUMP_PROGRAM_ID.as_ref()],
            &PUMP_FEES_PROGRAM_ID
        ),
        PUMP_FEE_CONFIG
    );
    assert_eq!(
        pda(&[pump_amm::GLOBAL_CONFIG_SEED], &PUMP_AMM_PROGRAM_ID),
        PUMP_AMM_GLOBAL_CONFIG
    );
    assert_eq!(
        pda(
            &[pump::FEE_CONFIG_SEED, PUMP_AMM_PROGRAM_ID.as_ref()],
            &PUMP_FEES_PROGRAM_ID
        ),
        PUMP_AMM_FEE_CONFIG
    );
    assert_eq!(
        pda(&[pump::EVENT_AUTHORITY_SEED], &PUMP_PROGRAM_ID),
        shortwin::constants::PUMP_EVENT_AUTHORITY
    );
    assert_eq!(
        pda(&[pump::GLOBAL_VOLUME_ACCUMULATOR_SEED], &PUMP_PROGRAM_ID),
        shortwin::constants::PUMP_GLOBAL_VOLUME_ACCUMULATOR
    );
    // Every role at its keys, with the expected shape.
    for coin in PumpCoin::ALL {
        let (env, c) = Env::fork_pump_coin(coin);
        assert_eq!(c.curve, pda::bonding_curve(&c.mint));
        let st = c.state(&env.svm);
        let len = c.data_len(&env.svm);
        match coin {
            PumpCoin::Fresh => assert!(st.is_live() && len == pump::CURVE_LEN_V2),
            PumpCoin::Near => {
                assert!(st.is_live() && len == pump::CURVE_LEN_EXTENDED);
                assert!(st.real_token_reserves <= 100_000_000_000_000);
            }
            PumpCoin::T22_115 | PumpCoin::Legacy115 => {
                assert!(st.is_live() && len == pump::CURVE_MIN_LEN)
            }
            PumpCoin::Legacy49 => assert!(st.is_live() && len == pump::CURVE_LEN_ORIGINAL),
            PumpCoin::Graduated => {
                assert!(st.is_migrated() && !st.is_mayhem_mode);
                let p = PumpPool::of(coin);
                let pool = p.state(&env.svm);
                assert_eq!(pool.base_mint, c.mint.to_bytes());
                assert!(pool.is_sol_quote() && pool.virtual_quote_reserves > 0);
                let r = p.reserves(&env.svm).unwrap();
                assert_eq!(
                    r.effective_quote,
                    u128::from(r.real_quote) + u128::try_from(pool.virtual_quote_reserves).unwrap()
                );
                assert!(
                    p.boost_amount(&env.svm) > 1_000_000_000,
                    "the boost is running"
                );
            }
            PumpCoin::Mayhem => assert!(st.is_mayhem_mode && st.is_live()),
            PumpCoin::Usdc => assert!(!st.is_sol_quote() && st.is_live()),
            PumpCoin::Legacy150 => {
                assert!(st.is_migrated() && len == pump::CURVE_LEN_LEGACY_EXTENDED);
                assert!(
                    PumpPool::of(coin).boost_amount(&env.svm) == 0,
                    "no boost ATA"
                );
            }
            PumpCoin::Fartcoin => {
                assert!(st.is_migrated());
                assert!(env.svm.get_account(&pda::canonical_pool(&c.mint)).is_none());
            }
        }
    }
}

/// The reader's streamed tier selection equals `shortwin_math::fees::fee_tier` on PumpSwap's 25
/// live tiers (and the stable ones) at every threshold, one below it, and at random caps.
#[test]
fn reader_tier_selection_matches_shortwin_math() {
    let (env, _) = Env::fork_pump_coin(PumpCoin::Fresh);
    let acc = env.svm.get_account(&PUMP_AMM_FEE_CONFIG).unwrap();
    let c = pump::read_fee_config(&acc.owner.to_bytes(), &acc.data).unwrap();
    let mut rng = Rng(7);
    for tiers in [c.fee_tiers, c.stable_fee_tiers] {
        let math: Vec<MathFeeTier> = tiers
            .iter()
            .map(|t| MathFeeTier {
                threshold: t.threshold,
                fees: FeeBps::new(t.fees.lp, t.fees.protocol, t.fees.creator),
            })
            .collect();
        let mut caps: Vec<u128> = vec![0, 1, u128::MAX];
        for t in tiers.iter() {
            caps.extend([t.threshold.saturating_sub(1), t.threshold, t.threshold + 1]);
        }
        caps.extend((0..500).map(|_| u128::from(rng.next() % 200_000_000_000_000)));
        for m in caps {
            let a = tiers.select(m).unwrap();
            let b = mfees::fee_tier(&math, m).unwrap();
            assert_eq!(
                (a.lp, a.protocol, a.creator),
                (b.lp, b.protocol, b.creator),
                "mcap {m}"
            );
        }
    }
}
