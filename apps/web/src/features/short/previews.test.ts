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
import { type FeeBps, notExecutable, quoteBuy, spotPrice } from '../../venues/state.ts';
import {
    assignTier,
    capsFor,
    openBlocked,
    positionMetrics,
    previewAddMargin,
    previewCloseFor,
    previewOpen,
    withTolerance,
} from './previews.ts';
import { type AccountSource, type ChainSnapshot, decodePosition, exposureQ } from './snapshot.ts';
import { programVectors, pumpVectors, testMath, type VectorCase, vectorCase, vectorSnapshot } from './testing.ts';

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
    // The mock venue's opens and pump v3's on near166 (`sell_v3` on the 2026-10-13 ELF).
    const opens = [...vectors.cases, ...pumpVectors()].filter((c) => c.kind === 'open');

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
                expect(math.spotNearEma(spot, podU128ToBigInt(s.market.emaPrice), s.config.params.openEmaDevBps)).toBe(
                    true,
                );
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

    // pump v3 (design pump-final §5.2, §8): the release build on pump's 2026-10-13 ELF. Below `rT`
    // the curve leg; past it the crowd bought the curve down to q/2 and one `close_short` bought rT on
    // the curve and the rest from the would-be pool, completing it. The preview is the program's own
    // quote `B̂_live` (`B̂_v3` past rT), at most §5.2's 7 lamports above pump's charge, and the
    // payout what the program paid less that slack (FE-3 closed: no CurveStraddle).
    for (const name of ['pump-close-below-rt', 'pump-close-past-rt']) {
        it(`${name} (pump v3)`, async () => {
            const c = vectorCase(name);
            const { s, get } = await snapshotOf(c);
            const pos = decodePosition(get(c.position as Address));
            if (!pos) throw new Error('position');
            const e = c.result.event ?? {};
            expect(c.result.ok).toBe(true);
            expect(s.kind).toBe('pump_curve');
            expect(s.venue.executable).toBe(true);
            expect(s.venue.curve?.pricing?.syntheticFees).not.toBeNull();
            expect(pos.q > (s.venue.curve?.live.rt ?? 0n)).toBe(name === 'pump-close-past-rt');
            const slack = big(e.bLive) - big(e.buybackCost);
            expect(slack >= 0n && slack <= 7n).toBe(true);
            const mx = positionMetrics(math, s, pos);
            expect(mx.rentDue).toBe(big(e.rent));
            expect(mx.buyback).toBe(big(e.bLive));
            expect(mx.insufficientVault).toBe(false);
            const pv = previewCloseFor(math, s, pos, 100);
            expect(pv).not.toBeNull();
            if (!pv) return;
            expect(pv.buyback).toBe(big(e.bLive));
            expect(pv.rent).toBe(big(e.rent));
            expect(pv.rentProtocol).toBe(big(e.rentProtocol));
            expect(pv.payout).toBe(big(e.borrowerPayout) - slack);
            expect(pv.maxBuybackCost >= big(e.bLive)).toBe(true);
            expect(mx.crBps !== null && mx.crBps > BigInt(pos.crLiqBps)).toBe(true);
            if (name !== 'pump-close-past-rt') return;
            // Without the mint's supply there is no synthetic route: no quote past rT, so no
            // preview — "No buyback quote now", not a thin vault.
            const noRoute = {
                ...s,
                venue: {
                    ...s.venue,
                    curve: s.venue.curve && {
                        ...s.venue.curve,
                        pricing: s.venue.curve.pricing && { ...s.venue.curve.pricing, syntheticFees: null },
                    },
                },
            };
            const none = positionMetrics(math, noRoute, pos);
            expect(none.buyback).toBeNull();
            expect(none.insufficientVault).toBe(false);
            expect(previewCloseFor(math, noRoute, pos, 100)).toBeNull();
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

describe("the risk view equals liquidate past a curve's rT (pump v3)", () => {
    // T-SYNTH-2 (design pump-final §5.2, §8): ECON's D@80 % position on the fresh curve, the crowd's
    // buy down to rT = q/2, and the keeper's `liquidate` landing on the curve in the second dumped
    // here (the release build on pump's 2026-10-13 ELF). Its event is the program's risk view
    // (`liquidate::assess`): the execution quote `B̂_v3` (`b_live`), `B̂_v3` at the position's exit
    // risk cap (`b_live_risk`, `VenueState::buy_risk`) and the EMA leg's CR on the newest ring entry
    // (`cr_ema_bps`, the capped `buy_xy`). The position's metrics reproduce each to the lamport and
    // the basis point. (The event's spot CR is on the robust spot at `max(L_live, L_ema')`, which
    // the web does not model: its spot CR is the live risk quote's, below `L` here as well.)
    it('pump-liquidate-past-rt', async () => {
        const c = vectorCase('pump-liquidate-past-rt');
        const { s, get } = await snapshotOf(c);
        const pos = decodePosition(get(c.position as Address));
        if (!pos) throw new Error('position');
        const e = c.result.event ?? {};
        expect(c.result.ok).toBe(true);
        expect(s.kind).toBe('pump_curve');
        expect(s.venue.executable).toBe(true);
        expect(s.venue.curve?.pricing?.syntheticFees).not.toBeNull();
        expect(pos.q > (s.venue.curve?.live.rt ?? pos.q)).toBe(true);
        const mx = positionMetrics(math, s, pos);
        expect(mx.rentDue).toBe(big(e.rent));
        expect(mx.buyback).toBe(big(e.bLive));
        expect(mx.buybackRisk).toBe(big(e.bLiveRisk));
        expect(mx.crEmaBps).toBe(big(e.crEmaBps));
        const L = BigInt(pos.crLiqBps);
        expect(big(e.crSpotBps) < L && big(e.crEmaBps) < L).toBe(true);
        expect(mx.crBps).toBe(math.crBps(mx.vEff, big(e.bLiveRisk)));
        expect(mx.crBps !== null && mx.crBps < L).toBe(true);
        // Already below L at the spot: the liquidation price is the spot itself.
        expect(mx.liqPriceWad).toBe(spotPrice(math, s.venue));
        // pump charged at most §5.2's 7 lamports below the program's quote.
        const slack = big(e.bLive) - big(e.buybackCost);
        expect(slack >= 0n && slack <= 7n).toBe(true);
    });
});

describe('AIR-10: a PumpSwap position is judged at most at the tier at P_ema', () => {
    // As `liquidate::assess`, `sol_settle` and the keeper (`decide::tests::air10_*`): a pool whose
    // live tier is 50 bps and whose tier at `P_ema`'s market cap is 30 bps prices the exit risk view
    // (the CR legs, the liquidation price) at 30; the execution quote keeps the live 50.
    it('the risk quote, both CR legs and the liquidation price take the exit risk cap', async () => {
        const c = vectors.cases.find((x) => x.kind === 'close');
        if (!c) throw new Error('vector');
        const { s, get } = await snapshotOf(c);
        const pos0 = decodePosition(get(c.position as Address));
        if (!pos0) throw new Error('position');
        // The vector's own reserves, priced as a PumpSwap pool (20 + 5 + 25 bps).
        const fees: FeeBps = { lp: 20, protocol: 5, creator: 25 };
        const onPool = (feePpmAtEma: number) => ({
            ...s,
            kind: 'pump_swap' as const,
            venue: {
                ...notExecutable('pump_swap'),
                executable: true,
                x: s.venue.x,
                y: s.venue.y,
                r: s.venue.r,
                pricing: { type: 'pumpSwap' as const, fees },
                feePpm: 5_000,
                pool: {
                    disableFlags: 0,
                    boostAmount: 0n,
                    virtualPart: 0n,
                    isCashback: false,
                    coinCreator: pos0.borrower,
                    fees,
                    feePpmAtEma,
                    schedule: null,
                },
            },
        });
        const atEma = onPool(3_000);
        expect(pos0.venueFeeHardCapBps).toBeGreaterThanOrEqual(50);
        const quote = (cap?: number) => quoteBuy(math, atEma.venue, pos0.q, cap)?.cost ?? 0n;
        const [b30, b50] = [quote(30), quote(50)];
        expect(b30 > 0n && b30 < b50).toBe(true);
        expect(quote()).toBe(b50);
        // The smallest vault whose CR at the 30-bps quote is the position's `L`: healthy at the
        // tier at `P_ema`, below `L` at the live tier.
        const L = BigInt(pos0.crLiqBps);
        const due = positionMetrics(math, atEma, pos0).rentDue;
        const vEff = (L * b30 + 9_999n) / 10_000n;
        const edge = { ...pos0, v: vEff + due };
        const mx = positionMetrics(math, atEma, edge);
        expect(mx.buyback).toBe(b50);
        expect(mx.buybackRisk).toBe(b30);
        expect(mx.crBps).toBe(math.crBps(vEff, b30));
        expect(mx.crBps !== null && mx.crBps >= L).toBe(true);
        const healthy = positionMetrics(math, atEma, pos0);
        // The fee at `P_ema` unresolved (no price, a schedule that does not resolve), or above the
        // live tier: the hard cap alone, i.e. the live 50 bps here (the web before this fix).
        for (const other of [0xffff_ffff, 8_000]) {
            const at = onPool(other);
            const m50 = positionMetrics(math, at, edge);
            expect(m50.buybackRisk).toBe(b50);
            expect(m50.crBps !== null && m50.crBps < L).toBe(true);
            const h50 = positionMetrics(math, at, pos0);
            expect(h50.crEmaBps !== null && healthy.crEmaBps !== null && h50.crEmaBps < healthy.crEmaBps).toBe(true);
            expect(
                h50.liqPriceWad !== null && healthy.liqPriceWad !== null && h50.liqPriceWad < healthy.liqPriceWad,
            ).toBe(true);
        }
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
