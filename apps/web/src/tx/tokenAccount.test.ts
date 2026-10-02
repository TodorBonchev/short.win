import { address } from '@solana/kit';
import { describe, expect, it } from 'vitest';
import { SYSTEM_PROGRAM, TOKEN_2022_PROGRAM, TOKEN_PROGRAM } from '../config/programs.ts';
import {
    EXT_CPI_GUARD,
    EXT_MEMO_TRANSFER,
    parseTokenAccount,
    TOKEN_ACCOUNT_BASE_LEN,
    tokenAmount,
} from './tokenAccount.ts';

const ACC = address('So11111111111111111111111111111111111111112');

function base(amount: bigint): Uint8Array {
    const b = new Uint8Array(TOKEN_ACCOUNT_BASE_LEN);
    for (let i = 0; i < 8; i++) b[64 + i] = Number((amount >> BigInt(8 * i)) & 0xffn);
    b[108] = 1; // state: initialized
    return b;
}

function t22(amount: bigint, exts: [number, number[]][]): Uint8Array {
    const parts: number[] = [...base(amount), 2];
    for (const [type, value] of exts)
        parts.push(type & 0xff, type >> 8, value.length & 0xff, value.length >> 8, ...value);
    return new Uint8Array(parts);
}

describe('token account inspection', () => {
    it('reads an SPL Token account without extensions', () => {
        const info = parseTokenAccount(ACC, TOKEN_PROGRAM, base(1_000n));
        expect(info).toMatchObject({ program: 'token', amount: 1_000n, cpiGuard: false, memoRequired: false });
    });

    it('detects CPI Guard and required memos on a Token-2022 account', () => {
        const data = t22(5n, [
            [7, []], // ImmutableOwner (zero-length)
            [EXT_MEMO_TRANSFER, [1]],
            [EXT_CPI_GUARD, [1]],
        ]);
        expect(parseTokenAccount(ACC, TOKEN_2022_PROGRAM, data)).toMatchObject({
            program: 'token-2022',
            amount: 5n,
            cpiGuard: true,
            memoRequired: true,
        });
    });

    it('reads disabled extensions as off and ignores a malformed TLV tail', () => {
        const data = t22(0n, [
            [EXT_CPI_GUARD, [0]],
            [EXT_MEMO_TRANSFER, [0]],
        ]);
        expect(parseTokenAccount(ACC, TOKEN_2022_PROGRAM, data)).toMatchObject({
            cpiGuard: false,
            memoRequired: false,
        });
        const truncated = t22(0n, [[EXT_CPI_GUARD, [1]]]).slice(0, TOKEN_ACCOUNT_BASE_LEN + 3);
        expect(parseTokenAccount(ACC, TOKEN_2022_PROGRAM, truncated)?.cpiGuard).toBe(false);
    });

    it('refuses accounts of other programs and short data', () => {
        expect(parseTokenAccount(ACC, SYSTEM_PROGRAM, base(1n))).toBeNull();
        expect(parseTokenAccount(ACC, TOKEN_PROGRAM, new Uint8Array(10))).toBeNull();
        expect(tokenAmount(new Uint8Array(10))).toBeNull();
    });
});
