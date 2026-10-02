/**
 * Program-derived addresses of the venues (R-PUMP §3.1, R-VEN §1.3 / §2.2, R-RAY §1.1; ARCH §3.12
 * "seed gotchas": pump's `creator-vault` has a hyphen, PumpSwap's `creator_vault` an underscore).
 * The same derivations as the program's and the keeper's (`venues/{pump_curve,pump_amm,mock}.rs`,
 * `services/keeper/src/pda.rs`); `lists.test.ts` checks them through the keeper's vectors.
 */

import {
    type Address,
    getAddressEncoder,
    getProgramDerivedAddress,
    getU8Encoder,
    getU16Encoder,
    type ProgramDerivedAddress,
} from '@solana/kit';
import { findAssociatedTokenPda } from '@solana-program/token';
import {
    MOCK_CPAMM_PROGRAM,
    PUMP_AMM_PROGRAM,
    PUMP_FEES_PROGRAM,
    PUMP_PROGRAM,
    SHORTWIN_PROGRAM,
    TOKEN_PROGRAM,
    WSOL_MINT,
} from './constants.ts';

const addr = getAddressEncoder();
const text = new TextEncoder();

function pda(program: Address, seeds: readonly Uint8Array[]): Promise<Address> {
    return getProgramDerivedAddress({ programAddress: program, seeds: [...seeds] }).then(([a]) => a);
}

function seedKey(a: Address): Uint8Array {
    return new Uint8Array(addr.encode(a));
}

/** `ATA(owner, mint)` under `tokenProgram`. */
export async function ata(owner: Address, mint: Address, tokenProgram: Address): Promise<Address> {
    const [a] = await findAssociatedTokenPda({ owner, mint, tokenProgram });
    return a;
}

/** `ATA(owner, WSOL)` under SPL Token. */
export function wsolAta(owner: Address): Promise<Address> {
    return ata(owner, WSOL_MINT, TOKEN_PROGRAM);
}

// ---- pump ---------------------------------------------------------------------------------

export function bondingCurve(mint: Address): Promise<Address> {
    return pda(PUMP_PROGRAM, [text.encode('bonding-curve'), seedKey(mint)]);
}

/** `PDA(pump, ["pool-authority", mint])`: the canonical PumpSwap pool's `creator`. */
export function poolAuthority(mint: Address): Promise<Address> {
    return pda(PUMP_PROGRAM, [text.encode('pool-authority'), seedKey(mint)]);
}

/** `PDA(pump, ["creator-vault", creator])` (hyphen). */
export function pumpCreatorVault(creator: Address): Promise<Address> {
    return pda(PUMP_PROGRAM, [text.encode('creator-vault'), seedKey(creator)]);
}

/** `PDA(pump, ["user_volume_accumulator", user])`. */
export function pumpUserVolumeAccumulator(user: Address): Promise<Address> {
    return pda(PUMP_PROGRAM, [text.encode('user_volume_accumulator'), seedKey(user)]);
}

/** `PDA(pfee, ["sharing-config", mint])`. */
export function sharingConfig(mint: Address): Promise<Address> {
    return pda(PUMP_FEES_PROGRAM, [text.encode('sharing-config'), seedKey(mint)]);
}

// ---- PumpSwap -----------------------------------------------------------------------------

/** The canonical pool `PDA(pAMM, ["pool", 0u16 LE, pool_authority(mint), mint, WSOL])`. */
export async function canonicalPool(mint: Address, authority?: Address): Promise<Address> {
    const auth = authority ?? (await poolAuthority(mint));
    return pda(PUMP_AMM_PROGRAM, [
        text.encode('pool'),
        new Uint8Array(getU16Encoder().encode(0)),
        seedKey(auth),
        seedKey(mint),
        seedKey(WSOL_MINT),
    ]);
}

/** `PDA(pAMM, ["pool_lp_mint", pool])`. */
export function pammLpMint(pool: Address): Promise<Address> {
    return pda(PUMP_AMM_PROGRAM, [text.encode('pool_lp_mint'), seedKey(pool)]);
}

/** `PDA(pAMM, ["pool-v2", mint])` (a marker; need not exist). */
export function pammPoolV2(mint: Address): Promise<Address> {
    return pda(PUMP_AMM_PROGRAM, [text.encode('pool-v2'), seedKey(mint)]);
}

/** `PDA(pAMM, ["boost_vault", pool])`. */
export function pammBoostVault(pool: Address): Promise<Address> {
    return pda(PUMP_AMM_PROGRAM, [text.encode('boost_vault'), seedKey(pool)]);
}

/** `PDA(pAMM, ["user_volume_accumulator", user])`. */
export function pammUserVolumeAccumulator(user: Address): Promise<Address> {
    return pda(PUMP_AMM_PROGRAM, [text.encode('user_volume_accumulator'), seedKey(user)]);
}

/** `PDA(pAMM, ["creator_vault", coin_creator])` (underscore). */
export function pammCoinCreatorVaultAuthority(coinCreator: Address): Promise<Address> {
    return pda(PUMP_AMM_PROGRAM, [text.encode('creator_vault'), seedKey(coinCreator)]);
}

// ---- mock (TEST-ONLY) ---------------------------------------------------------------------

/** The mock venue's pool of `mint`: `PDA(mock, ["pool", mint, WSOL])`. */
export function mockPool(mint: Address): Promise<Address> {
    return pda(MOCK_CPAMM_PROGRAM, [text.encode('pool'), seedKey(mint), seedKey(WSOL_MINT)]);
}

// ---- short.win ----------------------------------------------------------------------------

/** `["venue_extras", [venue]]` under short.win. */
export function venueExtras(seed: number): Promise<Address> {
    return pda(SHORTWIN_PROGRAM, [text.encode('venue_extras'), new Uint8Array(getU8Encoder().encode(seed))]);
}

/** Anchor's `emit_cpi!` authority of short.win (one derivation for the app: `src/rpc/pda.ts`). */
export { eventAuthority } from '../rpc/pda.ts';

/**
 * The canonical PDA of `program` for raw `seeds` and its bump (`find_program_address`), for the
 * extras templates (`lists.ts`).
 */
export function findPda(program: Address, seeds: readonly Uint8Array[]): Promise<ProgramDerivedAddress> {
    return getProgramDerivedAddress({ programAddress: program, seeds: [...seeds] });
}
