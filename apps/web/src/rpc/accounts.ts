/**
 * Direct account reads through RPC (FRONTEND.md §6.5): the source of every number a transaction
 * depends on (previews, bounds), and the fallback when the indexer is unavailable. Decoders come
 * from `@shortwin/client` (zero-copy layouts generated from the IDL, A67).
 */
import {
    fetchMaybeGlobalConfig,
    fetchMaybeLenderPosition,
    fetchMaybeMarket,
    fetchMaybeMarketVault,
    fetchMaybeOracleCache,
    fetchMaybeShortPosition,
    fetchMaybeVenueGuard,
    fetchMaybeWalletExposure,
    findConfigPda,
    findExposurePda,
    findLenderPda,
    findMarketPda,
    findMarketVaultPda,
    findOracleCachePda,
    findVenueGuardPda,
} from '@shortwin/client';
import type { Address } from '@solana/kit';
import type { AppRpc } from './client.ts';

const COMMITMENT = { commitment: 'confirmed' } as const;

export async function marketAddress(mint: Address): Promise<Address> {
    const [pda] = await findMarketPda({ mint });
    return pda;
}

export async function readMarket(rpc: AppRpc, mint: Address) {
    return fetchMaybeMarket(rpc, await marketAddress(mint), COMMITMENT);
}

export async function readMarketVault(rpc: AppRpc, mint: Address) {
    const [pda] = await findMarketVaultPda({ market: await marketAddress(mint) });
    return fetchMaybeMarketVault(rpc, pda, COMMITMENT);
}

export async function readLender(rpc: AppRpc, mint: Address, owner: Address) {
    const [pda] = await findLenderPda({ market: await marketAddress(mint), owner });
    return fetchMaybeLenderPosition(rpc, pda, COMMITMENT);
}

export async function readExposure(rpc: AppRpc, mint: Address, borrower: Address) {
    const [pda] = await findExposurePda({ market: await marketAddress(mint), borrower });
    return fetchMaybeWalletExposure(rpc, pda, COMMITMENT);
}

export async function readPosition(rpc: AppRpc, position: Address) {
    return fetchMaybeShortPosition(rpc, position, COMMITMENT);
}

export async function readGlobalConfig(rpc: AppRpc) {
    const [pda] = await findConfigPda();
    return fetchMaybeGlobalConfig(rpc, pda, COMMITMENT);
}

export async function readVenueGuard(rpc: AppRpc) {
    const [pda] = await findVenueGuardPda();
    return fetchMaybeVenueGuard(rpc, pda, COMMITMENT);
}

export async function readOracle(rpc: AppRpc) {
    const [pda] = await findOracleCachePda();
    return fetchMaybeOracleCache(rpc, pda, COMMITMENT);
}
