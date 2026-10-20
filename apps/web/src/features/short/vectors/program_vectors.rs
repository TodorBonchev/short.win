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
//! mkdir -p tests/litesvm/examples
//! cp apps/web/src/features/short/vectors/program_vectors.rs tests/litesvm/examples/web_short_vectors.rs
//! cargo run -p shortwin-litesvm-tests --example web_short_vectors \
//!     > apps/web/src/features/short/vectors/program-vectors.json
//! rm tests/litesvm/examples/web_short_vectors.rs
//! ```
//!
//! **pump v3 (design pump-final §8, PUMP-V3 P5):** with the argument `pump` it prints
//! `pump-vectors.json` instead: the **release** build on the dumped pump, PumpSwap and pump-fees
//! ELFs selected by `PumpDeploy::CURRENT` (the pump/PumpSwap 2026-10-19 deploys
//! observed on 2026-10-21 local; pump-fees remains the 2026-10-12 build).
//! `Env::fork_pump_coin(Near166)` needs `target/deploy/shortwin.so` and the
//! fixtures. It records a curve open (`sell_v3`), a close below the curve's `rT` and a close past it, which
//! completes the curve by pump's synthetic migration in one `close_short` (`B̂_v3`, owner D3),
//! and the keeper's `liquidate` past `rT` (T-SYNTH-2's ECON D@80 % on the fresh curve): its event
//! carries the program's risk view — `B̂_v3` at the exit risk cap (`b_live_risk`) and the EMA leg's
//! CR on the newest ring entry (`cr_ema_bps`) — which the position metrics must reproduce.
//! Each also carries the curve ReadSet and the mint (its live supply prices `B̂_v3`'s fee).
//!
//! ```sh
//! cargo run -p shortwin-litesvm-tests --example web_short_vectors -- pump \
//!     > apps/web/src/features/short/vectors/pump-vectors.json
//! ```

use anchor_lang::__private::base64::{self, Engine};
use anchor_lang::prelude::Pubkey;
use shortwin::errors::ShortwinError;
use shortwin::events::{Close, Liquidate, MarginAdded, Open};
use shortwin::state::{ShortPosition, VenueKind};
use shortwin::venues::ExtrasSel;
use shortwin_litesvm_tests::engine::{
    self, Borrower, MarketSpec, MockMarket, VenueMarket, add_margin_ix, close_short_ix,
    liquidate_ix, open_short_ix, with_cu_limit,
};
use shortwin_litesvm_tests::fork::{self, PumpCoin};
use shortwin_litesvm_tests::pump::{self, CurveUser, PumpMarket, PumpSpec};
use shortwin_litesvm_tests::tokens::TokenProgram;
use shortwin_litesvm_tests::{Env, TxResult, errors, events, pda, read};
use solana_keypair::Keypair;
use solana_signer::Signer;

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// The mock venue's accounts a preview reads: its ReadSet and the `VenueExtras` PDA.
fn mock_venue_keys(mm: &MockMarket) -> Vec<Pubkey> {
    vec![
        mm.mock().pool,
        mm.mock().base_vault,
        mm.mock().quote_vault,
        engine::mock_extras_key(),
    ]
}

/// A curve market's: the curve ReadSet (C0 … C6, the canonical pool absent while live) and the
/// PumpCurve `VenueExtras` PDA (absent: no set).
fn pump_venue_keys(pm: &PumpMarket) -> Vec<Pubkey> {
    let mut keys: Vec<Pubkey> = pm.read_set(false).iter().map(|a| a.pubkey).collect();
    keys.push(pda::venue_extras(VenueKind::PumpCurve));
    keys
}

/// The accounts a preview reads, as `{address: {owner, lamports, data}}` (absent accounts out):
/// the protocol accounts, the market's, the mint, `venue` (the venue's), the borrower's exposure,
/// the borrower and the position.
fn snapshot(
    env: &Env,
    mm: &impl VenueMarket,
    venue: &[Pubkey],
    b: &Borrower,
    nonce: Option<u64>,
) -> String {
    let market = mm.market();
    let mut keys = vec![
        pda::config(),
        pda::venue_guard(),
        pda::oracle_cache(),
        market,
        pda::market_vault(&market).0,
        pda::pool_vault(&market).0,
        mm.mint(),
    ];
    keys.extend_from_slice(venue);
    keys.push(pda::exposure(&market, &b.key()).0);
    keys.push(b.key());
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
fn open_case(
    env: &mut Env,
    mm: &impl VenueMarket,
    venue: &[Pubkey],
    b: &mut Borrower,
    name: &str,
    q: u64,
) -> String {
    env.crank(mm);
    // The first crank after an idle gap can only roll the old sample. Complete a fresh bucket
    // before snapshotting a successful open; advancing ema_ts alone no longer passes the gate.
    let m = env.market(&mm.market());
    let now = env.clock().unix_timestamp;
    let max_age = shortwin_math::consts::COVERAGE_GAP_S;
    if now.saturating_sub(m.ema_ts) > max_age
        || (m.lb_lo.get() != 0 && now.saturating_sub(m.lb_sec) > max_age)
    {
        env.warp(1);
        env.crank(mm);
    }
    let nonce = b.next_nonce;
    let before = snapshot(env, mm, venue, b, None);
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
fn close_case(
    env: &mut Env,
    mm: &impl VenueMarket,
    venue: &[Pubkey],
    b: &Borrower,
    name: &str,
    nonce: u64,
) -> String {
    env.crank(mm);
    let before = snapshot(env, mm, venue, b, Some(nonce));
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

/// The keeper's `liquidate` of `nonce`, tried every second right after its crank until it lands
/// (`synthetic_exit_it::liquidate_on_the_curve`); the refusals before are `NotLiquidatable` (the
/// EMA leg catching up) or `ExitBoundNotMet` (the reference behind a fast move). The case is the
/// second it landed: the accounts dumped after that second's crank, and the `Liquidate` event.
#[allow(clippy::too_many_arguments)]
fn liquidate_case(
    env: &mut Env,
    mm: &PumpMarket,
    venue: &[Pubkey],
    b: &Borrower,
    keeper: &Keypair,
    name: &str,
    nonce: u64,
    max_s: u32,
) -> String {
    for _ in 0..max_s {
        env.warp(1);
        env.crank(mm);
        let before = snapshot(env, mm, venue, b, Some(nonce));
        let at = clock(env);
        let ix = liquidate_ix(mm, &keeper.pubkey(), &b.key(), nonce);
        let ixj = ix_json(&ix);
        let res = env.send(&[pump::cu_limit_ix(pump::CURVE_KEEPER_EXIT_CU), ix], &[keeper]);
        let Ok(meta) = &res else {
            let code = errors::custom_code(&res);
            assert!(
                code == Some(u32::from(ShortwinError::NotLiquidatable))
                    || code == Some(u32::from(ShortwinError::ExitBoundNotMet)),
                "{name}: the liquidation refused {code:?}"
            );
            continue;
        };
        let ev = events::one::<Liquidate>(meta);
        assert!(!ev.expired, "{name}: a CR liquidation, not an expiry");
        let result = format!(
            "{{\"ok\":true,\"event\":{{\"crSpotBps\":\"{}\",\"crEmaBps\":\"{}\",\"bLive\":\"{}\",\
             \"bLiveRisk\":\"{}\",\"bRef\":\"{}\",\"maxIn\":\"{}\",\"buybackCost\":\"{}\",\
             \"rent\":\"{}\",\"penalty\":\"{}\",\"borrowerPayout\":\"{}\"}}}}",
            ev.cr_spot_bps,
            ev.cr_ema_bps,
            ev.b_live,
            ev.b_live_risk,
            ev.b_ref,
            ev.max_in,
            ev.buyback_cost,
            ev.rent,
            ev.penalty,
            ev.borrower_payout
        );
        return format!(
            "{{\"name\":\"{name}\",\"kind\":\"liquidate\",\"mint\":\"{}\",\"borrower\":\"{}\",\
             \"nonce\":{nonce},\"position\":\"{}\",\"clock\":{at},\"accounts\":{before},\
             \"ix\":{ixj},\"result\":{result}}}",
            mm.mint(),
            b.key(),
            pda::position(&mm.market(), &b.key(), nonce).0
        );
    }
    panic!("{name}: not liquidated within {max_s} s");
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
    let before = snapshot(env, mm, &mock_venue_keys(mm), b, Some(nonce));
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
    let (name, cases) = if std::env::args().nth(1).as_deref() == Some("pump") {
        ("program_vectors.rs pump", pump_cases())
    } else {
        ("program_vectors.rs", mock_cases())
    };
    println!(
        "{{\"generator\":\"apps/web/src/features/short/vectors/{name}\",\"cases\":[{}]}}",
        cases.join(",\n")
    );
}

/// pump v3 (design pump-final §5.2, §8): a curve market on `near166` (166 bytes, near completion)
/// as `synthetic_exit_it` lists it — `q = vT/200`, `40·q` lent, 60 s of warm-up. Two opens; the
/// smaller position closes below `rT`; the crowd then buys the curve down to `q/2`, and the larger
/// one's close buys `rT` on the curve and `q − rT` from the would-be pool in one `close_short`
/// (`B̂_v3`), completing the curve.
fn pump_cases() -> Vec<String> {
    let mut cases: Vec<String> = Vec::new();
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::Near166);
    env.check_invariants = false;
    let crowd = CurveUser::new(&mut env, &c);
    env.fund(&crowd.kp.pubkey(), 2_000_000_000_000);
    errors::ok(env.list_pump(&c));
    let q = c.state(&env.svm).virtual_token_reserves / 200;
    let pm = env.pump_market_listed(PumpSpec::new(PumpCoin::Near166, q * 40), c);
    let venue = pump_venue_keys(&pm);
    let mut erin = env.borrower();
    let mut frank = env.borrower();
    cases.push(open_case(&mut env, &pm, &venue, &mut erin, "pump-open", q));
    env.warp(1);
    cases.push(open_case(&mut env, &pm, &venue, &mut frank, "pump-open-small", q / 2));
    for _ in 0..2 {
        env.warp(1);
        env.crank(&pm);
    }
    cases.push(close_case(&mut env, &pm, &venue, &frank, "pump-close-below-rt", 0));
    env.warp(1);
    let rt = pm.curve.state(&env.svm).real_token_reserves;
    errors::ok(crowd.buy(&mut env, &pm.curve, rt - q / 2, u64::MAX));
    env.warp(1);
    cases.push(close_case(&mut env, &pm, &venue, &erin, "pump-close-past-rt", 0));
    cases.push(d80_liquidation_past_rt());
    cases
}

/// T-SYNTH-2's liquidation (`synthetic_exit_it::a_straddled_liquidation_lands_on_the_curve`):
/// ECON's `D@80 %` market (`Sim::d_market`: the fresh curve at pump's initial reserves, another
/// user's `buy_v2` of 80 % of `rT0`, `create_market(Auto)`, `⌈1.25·q⌉` lent, 60 s of warm-up, the
/// wallet cap at 10⁴ bps), its `q` opened, 30 s of cranks, the crowd buys the curve down to `rT =
/// q/2` (the price ×2.4, past Tier D's 175 %), and the keeper's `liquidate` lands on the curve,
/// completing it (`B̂_v3`).
fn d80_liquidation_past_rt() -> String {
    const VT0: u64 = 1_073_000_000_000_000;
    const VS0: u64 = 30_000_000_000;
    const RT0: u64 = 793_100_000_000_000;
    const SOLD: u64 = 634_480_000_000_000;
    const Q: u64 = 6_529_192_912_424;
    let mut env = Env::new();
    let mut args = env.default_init_args();
    args.params.wallet_cap_bps = 10_000;
    errors::ok(env.initialize_config(args));
    env.load_pump_programs();
    env.load_pump_statics();
    let c = env.load_pump_coin(PumpCoin::Fresh);
    env.set_pump_rent();
    env.set_pump_clock();
    env.check_invariants = false;
    // The fixture's 1-lamport creation buy undone in the reserves (`Sim::d_market`).
    fork::set_curve_reserves(&mut env.svm, &c.curve, VT0, VS0, RT0, 0);
    let crowd = CurveUser::new(&mut env, &c);
    errors::ok(crowd.buy(&mut env, &c, SOLD, u64::MAX));
    errors::ok(env.list_pump(&c));
    let pm = env.pump_market_listed(PumpSpec::new(PumpCoin::Fresh, (Q * 5).div_ceil(4)), c);
    let venue = pump_venue_keys(&pm);
    let keeper = env.funded_keypair();
    let mut gina = env.borrower();
    let (res, nonce) = env.open(&pm, &mut gina, Q);
    errors::ok(res);
    for _ in 0..30 {
        env.warp(1);
        env.crank(&pm);
    }
    let rt = pm.curve.state(&env.svm).real_token_reserves;
    errors::ok(crowd.buy(&mut env, &pm.curve, rt - Q / 2, u64::MAX));
    liquidate_case(
        &mut env,
        &pm,
        &venue,
        &gina,
        &keeper,
        "pump-liquidate-past-rt",
        nonce,
        900,
    )
}

fn mock_cases() -> Vec<String> {
    let mut cases: Vec<String> = Vec::new();
    let mut env = Env::test_venue();
    env.check_invariants = false;

    // ---- SPL market at the harness defaults: 10⁹ tokens (6 dp) against 1,000 SOL, 25 bps ----
    let spl = env.mock_market(MarketSpec::default());
    let v = mock_venue_keys(&spl);
    let mut alice = env.borrower();
    for (i, q) in [100_000_000_000u64, 2_000_000_000_000].into_iter().enumerate() {
        env.tick(&spl, 1);
        cases.push(open_case(&mut env, &spl, &v, &mut alice, &format!("spl-open-{i}"), q));
    }
    // Above the per-position impact cap (CapPositionImpact), then within it but above the
    // wallet's share of the market cap (CapWallet).
    env.tick(&spl, 1);
    cases.push(open_case(&mut env, &spl, &v, &mut alice, "spl-open-position-cap", 10_000_000_000_000));
    env.tick(&spl, 1);
    cases.push(open_case(&mut env, &spl, &v, &mut alice, "spl-open-wallet-cap", 5_000_000_000_000));

    // Rent accrues, the price moves; add margin to one position, close the other after a rise,
    // then the first after a fall.
    let actor = env.actor(&spl, 50_000_000_000_000, 400_000_000_000);
    env.tick(&spl, 30);
    let up = env.spot(&spl) * 108 / 100;
    env.push_price(&spl, &actor, up);
    env.tick(&spl, 90);
    cases.push(add_margin_case(&mut env, &spl, &alice, "spl-add-margin", 1, 1_500_000_000));
    env.tick(&spl, 1);
    cases.push(close_case(&mut env, &spl, &v, &alice, "spl-close-after-rise", 0));
    let down = env.spot(&spl) * 85 / 100;
    env.push_price(&spl, &actor, down);
    env.tick(&spl, 600);
    cases.push(close_case(&mut env, &spl, &v, &alice, "spl-close-after-fall", 1));
    // Another borrower: an open closed within its minimum rent.
    let mut dave = env.borrower();
    env.tick(&spl, 1);
    cases.push(open_case(&mut env, &spl, &v, &mut dave, "spl-open-again", 3_000_000_000_000));
    env.tick(&spl, 20);
    cases.push(close_case(&mut env, &spl, &v, &dave, "spl-close-min-rent", 0));

    // A spot far from the EMA refuses opens (PriceDeviatesFromEma).
    let pump = env.spot(&spl) * 140 / 100;
    env.push_price(&spl, &actor, pump);
    env.warp(1);
    cases.push(open_case(&mut env, &spl, &v, &mut dave, "spl-open-deviation", 1_000_000_000_000));

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
    let v22 = mock_venue_keys(&t22);
    let mut bob = env.borrower();
    env.tick(&t22, 1);
    cases.push(open_case(&mut env, &t22, &v22, &mut bob, "t22-open-0", 1_000_000_000_000_000));
    env.tick(&t22, 1);
    cases.push(open_case(&mut env, &t22, &v22, &mut bob, "t22-open-1", 4_000_000_000_000_000));
    env.tick(&t22, 45);
    cases.push(close_case(&mut env, &t22, &v22, &bob, "t22-close", 0));

    // ---- A market still in its warm-up (MarketWarmingUp) --------------------------------------
    let young = env.mock_market(MarketSpec {
        warmup: 10,
        ..MarketSpec::default()
    });
    let vy = mock_venue_keys(&young);
    let mut carol = env.borrower();
    cases.push(open_case(&mut env, &young, &vy, &mut carol, "warmup-open", 1_000_000_000_000));
    cases
}
