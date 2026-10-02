import { address, getAddressEncoder } from '@solana/kit';
import { describe, expect, it } from 'vitest';
import { SYSTEM_PROGRAM, TOKEN_2022_PROGRAM, TOKEN_PROGRAM } from '../config/programs.ts';
import { parseMint } from './mint.ts';

const MINT = address('So11111111111111111111111111111111111111112');
const AUTH = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');

function base({ freeze = false, decimals = 6, supply = 1_000n } = {}): number[] {
    const b = new Array<number>(82).fill(0);
    b[0] = 1;
    b.splice(4, 32, ...getAddressEncoder().encode(AUTH));
    for (let i = 0; i < 8; i++) b[36 + i] = Number((supply >> BigInt(8 * i)) & 0xffn);
    b[44] = decimals;
    b[45] = 1;
    if (freeze) {
        b[46] = 1;
        b.splice(50, 32, ...getAddressEncoder().encode(AUTH));
    }
    return b;
}

describe('parseMint', () => {
    it('reads an SPL Token mint', () => {
        const m = parseMint(MINT, TOKEN_PROGRAM, new Uint8Array(base({ decimals: 9, supply: 123n })));
        expect(m).toMatchObject({
            program: 'token',
            decimals: 9,
            supply: 123n,
            initialized: true,
            freezeAuthority: null,
        });
        expect(m?.mintAuthority).toBe(AUTH);
        expect(m?.extensions).toEqual([]);
    });

    it('reads a freeze authority and Token-2022 extension types', () => {
        const t22 = [
            ...base({ freeze: true }),
            ...new Array<number>(165 - 82).fill(0),
            1,
            18,
            0,
            64,
            0,
            ...new Array(64).fill(0),
            12,
            0,
            32,
            0,
            ...new Array(32).fill(0),
        ];
        const m = parseMint(MINT, TOKEN_2022_PROGRAM, new Uint8Array(t22));
        expect(m?.freezeAuthority).toBe(AUTH);
        expect(m?.extensions).toEqual([18, 12]); // MetadataPointer, PermanentDelegate
    });

    it('flags a malformed TLV and refuses other owners', () => {
        const bad = [...base(), ...new Array<number>(165 - 82).fill(0), 1, 12, 0, 200, 0, 1, 2];
        expect(parseMint(MINT, TOKEN_2022_PROGRAM, new Uint8Array(bad))?.extensions).toBeNull();
        expect(parseMint(MINT, SYSTEM_PROGRAM, new Uint8Array(base()))).toBeNull();
    });
});
