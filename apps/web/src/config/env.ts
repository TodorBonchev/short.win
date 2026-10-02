/** The runtime configuration (FRONTEND.md §2.1): `parseConfig` over `import.meta.env`. */
import { type AppConfig, parseConfig } from './parseConfig.ts';

export type { AppConfig, Cluster, WalletChain } from './parseConfig.ts';

export const config: AppConfig = parseConfig(import.meta.env);

/** An explorer link for a transaction signature. */
export function explorerTx(signature: string): string {
    return config.explorerTxUrl.replace('{sig}', encodeURIComponent(signature));
}
