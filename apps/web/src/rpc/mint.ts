/**
 * Mint reads (FRONTEND.md §3.5, §6.5): token program, decimals, authorities and the Token-2022
 * extension types — the inputs of the create-market safety verdict (the program's
 * `check_mint_safety`, ARCH §3.2 check 1) and of every token amount's formatting.
 */
import { type Address, getAddressDecoder, getBase64Encoder } from '@solana/kit';
import { TOKEN_2022_PROGRAM, TOKEN_PROGRAM } from '../config/programs.ts';
import type { AppRpc } from './client.ts';

export const MINT_BASE_LEN = 82;
const ACCOUNT_TYPE_OFFSET = 165;

export interface MintInfo {
    readonly address: Address;
    readonly program: 'token' | 'token-2022';
    readonly decimals: number;
    readonly supply: bigint;
    readonly initialized: boolean;
    readonly mintAuthority: Address | null;
    readonly freezeAuthority: Address | null;
    /** Token-2022 `ExtensionType` values present (empty for SPL Token); `null` if the TLV is malformed. */
    readonly extensions: readonly number[] | null;
}

const addr = getAddressDecoder();

function cOption(data: Uint8Array, off: number): Address | null {
    const tag =
        (data[off] ?? 0) | ((data[off + 1] ?? 0) << 8) | ((data[off + 2] ?? 0) << 16) | ((data[off + 3] ?? 0) << 24);
    return tag === 1 ? addr.decode(data.subarray(off + 4, off + 36)) : null;
}

function u64(data: Uint8Array, off: number): bigint {
    let v = 0n;
    for (let i = 7; i >= 0; i--) v = (v << 8n) | BigInt(data[off + i] ?? 0);
    return v;
}

/** Parses a mint account; `null` if the owner is neither token program or the data is too short. */
export function parseMint(address: Address, owner: Address, data: Uint8Array): MintInfo | null {
    const program = owner === TOKEN_PROGRAM ? 'token' : owner === TOKEN_2022_PROGRAM ? 'token-2022' : null;
    if (!program || data.length < MINT_BASE_LEN) return null;
    let extensions: number[] | null = [];
    if (program === 'token-2022' && data.length > MINT_BASE_LEN) {
        if (data.length <= ACCOUNT_TYPE_OFFSET || data[ACCOUNT_TYPE_OFFSET] !== 1) extensions = null;
        else {
            let off = ACCOUNT_TYPE_OFFSET + 1;
            while (off + 4 <= data.length) {
                const type = (data[off] ?? 0) | ((data[off + 1] ?? 0) << 8);
                const len = (data[off + 2] ?? 0) | ((data[off + 3] ?? 0) << 8);
                if (type === 0) break;
                if (off + 4 + len > data.length) {
                    extensions = null;
                    break;
                }
                extensions.push(type);
                off += 4 + len;
            }
        }
    }
    return {
        address,
        program,
        decimals: data[44] ?? 0,
        supply: u64(data, 36),
        initialized: data[45] === 1,
        mintAuthority: cOption(data, 0),
        freezeAuthority: cOption(data, 46),
        extensions,
    };
}

export async function readMint(rpc: AppRpc, mint: Address): Promise<MintInfo | null> {
    const { value } = await rpc.getAccountInfo(mint, { encoding: 'base64', commitment: 'confirmed' }).send();
    if (!value) return null;
    const bytes = new Uint8Array(getBase64Encoder().encode(value.data[0]));
    return parseMint(mint, value.owner, bytes);
}
