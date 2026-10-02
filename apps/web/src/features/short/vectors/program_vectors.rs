//! Program-derived vectors for the Short panel's previews (FRONTEND.md §10.1 SHORT: "the open
//! preview against `shortwin_sim`-derived numbers (margin, fee, CR, liquidation price, caps),
//! close / add-margin previews"): the **program itself** on LiteSVM (the TEST-ONLY `test-venue`
//! build on the mock venue, the M2 engine harness of `tests/litesvm`), so a preview is held to what
//! `open_short` / `close_short` / `add_margin` actually booked from the same accounts.
//!
//! Each case dumps every account the preview reads (config, venue guard, oracle cache, market,
//! market vault, mint, the mock ReadSet, the borrower's exposure and position) right after a
//! `crank_price` in the same second — so the instruction's prologue has nothing left to roll — and
//! then sends the instruction and records the program's result (the `Open` / `Close` event, the
//! position after, or the custom error code of a refusal).
//!
//! This file is the generator of `program-vectors.json` next to it and is not compiled by the web
//! build. To regenerate (needs `target/test-venue/deploy/shortwin.so` from
//! `scripts/build-test-venue.sh` and `target/deploy/mock_cpamm.so` from `anchor build`):
//!
//! ```sh
//! cp apps/web/src/features/short/vectors/program_vectors.rs tests/litesvm/examples/web_short_vectors.rs
//! cargo run -p shortwin-litesvm-tests --example web_short_vectors \
//!     > apps/web/src/features/short/vectors/program-vectors.json
//! rm tests/litesvm/examples/web_short_vectors.rs
//! ```

use anchor_lang::__private::base64::{self, Engine};
use shortwin::events::{Close, MarginAdded, Open};
use shortwin::state::ShortPosition;
use shortwin::venues::ExtrasSel;
use shortwin_litesvm_tests::engine::{
    self, Borrower, MarketSpec, MockMarket, add_margin_ix, close_short_ix,
    open_short_ix, with_cu_limit,
};
use shortwin_litesvm_tests::tokens::TokenProgram;
use shortwin_litesvm_tests::{Env, TxResult, errors, events, pda, read};

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// The accounts a preview reads, as `{address: {owner, lamports, data}}` (absent accounts out).
fn snapshot(env: &Env, mm: &MockMarket, b: &Borrower, nonce: Option<u64>) -> String {
    let market = mm.market();
    let mut keys = vec![
        pda::config(),
        pda::venue_guard(),
        pda::oracle_cache(),
        market,
        pda::market_vault(&market).0,
        pda::pool_vault(&market).0,
        mm.mint(),
        mm.mock().pool,
        mm.mock().base_vault,
        mm.mock().quote_vault,
        pda::exposure(&market, &b.key()).0,
        engine::mock_extras_key(),
        b.key(),
    ];
    if let Some(n) = nonce {
        keys.push(pda::position(&market, &b.key(), n).0);
    }
    let entries = keys
        .iter()
        .filter_map(|k| {
            read::account(&env.svm, k).map(|a| {
                format!(
                    "\"{k}\":{{\"owner\":\"{}\",\"lamports\":\"{}\",\"data\":\"{}\"}}",
                    a.owner,
                    a.lamports,
                    b64(&a.data)
                )
            })
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{entries}}}")
}

/// The instruction the program executed, as `{program, accounts: [{key, signer, writable}], data}`
/// (hex): the web plans must build exactly this from the same inputs.
fn ix_json(ix: &anchor_lang::solana_program::instruction::Instruction) -> String {
    let accounts = ix
        .accounts
        .iter()
        .map(|a| {
            format!(
                "{{\"key\":\"{}\",\"signer\":{},\"writable\":{}}}",
                a.pubkey, a.is_signer, a.is_writable
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let data = ix.data.iter().map(|b| format!("{b:02x}")).collect::<String>();
    format!(
        "{{\"program\":\"{}\",\"accounts\":[{accounts}],\"data\":\"{data}\"}}",
        ix.program_id
    )
}

fn clock(env: &Env) -> String {
    let c = env.clock();
    format!("{{\"unix\":{},\"slot\":{}}}", c.unix_timestamp, c.slot)
}

fn refusal(res: &TxResult) -> String {
    match errors::custom_code(res) {
        Some(code) => format!("{{\"ok\":false,\"code\":{code}}}"),
        None => format!("{{\"ok\":false,\"code\":null,\"err\":\"{:?}\"}}", res.as_ref().err().map(|e| &e.err)),
    }
}

fn position_json(p: &ShortPosition) -> String {
    format!(
        "{{\"q\":\"{}\",\"s\":\"{}\",\"m\":\"{}\",\"v\":\"{}\",\"nOpen\":\"{}\",\"feeOpen\":\"{}\",\
         \"rMin\":\"{}\",\"deadline\":{},\"tOpen\":{},\"tier\":{},\"entryPrice\":\"{}\",\
         \"crLiqBps\":{},\"penaltyBps\":{},\"maxLiqSlipBps\":{},\"rentProtocolBps\":{},\
         \"venueFeeHardCapBps\":{}}}",
        p.q,
        p.s,
        p.m,
        p.v,
        p.n_open,
        p.fee_open,
        p.r_min,
        p.deadline,
        p.t_open,
        p.tier,
        p.entry_price,
        p.cr_liq_bps,
        p.penalty_bps,
        p.max_liq_slip_bps,
        p.rent_protocol_bps,
        p.venue_fee_hard_cap_bps
    )
}

/// `open_short(q)` right after a crank in the same second, with the program's own bounds wide
/// open (min proceeds 0, max margin / fee `u64::MAX`), so the result is the program's math alone.
fn open_case(env: &mut Env, mm: &MockMarket, b: &mut Borrower, name: &str, q: u64) -> String {
    env.crank(mm);
    let nonce = b.next_nonce;
    let before = snapshot(env, mm, b, None);
    let at = clock(env);
    let ix = open_short_ix(mm, &b.key(), nonce, q, 0, u64::MAX, u64::MAX, ExtrasSel::NONE, &[]);
    let ixj = ix_json(&ix);
    let kp = b.kp.insecure_clone();
    let res = env.send(&with_cu_limit(mm, ix), &[&kp]);
    let result = match &res {
        Ok(meta) => {
            b.next_nonce += 1;
            let ev = events::one::<Open>(meta);
            let pos = env.position(mm, &b.key(), nonce);
            format!(
                "{{\"ok\":true,\"event\":{{\"s\":\"{}\",\"m\":\"{}\",\"v\":\"{}\",\"nOpen\":\"{}\",\
                 \"feeOpen\":\"{}\",\"tier\":{},\"spotWad\":\"{}\",\"emaWad\":\"{}\",\"bSpot\":\"{}\",\
                 \"bEma\":\"{}\",\"deadline\":{}}},\"position\":{}}}",
                ev.s,
                ev.m,
                ev.v,
                ev.n_open,
                ev.fee_open,
                ev.tier,
                ev.spot_wad,
                ev.ema_wad,
                ev.b_spot,
                ev.b_ema,
                ev.deadline,
                position_json(&pos)
            )
        }
        Err(_) => refusal(&res),
    };
    format!(
        "{{\"name\":\"{name}\",\"kind\":\"open\",\"mint\":\"{}\",\"borrower\":\"{}\",\"nonce\":{nonce},\
         \"q\":\"{q}\",\"clock\":{at},\"accounts\":{before},\"ix\":{ixj},\"result\":{result}}}",
        mm.mint(),
        b.key()
    )
}

/// `close_short` of `nonce` right after a crank in the same second, `max_buyback_cost` wide open.
fn close_case(env: &mut Env, mm: &MockMarket, b: &Borrower, name: &str, nonce: u64) -> String {
    env.crank(mm);
    let before = snapshot(env, mm, b, Some(nonce));
    let at = clock(env);
    let ix = close_short_ix(mm, &b.key(), nonce, u64::MAX, ExtrasSel::NONE, &[]);
    let ixj = ix_json(&ix);
    let kp = b.kp.insecure_clone();
    let res = env.send(&with_cu_limit(mm, ix), &[&kp]);
    let result = match &res {
        Ok(meta) => {
            let ev = events::one::<Close>(meta);
            format!(
                "{{\"ok\":true,\"event\":{{\"buybackCost\":\"{}\",\"rent\":\"{}\",\"rentProtocol\":\"{}\",\
                 \"borrowerPayout\":\"{}\",\"bLive\":\"{}\"}}}}",
                ev.buyback_cost, ev.rent, ev.rent_protocol, ev.borrower_payout, ev.b_live
            )
        }
        Err(_) => refusal(&res),
    };
    format!(
        "{{\"name\":\"{name}\",\"kind\":\"close\",\"mint\":\"{}\",\"borrower\":\"{}\",\"nonce\":{nonce},\
         \"position\":\"{}\",\"clock\":{at},\"accounts\":{before},\"ix\":{ixj},\"result\":{result}}}",
        mm.mint(),
        b.key(),
        pda::position(&mm.market(), &b.key(), nonce).0
    )
}

fn add_margin_case(
    env: &mut Env,
    mm: &MockMarket,
    b: &Borrower,
    name: &str,
    nonce: u64,
    amount: u64,
) -> String {
    env.crank(mm);
    let before = snapshot(env, mm, b, Some(nonce));
    let at = clock(env);
    let kp = b.kp.insecure_clone();
    let ix = add_margin_ix(mm, &b.key(), nonce, amount);
    let ixj = ix_json(&ix);
    let res = env.send(&[ix], &[&kp]);
    let result = match &res {
        Ok(meta) => {
            let ev = events::one::<MarginAdded>(meta);
            let pos = env.position(mm, &b.key(), nonce);
            format!(
                "{{\"ok\":true,\"amount\":\"{}\",\"position\":{}}}",
                ev.amount,
                position_json(&pos)
            )
        }
        Err(_) => refusal(&res),
    };
    format!(
        "{{\"name\":\"{name}\",\"kind\":\"add_margin\",\"mint\":\"{}\",\"borrower\":\"{}\",\
         \"nonce\":{nonce},\"position\":\"{}\",\"amount\":\"{amount}\",\"clock\":{at},\
         \"accounts\":{before},\"ix\":{ixj},\"result\":{result}}}",
        mm.mint(),
        b.key(),
        pda::position(&mm.market(), &b.key(), nonce).0
    )
}

fn main() {
    let mut cases: Vec<String> = Vec::new();
    let mut env = Env::test_venue();
    env.check_invariants = false;

    // ---- SPL market at the harness defaults: 10⁹ tokens (6 dp) against 1,000 SOL, 25 bps ----
    let spl = env.mock_market(MarketSpec::default());
    let mut alice = env.borrower();
    for (i, q) in [100_000_000_000u64, 2_000_000_000_000].into_iter().enumerate() {
        env.tick(&spl, 1);
        cases.push(open_case(&mut env, &spl, &mut alice, &format!("spl-open-{i}"), q));
    }
    // Above the per-position impact cap (CapPositionImpact), then within it but above the
    // wallet's share of the market cap (CapWallet).
    env.tick(&spl, 1);
    cases.push(open_case(&mut env, &spl, &mut alice, "spl-open-position-cap", 10_000_000_000_000));
    env.tick(&spl, 1);
    cases.push(open_case(&mut env, &spl, &mut alice, "spl-open-wallet-cap", 5_000_000_000_000));

    // Rent accrues, the price moves; add margin to one position, close the other after a rise,
    // then the first after a fall.
    let actor = env.actor(&spl, 50_000_000_000_000, 400_000_000_000);
    env.tick(&spl, 30);
    let up = env.spot(&spl) * 108 / 100;
    env.push_price(&spl, &actor, up);
    env.tick(&spl, 90);
    cases.push(add_margin_case(&mut env, &spl, &alice, "spl-add-margin", 1, 1_500_000_000));
    env.tick(&spl, 1);
    cases.push(close_case(&mut env, &spl, &alice, "spl-close-after-rise", 0));
    let down = env.spot(&spl) * 85 / 100;
    env.push_price(&spl, &actor, down);
    env.tick(&spl, 600);
    cases.push(close_case(&mut env, &spl, &alice, "spl-close-after-fall", 1));
    // Another borrower: an open closed within its minimum rent.
    let mut dave = env.borrower();
    env.tick(&spl, 1);
    cases.push(open_case(&mut env, &spl, &mut dave, "spl-open-again", 3_000_000_000_000));
    env.tick(&spl, 20);
    cases.push(close_case(&mut env, &spl, &dave, "spl-close-min-rent", 0));

    // A spot far from the EMA refuses opens (PriceDeviatesFromEma).
    let pump = env.spot(&spl) * 140 / 100;
    env.push_price(&spl, &actor, pump);
    env.warp(1);
    cases.push(open_case(&mut env, &spl, &mut dave, "spl-open-deviation", 1_000_000_000_000));

    // ---- Token-2022 market: 9 decimals, a 1 % fee, a thinner pool -----------------------------
    let t22 = env.mock_market(MarketSpec {
        program: TokenProgram::Token2022,
        decimals: 9,
        fee_ppm: 10_000,
        base: 500_000_000_000_000_000,
        quote: 80_000_000_000,
        lend: 60_000_000_000_000_000,
        warmup: 60,
    });
    let mut bob = env.borrower();
    env.tick(&t22, 1);
    cases.push(open_case(&mut env, &t22, &mut bob, "t22-open-0", 1_000_000_000_000_000));
    env.tick(&t22, 1);
    cases.push(open_case(&mut env, &t22, &mut bob, "t22-open-1", 4_000_000_000_000_000));
    env.tick(&t22, 45);
    cases.push(close_case(&mut env, &t22, &bob, "t22-close", 0));

    // ---- A market still in its warm-up (MarketWarmingUp) --------------------------------------
    let young = env.mock_market(MarketSpec {
        warmup: 10,
        ..MarketSpec::default()
    });
    let mut carol = env.borrower();
    cases.push(open_case(&mut env, &young, &mut carol, "warmup-open", 1_000_000_000_000));

    println!(
        "{{\"generator\":\"apps/web/src/features/short/vectors/program_vectors.rs\",\"cases\":[{}]}}",
        cases.join(",\n")
    );
}
