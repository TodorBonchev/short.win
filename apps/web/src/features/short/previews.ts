/**
 * SHORT track — the borrower's previews (FRONTEND.md §3.7, §3.8, §6.7): the open preview (`S_exp`,
 * the D4 margin, the open fee, the opening CRs, the liquidation price, rent, the deadline, the tier
 * terms locked at open, every size cap and the refusal the program would return), the live metrics
 * of an open position (CR on both legs, liquidation price, rent due, P&L if closed now, time
 * left, risk) and the close and add-margin previews.
 *
 * Every protocol number is an export of the WASM facade (`@shortwin/math-wasm`) on the accounts as
 * read from the chain — the composition follows the program's handlers step by step
 * (`instructions/open_short.rs`, `close_short.rs`, `add_margin.rs`, `engine/{guard,caps,tier}.rs`):
 * which reserves, which fee view and which stored state each step reads. `previews.test.ts` holds
 * the results to the program itself (`vectors/program-vectors.json`: the test-venue build on
 * LiteSVM, every account dumped in the same second as the instruction).
 *
 * The static open guards (flags and timers) and the boost gate, which the facade does not export,
 * are written out here as the program states them; they gate, they do not price. The price guards
 * are the facade's (`spotNearEma`, `pOpenMax`; FE-15).
 */
import {
    type GlobalConfig,
    type Market,
    type OracleCache,
    podU128ToBigInt,
    type ShortPosition,
    type TierParams,
    type VenueGuard,
} from '@shortwin/client';
import type { MathModule } from '../../math/load.ts';
import {
    CONFIG_FLAG_PAUSED_OPENS,
    COVERAGE_CLEAR_DELAY_S,
    GUARD_MASK,
    type KindName,
    MARKET_FLAG,
    PAMM_CREATE_POOL_DISABLED_BIT,
    VENUE_BIT,
} from '../../venues/constants.ts';
import {
    afterSale,
    type Buy,
    capFeePpm,
    exitRiskCapBps,
    liquidityOf,
    quoteBuy,
    quoteBuyAt,
    quoteSell,
    spotPrice,
    type VenueState,
    withVenue,
} from '../../venues/state.ts';

export const WAD = 10n ** 18n;
const U64_MAX = (1n << 64n) - 1n;
/** FE-D6: the default margin / open-fee tolerance (+2 %). */
export const MARGIN_TOLERANCE_BPS = 200n;
/** FE-D6: the default open / close slippage (1 %), and its allowed range [0.1 %, 50 %]. */
export const DEFAULT_SLIPPAGE_BPS = 100;
export const MIN_SLIPPAGE_BPS = 10;
export const MAX_SLIPPAGE_BPS = 5_000;
/** §5.10: add margin keeps this much SOL in the wallet for fees. */
export const FEE_RESERVE_LAMPORTS = 10_000_000n;

export type TierIndex = 0 | 1 | 2 | 3;
export const TIER_NAMES = ['A', 'B', 'C', 'D'] as const;

/** Everything the previews read for one market at one slot (FE-D1: from the chain). */
export interface MarketSnapshot {
    /** The cluster's `Clock::unix_timestamp` at the read. */
    readonly now: bigint;
    readonly slot: bigint;
    readonly config: GlobalConfig;
    readonly guard: VenueGuard | null;
    readonly oracle: OracleCache | null;
    readonly market: Market;
    readonly kind: KindName;
    readonly venue: VenueState;
    readonly decimals: number;
}

const p128 = podU128ToBigInt;
const ceilDiv = (a: bigint, b: bigint) => (a + b - 1n) / b;

/** `⌈x·(10⁴ + bps)/10⁴⌉`: an upper bound with a tolerance. */
export function withTolerance(x: bigint, bps: bigint): bigint {
    return ceilDiv(x * (10_000n + bps), 10_000n);
}

/** `⌊x·(10⁴ − bps)/10⁴⌋`: a lower bound with a slippage. */
export function lessSlippage(x: bigint, bps: bigint): bigint {
    return (x * (10_000n - bps)) / 10_000n;
}

// ---- market state ---------------------------------------------------------------------------------

export function marketPrice(s: MarketSnapshot): { pEma: bigint; lEma: bigint; lSlow: bigint } {
    return { pEma: p128(s.market.emaPrice), lEma: p128(s.market.emaLiq), lSlow: p128(s.market.liqSlow) };
}

/** `Market::newest_ring_xy`: the newest ring entry's cached `(x, y)`, `(0, 0)` for an empty ring. */
export function newestRingXy(m: Market): readonly [bigint, bigint] {
    if (m.ringCount === 0) return [0n, 0n];
    const e = m.ring[m.ringHead];
    return e ? [p128(e.x), p128(e.y)] : [0n, 0n];
}

/** The rent basis now: `Market.rent_basis` accrued to `now` at `P_ema` and the current utilisation. */
export function rentBasisNow(math: MathModule, s: MarketSnapshot) {
    const [j0, j1, j2] = s.market.rentBasis.map(p128) as [bigint, bigint, bigint];
    const basis = new math.RentBasis(j0, j1, j2);
    const dt = s.now - s.market.lastAccrualTs;
    if (dt <= 0n) return basis;
    try {
        const util = math.utilizationBps(s.market.tokensFree, s.market.borrowedQ);
        return basis.accrue(marketPrice(s).pEma, util, s.market.utilKinkBps, dt);
    } finally {
        basis.free();
    }
}

// ---- tier ----------------------------------------------------------------------------------------

/** The `TierInputs` venue code of a market (`engine::tier::tier_venue`); `null` for list-only. */
export function tierVenue(kind: KindName): number | null {
    switch (kind) {
        case 'pump_curve':
        case 'pump_migrating':
            return 0;
        case 'pump_swap':
            return 1;
        case 'raydium_v4':
        case 'raydium_cpmm':
            return 2;
        case 'mock':
            return 3;
        case 'none':
            return null;
    }
}

function tierInputs(math: MathModule, s: MarketSnapshot) {
    const venue = tierVenue(s.kind);
    if (venue === null) return null;
    const { pEma, lSlow } = marketPrice(s);
    const t = new math.TierInputs();
    t.venue = venue;
    t.lockedDepth = p128(s.market.poolD);
    t.lambdaMinBps = s.config.params.poolLockMinBps;
    t.rNow = s.venue.r;
    let ySlow = 0n;
    if (lSlow > 0n && pEma > 0n) {
        const r = math.reprice(lSlow, pEma);
        if (r) {
            ySlow = r.y;
            r.free();
        }
    }
    t.ySlow = ySlow;
    // PumpSwap's boost `max(VQR + buckets, 0)` (PUMP-2026-10-1); 0 elsewhere.
    t.vqr = s.venue.pool?.virtualPart ?? 0n;
    t.pEma = pEma;
    t.now = s.now;
    t.graduatedAt = s.market.graduatedAt;
    if (s.oracle) {
        t.oraclePrice = s.oracle.price;
        t.oracleConf = s.oracle.conf;
        t.oracleExpo = s.oracle.expo;
        t.oraclePublishTime = s.oracle.publishTime;
    }
    t.oracleStaleS = s.config.params.oracleStaleS;
    t.thetaAUsdE6 = s.config.params.thetaAUsdE6;
    t.thetaBUsdE6 = s.config.params.thetaBUsdE6;
    return t;
}

/** The tier a new open gets now (`engine::tier::assign`), `null` for a list-only market. */
export function assignTier(math: MathModule, s: MarketSnapshot): TierIndex | null {
    const t = tierInputs(math, s);
    if (!t) return null;
    try {
        return t.tier() as TierIndex;
    } finally {
        t.free();
    }
}

/** Raydium's `R_eff` (`engine::tier::r_eff`): half the tier liquidity; `null` if undefined. */
export function rEff(math: MathModule, s: MarketSnapshot): bigint | null {
    const t = tierInputs(math, s);
    if (!t) return null;
    try {
        const l = t.liquidityLamports();
        return l === undefined ? null : l / 2n;
    } catch {
        return null;
    } finally {
        t.free();
    }
}

// ---- open guards ----------------------------------------------------------------------------------

/** Why an open is refused before any size is chosen: the program error name, in the program's order. */
export interface Blocked {
    readonly code: string;
    /** A disclosure that explains the state, if one exists (§4). */
    readonly disclosure?:
        | 'market.coveragePause'
        | 'market.routeBroken'
        | 'market.venueUpgraded'
        | 'market.migrating'
        | 'market.graduationHalted'
        | 'market.listOnly'
        | 'market.mintUnsafe';
    /** Seconds until it ends by itself, when it is a timer. */
    readonly waitS?: bigint;
}

/**
 * The open guards of `open_short` steps 1, 3 and 4 (`engine::guard::open_static`, the latch, the
 * venue guards, the open guards), in the program's order: the first that refuses, else `null`.
 */
export function openBlocked(math: MathModule, s: MarketSnapshot): Blocked | null {
    const { config, market: m, now, kind, venue } = s;
    const p = config.params;
    const flag = (bit: number) => (m.flags & bit) !== 0;
    if ((config.flags & CONFIG_FLAG_PAUSED_OPENS) !== 0 || flag(MARKET_FLAG.pausedOpens))
        return { code: 'OpensPaused' };
    const clearEnds = m.coverageClearedAt + COVERAGE_CLEAR_DELAY_S;
    if (flag(MARKET_FLAG.pausedCoverage)) return { code: 'CoveragePaused', disclosure: 'market.coveragePause' };
    if (now < clearEnds) return { code: 'CoveragePaused', disclosure: 'market.coveragePause', waitS: clearEnds - now };
    const bit = VENUE_BIT[kind] ?? 0;
    if (flag(MARKET_FLAG.routeBroken) || (bit !== 0 && (config.venueRouteBroken & bit) !== 0)) {
        return { code: 'RouteBroken', disclosure: 'market.routeBroken' };
    }
    if (kind === 'none') return { code: 'NoSupportedVenue', disclosure: 'market.listOnly' };
    if (kind === 'pump_migrating') return { code: 'VenueMigrating', disclosure: 'market.migrating' };
    if (bit === 0 || (config.venueEnabled & bit) === 0) return { code: 'VenueDisabled' };
    if (flag(MARKET_FLAG.mintUnsafe)) return { code: 'MintUnsafeFlagged', disclosure: 'market.mintUnsafe' };
    if (p128(m.totalShares) === 0n) return { code: 'InsufficientInventory' };
    // The prologue's venue sync: a curve that completed moves the market (`VenueMigrating`).
    if (kind === 'pump_curve' && venue.curve?.live.complete)
        return { code: 'VenueMigrating', disclosure: 'market.migrating' };
    const mask = GUARD_MASK[kind] ?? 0;
    if (s.guard && (s.guard.upgraded & mask) !== 0)
        return { code: 'VenueUpgraded', disclosure: 'market.venueUpgraded' };
    if (!venue.executable) return { code: 'NoRoute' };
    if (math.exceedsCap(venue.feePpm, p.venueFeeHardCapBps) || math.exceedsCap(venue.feePpm, p.venueFeeCapBps)) {
        return { code: 'VenueFeeTooHigh' };
    }
    if (kind === 'raydium_v4' || kind === 'raydium_cpmm') {
        const windowEnd = m.poolSeenAt + BigInt(p.poolListWindowS);
        if (now < windowEnd) return { code: 'PoolListWindow', waitS: windowEnd - now };
        if (m.emaWarm !== 0) return { code: 'EmaWarmingUp' };
        const r = rEff(math, s);
        if (r === null || r < p.poolMinRealSol) return { code: 'PoolTooThin' };
    }
    if (kind === 'pump_swap') {
        if (venue.pool?.disableFlags !== 0) return { code: 'VenueFlagged' };
        if (m.emaWarm !== 0) return { code: 'EmaWarmingUp' };
    }
    if (kind === 'pump_curve') {
        const flags = venue.curve?.pricing?.poolDisableFlags;
        if (flags === undefined) return { code: 'NoRoute' };
        if ((flags & PAMM_CREATE_POOL_DISABLED_BIT) !== 0)
            return { code: 'GraduationHalted', disclosure: 'market.graduationHalted' };
    }
    const warm = m.createdAt + BigInt(p.marketWarmupS);
    if (now < warm) return { code: 'MarketWarmingUp', waitS: warm - now };
    const pEma = marketPrice(s).pEma;
    const pSpot = spotPrice(math, venue);
    if (pSpot === null) return { code: 'NoRoute' };
    if (!math.spotNearEma(pSpot, pEma, p.openEmaDevBps)) return { code: 'PriceDeviatesFromEma' };
    if (pEma > math.pOpenMax()) return { code: 'PriceAboveOpenMax' };
    if (venue.pool && venue.pool.boostAmount > p.boostDustLamports && now < m.graduatedAt + BigInt(p.boostWindowS)) {
        return { code: 'BoostInProgress', waitS: m.graduatedAt + BigInt(p.boostWindowS) - now };
    }
    return null;
}

// ---- caps ----------------------------------------------------------------------------------------

export type CapName = 'position' | 'market' | 'utilisation' | 'wallet' | 'gamma' | 'inventory' | 'notional';

/** The program error of each cap (`engine::caps::check_open`, then the notional of step 9). */
export const CAP_ERROR: Readonly<Record<CapName, string>> = {
    position: 'CapPositionImpact',
    market: 'CapMarketOi',
    utilisation: 'CapUtilization',
    gamma: 'CapCurveGamma',
    wallet: 'CapWallet',
    inventory: 'InsufficientInventory',
    notional: 'MarketNotionalCap',
};

export interface Caps {
    /** `X₀`, the conservative pre-trade token reserve. */
    readonly x0: bigint;
    /** The fee φ the fee-scaled caps take (ppm). */
    readonly phiPpm: number;
    /** The largest `q` each cap allows now (`gamma` only on the curve). */
    readonly max: {
        readonly position: bigint;
        readonly market: bigint;
        readonly utilisation: bigint;
        readonly gamma: bigint | null;
        readonly wallet: bigint;
        readonly inventory: bigint;
    };
    /** The binding size cap for the next open and its value. */
    readonly binding: Exclude<CapName, 'notional'>;
    readonly maxQ: bigint;
}

/** The curve's cap reserves at the EMA price (`engine::caps::curve_reserves`, M5-SEC-3). */
function curveCapReserves(math: MathModule, s: MarketSnapshot, pEma: bigint): readonly [bigint, bigint] | null {
    const c = s.venue.curve?.live;
    const pSpot = spotPrice(math, s.venue);
    if (!c || pSpot === null) return null;
    if (pEma <= pSpot) return [c.vt, c.rt];
    const l = math.liquidity(c.vt, c.vq);
    const r = math.reprice(l, pEma);
    const xEma = r ? r.x : 0n;
    r?.free();
    const offset = c.vt - c.rt;
    const x0 = c.vt < xEma ? c.vt : xEma;
    const rtEma = xEma > offset ? xEma - offset : 0n;
    return [x0, c.rt < rtEma ? c.rt : rtEma];
}

/** Largest `q ∈ [0, hi]` with `ok(q)` for a predicate true at 0 and monotone non-increasing. */
function maxTrue(hi: bigint, ok: (q: bigint) => boolean): bigint {
    if (hi <= 0n || ok(hi)) return hi < 0n ? 0n : hi;
    let lo = 0n;
    let h = hi;
    while (h - lo > 1n) {
        const mid = (lo + h) / 2n;
        if (ok(mid)) lo = mid;
        else h = mid;
    }
    return lo;
}

/** The size caps of the next open by `walletQ`'s wallet on tier `tp` (`engine::caps::check_open`). */
export function capsFor(math: MathModule, s: MarketSnapshot, tp: TierParams, walletQ: bigint): Caps | null {
    const p = s.config.params;
    const { pEma, lSlow } = marketPrice(s);
    const phi = capFeePpm(s.venue);
    let x0: bigint;
    let rtGuard: bigint | null = null;
    if (s.kind === 'pump_curve') {
        const r = curveCapReserves(math, s, pEma);
        if (!r) return null;
        [x0, rtGuard] = r;
    } else {
        const venue = tierVenue(s.kind);
        if (venue === null || venue === 0) return null;
        try {
            x0 = math.conservativeX0(venue, s.venue.x, lSlow, pEma, p128(s.market.poolD), p.poolLockMinBps);
        } catch {
            return null;
        }
    }
    const T = s.market.tokensFree;
    const B = s.market.borrowedQ;
    let position = math.maxQPosition(x0, tp.impactPosBps);
    const fee = math.feeSizeCap(x0, phi, p.feeSizeCapX100);
    if (fee !== undefined && fee < position) position = fee;
    const oiCapAt = (q: bigint): bigint | null => {
        try {
            return math.oiCap(x0 + q, tp.impactMktBps, phi, p.feeOiCapX100);
        } catch {
            return null;
        }
    };
    const hi = T < U64_MAX ? T : U64_MAX;
    const market = maxTrue(hi, (q) => {
        const cap = oiCapAt(q);
        return cap !== null && B + q <= cap;
    });
    const utilisation = maxTrue(hi, (q) => {
        try {
            return math.utilOk(T, B, q, p.utilCapBps);
        } catch {
            return false;
        }
    });
    const wallet = maxTrue(hi, (q) => {
        const cap = oiCapAt(q);
        try {
            return cap !== null && math.walletOk(walletQ, q, cap, p.walletCapBps);
        } catch {
            return false;
        }
    });
    const gamma = rtGuard === null ? null : math.curveGammaMaxQ(B, rtGuard, p.curveOiGammaBps);
    const max = { position, market, utilisation, gamma, wallet, inventory: T };
    let binding: Exclude<CapName, 'notional'> = 'inventory';
    let maxQ = T;
    for (const k of ['position', 'market', 'utilisation', 'gamma', 'wallet', 'inventory'] as const) {
        const v = max[k];
        if (v !== null && v < maxQ) {
            maxQ = v;
            binding = k;
        }
    }
    return { x0, phiPpm: phi, max, binding, maxQ };
}

/** The first cap `q` breaks, in the program's order (`check_open`). */
export function capRefusal(
    math: MathModule,
    s: MarketSnapshot,
    tp: TierParams,
    caps: Caps,
    q: bigint,
    walletQ: bigint,
): CapName | null {
    const p = s.config.params;
    if (q > caps.max.position) return 'position';
    const oi = math.oiCap(caps.x0 + q, tp.impactMktBps, caps.phiPpm, p.feeOiCapX100);
    if (s.market.borrowedQ + q > oi) return 'market';
    if (!math.utilOk(s.market.tokensFree, s.market.borrowedQ, q, p.utilCapBps)) return 'utilisation';
    if (caps.max.gamma !== null && q > caps.max.gamma) return 'gamma';
    if (!math.walletOk(walletQ, q, oi, p.walletCapBps)) return 'wallet';
    if (q > s.market.tokensFree) return 'inventory';
    return null;
}

// ---- open preview ---------------------------------------------------------------------------------

export interface OpenPreview {
    readonly q: bigint;
    readonly tier: TierIndex;
    readonly terms: TierParams;
    /** `S_exp` and the instruction's `min_proceeds` at the slippage. */
    readonly proceeds: bigint;
    readonly minProceeds: bigint;
    /** `N_open = B̂_spot(q)` on the post-sale reserves; `B̂_ema` on the newest ring entry. */
    readonly nOpen: bigint;
    readonly bEma: bigint | null;
    readonly margin: bigint;
    readonly maxMargin: bigint;
    readonly openFee: bigint;
    readonly maxOpenFee: bigint;
    /** The vault `V = S + M`. */
    readonly vault: bigint;
    readonly crOpenBps: bigint;
    readonly crEmaBps: bigint | null;
    /** `r_min` (rent charged at least) and the vault net of it. */
    readonly minRent: bigint;
    /** Liquidation price (WAD) of the spot leg at open, `null` if none. */
    readonly liqPriceWad: bigint | null;
    /** APR after the open (display) and rent per day at `P_ema`. */
    readonly rentRateBps: number;
    readonly rentPerDay: bigint;
    readonly maxDurationS: number;
    readonly deadline: bigint;
    readonly caps: Caps | null;
    /** The first refusal the program would return for this open, or `null`. */
    readonly refusal: string | null;
    readonly refusalCap: CapName | null;
}

export interface OpenInputs {
    readonly q: bigint;
    readonly slippageBps: number;
    /** The wallet's `WalletExposure.q_open` in this market (0 without one). */
    readonly walletQ: bigint;
}

/**
 * The open preview (§3.7; `open_short` steps 5–12 on the snapshot): `null` when there is no
 * tier (list-only) or the venue gives no sale quote (e.g. `q ≥ X`).
 */
export function previewOpen(math: MathModule, s: MarketSnapshot, inp: OpenInputs): OpenPreview | null {
    const { q } = inp;
    if (q <= 0n) return null;
    const tier = assignTier(math, s);
    if (tier === null) return null;
    const tp = s.config.tiers[tier];
    if (!tp) return null;
    const p = s.config.params;
    const caps = capsFor(math, s, tp, inp.walletQ);
    const sale = quoteSell(math, s.venue, q);
    if (!sale) return null;
    const minProceeds = lessSlippage(sale.proceeds, BigInt(inp.slippageBps));
    // Step 9: the venue as re-read after the sale; `N_open` at its live fee, `B̂_ema` on the ring.
    const post = afterSale(math, s.venue, q, sale);
    const nOpen: Buy | null = quoteBuy(math, post, q);
    if (!nOpen) return null;
    const [ex, ey] = newestRingXy(s.market);
    const bEma = quoteBuyAt(math, post, q, ex, ey)?.cost ?? null;
    const margin = math.marginD4(sale.proceeds, nOpen.cost, bEma ?? undefined, tp.crOpenBps);
    const openFee = math.openFee(nOpen.cost, p.openFeeBps);
    const vault = sale.proceeds + margin;
    const { pEma } = marketPrice(s);
    const utilPre = math.utilizationBps(s.market.tokensFree, s.market.borrowedQ);
    const kink = s.market.utilKinkBps;
    const minRent = math.minRent(
        q,
        pEma,
        tp.rateBaseBps,
        tp.rateSlope1Bps,
        tp.rateSlope2Bps,
        utilPre,
        kink,
        tp.minRentS,
    );
    const crOpenBps = math.crBps(math.vEff(vault, 0n), nOpen.cost);
    const crEmaBps = bEma === null ? null : math.crBps(vault, bEma);
    // The spot leg's liquidation price at open: the exit risk view (the position's hard cap; on
    // PumpSwap at most the tier at `P_ema`, AIR-10), the post-open liquidity, from the post-open
    // spot; the vault net of the rent always due (r_min).
    const lPost = liquidityOf(math, post);
    const pPost = spotPrice(math, post);
    const vEffOpen = math.vEff(vault, minRent);
    const riskCapBps = exitRiskCapBps(post, p.venueFeeHardCapBps);
    const liqPriceWad =
        lPost === null || pPost === null
            ? null
            : (withVenue(math, post, { live: false, capBps: riskCapBps }, (v) =>
                  v.liquidationPrice(q, vEffOpen, tp.crLiqBps, lPost, pPost),
              ) ?? null);
    const tAfter = s.market.tokensFree - q;
    const utilAfter = tAfter >= 0n ? math.utilizationBps(tAfter, s.market.borrowedQ + q) : 10_000;
    const rentRateBps = math.rateBps(tp.rateBaseBps, tp.rateSlope1Bps, tp.rateSlope2Bps, utilAfter, kink);
    const rentPerDay = rentOver(math, q, pEma, tp, p.rentProtocolBps, utilAfter, kink, 86_400n);
    let refusalCap = caps ? capRefusal(math, s, tp, caps, q, inp.walletQ) : null;
    let refusal: string | null = refusalCap ? CAP_ERROR[refusalCap] : null;
    if (!refusal && nOpen.cost < p.minNotional) refusal = 'BelowMinNotional';
    if (!refusal && !math.notionalOk(nOpen.cost, s.market.totalNotionalOpen, p.minNotional, p.marketMaxNotional)) {
        refusal = 'MarketNotionalCap';
        refusalCap = 'notional';
    }
    return {
        q,
        tier,
        terms: tp,
        proceeds: sale.proceeds,
        minProceeds,
        nOpen: nOpen.cost,
        bEma,
        margin,
        maxMargin: withTolerance(margin, MARGIN_TOLERANCE_BPS),
        openFee,
        maxOpenFee: withTolerance(openFee, MARGIN_TOLERANCE_BPS),
        vault,
        crOpenBps,
        crEmaBps,
        minRent,
        liqPriceWad,
        rentRateBps,
        rentPerDay,
        maxDurationS: tp.maxDurationS,
        deadline: s.now + BigInt(tp.maxDurationS),
        caps,
        refusal,
        refusalCap,
    };
}

/** Rent of `q` over `dt` seconds at `P_ema` and a constant utilisation (`RentBasis.accrue`, `PositionRent.accrued`). */
function rentOver(
    math: MathModule,
    q: bigint,
    pEma: bigint,
    tp: TierParams,
    rentProtocolBps: number,
    utilBps: number,
    kinkBps: number,
    dt: bigint,
): bigint {
    const b0 = new math.RentBasis(0n, 0n, 0n);
    const b1 = b0.accrue(pEma, utilBps, kinkBps, dt);
    const pr = new math.PositionRent(q, b0, tp.rateBaseBps, tp.rateSlope1Bps, tp.rateSlope2Bps, rentProtocolBps, 0n);
    try {
        return pr.accrued(b1);
    } finally {
        pr.free();
        b1.free();
        b0.free();
    }
}

// ---- open positions --------------------------------------------------------------------------------

export interface PositionMetrics {
    /** `R_due = max(accrued, r_min)` now. */
    readonly rentDue: bigint;
    readonly rentAccrued: bigint;
    readonly vEff: bigint;
    /**
     * The live buyback quote at the live fee (closes) and at the position's exit risk fee
     * (liquidation; `exitRiskCapBps`, which the CRs and the liquidation price take too). On a live
     * curve past `rT` both are `B̂_v3`: the close completes the curve by pump's synthetic migration
     * in one instruction (pump-final §5.2; owner D3).
     */
    readonly buyback: bigint | null;
    readonly buybackRisk: bigint | null;
    readonly crBps: bigint | null;
    readonly crEmaBps: bigint | null;
    readonly liqPriceWad: bigint | null;
    /** What closing now pays the borrower (`previewClose`), and the P&L against what it put in. */
    readonly payout: bigint | null;
    readonly pnl: bigint | null;
    readonly timeLeftS: bigint;
    /**
     * `true` when the buyback (and rent and `ert`) exceeds the vault: a close refuses
     * `InsufficientVault`. `false` without a buyback quote (`buyback` is `null` then).
     */
    readonly insufficientVault: boolean;
}

/** The live metrics of an open position on its market's snapshot (§3.8). */
export function positionMetrics(math: MathModule, s: MarketSnapshot, pos: ShortPosition): PositionMetrics {
    const basis = rentBasisNow(math, s);
    const jOpen = new math.RentBasis(pos.jOpen[0] ?? 0n, pos.jOpen[1] ?? 0n, pos.jOpen[2] ?? 0n);
    const pr = new math.PositionRent(
        pos.q,
        jOpen,
        pos.rateBaseBps,
        pos.rateSlope1Bps,
        pos.rateSlope2Bps,
        pos.rentProtocolBps,
        pos.rMin,
    );
    let rentDue: bigint;
    let rentAccrued: bigint;
    try {
        rentDue = pr.due(basis);
        rentAccrued = pr.accrued(basis);
    } finally {
        pr.free();
        jOpen.free();
        basis.free();
    }
    const vEff = math.vEff(pos.v, rentDue);
    const live = quoteBuy(math, s.venue, pos.q)?.cost ?? null;
    // The exit risk view at the program's cap: the position's locked hard cap and, on PumpSwap,
    // at most the tier at `P_ema` (`liquidate::assess`, `sol_settle`, the keeper; AIR-10).
    const capBps = exitRiskCapBps(s.venue, pos.venueFeeHardCapBps);
    const risk = quoteBuy(math, s.venue, pos.q, capBps)?.cost ?? null;
    const [ex, ey] = newestRingXy(s.market);
    const emaRisk = quoteBuyAt(math, s.venue, pos.q, ex, ey, capBps)?.cost ?? null;
    const crBps = risk === null ? null : math.crBps(vEff, risk);
    const crEmaBps = emaRisk === null ? null : math.crBps(vEff, emaRisk);
    const l = liquidityOf(math, s.venue);
    const pSpot = spotPrice(math, s.venue);
    const liqPriceWad =
        l === null || pSpot === null
            ? null
            : (withVenue(math, s.venue, { live: false, capBps }, (v) =>
                  v.liquidationPrice(pos.q, vEff, pos.crLiqBps, l, pSpot),
              ) ?? null);
    // Without a buyback quote (no route, or past a curve's rT without the synthetic route) the
    // close has nothing to compare: no quote, not a thin vault.
    const insufficientVault = live !== null && live + rentDue + pos.ert > pos.v;
    let payout: bigint | null = null;
    if (live !== null && !insufficientVault) {
        const w = math.previewClose(pos.v, live, rentDue, pos.rentProtocolBps);
        payout = w.borrower;
        w.free();
    }
    return {
        rentDue,
        rentAccrued,
        vEff,
        buyback: live,
        buybackRisk: risk,
        crBps,
        crEmaBps,
        liqPriceWad,
        payout,
        pnl: payout === null ? null : payout - pos.m - pos.feeOpen,
        timeLeftS: pos.deadline > s.now ? pos.deadline - s.now : 0n,
        insufficientVault,
    };
}

export interface ClosePreview {
    readonly buyback: bigint;
    readonly rent: bigint;
    readonly rentProtocol: bigint;
    readonly payout: bigint;
    readonly pnl: bigint;
    /** `close_short`'s `max_buyback_cost = ⌈B̂_live·(1 + slippage)⌉`. */
    readonly maxBuybackCost: bigint;
}

/**
 * The close preview (`close_short` steps 3–6): `null` when a close would refuse (see
 * `positionMetrics`). Defined whenever the live buyback is: past a live curve's `rT` at `B̂_v3`.
 */
export function previewCloseFor(
    math: MathModule,
    s: MarketSnapshot,
    pos: ShortPosition,
    slippageBps: number,
): ClosePreview | null {
    const mx = positionMetrics(math, s, pos);
    if (mx.buyback === null || mx.insufficientVault) return null;
    const w = math.previewClose(pos.v, mx.buyback, mx.rentDue, pos.rentProtocolBps);
    try {
        return {
            buyback: w.buyback,
            rent: w.rent,
            rentProtocol: w.rentProtocol,
            payout: w.borrower,
            pnl: w.borrower - pos.m - pos.feeOpen,
            maxBuybackCost: withTolerance(mx.buyback, BigInt(slippageBps)),
        };
    } finally {
        w.free();
    }
}

/** CR after adding `amount` lamports of margin (`add_margin`: `v += amount`, nothing else changes). */
export function previewAddMargin(
    math: MathModule,
    s: MarketSnapshot,
    pos: ShortPosition,
    amount: bigint,
): bigint | null {
    const mx = positionMetrics(math, s, pos);
    if (mx.buybackRisk === null) return null;
    return math.crBps(math.vEff(pos.v + amount, mx.rentDue), mx.buybackRisk);
}

/** The risk band of a CR against the position's liquidation CR (§7.1). */
export { riskLevel } from '../../components/Badge.tsx';
