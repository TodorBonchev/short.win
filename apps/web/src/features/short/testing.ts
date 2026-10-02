/**
 * Test helpers (Vitest only): the program-derived vectors of `vectors/program-vectors.json` (the
 * program on LiteSVM, every account dumped in the instruction's own second) as snapshots.
 */
import type { Address } from '@solana/kit';
import { getBase64Encoder } from '@solana/kit';
import { initMathSync, type MathModule } from '../../math/load.ts';
import { readRepo, readRepoBytes } from '../../test/repo.ts';
import type { RawAccount } from '../../venues/readers.ts';
import { type AccountSource, assembleSnapshot, type ChainSnapshot } from './snapshot.ts';

export interface VectorCase {
    readonly name: string;
    readonly kind: 'open' | 'close' | 'add_margin';
    readonly mint: string;
    readonly borrower: string;
    readonly nonce: number;
    readonly q?: string;
    readonly position?: string;
    readonly amount?: string;
    readonly clock: { readonly unix: number; readonly slot: number };
    readonly accounts: Readonly<Record<string, { owner: string; lamports: string; data: string }>>;
    readonly result: {
        readonly ok: boolean;
        readonly code?: number | null;
        readonly event?: Readonly<Record<string, string | number>>;
        readonly position?: Readonly<Record<string, string | number>>;
        readonly amount?: string;
    };
}

let cases: VectorCase[] | null = null;
let math: MathModule | null = null;

export function programVectors(): VectorCase[] {
    cases ??= (
        JSON.parse(readRepo('apps/web/src/features/short/vectors/program-vectors.json')) as { cases: VectorCase[] }
    ).cases;
    return cases;
}

export function vectorCase(name: string): VectorCase {
    const c = programVectors().find((x) => x.name === name);
    if (!c) throw new Error(`no vector ${name}`);
    return c;
}

export function testMath(): MathModule {
    math ??= initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
    return math;
}

const b64 = getBase64Encoder();

export function vectorSource(c: VectorCase): AccountSource {
    const map = new Map<string, RawAccount>();
    for (const [k, v] of Object.entries(c.accounts)) {
        map.set(k, {
            address: k as Address,
            owner: v.owner as Address,
            lamports: BigInt(v.lamports),
            executable: false,
            data: new Uint8Array(b64.encode(v.data)),
        });
    }
    return (a) => map.get(a) ?? null;
}

export async function vectorSnapshot(c: VectorCase): Promise<{ s: ChainSnapshot; get: AccountSource }> {
    const get = vectorSource(c);
    const s = await assembleSnapshot(testMath(), {
        mint: c.mint as Address,
        now: BigInt(c.clock.unix),
        slot: BigInt(c.clock.slot),
        get,
        fetchedAt: 0,
        allowTestVenue: true,
    });
    if (!s) throw new Error(`${c.name}: no snapshot`);
    return { s, get };
}
