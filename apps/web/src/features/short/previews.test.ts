/**
 * Preview == program math (FRONTEND.md §10.1 SHORT, §6.7): every case of
 * `vectors/program-vectors.json` is the program itself (the TEST-ONLY test-venue build on the
 * LiteSVM engine harness, the mock venue), with every account the previews read dumped right after
 * a crank in the instruction's own second — so the previews computed here from those accounts with
 * the WASM facade must equal what `open_short` / `close_short` / `add_margin` booked, to the
 * lamport, and must predict each refusal's program error.
 */
import { findExposurePda, podU128ToBigInt } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { beforeAll, describe, expect, it } from 'vitest';
import { programError } from '../../config/errors.ts';
import type { MathModule } from '../../math/load.ts';
import {
    assignTier,
    capsFor,
    openBlocked,
    positionMetrics,
    previewAddMargin,
    previewCloseFor,
    previewOpen,
    spotNearEma,
    withTolerance,
} from './previews.ts';
import { type AccountSource, type ChainSnapshot, decodePosition, exposureQ } from './snapshot.ts';
import { programVectors, testMath, type VectorCase, vectorSnapshot } from './testing.ts';

type Case = VectorCase;
const vectors = { cases: programVectors() };
let math: MathModule;

beforeAll(() => {
    math = testMath();
});

const snapshotOf = vectorSnapshot;

async function exposureOf(c: Case, s: ChainSnapshot, get: AccountSource): Promise<bigint> {
    const [pda] = await findExposurePda({ market: s.marketAddress, borrower: c.borrower as Address });
    return exposureQ(get(pda));
}

const big = (v: string | number | undefined) => BigInt(v ?? 0);
const errName = (code: number | null | undefined) => (code ? (programError(code)?.name ?? String(code)) : null);

describe('the open preview equals open_short', () => {
    const opens = vectors.cases.filter((c) => c.kind === 'open');

    it('covers successful opens on SPL and Token-2022 and refusals', () => {
        expect(opens.filter((c) => c.result.ok).length).toBeGreaterThanOrEqual(4);
        expect(new Set(opens.filter((c) => !c.result.ok).map((c) => errName(c.result.code)))).toEqual(
            new Set(['CapPositionImpact', 'CapWallet', 'PriceDeviatesFromEma', 'MarketWarmingUp']),
        );
    });

    for (const c of opens) {
        it(`${c.name} (q = ${c.q})`, async () => {
            const { s, get } = await snapshotOf(c);
            const walletQ = await exposureOf(c, s, get);
            const blocked = openBlocked(math, s);
            const preview = previewOpen(math, s, { q: big(c.q), slippageBps: 100, walletQ });
            if (c.result.ok) {
                const e = c.result.event ?? {};
                const pos = c.result.position ?? {};
                expect(blocked).toBeNull();
                expect(preview).not.toBeNull();
                if (!preview) return;
                expect(preview.refusal).toBeNull();
                expect(preview.tier).toBe(Number(e.tier));
                expect(preview.proceeds).toBe(big(e.s));
                expect(preview.nOpen).toBe(big(e.nOpen));
                expect(preview.bEma).toBe(big(e.bEma));
                expect(preview.margin).toBe(big(e.m));
                expect(preview.openFee).toBe(big(e.feeOpen));
                expect(preview.vault).toBe(big(e.v));
                expect(preview.minRent).toBe(big(pos.rMin));
                expect(preview.deadline).toBe(big(e.deadline));
                // The bounds the panel signs with admit exactly this execution.
                expect(preview.minProceeds <= preview.proceeds).toBe(true);
                expect(preview.maxMargin).toBe(withTolerance(big(e.m), 200n));
                expect(preview.maxOpenFee >= big(e.feeOpen)).toBe(true);
                // The opening CR is at least the tier's opening CR (D4).
                const tp = s.config.tiers[preview.tier];
                expect(preview.crOpenBps >= BigInt(tp?.crOpenBps ?? 0)).toBe(true);
                // The position's caps include this size.
                expect(preview.caps && preview.q <= preview.caps.maxQ).toBe(true);
                // A liquidation price above the spot exists for a healthy open.
                const spot = big(e.spotWad);
                expect(preview.liqPriceWad !== null && preview.liqPriceWad > spot).toBe(true);
                expect(spotNearEma(spot, podU128ToBigInt(s.market.emaPrice), s.config.params.openEmaDevBps)).toBe(true);
            } else {
                const want = errName(c.result.code);
                const got = blocked?.code ?? preview?.refusal ?? null;
                expect(got).toBe(want);
            }
        });
    }

    it('the binding cap of the next open names the refusal just above it', async () => {
        const c = opens.find((x) => x.name === 'spl-open-position-cap');
        if (!c) throw new Error('vector');
        const { s, get } = await snapshotOf(c);
        const walletQ = await exposureOf(c, s, get);
        const tier = assignTier(math, s);
        const tp = tier === null ? undefined : s.config.tiers[tier];
        if (!tp) throw new Error('tier');
        const caps = capsFor(math, s, tp, walletQ);
        if (!caps) throw new Error('caps');
        const at = previewOpen(math, s, { q: caps.maxQ, slippageBps: 100, walletQ });
        const above = previewOpen(math, s, { q: caps.maxQ + 1n, slippageBps: 100, walletQ });
        expect(at?.refusal).toBeNull();
        expect(above?.refusalCap).toBe(caps.binding);
    });
});

describe('the close preview equals close_short', () => {
    for (const c of vectors.cases.filter((x) => x.kind === 'close')) {
        it(c.name, async () => {
            const { s, get } = await snapshotOf(c);
            const pos = decodePosition(get(c.position as Address));
            if (!pos) throw new Error('position');
            const e = c.result.event ?? {};
            expect(c.result.ok).toBe(true);
            const mx = positionMetrics(math, s, pos);
            expect(mx.rentDue).toBe(big(e.rent));
            expect(mx.buyback).toBe(big(e.bLive));
            const pv = previewCloseFor(math, s, pos, 100);
            expect(pv).not.toBeNull();
            if (!pv) return;
            expect(pv.buyback).toBe(big(e.buybackCost));
            expect(pv.rent).toBe(big(e.rent));
            expect(pv.rentProtocol).toBe(big(e.rentProtocol));
            expect(pv.payout).toBe(big(e.borrowerPayout));
            expect(pv.pnl).toBe(big(e.borrowerPayout) - pos.m - pos.feeOpen);
            expect(pv.maxBuybackCost >= pv.buyback).toBe(true);
            expect(mx.timeLeftS).toBe(pos.deadline - s.now);
            expect(mx.crBps !== null && mx.crBps > BigInt(pos.crLiqBps)).toBe(true);
        });
    }

    it('a rise lowers the P&L below a fall', async () => {
        const rise = vectors.cases.find((x) => x.name === 'spl-close-after-rise');
        const fall = vectors.cases.find((x) => x.name === 'spl-close-after-fall');
        if (!rise || !fall) throw new Error('vectors');
        const pnl = async (c: Case) => {
            const { s, get } = await snapshotOf(c);
            const pos = decodePosition(get(c.position as Address));
            if (!pos) throw new Error('position');
            const pv = previewCloseFor(math, s, pos, 100);
            return pv ? (pv.pnl * 10_000n) / pos.s : 0n;
        };
        expect((await pnl(rise)) < 0n).toBe(true);
        expect((await pnl(fall)) > 0n).toBe(true);
    });
});

describe('the add-margin preview equals add_margin', () => {
    for (const c of vectors.cases.filter((x) => x.kind === 'add_margin')) {
        it(c.name, async () => {
            const { s, get } = await snapshotOf(c);
            const pos = decodePosition(get(c.position as Address));
            if (!pos) throw new Error('position');
            const amount = big(c.amount);
            const after = c.result.position ?? {};
            expect(c.result.ok).toBe(true);
            expect(pos.v + amount).toBe(big(after.v));
            expect(pos.m + amount).toBe(big(after.m));
            const before = positionMetrics(math, s, pos);
            const cr = previewAddMargin(math, s, pos, amount);
            expect(cr).not.toBeNull();
            expect(before.crBps !== null && cr !== null && cr > before.crBps).toBe(true);
            // Exactly the CR of the position as the program left it.
            const later = positionMetrics(math, s, { ...pos, v: big(after.v), m: big(after.m) });
            expect(cr).toBe(later.crBps);
            expect(
                later.liqPriceWad !== null && before.liqPriceWad !== null && later.liqPriceWad > before.liqPriceWad,
            ).toBe(true);
        });
    }
});
