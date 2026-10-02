/**
 * The TEST-ONLY mock venue `mock_cpamm` (tests/mock_cpamm/INTERFACE.md; A49): PDAs, the instruction
 * builders the seed and the specs use (pool creation, liquidity, swaps to move the price, knobs),
 * and the pool reader. Hand-encoded from the binding interface (it has no generated client); the
 * app never builds these, it reaches the mock only through short.win's `VenueKind::Mock`.
 */
import {
    AccountRole,
    type Address,
    getAddressEncoder,
    getProgramDerivedAddress,
    type Instruction,
    type ReadonlyUint8Array,
} from '@solana/kit';
import { getAccount, PROGRAMS, WSOL_MINT } from './chain.ts';

const enc = getAddressEncoder();
const text = new TextEncoder();

const DISC = {
    initializePool: [0x5f, 0xb4, 0x0a, 0xac, 0x54, 0xae, 0xe8, 0x28],
    addLiquidity: [0xb5, 0x9d, 0x59, 0x43, 0x8f, 0xb6, 0x34, 0x48],
    swapExactIn: [0x68, 0x68, 0x83, 0x56, 0xa1, 0xbd, 0xb4, 0xd8],
    setKnobs: [0x4c, 0x73, 0x3d, 0xa0, 0x62, 0x18, 0x6c, 0x4f],
} as const;

/** `MockPool` discriminator (`sha256("account:MockPool")[..8]`). */
export const POOL_DISCRIMINATOR = new Uint8Array([0x3a, 0xc4, 0xd0, 0xf1, 0x1f, 0xd8, 0xf3, 0x30]);

async function mockPda(seeds: ReadonlyUint8Array[]): Promise<Address> {
    const [a] = await getProgramDerivedAddress({ programAddress: PROGRAMS.mock, seeds });
    return a;
}

export interface MockPool {
    readonly pool: Address;
    readonly baseMint: Address;
    readonly baseVault: Address;
    readonly quoteVault: Address;
    readonly baseProgram: Address;
    readonly authority: Address;
}

/** The keys of the pool of `baseMint` (`["pool", base_mint, WSOL]`; derived, the pool need not exist). */
export async function derivePool(baseMint: Address, baseProgram: Address): Promise<MockPool> {
    const pool = await mockPda([text.encode('pool'), enc.encode(baseMint), enc.encode(WSOL_MINT)]);
    return {
        pool,
        baseMint,
        baseProgram,
        baseVault: await mockPda([text.encode('vault'), enc.encode(pool), enc.encode(baseMint)]),
        quoteVault: await mockPda([text.encode('vault'), enc.encode(pool), enc.encode(WSOL_MINT)]),
        authority: await mockPda([text.encode('authority')]),
    };
}

function u32(v: number): Uint8Array {
    const b = new Uint8Array(4);
    new DataView(b.buffer).setUint32(0, v, true);
    return b;
}

function u64(v: bigint): Uint8Array {
    const b = new Uint8Array(8);
    new DataView(b.buffer).setBigUint64(0, v, true);
    return b;
}

function data(disc: readonly number[], ...args: Uint8Array[]): Uint8Array {
    const len = 8 + args.reduce((n, a) => n + a.length, 0);
    const out = new Uint8Array(len);
    out.set(disc, 0);
    let o = 8;
    for (const a of args) {
        out.set(a, o);
        o += a.length;
    }
    return out;
}

const W = AccountRole.WRITABLE;
const R = AccountRole.READONLY;
const WS = AccountRole.WRITABLE_SIGNER;
const RS = AccountRole.READONLY_SIGNER;

/** `initialize_pool(fee_ppm)`; `admin` pays and becomes the pool admin. */
export function initializePoolIx(p: MockPool, admin: Address, feePpm: number): Instruction {
    return {
        programAddress: PROGRAMS.mock,
        accounts: [
            { address: admin, role: WS },
            { address: p.baseMint, role: R },
            { address: WSOL_MINT, role: R },
            { address: p.pool, role: W },
            { address: p.authority, role: R },
            { address: p.baseVault, role: W },
            { address: p.quoteVault, role: W },
            { address: p.baseProgram, role: R },
            { address: PROGRAMS.token, role: R },
            { address: PROGRAMS.system, role: R },
        ],
        data: data(DISC.initializePool, u32(feePpm)),
    };
}

/** `add_liquidity(base, quote)` from the depositor's base account and WSOL account. */
export function addLiquidityIx(
    p: MockPool,
    depositor: Address,
    baseSource: Address,
    quoteSource: Address,
    base: bigint,
    quote: bigint,
): Instruction {
    return {
        programAddress: PROGRAMS.mock,
        accounts: [
            { address: depositor, role: RS },
            { address: p.pool, role: R },
            { address: baseSource, role: W },
            { address: quoteSource, role: W },
            { address: p.baseVault, role: W },
            { address: p.quoteVault, role: W },
            { address: p.baseMint, role: R },
            { address: WSOL_MINT, role: R },
            { address: p.baseProgram, role: R },
            { address: PROGRAMS.token, role: R },
        ],
        data: data(DISC.addLiquidity, u64(base), u64(quote)),
    };
}

/**
 * `swap_exact_in(amount_in, min_out)`. `buy` = quote (WSOL) in, base out (the price rises);
 * otherwise base in, WSOL out (the price falls).
 */
export function swapExactInIx(
    p: MockPool,
    authority: Address,
    source: Address,
    destination: Address,
    buy: boolean,
    amountIn: bigint,
    minOut = 0n,
): Instruction {
    const [inVault, outVault, inMint, outMint, inProg, outProg] = buy
        ? [p.quoteVault, p.baseVault, WSOL_MINT, p.baseMint, PROGRAMS.token, p.baseProgram]
        : [p.baseVault, p.quoteVault, p.baseMint, WSOL_MINT, p.baseProgram, PROGRAMS.token];
    return {
        programAddress: PROGRAMS.mock,
        accounts: [
            { address: authority, role: RS },
            { address: p.pool, role: W },
            { address: p.authority, role: R },
            { address: source, role: W },
            { address: destination, role: W },
            { address: inVault, role: W },
            { address: outVault, role: W },
            { address: inMint, role: R },
            { address: outMint, role: R },
            { address: inProg, role: R },
            { address: outProg, role: R },
        ],
        data: data(DISC.swapExactIn, u64(amountIn), u64(minOut)),
    };
}

/** `set_knobs(disabled, fee_ppm, layout_version = 0, over_delivery = 0, required_extras = [])`. */
export function setKnobsIx(p: MockPool, admin: Address, disabled: boolean, feePpm: number): Instruction {
    return {
        programAddress: PROGRAMS.mock,
        accounts: [
            { address: admin, role: RS },
            { address: p.pool, role: W },
        ],
        data: data(
            DISC.setKnobs,
            new Uint8Array([disabled ? 1 : 0]),
            u32(feePpm),
            new Uint8Array([0]),
            u64(0n),
            u32(0),
        ),
    };
}

/** The pool's pricing reserves `X = base − counter`, `Y = quote − counter` (INTERFACE.md "Pool layout"). */
export async function reserves(p: MockPool): Promise<{ x: bigint; y: bigint; feePpm: number }> {
    const [pool, base, quote] = await Promise.all([
        getAccount(p.pool),
        getAccount(p.baseVault),
        getAccount(p.quoteVault),
    ]);
    if (!pool || !base || !quote) throw new Error(`mock pool ${p.pool} incomplete`);
    const v = new DataView(pool.data.buffer, pool.data.byteOffset);
    const amount = (d: Uint8Array) => new DataView(d.buffer, d.byteOffset).getBigUint64(64, true);
    return {
        x: amount(base.data) - v.getBigUint64(22, true),
        y: amount(quote.data) - v.getBigUint64(30, true),
        feePpm: v.getUint32(10, true),
    };
}
