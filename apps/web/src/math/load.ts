/**
 * The WASM math facade (`@shortwin/math-wasm`, built from crates/shortwin-math-wasm; FRONTEND.md
 * §6.7): the same `shortwin-math` the program and the keeper run, so previews equal what the
 * program computes. Loaded once, lazily; `initMathSync` is for Node tests.
 */
import init, * as wasm from '@shortwin/math-wasm';

export type MathModule = typeof wasm;

let loading: Promise<MathModule> | null = null;

/** Initialises the module (browser: fetches the .wasm asset Vite emitted). */
export function loadMath(): Promise<MathModule> {
    if (!loading) {
        loading = init().then(() => wasm);
        loading.catch(() => {
            loading = null;
        });
    }
    return loading;
}

/** Synchronous initialisation from the module's bytes (tests: read pkg/shortwin_math_wasm_bg.wasm). */
export function initMathSync(bytes: BufferSource): MathModule {
    wasm.initSync({ module: bytes });
    loading = Promise.resolve(wasm);
    return wasm;
}

/** Tier codes as the math crate numbers them (`shortwin_math::tier::Tier`). */
export const TIER_CODE = { A: 0, B: 1, C: 2, D: 3 } as const;

export interface TierTerms {
    readonly crOpenBps: number;
    readonly crLiqBps: number;
    readonly penaltyBps: number;
    readonly maxDurationS: number;
    readonly maxLiqSlipBps: number;
    readonly minRentS: number;
    readonly impactPosBps: number;
    readonly impactMktBps: number;
    readonly rateBaseBps: number;
    readonly rateSlope1Bps: number;
    readonly rateSlope2Bps: number;
}

/**
 * The compiled default terms of a tier (SPEC §4 / §6.1 tables). For display and explanation only:
 * previews use the live `GlobalConfig` terms and a position's locked terms (FRONTEND.md §6.7).
 */
export function defaultTierTerms(m: MathModule, tier: keyof typeof TIER_CODE): TierTerms {
    const p = m.tierParams(TIER_CODE[tier]);
    try {
        return {
            crOpenBps: p.crOpenBps,
            crLiqBps: p.crLiqBps,
            penaltyBps: p.penaltyBps,
            maxDurationS: p.maxDurationS,
            maxLiqSlipBps: p.maxLiqSlipBps,
            minRentS: p.minRentS,
            impactPosBps: p.impactPosBps,
            impactMktBps: p.impactMktBps,
            rateBaseBps: p.rateBaseBps,
            rateSlope1Bps: p.rateSlope1Bps,
            rateSlope2Bps: p.rateSlope2Bps,
        };
    } finally {
        p.free();
    }
}
