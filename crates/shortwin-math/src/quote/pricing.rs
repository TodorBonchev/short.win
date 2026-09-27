//! Venue dispatch for quotes on any state: the live reserves or a reprojection
//! `reprice(L, P)` (ECON §2, §3.1, §5.3; ARCH §4.3).
//!
//! Every risk predicate prices `B̂(Q; reprice(L, P))` on some `(L, P)` (the spot leg, the
//! settlement-box corners, the refutation, FV, `B̂_ref`) or on a ring entry's cached
//! `(x, y) = reprice(l, p)` (the liquidation EMA leg, the D4 margin; ECON §2.3), so the
//! program, the keeper and the WASM facade dispatch the same way: [`Pricing`] for the
//! constant-product pools (PumpSwap, Raydium AMM v4, CPMM), [`CurvePricing`] for the pump
//! curve, whose buyback depends on `rT` (`B̂_str` above it, V1-9) and so needs a
//! [`CurveState`] rather than `(X, Y)` (review F-S3).

use super::{
    BuyQuote, CurveOffsets, CurveState, MigrationParams, SellQuote, cpmm, curve_reprice,
    curve_straddle, ray_v4, reprice, sol_leg,
};
use crate::error::MathError;
use crate::fees::{CpmmFees, FeeBps, PammFeeSchedule, RayV4Fee};
use crate::risk::liquidation_price;

/// A constant-product pool family with its fees already resolved (`fees::*`), at the
/// live fee for execution quotes or [`Pricing::capped`] for risk quotes (ARCH §4.3 "two
/// fee views", V2-1). Reserves are always passed as `(x, y)` = (token, SOL), the order
/// of [`reprice()`]; the Raydium modules' (input, output) order is handled here. The pump
/// curve is [`CurvePricing`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pricing {
    /// PumpSwap: fees on the SOL leg, ε₁ = 0 ([`sol_leg`]).
    PumpSwap(FeeBps),
    /// Raydium AMM v4 ([`ray_v4`]).
    RayV4(RayV4Fee),
    /// Raydium CPMM ([`cpmm`]), with the fees resolved per direction (the creator-fee
    /// side depends on it, `fees::cpmm`).
    Cpmm {
        /// Fees of a buy (SOL in).
        buy: CpmmFees,
        /// Fees of a sale (token in).
        sell: CpmmFees,
    },
}

impl Pricing {
    /// Exact-out buy of `q` tokens on `(x, y)` (ECON §3.1 `B̂`). `None` exactly when the
    /// family's quote is undefined.
    pub fn buy(&self, q: u64, x: u128, y: u128) -> Option<BuyQuote> {
        match self {
            Self::PumpSwap(fees) => sol_leg::buy_exact_out(q, x, y, false, fees),
            Self::RayV4(f) => ray_v4::buy_exact_out(q, y, x, f.num, f.den),
            Self::Cpmm { buy, .. } => cpmm::buy_exact_out(q, y, x, buy),
        }
    }

    /// Exact-in sale of `q` tokens on `(x, y)` (ECON §3.1 `S`). `real_quote_cap` is the
    /// real SOL PumpSwap can pay out (its quote vault); Raydium ignores it (its SOL
    /// reserve is real).
    pub fn sell(&self, q: u64, x: u128, y: u128, real_quote_cap: u64) -> Option<SellQuote> {
        match self {
            Self::PumpSwap(fees) => sol_leg::sell_exact_in(q, x, y, fees, real_quote_cap),
            Self::RayV4(f) => ray_v4::sell_exact_in(q, x, y, f.num, f.den),
            Self::Cpmm { sell, .. } => cpmm::sell_exact_in(q, x, y, sell),
        }
    }

    /// `B̂(q; reprice(L, P))`: the buy on the state with liquidity `L` at price `P`
    /// (ECON §2.1, §5.5; ARCH §4.3 EMA leg). `None` if [`reprice()`] or the quote is.
    /// Non-decreasing in `P` for fixed fees.
    ///
    /// The fees are this `Pricing`'s, i.e. resolved on the **live** state: a hypothetical
    /// state is quoted at the live pump-fees tier, never at the tier of its own market cap
    /// (ECON §3.1, ARCH §4.2, R3B-7 b; the sims' `Market.quote_xy`), which would make `B̂`
    /// dip at each tier threshold as `P` rises.
    pub fn buy_repriced(&self, q: u64, l: u128, p: u128) -> Option<BuyQuote> {
        let (x, y) = reprice(l, p)?;
        self.buy(q, x, y)
    }

    /// The risk view `min(φ_live, cap_bps)` of every fee (ARCH §4.3, V2-1): see
    /// [`FeeBps::capped`] (one combined fee at the cap), [`RayV4Fee::capped`],
    /// [`CpmmFees::capped`] (gross-up at `min(rate, cap)`).
    pub fn capped(&self, cap_bps: u64) -> Self {
        match self {
            Self::PumpSwap(fees) => Self::PumpSwap(fees.capped(cap_bps)),
            Self::RayV4(f) => Self::RayV4(f.capped(cap_bps)),
            Self::Cpmm { buy, sell } => Self::Cpmm {
                buy: buy.capped(cap_bps),
                sell: sell.capped(cap_bps),
            },
        }
    }

    /// One-way fee φ of a buy in ppm (ECON §2.1, ARCH §4.2): the `φ` of the fee-scaled
    /// caps (`caps::{fee_size_cap, fee_oi_cap}`) and of the venue fee gates
    /// ([`fees::exceeds_cap`](crate::fees::exceeds_cap)).
    pub fn oneway_ppm(&self) -> u32 {
        match self {
            Self::PumpSwap(fees) => fees.oneway_ppm(),
            Self::RayV4(f) => f.oneway_ppm(),
            Self::Cpmm { buy, .. } => buy.oneway_ppm(),
        }
    }

    /// One-way fee of a buy in bps, `⌈φ_ppm/100⌉`, for display only.
    pub fn oneway_bps(&self) -> u64 {
        match self {
            Self::PumpSwap(fees) => fees.oneway_bps(),
            Self::RayV4(f) => f.oneway_bps(),
            Self::Cpmm { buy, .. } => buy.oneway_bps(),
        }
    }
}

/// The pump bonding curve as a venue (ECON §3.1, §5.3, §10.2; ARCH §4.3, §5.4): its
/// resolved fees, the constant offsets that make a repriced state know its `rT`, the
/// migration, and the PumpSwap fee schedule of the post-migration pool that `B̂_str`
/// prices on. Build it at the live fee for execution quotes and with
/// [`CurvePricing::capped`] for risk quotes (ARCH §4.3, V2-1).
#[derive(Clone, Copy, Debug)]
pub struct CurvePricing<'a> {
    /// The curve's resolved fees (`fees::pump_curve` on the live state).
    pub fees: FeeBps,
    /// `vT − rT`, `vQ − rQ` of the live curve ([`CurveOffsets::of`]).
    pub offsets: CurveOffsets,
    /// What `migrate_v2` seeds the pool with (EMA leg: the standard 206.9 M tokens).
    pub migration: MigrationParams,
    /// Fee schedule of the post-migration pool (tier resolved at its market cap).
    pub pool: PammFeeSchedule<'a>,
}

impl CurvePricing<'_> {
    /// Buyback of `q` on `state` along the executed path: [`curve_straddle`] (the curve for
    /// `q ≤ rT`, `B̂_str` on the post-migration pool above it). `None` exactly when that
    /// is.
    pub fn buy(&self, q: u64, state: &CurveState) -> Option<BuyQuote> {
        curve_straddle(q, state, &self.fees, &self.migration, &self.pool)
    }

    /// Exact-in sale of `q` on `state` (`sol_leg::sell_exact_in` on `(vT, vQ)`, paid out
    /// of `rQ`). `None` on a complete curve (pump reverts with 6005) or when the sale is.
    pub fn sell(&self, q: u64, state: &CurveState) -> Option<SellQuote> {
        if state.complete {
            return None;
        }
        let cap = u64::try_from(state.rq).unwrap_or(u64::MAX);
        sol_leg::sell_exact_in(q, state.vt, state.vq, &self.fees, cap)
    }

    /// `B̂(q; curve_reprice(L, P))`: the curve at price `P` with liquidity `L` and the
    /// live offsets, straddle included (ECON §5.3; ARCH §4.3 EMA leg, FV, `B̂_ref`). The
    /// curve's fees are the live ones (R3B-7 b); the straddle's pool tier is resolved at
    /// the migrated pool's market cap (ARCH §4.3, as the sim).
    pub fn buy_repriced(&self, q: u64, l: u128, p: u128) -> Option<BuyQuote> {
        self.buy(q, &curve_reprice(l, p, self.offsets)?)
    }

    /// The risk view (ARCH §4.3, V2-1): the curve's fees and the post-migration pool's
    /// capped at `cap_bps` (a lower existing pool cap is kept), each as one combined fee at
    /// the cap when above it ([`FeeBps::capped`]; the sims' `quote_at` with `cap_ppm`).
    pub fn capped(&self, cap_bps: u64) -> Self {
        let mut c = *self;
        c.fees = self.fees.capped(cap_bps);
        c.pool.cap_bps = Some(self.pool.cap_bps.map_or(cap_bps, |k| k.min(cap_bps)));
        c
    }

    /// One-way fee φ of a buy on the curve in ppm (ECON §2.1, ARCH §4.2).
    pub fn oneway_ppm(&self) -> u32 {
        self.fees.oneway_ppm()
    }

    /// One-way fee of a buy on the curve in bps, for display only.
    pub fn oneway_bps(&self) -> u64 {
        self.fees.oneway_bps()
    }

    /// Liquidation price of a curve position (ECON §3.3; ARCH §7.3; review F-A2): the
    /// smallest `P ≥ p_from` with `CR(V_eff, B̂(q; curve_reprice(L, P))) < L_liq`.
    ///
    /// `B̂_str` dips (≈ 0.02 %) where `rT(P)` falls below `q`, so one bisection over all
    /// `P` can miss the first crossing. This searches the curve regime (`rT(P) ≥ q`, the
    /// curve quote, non-decreasing in `P`) first and the pool regime above it only if the
    /// curve regime never crosses. In the pool regime `B̂_str` is flat up to the rounding
    /// of `Y_p` (a few lamports) until `rT(P) = 0` and increasing after, so the result
    /// there is exact up to that rounding. Off-chain only (≤ 260 quotes).
    ///
    /// Errors: as [`liquidation_price`].
    pub fn liquidation_price(
        &self,
        q: u64,
        v_eff: u64,
        cr_liq_bps: u32,
        l: u128,
        p_from: u128,
    ) -> Result<Option<u128>, MathError> {
        let quote = |p: u128| self.buy_repriced(q, l, p).map(|b| b.cost);
        let in_pool = |p: u128| curve_reprice(l, p, self.offsets).map(|s| s.rt < u128::from(q));
        // Smallest P ≥ p_from in the pool regime (`rT(P)` is non-increasing in P).
        let split = match (in_pool(p_from), in_pool(u128::MAX)) {
            (Some(false), Some(true)) => {
                let (mut lo, mut hi) = (p_from, u128::MAX);
                while lo.checked_add(1).is_some_and(|next| next < hi) {
                    let mid = lo.midpoint(hi);
                    if in_pool(mid) == Some(true) {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                Some(hi)
            }
            _ => None,
        };
        let Some(split) = split else {
            return liquidation_price(p_from, v_eff, cr_liq_bps, quote);
        };
        // Curve regime [p_from, split − 1]: held constant above it, so the bisection finds
        // the first crossing below `split` if there is one.
        let last = split.saturating_sub(1);
        if let Some(p) = liquidation_price(p_from, v_eff, cr_liq_bps, |p| quote(p.min(last)))? {
            return Ok(Some(p));
        }
        liquidation_price(split, v_eff, cr_liq_bps, quote)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::fees::{self, CpmmConfig, CpmmPoolFees, Direction};

    #[test]
    fn dispatch_matches_the_venue_modules() {
        let (x, y) = (37_639_277_914_829_371u128, 17_595_123_332_536u128);
        let v4 = Pricing::RayV4(fees::ray_v4(25, 10_000).unwrap());
        // R-RAY §3.5 (A): the (input, output) order is swapped internally.
        let b = v4.buy(235_932_179_000, x, y).unwrap();
        assert_eq!((b.net, b.cost), (110_291_219, 110_567_639));
        assert_eq!(
            v4.sell(1_000, x, y, 0),
            ray_v4::sell_exact_in(1_000, x, y, 25, 10_000)
        );

        let cfg = CpmmConfig {
            trade_fee_rate: 2_500,
            protocol_fee_rate: 120_000,
            fund_fee_rate: 40_000,
            creator_fee_rate: 500,
        };
        let pool = CpmmPoolFees {
            enable_creator_fee: true,
            creator_fee_on: 1,
            sol_is_token0: true,
        };
        let cp = Pricing::Cpmm {
            buy: fees::cpmm(&cfg, &pool, Direction::Buy).unwrap(),
            sell: fees::cpmm(&cfg, &pool, Direction::Sell).unwrap(),
        };
        let Pricing::Cpmm { buy, sell } = cp else {
            unreachable!()
        };
        assert_eq!(
            cp.buy(1_000_000, x, y),
            cpmm::buy_exact_out(1_000_000, y, x, &buy)
        );
        assert_eq!(
            cp.sell(1_000_000, x, y, 0),
            cpmm::sell_exact_in(1_000_000, x, y, &sell)
        );
        assert_eq!((cp.oneway_bps(), cp.oneway_ppm()), (30, 3_000));

        let (vt, vq) = (482_553_257_944_361u128, 18_917_931_618u128 + 17_584_505_333);
        let pump = Pricing::PumpSwap(FeeBps::new(2, 93, 30));
        assert_eq!(
            pump.buy(9_155_477_586_733, vt, vq).unwrap().cost,
            714_778_826
        );
        assert_eq!(pump.capped(100).oneway_bps(), 100);
        assert_eq!(pump.capped(100).oneway_ppm(), 10_000);
        assert_eq!(pump.oneway_ppm(), 12_500);
        assert_eq!(v4.capped(10).oneway_bps(), 10);
        assert_eq!(v4.capped(10).oneway_ppm(), 1_000);
    }

    /// Review F-S3: a curve buyback above `rT` is `B̂_str` (ARCH §4.3), never the plain
    /// curve formula, which `Pricing` can no longer express.
    #[test]
    fn curve_pricing_is_the_straddle() {
        use crate::fees::{FeeTier, PammFeeSchedule};
        use crate::quote::{CurveOffsets, CurveState, MigrationParams, curve_reprice};
        let s = CurveState {
            vt: 400_000_000_000_000,
            vq: 80_475_000_000,
            rt: 120_100_000_000_000,
            rq: 50_475_000_000,
            complete: false,
        };
        let tiers = [
            FeeTier {
                threshold: 0,
                fees: FeeBps::new(2, 93, 30),
            },
            FeeTier {
                threshold: 420_000_000_000,
                fees: FeeBps::new(20, 5, 95),
            },
        ];
        let c = CurvePricing {
            fees: FeeBps::new(0, 95, 30),
            offsets: CurveOffsets::of(&s).unwrap(),
            migration: MigrationParams {
                migration_fee: 15_000_001,
                pool_tokens: 206_900_000_000_000,
            },
            pool: PammFeeSchedule {
                tiers: &tiers,
                base_supply: 1_000_000_000_000_000,
                creator_fee_configurable: true,
                pool_creator_fee_bps: 0,
                coin_creator_is_default: false,
                cap_bps: None,
            },
        };
        let q = 150_000_000_000_000;
        let b = c.buy(q, &s).unwrap();
        assert_eq!(
            Some(b),
            curve_straddle(q, &s, &c.fees, &c.migration, &c.pool)
        );
        // Several times the plain curve formula on (vT, vQ) (48,888,562,504 lamports).
        let plain = sol_leg::buy_exact_out(q, s.vt, s.vq, true, &c.fees).unwrap();
        assert!(b.cost > 4 * plain.cost, "{} vs {}", b.cost, plain.cost);
        // Repriced: the straddle on curve_reprice; capped: both fee layers capped.
        let (l, p) = (
            crate::quote::liquidity(s.vt, s.vq).unwrap(),
            s.vq * crate::num::WAD / s.vt,
        );
        let ema = curve_reprice(l, p, c.offsets).unwrap();
        assert!(u128::from(q) > ema.rt);
        assert_eq!(c.buy_repriced(q, l, p), c.buy(q, &ema));
        let risk = c.capped(100);
        assert_eq!((risk.oneway_bps(), risk.pool.cap_bps), (100, Some(100)));
        assert_eq!((c.oneway_ppm(), risk.oneway_ppm()), (12_500, 10_000));
        assert_eq!(risk.fees, FeeBps::new(0, 100, 0));
        assert_eq!(risk.capped(500).pool.cap_bps, Some(100));
        assert!(risk.buy(q, &s).unwrap().cost < b.cost);
        // Sales pay out of rQ; a complete curve does not trade.
        assert!(c.sell(1_000_000, &s).is_some());
        let done = CurveState {
            complete: true,
            ..s
        };
        assert_eq!(c.sell(1_000_000, &done), None);
        assert_eq!(c.buy(1, &done), None);
    }

    #[test]
    fn repriced_buy_is_the_buy_on_reprice() {
        let f = Pricing::PumpSwap(FeeBps::new(20, 5, 95));
        let (l, p) = (5_000_000_000_000u128, 70_000_000_000_000u128);
        let (x, y) = reprice(l, p).unwrap();
        assert_eq!(
            f.buy_repriced(1_000_000_000, l, p),
            f.buy(1_000_000_000, x, y)
        );
        assert_eq!(f.buy_repriced(1, 0, p), None);
    }
}
