/**
 * `initialize_config`'s parameter arguments from the ECON §15 defaults, read from
 * `@shortwin/math-wasm`'s `ParamSet` by name (the same compiled defaults the program validates), so
 * the seed never carries a hand-copied parameter table. The E2E keeps the defaults: the market
 * warm-up (`market_warmup_s` = 50 s) is already its compiled minimum (10·EMA_TAU_S).
 */
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import type { EmaParamsArgsArgs, ParamsArgs, TierParamsArgs } from '@shortwin/client';
import { initSync, ParamSet } from '@shortwin/math-wasm';

let ready = false;

/** Initialises the WASM module in Node (`initSync` with the `.wasm` bytes). */
export function initMath(): void {
    if (ready) return;
    const require = createRequire(import.meta.url);
    const wasmPath = require.resolve('@shortwin/math-wasm/wasm');
    initSync({ module: readFileSync(wasmPath) });
    ready = true;
}

/** `thetaAUsdE6` → `theta_a_usd_e6`, `rateSlope1Bps` → `rate_slope1_bps` (digits stay on their word). */
const snake = (camel: string): string => camel.replace(/[A-Z]/g, (m) => `_${m.toLowerCase()}`);

/** Field names of `Params`, in the IDL's order (the encoder takes an object, the order is cosmetic). */
const PARAM_FIELDS = [
    'settleKeeperFixed',
    'settleKeeperCap',
    'expiryKeeperFixed',
    'expiryKeeperCap',
    'minNotional',
    'marketMaxNotional',
    'marketCreationFee',
    'boostDustLamports',
    'execRentTol',
    'thetaAUsdE6',
    'thetaBUsdE6',
    'poolMinRealSol',
    'openFeeBps',
    'rentProtocolBps',
    'penaltyLenderBps',
    'penaltyProtocolBps',
    'penaltyKeeperBps',
    'expiryFeeBps',
    'settleKeeperBps',
    'expiryKeeperBps',
    'utilCapBps',
    'walletCapBps',
    'curveOiGammaBps',
    'feeSizeCapX100',
    'feeOiCapX100',
    's3WaitS',
    'gradGraceS',
    'maxMigrationPauseS',
    'boostWindowS',
    'openEmaDevBps',
    'marketWarmupS',
    'emaForceDelayS',
    'settlePersistN',
    'settleArmGapS',
    'fvDevBps',
    'lumpVestS',
    'lockGraceS',
    'oracleMaxAgeS',
    'oracleStaleS',
    'oracleMaxConfBps',
    'oracleMaxStepBps',
    'venueFeeCapBps',
    'venueFeeHardCapBps',
    'poolLockMinBps',
    'poolSwitchMarginBps',
    'poolListWindowS',
    'poolListMarginBps',
    'poolFastRouteS',
    'poolFastMinBps',
    'poolFastWindowS',
    'kappaLiqBps',
    'kappaBaseBps',
    'deltaLiqBps',
    'deltaExpBps',
    'execTolBps',
    'refMaxAgeS',
    'liqPersistN',
    'kappaGrowthN',
] as const;

/** u64 fields of `Params` (the rest are u32 / u16 / u8 and encode from `number`). */
const U64_FIELDS = new Set<string>(PARAM_FIELDS.slice(0, 12));

const TIER_FIELDS = [
    'mBps',
    'crOpenBps',
    'crLiqBps',
    'penaltyBps',
    'maxDurationS',
    'impactPosBps',
    'impactMktBps',
    'maxLiqSlipBps',
    'rateBaseBps',
    'rateSlope1Bps',
    'rateSlope2Bps',
    'minRentS',
] as const;

const EMA_FIELDS = ['emaDtmaxS', 'emaClampBps', 'liqSlowTauS', 'liqSlowDtmaxS'] as const;

function get(ps: ParamSet, camel: string, tier?: number): bigint {
    const name = snake(camel);
    const v = ps.get(name, tier);
    if (v === undefined || v === null) throw new Error(`ParamSet has no parameter ${name}`);
    return BigInt(v);
}

export interface DefaultParams {
    readonly params: ParamsArgs;
    readonly tiers: TierParamsArgs[];
    readonly ema: EmaParamsArgsArgs;
    readonly utilKinkBps: number;
}

/** The ECON §15 defaults (validated by the WASM `ParamSet.validate()` before use). */
export function defaultParams(): DefaultParams {
    initMath();
    const ps = new ParamSet();
    try {
        const bad = ps.validate();
        if (bad) throw new Error(`the default parameter set fails ${bad}`);
        const params = Object.fromEntries(
            PARAM_FIELDS.map((f) => [f, U64_FIELDS.has(f) ? get(ps, f) : Number(get(ps, f))]),
        ) as unknown as ParamsArgs;
        const tiers = [0, 1, 2, 3].map(
            (t) => Object.fromEntries(TIER_FIELDS.map((f) => [f, Number(get(ps, f, t))])) as unknown as TierParamsArgs,
        );
        const ema = Object.fromEntries(EMA_FIELDS.map((f) => [f, Number(get(ps, f))])) as unknown as EmaParamsArgsArgs;
        return { params, tiers, ema, utilKinkBps: Number(get(ps, 'utilKinkBps')) };
    } finally {
        ps.free();
    }
}
