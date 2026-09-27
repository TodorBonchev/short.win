//! Quotes against the normative sim `docs/design/sim/shortwin_sim.py` v0.4.1, default
//! switches (ARCHITECTURE §8.1, §9 M0b). Vectors: `tests/vectors/quotes/sim.json`
//! (`gen_sim.py`). Every comparison with the sim is exact (no tolerance):
//!
//! - live execution quotes and sales on PumpSwap, the curve (ECON §3.1's
//!   `⌈q·vQ/(vT − q)⌉ + 1`, the straddle `B̂_str`) and Raydium;
//! - risk quotes at a fee hard cap: one combined fee at the cap on pump venues
//!   (`FeeBps::capped`, the sims' `Venue._risk_fees`), `min(rate, cap)` on Raydium;
//! - quotes on hypothetical states (`Market.quote_repriced`): `reprice` with
//!   `sp = isqrt⁺(P)`, X floor, Y ceil, at the live fee tier (R3B-7 b);
//! - `reprice`, `liquidity` (`sqrt_k`), `pool_select::{lambda_bps, locked_depth}`.
//!
//! The sim's own execution of a curve trade (`exact_*`: the program's `⌊·⌋ + 1` charge
//! and, for a straddle, the pool that migration actually creates) equals
//! `sol_leg::curve_charge_exact_out` on the curve and sits at most `tolerance` lamports
//! below the quote (a quote-vs-execution bound, ECON §3.2).

mod common;

use common::Dec;
use serde::Deserialize;
use shortwin_math::fees::{self, CpmmFees, FeeBps, FeeTier, PammFeeSchedule, RayV4Fee};
use shortwin_math::pool_select;
use shortwin_math::quote::{
    self, CurveOffsets, CurvePricing, CurveState, MigrationParams, Pricing, cpmm, ray_v4, sol_leg,
};

#[derive(Deserialize)]
struct File {
    generator: String,
    pumpswap_tiers: Vec<Tier>,
    pump_migration_fee: Dec<u64>,
    pump_pool_tokens: Dec<u128>,
    pump_supply: Dec<u64>,
    pumpswap: Vec<PammCase>,
    curve: Vec<CurveCase>,
    raydium: Vec<RayCase>,
    repriced: Vec<RepricedCase>,
    reprice: Vec<RepriceCase>,
    liquidity: Vec<LiquidityCase>,
    locked_depth: Vec<DepthCase>,
}

#[derive(Deserialize)]
struct Tier {
    threshold: Dec<u128>,
    lp: Dec<u64>,
    protocol: Dec<u64>,
    creator: Dec<u64>,
}

#[derive(Deserialize)]
struct PammCase {
    q: Dec<u64>,
    x: Dec<u128>,
    y: Dec<u128>,
    y_real: Dec<u64>,
    supply: Dec<u64>,
    fees: [Dec<u64>; 3],
    oneway_ppm: Dec<u32>,
    cap_bps: Dec<u64>,
    buy: Option<[Dec<u64>; 2]>,
    buy_risk: Option<[Dec<u64>; 2]>,
    sell: Option<Dec<u64>>,
}

#[derive(Deserialize)]
struct CurveCase {
    q: Dec<u64>,
    vt: Dec<u128>,
    vq: Dec<u128>,
    rt: Dec<u128>,
    rq: Dec<u128>,
    off_t: Dec<u128>,
    off_q: Dec<u128>,
    ema: Option<[Dec<u128>; 2]>,
    fees: [Dec<u64>; 3],
    cap_bps: Dec<u64>,
    sim_bundle: Option<Dec<u64>>,
    sim_risk: Option<Dec<u64>>,
    sim_two_leg: Option<Dec<u64>>,
    exact_bundle: Option<Dec<u64>>,
    exact_two_leg: Option<Dec<u64>>,
    sell_checked: bool,
    sell: Option<Dec<u64>>,
    tolerance: Dec<u64>,
    tolerance_two_leg: Dec<u64>,
}

#[derive(Deserialize)]
struct RayCase {
    q: Dec<u64>,
    x: Dec<u128>,
    y: Dec<u128>,
    fee_ppm: Dec<u64>,
    creator_ppm: Dec<u64>,
    oneway_ppm: Dec<u32>,
    cap_bps: Dec<u64>,
    buy: Option<[Dec<u64>; 2]>,
    buy_risk: Option<[Dec<u64>; 2]>,
    sell: Option<Dec<u64>>,
}

#[derive(Deserialize)]
struct RepricedCase {
    kind: String,
    x: Dec<u128>,
    y: Dec<u128>,
    supply: Dec<u64>,
    fee_ppm: Dec<u64>,
    creator_ppm: Dec<u64>,
    live_fees: Option<[Dec<u64>; 3]>,
    l: Dec<u128>,
    p: Dec<u128>,
    cap_bps: Dec<u64>,
    q: Dec<u64>,
    xy: Option<[Dec<u128>; 2]>,
    risk: Option<Dec<u64>>,
}

#[derive(Deserialize)]
struct RepriceCase {
    l: Dec<u128>,
    p: Dec<u128>,
    xy: Option<[Dec<u128>; 2]>,
}

#[derive(Deserialize)]
struct LiquidityCase {
    x: Dec<u128>,
    y: Dec<u128>,
    l: Option<Dec<u128>>,
}

#[derive(Deserialize)]
struct DepthCase {
    lp_counter: Dec<u64>,
    lp_supply: Dec<u64>,
    x: Dec<u64>,
    y: Dec<u64>,
    lambda_bps: Dec<u32>,
    locked_depth: Dec<u128>,
}

fn load() -> File {
    let f: File = common::load_vectors("quotes", "sim.json");
    assert_eq!(f.generator, "gen_sim.py");
    f
}

fn tiers(f: &File) -> Vec<FeeTier> {
    f.pumpswap_tiers
        .iter()
        .map(|t| FeeTier {
            threshold: t.threshold.0,
            fees: FeeBps::new(t.lp.0, t.protocol.0, t.creator.0),
        })
        .collect()
}

fn sched(tiers: &[FeeTier], supply: u64) -> PammFeeSchedule<'_> {
    PammFeeSchedule {
        tiers,
        base_supply: supply,
        creator_fee_configurable: false,
        pool_creator_fee_bps: 0,
        coin_creator_is_default: false,
        cap_bps: None,
    }
}

fn opt(x: &Option<Dec<u64>>) -> Option<u64> {
    x.as_ref().map(|d| d.0)
}

fn pair(x: &Option<[Dec<u64>; 2]>) -> Option<(u64, u64)> {
    x.as_ref().map(|[n, t]| (n.0, t.0))
}

/// The sim's Raydium pool as a CPMM with the creator fee on the input (its only model).
fn cpmm_fees(trade: u64, creator: u64) -> CpmmFees {
    CpmmFees {
        trade,
        creator,
        creator_on_input: true,
        protocol: 120_000,
        fund: 40_000,
    }
}

#[test]
fn pumpswap_matches_sim() {
    let f = load();
    let tiers = tiers(&f);
    assert!(f.pumpswap.len() >= 200);
    let mut combined = 0;
    for (i, c) in f.pumpswap.iter().enumerate() {
        let fee = fees::pump_amm(c.y.0, c.x.0, &sched(&tiers, c.supply.0)).unwrap();
        assert_eq!(
            fee,
            FeeBps::new(c.fees[0].0, c.fees[1].0, c.fees[2].0),
            "case {i}"
        );
        assert_eq!(fee.oneway_ppm(), c.oneway_ppm.0, "case {i}");
        let b = sol_leg::buy_exact_out(c.q.0, c.x.0, c.y.0, false, &fee);
        assert_eq!(b.map(|b| (b.net, b.cost)), pair(&c.buy), "case {i}");
        // Risk view: one combined fee at the cap when φ is above it.
        let cap = c.cap_bps.0;
        let risk_fee = fee.capped(cap);
        if fees::exceeds_cap(fee.oneway_ppm(), cap) {
            combined += 1;
            assert_eq!(risk_fee, FeeBps::new(0, cap, 0), "case {i}");
        } else {
            assert_eq!(risk_fee, fee, "case {i}");
        }
        let r = Pricing::PumpSwap(fee)
            .capped(cap)
            .buy(c.q.0, c.x.0, c.y.0)
            .map(|b| (b.net, b.cost));
        assert_eq!(r, pair(&c.buy_risk), "case {i} risk");
        // The schedule's own risk cap resolves to the same fee.
        let capped_sched = PammFeeSchedule {
            cap_bps: Some(cap),
            ..sched(&tiers, c.supply.0)
        };
        assert_eq!(
            fees::pump_amm(c.y.0, c.x.0, &capped_sched),
            Ok(risk_fee),
            "case {i}"
        );
        let s = sol_leg::sell_exact_in(c.q.0, c.x.0, c.y.0, &fee, c.y_real.0);
        assert_eq!(s.map(|s| s.proceeds), opt(&c.sell), "case {i}");
    }
    assert!(combined >= 50, "{combined}");
}

#[test]
fn curve_and_straddle_match_sim() {
    let f = load();
    let tiers = tiers(&f);
    let mig = MigrationParams {
        migration_fee: f.pump_migration_fee.0,
        pool_tokens: f.pump_pool_tokens.0,
    };
    let pool = sched(&tiers, f.pump_supply.0);
    let (mut straddles, mut emas, mut capped) = (0, 0, 0);
    for (i, c) in f.curve.iter().enumerate() {
        let off = CurveOffsets {
            token: c.off_t.0,
            quote: c.off_q.0,
        };
        let state = match &c.ema {
            None => CurveState {
                vt: c.vt.0,
                vq: c.vq.0,
                rt: c.rt.0,
                rq: c.rq.0,
                complete: false,
            },
            Some([l, p]) => {
                emas += 1;
                let s = quote::curve_reprice(l.0, p.0, off).unwrap();
                assert_eq!(
                    (s.vt, s.vq, s.rt, s.rq),
                    (c.vt.0, c.vq.0, c.rt.0, c.rq.0),
                    "case {i}"
                );
                s
            }
        };
        let fee = FeeBps::new(c.fees[0].0, c.fees[1].0, c.fees[2].0);
        let q = c.q.0;
        if u128::from(q) > state.rt {
            straddles += 1;
        }
        let pricing = CurvePricing {
            fees: fee,
            offsets: off,
            migration: mig,
            pool,
        };

        let b = quote::curve_straddle(q, &state, &fee, &mig, &pool).map(|b| b.cost);
        assert_eq!(b, opt(&c.sim_bundle), "case {i} bundle");
        assert_eq!(pricing.buy(q, &state).map(|b| b.cost), b, "case {i}");
        if let Some([l, p]) = &c.ema {
            assert_eq!(
                pricing.buy_repriced(q, l.0, p.0).map(|b| b.cost),
                b,
                "case {i}"
            );
        }
        assert_eq!(b.is_some(), c.exact_bundle.is_some(), "case {i}");
        if let (Some(b), Some(e)) = (b, opt(&c.exact_bundle)) {
            assert!(
                b >= e && b - e <= c.tolerance.0,
                "case {i}: exact {e} rust {b}"
            );
        }
        if u128::from(q) <= state.rt {
            let charge = sol_leg::curve_charge_exact_out(q, state.vt, state.vq, &fee);
            assert_eq!(
                charge.map(|b| b.cost),
                opt(&c.exact_bundle),
                "case {i} charge"
            );
        }

        // Risk quote: the curve's fee and the migrated pool's, each capped (V2-1).
        let risk = pricing.capped(c.cap_bps.0);
        if fees::exceeds_cap(fee.oneway_ppm(), c.cap_bps.0) {
            capped += 1;
        }
        let r = risk.buy(q, &state).map(|b| b.cost);
        assert_eq!(r, opt(&c.sim_risk), "case {i} risk");
        if let Some([l, p]) = &c.ema {
            assert_eq!(
                risk.buy_repriced(q, l.0, p.0).map(|b| b.cost),
                r,
                "case {i}"
            );
        }

        let t = quote::curve_two_leg(q, &state, &fee, &mig, &pool).map(|b| b.cost);
        assert_eq!(t, opt(&c.sim_two_leg), "case {i} two-leg");
        assert_eq!(t.is_some(), c.exact_two_leg.is_some(), "case {i}");
        if let (Some(t), Some(e)) = (t, opt(&c.exact_two_leg)) {
            assert!(
                t >= e && t - e <= c.tolerance_two_leg.0,
                "case {i}: exact {e} rust {t}"
            );
        }
        if let (Some(t), Some(b)) = (t, b) {
            // V1-9: the two-leg form never prices above the executed bundle.
            assert!(t <= b, "case {i}");
        }

        if c.sell_checked {
            let s = u64::try_from(state.rq).unwrap();
            let got = sol_leg::sell_exact_in(q, state.vt, state.vq, &fee, s).map(|s| s.proceeds);
            assert_eq!(got, opt(&c.sell), "case {i} sell");
        }
    }
    assert!(
        straddles >= 50 && emas >= 50 && capped >= 50,
        "{straddles} {emas} {capped}"
    );
}

#[test]
fn raydium_matches_sim() {
    let f = load();
    assert!(f.raydium.len() >= 150);
    let mut capped = 0;
    for (i, c) in f.raydium.iter().enumerate() {
        let (t, cr) = (c.fee_ppm.0, c.creator_ppm.0);
        let fee = cpmm_fees(t, cr);
        assert_eq!(fee.oneway_ppm(), c.oneway_ppm.0, "case {i}");
        // The sim's pool is (X token, Y SOL); a buy pays SOL in.
        let b = cpmm::buy_exact_out(c.q.0, c.y.0, c.x.0, &fee).map(|b| (b.net, b.cost));
        assert_eq!(b, pair(&c.buy), "case {i} cpmm buy");
        let s = cpmm::sell_exact_in(c.q.0, c.x.0, c.y.0, &fee).map(|s| s.proceeds);
        assert_eq!(s, opt(&c.sell), "case {i} cpmm sell");
        let cap = c.cap_bps.0;
        capped += usize::from(fees::exceeds_cap(fee.oneway_ppm(), cap));
        let rc = Pricing::Cpmm {
            buy: fee,
            sell: fee,
        }
        .capped(cap);
        let r = rc.buy(c.q.0, c.x.0, c.y.0).map(|b| (b.net, b.cost));
        assert_eq!(r, pair(&c.buy_risk), "case {i} cpmm risk");
        if cr == 0 {
            let v4 = fees::ray_v4(t, 1_000_000).unwrap();
            assert_eq!(u64::from(v4.oneway_ppm()), t, "case {i}");
            let b =
                ray_v4::buy_exact_out(c.q.0, c.y.0, c.x.0, v4.num, v4.den).map(|b| (b.net, b.cost));
            assert_eq!(b, pair(&c.buy), "case {i} v4 buy");
            let s = ray_v4::sell_exact_in(c.q.0, c.x.0, c.y.0, v4.num, v4.den).map(|s| s.proceeds);
            assert_eq!(s, opt(&c.sell), "case {i} v4 sell");
            let r = Pricing::RayV4(v4)
                .capped(cap)
                .buy(c.q.0, c.x.0, c.y.0)
                .map(|b| (b.net, b.cost));
            assert_eq!(r, pair(&c.buy_risk), "case {i} v4 risk");
        }
    }
    assert!(capped >= 30, "{capped}");
}

/// Risk quotes on hypothetical states (`Market.quote_repriced`): `reprice(L, P)` quoted at
/// the fees resolved on the live state (R3B-7 b), capped at the hard cap (V2-1).
#[test]
fn repriced_quotes_match_sim() {
    let f = load();
    let tiers = tiers(&f);
    let (mut defined, mut tier_differs) = (0, 0);
    for (i, c) in f.repriced.iter().enumerate() {
        let (l, p, q, cap) = (c.l.0, c.p.0, c.q.0, c.cap_bps.0);
        assert_eq!(
            quote::reprice(l, p),
            c.xy.as_ref().map(|[x, y]| (x.0, y.0)),
            "case {i}"
        );
        let live = match c.kind.as_str() {
            "pumpswap" => {
                let s = sched(&tiers, c.supply.0);
                let fee = fees::pump_amm(c.y.0, c.x.0, &s).unwrap();
                let lf = c.live_fees.as_ref().unwrap();
                assert_eq!(fee, FeeBps::new(lf[0].0, lf[1].0, lf[2].0), "case {i}");
                // Where the tier at the repriced market cap differs, the live one is
                // normative.
                if let Some((x, y)) = quote::reprice(l, p)
                    && fees::pump_amm(y, x, &s).is_ok_and(|h| h != fee)
                {
                    tier_differs += 1;
                }
                Pricing::PumpSwap(fee)
            }
            "raydium" => {
                let fee = cpmm_fees(c.fee_ppm.0, c.creator_ppm.0);
                Pricing::Cpmm {
                    buy: fee,
                    sell: fee,
                }
            }
            k => panic!("kind {k}"),
        };
        let got = live.capped(cap).buy_repriced(q, l, p).map(|b| b.cost);
        assert_eq!(got, opt(&c.risk), "case {i}");
        defined += usize::from(got.is_some());
    }
    assert!(
        defined >= 300 && tier_differs >= 10,
        "{defined} {tier_differs}"
    );
}

#[test]
fn reprice_matches_sim() {
    let f = load();
    for c in &f.reprice {
        let want = c.xy.as_ref().map(|[x, y]| (x.0, y.0));
        assert_eq!(
            quote::reprice(c.l.0, c.p.0),
            want,
            "L {} P {}",
            c.l.0,
            c.p.0
        );
        if let Some((x, _)) = want {
            assert_eq!(quote::reprice_token(c.l.0, c.p.0), Ok(x));
        }
    }
}

#[test]
fn liquidity_matches_sim() {
    let f = load();
    for c in &f.liquidity {
        assert_eq!(
            quote::liquidity(c.x.0, c.y.0).ok(),
            c.l.as_ref().map(|l| l.0),
            "X {} Y {}",
            c.x.0,
            c.y.0
        );
    }
}

#[test]
fn locked_depth_matches_sim() {
    let f = load();
    assert!(f.locked_depth.len() >= 100);
    for (i, c) in f.locked_depth.iter().enumerate() {
        let (counter, supply) = (c.lp_counter.0, c.lp_supply.0);
        assert_eq!(
            pool_select::lambda_bps(counter, supply),
            c.lambda_bps.0,
            "case {i}"
        );
        assert_eq!(
            pool_select::locked_depth(counter, supply, c.x.0, c.y.0),
            Ok(c.locked_depth.0),
            "case {i}"
        );
    }
}

/// AMM v4's `num/den` rate in ppm is `⌈num·10⁶/den⌉` (ARCH §4.2), which the sim's
/// `fee_ppm` is for the 25/10,000 pools of every sampled market.
#[test]
fn v4_rate_in_ppm() {
    let v4: RayV4Fee = fees::ray_v4(25, 10_000).unwrap();
    assert_eq!(v4.oneway_ppm(), 2_500);
    assert_eq!(fees::ray_v4(1, 3).unwrap().oneway_ppm(), 333_334);
}
