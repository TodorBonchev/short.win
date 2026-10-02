import { type Address, isAddress } from '@solana/kit';

/** A trimmed base58 address, or `null`. Markets are always keyed by mint (R-MINT §10.11). */
export function parseAddress(input: string): Address | null {
    const s = input.trim();
    return s.length >= 32 && s.length <= 44 && isAddress(s) ? s : null;
}
