/**
 * Raydium candidates judged as `create_market(venue_hint = Raydium…)` would (ARCH §2.6): `verify`,
 * the venue predicate and `eligible()` at the candidate's own spot, on the dumped Raydium pools of
 * `tests/fixtures` with the protocol's default parameters (ECON §15, the WASM `ParamSet`).
 */
import type { Address } from '@solana/kit';
import { beforeAll, describe, expect, it } from 'vitest';
import { initMathSync, type MathModule } from '../math/load.ts';
import { readRepoBytes } from '../test/repo.ts';
import { candidateReadKeys, discoveryQueries, type EligibilityParams, judgeCandidate } from './candidates.ts';
import { GUARD_MASK, TOKEN_PROGRAM, WSOL_MINT } from './constants.ts';
import type { RawAccount } from './readers.ts';
import { spotPrice } from './state.ts';
import { fixture, fixtureAccount, fixtureAddress, snapshotTime } from './testing.ts';

let m: MathModule;
let params: EligibilityParams;

beforeAll(() => {
    m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
    const p = new m.ParamSet();
    params = {
        venueFeeCapBps: Number(p.get('venue_fee_cap_bps')),
        poolMinRealSol: p.get('pool_min_real_sol') ?? 0n,
        poolLockMinBps: Number(p.get('pool_lock_min_bps')),
    };
    p.free();
});

const accOf = (n: string) => {
    const a = fixtureAccount(n);
    if (!a) throw new Error(n);
    return a;
};

function readFor(kind: 'raydium_v4' | 'raydium_cpmm', poolName: string): (RawAccount | null)[] {
    const pool = accOf(poolName);
    const keys = candidateReadKeys(kind, pool);
    if (!keys) throw new Error('keys');
    const byAddress = new Map<string, string>();
    for (const n of [
        'ray_v4_fartcoin_coin_vault',
        'ray_v4_fartcoin_pc_vault',
        'ray_v4_fartcoin_lp_mint',
        'ray_v4_fartcoin_rev_coin_vault',
        'ray_v4_fartcoin_rev_pc_vault',
        'ray_v4_fartcoin_rev_lp_mint',
        ...['useless', 'creator_fee', 'token2022', 'wsol_token1', 'creator_both'].flatMap((r) => [
            `cpmm_${r}_token_0_vault`,
            `cpmm_${r}_token_1_vault`,
            `cpmm_${r}_lp_mint`,
        ]),
        ...Array.from({ length: 21 }, (_, i) => `cpmm_config_${String(i).padStart(2, '0')}`),
    ]) {
        const a = fixture(n).address;
        if (a) byAddress.set(a, n);
    }
    return [pool, ...keys.slice(1).map((k) => (byAddress.has(k) ? accOf(byAddress.get(k) as string) : null))];
}

describe('raydium candidates', () => {
    it('discovers by both mint orientations on both programs at the verified sizes', () => {
        const qs = discoveryQueries(fixtureAddress('mint_fartcoin'));
        expect(qs).toHaveLength(4);
        expect(qs.map((q) => q.size)).toEqual([752, 752, 637, 637]);
        expect(qs[1]?.filters.map((f) => f.bytes)).toEqual([WSOL_MINT, fixtureAddress('mint_fartcoin')]);
    });

    it('the Fartcoin AMM v4 pool is eligible: verified, executable, the locked-depth floor met', () => {
        const now = snapshotTime('raydium_state');
        const read = readFor('raydium_v4', 'ray_v4_fartcoin_pool');
        const j = judgeCandidate(m, 'raydium_v4', fixtureAddress('mint_fartcoin'), TOKEN_PROGRAM, read, now, params, 0);
        expect(j.reasons).toEqual([]);
        expect(j.eligible).toBe(true);
        expect(j.lambdaBps).toBe(9_978);
        expect(j.feePpm).toBe(2_500);
        expect(j.lockedDepth).toBe(j.state?.lockedDepth);
        const p = j.state ? spotPrice(m, j.state) : null;
        if (p === null || !j.state) throw new Error('spot');
        expect(m.lockedFloorOk(j.lockedDepth, p, params.poolLockMinBps, params.poolMinRealSol)).toBe(true);
        expect(m.lockedSol(j.lockedDepth, p) > 0n).toBe(true);
    });

    it('refuses another mint, a Token-2022 market on AMM v4, and a latched Raydium program', () => {
        const now = snapshotTime('raydium_state');
        const read = readFor('raydium_v4', 'ray_v4_fartcoin_pool');
        const other = judgeCandidate(m, 'raydium_v4', WSOL_MINT, TOKEN_PROGRAM, read, now, params, 0);
        expect(other.reasons).toEqual(['PoolIneligible']);
        const t22 = judgeCandidate(
            m,
            'raydium_v4',
            fixtureAddress('mint_fartcoin'),
            'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb' as Address,
            read,
            now,
            params,
            0,
        );
        expect(t22.eligible).toBe(false);
        const latched = judgeCandidate(
            m,
            'raydium_v4',
            fixtureAddress('mint_fartcoin'),
            TOKEN_PROGRAM,
            read,
            now,
            params,
            GUARD_MASK.raydium_v4 ?? 0,
        );
        expect(latched.reasons).toContain('VenueUpgraded');
    });

    for (const role of ['useless', 'creator_fee', 'token2022', 'wsol_token1', 'creator_both']) {
        it(`CPMM ${role}: verified and judged on its own spot`, () => {
            const now = snapshotTime('raydium_state');
            const pool = accOf(`cpmm_${role}_pool`);
            const mint = fixtureAddress(`cpmm_${role}_token_mint`);
            const program = fixture(`cpmm_${role}_token_mint`).owner as Address;
            const read = readFor('raydium_cpmm', `cpmm_${role}_pool`);
            const j = judgeCandidate(m, 'raydium_cpmm', mint, program, read, now, params, 0);
            expect(j.pool).toBe(pool.address);
            // Verified: either eligible, or refused only on the economics (fee cap, depth floor).
            expect(j.state?.executable).toBe(true);
            for (const r of j.reasons) expect(['PoolIneligible', 'PoolTooThin']).toContain(r);
            const p = j.state ? spotPrice(m, j.state) : null;
            if (p !== null && j.state) {
                const floor = m.lockedFloorOk(j.lockedDepth, p, params.poolLockMinBps, params.poolMinRealSol);
                const fee = m.exceedsCap(j.feePpm, params.venueFeeCapBps);
                expect(j.eligible).toBe(floor && !fee);
            }
        });
    }
});
