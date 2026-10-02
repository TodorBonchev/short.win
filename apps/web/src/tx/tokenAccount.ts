/**
 * Token-account inspection (ARCH §3.3; R-MINT §8.4; FRONTEND.md §5.7): which token program owns
 * an account, its balance, and the two owner-controlled Token-2022 extensions that change a flow —
 * `CpiGuard.lock_cpi` (a deposit through short.win's CPI fails, TokenError 42) and
 * `MemoTransfer.require_incoming_transfer_memos` (a withdrawal needs `memo_program`, else
 * TokenError 36 / short.win's `MemoProgramRequired`).
 *
 * Parsed by hand from the account bytes so the check has no dependency on a decoder's extension
 * coverage: base layout 165 bytes, then (Token-2022 only) the account-type byte at 165 = 2 and TLV
 * entries `[u16 type][u16 length][value]` from 166.
 */
import type { Address } from '@solana/kit';
import { TOKEN_2022_PROGRAM, TOKEN_PROGRAM } from '../config/programs.ts';

export const TOKEN_ACCOUNT_BASE_LEN = 165;
/** Token-2022 `ExtensionType` values (spl-token-2022-interface `extension/mod.rs`). */
export const EXT_MEMO_TRANSFER = 8;
export const EXT_CPI_GUARD = 11;
/** Token-2022 `TokenError` codes seen through short.win's CPIs (spl-token-2022-interface 2.1 `error.rs`). */
export const TOKEN_ERROR_NO_MEMO = 36;
export const TOKEN_ERROR_CPI_GUARD_TRANSFER_BLOCKED = 42;

export interface TokenAccountInfo {
    readonly address: Address;
    readonly program: 'token' | 'token-2022';
    readonly mint: Address | null;
    readonly owner: Address | null;
    readonly amount: bigint;
    /** `CpiGuard.lock_cpi` is set: short.win cannot transfer from this account by CPI. */
    readonly cpiGuard: boolean;
    /** `MemoTransfer.require_incoming_transfer_memos` is set: credits need a memo. */
    readonly memoRequired: boolean;
}

function readU64Le(b: Uint8Array, off: number): bigint {
    let v = 0n;
    for (let i = 7; i >= 0; i--) v = (v << 8n) | BigInt(b[off + i] ?? 0);
    return v;
}

/** Iterates Token-2022 TLV entries of an account (empty for a plain SPL Token account). */
export function* tlvEntries(data: Uint8Array): Generator<{ type: number; value: Uint8Array }> {
    if (data.length <= TOKEN_ACCOUNT_BASE_LEN || data[TOKEN_ACCOUNT_BASE_LEN] !== 2) return;
    let off = TOKEN_ACCOUNT_BASE_LEN + 1;
    while (off + 4 <= data.length) {
        const type = (data[off] ?? 0) | ((data[off + 1] ?? 0) << 8);
        const len = (data[off + 2] ?? 0) | ((data[off + 3] ?? 0) << 8);
        if (type === 0) return; // Uninitialized: padding
        const start = off + 4;
        if (start + len > data.length) return; // malformed: stop (callers treat as "no extension")
        yield { type, value: data.subarray(start, start + len) };
        off = start + len;
    }
}

/**
 * Parses a token account. `programOwner` is the account's owner program (from the RPC). Returns
 * `null` for an account that is not a token account of either program.
 */
export function parseTokenAccount(address: Address, programOwner: Address, data: Uint8Array): TokenAccountInfo | null {
    const program =
        programOwner === TOKEN_PROGRAM ? 'token' : programOwner === TOKEN_2022_PROGRAM ? 'token-2022' : null;
    if (!program || data.length < TOKEN_ACCOUNT_BASE_LEN) return null;
    let cpiGuard = false;
    let memoRequired = false;
    if (program === 'token-2022') {
        for (const e of tlvEntries(data)) {
            if (e.type === EXT_CPI_GUARD) cpiGuard = (e.value[0] ?? 0) !== 0;
            if (e.type === EXT_MEMO_TRANSFER) memoRequired = (e.value[0] ?? 0) !== 0;
        }
    }
    return {
        address,
        program,
        mint: null,
        owner: null,
        amount: readU64Le(data, 64),
        cpiGuard,
        memoRequired,
    };
}

/** The token amount at offset 64 of a token account (both programs), or `null` if too short. */
export function tokenAmount(data: Uint8Array): bigint | null {
    return data.length >= 72 ? readU64Le(data, 64) : null;
}
