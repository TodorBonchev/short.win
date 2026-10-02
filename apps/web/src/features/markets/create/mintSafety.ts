/**
 * The create-market safety verdict (FRONTEND.md §3.5, FE-D5): a TypeScript port of
 * `shortwin_venue_state::mint::check` (crates/shortwin-venue-state/src/mint.rs), the byte-level copy
 * of the program's `check_mint_safety` (programs/shortwin/src/mint_filter.rs; ARCH §3.2 check 1),
 * which a differential test in the program crate holds to the same verdict and reason.
 *
 * `programError` is exactly what `create_market` would fail with (the first failing rule, in the
 * program's order). `reasons` additionally lists every other failing rule the bytes show (each
 * disallowed extension, a freeze authority next to one), so the user sees everything wrong with
 * the token at once. The verdict is confirmed by simulating `create_market` before signing.
 */
import { type Address, getAddressDecoder } from '@solana/kit';
import { TOKEN_2022_PROGRAM, TOKEN_PROGRAM, WSOL_2022_MINT, WSOL_MINT } from '../../../config/programs.ts';

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

/** Token-2022 `ExtensionType` names (spl-token-2022-interface; 28 is newer than the linked 2.1.x). */
export const EXTENSION_NAMES: Readonly<Record<number, string>> = {
    1: 'TransferFeeConfig',
    2: 'TransferFeeAmount',
    3: 'MintCloseAuthority',
    4: 'ConfidentialTransferMint',
    5: 'ConfidentialTransferAccount',
    6: 'DefaultAccountState',
    7: 'ImmutableOwner',
    8: 'MemoTransfer',
    9: 'NonTransferable',
    10: 'InterestBearingConfig',
    11: 'CpiGuard',
    12: 'PermanentDelegate',
    13: 'NonTransferableAccount',
    14: 'TransferHook',
    15: 'TransferHookAccount',
    16: 'ConfidentialTransferFeeConfig',
    17: 'ConfidentialTransferFeeAmount',
    18: 'MetadataPointer',
    19: 'TokenMetadata',
    20: 'GroupPointer',
    21: 'TokenGroup',
    22: 'GroupMemberPointer',
    23: 'TokenGroupMember',
    24: 'ConfidentialMintBurn',
    25: 'ScaledUiAmount',
    26: 'Pausable',
    27: 'PausableAccount',
    28: 'PermissionedBurn',
};

export function extensionName(t: number): string {
    return EXTENSION_NAMES[t] ?? `Unknown type ${t}`;
}

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
            detail: `Extension ${extensionName(t)} (${t}) is not allowed.`,
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

/** The distinct failing program errors, in the program's order (one `createReason-<code>` each). */
export function reasonCodes(v: MintVerdict): MintRejectCode[] {
    return [...new Set(v.reasons.map((r) => r.code))];
}
