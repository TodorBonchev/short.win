/**
 * A display-only fallback for the controlled devnet smoke mint. Pump CreateV2 on this devnet
 * created a Token-2022 mint without the Metaplex account the indexer currently reads, so the
 * API knows its decimals but not its name or symbol. The exact mint remains visible beside the
 * symbol, and indexer metadata takes precedence whenever it becomes available.
 */
import type { Cluster } from './parseConfig.ts';

const SMOKE_MINT = 'GCLN3PspeYrV3U9BZeGbmn7VVf7kabW23h2gBHvoKRXD';

export function displayTokenMetadata(
    cluster: Cluster,
    mint: string,
    symbol: string | null | undefined,
    name: string | null | undefined,
): { symbol: string | null; name: string | null } {
    if (cluster === 'devnet' && mint === SMOKE_MINT) {
        return {
            symbol: symbol?.trim() ? symbol : 'SWTEST',
            name: name?.trim() ? name : 'ShortWin Devnet Test',
        };
    }
    return { symbol: symbol ?? null, name: name ?? null };
}
