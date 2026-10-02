import { describe, expect, it } from 'vitest';
import { readRepo } from '../../../test/repo.ts';
import { AGE_A_S, AGE_C_S, explainTier, oracleUsd, type TierWhyInput } from './tierWhy.ts';

const NOW = 1_800_000_000;
// $121.20 per SOL, Pyth style (the math crate's tier tests).
const ORACLE = { price: 12_120_000_000n, conf: 0n, expo: -8, publishTime: NOW - 30 };
const base: TierWhyInput = {
    venue: 'raydium_cpmm',
    tier: 'B',
    now: NOW,
    graduatedAt: NOW - 3 * 86_400,
    createdTs: NOW - 3 * 86_400,
    thetaAUsdE6: 500_000_000_000n,
    thetaBUsdE6: 100_000_000_000n,
    oracle: ORACLE,
    oracleStaleS: 600,
};

describe('explainTier', () => {
    it('uses the math crate’s age thresholds', () => {
        const src = readRepo('crates/shortwin-math/src/tier.rs');
        expect(src).toContain(`pub const AGE_C_S: i64 = ${AGE_C_S.toLocaleString('en-US').replaceAll(',', '_')};`);
        expect(src).toContain(`pub const AGE_A_S: i64 = ${AGE_A_S.toLocaleString('en-US').replaceAll(',', '_')};`);
    });

    it('explains the age cap and its basis (O13)', () => {
        const listed = explainTier(base);
        expect(listed.ageCap).toBe('B');
        expect(listed.ageBasis).toBe('listing');
        expect(listed.lines.find((l) => l.key === 'age')?.effect).toMatch(/At most Tier B until/);
        const young = explainTier({ ...base, graduatedAt: NOW - 3_600, createdTs: NOW - 3_600 });
        expect(young.ageCap).toBe('C');
        const old = explainTier({ ...base, graduatedAt: NOW - 30 * 86_400, createdTs: NOW - 86_400 });
        expect(old.ageCap).toBeNull();
        expect(old.ageBasis).toBe('pool-creation');
        const graduated = explainTier({ ...base, graduatedAt: NOW - 86_400 * 2, createdTs: NOW - 86_400 * 10 });
        expect(graduated.ageBasis).toBe('graduation');
        expect(explainTier({ ...base, graduatedAt: null }).ageCap).toBe('C');
    });

    it('converts the thresholds at the oracle price (ARCH §4.7: $500k ≈ 4,125 SOL at $121.2)', () => {
        const why = explainTier(base);
        const liq = why.lines.find((l) => l.key === 'liquidity')?.value ?? '';
        expect(liq).toContain('$500,000');
        expect(liq).toContain('≈ 4,125 SOL');
        expect(liq).toContain('≈ 825 SOL');
        expect(oracleUsd(12_120_000_000n, -8)).toBe('121.2');
    });

    it('flags a stale oracle and the curve’s fixed tier', () => {
        const stale = explainTier({ ...base, oracle: { ...ORACLE, publishTime: NOW - 3_600 } });
        expect(stale.lines.find((l) => l.key === 'oracle')?.effect).toMatch(/fall back to C/);
        // An OracleCache that was never cranked has no reading to age (localnet, a fresh deploy).
        const never = explainTier({ ...base, oracle: { ...ORACLE, price: 0n, publishTime: 0 } });
        expect(never.lines.find((l) => l.key === 'oracle')?.value).toBe('No reading yet');
        const curve = explainTier({ ...base, venue: 'pump_curve', tier: 'D' });
        expect(curve.lines.map((l) => l.key)).toEqual(['venue']);
        expect(curve.lines[0]?.effect).toMatch(/Tier D/);
    });
});
