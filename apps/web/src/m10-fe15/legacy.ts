/**
 * FE-15 (M10): the TypeScript ports the app ran until M10, frozen verbatim as test oracles. The app
 * now calls the WASM facade (`@shortwin/math-wasm`: `spotNearEma`, `pOpenMax`, `lambdaBps`,
 * `lockedDepth`, `lockedSol`, `lockedFloorOk`, `mintCheck`); `parity.test.ts` holds the facade to
 * these copies and both to the Rust crates' own vectors. Never imported by the app.
 *
 * Sources (commit 7d19feb): `features/short/previews.ts` (`P_OPEN_MAX`, `spotNearEma`),
 * `venues/readers.ts` (`lambdaBps`), `venues/state.ts` (`lockedDepth`), `venues/candidates.ts`
 * (`lockedSol`, `lockedFloorOk`), `features/markets/create/mintSafety.ts` (`checkMintSafety`; its
 * `extensionName` in a `detail` replaced by the bare type).
 */
import { type Address, getAddressDecoder } from '@solana/kit';
import { TOKEN_2022_PROGRAM, TOKEN_PROGRAM, WSOL_2022_MINT, WSOL_MINT } from '../config/programs.ts';
import type { MathModule } from '../math/load.ts';

// ---- features/short/previews.ts ------------------------------------------------------------------

/** `P_OPEN_MAX` (WAD; `shortwin_math::consts`): opens refuse above it. */
export const P_OPEN_MAX = 10n ** 27n;

/** `caps::spot_near_ema`: `|P_spot − P_ema|·10⁴ ≤ dev·P_ema`; `false` without an EMA. */
export function spotNearEma(pSpot: bigint, pEma: bigint, devBps: number): boolean {
    if (pEma === 0n) return false;
    const diff = pSpot > pEma ? pSpot - pEma : pEma - pSpot;
    return diff * 10_000n <= BigInt(devBps) * pEma;
}

// ---- venues/readers.ts, venues/state.ts, venues/candidates.ts -------------------------------------

/** `locked_lp_bps` / `pool_select::lambda_bps`: `⌊(counter − supply)·10⁴/counter⌋`, 0 when unlocked. */
export function lambdaBps(lpCounter: bigint, lpSupply: bigint): number {
    if (lpCounter === 0n || lpSupply > lpCounter) return 0;
    return Number(((lpCounter - lpSupply) * 10_000n) / lpCounter);
}

/** `pool_select::locked_depth`: `⌊λ_bps·isqrt(X·Y)/10⁴⌋` (the root by the facade's `liquidity`). */
export function lockedDepth(m: MathModule, lpCounter: bigint, lpSupply: bigint, x: bigint, y: bigint): bigint {
    return (BigInt(lambdaBps(lpCounter, lpSupply)) * m.liquidity(x, y)) / 10_000n;
}

/** `tier::locked_sol`: `⌊D·isqrt(P)/10⁹⌋` (the root by the facade's `liquidity(P, 1)`). */
export function lockedSol(m: MathModule, depth: bigint, p: bigint): bigint {
    return (depth * m.liquidity(p, 1n)) / 1_000_000_000n;
}

/** `pool_select::locked_floor_ok`: `pool_min_real_sol·λ_min ≤ locked_sol·10⁴`; false for `λ_min = 0`. */
export function lockedFloorOk(
    m: MathModule,
    depth: bigint,
    pRef: bigint,
    lambdaMinBps: number,
    minRealSol: bigint,
): boolean {
    if (lambdaMinBps === 0) return false;
    return minRealSol * BigInt(lambdaMinBps) <= lockedSol(m, depth, pRef) * 10_000n;
}

// ---- features/markets/create/mintSafety.ts -------------------------------------------------------

/** Length of a base mint (both programs). */
export const MINT_LEN = 82;
/** `Account::LEN`: an extended mint is zero-padded to it. */
export const ACCOUNT_LEN = 165;
/** `Multisig::LEN`: never a valid extended account length. */
export const MULTISIG_LEN = 355;
/** Offset of the Token-2022 `AccountType` byte. */
export const ACCOUNT_TYPE_OFFSET = ACCOUNT_LEN;
/** Offset of the first TLV entry. */
export const TLV_OFFSET = ACCOUNT_LEN + 1;
/** `AccountType::Mint`. */
export const ACCOUNT_TYPE_MINT = 1;

/** The v1 allowlist (SPEC §3; ARCH §3.2 check 1; R-MINT §4): display and collection data only. */
export const ALLOWED_EXTENSIONS: readonly number[] = [18, 19, 20, 21, 22, 23];

/** The program errors of the mint group (ARCH §3.11), in the order the program checks them. */
export type MintRejectCode =
    | 'MintUnsupportedProgram'
    | 'MintNative'
    | 'MintMalformed'
    | 'MintFreezeAuthority'
    | 'MintDisallowedExtension';

export interface MintReason {
    readonly code: MintRejectCode;
    /** What exactly failed, in plain words (the mapped program message is shown next to it). */
    readonly detail: string;
    /** For `MintDisallowedExtension`: the extension type, or `null` for a TLV that does not parse. */
    readonly extension?: number | null;
}

export interface MintBase {
    readonly mintAuthority: Address | null;
    readonly supply: bigint;
    readonly decimals: number;
    readonly freezeAuthority: Address | null;
}

export interface MintVerdict {
    readonly ok: boolean;
    /** What `create_market` returns (the first failing rule), `null` when the mint passes. */
    readonly programError: MintRejectCode | null;
    /** Every failing rule; `reasons[0].code === programError`. */
    readonly reasons: readonly MintReason[];
    readonly program: 'token' | 'token-2022' | null;
    /** The base fields when they unpack. */
    readonly base: MintBase | null;
    /** Extension types in TLV order (allowed and not), for display. */
    readonly extensions: readonly number[];
}

const addressDecoder = getAddressDecoder();

function u16(d: Uint8Array, off: number): number {
    return (d[off] ?? 0) | ((d[off + 1] ?? 0) << 8);
}

function u32(d: Uint8Array, off: number): number {
    return ((d[off] ?? 0) | ((d[off + 1] ?? 0) << 8) | ((d[off + 2] ?? 0) << 16) | ((d[off + 3] ?? 0) << 24)) >>> 0;
}

function u64(d: Uint8Array, off: number): bigint {
    let v = 0n;
    for (let i = 7; i >= 0; i--) v = (v << 8n) | BigInt(d[off + i] ?? 0);
    return v;
}

/** A `COption<Pubkey>`: tag 0 = None, 1 = Some, anything else malformed (`undefined`). */
function coptionKey(d: Uint8Array, off: number): Address | null | undefined {
    const tag = u32(d, off);
    if (tag === 0) return null;
    if (tag === 1) return addressDecoder.decode(d.subarray(off + 4, off + 36));
    return undefined;
}

/** `base_of`: both `COption` tags valid and `is_initialized == 1`, as `Mint::unpack` reads it. */
function baseOf(d: Uint8Array): MintBase | null {
    const mintAuthority = coptionKey(d, 0);
    if (mintAuthority === undefined) return null;
    const init = d[45];
    if (init !== 0 && init !== 1) return null;
    const freezeAuthority = coptionKey(d, 46);
    if (freezeAuthority === undefined) return null;
    if (init !== 1) return null;
    return { mintAuthority, supply: u64(d, 36), decimals: d[44] ?? 0, freezeAuthority };
}

interface TlvWalk {
    readonly types: number[];
    /** Disallowed types in order. */
    readonly disallowed: number[];
    /** The walk stopped on a truncated entry (`DisallowedExtension(None)`). */
    readonly malformed: boolean;
    /** Which came first: a disallowed type or the malformed entry (for `programError`). */
}

/**
 * `check_tlv`, walked to the end: an entry ends the walk when fewer than 2 bytes remain or its type
 * is 0; a type with no room for its length or a value past the end is a malformed TLV.
 */
function walkTlv(tlv: Uint8Array): TlvWalk {
    const types: number[] = [];
    const disallowed: number[] = [];
    let at = 0;
    while (at < tlv.length) {
        if (at + 2 > tlv.length) break;
        const t = u16(tlv, at);
        if (t === 0) break;
        types.push(t);
        if (!ALLOWED_EXTENSIONS.includes(t)) disallowed.push(t);
        if (at + 4 > tlv.length) return { types, disallowed, malformed: true };
        const end = at + 4 + u16(tlv, at + 2);
        if (end > tlv.length) return { types, disallowed, malformed: true };
        at = end;
    }
    return { types, disallowed, malformed: false };
}

function verdict(
    reasons: MintReason[],
    program: MintVerdict['program'],
    base: MintBase | null,
    extensions: readonly number[],
): MintVerdict {
    return {
        ok: reasons.length === 0,
        programError: reasons[0]?.code ?? null,
        reasons,
        program,
        base,
        extensions,
    };
}

/**
 * The SPEC §3 allowlist filter on raw bytes, with the program's decision and first reason:
 * owner ∈ {SPL Token, Token-2022}; not a native mint; a well-formed base (SPL Token exactly 82
 * bytes; Token-2022 82 bytes or ≥ 166 with zero padding and `AccountType::Mint`, never 355); no
 * freeze authority; every TLV entry allowlisted and the walk parses.
 *
 * `owner` / `data` are `null` when no account exists at `key` (the program then sees a
 * System-owned account: `MintUnsupportedProgram`).
 */
export function checkMintSafety(key: Address, owner: Address | null, data: Uint8Array | null): MintVerdict {
    const legacy = owner === TOKEN_PROGRAM;
    if (owner === null || data === null) {
        return verdict(
            [{ code: 'MintUnsupportedProgram', detail: 'No account exists at this address.' }],
            null,
            null,
            [],
        );
    }
    if (!legacy && owner !== TOKEN_2022_PROGRAM) {
        return verdict(
            [{ code: 'MintUnsupportedProgram', detail: `The account is owned by ${owner}, not a token program.` }],
            null,
            null,
            [],
        );
    }
    const program = legacy ? 'token' : 'token-2022';
    const reasons: MintReason[] = [];
    if (key === WSOL_MINT || key === WSOL_2022_MINT) {
        reasons.push({ code: 'MintNative', detail: 'This is the wrapped-SOL (native) mint.' });
    }
    const badLength = (legacy && data.length !== MINT_LEN) || data.length < MINT_LEN || data.length === MULTISIG_LEN;
    const base = badLength ? null : baseOf(data);
    if (!base) {
        reasons.push({
            code: 'MintMalformed',
            detail: badLength
                ? `The account is ${data.length} bytes, which is not a valid ${legacy ? 'SPL Token' : 'Token-2022'} mint.`
                : 'The mint’s base fields do not unpack (uninitialized or corrupt).',
        });
        return verdict(reasons, program, null, []);
    }
    let tlv: Uint8Array = new Uint8Array(0);
    if (data.length > MINT_LEN) {
        const padding = data.length >= ACCOUNT_TYPE_OFFSET ? data.subarray(MINT_LEN, ACCOUNT_TYPE_OFFSET) : null;
        if (!padding || padding.some((b) => b !== 0)) {
            reasons.push({ code: 'MintMalformed', detail: 'The extension area’s padding is not zero.' });
            return verdict(reasons, program, base, []);
        }
        if (data[ACCOUNT_TYPE_OFFSET] !== ACCOUNT_TYPE_MINT) {
            reasons.push({ code: 'MintMalformed', detail: 'The account type is not a mint.' });
            return verdict(reasons, program, base, []);
        }
        tlv = data.subarray(TLV_OFFSET);
    }
    if (base.freezeAuthority !== null) {
        reasons.push({
            code: 'MintFreezeAuthority',
            detail: `Freeze authority: ${base.freezeAuthority}.`,
        });
    }
    const walk = walkTlv(tlv);
    for (const t of walk.disallowed) {
        reasons.push({
            code: 'MintDisallowedExtension',
            detail: `Extension ${String(t)} (${t}) is not allowed.`,
            extension: t,
        });
    }
    if (walk.malformed) {
        reasons.push({
            code: 'MintDisallowedExtension',
            detail: 'The extension list does not parse (a truncated entry).',
            extension: null,
        });
    }
    return verdict(reasons, program, base, walk.types);
}
