/**
 * The safety verdict (the facade's `mintCheck` = `shortwin_venue_state::mint::check`, FE-15) against
 * the fixtures of the program's own tests (programs/shortwin/src/mint_filter.rs `decisions`,
 * crates/shortwin-venue-state/src/mint.rs): one fixture per rule, every rejected extension type,
 * the TLV walk's edges and the malformed shapes. `src/m10-fe15/parity.test.ts` holds it to the
 * TypeScript port it replaced.
 */
import { type Address, address, getAddressEncoder } from '@solana/kit';
import { beforeAll, describe, expect, it } from 'vitest';
import {
    SYSTEM_PROGRAM,
    TOKEN_2022_PROGRAM,
    TOKEN_PROGRAM,
    WSOL_2022_MINT,
    WSOL_MINT,
} from '../../../config/programs.ts';
import { initMathSync, type MathModule } from '../../../math/load.ts';
import { readRepoBytes } from '../../../test/repo.ts';
import { checkMintSafety, extensionRejected, reasonCodes } from './mintSafety.ts';

/** The mint layout (crates/shortwin-venue-state/src/mint.rs), for building fixtures. */
const MINT_LEN = 82;
const MULTISIG_LEN = 355;
const ACCOUNT_TYPE_OFFSET = 165;
const TLV_OFFSET = 166;

let m: MathModule;
let allowedTypes: readonly number[];

beforeAll(() => {
    m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
    allowedTypes = [...m.allowedExtensions()];
});

const KEY = address('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
const AUTH = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const enc = getAddressEncoder();

function base(mintAuth = true, freeze = false): number[] {
    const d = new Array<number>(MINT_LEN).fill(0);
    if (mintAuth) {
        d[0] = 1;
        d.splice(4, 32, ...enc.encode(AUTH));
    }
    const supply = 1_000_000n;
    for (let i = 0; i < 8; i++) d[36 + i] = Number((supply >> BigInt(8 * i)) & 0xffn);
    d[44] = 6;
    d[45] = 1;
    if (freeze) {
        d[46] = 1;
        d.splice(50, 32, ...enc.encode(AUTH));
    }
    return d;
}

function extended(entries: readonly (readonly [number, number])[], freeze = false): number[] {
    const d = base(true, freeze);
    while (d.length < TLV_OFFSET) d.push(0);
    d[ACCOUNT_TYPE_OFFSET] = 1;
    for (const [t, len] of entries) {
        d.push(t & 0xff, t >> 8, len & 0xff, len >> 8);
        for (let i = 0; i < len; i++) d.push(0xab);
    }
    return d;
}

const run = (owner: Address | null, data: number[] | null, key: Address = KEY) =>
    checkMintSafety(m, key, owner, data === null ? null : new Uint8Array(data));

describe('checkMintSafety (the program’s rule, ARCH §3.2 check 1)', () => {
    it('passes plain SPL Token and Token-2022 mints and the pump.fun create_v2 shape', () => {
        expect(run(TOKEN_PROGRAM, base()).ok).toBe(true);
        expect(run(TOKEN_2022_PROGRAM, base(false)).ok).toBe(true);
        const pump = run(
            TOKEN_2022_PROGRAM,
            extended([
                [18, 64],
                [19, 152],
            ]),
        );
        expect(pump.ok).toBe(true);
        expect(pump.extensions).toEqual([18, 19]);
        expect(pump.base?.decimals).toBe(6);
        expect(pump.base?.supply).toBe(1_000_000n);
        const all = run(TOKEN_2022_PROGRAM, extended(allowedTypes.map((t) => [t, 16] as const)));
        expect(all.ok).toBe(true);
    });

    it('MintUnsupportedProgram: another owner, or no account at all', () => {
        expect(run(SYSTEM_PROGRAM, base()).programError).toBe('MintUnsupportedProgram');
        const missing = run(null, null);
        expect(missing.programError).toBe('MintUnsupportedProgram');
        expect(missing.reasons[0]?.detail).toMatch(/No account/);
    });

    it('MintNative: both wrapped-SOL mints', () => {
        expect(run(TOKEN_PROGRAM, base(false), WSOL_MINT).programError).toBe('MintNative');
        expect(run(TOKEN_2022_PROGRAM, base(false), WSOL_2022_MINT).programError).toBe('MintNative');
    });

    it('MintFreezeAuthority, also listed next to a disallowed extension', () => {
        expect(run(TOKEN_PROGRAM, base(true, true)).programError).toBe('MintFreezeAuthority');
        const both = run(
            TOKEN_2022_PROGRAM,
            extended(
                [
                    [18, 64],
                    [12, 32],
                ],
                true,
            ),
        );
        expect(both.programError).toBe('MintFreezeAuthority');
        expect(reasonCodes(both)).toEqual(['MintFreezeAuthority', 'MintDisallowedExtension']);
    });

    it('MintDisallowedExtension for every non-allowlisted type (spec rejections, account types, unknown)', () => {
        for (const t of [1, 3, 4, 6, 9, 10, 12, 14, 16, 24, 25, 26, 28, 29, 0xffff]) {
            const v = run(
                TOKEN_2022_PROGRAM,
                extended([
                    [18, 64],
                    [t, 8],
                ]),
            );
            expect(v.programError, String(t)).toBe('MintDisallowedExtension');
            expect(v.reasons[0]?.extension, String(t)).toBe(t);
        }
        for (const t of [2, 5, 7, 8, 11, 13, 15, 17, 27]) {
            expect(run(TOKEN_2022_PROGRAM, extended([[t, 8]])).programError, String(t)).toBe('MintDisallowedExtension');
        }
        for (let t = 1; t <= 64; t++) {
            const v = run(TOKEN_2022_PROGRAM, extended([[t, 8]]).slice(0, TLV_OFFSET + 12));
            expect(v.ok, String(t)).toBe(allowedTypes.includes(t));
        }
    });

    it('lists each disallowed extension (permanent delegate, transfer hook, transfer fee) by name', () => {
        const v = run(
            TOKEN_2022_PROGRAM,
            extended([
                [12, 32],
                [14, 64],
                [1, 108],
            ]),
        );
        expect(v.reasons.map((r) => r.extension)).toEqual([12, 14, 1]);
        expect(v.extensions).toEqual([12, 14, 1]);
        expect([12, 14, 1].every((t) => extensionRejected(v, t))).toBe(true);
        expect(allowedTypes).toEqual([18, 19, 20, 21, 22, 23]);
        expect(v.reasons.map((r) => r.detail).join(' ')).toMatch(/PermanentDelegate.*TransferHook.*TransferFeeConfig/);
        expect(reasonCodes(v)).toEqual(['MintDisallowedExtension']);
    });

    it('walks the TLV like get_extension_types: terminator, trailing byte, truncation', () => {
        const d = extended([
            [18, 4],
            [0, 0],
        ]);
        expect(run(TOKEN_2022_PROGRAM, d.slice(0, TLV_OFFSET + 16)).ok).toBe(true);
        expect(run(TOKEN_2022_PROGRAM, d.slice(0, TLV_OFFSET + 9)).ok).toBe(true);
        expect(run(TOKEN_2022_PROGRAM, d.slice(0, TLV_OFFSET)).ok).toBe(true);
        const typeOnly = run(TOKEN_2022_PROGRAM, d.slice(0, TLV_OFFSET + 3));
        expect(typeOnly.programError).toBe('MintDisallowedExtension');
        expect(typeOnly.reasons[0]?.extension).toBeNull();
        expect(run(TOKEN_2022_PROGRAM, d.slice(0, TLV_OFFSET + 7)).programError).toBe('MintDisallowedExtension');
        const cut = extended([[18, 64]]).slice(0, TLV_OFFSET + 20);
        expect(run(TOKEN_2022_PROGRAM, cut).programError).toBe('MintDisallowedExtension');
    });

    it('MintMalformed: lengths, padding, account type, token accounts, uninitialized, empty', () => {
        const d = extended([[18, 64]]);
        for (const len of [83, 100, 165]) {
            expect(run(TOKEN_2022_PROGRAM, d.slice(0, len)).programError, String(len)).toBe('MintMalformed');
        }
        const multisig = [...d];
        while (multisig.length < MULTISIG_LEN) multisig.push(0);
        expect(run(TOKEN_2022_PROGRAM, multisig.slice(0, MULTISIG_LEN)).programError).toBe('MintMalformed');
        const padding = [...d];
        padding[100] = 1;
        expect(run(TOKEN_2022_PROGRAM, padding).programError).toBe('MintMalformed');
        for (const accountType of [0, 2, 3]) {
            const t = [...d];
            t[ACCOUNT_TYPE_OFFSET] = accountType;
            expect(run(TOKEN_2022_PROGRAM, t).programError).toBe('MintMalformed');
        }
        // SPL Token has no extensions: any other length is malformed.
        expect(run(TOKEN_PROGRAM, extended([]).slice(0, 166)).programError).toBe('MintMalformed');
        // A token account passed as a mint.
        const account = new Array<number>(165).fill(0);
        account[108] = 1;
        expect(run(TOKEN_PROGRAM, account).programError).toBe('MintMalformed');
        const t22Account = [...account, 2, 0, 0, 0, 0];
        expect(run(TOKEN_2022_PROGRAM, t22Account).programError).toBe('MintMalformed');
        const uninit = base();
        uninit[45] = 0;
        expect(run(TOKEN_PROGRAM, uninit).programError).toBe('MintMalformed');
        const badTag = base();
        badTag[0] = 2;
        expect(run(TOKEN_PROGRAM, badTag).programError).toBe('MintMalformed');
        expect(run(TOKEN_PROGRAM, []).programError).toBe('MintMalformed');
    });

    it('reports the native mint first, as the program does', () => {
        const v = run(TOKEN_PROGRAM, base(false, true), WSOL_MINT);
        expect(reasonCodes(v)).toEqual(['MintNative', 'MintFreezeAuthority']);
        expect(v.programError).toBe('MintNative');
    });
});
