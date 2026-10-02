/**
 * Little-endian field reads with zero padding (`crates/shortwin-venue-state/src/bytes.rs`, ARCH
 * §4.1): a read past the end of a shorter account returns zeros for the missing bytes, so an
 * account older than the newest layout reads its appended fields as 0 / false / the default key.
 */
import { type Address, getAddressDecoder } from '@solana/kit';

const addressDecoder = getAddressDecoder();

/** The default (all-zero) key: `11111111111111111111111111111111`. */
export const DEFAULT_KEY = '11111111111111111111111111111111' as Address;

export function u8At(d: Uint8Array, off: number): number {
    return d[off] ?? 0;
}

export function u16At(d: Uint8Array, off: number): number {
    return u8At(d, off) | (u8At(d, off + 1) << 8);
}

export function u32At(d: Uint8Array, off: number): number {
    return (u8At(d, off) | (u8At(d, off + 1) << 8) | (u8At(d, off + 2) << 16)) + u8At(d, off + 3) * 0x1000000;
}

function uintAt(d: Uint8Array, off: number, bytes: number): bigint {
    let v = 0n;
    for (let i = bytes - 1; i >= 0; i--) v = (v << 8n) | BigInt(u8At(d, off + i));
    return v;
}

export function u64At(d: Uint8Array, off: number): bigint {
    return uintAt(d, off, 8);
}

export function i64At(d: Uint8Array, off: number): bigint {
    return BigInt.asIntN(64, uintAt(d, off, 8));
}

export function u128At(d: Uint8Array, off: number): bigint {
    return uintAt(d, off, 16);
}

export function i128At(d: Uint8Array, off: number): bigint {
    return BigInt.asIntN(128, uintAt(d, off, 16));
}

/** The 32 bytes at `off` (zero-padded). */
export function bytesAt(d: Uint8Array, off: number, len: number): Uint8Array {
    const out = new Uint8Array(len);
    if (off < d.length) out.set(d.subarray(off, Math.min(d.length, off + len)));
    return out;
}

/** A 32-byte key at `off` as a base58 address (zero-padded). */
export function keyAt(d: Uint8Array, off: number): Address {
    return addressDecoder.decode(bytesAt(d, off, 32));
}

/** `true` if the first 8 bytes of `d` are `disc` (never for a shorter account). */
export function hasDiscriminator(d: Uint8Array, disc: readonly number[]): boolean {
    if (d.length < 8) return false;
    for (let i = 0; i < 8; i++) if (d[i] !== disc[i]) return false;
    return true;
}

/** An Anchor / Rust `bool` byte: 0 or 1, anything else is malformed (`null`). */
export function boolAt(d: Uint8Array, off: number): boolean | null {
    const b = u8At(d, off);
    return b === 0 ? false : b === 1 ? true : null;
}
