/**
 * FE-15 (M10): the WASM exports that replaced the TypeScript ports equal those ports (`legacy.ts`,
 * frozen at 7d19feb) and the Rust crates' own vectors, on the repo's vector files and fixtures and
 * on seeded sweeps:
 *
 * - the open guards (`spotNearEma`, `pOpenMax`) on every program vector's spot and EMA
 *   (`features/short/vectors/program-vectors.json`, the program on LiteSVM: `spl-open-deviation` is
 *   its `PriceDeviatesFromEma`);
 * - pool selection (`lambdaBps`, `lockedDepth`, `lockedSol`, `lockedFloorOk`) on every dumped
 *   Raydium and PumpSwap pool of `tests/fixtures` (the Fartcoin pool's λ = 9,978 bps is
 *   `pool_select`'s own test value);
 * - the mint verdict (`mintCheck`) on every dumped mint, the shapes of `mint.rs`'s tests and
 *   28,800 byte mutations of them: same decision, first reason, every further reason and its
 *   extension type, base fields and extension list.
 */
import { podU128ToBigInt } from '@shortwin/client';
import { type Address, address, getAddressEncoder } from '@solana/kit';
import { beforeAll, describe, expect, it } from 'vitest';
import { SYSTEM_PROGRAM, TOKEN_2022_PROGRAM, TOKEN_PROGRAM, WSOL_2022_MINT, WSOL_MINT } from '../config/programs.ts';
import { checkMintSafety, reasonCodes } from '../features/markets/create/mintSafety.ts';
import { programVectors, testMath, vectorSnapshot } from '../features/short/testing.ts';
import type { MathModule } from '../math/load.ts';
import { spotPrice } from '../venues/state.ts';
import { fixtureAccount, fixtureManifest } from '../venues/testing.ts';
import * as legacy from './legacy.ts';

const WAD = 10n ** 18n;
const U64_MAX = (1n << 64n) - 1n;
const U128_MAX = (1n << 128n) - 1n;

let m: MathModule;

beforeAll(() => {
    m = testMath();
});

/** xorshift64 on BigInt: a reproducible sweep. */
function rng(seed: bigint) {
    let x = seed;
    const next = (): bigint => {
        x ^= (x << 13n) & U64_MAX;
        x ^= x >> 7n;
        x ^= (x << 17n) & U64_MAX;
        return x;
    };
    /** A value below 2^bits, biased to the edges (0, 1, the max, powers of ten). */
    const below = (bits: bigint): bigint => {
        const r = next() % 8n;
        const max = (1n << bits) - 1n;
        if (r === 0n) return 0n;
        if (r === 1n) return 1n;
        if (r === 2n) return max;
        if (r === 3n) return 10n ** (next() % (bits === 128n ? 38n : 19n));
        let v = 0n;
        for (let i = 0n; i < bits; i += 64n) v = (v << 64n) | next();
        return v & (max >> (next() % bits));
    };
    return { next, below };
}

describe('open guards: spotNearEma and P_OPEN_MAX', () => {
    it('P_OPEN_MAX is the port’s constant', () => {
        expect(m.pOpenMax()).toBe(legacy.P_OPEN_MAX);
    });

    it('agree on every program vector’s spot and EMA, as the program decided', async () => {
        let n = 0;
        for (const c of programVectors()) {
            if (c.kind !== 'open') continue;
            const { s } = await vectorSnapshot(c);
            const spot = spotPrice(m, s.venue);
            const ema = podU128ToBigInt(s.market.emaPrice);
            if (spot === null) continue;
            const dev = s.config.params.openEmaDevBps;
            const near = m.spotNearEma(spot, ema, dev);
            expect(near, c.name).toBe(legacy.spotNearEma(spot, ema, dev));
            // 6053 = PriceDeviatesFromEma: the program refused exactly the case the guard fails (the
            // other refusals come before the guard or after it).
            if (c.result.ok || c.result.code === 6053) expect(near, c.name).toBe(c.result.ok);
            expect(ema <= m.pOpenMax(), c.name).toBe(ema <= legacy.P_OPEN_MAX);
            n++;
        }
        expect(n).toBeGreaterThanOrEqual(8);
    });

    it('agree on a sweep of u128 prices and every deviation, edges included', () => {
        const r = rng(0x5eed_0001n);
        for (let i = 0; i < 20_000; i++) {
            const ema = r.below(128n);
            const spot = i % 3 === 0 ? ema + (r.below(64n) % (ema + 1n)) : r.below(128n);
            const dev = Number(r.next() % 20_001n);
            const s = spot > U128_MAX ? U128_MAX : spot;
            expect(m.spotNearEma(s, ema, dev), `${s} ${ema} ${dev}`).toBe(legacy.spotNearEma(s, ema, dev));
        }
        // The boundary itself, above and below the EMA: |P_spot − P_ema|·10⁴ = dev·P_ema passes and a
        // wei further out does not; the old port agrees at the edge and a wei either side of it.
        for (const dev of [0, 1, 500, 2_000, 10_000]) {
            const off = (WAD * BigInt(dev)) / 10_000n;
            for (const [edge, out] of [
                [WAD + off, 1n],
                [WAD - off, -1n],
            ] as const) {
                expect(m.spotNearEma(edge, WAD, dev), `edge ${edge} ${dev}`).toBe(true);
                expect(legacy.spotNearEma(edge, WAD, dev), `edge ${edge} ${dev}`).toBe(true);
                for (const e of [edge - out, edge, edge + out]) {
                    if (e < 0n) continue;
                    expect(m.spotNearEma(e, WAD, dev), `${e} ${dev}`).toBe(legacy.spotNearEma(e, WAD, dev));
                    // A wei further in passes too, except at dev 0 (where only P_spot = P_ema does).
                    const near = e === edge || (e === edge - out && dev > 0);
                    expect(m.spotNearEma(e, WAD, dev), `${e} ${dev}`).toBe(near);
                }
            }
        }
    }, 30_000);
});

/** Every dumped pool with an LP counter, with its LP mint's supply. */
function lpPools(): { name: string; counter: bigint; supply: bigint }[] {
    const accounts = fixtureManifest().accounts;
    const out: { name: string; counter: bigint; supply: bigint }[] = [];
    for (const a of accounts) {
        const d = a.decoded as Record<string, unknown> | null | undefined;
        if (!a.name.endsWith('_pool') && !a.name.endsWith('_pool_negative_vqr')) continue;
        const counter = d?.lp_amount ?? d?.lp_supply;
        const lpMint = d?.lp_mint;
        if (counter === undefined || typeof lpMint !== 'string') continue;
        const mint = accounts.find((x) => x.address === lpMint && x.decoded && 'supply' in x.decoded);
        if (!mint?.decoded) continue;
        out.push({
            name: a.name,
            counter: BigInt(counter as string | number),
            supply: BigInt(mint.decoded.supply as string | number),
        });
    }
    return out;
}

/** The token amounts of every dumped token account (realistic u64 reserves). */
function reserves(): bigint[] {
    const out: bigint[] = [];
    for (const a of fixtureManifest().accounts) {
        const amount = (a.decoded as Record<string, unknown> | null | undefined)?.amount;
        if (amount !== undefined && amount !== null) out.push(BigInt(amount as string | number));
    }
    return out;
}

describe('pool selection: lambdaBps, lockedDepth, lockedSol, lockedFloorOk', () => {
    it('agree on every dumped Raydium / PumpSwap pool (the Fartcoin pool: λ = 9,978 bps)', () => {
        const pools = lpPools();
        expect(pools.length).toBeGreaterThanOrEqual(10);
        const fart = pools.find((p) => p.name === 'ray_v4_fartcoin_pool');
        expect(fart && m.lambdaBps(fart.counter, fart.supply)).toBe(9_978);
        const amounts = reserves().filter((v) => v > 0n);
        expect(amounts.length).toBeGreaterThan(20);
        let n = 0;
        for (const p of pools) {
            expect(m.lambdaBps(p.counter, p.supply), p.name).toBe(legacy.lambdaBps(p.counter, p.supply));
            for (let i = 0; i < amounts.length; i++) {
                const x = amounts[i] ?? 0n;
                const y = amounts[(i * 7 + 3) % amounts.length] ?? 0n;
                const d = m.lockedDepth(p.counter, p.supply, x, y);
                expect(d, p.name).toBe(legacy.lockedDepth(m, p.counter, p.supply, x, y));
                let price: bigint;
                try {
                    price = m.priceWad(x, y);
                } catch {
                    continue;
                }
                expect(m.lockedSol(d, price), p.name).toBe(legacy.lockedSol(m, d, price));
                for (const [lam, min] of [
                    [2_000, 25_000_000_000n],
                    [5_000, 100_000_000_000n],
                    [10_000, 1n],
                    [0, 0n],
                ] as const) {
                    expect(m.lockedFloorOk(d, price, lam, min), p.name).toBe(
                        legacy.lockedFloorOk(m, d, price, lam, min),
                    );
                }
                n++;
            }
        }
        expect(n).toBeGreaterThan(200);
    }, 30_000);

    it('agree on a sweep of u64 counters and reserves, u128 prices', () => {
        const r = rng(0x5eed_0002n);
        for (let i = 0; i < 20_000; i++) {
            const counter = r.below(64n);
            const supply = i % 2 === 0 ? counter - (r.below(64n) % (counter + 1n)) : r.below(64n);
            const [x, y] = [r.below(64n), r.below(64n)];
            expect(m.lambdaBps(counter, supply)).toBe(legacy.lambdaBps(counter, supply));
            const d = m.lockedDepth(counter, supply, x, y);
            expect(d).toBe(legacy.lockedDepth(m, counter, supply, x, y));
            const p = r.below(128n);
            // D < 2⁶⁴ and isqrt(P) < 2⁶⁴: the locked SOL never reaches the facade's u128 saturation.
            expect(m.lockedSol(d, p)).toBe(legacy.lockedSol(m, d, p));
            const lam = Number(r.next() % 10_001n);
            const min = r.below(64n);
            expect(m.lockedFloorOk(d, p, lam, min), `${d} ${p} ${lam} ${min}`).toBe(
                legacy.lockedFloorOk(m, d, p, lam, min),
            );
        }
        // Outside the reachable domain the facade saturates as the program does (the port was unbounded).
        expect(m.lockedSol(U128_MAX, U128_MAX)).toBe(U128_MAX);
    }, 30_000);
});

const enc = getAddressEncoder();
const KEY = address('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
const AUTH = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');

function baseMint(freeze: boolean): number[] {
    const d = new Array<number>(82).fill(0);
    d[0] = 1;
    d.splice(4, 32, ...enc.encode(AUTH));
    d[36] = 0x40;
    d[37] = 0x42;
    d[38] = 0x0f;
    d[44] = 6;
    d[45] = 1;
    if (freeze) {
        d[46] = 1;
        d.splice(50, 32, ...enc.encode(AUTH));
    }
    return d;
}

function extendedMint(entries: readonly (readonly [number, number])[], freeze: boolean): number[] {
    const d = baseMint(freeze);
    while (d.length < 166) d.push(0);
    d[165] = 1;
    for (const [t, len] of entries) {
        d.push(t & 0xff, t >> 8, len & 0xff, len >> 8);
        for (let i = 0; i < len; i++) d.push(0xab);
    }
    return d;
}

/** The verdict fields both implementations define the same way. */
function shape(v: ReturnType<typeof checkMintSafety> | ReturnType<typeof legacy.checkMintSafety>) {
    const malformed = v.reasons.some((r) => r.code === 'MintMalformed');
    return {
        ok: v.ok,
        programError: v.programError,
        codes: v.reasons.map((r) => r.code),
        extensions: v.reasons.map((r) => (r.code === 'MintDisallowedExtension' ? (r.extension ?? null) : undefined)),
        program: v.program,
        // The port kept the base of a mint whose padding or account type fails; the facade reports
        // no base for any malformed mint (it is not a mint the program would read).
        base: malformed ? null : v.base,
        types: malformed ? [] : v.extensions,
    };
}

function same(key: Address, owner: Address | null, data: Uint8Array | null, label: string) {
    const now = checkMintSafety(m, key, owner, data);
    const old = legacy.checkMintSafety(key, owner, data);
    expect(shape(now), label).toEqual(shape(old));
    expect(reasonCodes(now), label).toEqual([...new Set(old.reasons.map((r) => r.code))]);
}

describe('mint verdict: mintCheck', () => {
    it('agrees on every dumped mint of tests/fixtures', () => {
        let n = 0;
        for (const a of fixtureManifest().accounts) {
            const d = a.decoded as Record<string, unknown> | null | undefined;
            if (!d || !('decimals' in d) || !('supply' in d)) continue;
            const acc = fixtureAccount(a.name);
            if (!acc) continue;
            same(acc.address, acc.owner, acc.data, a.name);
            n++;
        }
        expect(n).toBeGreaterThanOrEqual(15);
    });

    it('agrees on the rule fixtures and on 28,800 byte mutations of them', () => {
        const shapes = [
            baseMint(false),
            baseMint(true),
            extendedMint(
                [
                    [18, 64],
                    [19, 152],
                ],
                false,
            ),
            extendedMint(
                [
                    [18, 8],
                    [12, 8],
                    [0, 0],
                ],
                true,
            ),
            extendedMint(
                [
                    [14, 64],
                    [1, 108],
                    [22, 32],
                ],
                false,
            ),
            [...extendedMint([[12, 4]], false), 19, 0, 9],
        ];
        const owners: (Address | null)[] = [TOKEN_PROGRAM, TOKEN_2022_PROGRAM, SYSTEM_PROGRAM, null];
        const keys: Address[] = [KEY, WSOL_MINT, WSOL_2022_MINT];
        const r = rng(0x5eed_0003n);
        let n = 0;
        for (const s of shapes) {
            for (const owner of owners) {
                for (const key of keys) {
                    same(key, owner, owner === null ? null : Uint8Array.from(s), `${owner} ${key} as is`);
                    for (let i = 0; i < 400; i++) {
                        const d = [...s];
                        const flips = Number(r.next() % 4n);
                        for (let f = 0; f < flips; f++) {
                            d[Number(r.next() % BigInt(d.length))] = Number(r.next() % 256n);
                        }
                        // Bias some flips onto the rule bytes: tags, is_initialized, account type, TLV heads.
                        if (i % 3 === 0) {
                            const at = [0, 45, 46, 165, 166, 167, 170, 171][Number(r.next() % 8n)] ?? 0;
                            if (at < d.length) d[at] = Number(r.next() % 4n);
                        }
                        const len = i % 5 === 0 ? Number(r.next() % BigInt(d.length + 1)) : d.length;
                        same(key, owner, owner === null ? null : Uint8Array.from(d.slice(0, len)), `${i}`);
                        n++;
                    }
                }
            }
        }
        expect(n).toBe(shapes.length * owners.length * keys.length * 400);
    }, 60_000);
});
