/**
 * The live state of a market's venue (ARCH §4.1 `VenueState { X, Y, R, P, L, fee }`): a port of
 * each adapter's `read_state` (`programs/shortwin/src/venues/{mock,raydium_v4,raydium_cpmm,
 * pump_curve,pump_amm}.rs`) — the venue predicate (`executable`), the pricing reserves, the real
 * SOL, the locked depth and the venue's fees — and the quotes on it (`venues::Quoter`), every one
 * computed by the WASM facade of `shortwin-math` (FRONTEND.md §6.7: the frontend never
 * re-implements protocol math; this module only decides which reserves and fees the math sees, as
 * the program does).
 *
 * Fees are resolved with the facade's schedules (`CurveSchedule`, `PammSchedule`, `Venue.cpmm`,
 * `Venue.rayV4`), on the live state and, for the fee-scaled caps of an open, at the stored
 * `P_ema`'s market cap (M7-SEC-5, `VenueState::cap_fee_ppm`).
 */
import type { Address } from '@solana/kit';
import type { MathModule } from '../math/load.ts';
import { DEFAULT_KEY } from './bytes.ts';
import {
    type KindName,
    MOCK_PROTOCOL_FEE_RATE,
    PAMM_BUY_DISABLED_BIT,
    PAMM_CACHE,
    PUMP_AMM_PROGRAM,
    PUMP_CURVE_MCAP_SUPPLY,
    RAY_CACHE,
    STANDARD_POOL_TOKENS,
    TOKEN_PROGRAM,
    WSOL_MINT,
} from './constants.ts';
import { CPMM_SLOT, CURVE_SLOT, cached, type MarketKeys, MOCK_SLOT, PAMM_SLOT, solFirst, V4_SLOT } from './lists.ts';
import {
    type BondingCurve,
    boostVaultAmount,
    cpmmExecutable,
    cpmmFeePpm,
    cpmmReserves,
    cpmmSolIsToken0,
    type FeeTier,
    mockReserves,
    pammReserves,
    pammVirtualPart,
    type RawAccount,
    readAmmInfo,
    readCpmmConfig,
    readCpmmPool,
    readCurve,
    readFeeConfig,
    readMintBase,
    readMockPool,
    readPammGlobalConfig,
    readPammPool,
    readPumpGlobal,
    readTokenAccount,
    tokenOf,
    v4Executable,
    v4Reserves,
    v4SolIsCoin,
    value,
} from './readers.ts';

const WAD = 10n ** 18n;
const U32_MAX = 0xffff_ffff;
/** `u64::MAX`: the program's fallback for a virtual part past `u64` (never on a pool it prices). */
const U64_MAX_BIG = (1n << 64n) - 1n;

/** Fee components in bps (`fees::FeeBps`). */
export interface FeeBps {
    readonly lp: number;
    readonly protocol: number;
    readonly creator: number;
}

/** The constant-product venues' resolved pricing (`quote::Pricing`), as the facade's `Venue` takes it. */
export type PoolPricing =
    | { readonly type: 'pumpSwap'; readonly fees: FeeBps }
    | { readonly type: 'rayV4'; readonly num: bigint; readonly den: bigint }
    | {
          readonly type: 'cpmm';
          readonly trade: number;
          readonly protocol: number;
          readonly fund: number;
          readonly creator: number;
          readonly enableCreatorFee: boolean;
          readonly creatorFeeOn: number;
          readonly solIsToken0: boolean;
      };

/** The curve's fee schedule and the post-migration pool's (what `CurvePricingInputs` carries). */
export interface CurveSchedules {
    readonly curveTiers: readonly FeeTier[];
    readonly creatorFeeConfigurable: boolean;
    readonly curveCreatorFeeBps: number;
    readonly creatorIsDefault: boolean;
    readonly poolTiers: readonly FeeTier[];
    readonly poolCreatorFeeConfigurable: boolean;
}

export interface CurveLive {
    readonly vt: bigint;
    readonly vq: bigint;
    readonly rt: bigint;
    readonly rq: bigint;
    readonly complete: boolean;
}

/** The curve's pricing inputs (`CurvePricingInputs`), `null` when an account does not parse or `vT = 0`. */
export interface CurvePricingInputs {
    readonly fees: FeeBps;
    readonly offsetToken: bigint;
    readonly offsetQuote: bigint;
    /** The live leg's `X_p = curve_base_ata.amount − rT`. */
    readonly livePoolTokens: bigint;
    readonly migrationFee: bigint;
    readonly schedules: CurveSchedules;
    /** PumpSwap `GlobalConfig.disable_flags` (bit 0 halts graduation: `GraduationHalted`). */
    readonly poolDisableFlags: number;
    /** The fee (ppm) at the stored `P_ema`'s market cap; `U32_MAX` without a price. */
    readonly feePpmAtEma: number;
}

export interface CurveView {
    readonly live: CurveLive;
    readonly creator: Address;
    readonly canonicalPool: 'absent' | 'valid' | 'unreadable';
    readonly pricing: CurvePricingInputs | null;
    /** Complete with `vT > 0`. */
    readonly migrating: boolean;
}

/** What a PumpSwap market's opens and tier read besides the reserves (`pump_amm::PoolVenue`). */
export interface PoolView {
    readonly disableFlags: number;
    readonly boostAmount: bigint;
    /** `max(VQR + buckets, 0)`, the boost the tier subtracts (`PoolVenue::virtual_part`). */
    readonly virtualPart: bigint;
    readonly isCashback: boolean;
    readonly coinCreator: Address;
    readonly fees: FeeBps | null;
    readonly feePpmAtEma: number;
    /** The schedule the fee resolves on (re-resolved on a post-trade state). */
    readonly schedule: {
        readonly tiers: readonly FeeTier[];
        readonly supply: bigint;
        readonly creatorFeeConfigurable: boolean;
        readonly poolCreatorFeeBps: number;
        readonly coinCreatorIsDefault: boolean;
    } | null;
}

export interface VenueState {
    readonly kind: KindName;
    readonly executable: boolean;
    readonly x: bigint;
    readonly y: bigint;
    readonly r: bigint;
    readonly pricing: PoolPricing | null;
    readonly feePpm: number;
    readonly lockedDepth: bigint;
    readonly curve: CurveView | null;
    readonly pool: PoolView | null;
}

export function notExecutable(kind: KindName): VenueState {
    return {
        kind,
        executable: false,
        x: 0n,
        y: 0n,
        r: 0n,
        pricing: null,
        feePpm: 0,
        lockedDepth: 0n,
        curve: null,
        pool: null,
    };
}

export interface ReadContext {
    /** Cluster `unix_timestamp` (the venues' time gates). */
    readonly now: bigint;
    /** The mint's live supply (PumpSwap's fee tier). */
    readonly supply: bigint | null;
    /** The market's stored `P_ema` (the caps' fee at the EMA), `0n` without a price. */
    readonly pEma: bigint;
}

const at = (read: readonly (RawAccount | null)[], i: number): RawAccount | null => read[i] ?? null;

function feesOf(f: { lp: number; protocol: number; creator: number; free(): void }): FeeBps {
    const out = { lp: f.lp, protocol: f.protocol, creator: f.creator };
    f.free();
    return out;
}

/** `fees::pump_curve` on `(vT, vQ)` with the facade's `CurveSchedule`. */
export function resolveCurveFees(m: MathModule, s: CurveSchedules, vt: bigint, vq: bigint): FeeBps | null {
    const sch = new m.CurveSchedule(s.creatorFeeConfigurable, s.curveCreatorFeeBps, s.creatorIsDefault);
    try {
        for (const t of s.curveTiers) sch.addTier(t.threshold, t.lp, t.protocol, t.creator);
        return feesOf(sch.resolve(vt, vq));
    } catch {
        return null;
    } finally {
        sch.free();
    }
}

/** `fees::pump_amm` on `(effective quote, base)` with the facade's `PammSchedule`. */
export function resolvePammFees(
    m: MathModule,
    s: NonNullable<PoolView['schedule']>,
    quote: bigint,
    base: bigint,
): FeeBps | null {
    const sch = new m.PammSchedule(s.supply, s.creatorFeeConfigurable, s.poolCreatorFeeBps, s.coinCreatorIsDefault);
    try {
        for (const t of s.tiers) sch.addTier(t.threshold, t.lp, t.protocol, t.creator);
        return feesOf(sch.resolve(quote, base));
    } catch {
        return null;
    } finally {
        sch.free();
    }
}

/** One-way φ in ppm of resolved pump fees (`FeeBps::oneway_ppm`). */
export function onewayPpm(m: MathModule, f: FeeBps): number {
    const x = new m.Fees(f.lp, f.protocol, f.creator);
    try {
        return x.onewayPpm();
    } finally {
        x.free();
    }
}

// ---- per venue ------------------------------------------------------------------------------------

function mockState(read: readonly (RawAccount | null)[]): VenueState {
    const st = notExecutable('mock');
    const pa = at(read, MOCK_SLOT.pool);
    const ba = at(read, MOCK_SLOT.baseVault);
    const qa = at(read, MOCK_SLOT.quoteVault);
    if (!pa || !ba || !qa) return st;
    const pool = value(readMockPool(pa.owner, pa.data));
    if (!pool || pool.baseVault !== ba.address || pool.quoteVault !== qa.address) return st;
    const base = value(readTokenAccount(ba.owner, ba.data));
    const quote = value(readTokenAccount(qa.owner, qa.data));
    if (!base || !quote || base.mint !== pool.baseMint || quote.mint !== pool.quoteMint) return st;
    const r = mockReserves(pool, base.amount, quote.amount);
    if (!r) return st;
    return {
        ...st,
        executable: true,
        x: r[0],
        y: r[1],
        r: r[1],
        // The mock prices as CPMM with `trade = fee_ppm`, protocol 120,000, no creator fee (INTERFACE.md).
        pricing: {
            type: 'cpmm',
            trade: pool.feePpm,
            protocol: MOCK_PROTOCOL_FEE_RATE,
            fund: 0,
            creator: 0,
            enableCreatorFee: false,
            creatorFeeOn: 0,
            solIsToken0: false,
        },
        feePpm: pool.feePpm,
    };
}

function lpMintSupply(acc: RawAccount | null, program?: Address): bigint | null {
    if (!acc) return null;
    if (program && acc.owner !== program) return null;
    return value(readMintBase(acc.owner, acc.data))?.supply ?? null;
}

function v4State(m: MathModule, market: MarketKeys, read: readonly (RawAccount | null)[], now: bigint): VenueState {
    const st = notExecutable('raydium_v4');
    const pa = at(read, V4_SLOT.pool);
    const ca = at(read, V4_SLOT.coinVault);
    const pca = at(read, V4_SLOT.pcVault);
    const la = at(read, V4_SLOT.lpMint);
    if (!pa || !ca || !pca || !la) return st;
    const info = value(readAmmInfo(pa.owner, pa.data));
    if (!info || info.coinVault !== ca.address || info.pcVault !== pca.address || info.lpMint !== la.address) return st;
    const coin = tokenOf(ca, TOKEN_PROGRAM);
    const pc = tokenOf(pca, TOKEN_PROGRAM);
    const lp = lpMintSupply(la, TOKEN_PROGRAM);
    if (!coin || !pc || lp === null || coin.mint !== info.coinVaultMint || pc.mint !== info.pcVaultMint) return st;
    // `consistent`: nonce 254, the recorded SOL side, the market's mint, an SPL Token market.
    const sf = solFirst(market);
    const tokMint = sf ? info.pcVaultMint : info.coinVaultMint;
    if (
        info.nonce !== 254n ||
        v4SolIsCoin(info) !== sf ||
        tokMint !== market.mint ||
        market.tokenProgram !== TOKEN_PROGRAM
    ) {
        return st;
    }
    if (info.swapFeeNumerator >= info.swapFeeDenominator) return st;
    if (!v4Executable(info, now, coin.amount, pc.amount)) return st;
    const r = v4Reserves(info, coin.amount, pc.amount);
    if (!r) return st;
    const [x, y] = sf ? [r[1], r[0]] : [r[0], r[1]];
    let feePpm: number;
    const v = m.Venue.rayV4(info.swapFeeNumerator, info.swapFeeDenominator);
    try {
        feePpm = v.onewayPpm();
    } finally {
        v.free();
    }
    return {
        ...st,
        executable: true,
        x,
        y,
        r: y,
        pricing: { type: 'rayV4', num: info.swapFeeNumerator, den: info.swapFeeDenominator },
        feePpm,
        lockedDepth: m.lockedDepth(info.lpAmount, lp, x, y),
    };
}

function cpmmState(m: MathModule, market: MarketKeys, read: readonly (RawAccount | null)[], now: bigint): VenueState {
    const st = notExecutable('raydium_cpmm');
    const pa = at(read, CPMM_SLOT.pool);
    const ca = at(read, CPMM_SLOT.ammConfig);
    const v0a = at(read, CPMM_SLOT.token0Vault);
    const v1a = at(read, CPMM_SLOT.token1Vault);
    const la = at(read, CPMM_SLOT.lpMint);
    if (!pa || !ca || !v0a || !v1a || !la) return st;
    const pool = value(readCpmmPool(pa.owner, pa.data));
    if (
        !pool ||
        pool.ammConfig !== ca.address ||
        pool.token0Vault !== v0a.address ||
        pool.token1Vault !== v1a.address ||
        pool.lpMint !== la.address
    ) {
        return st;
    }
    const config = value(readCpmmConfig(ca.owner, ca.data));
    const v0 = tokenOf(v0a, pool.token0Program);
    const v1 = tokenOf(v1a, pool.token1Program);
    const lp = lpMintSupply(la, TOKEN_PROGRAM);
    if (!config || !v0 || !v1 || lp === null || v0.mint !== pool.token0Mint || v1.mint !== pool.token1Mint) return st;
    const sf = solFirst(market);
    const [tokMint, solProg, tokProg] = sf
        ? [pool.token1Mint, pool.token0Program, pool.token1Program]
        : [pool.token0Mint, pool.token1Program, pool.token0Program];
    if (
        cpmmSolIsToken0(pool) !== sf ||
        tokMint !== market.mint ||
        solProg !== TOKEN_PROGRAM ||
        tokProg !== market.tokenProgram ||
        pool.observationKey !== cached(market, RAY_CACHE.observation)
    ) {
        return st;
    }
    const feePpm = cpmmFeePpm(pool, config);
    if (feePpm === null) return st;
    if (!cpmmExecutable(pool, config, now, v0.amount, v1.amount)) return st;
    const r = cpmmReserves(pool, v0.amount, v1.amount);
    if (!r) return st;
    const [x, y] = sf ? [r[1], r[0]] : [r[0], r[1]];
    return {
        ...st,
        executable: true,
        x,
        y,
        r: y,
        pricing: {
            type: 'cpmm',
            trade: Number(config.tradeFeeRate),
            protocol: Number(config.protocolFeeRate),
            fund: Number(config.fundFeeRate),
            creator: Number(config.creatorFeeRate),
            enableCreatorFee: pool.enableCreatorFee,
            creatorFeeOn: pool.creatorFeeOn,
            solIsToken0: sf,
        },
        feePpm,
        lockedDepth: m.lockedDepth(pool.lpSupply, lp, x, y),
    };
}

/** The canonical pool account at `cp` (`detect::canonical_pool_read`). */
export function canonicalPoolState(
    cp: RawAccount | null,
    mint: Address,
    authority: Address,
): { state: 'absent' | 'valid' | 'unreadable'; pool: ReturnType<typeof readPammPool> | null } {
    if (!cp || cp.owner === ('11111111111111111111111111111111' as Address)) return { state: 'absent', pool: null };
    if (cp.owner !== PUMP_AMM_PROGRAM) return { state: 'unreadable', pool: null };
    const p = readPammPool(cp.owner, cp.data);
    if (!p.ok) return { state: 'unreadable', pool: null };
    const valid =
        p.value.index === 0 &&
        p.value.creator === authority &&
        p.value.baseMint === mint &&
        p.value.quoteMint === WSOL_MINT &&
        !p.value.isMayhemMode;
    return valid ? { state: 'valid', pool: p } : { state: 'unreadable', pool: null };
}

function curveState(
    m: MathModule,
    kind: KindName,
    market: MarketKeys,
    read: readonly (RawAccount | null)[],
    ctx: ReadContext,
): VenueState {
    const st = notExecutable(kind);
    const ca = at(read, CURVE_SLOT.curve);
    if (!ca) return st;
    const c = value(readCurve(ca.owner, ca.data));
    if (!c) return st;
    const live: CurveLive = { vt: c.vt, vq: c.vq, rt: c.rt, rq: c.rq, complete: c.complete };
    const authority = market.pumpPoolAuthority;
    const cp = c.complete
        ? canonicalPoolState(at(read, CURVE_SLOT.canonicalPool), market.mint, authority).state
        : 'absent';
    const pricing = c.vt > 0n ? curvePricing(m, market, read, c, live, ctx) : null;
    const listable = ca.data.length >= 115 && c.quoteMint === DEFAULT_KEY && !c.isMayhemMode;
    const reserves = c.vt > 0n && c.vq > 0n;
    const migrating = c.complete && c.vt > 0n;
    const executable = listable && reserves && !c.complete && pricing !== null;
    const observed = listable && reserves && (executable || migrating);
    return {
        ...st,
        executable,
        x: observed ? c.vt : 0n,
        y: observed ? c.vq : 0n,
        r: observed ? c.rq : 0n,
        feePpm: executable && pricing ? onewayPpm(m, pricing.fees) : 0,
        curve: { live, creator: c.creator, canonicalPool: cp, pricing, migrating },
    };
}

function curvePricing(
    m: MathModule,
    market: MarketKeys,
    read: readonly (RawAccount | null)[],
    c: BondingCurve,
    live: CurveLive,
    ctx: ReadContext,
): CurvePricingInputs | null {
    const ga = at(read, CURVE_SLOT.global);
    const fa = at(read, CURVE_SLOT.feeConfig);
    const ba = at(read, CURVE_SLOT.curveBaseAta);
    const pfa = at(read, CURVE_SLOT.pammFeeConfig);
    const gca = at(read, CURVE_SLOT.pammGlobalConfig);
    if (!ga || !fa || !ba || !pfa || !gca) return null;
    const global = value(readPumpGlobal(ga.owner, ga.data));
    const fc = value(readFeeConfig(fa.owner, fa.data));
    const pfc = value(readFeeConfig(pfa.owner, pfa.data));
    const gc = value(readPammGlobalConfig(gca.owner, gca.data));
    if (!global || !fc || !pfc || !gc) return null;
    const schedules: CurveSchedules = {
        curveTiers: fc.feeTiers,
        creatorFeeConfigurable: global.creatorFeeConfigurable,
        curveCreatorFeeBps: Number(c.creatorFeeBps),
        creatorIsDefault: c.creator === DEFAULT_KEY,
        poolTiers: pfc.feeTiers,
        poolCreatorFeeConfigurable: gc.creatorFeeConfigurable,
    };
    const fees = resolveCurveFees(m, schedules, live.vt, live.vq);
    if (!fees) return null;
    if (live.rt > live.vt || live.rq > live.vq) return null;
    if (ba.owner !== market.tokenProgram) return null;
    const t = value(readTokenAccount(ba.owner, ba.data));
    if (!t || t.mint !== market.mint || t.owner !== market.bondingCurve || t.amount < live.rt) return null;
    let feePpmAtEma = U32_MAX;
    if (ctx.pEma > 0n) {
        const vqEma = (ctx.pEma * live.vt) / WAD;
        const f = resolveCurveFees(m, schedules, live.vt, vqEma);
        if (f) feePpmAtEma = onewayPpm(m, f);
    }
    return {
        fees,
        offsetToken: live.vt - live.rt,
        offsetQuote: live.vq - live.rq,
        livePoolTokens: t.amount - live.rt,
        migrationFee: global.poolMigrationFee,
        schedules,
        poolDisableFlags: gc.disableFlags,
        feePpmAtEma,
    };
}

function pammState(
    m: MathModule,
    market: MarketKeys,
    read: readonly (RawAccount | null)[],
    ctx: ReadContext,
): VenueState {
    const st = notExecutable('pump_swap');
    const pa = at(read, PAMM_SLOT.pool);
    if (!pa || pa.owner !== PUMP_AMM_PROGRAM) return st;
    const pool = value(readPammPool(pa.owner, pa.data));
    if (!pool) return st;
    const canonical =
        pool.index === 0 &&
        pool.creator === market.pumpPoolAuthority &&
        pool.baseMint === market.mint &&
        pool.quoteMint === WSOL_MINT &&
        !pool.isMayhemMode;
    const ba = at(read, PAMM_SLOT.baseVault);
    const qa = at(read, PAMM_SLOT.quoteVault);
    const gca = at(read, PAMM_SLOT.globalConfig);
    const fca = at(read, PAMM_SLOT.feeConfig);
    const boost = at(read, PAMM_SLOT.boostVaultAta);
    const lpa = at(read, PAMM_SLOT.lpMint);
    if (!ba || !qa || !gca || !fca || !lpa) return st;
    if (!canonical || pool.poolBaseTokenAccount !== ba.address || pool.poolQuoteTokenAccount !== qa.address) return st;
    const base = tokenOf(ba, market.tokenProgram);
    const quote = tokenOf(qa, TOKEN_PROGRAM);
    if (!base || base.mint !== market.mint || base.owner !== pa.address) return st;
    if (!quote || quote.mint !== WSOL_MINT || quote.owner !== pa.address) return st;
    const gc = value(readPammGlobalConfig(gca.owner, gca.data));
    const fc = value(readFeeConfig(fca.owner, fca.data));
    if (!gc || !fc) return st;
    const boostAmount = value(boostVaultAmount(boost, cached(market, PAMM_CACHE.boostVault)));
    if (boostAmount === null) return st;
    const reserves = pammReserves(pool, base.amount, quote.amount);
    if (!reserves) return st;
    const schedule =
        ctx.supply === null
            ? null
            : {
                  tiers: fc.feeTiers,
                  supply: ctx.supply,
                  creatorFeeConfigurable: gc.creatorFeeConfigurable,
                  poolCreatorFeeBps: Number(pool.creatorFeeBps),
                  coinCreatorIsDefault: pool.coinCreator === DEFAULT_KEY,
              };
    const fees = schedule ? resolvePammFees(m, schedule, reserves.effectiveQuote, reserves.base) : null;
    let feePpmAtEma = U32_MAX;
    if (schedule && ctx.pEma > 0n) {
        const q = (ctx.pEma * reserves.base) / WAD;
        const f = resolvePammFees(m, schedule, q, reserves.base);
        if (f) feePpmAtEma = onewayPpm(m, f);
    }
    const poolView: PoolView = {
        disableFlags: gc.disableFlags,
        boostAmount,
        virtualPart: pammVirtualPart(pool) ?? U64_MAX_BIG,
        isCashback: pool.isCashbackCoin,
        coinCreator: pool.coinCreator,
        fees,
        feePpmAtEma,
        schedule,
    };
    const lpSupply = lpa.address === pool.lpMint ? lpMintSupply(lpa) : null;
    const depth =
        lpSupply === null ? null : m.lockedDepth(pool.lpSupply, lpSupply, reserves.base, reserves.effectiveQuote);
    const executable =
        reserves.base > 0n &&
        reserves.effectiveQuote > 0n &&
        fees !== null &&
        depth !== null &&
        (gc.disableFlags & PAMM_BUY_DISABLED_BIT) === 0;
    if (!executable || !fees || depth === null) return { ...st, pool: poolView };
    return {
        ...st,
        executable: true,
        x: reserves.base,
        y: reserves.effectiveQuote,
        // The payable SOL (the vault less the fee buckets), at most the effective reserve.
        r: reserves.payableQuote < reserves.effectiveQuote ? reserves.payableQuote : reserves.effectiveQuote,
        pricing: { type: 'pumpSwap', fees },
        feePpm: onewayPpm(m, fees),
        lockedDepth: depth,
        pool: poolView,
    };
}

/**
 * The venue state of a market on `kind` from its ReadSet accounts as read now (`venues::
 * read_state_ema` with `ema_fee`, the open's pre-trade read). Never throws: whatever does not parse
 * is "not executable" (A63).
 */
export function readVenueState(
    m: MathModule,
    kind: KindName,
    market: MarketKeys,
    read: readonly (RawAccount | null)[],
    ctx: ReadContext,
): VenueState {
    switch (kind) {
        case 'mock':
            return mockState(read);
        case 'raydium_v4':
            return v4State(m, market, read, ctx.now);
        case 'raydium_cpmm':
            return cpmmState(m, market, read, ctx.now);
        case 'pump_curve':
        case 'pump_migrating':
            return curveState(m, kind, market, read, ctx);
        case 'pump_swap':
            return pammState(m, market, read, ctx);
        case 'none':
            return notExecutable('none');
    }
}

// ---- quotes (`venues::Quoter`) -----------------------------------------------------------------------

/** `VenueState::observed`: executable, or a migrating curve. */
export function observed(s: VenueState): boolean {
    return s.executable || (s.curve?.migrating ?? false);
}

/** Spot `⌊Y·WAD/X⌋` of an observed venue (`VenueState::price`). */
export function spotPrice(m: MathModule, s: VenueState): bigint | null {
    if (!observed(s) || s.x === 0n || s.y === 0n) return null;
    try {
        const p = m.priceWad(s.x, s.y);
        return p > 0n ? p : null;
    } catch {
        return null;
    }
}

/** `L = isqrt(X·Y)` of an observed venue. */
export function liquidityOf(m: MathModule, s: VenueState): bigint | null {
    if (!observed(s)) return null;
    try {
        const l = m.liquidity(s.x, s.y);
        return l > 0n ? l : null;
    } catch {
        return null;
    }
}

/** `VenueState::cap_fee_ppm`: `min(φ_live, φ at P_ema)` on the pump venues, `φ_live` elsewhere. */
export function capFeePpm(s: VenueState): number {
    const atEma =
        s.kind === 'pump_swap'
            ? (s.pool?.feePpmAtEma ?? U32_MAX)
            : s.kind === 'pump_curve' || s.kind === 'pump_migrating'
              ? (s.curve?.pricing?.feePpmAtEma ?? U32_MAX)
              : U32_MAX;
    return Math.min(s.feePpm, atEma);
}

/**
 * `VenueState::exit_risk_cap_bps` (AIR-10): the fee cap of a position's exit risk view, which
 * `liquidate`'s assessment, the keeper exits' `B̂_ref` and `sol_settle` price at — the position's
 * locked `hardCapBps` and, on PumpSwap once the fee at the stored `P_ema`'s market cap resolved,
 * at most `⌈φ(P_ema)/100⌉` bps (a sale earlier in the exit's transaction raises the live tier, not
 * that one). Elsewhere the hard cap alone. The execution quote keeps the live fee.
 */
export function exitRiskCapBps(s: VenueState, hardCapBps: number): number {
    const atEma = s.kind === 'pump_swap' ? (s.pool?.feePpmAtEma ?? U32_MAX) : U32_MAX;
    return atEma === U32_MAX ? hardCapBps : Math.min(hardCapBps, Math.ceil(atEma / 100));
}

type WasmVenue = MathModule['Venue']['prototype'];

function poolVenue(m: MathModule, p: PoolPricing): WasmVenue {
    switch (p.type) {
        case 'pumpSwap': {
            const f = new m.Fees(p.fees.lp, p.fees.protocol, p.fees.creator);
            try {
                return m.Venue.pumpSwap(f);
            } finally {
                f.free();
            }
        }
        case 'rayV4':
            return m.Venue.rayV4(p.num, p.den);
        case 'cpmm':
            return m.Venue.cpmm(
                p.trade,
                p.protocol,
                p.fund,
                p.creator,
                p.enableCreatorFee,
                p.creatorFeeOn,
                p.solIsToken0,
            );
    }
}

function curveVenue(m: MathModule, c: CurvePricingInputs, poolTokens: bigint): WasmVenue {
    const f = new m.Fees(c.fees.lp, c.fees.protocol, c.fees.creator);
    const sch = new m.PammSchedule(
        PUMP_CURVE_MCAP_SUPPLY,
        c.schedules.poolCreatorFeeConfigurable,
        0,
        c.schedules.creatorIsDefault,
    );
    try {
        for (const t of c.schedules.poolTiers) sch.addTier(t.threshold, t.lp, t.protocol, t.creator);
        // No post-complete quote and B̂_str above rT until the web follows the v3 adapter
        // (pump-final §8, P5: postCompleteQuoteIn and withSynthetic).
        return m.Venue.pumpCurve(f, c.offsetToken, c.offsetQuote, c.migrationFee, poolTokens, 0n, sch);
    } finally {
        f.free();
        sch.free();
    }
}

/**
 * Runs `fn` with the facade's venue of `s` (a pool's resolved pricing, or the curve: `live` uses
 * the live leg's `X_p`, else the deterministic `STANDARD_POOL_TOKENS` of the EMA leg), capped at
 * `capBps` when given (the risk view, V2-1), and frees it. `null` without pricing.
 */
export function withVenue<T>(
    m: MathModule,
    s: VenueState,
    opts: { readonly live: boolean; readonly capBps?: number | undefined },
    fn: (v: WasmVenue) => T,
): T | null {
    let v: WasmVenue | null = null;
    if (s.pricing) v = poolVenue(m, s.pricing);
    else if (s.curve?.pricing)
        v = curveVenue(m, s.curve.pricing, opts.live ? s.curve.pricing.livePoolTokens : STANDARD_POOL_TOKENS);
    if (!v) return null;
    let use = v;
    try {
        if (opts.capBps !== undefined) use = v.capped(opts.capBps);
        return fn(use);
    } finally {
        if (use !== v) use.free();
        v.free();
    }
}

export interface Buy {
    readonly cost: bigint;
    readonly net: bigint;
}
export interface Sell {
    readonly proceeds: bigint;
    readonly tokenAfter: bigint;
    readonly solAfter: bigint;
}

function takeBuy(q: { cost: bigint; net: bigint; free(): void } | undefined): Buy | null {
    if (!q) return null;
    const out = { cost: q.cost, net: q.net };
    q.free();
    return out;
}

function curveStateOf(m: MathModule, c: CurveLive) {
    return new m.CurveState(c.vt, c.vq, c.rt, c.rq, c.complete);
}

/** Exact-in sale of `q` at the live fee (`VenueState::sell`, `S_exp`). */
export function quoteSell(m: MathModule, s: VenueState, q: bigint): Sell | null {
    if (!s.executable) return null;
    return withVenue(m, s, { live: true }, (v) => {
        let r: { proceeds: bigint; tokenAfter: bigint; solAfter: bigint; free(): void } | undefined;
        if (s.curve) {
            const cs = curveStateOf(m, s.curve.live);
            try {
                r = v.sellCurve(q, cs);
            } finally {
                cs.free();
            }
        } else r = v.sell(q, s.x, s.y, s.r);
        if (!r) return null;
        const out = { proceeds: r.proceeds, tokenAfter: r.tokenAfter, solAfter: r.solAfter };
        r.free();
        return out;
    });
}

/** `B̂_live(q)` at the live fee, or at `min(φ_live, capBps)` (`VenueState::buy` / `buy_risk`). */
export function quoteBuy(m: MathModule, s: VenueState, q: bigint, capBps?: number): Buy | null {
    if (!s.executable) return null;
    return withVenue(m, s, { live: true, capBps }, (v) => {
        if (s.curve) {
            const cs = curveStateOf(m, s.curve.live);
            try {
                return takeBuy(v.buyCurve(q, cs));
            } finally {
                cs.free();
            }
        }
        if (s.x === 0n) return null;
        return takeBuy(v.buy(q, s.x, s.y));
    });
}

/**
 * `B̂(q; x, y)` on a hypothetical state — a ring entry's cached reprojection (the EMA leg, the D4
 * margin): a pool on `(x, y)`; the curve at `(vT, vQ) = (x, y)` with the live offsets and the
 * deterministic migration (`Quoter::buy_xy`). `capBps` gives the risk view.
 */
export function quoteBuyAt(m: MathModule, s: VenueState, q: bigint, x: bigint, y: bigint, capBps?: number): Buy | null {
    if (x === 0n) return null;
    return withVenue(m, s, { live: false, capBps }, (v) => {
        const c = s.curve?.pricing;
        if (s.curve && c) {
            const rt = x > c.offsetToken ? x - c.offsetToken : 0n;
            const rq = y > c.offsetQuote ? y - c.offsetQuote : 0n;
            const cs = new m.CurveState(x, y, rt, rq, false);
            try {
                return takeBuy(v.buyCurve(q, cs));
            } finally {
                cs.free();
            }
        }
        return takeBuy(v.buy(q, x, y));
    });
}

/**
 * The state right after a sale of `q` with quote `sale` (the open's step 9 re-read): the pricing
 * reserves the quote leaves, the pump venues' fees re-resolved on them (the tier follows the
 * market cap), the curve's real reserves moved with its virtual ones (constant offsets).
 */
export function afterSale(m: MathModule, s: VenueState, q: bigint, sale: Sell): VenueState {
    if (s.curve?.pricing) {
        const c = s.curve.pricing;
        const live: CurveLive = {
            vt: sale.tokenAfter,
            vq: sale.solAfter,
            rt: s.curve.live.rt + q,
            rq: sale.solAfter - c.offsetQuote,
            complete: false,
        };
        const fees = resolveCurveFees(m, c.schedules, live.vt, live.vq) ?? c.fees;
        return {
            ...s,
            x: live.vt,
            y: live.vq,
            r: live.rq,
            feePpm: onewayPpm(m, fees),
            curve: { ...s.curve, live, pricing: { ...c, fees } },
        };
    }
    if (s.pricing?.type === 'pumpSwap' && s.pool?.schedule) {
        const fees = resolvePammFees(m, s.pool.schedule, sale.solAfter, sale.tokenAfter) ?? s.pricing.fees;
        return {
            ...s,
            x: sale.tokenAfter,
            y: sale.solAfter,
            pricing: { type: 'pumpSwap', fees },
            feePpm: onewayPpm(m, fees),
        };
    }
    return { ...s, x: sale.tokenAfter, y: sale.solAfter, r: sale.solAfter };
}

/** The live curve's real token reserve `rT` (a close of `q > rT` fails `CurveStraddle`). */
export function curveRealTokens(s: VenueState): bigint | null {
    return s.kind === 'pump_curve' && s.curve ? s.curve.live.rt : null;
}
