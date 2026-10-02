/**
 * Raw chain reads the MARKETS pages need beyond `src/rpc` (FRONTEND.md §6.5): a raw account (the
 * create-market safety verdict runs on the mint's bytes, FE-D5), rent minimums, and PumpSwap's
 * `GlobalConfig.disable_flags` (the graduation halt of a curve market, M6-SEC-3). Kept in one module
 * so the page tests mock the chain at this boundary.
 */
import { type Address, address, getBase64Encoder } from '@solana/kit';
import { PUMP_AMM_PROGRAM } from '../../../config/programs.ts';
import type { AppRpc } from '../../../rpc/client.ts';

export interface RawAccount {
    readonly address: Address;
    readonly owner: Address;
    readonly lamports: bigint;
    readonly data: Uint8Array;
    readonly executable: boolean;
}

const b64 = getBase64Encoder();

/** The account at `addr` at `confirmed`, or `null` when none exists. */
export async function readRawAccount(rpc: AppRpc, addr: Address): Promise<RawAccount | null> {
    const { value } = await rpc.getAccountInfo(addr, { encoding: 'base64', commitment: 'confirmed' }).send();
    if (!value) return null;
    return {
        address: addr,
        owner: value.owner,
        lamports: BigInt(value.lamports),
        data: new Uint8Array(b64.encode(value.data[0])),
        executable: value.executable,
    };
}

/** `getMinimumBalanceForRentExemption` for each size (lamports), in the order given. */
export async function readRentMinimums(rpc: AppRpc, sizes: readonly number[]): Promise<bigint[]> {
    return Promise.all(sizes.map(async (s) => BigInt(await rpc.getMinimumBalanceForRentExemption(BigInt(s)).send())));
}

/** PumpSwap's `GlobalConfig` (constants.rs `PUMP_AMM_GLOBAL_CONFIG`). */
export const PUMP_AMM_GLOBAL_CONFIG: Address = address('ADyA8hdefvWN2dbGGWFotbzWxrAvLW83WG6QCVXvJKqw');
/** Anchor discriminator of PumpSwap's `GlobalConfig` (crates/shortwin-venue-state `pump_amm.rs`). */
export const PAMM_GLOBAL_CONFIG_DISCRIMINATOR = [0x95, 0x08, 0x9c, 0xca, 0xa0, 0xfc, 0xb0, 0xd9] as const;
/** `disable_flags` offset (C6 @56). */
export const PAMM_DISABLE_FLAGS_OFFSET = 56;
/** `disable_flags` bit 0: `create_pool` (and with it pump's `migrate_v2`) halted (T-DISABLE). */
export const PAMM_CREATE_POOL_DISABLED_BIT = 1;

/**
 * Whether PumpSwap pool creation is halted, from its `GlobalConfig` bytes; `null` if the account
 * is missing or does not read as PumpSwap's config (e.g. on a local validator without it).
 */
export function pammCreatePoolDisabled(acc: RawAccount | null): boolean | null {
    if (!acc || acc.owner !== PUMP_AMM_PROGRAM) return null;
    const d = acc.data;
    if (d.length <= PAMM_DISABLE_FLAGS_OFFSET) return null;
    if (PAMM_GLOBAL_CONFIG_DISCRIMINATOR.some((b, i) => d[i] !== b)) return null;
    return ((d[PAMM_DISABLE_FLAGS_OFFSET] ?? 0) & PAMM_CREATE_POOL_DISABLED_BIT) !== 0;
}
