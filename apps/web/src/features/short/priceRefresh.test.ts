/** Chain-derived recovery states and the real crank instruction/review safety boundary. */
import { bigIntToPodU128, getCrankPriceInstructionDataDecoder, podU128ToBigInt } from '@shortwin/client';
import { AccountRole } from '@solana/kit';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import type { MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import type { SimulationReport } from '../../tx/types.ts';
import { COVERAGE_GAP_S, MARKET_FLAG } from '../../venues/constants.ts';
import { openBlocked } from './previews.ts';
import { freshPriceRefreshPlan, priceRefreshPlan, priceRefreshStep } from './priceRefresh.ts';
import { type ChainSnapshot, loadSnapshot } from './snapshot.ts';
import { testMath, vectorCase, vectorSnapshot } from './testing.ts';

vi.mock('./snapshot.ts', async (original) => ({
    ...(await original<typeof import('./snapshot.ts')>()),
    loadSnapshot: vi.fn(),
}));

let math: MathModule;
let base: ChainSnapshot;
beforeAll(async () => {
    math = testMath();
    base = (await vectorSnapshot(vectorCase('spl-open-0'))).s;
});

function stale(): ChainSnapshot {
    return { ...base, now: base.now + COVERAGE_GAP_S + 1n };
}

function recorded(): ChainSnapshot {
    const s = stale();
    return { ...s, market: { ...s.market, emaTs: s.now, bSec: s.now, bLo: s.market.emaPrice } };
}

function report(s: ChainSnapshot, values: Record<string, bigint>): SimulationReport {
    return {
        ok: true,
        unitsConsumed: 1n,
        computeUnitLimit: 1,
        priorityFeeMicroLamports: 0n,
        feeLamports: 5_000n,
        sizeBytes: 1,
        logs: [],
        error: null,
        simulatedAt: Date.now(),
        deltas: Object.entries(values).map(([field, after]) => ({
            key: `${s.marketAddress}:${field}`,
            label: field,
            before: null,
            after,
            unit: '',
            scale: 0,
        })),
    };
}

describe('idle-market price refresh', () => {
    it('fails closed when an RPC snapshot used a wall-clock fallback or the fresh Clock is missing', async () => {
        const s = { ...stale(), clockVerified: false };
        expect(priceRefreshStep(math, s)).toBeNull();
        await expect(priceRefreshPlan(math, s, s.mint, 'MOCK')).rejects.toThrow(/No price refresh/);
        vi.mocked(loadSnapshot).mockResolvedValue({ snapshot: stale(), get: () => null });
        await expect(freshPriceRefreshPlan({} as AppRpc, math, s.mint, s.mint, 'MOCK')).rejects.toThrow(
            /chain clock could not be read/,
        );
    });

    it('keeps the 40 s boundary fresh, and records when the pending observation is stale or missing', () => {
        const atBoundary = { ...base, now: base.now + COVERAGE_GAP_S };
        // Pin both timestamps to avoid the vector’s preceding-second bucket affecting the boundary.
        const m = { ...base.market, emaTs: base.now, lbSec: base.now };
        expect(priceRefreshStep(math, { ...atBoundary, market: m })).toBeNull();
        expect(priceRefreshStep(math, { ...atBoundary, now: atBoundary.now + 1n, market: m })).toBe('record');
        expect(priceRefreshStep(math, stale())).toBe('record');
        expect(priceRefreshStep(math, { ...stale(), market: { ...base.market, bLo: bigIntToPodU128(0n) } })).toBe(
            'record',
        );
    });

    it('waits for an actual later Clock second, then integrates; a monotone accrual clock alone is insufficient', () => {
        const s = recorded();
        expect(priceRefreshStep(math, s)).toBe('wait');
        expect(priceRefreshStep(math, { ...s, market: { ...s.market, lastAccrualTs: s.now + 1n } })).toBe('wait');
        expect(priceRefreshStep(math, { ...s, now: s.now + 1n })).toBe('integrate');
        // The open can roll the new bucket, but its UI quote still needs the newly integrated EMA.
        expect(openBlocked(math, { ...s, now: s.now + 1n })).toBeNull();
    });

    it('recognises another caller’s completed refresh and restarts if the pending sample expires', () => {
        const s = recorded();
        const fresh = { ...s, now: s.now + 1n, market: { ...s.market, emaTs: s.now + 1n, lbSec: s.now } };
        expect(priceRefreshStep(math, fresh)).toBeNull();
        expect(priceRefreshStep(math, { ...s, now: s.now + COVERAGE_GAP_S + 1n })).toBe('record');
    });

    it('preserves listing warmup, coverage and venue prerequisites', async () => {
        expect(priceRefreshStep(math, (await vectorSnapshot(vectorCase('warmup-open'))).s)).toBeNull();
        const s = stale();
        expect(priceRefreshStep(math, { ...s, market: { ...s.market, flags: MARKET_FLAG.pausedCoverage } })).toBeNull();
        expect(priceRefreshStep(math, { ...s, venue: { ...s.venue, executable: false } })).toBeNull();
    });

    it('does not offer repeated cranks to bypass a fresh price-deviation refusal', () => {
        const s = {
            ...base,
            market: { ...base.market, emaPrice: bigIntToPodU128(podU128ToBigInt(base.market.emaPrice) * 10n) },
        };
        expect(openBlocked(math, s)?.code).toBe('PriceDeviatesFromEma');
        expect(priceRefreshStep(math, s)).toBeNull();
    });

    it('builds exactly one real crank with read-only venue accounts and separate fee disclosure', async () => {
        const s = stale();
        const plan = await priceRefreshPlan(math, s, s.mint, 'MOCK');
        expect(plan.kind).toBe('crank_price');
        expect(plan.instructions).toHaveLength(1);
        const ix = plan.instructions[0];
        expect(() => getCrankPriceInstructionDataDecoder().decode(ix?.data ?? new Uint8Array())).not.toThrow();
        expect(ix?.accounts?.slice(-s.readKeys.length)).toEqual(
            s.readKeys.map((address) => ({ address, role: AccountRole.READONLY })),
        );
        expect(plan.disclosures).toEqual(['market.priceRefresh']);
        expect(plan.watch.map((w) => w.kind)).toEqual(['lamports', 'program']);
        expect(plan.holdSign?.(report(s, { bSec: s.now - 1n }))).toMatch(/did not record/);
        expect(plan.holdSign?.(report(s, { bSec: s.now }))).toBeNull();
    });

    it('refuses a same-second second transaction, and holds Sign if simulation did not integrate the fresh bucket', async () => {
        const s = recorded();
        await expect(priceRefreshPlan(math, s, s.mint, 'MOCK')).rejects.toThrow(/chain clock/);
        const later = { ...s, now: s.now + 1n };
        const plan = await priceRefreshPlan(math, later, s.mint, 'MOCK');
        expect(plan.title).toMatch(/^Update smoothed price/);
        expect(plan.holdSign?.(report(later, { bSec: later.now, lbSec: s.now - 1n, emaTs: later.now }))).toMatch(
            /not been integrated/,
        );
        expect(plan.holdSign?.(report(later, { bSec: later.now, lbSec: s.now, emaTs: s.now }))).toMatch(
            /not been integrated/,
        );
        expect(plan.holdSign?.(report(later, { bSec: later.now, lbSec: s.now, emaTs: later.now }))).toBeNull();
        expect(
            plan.holdSign?.(
                report(later, {
                    bSec: s.now + COVERAGE_GAP_S + 1n,
                    lbSec: s.now,
                    emaTs: s.now + COVERAGE_GAP_S + 1n,
                }),
            ),
        ).toMatch(/expired.*Close and review/);
        await expect(priceRefreshPlan(math, base, base.mint, 'MOCK')).rejects.toThrow(/No price refresh/);
    });
});
