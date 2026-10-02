/**
 * The WASM math facade is wired in: the browser build of crates/shortwin-math-wasm loads in Node
 * and computes the SPEC's numbers (FRONTEND.md §6.7).
 */
import { beforeAll, describe, expect, it } from 'vitest';
import { readRepoBytes } from '../test/repo.ts';
import { defaultTierTerms, initMathSync, type MathModule } from './load.ts';

let m: MathModule;

beforeAll(() => {
    m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
});

describe('@shortwin/math-wasm', () => {
    it('loads and reports its version', () => {
        expect(m.version()).toMatch(/^\d+\.\d+\.\d+/);
    });

    it('carries the SPEC §4 tier table', () => {
        expect(defaultTierTerms(m, 'A')).toMatchObject({
            crOpenBps: 20_000,
            crLiqBps: 12_500,
            penaltyBps: 500,
            maxDurationS: 30 * 86_400,
        });
        expect(defaultTierTerms(m, 'D')).toMatchObject({
            crOpenBps: 40_000,
            crLiqBps: 17_500,
            penaltyBps: 1_000,
            maxDurationS: 72 * 3_600,
        });
    });

    it('computes the D4 margin and the open fee', () => {
        // S = 1 SOL of proceeds, buyback 1.02 SOL at spot and 1.05 at EMA, Tier C opening CR 300 %:
        // M = ⌈3.00 · max(1.02, 1.05)⌉ − 1 = 2.15 SOL.
        expect(m.marginD4(1_000_000_000n, 1_020_000_000n, 1_050_000_000n, 30_000)).toBe(2_150_000_000n);
        expect(m.openFee(1_000_000_000n, 25)).toBe(2_500_000n);
    });

    it('computes a utilisation-kinked rent rate', () => {
        expect(m.utilizationBps(500n, 500n)).toBe(5_000);
        // Tier A: base 10 %, 50 % at the 80 % kink → slope1 = 50 %; 80 % utilisation → 50 % APR.
        expect(m.rateBps(1_000, 5_000, 100_000, 8_000, 8_000)).toBe(5_000);
    });
});
