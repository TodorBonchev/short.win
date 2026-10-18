/**
 * The venue state of each real venue on the dumped mainnet fixtures (`tests/fixtures`): the venue
 * predicate, the pricing reserves and real SOL, the resolved fee, the locked depth, and quotes that
 * behave (FRONTEND.md §10.1 SHORT: venue-state readers against the fixture accounts). The markets
 * are written as the program's listing writes them (the pool caches, the SOL side, the derived keys).
 */
import type { Address } from '@solana/kit';
import { beforeAll, describe, expect, it } from 'vitest';
import { initMathSync, type MathModule } from '../math/load.ts';
import { readRepoBytes } from '../test/repo.ts';
import { MARKET_FLAG, PAMM_CACHE, RAY_CACHE } from './constants.ts';
import { detectFrom } from './detect.ts';
import type { MarketKeys } from './lists.ts';
import { canonicalPool, pammBoostVault, pammLpMint, poolAuthority, wsolAta } from './pda.ts';
import {
    PAMM_POOL_OFF_CREATOR_FEES,
    PAMM_POOL_OFF_PROTOCOL_FEES,
    type RawAccount,
    readFeeConfig,
    value,
} from './readers.ts';
import {
    afterSale,
    capFeePpm,
    exitRiskCapBps,
    fvRepriced,
    liquidityOf,
    notExecutable,
    type PoolView,
    quoteBuy,
    quoteBuyAt,
    quoteSell,
    readVenueState,
    resolveSyntheticFees,
    spotPrice,
} from './state.ts';
import { fixture, fixtureAccount, fixtureAddress, snapshotTime } from './testing.ts';

let m: MathModule;
beforeAll(() => {
    m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
});

const D = '11111111111111111111111111111111' as Address;
const cache = () => Array.from({ length: 10 }, () => D);
const dec = (n: string) => fixture(n).decoded as Record<string, unknown>;
const big = (v: unknown) => BigInt(v as string | number);

function market(fields: Partial<MarketKeys>): MarketKeys {
    return {
        mint: D,
        tokenProgram: D,
        trader: D,
        traderWsol: D,
        traderBaseAta: D,
        bondingCurve: D,
        canonicalPool: D,
        curveBaseAta: D,
        curveQuoteAta: D,
        sharingConfig: D,
        pumpUva: D,
        pumpUvaQuoteAta: D,
        cachedCreator: D,
        cachedCreatorVault: D,
        cachedCreatorVaultAta: D,
        poolCache: cache(),
        flags: 0,
        traderWsolAta: D,
        pumpPoolAuthority: D,
        ...fields,
    } as MarketKeys;
}

const acc = (n: string) => fixtureAccount(n);

describe('pump curve', () => {
    for (const role of ['fresh', 'near']) {
        it(`${role}: executable on (vT, vQ), the FeeConfig fee, the live leg's X_p`, async () => {
            const mint = fixtureAddress(`pump_${role}_mint`);
            const authority = await poolAuthority(mint);
            const mk = market({
                mint,
                tokenProgram: fixture(`pump_${role}_mint`).owner as Address,
                bondingCurve: fixtureAddress(`pump_${role}_curve`),
                canonicalPool: await canonicalPool(mint, authority),
                curveBaseAta: fixtureAddress(`pump_${role}_curve_base_ata`),
                pumpPoolAuthority: authority,
            });
            const read: (RawAccount | null)[] = [
                acc(`pump_${role}_curve`),
                acc('pump_global'),
                acc('pump_fee_config'),
                null,
                acc(`pump_${role}_curve_base_ata`),
                acc('pamm_fee_config'),
                acc('pamm_global_config'),
            ];
            const c = dec(`pump_${role}_curve`);
            const s = readVenueState(m, 'pump_curve', mk, read, { now: 0n, supply: null, pEma: 0n });
            expect(s.executable).toBe(true);
            expect(s.x).toBe(big(c.virtual_token_reserves));
            expect(s.y).toBe(big(c.virtual_quote_reserves));
            expect(s.r).toBe(big(c.real_quote_reserves));
            // pump-fees tier 0: protocol 95 + creator 30 bps (the creator is set) = 1.25 %.
            expect(s.feePpm).toBe(12_500);
            expect(s.curve?.pricing?.livePoolTokens).toBe(
                big(dec(`pump_${role}_curve_base_ata`).amount) - big(c.real_token_reserves),
            );
            expect(s.curve?.pricing?.offsetToken).toBe(big(c.virtual_token_reserves) - big(c.real_token_reserves));
            // A sale and a buyback of 1 % of the real reserve, with the round trip losing the fees. A
            // sale is paid out of the real SOL: the `fresh` curve holds 1 lamport, so it has no quote.
            const q = big(c.real_token_reserves) / 100n;
            const sale = quoteSell(m, s, q);
            const buy = quoteBuy(m, s, q);
            expect(buy).not.toBeNull();
            if (big(c.real_quote_reserves) < 1_000_000n) {
                expect(sale).toBeNull();
                return;
            }
            expect(sale && buy && buy.cost > sale.proceeds).toBe(true);
            // Selling moves the curve: more tokens, less SOL, a lower spot.
            if (!sale) return;
            const post = afterSale(m, s, q, sale);
            expect(post.x).toBe(s.x + q);
            const before = spotPrice(m, s);
            const after = spotPrice(m, post);
            expect(before !== null && after !== null && after < before).toBe(true);
            // The EMA leg on the live state's own (x, y) quotes the same curve (deterministic X_p).
            expect(quoteBuyAt(m, s, q, s.x, s.y)).not.toBeNull();
            expect(capFeePpm(s)).toBe(12_500);
            // The risk view at a 1 % cap is cheaper than the live 1.25 % fee.
            const risk = quoteBuy(m, s, q, 100);
            expect(risk && buy && risk.cost < buy.cost).toBe(true);
        });
    }

    it('a complete curve is not executable (migrating when vT > 0)', async () => {
        const mint = fixtureAddress('pump_graduated_mint');
        const authority = await poolAuthority(mint);
        const mk = market({ mint, bondingCurve: fixtureAddress('pump_graduated_curve'), pumpPoolAuthority: authority });
        const read = [
            acc('pump_graduated_curve'),
            acc('pump_global'),
            acc('pump_fee_config'),
            acc('pump_graduated_pool'),
            null,
            null,
            null,
        ];
        const s = readVenueState(m, 'pump_curve', mk, read, { now: 0n, supply: null, pEma: 0n });
        expect(s.executable).toBe(false);
        expect(s.curve?.live.complete).toBe(true);
        expect(s.curve?.canonicalPool).toBe('valid');
    });
});

/**
 * pump's v3 adapter (design pump-final §5.2–§5.4, §6.1.5–§6.1.6, §8), on the dumped 2026-10-13
 * state: the live curve's synthetic exit `B̂_v3`, the listing guards, and a migrating curve priced on
 * the pool `migrate_v2` will create (98u8RJ's completion, `pump_graduated_synthetic`, forged onto
 * `near166`'s accounts before its migration).
 */
describe('pump v3 (pump-final §8)', () => {
    const near = (n: string) => `pump_near166_${n}`;
    const u64le = (d: Uint8Array, off: number, v: bigint) =>
        new DataView(d.buffer, d.byteOffset, d.byteLength).setBigUint64(off, v, true);
    const patched = (a: RawAccount, edit: (d: Uint8Array) => void): RawAccount => {
        const data = a.data.slice();
        edit(data);
        return { ...a, data };
    };
    const supply = () => big(dec(near('mint')).supply);
    async function nearMarket(): Promise<MarketKeys> {
        const mint = fixtureAddress(near('mint'));
        const authority = await poolAuthority(mint);
        return market({
            mint,
            tokenProgram: fixture(near('mint')).owner as Address,
            bondingCurve: fixtureAddress(near('curve')),
            canonicalPool: await canonicalPool(mint, authority),
            curveBaseAta: fixtureAddress(near('curve_base_ata')),
            pumpPoolAuthority: authority,
        });
    }
    /** The curve ReadSet (C0 … C6), the canonical pool absent. */
    function nearRead(curve = acc(near('curve')) as RawAccount, baseAta = acc(near('curve_base_ata')) as RawAccount) {
        return [
            curve,
            acc('pump_global_1088'),
            acc('pump_fee_config_2026_10_13'),
            null,
            baseAta,
            acc('pump_pamm_fee_config_2026_10_13'),
            acc('pump_pamm_global_config_2026_10_13'),
        ];
    }
    const ctx = (s: bigint | null) => ({ now: 0n, supply: s, pEma: 0n });

    it('a live curve quotes B̂_v3 past rT: G0 near166 rT + 10e12, pump charged 22,944,563,744', async () => {
        const mk = await nearMarket();
        const s = readVenueState(m, 'pump_curve', mk, nearRead(), ctx(supply()));
        const c = dec(near('curve'));
        const rt = big(c.real_token_reserves);
        expect(s.executable).toBe(true);
        // One flat tier (0, 95, 30): the live supply's tier is the 10¹⁵ one.
        expect(s.curve?.pricing?.syntheticFees).toEqual({ lp: 0, protocol: 95, creator: 30 });
        expect(s.curve?.pricing?.livePoolTokens).toBe(big(dec(near('curve_base_ata')).amount) - rt);
        expect(s.curve?.pricing?.postComplete).toBe(false);
        expect(s.curve?.pricing?.postCompleteKnown).toBe(true);
        // The facade's pump_curve_synthetic vector: B̂_v3 = pump's v3 charge + 2 (≤ §5.2's 7).
        const q = rt + 10_000_000_000_000n;
        expect(quoteBuy(m, s, q)?.cost).toBe(22_944_563_746n);
        // Continuous at rT and monotone; the capped view caps both synthetic legs.
        const atRt = quoteBuy(m, s, rt)?.cost ?? 0n;
        const past = quoteBuy(m, s, rt + 1n)?.cost ?? 0n;
        expect(past).toBeGreaterThanOrEqual(atRt);
        expect(past - atRt).toBeLessThan(1_000n);
        expect((quoteBuy(m, s, q, 100)?.cost ?? q) < 22_944_563_746n).toBe(true);
        // The EMA leg on the live state's own (x, y) prices the same path with 206.9e12 (no donation
        // here, so the same quote).
        expect(quoteBuyAt(m, s, q, s.x, s.y)?.cost).toBe(22_944_563_746n);
        // A sale re-reads the venue: the synthetic route stays, on the post-sale market cap.
        const sale = quoteSell(m, s, 1_000_000_000_000n);
        if (!sale) throw new Error('sale');
        expect(afterSale(m, s, 1_000_000_000_000n, sale).curve?.pricing?.syntheticFees).toEqual(
            s.curve?.pricing?.syntheticFees,
        );
        // Without the mint's supply there is no synthetic route: nothing executes past rT.
        const noSupply = readVenueState(m, 'pump_curve', mk, nearRead(), ctx(null));
        expect(noSupply.executable).toBe(true);
        expect(noSupply.curve?.pricing?.syntheticFees).toBeNull();
        expect(quoteBuy(m, noSupply, q)).toBeNull();
        expect(quoteBuy(m, noSupply, rt)?.cost).toBe(atRt);
    });

    it("a sale re-resolves φ_s across a tier: G0's seven-tier curve FeeConfig", async () => {
        // G0's tier tripwire (pump-final §5.2, P1 correction (a); `quotes_synthetic_g0.rs`): on
        // near166's burned supply the live market cap (290,200,481,257) is in the 80 / 25 tier and
        // the 10¹⁵ one (290,253,172,320) in the 70 / 22 one. A sale of 10¹² tokens takes the live
        // market cap below 290,174,135,726, so the post-sale read (the open's step 9, the same
        // supply) prices B̂_v3 at 90 / 28: afterSale must re-resolve φ_s, not carry the old one.
        const tiers: readonly (readonly [bigint, number, number])[] = [
            [0n, 90, 28],
            [290_174_135_726n, 80, 25],
            [290_226_826_789n, 70, 22],
            [350_479_254_060n, 60, 19],
            [410_755_463_916n, 50, 16],
            [562_490_950_388n, 40, 13],
            [865_861_667_099n, 30, 10],
        ];
        // The dumped FeeConfig with its curve tiers (u32 count @65, 40 bytes each) replaced.
        const fc = acc('pump_fee_config_2026_10_13') as RawAccount;
        const n = new DataView(fc.data.buffer, fc.data.byteOffset, fc.data.byteLength).getUint32(65, true);
        const rest = fc.data.subarray(69 + 40 * n);
        const data = new Uint8Array(69 + 40 * tiers.length + rest.length);
        data.set(fc.data.subarray(0, 65));
        data.set(rest, 69 + 40 * tiers.length);
        const dv = new DataView(data.buffer);
        dv.setUint32(65, tiers.length, true);
        tiers.forEach(([threshold, protocol, creator], i) => {
            dv.setBigUint64(69 + 40 * i, threshold, true);
            dv.setBigUint64(69 + 40 * i + 24, BigInt(protocol), true);
            dv.setBigUint64(69 + 40 * i + 32, BigInt(creator), true);
        });
        const tiered = { ...fc, data };
        const mk = await nearMarket();
        const read = nearRead();
        read[2] = tiered;
        const s = readVenueState(m, 'pump_curve', mk, read, ctx(supply()));
        const pricing = s.curve?.pricing;
        if (!pricing) throw new Error('pricing');
        expect(pricing.schedules.curveTiers).toHaveLength(7);
        expect(pricing.fees).toEqual({ lp: 0, protocol: 70, creator: 22 });
        expect(pricing.syntheticFees).toEqual({ lp: 0, protocol: 80, creator: 25 });
        const q = 1_000_000_000_000n;
        const sale = quoteSell(m, s, q);
        if (!sale) throw new Error('sale');
        const post = afterSale(m, s, q, sale);
        const phiS = resolveSyntheticFees(m, pricing.schedules, sale.tokenAfter, sale.solAfter, supply());
        expect(phiS).toEqual({ lp: 0, protocol: 90, creator: 28 });
        expect(post.curve?.pricing?.syntheticFees).toEqual(phiS);
        // The same as a fresh read of the post-sale accounts: the curve's reserves moved by the sale
        // and the sold tokens in its base ATA.
        const rt = s.curve?.live.rt ?? 0n;
        const curve = patched(acc(near('curve')) as RawAccount, (d) => {
            u64le(d, 8, sale.tokenAfter);
            u64le(d, 16, sale.solAfter);
            u64le(d, 24, rt + q);
            u64le(d, 32, post.curve?.live.rq ?? 0n);
        });
        const ata = patched(acc(near('curve_base_ata')) as RawAccount, (d) =>
            u64le(d, 64, big(dec(near('curve_base_ata')).amount) + q),
        );
        const reread = nearRead(curve, ata);
        reread[2] = tiered;
        const fresh = readVenueState(m, 'pump_curve', mk, reread, ctx(supply()));
        expect(post.curve?.pricing).toEqual(fresh.curve?.pricing);
        expect([post.x, post.y, post.r, post.feePpm]).toEqual([fresh.x, fresh.y, fresh.r, fresh.feePpm]);
    });

    it('listing: a live cashback curve is not executable and refused 6109; a complete one is observed', async () => {
        const mint = fixtureAddress('pump_cashback166_mint');
        const authority = await poolAuthority(mint);
        const mk = market({
            mint,
            tokenProgram: fixture('pump_cashback166_mint').owner as Address,
            bondingCurve: fixtureAddress('pump_cashback166_curve'),
            curveBaseAta: fixtureAddress('pump_cashback166_curve_base_ata'),
            pumpPoolAuthority: authority,
        });
        const read = nearRead(
            acc('pump_cashback166_curve') as RawAccount,
            acc('pump_cashback166_curve_base_ata') as RawAccount,
        );
        const live = readVenueState(m, 'pump_curve', mk, read, ctx(big(dec('pump_cashback166_mint').supply)));
        expect(live.executable).toBe(false);
        expect(live.x).toBe(0n);
        expect(live.curve?.pricing?.syntheticFees).toBeNull();
        const d = await detectFrom(
            mint,
            {
                bondingCurve: fixtureAddress('pump_cashback166_curve'),
                canonicalPool: await canonicalPool(mint, authority),
                authority,
            },
            acc('pump_cashback166_curve'),
            null,
        );
        expect(d.refusal).toBe('CurveCashbackCoin');
        expect(d.kind).toBe('none');
        // The same coin complete (no pool yet): PumpMigrating, observed and priced (§6.1.6, R-4).
        const done = patched(acc('pump_cashback166_curve') as RawAccount, (x) => {
            x[48] = 1;
        });
        const mig = readVenueState(
            m,
            'pump_migrating',
            mk,
            nearRead(done, acc('pump_cashback166_curve_base_ata') as RawAccount),
            ctx(null),
        );
        expect(mig.executable).toBe(false);
        expect(mig.curve?.migrating).toBe(true);
        expect(mig.x).toBe(big(dec('pump_cashback166_curve').virtual_token_reserves));
        expect(mig.curve?.pricing).not.toBeNull();
        const dc = await detectFrom(
            mint,
            {
                bondingCurve: fixtureAddress('pump_cashback166_curve'),
                canonicalPool: await canonicalPool(mint, authority),
                authority,
            },
            done,
            null,
        );
        expect(dc.refusal).toBeNull();
        expect(dc.kind).toBe('pump_migrating');
    });

    it('listing: a live curve of depth 1, mayhem, and a live curve carrying pc* (G-3) do not execute', async () => {
        const mk = await nearMarket();
        const curve = acc(near('curve')) as RawAccount;
        const deep = patched(curve, (x) => {
            x[141] = 1;
        });
        expect(readVenueState(m, 'pump_curve', mk, nearRead(deep), ctx(supply())).executable).toBe(false);
        const mint = fixtureAddress(near('mint'));
        const authority = await poolAuthority(mint);
        const keys = { bondingCurve: fixtureAddress(near('curve')), canonicalPool: mk.canonicalPool, authority };
        expect((await detectFrom(mint, keys, deep, null)).refusal).toBe('CurveQuoteNotSol');
        expect((await detectFrom(mint, keys, curve, null)).refusal).toBeNull();
        const mayhem = acc('pump_mayhem166_curve') as RawAccount;
        expect(readVenueState(m, 'pump_curve', mk, nearRead(mayhem), ctx(supply())).executable).toBe(false);
        // G-3: pump writes pcB / pcQ only in the completing buy; a live curve carrying either has no
        // pricing (not executable; A63's cap-only risk view).
        for (const off of [150, 158]) {
            const pc = patched(curve, (x) => u64le(x, off, 1n));
            const s = readVenueState(m, 'pump_curve', mk, nearRead(pc), ctx(supply()));
            expect(s.curve?.pricing, `@${off}`).toBeNull();
            expect(s.executable, `@${off}`).toBe(false);
        }
    });

    describe('a migrating curve after a synthetic pool part (98u8RJ, §5.4)', () => {
        // 98u8RJ's completed curve before migrate_v2 (pump-final V4, T-MATH-1): rT 0, rQ excluding
        // the pool part, pcB 6,238,850,918,425 and pcQ 2,642,475,552; its base ATA holds
        // 206.9e12 − pcB, the pool migrate_v2 created: (200,661,149,081,575, 87,632,834,741).
        const X1 = 200_661_149_081_575n;
        const Y1 = 87_632_834_741n;
        const RQ = 85_005_359_190n;
        function completed(o: { pcb?: bigint; pcq?: bigint; ivqr?: bigint; ata?: bigint } = {}) {
            const curve = patched(acc(near('curve')) as RawAccount, (x) => {
                u64le(x, 8, 279_900_000_000_000n);
                u64le(x, 16, 30_000_000_000n + RQ);
                u64le(x, 24, 0n);
                u64le(x, 32, RQ);
                x[48] = 1;
                u64le(x, 142, o.ivqr ?? 30_000_000_000n);
                u64le(x, 150, o.pcb ?? 6_238_850_918_425n);
                u64le(x, 158, o.pcq ?? 2_642_475_552n);
            });
            const ata = patched(acc(near('curve_base_ata')) as RawAccount, (x) => u64le(x, 64, o.ata ?? X1));
            return nearRead(curve, ata);
        }
        /** The new pool at its own PumpSwap tier (a coin creator set, no pool creator fee). */
        const pool = (y: bigint, x: bigint) => {
            const fc = acc('pump_pamm_fee_config_2026_10_13') as RawAccount;
            const sch = new m.PammSchedule(10n ** 15n, true, 0, false);
            try {
                for (const t of value(readFeeConfig(fc.owner, fc.data))?.feeTiers ?? [])
                    sch.addTier(t.threshold, t.lp, t.protocol, t.creator);
                const f = sch.resolve(y, x);
                const v = m.Venue.pumpSwap(f);
                f.free();
                return v;
            } finally {
                sch.free();
            }
        };
        const q = 10_000_000_000_000n;

        it('the EMA leg prices the pool migrate_v2 creates: (live curve_base_ata, rQ − fee + pcQ)', async () => {
            const s = readVenueState(m, 'pump_migrating', await nearMarket(), completed(), ctx(supply()));
            expect(s.executable).toBe(false);
            expect(s.curve?.migrating).toBe(true);
            const p = s.curve?.pricing;
            expect(p?.postComplete).toBe(true);
            expect(p?.postCompleteKnown).toBe(true);
            expect(p?.postCompleteQuoteIn).toBe(2_642_475_552n);
            expect(p?.livePoolTokens).toBe(X1);
            expect(p?.syntheticFees).toBeNull();
            // The completed curve's ring state (rT = 0): B̂_str = PumpSwap's buy on (X₁, Y₁) exactly.
            const v = pool(Y1, X1);
            try {
                expect(quoteBuyAt(m, s, q, s.x, s.y)?.cost).toBe(v.buy(q, X1, Y1)?.cost);
            } finally {
                v.free();
            }
            // Without the pool part the EMA leg prices the deterministic 206.9e12 pool: cheaper.
            const plain = readVenueState(
                m,
                'pump_migrating',
                await nearMarket(),
                completed({ pcb: 0n, pcq: 0n, ata: 206_900_000_000_000n }),
                ctx(supply()),
            );
            expect(plain.curve?.pricing?.postComplete).toBe(false);
            expect(
                (quoteBuyAt(m, plain, q, plain.x, plain.y)?.cost ?? 0n) < (quoteBuyAt(m, s, q, s.x, s.y)?.cost ?? 0n),
            ).toBe(true);
        });

        it('FV is ρ-scaled onto that pool, unchanged without a pool part, and undefined when unknown', async () => {
            const mk = await nearMarket();
            const s = readVenueState(m, 'pump_migrating', mk, completed(), ctx(supply()));
            // At the frozen curve's pool price P(X₀, Y₀), ρ reprices FV onto (X₁, Y₁).
            const X0 = 206_900_000_000_000n;
            const Y0 = RQ - big(dec('pump_global_1088').pool_migration_fee);
            const p0 = m.priceWad(X0, Y0);
            const fv = fvRepriced(m, s, q, 0n, p0)?.cost ?? 0n;
            const v1 = pool(Y1, X1);
            const v0 = pool(Y0, X0);
            try {
                const onPool = v1.buy(q, X1, Y1)?.cost ?? 0n;
                expect(fv > 0n && (fv > onPool ? fv - onPool : onPool - fv) * 10_000n < onPool).toBe(true);
                // No pool part (and no donation counted): today's FV, at p itself.
                const plain = readVenueState(
                    m,
                    'pump_migrating',
                    mk,
                    completed({ pcb: 0n, pcq: 0n, ata: X0 + 5_000_000_000_000n }),
                    ctx(supply()),
                );
                const l0 = m.liquidity(X0, Y0);
                expect(fvRepriced(m, plain, q, l0, p0)?.cost).toBe(v0.buyRepriced(q, l0, p0)?.cost);
                expect((fvRepriced(m, plain, q, l0, p0)?.cost ?? fv) < fv).toBe(true);
            } finally {
                v1.free();
                v0.free();
            }
            // A token donation after the pool part counts (migrate_v2 seeds it): FV falls.
            const donated = readVenueState(
                m,
                'pump_migrating',
                mk,
                completed({ ata: X1 + 10_000_000_000_000n }),
                ctx(supply()),
            );
            expect((fvRepriced(m, donated, q, 0n, p0)?.cost ?? fv) < fv).toBe(true);
            // The risk view caps the pool's tier.
            expect((fvRepriced(m, s, q, 0n, p0, 20)?.cost ?? fv) < fv).toBe(true);
            // §6.1.5: the post-complete fields not known — one of pcB / pcQ alone, or vQ − rQ ≠ ivqr —
            // keep the curve's quotes but leave FV undefined (S3 gives the lenders the whole take).
            for (const o of [{ pcq: 0n }, { pcb: 0n }, { ivqr: 1n }]) {
                const u = readVenueState(m, 'pump_migrating', mk, completed(o), ctx(supply()));
                expect(
                    u.curve?.pricing?.postCompleteKnown,
                    JSON.stringify(o, (_, x) => String(x)),
                ).toBe(false);
                expect(fvRepriced(m, u, q, 0n, p0)).toBeNull();
                expect(quoteBuyAt(m, u, q, u.x, u.y)).not.toBeNull();
            }
        });
    });
});

describe('PumpSwap', () => {
    it('PUMP-2026-10-1: R is the payable SOL under fee buckets, the tier subtracts the boost', async () => {
        const role = 'mature';
        const mint = fixtureAddress(`pamm_${role}_mint`);
        const pool = fixtureAddress(`pamm_${role}_pool`);
        const pc = cache();
        pc[PAMM_CACHE.poolBaseVault] = fixtureAddress(`pamm_${role}_pool_base_vault`);
        pc[PAMM_CACHE.poolQuoteVault] = fixtureAddress(`pamm_${role}_pool_quote_vault`);
        const boostVault = await pammBoostVault(pool);
        pc[PAMM_CACHE.boostVault] = boostVault;
        pc[PAMM_CACHE.boostVaultAta] = await wsolAta(boostVault);
        const mk = market({
            mint,
            tokenProgram: fixture(`pamm_${role}_mint`).owner as Address,
            canonicalPool: pool,
            poolCache: pc,
            pumpPoolAuthority: await poolAuthority(mint),
        });
        const p = dec(`pamm_${role}_pool`);
        const quote = big(dec(`pamm_${role}_pool_quote_vault`).amount);
        const boost = big(p.virtual_quote_reserves ?? 0);
        const supply = big(dec(`pamm_${role}_mint`).supply);
        // The buckets a third party's v2 trades leave: in the vault, off the VQR (`VQR = boost − B`).
        const state = (protocol: bigint, creator: bigint, vqr: bigint) => {
            const a = acc(`pamm_${role}_pool`) as RawAccount;
            const data = a.data.slice();
            const v = new DataView(data.buffer, data.byteOffset, data.byteLength);
            v.setBigUint64(PAMM_POOL_OFF_PROTOCOL_FEES, protocol, true);
            v.setBigUint64(PAMM_POOL_OFF_CREATOR_FEES, creator, true);
            v.setBigUint64(245, BigInt.asUintN(64, vqr), true);
            v.setBigInt64(253, vqr < 0n ? -1n : 0n, true);
            const read = [
                { ...a, data },
                acc(`pamm_${role}_pool_base_vault`),
                acc(`pamm_${role}_pool_quote_vault`),
                acc('pamm_state_global_config'),
                acc('pamm_state_fee_config'),
                acc(`pamm_${role}_boost_vault_ata`),
                acc(`pamm_${role}_lp_mint`),
            ];
            return readVenueState(m, 'pump_swap', mk, read, { now: 0n, supply, pEma: 0n });
        };
        const b = boost / 2n;
        const s = state(b / 2n, b - b / 2n, boost - b);
        expect(s.executable).toBe(true);
        expect(s.y).toBe(quote + boost - b);
        expect(s.r).toBe(quote - b);
        expect(s.pool?.virtualPart).toBe(boost);
        // The open's sale cap is the payable SOL.
        const sale = quoteSell(m, s, 1_000_000n);
        expect(sale && sale.proceeds > 0n).toBe(true);
        expect(quoteSell(m, { ...s, r: 1n }, s.x / 10n)).toBeNull();
        // No buckets: the 2026-09 reading.
        const plain = state(0n, 0n, boost);
        expect(plain.r).toBe(quote);
        expect(plain.pool?.virtualPart).toBe(boost);
        // Buckets above the vault: not executable.
        expect(state(quote, 1n, boost).executable).toBe(false);
    });

    for (const role of ['fresh', 'mature', 'cashback']) {
        it(`${role}: vault + VQR, the tier at the live supply, the burned-LP depth`, async () => {
            const mint = fixtureAddress(`pamm_${role}_mint`);
            const pool = fixtureAddress(`pamm_${role}_pool`);
            const authority = await poolAuthority(mint);
            const pc = cache();
            pc[PAMM_CACHE.poolBaseVault] = fixtureAddress(`pamm_${role}_pool_base_vault`);
            pc[PAMM_CACHE.poolQuoteVault] = fixtureAddress(`pamm_${role}_pool_quote_vault`);
            const boost = await pammBoostVault(pool);
            pc[PAMM_CACHE.boostVault] = boost;
            pc[PAMM_CACHE.boostVaultAta] = await wsolAta(boost);
            const mk = market({
                mint,
                tokenProgram: fixture(`pamm_${role}_mint`).owner as Address,
                canonicalPool: pool,
                poolCache: pc,
                pumpPoolAuthority: authority,
            });
            expect(await pammLpMint(pool)).toBe(fixtureAddress(`pamm_${role}_lp_mint`));
            const read = [
                acc(`pamm_${role}_pool`),
                acc(`pamm_${role}_pool_base_vault`),
                acc(`pamm_${role}_pool_quote_vault`),
                acc('pamm_state_global_config'),
                acc('pamm_state_fee_config'),
                acc(`pamm_${role}_boost_vault_ata`),
                acc(`pamm_${role}_lp_mint`),
            ];
            const supply = big(dec(`pamm_${role}_mint`).supply);
            const s = readVenueState(m, 'pump_swap', mk, read, { now: 0n, supply, pEma: 0n });
            const p = dec(`pamm_${role}_pool`);
            const base = big(dec(`pamm_${role}_pool_base_vault`).amount);
            const quote = big(dec(`pamm_${role}_pool_quote_vault`).amount);
            const vqr = big(p.virtual_quote_reserves ?? 0);
            expect(s.executable).toBe(true);
            expect(s.x).toBe(base);
            expect(s.y).toBe(quote + vqr);
            expect(s.r).toBe(quote < quote + vqr ? quote : quote + vqr);
            expect(s.pool?.isCashback).toBe(p.is_cashback_coin === 1);
            expect(s.feePpm).toBeGreaterThan(0);
            const lp = big(dec(`pamm_${role}_lp_mint`).supply);
            expect(s.lockedDepth).toBe(m.lockedDepth(big(p.lp_supply), lp, base, quote + vqr));
            const q = base / 1_000n;
            const sale = quoteSell(m, s, q);
            const buy = quoteBuy(m, s, q);
            expect(sale && buy && buy.cost > sale.proceeds).toBe(true);
            // Without the supply the fee tier cannot resolve: not executable (A63).
            expect(readVenueState(m, 'pump_swap', mk, read, { now: 0n, supply: null, pEma: 0n }).executable).toBe(
                false,
            );
        });
    }
});

describe('the exit risk cap (AIR-10)', () => {
    // The program's `venues::tests::exit_risk_cap_takes_the_fee_at_p_ema_on_pumpswap_only`: the hard
    // cap and, on PumpSwap once resolved, ⌈φ at P_ema/100⌉ — also on a halted pool — and the hard cap
    // alone without a fee at P_ema and on every other venue.
    const pool = (feePpmAtEma: number): PoolView => ({
        disableFlags: 0,
        boostAmount: 0n,
        virtualPart: 0n,
        isCashback: false,
        coinCreator: D,
        fees: { lp: 40, protocol: 7, creator: 3 },
        feePpmAtEma,
        schedule: null,
    });
    const withPool = (kind: 'pump_swap' | 'raydium_cpmm' | 'pump_curve', atEma: number) => ({
        ...notExecutable(kind),
        pool: pool(atEma),
    });

    it('takes the fee at P_ema on PumpSwap only', () => {
        expect(exitRiskCapBps(withPool('pump_swap', 3_000), 500)).toBe(30);
        expect(exitRiskCapBps(withPool('pump_swap', 2_950), 500)).toBe(30);
        expect(exitRiskCapBps(withPool('pump_swap', 3_000), 25)).toBe(25);
        expect(exitRiskCapBps(withPool('pump_swap', 0xffff_ffff), 500)).toBe(500);
        expect(exitRiskCapBps(notExecutable('pump_swap'), 500)).toBe(500);
        expect(exitRiskCapBps(withPool('raydium_cpmm', 3_000), 500)).toBe(500);
        expect(exitRiskCapBps(withPool('pump_curve', 3_000), 500)).toBe(500);
    });
});

describe('Raydium', () => {
    it('AMM v4 Fartcoin: SOL is the coin, fee 25 bps, depth uses the raw locked LP ratio', () => {
        const pc = cache();
        pc[RAY_CACHE.pool] = fixtureAddress('ray_v4_fartcoin_pool');
        pc[RAY_CACHE.vaultSol] = fixtureAddress('ray_v4_fartcoin_coin_vault');
        pc[RAY_CACHE.vaultTok] = fixtureAddress('ray_v4_fartcoin_pc_vault');
        pc[RAY_CACHE.lpMint] = fixtureAddress('ray_v4_fartcoin_lp_mint');
        const mk = market({
            mint: fixtureAddress('mint_fartcoin'),
            tokenProgram: 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA' as Address,
            poolCache: pc,
            flags: MARKET_FLAG.solIsToken0OrCoin,
        });
        const read = [
            acc('ray_v4_fartcoin_pool'),
            acc('ray_v4_fartcoin_coin_vault'),
            acc('ray_v4_fartcoin_pc_vault'),
            acc('ray_v4_fartcoin_lp_mint'),
        ];
        const now = snapshotTime('raydium_state');
        const s = readVenueState(m, 'raydium_v4', mk, read, { now, supply: null, pEma: 0n });
        expect(s.executable).toBe(true);
        expect(s.x).toBe(big(dec('ray_v4_fartcoin_pc_vault').amount));
        expect(s.y).toBe(big(dec('ray_v4_fartcoin_coin_vault').amount));
        expect(s.feePpm).toBe(2_500);
        const counter = big(dec('ray_v4_fartcoin_pool').lp_amount);
        const supply = big(dec('ray_v4_fartcoin_lp_mint').supply);
        expect(s.lockedDepth).toBe(((counter - supply) * m.liquidity(s.x, s.y)) / counter);
        expect(liquidityOf(m, s)).toBe(m.liquidity(s.x, s.y));
        // The market's recorded SOL side must match the pool's: the other side is not executable.
        const wrong = readVenueState(m, 'raydium_v4', { ...mk, flags: 0 }, read, { now, supply: null, pEma: 0n });
        expect(wrong.executable).toBe(false);
    });

    it('CPMM USELESS: WSOL token 0, vault − counters, the config fee', () => {
        const p = dec('cpmm_useless_pool');
        const pc = cache();
        pc[RAY_CACHE.pool] = fixtureAddress('cpmm_useless_pool');
        pc[RAY_CACHE.ammConfig] = p.amm_config as Address;
        pc[RAY_CACHE.vaultSol] = fixtureAddress('cpmm_useless_token_0_vault');
        pc[RAY_CACHE.vaultTok] = fixtureAddress('cpmm_useless_token_1_vault');
        pc[RAY_CACHE.observation] = p.observation_key as Address;
        pc[RAY_CACHE.lpMint] = fixtureAddress('cpmm_useless_lp_mint');
        const config = ['00', '01', '02', '03', '04', '05', '06', '07', '08', '09', '10']
            .map((i) => `cpmm_config_${i}`)
            .find((n) => fixture(n).address === p.amm_config);
        if (!config) throw new Error('config');
        const mk = market({
            mint: fixtureAddress('cpmm_useless_token_mint'),
            tokenProgram: 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA' as Address,
            poolCache: pc,
            flags: MARKET_FLAG.solIsToken0OrCoin,
        });
        const read = [
            acc('cpmm_useless_pool'),
            acc(config),
            acc('cpmm_useless_token_0_vault'),
            acc('cpmm_useless_token_1_vault'),
            acc('cpmm_useless_lp_mint'),
        ];
        const s = readVenueState(m, 'raydium_cpmm', mk, read, {
            now: snapshotTime('raydium_state'),
            supply: null,
            pEma: 0n,
        });
        expect(s.executable).toBe(true);
        const counters0 = big(p.protocol_fees_token_0) + big(p.fund_fees_token_0) + big(p.creator_fees_token_0 ?? 0);
        const counters1 = big(p.protocol_fees_token_1) + big(p.fund_fees_token_1) + big(p.creator_fees_token_1 ?? 0);
        expect(s.y).toBe(big(dec('cpmm_useless_token_0_vault').amount) - counters0);
        expect(s.x).toBe(big(dec('cpmm_useless_token_1_vault').amount) - counters1);
        expect(s.feePpm).toBe(Number(big(dec(config).trade_fee_rate)));
        const q = s.x / 1_000n;
        expect(quoteSell(m, s, q)?.proceeds).toBeGreaterThan(0n);
    });
});

describe('detection (classify_pump)', () => {
    async function detect(role: string) {
        const mint = fixtureAddress(`pump_${role}_mint`);
        const authority = await poolAuthority(mint);
        const cp = await canonicalPool(mint, authority);
        const pool = (() => {
            try {
                return acc(`pump_${role}_pool`);
            } catch {
                return null;
            }
        })();
        return detectFrom(
            mint,
            { bondingCurve: fixtureAddress(`pump_${role}_curve`), canonicalPool: cp, authority },
            acc(`pump_${role}_curve`),
            pool,
        );
    }

    it('lists a live curve with the pump init set', async () => {
        const d = await detect('fresh');
        expect(d.kind).toBe('pump_curve');
        expect(d.refusal).toBeNull();
        expect(d.remainingAccounts.map((a) => a.role)).toEqual([0, 0, 0, 1, 0]);
        expect(d.remainingAccounts[2]?.address).toBe('6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P');
    });

    it('lists a graduated coin on its canonical pool with the PumpSwap init set', async () => {
        const d = await detect('graduated');
        expect(d.kind).toBe('pump_swap');
        expect(d.remainingAccounts[2]?.address).toBe('pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA');
    });

    it('refuses mayhem and USDC curves, and a short curve without a payer to extend it', async () => {
        expect((await detect('mayhem')).refusal).toBe('CurveMayhemUnsupported');
        expect((await detect('usdc')).refusal).toBe('CurveQuoteNotSol');
        const legacy = await detect('legacy49');
        expect(legacy.refusal).toBe('CurveNeedsExtend');
    });

    it('a curve that graduated elsewhere lists as list-only', async () => {
        const d = await detect('fartcoin');
        expect(d.kind).toBe('none');
        expect(d.refusal).toBeNull();
        expect(d.remainingAccounts).toHaveLength(2);
    });
});
