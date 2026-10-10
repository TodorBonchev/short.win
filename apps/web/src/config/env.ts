/** The runtime configuration (FRONTEND.md §2.1): `parseConfig` over `import.meta.env`. */
import { type AppConfig, parseConfig } from './parseConfig.ts';

export type { AppConfig, Cluster, WalletChain } from './parseConfig.ts';

export const config: AppConfig = parseConfig(import.meta.env);

/** An explorer link for a transaction signature. */
export function explorerTx(signature: string): string {
    return config.explorerTxUrl.replace('{sig}', encodeURIComponent(signature));
}

/**
 * An explorer link for an account, derived from the transaction URL (`…/tx/{sig}…` →
 * `…/address/<address>…`, the Solana Explorer and Solana FM form); `null` when the configured URL
 * has no `/tx/{sig}` to derive it from.
 */
export function explorerAddress(address: string): string | null {
    const url = config.explorerTxUrl;
    if (!url.includes('/tx/{sig}')) return null;
    return url.replace('/tx/{sig}', `/address/${encodeURIComponent(address)}`);
}
