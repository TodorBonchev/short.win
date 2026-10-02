import { describe, expect, it } from 'vitest';
import { computeUnitLimitFor, feeLamports, SIMULATION_CU_LIMIT } from './message.ts';
import { clampFee, percentile } from './priorityFee.ts';

describe('compute budget and fees', () => {
    it('sizes the compute-unit limit from the simulation with headroom and bounds', () => {
        expect(computeUnitLimitFor(200_000n)).toBe(240_000);
        expect(computeUnitLimitFor(1_000n)).toBe(20_000);
        expect(computeUnitLimitFor(1_390_000n)).toBe(SIMULATION_CU_LIMIT);
        expect(computeUnitLimitFor(null)).toBe(400_000);
    });

    it('computes base plus priority fee, rounding the priority part up', () => {
        expect(feeLamports(1, 200_000, 0n)).toBe(5_000n);
        expect(feeLamports(1, 200_000, 10_000n)).toBe(5_000n + 2_000n);
        expect(feeLamports(1, 3, 1n)).toBe(5_001n);
    });

    it('takes percentiles of recent fees and clamps them', () => {
        expect(percentile([], 75)).toBe(0n);
        expect(percentile([5n, 1n, 3n, 2n, 4n], 75)).toBe(4n);
        expect(percentile([5n, 1n, 3n, 2n, 4n], 95)).toBe(5n);
        expect(clampFee(10n, 1_000n, 100_000n)).toBe(1_000n);
        expect(clampFee(10_000_000n, 1_000n, 100_000n)).toBe(100_000n);
    });
});
