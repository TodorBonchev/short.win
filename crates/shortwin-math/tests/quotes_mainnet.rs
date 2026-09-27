//! Quotes against mainnet trades, to the lamport / raw unit (ARCHITECTURE §8.1, §4.3;
//! R-PUMP §4.1, R-VEN §2.4, R-RAY §3.4). Vectors: `tests/vectors/quotes/mainnet.json`,
//! extracted by `gen_mainnet.py` from the research passes' transaction dumps; every
//! expected value is one the venue itself reported.

mod common;

use common::Dec;
use serde::Deserialize;
use shortwin_math::fees::{self, CpmmFees, FeeBps, FeeTier, PammFeeSchedule};
use shortwin_math::quote::{self, CurveState, MigrationParams, cpmm, ray_v4, sol_leg};

#[derive(Deserialize)]
struct File {
    pump_curve: Vec<PumpTrade>,
    pump_migrations: Vec<Migration>,
    pump_fee_config: FeeConfig,
    pump_creator_fee_configurable: bool,
    pumpswap: Vec<PammTrade>,
    pumpswap_fee_config: FeeConfig,
    pumpswap_creator_fee_configurable: bool,
    ray_v4: Vec<V4Swap>,
    cpmm: Vec<CpSwap>,
}

#[derive(Deserialize)]
struct PumpTrade {
    sig: String,
    creator_is_default: bool,
    curve_creator_fee_bps: Dec<u64>,
    side: String,
    q: Dec<u64>,
    vt: Dec<u128>,
    vq: Dec<u128>,
    rq: Dec<u64>,
    protocol_bps: Dec<u64>,
    creator_bps: Dec<u64>,
    sol_amount: Dec<u64>,
    vt_after: Dec<u128>,
    vq_after: Dec<u128>,
    cost: Option<Dec<u64>>,
    proceeds: Option<Dec<u64>>,
}

#[derive(Deserialize)]
struct Migration {
    sig: String,
    vt: Dec<u128>,
    vq: Dec<u128>,
    rt: Dec<u128>,
    rq: Dec<u128>,
    migration_fee: Dec<u64>,
    pool_tokens: Dec<u128>,
    migrated_sol: Dec<u128>,
}

#[derive(Deserialize)]
struct PammTrade {
    sig: String,
    side: String,
    canonical: bool,
    mayhem: bool,
    pool_creator_fee_bps: Dec<u64>,
    q: Dec<u64>,
    base: Dec<u128>,
    quote_real: Dec<u64>,
    vqr: Dec<u128>,
    base_supply: Dec<u64>,
    coin_creator_is_default: bool,
    lp_bps: Dec<u64>,
    protocol_bps: Dec<u64>,
    creator_bps: Dec<u64>,
    lp_fee: Dec<u64>,
    creator_fee: Dec<u64>,
    cashback_bps: Dec<u64>,
    cashback: Dec<u64>,
    net: Option<Dec<u64>>,
    cost: Option<Dec<u64>>,
    gross: Option<Dec<u64>>,
    proceeds: Option<Dec<u64>>,
}

#[derive(Deserialize)]
struct FeeConfig {
    flat: [Dec<u64>; 3],
    tiers: Vec<Tier>,
}

impl FeeConfig {
    fn tiers(&self) -> Vec<FeeTier> {
        self.tiers
            .iter()
            .map(|t| FeeTier {
                threshold: t.threshold.0,
                fees: FeeBps::new(t.lp.0, t.protocol.0, t.creator.0),
            })
            .collect()
    }

    fn flat(&self) -> [FeeTier; 1] {
        [FeeTier {
            threshold: 0,
            fees: FeeBps::new(self.flat[0].0, self.flat[1].0, self.flat[2].0),
        }]
    }
}

#[derive(Deserialize)]
struct Tier {
    threshold: Dec<u128>,
    lp: Dec<u64>,
    protocol: Dec<u64>,
    creator: Dec<u64>,
}

#[derive(Deserialize)]
struct V4Swap {
    sig: String,
    kind: String,
    x_in: Dec<u128>,
    y_out: Dec<u128>,
    num: Dec<u64>,
    den: Dec<u64>,
    amount_in: Dec<u64>,
    amount_out: Dec<u64>,
}

#[derive(Deserialize)]
struct CpSwap {
    sig: String,
    kind: String,
    x_in: Dec<u128>,
    y_out: Dec<u128>,
    amount_in: Dec<u64>,
    amount_out: Dec<u64>,
    trade_rate: Dec<u64>,
    protocol_rate: Dec<u64>,
    fund_rate: Dec<u64>,
    creator_rate: Dec<u64>,
    enable_creator_fee: bool,
    creator_fee_on: u8,
    sol_pool: bool,
    sol_is_token0: bool,
    input_is_token0: bool,
    sol_in: bool,
    creator_on_input: bool,
    trade_fee: Dec<u64>,
    creator_fee: Dec<u64>,
}

fn load() -> File {
    common::load_vectors("quotes", "mainnet.json")
}

#[test]
fn pump_curve_trades() {
    let f = load();
    assert!(f.pump_curve.len() >= 120, "{}", f.pump_curve.len());
    let tiers = f.pump_fee_config.tiers();
    for t in &f.pump_curve {
        let fees = FeeBps::new(0, t.protocol_bps.0, t.creator_bps.0);
        let sched = fees::CurveFeeSchedule {
            tiers: &tiers,
            creator_fee_configurable: f.pump_creator_fee_configurable,
            curve_creator_fee_bps: t.curve_creator_fee_bps.0,
            creator_is_default: t.creator_is_default,
            cap_bps: None,
        };
        assert_eq!(
            fees::pump_curve(t.vt.0, t.vq.0, &sched),
            Ok(fees),
            "{}",
            t.sig
        );
        if t.side == "buy" {
            let b = sol_leg::curve_charge_exact_out(t.q.0, t.vt.0, t.vq.0, &fees)
                .unwrap_or_else(|| panic!("{}: no quote", t.sig));
            assert_eq!(b.net, t.sol_amount.0, "{}", t.sig);
            assert_eq!(b.cost, t.cost.as_ref().unwrap().0, "{}", t.sig);
            assert_eq!(
                (b.token_after, b.sol_after),
                (t.vt_after.0, t.vq_after.0),
                "{}",
                t.sig
            );
            // short.win's quote (ECON §3.1) covers the charge, by at most 1 + #fees.
            let quote = sol_leg::buy_exact_out(t.q.0, t.vt.0, t.vq.0, true, &fees).unwrap();
            assert!(
                quote.cost >= b.cost && quote.cost - b.cost <= 3,
                "{}",
                t.sig
            );
        } else {
            let s = sol_leg::sell_exact_in(t.q.0, t.vt.0, t.vq.0, &fees, t.rq.0)
                .unwrap_or_else(|| panic!("{}: no quote", t.sig));
            assert_eq!(s.proceeds, t.proceeds.as_ref().unwrap().0, "{}", t.sig);
            assert_eq!(
                (s.token_after, s.sol_after),
                (t.vt_after.0, t.vq_after.0),
                "{}",
                t.sig
            );
        }
    }
}

/// The straddle's post-migration pool (ARCH §4.3): `rQ + n(rT) − fee` on the completing
/// buy's pre-state is exactly the SOL `migrate_v2` moved when `n` is the program's charge,
/// and `migrated_pool` (ECON §3.1's quote `n`) is at most one lamport above it.
#[test]
fn pump_migrations() {
    let f = load();
    assert!(!f.pump_migrations.is_empty());
    for m in &f.pump_migrations {
        let state = CurveState {
            vt: m.vt.0,
            vq: m.vq.0,
            rt: m.rt.0,
            rq: m.rq.0,
            complete: false,
        };
        let params = MigrationParams {
            migration_fee: m.migration_fee.0,
            pool_tokens: m.pool_tokens.0,
        };
        let rt = u64::try_from(m.rt.0).unwrap();
        let fees = FeeBps::new(0, 0, 0);
        let charged = sol_leg::curve_charge_exact_out(rt, m.vt.0, m.vq.0, &fees).unwrap();
        let quoted = sol_leg::buy_exact_out(rt, m.vt.0, m.vq.0, true, &fees).unwrap();
        let fee = u128::from(m.migration_fee.0);
        assert_eq!(
            m.rq.0 + u128::from(charged.net) - fee,
            m.migrated_sol.0,
            "{}",
            m.sig
        );
        let extra = u128::from(quoted.net - charged.net);
        assert!(extra <= 1, "{}", m.sig);
        assert_eq!(
            quote::migrated_pool(&state, &params),
            Some((m.pool_tokens.0, m.migrated_sol.0 + extra)),
            "{}",
            m.sig
        );
    }
}

#[test]
fn pumpswap_trades_and_fee_tiers() {
    let f = load();
    let tiers = f.pumpswap_fee_config.tiers();
    let flat = f.pumpswap_fee_config.flat();
    assert_eq!(tiers.len(), 25);
    assert!(f.pumpswap.len() >= 100, "{}", f.pumpswap.len());
    let mut seen = std::collections::BTreeSet::new();
    for t in &f.pumpswap {
        let y = u128::from(t.quote_real.0) + t.vqr.0;
        // A cashback coin charges its creator-fee slot as cashback (R-VEN §1.5).
        assert!(t.creator_bps.0 == 0 || t.cashback_bps.0 == 0, "{}", t.sig);
        assert!(t.creator_fee.0 == 0 || t.cashback.0 == 0, "{}", t.sig);
        let charged = FeeBps::new(
            t.lp_bps.0,
            t.protocol_bps.0,
            t.creator_bps.0 + t.cashback_bps.0,
        );
        // Canonical pools pay the market-cap tiers, others `flat_fees`; mayhem pools use
        // a fixed 10¹⁵ supply as the market-cap basis (R-VEN §1.5, §2.3).
        let sched = PammFeeSchedule {
            tiers: if t.canonical { &tiers } else { &flat },
            base_supply: if t.mayhem {
                1_000_000_000_000_000
            } else {
                t.base_supply.0
            },
            creator_fee_configurable: f.pumpswap_creator_fee_configurable,
            pool_creator_fee_bps: t.pool_creator_fee_bps.0,
            coin_creator_is_default: t.coin_creator_is_default,
            cap_bps: None,
        };
        assert_eq!(
            fees::pump_amm(y, t.base.0, &sched),
            Ok(charged),
            "{}",
            t.sig
        );
        seen.insert((
            t.side.clone(),
            charged.lp,
            charged.protocol,
            charged.creator,
        ));
        if t.side == "buy" {
            let b = sol_leg::buy_exact_out(t.q.0, t.base.0, y, false, &charged).unwrap();
            assert_eq!(b.net, t.net.as_ref().unwrap().0, "{}", t.sig);
            assert_eq!(b.cost, t.cost.as_ref().unwrap().0, "{}", t.sig);
            // The quote vault grows by net + LP fee (R-VEN §1.4).
            let net = u128::from(t.net.as_ref().unwrap().0);
            assert_eq!(b.sol_after, y + net + u128::from(t.lp_fee.0), "{}", t.sig);
        } else {
            let s = sol_leg::sell_exact_in(t.q.0, t.base.0, y, &charged, t.quote_real.0).unwrap();
            assert_eq!(s.proceeds, t.proceeds.as_ref().unwrap().0, "{}", t.sig);
            let paid = u128::from(t.gross.as_ref().unwrap().0 - t.lp_fee.0);
            assert_eq!(s.sol_after, y - paid, "{}", t.sig);
            // One lamport less of real quote reserve and the payout no longer fits.
            let cap = u64::try_from(paid).unwrap();
            assert!(sol_leg::sell_exact_in(t.q.0, t.base.0, y, &charged, cap).is_some());
            assert!(sol_leg::sell_exact_in(t.q.0, t.base.0, y, &charged, cap - 1).is_none());
        }
    }
    // Buys and sells, and more than one fee schedule.
    assert!(seen.iter().any(|k| k.0 == "buy") && seen.iter().any(|k| k.0 == "sell"));
    assert!(seen.len() >= 15, "{seen:?}");
}

#[test]
fn raydium_amm_v4_swaps() {
    let f = load();
    assert!(f.ray_v4.len() >= 388, "{}", f.ray_v4.len());
    let (mut ins, mut outs) = (0, 0);
    for s in &f.ray_v4 {
        let fee = fees::ray_v4(s.num.0, s.den.0).unwrap();
        if s.kind == "exact_in" {
            let q = ray_v4::sell_exact_in(s.amount_in.0, s.x_in.0, s.y_out.0, fee.num, fee.den)
                .unwrap_or_else(|| panic!("{}: no quote", s.sig));
            assert_eq!(q.proceeds, s.amount_out.0, "{}", s.sig);
            assert_eq!(q.token_after, s.x_in.0 + u128::from(s.amount_in.0));
            ins += 1;
        } else {
            let q = ray_v4::buy_exact_out(s.amount_out.0, s.x_in.0, s.y_out.0, fee.num, fee.den)
                .unwrap_or_else(|| panic!("{}: no quote", s.sig));
            assert_eq!(q.cost, s.amount_in.0, "{}", s.sig);
            assert_eq!(q.token_after, s.y_out.0 - u128::from(s.amount_out.0));
            outs += 1;
        }
    }
    assert!(ins >= 377 && outs >= 11, "{ins} {outs}");
}

#[test]
fn raydium_cpmm_swaps() {
    let f = load();
    assert!(f.cpmm.len() >= 285, "{}", f.cpmm.len());
    let mut seen = std::collections::BTreeSet::new();
    for s in &f.cpmm {
        let cfg = fees::CpmmConfig {
            trade_fee_rate: s.trade_rate.0,
            protocol_fee_rate: s.protocol_rate.0,
            fund_fee_rate: s.fund_rate.0,
            creator_fee_rate: s.creator_rate.0,
        };
        // The side rule depends only on whether the input is token 0; a pool without SOL
        // is resolved as if its input token were the SOL side of a buy.
        let pool = fees::CpmmPoolFees {
            enable_creator_fee: s.enable_creator_fee,
            creator_fee_on: s.creator_fee_on,
            sol_is_token0: s.input_is_token0,
        };
        let resolved: CpmmFees = fees::cpmm(&cfg, &pool, fees::Direction::Buy).unwrap();
        if s.sol_pool {
            let dir = if s.sol_in {
                fees::Direction::Buy
            } else {
                fees::Direction::Sell
            };
            let sol_pool = fees::CpmmPoolFees {
                sol_is_token0: s.sol_is_token0,
                ..pool
            };
            assert_eq!(fees::cpmm(&cfg, &sol_pool, dir), Ok(resolved), "{}", s.sig);
        }
        // The venue's own side flag for this swap matches the resolution.
        assert_eq!(resolved.creator_on_input, s.creator_on_input, "{}", s.sig);
        seen.insert((
            s.kind.clone(),
            resolved.creator_on_input,
            resolved.creator > 0,
        ));

        let (tf, cf) = (u128::from(s.trade_fee.0), u128::from(s.creator_fee.0));
        let carved = tf * u128::from(s.protocol_rate.0) / 1_000_000
            + tf * u128::from(s.fund_rate.0) / 1_000_000;
        let creator_in = if s.creator_on_input { cf } else { 0 };
        let (x, y, a_in, a_out) = (s.x_in.0, s.y_out.0, s.amount_in.0, s.amount_out.0);
        if s.kind == "exact_in" {
            let q = cpmm::sell_exact_in(a_in, x, y, &resolved)
                .unwrap_or_else(|| panic!("{}: no quote", s.sig));
            assert_eq!(q.proceeds, a_out, "{}", s.sig);
            // Post-trade reserves from the venue's own fee split.
            assert_eq!(
                q.token_after,
                x + u128::from(a_in) - carved - creator_in,
                "{}",
                s.sig
            );
            let sw = u128::from(a_out) + if s.creator_on_input { 0 } else { cf };
            assert_eq!(q.sol_after, y - sw, "{}", s.sig);
        } else {
            let q = cpmm::buy_exact_out(a_out, x, y, &resolved)
                .unwrap_or_else(|| panic!("{}: no quote", s.sig));
            assert_eq!(q.cost, a_in, "{}", s.sig);
            assert_eq!(
                q.sol_after,
                x + u128::from(a_in) - carved - creator_in,
                "{}",
                s.sig
            );
            let actual = u128::from(a_out) + if s.creator_on_input { 0 } else { cf };
            assert_eq!(q.token_after, y - actual, "{}", s.sig);
        }
    }
    // Creator fee off, on the input, and on the output (exact in); exact out.
    for key in [
        ("exact_in".to_string(), true, false),
        ("exact_in".to_string(), true, true),
        ("exact_in".to_string(), false, true),
        ("exact_out".to_string(), true, false),
    ] {
        assert!(seen.contains(&key), "{key:?}");
    }
}

/// ARCH §4.2 fee-resolution branches with no mainnet trade in `mainnet.json` (review
/// F-S5). Each is covered by the reference-model proptests (`quotes_props.rs`
/// `fee_resolution`, `cpmm_equals_reference`) and, for CPMM, by `port.json`; the README
/// lists them as UNVERIFIED with their test gates.
const UNVERIFIED: [&str; 5] = [
    "curve: creator is the default key",
    "curve: per-curve creator_fee_bps override",
    "pumpswap: per-pool creator_fee_bps override",
    "cpmm: exact-out, creator fee on the input",
    "cpmm: exact-out, creator fee on the output",
];

/// Which ARCH §4.2 fee-resolution branches the mainnet trades exercise (review F-S5):
/// every branch is either covered by a trade or listed in `UNVERIFIED`, and a listed
/// branch that gains a trade must leave the list (and the README's UNVERIFIED items).
#[test]
fn fee_branch_coverage() {
    let f = load();
    let mut covered = std::collections::BTreeSet::new();
    for t in &f.pump_curve {
        if t.creator_is_default {
            covered.insert("curve: creator is the default key");
        }
        if t.curve_creator_fee_bps.0 != 0 {
            covered.insert("curve: per-curve creator_fee_bps override");
        }
        covered.insert("curve: schedule fees");
    }
    for t in &f.pumpswap {
        covered.insert(if t.canonical {
            "pumpswap: canonical (market-cap tiers)"
        } else {
            "pumpswap: non-canonical (flat_fees)"
        });
        if t.coin_creator_is_default {
            covered.insert("pumpswap: default coin creator");
        }
        if t.pool_creator_fee_bps.0 > 0 {
            covered.insert("pumpswap: per-pool creator_fee_bps override");
        }
    }
    for s in &f.cpmm {
        let with_creator = s.enable_creator_fee && s.creator_rate.0 > 0;
        covered.insert(match (s.kind.as_str(), with_creator, s.creator_on_input) {
            ("exact_out", true, true) => "cpmm: exact-out, creator fee on the input",
            ("exact_out", true, false) => "cpmm: exact-out, creator fee on the output",
            ("exact_out", false, _) => "cpmm: exact-out, no creator fee",
            (_, true, true) => "cpmm: exact-in, creator fee on the input",
            (_, true, false) => "cpmm: exact-in, creator fee on the output",
            _ => "cpmm: exact-in, no creator fee",
        });
    }
    for need in [
        "curve: schedule fees",
        "pumpswap: canonical (market-cap tiers)",
        "pumpswap: non-canonical (flat_fees)",
        "pumpswap: default coin creator",
        "cpmm: exact-out, no creator fee",
        "cpmm: exact-in, creator fee on the input",
        "cpmm: exact-in, creator fee on the output",
        "cpmm: exact-in, no creator fee",
    ] {
        assert!(covered.contains(need), "no mainnet trade for {need}");
    }
    for u in UNVERIFIED {
        assert!(
            !covered.contains(u),
            "{u} now has a mainnet trade: drop it from UNVERIFIED and the README"
        );
    }
}
