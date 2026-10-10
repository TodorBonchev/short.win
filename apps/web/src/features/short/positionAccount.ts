/**
 * SHORT track — reading one `ShortPosition` by address (FRONTEND.md §3.8). The generated decoder
 * decodes any account's bytes (a market read as a "position": D8), so the account is checked first:
 * owned by short.win, the `ShortPosition` discriminator, at least its size.
 */
import {
    decodeShortPosition,
    getShortPositionSize,
    SHORT_POSITION_DISCRIMINATOR,
    type ShortPosition,
} from '@shortwin/client';
import { type Address, fetchEncodedAccount } from '@solana/kit';
import { SHORTWIN_PROGRAM } from '../../config/programs.ts';
import type { AppRpc } from '../../rpc/client.ts';

/** Whether an account (owner, bytes) is a short.win `ShortPosition`. */
export function isShortPositionAccount(owner: Address, data: Uint8Array): boolean {
    if (owner !== SHORTWIN_PROGRAM || data.length < getShortPositionSize()) return false;
    return SHORT_POSITION_DISCRIMINATOR.every((b, i) => data[i] === b);
}

/** The position at `address`, or `null` when there is none (no account, or another kind of account). */
export async function readShortPosition(rpc: AppRpc, address: Address): Promise<ShortPosition | null> {
    const acc = await fetchEncodedAccount(rpc, address, { commitment: 'confirmed' });
    if (!acc.exists || !isShortPositionAccount(acc.programAddress, acc.data)) return null;
    return decodeShortPosition(acc).data;
}
