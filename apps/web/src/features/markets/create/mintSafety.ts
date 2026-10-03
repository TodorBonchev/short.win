/**
 * The create-market safety verdict (FRONTEND.md §3.5, FE-D5): `shortwin_venue_state::mint::check`
 * (crates/shortwin-venue-state/src/mint.rs, the byte-level copy of the program's `check_mint_safety`,
 * programs/shortwin/src/mint_filter.rs, which a differential test in the program crate holds to the
 * same verdict and reason; ARCH §3.2 check 1), run by the WASM facade's `mintCheck` on the mint's
 * bytes (FE-15: no TypeScript copy of the rule).
 *
 * `programError` is exactly what `create_market` would fail with (the first failing rule, in the
 * program's order). `reasons` additionally lists every other failing rule the bytes show (each
 * disallowed extension, a freeze authority next to one), so the user sees everything wrong with
 * the token at once — each one the filter's own verdict once the rules before it are lifted. This
 * module only words them. The verdict is confirmed by simulating `create_market` before signing.
 */
import { type Address, getAddressDecoder, getAddressEncoder } from '@solana/kit';
import { SYSTEM_PROGRAM, TOKEN_2022_PROGRAM, TOKEN_PROGRAM } from '../../../config/programs.ts';
import type { MathModule } from '../../../math/load.ts';

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
const addressEncoder = getAddressEncoder();

const REJECT_CODES: readonly MintRejectCode[] = [
    'MintUnsupportedProgram',
    'MintNative',
    'MintMalformed',
    'MintFreezeAuthority',
    'MintDisallowedExtension',
];

function rejectCode(code: string): MintRejectCode {
    const c = REJECT_CODES.find((r) => r === code);
    if (!c) throw new Error(`mintCheck: unknown rejection ${code}`);
    return c;
}

function addressOf(bytes: Uint8Array | undefined): Address | null {
    return bytes ? addressDecoder.decode(bytes) : null;
}

/** The words for one failing rule (the mapped program message is shown next to it). */
function detail(
    code: MintRejectCode,
    extension: number | null,
    data: Uint8Array,
    program: 'token' | 'token-2022',
    base: MintBase | null,
): string {
    switch (code) {
        case 'MintUnsupportedProgram':
            return 'The account is not owned by a token program.';
        case 'MintNative':
            return 'This is the wrapped-SOL (native) mint.';
        case 'MintMalformed':
            return `The account (${data.length} bytes) is not a valid ${program === 'token' ? 'SPL Token' : 'Token-2022'} mint: its length, base fields, padding or account type do not unpack.`;
        case 'MintFreezeAuthority':
            return `Freeze authority: ${base?.freezeAuthority ?? 'set'}.`;
        case 'MintDisallowedExtension':
            return extension === null
                ? 'The extension list does not parse (a truncated entry).'
                : `Extension ${extensionName(extension)} (${extension}) is not allowed.`;
    }
}

/**
 * The SPEC §3 allowlist filter on raw bytes, with the program's decision and first reason (the
 * facade's `mintCheck`): owner ∈ {SPL Token, Token-2022}; not a native mint; a well-formed base; no
 * freeze authority; every TLV entry allowlisted and the walk parses.
 *
 * `owner` / `data` are `null` when no account exists at `key` (the program then sees a
 * System-owned account: `MintUnsupportedProgram`).
 */
export function checkMintSafety(
    m: MathModule,
    key: Address,
    owner: Address | null,
    data: Uint8Array | null,
): MintVerdict {
    const v = m.mintCheck(
        addressEncoder.encode(key) as Uint8Array,
        addressEncoder.encode(owner ?? SYSTEM_PROGRAM) as Uint8Array,
        data ?? new Uint8Array(0),
    );
    try {
        const program = owner === TOKEN_PROGRAM ? 'token' : owner === TOKEN_2022_PROGRAM ? 'token-2022' : null;
        const base: MintBase | null = v.hasBase
            ? {
                  mintAuthority: addressOf(v.mintAuthority),
                  supply: v.supply,
                  decimals: v.decimals,
                  freezeAuthority: addressOf(v.freezeAuthority),
              }
            : null;
        const exts = v.extensions;
        const reasons: MintReason[] = v.codes.map((c, i) => {
            const code = rejectCode(c);
            if (owner === null || data === null || program === null) {
                return {
                    code,
                    detail:
                        owner === null || data === null
                            ? 'No account exists at this address.'
                            : `The account is owned by ${owner}, not a token program.`,
                };
            }
            const ext = exts[i] ?? -1;
            const extension = code === 'MintDisallowedExtension' ? (ext < 0 ? null : ext) : undefined;
            return {
                code,
                detail: detail(code, extension ?? null, data, program, base),
                ...(extension === undefined ? {} : { extension }),
            };
        });
        return {
            ok: v.ok,
            programError: reasons[0]?.code ?? null,
            reasons,
            program,
            base,
            extensions: [...v.types],
        };
    } finally {
        v.free();
    }
}

/** Whether the verdict rejects extension type `t` (a `MintDisallowedExtension` names it). */
export function extensionRejected(v: MintVerdict, t: number): boolean {
    return v.reasons.some((r) => r.code === 'MintDisallowedExtension' && r.extension === t);
}

/** The distinct failing program errors, in the program's order (one `createReason-<code>` each). */
export function reasonCodes(v: MintVerdict): MintRejectCode[] {
    return [...new Set(v.reasons.map((r) => r.code))];
}
