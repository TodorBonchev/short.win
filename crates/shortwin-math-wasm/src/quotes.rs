//! Quotes, fees and the venue a market trades on (`shortwin_math::{quote, fees}`;
//! ECON §2, §3.1; ARCH §4.2, §4.3), plus the liquidation price (`risk::liquidation_price`).

use shortwin_math::MathError;
use shortwin_math::fees::{
    self, CpmmConfig, CpmmPoolFees, CurveFeeSchedule, Direction, FeeBps, FeeTier, PammFeeSchedule,
};
use shortwin_math::quote::{self, CurveOffsets, CurvePricing, MigrationParams, Pricing};
use shortwin_math::risk as mrisk;
use wasm_bindgen::JsError;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{bps_number, or_throw};

/// Fee components of a pump venue in bps (`fees::FeeBps`): charged on the SOL leg, each
/// rounded up separately.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fees(pub(crate) FeeBps);

#[wasm_bindgen]
impl Fees {
    /// `(lp, protocol, creator)` in bps.
    #[wasm_bindgen(constructor)]
    pub fn new(lp: u32, protocol: u32, creator: u32) -> Fees {
        Fees(FeeBps::new(lp.into(), protocol.into(), creator.into()))
    }

    /// LP fee, bps.
    #[wasm_bindgen(getter)]
    pub fn lp(&self) -> u32 {
        bps_number(self.0.lp)
    }

    /// Protocol fee, bps.
    #[wasm_bindgen(getter)]
    pub fn protocol(&self) -> u32 {
        bps_number(self.0.protocol)
    }

    /// Creator fee, bps.
    #[wasm_bindgen(getter)]
    pub fn creator(&self) -> u32 {
        bps_number(self.0.creator)
    }

    /// One-way fee φ in bps, rounded up (display only; gates compare ppm).
    #[wasm_bindgen(js_name = onewayBps)]
    pub fn oneway_bps(&self) -> u32 {
        bps_number(self.0.oneway_bps())
    }

    /// One-way fee φ in ppm, the value every fee gate compares (ECON §2.1; ARCH §4.2).
    #[wasm_bindgen(js_name = onewayPpm)]
    pub fn oneway_ppm(&self) -> u32 {
        self.0.oneway_ppm()
    }

    /// The risk view `min(φ, capBps)` (ARCH §4.3, V2-1).
    pub fn capped(&self, cap_bps: u32) -> Fees {
        Fees(self.0.capped(cap_bps.into()))
    }
}

/// pump-fees tiers in account order, as `FeeTier`s.
#[derive(Clone, Debug, Default)]
struct Tiers(Vec<FeeTier>);

impl Tiers {
    fn add(&mut self, threshold: u128, lp: u32, protocol: u32, creator: u32) {
        self.0.push(FeeTier {
            threshold,
            fees: FeeBps::new(lp.into(), protocol.into(), creator.into()),
        });
    }
}

/// Fee inputs of a pump-curve coin (`fees::CurveFeeSchedule`; ARCH §4.1, §4.2).
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct CurveSchedule {
    tiers: Tiers,
    creator_fee_configurable: bool,
    curve_creator_fee_bps: u64,
    creator_is_default: bool,
}

#[wasm_bindgen]
impl CurveSchedule {
    /// `Global.creator_fee_configurable`, `BondingCurve.creator_fee_bps`, and whether the
    /// curve's creator is the default key. Add the `FeeConfig` tiers with `addTier`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        creator_fee_configurable: bool,
        curve_creator_fee_bps: u32,
        creator_is_default: bool,
    ) -> CurveSchedule {
        CurveSchedule {
            tiers: Tiers::default(),
            creator_fee_configurable,
            curve_creator_fee_bps: curve_creator_fee_bps.into(),
            creator_is_default,
        }
    }

    /// Appends a `FeeConfig` tier (account order).
    #[wasm_bindgen(js_name = addTier)]
    pub fn add_tier(&mut self, threshold: u128, lp: u32, protocol: u32, creator: u32) {
        self.tiers.add(threshold, lp, protocol, creator);
    }

    /// The live fees of a trade on the curve state `(vT, vQ)` (`fees::pump_curve`).
    pub fn resolve(&self, vt: u128, vq: u128) -> Result<Fees, JsError> {
        let s = CurveFeeSchedule {
            tiers: &self.tiers.0,
            creator_fee_configurable: self.creator_fee_configurable,
            curve_creator_fee_bps: self.curve_creator_fee_bps,
            creator_is_default: self.creator_is_default,
            cap_bps: None,
        };
        Ok(Fees(fees::pump_curve(vt, vq, &s)?))
    }
}

/// Fee inputs of a canonical PumpSwap pool (`fees::PammFeeSchedule`; ARCH §4.1, §4.2).
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct PammSchedule {
    tiers: Tiers,
    base_supply: u64,
    creator_fee_configurable: bool,
    pool_creator_fee_bps: u64,
    coin_creator_is_default: bool,
    cap_bps: Option<u64>,
}

impl PammSchedule {
    fn schedule(&self) -> PammFeeSchedule<'_> {
        PammFeeSchedule {
            tiers: &self.tiers.0,
            base_supply: self.base_supply,
            creator_fee_configurable: self.creator_fee_configurable,
            pool_creator_fee_bps: self.pool_creator_fee_bps,
            coin_creator_is_default: self.coin_creator_is_default,
            cap_bps: self.cap_bps,
        }
    }
}

#[wasm_bindgen]
impl PammSchedule {
    /// Base-mint supply (10¹⁵ before graduation), `GlobalConfig.creator_fee_configurable`,
    /// `Pool.creator_fee_bps`, and whether the coin creator is the default key. Add the
    /// `FeeConfig` tiers with `addTier`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        base_supply: u64,
        creator_fee_configurable: bool,
        pool_creator_fee_bps: u32,
        coin_creator_is_default: bool,
    ) -> PammSchedule {
        PammSchedule {
            tiers: Tiers::default(),
            base_supply,
            creator_fee_configurable,
            pool_creator_fee_bps: pool_creator_fee_bps.into(),
            coin_creator_is_default,
            cap_bps: None,
        }
    }

    /// Appends a `FeeConfig` tier (account order).
    #[wasm_bindgen(js_name = addTier)]
    pub fn add_tier(&mut self, threshold: u128, lp: u32, protocol: u32, creator: u32) {
        self.tiers.add(threshold, lp, protocol, creator);
    }

    /// The live fees of a trade on `(effective quote, base reserve)` (`fees::pump_amm`).
    pub fn resolve(&self, effective_quote: u128, base_reserve: u128) -> Result<Fees, JsError> {
        let mut s = self.schedule();
        s.cap_bps = None;
        Ok(Fees(fees::pump_amm(effective_quote, base_reserve, &s)?))
    }
}

/// An exact-out buy (`quote::BuyQuote`).
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuyQuote(pub(crate) quote::BuyQuote);

#[wasm_bindgen]
impl BuyQuote {
    /// `B̂`: lamports the venue debits, fees included.
    #[wasm_bindgen(getter)]
    pub fn cost(&self) -> u64 {
        self.0.cost
    }

    /// Pre-fee SOL input `n` (σ uses it).
    #[wasm_bindgen(getter)]
    pub fn net(&self) -> u64 {
        self.0.net
    }

    /// Token pricing reserve after the trade.
    #[wasm_bindgen(getter, js_name = tokenAfter)]
    pub fn token_after(&self) -> u128 {
        self.0.token_after
    }

    /// SOL pricing reserve after the trade.
    #[wasm_bindgen(getter, js_name = solAfter)]
    pub fn sol_after(&self) -> u128 {
        self.0.sol_after
    }
}

/// An exact-in sale (`quote::SellQuote`).
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SellQuote(pub(crate) quote::SellQuote);

#[wasm_bindgen]
impl SellQuote {
    /// `S`: lamports the seller receives, fees deducted.
    #[wasm_bindgen(getter)]
    pub fn proceeds(&self) -> u64 {
        self.0.proceeds
    }

    /// Token pricing reserve after the trade.
    #[wasm_bindgen(getter, js_name = tokenAfter)]
    pub fn token_after(&self) -> u128 {
        self.0.token_after
    }

    /// SOL pricing reserve after the trade.
    #[wasm_bindgen(getter, js_name = solAfter)]
    pub fn sol_after(&self) -> u128 {
        self.0.sol_after
    }
}

/// A constant-product state `(x, y)` = (token, SOL).
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reserves {
    x: u128,
    y: u128,
}

#[wasm_bindgen]
impl Reserves {
    /// Token reserve.
    #[wasm_bindgen(getter)]
    pub fn x(&self) -> u128 {
        self.x
    }

    /// SOL reserve.
    #[wasm_bindgen(getter)]
    pub fn y(&self) -> u128 {
        self.y
    }
}

/// `reprice(L, P)`: the state with liquidity `L` at price `P` (WAD) (ECON §2).
#[wasm_bindgen]
pub fn reprice(l: u128, p: u128) -> Option<Reserves> {
    quote::reprice(l, p).map(|(x, y)| Reserves { x, y })
}

/// The fee gate `φ > cap` compared exactly in ppm (`fees::exceeds_cap`; ECON §2.1): opens
/// above `venue_fee_cap_bps`, S3 evidence above `venue_fee_hard_cap_bps`.
#[wasm_bindgen(js_name = exceedsCap)]
pub fn exceeds_cap(phi_ppm: u32, cap_bps: u32) -> bool {
    fees::exceeds_cap(phi_ppm, cap_bps.into())
}

/// Liquidity `L = isqrt(X·Y)` (ECON §2.1); throws `overflow` if `X·Y ≥ 2¹²⁸`.
#[wasm_bindgen]
pub fn liquidity(x: u128, y: u128) -> u128 {
    crate::or_throw(quote::liquidity(x, y))
}

/// Marginal price `P = ⌊Y·WAD/X⌋` (ECON §2).
#[wasm_bindgen(js_name = priceWad)]
pub fn price_wad(x: u128, y: u128) -> Result<u128, JsError> {
    Ok(mrisk::price_wad(x, y)?)
}

/// A pump bonding-curve state (`quote::CurveState`).
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurveState(pub(crate) quote::CurveState);

#[wasm_bindgen]
impl CurveState {
    /// `(vT, vQ, rT, rQ, complete)` from the `BondingCurve` account.
    #[wasm_bindgen(constructor)]
    pub fn new(vt: u128, vq: u128, rt: u128, rq: u128, complete: bool) -> CurveState {
        CurveState(quote::CurveState {
            vt,
            vq,
            rt,
            rq,
            complete,
        })
    }

    /// `curve_reprice(L, P, offsets)` (ARCH §4.3 EMA leg): the curve at price `P` with
    /// liquidity `L` and the offsets `vT − rT`, `vQ − rQ`.
    pub fn reprice(l: u128, p: u128, offset_token: u128, offset_quote: u128) -> Option<CurveState> {
        let offset = CurveOffsets {
            token: offset_token,
            quote: offset_quote,
        };
        quote::curve_reprice(l, p, offset).map(CurveState)
    }

    /// `virtual_token_reserves`.
    #[wasm_bindgen(getter)]
    pub fn vt(&self) -> u128 {
        self.0.vt
    }

    /// `virtual_quote_reserves`.
    #[wasm_bindgen(getter)]
    pub fn vq(&self) -> u128 {
        self.0.vq
    }

    /// `real_token_reserves`.
    #[wasm_bindgen(getter)]
    pub fn rt(&self) -> u128 {
        self.0.rt
    }

    /// `real_quote_reserves`.
    #[wasm_bindgen(getter)]
    pub fn rq(&self) -> u128 {
        self.0.rq
    }

    /// `complete`.
    #[wasm_bindgen(getter)]
    pub fn complete(&self) -> bool {
        self.0.complete
    }

    /// The canonical pool right after migration, `(X_p, Y_p)` (`quote::migrated_pool`).
    #[wasm_bindgen(js_name = migratedPool)]
    pub fn migrated_pool(&self, migration_fee: u64, pool_tokens: u128) -> Option<Reserves> {
        let m = MigrationParams {
            migration_fee,
            pool_tokens,
        };
        quote::migrated_pool(&self.0, &m).map(|(x, y)| Reserves { x, y })
    }
}

/// The pump curve as a venue: its own fees, the offsets for repricing, and the
/// post-migration pool of the straddle (ARCH §4.3, V1-9); an owned `quote::CurvePricing`.
#[derive(Clone, Debug)]
struct CurveVenue {
    fees: FeeBps,
    offsets: CurveOffsets,
    migration: MigrationParams,
    pool: PammSchedule,
}

impl CurveVenue {
    fn pricing(&self) -> CurvePricing<'_> {
        CurvePricing {
            fees: self.fees,
            offsets: self.offsets,
            migration: self.migration,
            pool: self.pool.schedule(),
        }
    }
}

#[derive(Clone, Debug)]
enum Kind {
    Cp(Pricing),
    Curve(Box<CurveVenue>),
}

/// The venue a market trades on, with resolved fees: PumpSwap, Raydium AMM v4 or CPMM
/// (`quote::Pricing`), or the pump curve (straddle-aware). Build the risk view with
/// `capped`.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct Venue(Kind);

#[wasm_bindgen]
impl Venue {
    /// A PumpSwap pool with resolved fees (`PammSchedule.resolve`).
    #[wasm_bindgen(js_name = pumpSwap)]
    pub fn pump_swap(fees: &Fees) -> Venue {
        Venue(Kind::Cp(Pricing::PumpSwap(fees.0)))
    }

    /// A Raydium AMM v4 pool (`swap_fee_numerator`, `swap_fee_denominator`).
    #[wasm_bindgen(js_name = rayV4)]
    pub fn ray_v4(num: u64, den: u64) -> Result<Venue, JsError> {
        Ok(Venue(Kind::Cp(Pricing::RayV4(fees::ray_v4(num, den)?))))
    }

    /// A Raydium CPMM pool: `AmmConfig` rates (ppm) and the pool's creator-fee fields.
    #[allow(clippy::too_many_arguments)]
    pub fn cpmm(
        trade_fee_rate: u32,
        protocol_fee_rate: u32,
        fund_fee_rate: u32,
        creator_fee_rate: u32,
        enable_creator_fee: bool,
        creator_fee_on: u8,
        sol_is_token0: bool,
    ) -> Result<Venue, JsError> {
        let config = CpmmConfig {
            trade_fee_rate: trade_fee_rate.into(),
            protocol_fee_rate: protocol_fee_rate.into(),
            fund_fee_rate: fund_fee_rate.into(),
            creator_fee_rate: creator_fee_rate.into(),
        };
        let pool = CpmmPoolFees {
            enable_creator_fee,
            creator_fee_on,
            sol_is_token0,
        };
        Ok(Venue(Kind::Cp(Pricing::Cpmm {
            buy: fees::cpmm(&config, &pool, Direction::Buy)?,
            sell: fees::cpmm(&config, &pool, Direction::Sell)?,
        })))
    }

    /// The pump curve: its resolved fees, the offsets `vT − rT` and `vQ − rQ`, the
    /// migration (`pool_migration_fee`, tokens seeded into the pool) and the PumpSwap fee
    /// schedule of the post-migration pool.
    #[wasm_bindgen(js_name = pumpCurve)]
    pub fn pump_curve(
        fees: &Fees,
        offset_token: u128,
        offset_quote: u128,
        migration_fee: u64,
        pool_tokens: u128,
        pool: &PammSchedule,
    ) -> Venue {
        Venue(Kind::Curve(Box::new(CurveVenue {
            fees: fees.0,
            offsets: CurveOffsets {
                token: offset_token,
                quote: offset_quote,
            },
            migration: MigrationParams {
                migration_fee,
                pool_tokens,
            },
            pool: pool.clone(),
        })))
    }

    /// The risk view: every fee at `min(φ_live, capBps)` (ARCH §4.3, V2-1), for CR, the
    /// liquidation price and FV.
    pub fn capped(&self, cap_bps: u32) -> Venue {
        let cap = u64::from(cap_bps);
        Venue(match &self.0 {
            Kind::Cp(p) => Kind::Cp(p.capped(cap)),
            Kind::Curve(c) => {
                let capped = c.pricing().capped(cap);
                let mut c = c.clone();
                c.fees = capped.fees;
                c.pool.cap_bps = capped.pool.cap_bps;
                Kind::Curve(c)
            }
        })
    }

    /// One-way fee φ of a buy in bps, rounded up (the curve's own fee on a curve; display
    /// only).
    #[wasm_bindgen(js_name = onewayBps)]
    pub fn oneway_bps(&self) -> u32 {
        bps_number(match &self.0 {
            Kind::Cp(p) => p.oneway_bps(),
            Kind::Curve(c) => c.fees.oneway_bps(),
        })
    }

    /// One-way fee φ of a buy in ppm (ECON §2.1): what `venue_fee_cap_bps` and
    /// `venue_fee_hard_cap_bps` are compared with, as `cap·100` (`exceedsCap`).
    #[wasm_bindgen(js_name = onewayPpm)]
    pub fn oneway_ppm(&self) -> u32 {
        match &self.0 {
            Kind::Cp(p) => p.oneway_ppm(),
            Kind::Curve(c) => c.fees.oneway_ppm(),
        }
    }

    /// Exact-out buy of `q` on the pool reserves `(x, y)` = (token, SOL). A curve is
    /// quoted with `buyCurve`.
    pub fn buy(&self, q: u64, x: u128, y: u128) -> Result<Option<BuyQuote>, JsError> {
        Ok(self.pricing()?.buy(q, x, y).map(BuyQuote))
    }

    /// Exact-in sale of `q` on the pool reserves `(x, y)`; `realQuoteCap` is PumpSwap's
    /// quote vault (Raydium ignores it). A curve is quoted with `sellCurve`.
    pub fn sell(
        &self,
        q: u64,
        x: u128,
        y: u128,
        real_quote_cap: u64,
    ) -> Result<Option<SellQuote>, JsError> {
        Ok(self.pricing()?.sell(q, x, y, real_quote_cap).map(SellQuote))
    }

    /// Buyback of `q` on a curve state along the executed path (`CurvePricing::buy`,
    /// `quote::curve_straddle`): the curve for `q ≤ rT`, all of `q` on the post-migration
    /// pool above it.
    #[wasm_bindgen(js_name = buyCurve)]
    pub fn buy_curve(&self, q: u64, state: &CurveState) -> Result<Option<BuyQuote>, JsError> {
        Ok(self.curve()?.pricing().buy(q, &state.0).map(BuyQuote))
    }

    /// Sale of `q` on a curve state (`CurvePricing::sell`: `sol_leg::sell_exact_in` on
    /// `(vT, vQ)`, paid out of `rQ`; `undefined` on a complete curve).
    #[wasm_bindgen(js_name = sellCurve)]
    pub fn sell_curve(&self, q: u64, state: &CurveState) -> Result<Option<SellQuote>, JsError> {
        Ok(self.curve()?.pricing().sell(q, &state.0).map(SellQuote))
    }

    /// `B̂(q; reprice(L, P))`: the buy on the state repriced to `(L, P)` (on a curve,
    /// the straddle on `curve_reprice`) — the EMA leg, FV and the reference quote.
    #[wasm_bindgen(js_name = buyRepriced)]
    pub fn buy_repriced(&self, q: u64, l: u128, p: u128) -> Option<BuyQuote> {
        self.repriced(q, l, p).map(BuyQuote)
    }

    /// Liquidation price (ECON §3.3; `risk::liquidation_price`, on a curve
    /// `CurvePricing::liquidation_price`): the smallest `P ≥ pFrom` at which
    /// `CR(vEff, B̂(q; reprice(L, P))) < crLiqBps`. Use the capped venue and `L_live` for
    /// the spot leg, `L_ema` for the EMA leg. `undefined` if no price is.
    #[wasm_bindgen(js_name = liquidationPrice)]
    pub fn liquidation_price(
        &self,
        q: u64,
        v_eff: u64,
        cr_liq_bps: u32,
        l: u128,
        p_from: u128,
    ) -> Option<u128> {
        or_throw(match &self.0 {
            Kind::Cp(pricing) => mrisk::liquidation_price(p_from, v_eff, cr_liq_bps, |p| {
                pricing.buy_repriced(q, l, p).map(|b| b.cost)
            }),
            Kind::Curve(c) => c
                .pricing()
                .liquidation_price(q, v_eff, cr_liq_bps, l, p_from),
        })
    }
}

impl Venue {
    fn pricing(&self) -> Result<&Pricing, MathError> {
        match &self.0 {
            Kind::Cp(p) => Ok(p),
            Kind::Curve(_) => Err(MathError::InvalidInput),
        }
    }

    fn curve(&self) -> Result<&CurveVenue, MathError> {
        match &self.0 {
            Kind::Curve(c) => Ok(c),
            Kind::Cp(_) => Err(MathError::InvalidInput),
        }
    }

    fn repriced(&self, q: u64, l: u128, p: u128) -> Option<quote::BuyQuote> {
        match &self.0 {
            Kind::Cp(pricing) => pricing.buy_repriced(q, l, p),
            Kind::Curve(c) => c.pricing().buy_repriced(q, l, p),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const WAD: u128 = shortwin_math::num::WAD;

    #[test]
    fn pump_curve_worked_example() {
        // R-PUMP §4.2 / ECON §3.2: the program charges 485,764,634 lamports for 10¹³ raw;
        // short.win's quote rounds up to 485,764,635. The sale pays 462,410,820.
        let mut pamm = PammSchedule::new(1_000_000_000_000_000, true, 0, false);
        pamm.add_tier(0, 2, 93, 30);
        let v = Venue::pump_curve(
            &Fees::new(0, 95, 30),
            279_900_000_000_000,
            30_000_000_000,
            15_000_001,
            206_900_000_000_000,
            &pamm,
        );
        let s = CurveState::new(
            824_130_602_106_172,
            39_059_343_535,
            544_230_602_106_172,
            9_059_343_535,
            false,
        );
        let q = 10_000_000_000_000;
        assert_eq!(v.buy_curve(q, &s).unwrap().unwrap().cost(), 485_764_635);
        assert_eq!(
            v.sell_curve(q, &s).unwrap().unwrap().proceeds(),
            462_410_820
        );
        assert_eq!((v.oneway_bps(), v.oneway_ppm()), (125, 12_500));
        assert!(exceeds_cap(v.oneway_ppm(), 124) && !exceeds_cap(v.oneway_ppm(), 125));
        assert_eq!(v.capped(100).oneway_bps(), 100);
        // The repriced curve at the live (L, P) is within rounding of the live quote.
        let l = liquidity(s.vt(), s.vq());
        let p = price_wad(s.vt(), s.vq()).unwrap();
        let r = v.buy_repriced(q, l, p).unwrap().cost();
        assert!(r.abs_diff(485_764_635) < 100_000);
        let p_liq = v.liquidation_price(q, 2 * r, 12_500, l, p).unwrap();
        assert!(p_liq > p && p_liq < 2 * p);
    }

    #[test]
    fn cp_venues() {
        let v4 = Venue::ray_v4(25, 10_000).unwrap();
        // R-RAY §3.5 (A), with (token, SOL) reserves.
        let b = v4
            .buy(235_932_179_000, 37_639_277_914_829_371, 17_595_123_332_536)
            .unwrap()
            .unwrap();
        assert_eq!((b.net(), b.cost()), (110_291_219, 110_567_639));
        let cp = Venue::cpmm(2_500, 120_000, 40_000, 0, false, 0, true).unwrap();
        assert_eq!(cp.oneway_bps(), 25);
        let ps = Venue::pump_swap(&Fees::new(2, 93, 30));
        let y = 18_917_931_618u128 + 17_584_505_333;
        let b = ps
            .buy(9_155_477_586_733, 482_553_257_944_361, y)
            .unwrap()
            .unwrap();
        assert_eq!((b.net(), b.cost()), (705_954_395, 714_778_826));
        assert!(ps.buy_repriced(1, 0, WAD).is_none());
        assert_eq!(
            reprice(1, 1),
            Some(Reserves {
                x: 1_000_000_000,
                y: 1
            })
        );
    }

    #[test]
    fn fee_schedules_resolve() {
        let mut c = CurveSchedule::new(true, 0, false);
        c.add_tier(0, 0, 95, 30);
        let f = c.resolve(824_130_602_106_172, 39_059_343_535).unwrap();
        assert_eq!((f.lp(), f.protocol(), f.creator()), (0, 95, 30));
        let mut p = PammSchedule::new(1_000_000_000_000_000, true, 0, false);
        p.add_tier(0, 2, 93, 30);
        p.add_tier(420_000_000_000, 20, 5, 95);
        // mcap ⌊Y·supply/X⌋: 410.8 SOL (tier 0), then 483.3 SOL (tier 1).
        let f = p.resolve(84_990_359_345, 206_900_000_000_000).unwrap();
        assert_eq!((f.lp(), f.protocol(), f.creator()), (2, 93, 30));
        let f = p.resolve(100_000_000_000, 206_900_000_000_000).unwrap();
        assert_eq!((f.lp(), f.protocol(), f.creator()), (20, 5, 95));
    }
}
