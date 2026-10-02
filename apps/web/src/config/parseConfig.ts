/**
 * The app configuration (FRONTEND.md §2.1), parsed and validated from `VITE_*` variables. Pure:
 * `env.ts` applies it to `import.meta.env` at runtime and `vite.config.ts` at build time, so a bad
 * configuration fails the build rather than the user.
 */
import { type Address, isAddress } from '@solana/kit';

export type Cluster = 'mainnet' | 'devnet' | 'localnet';
export type WalletChain = `solana:${Cluster}`;

export interface AppConfig {
    readonly cluster: Cluster;
    readonly chain: WalletChain;
    readonly rpcUrl: string;
    readonly rpcWsUrl: string;
    readonly indexerUrl: string;
    readonly indexerWsUrl: string;
    readonly protocolAlt: Address | null;
    readonly priorityFeeMaxMicroLamports: bigint;
    readonly enableTestVenue: boolean;
    readonly explorerTxUrl: string;
}

export class ConfigError extends Error {
    readonly problems: readonly string[];

    constructor(problems: readonly string[]) {
        super(`invalid configuration:\n- ${problems.join('\n- ')}`);
        this.name = 'ConfigError';
        this.problems = problems;
    }
}

const CLUSTERS: readonly Cluster[] = ['mainnet', 'devnet', 'localnet'];

/** Localnet defaults (solana-test-validator, a local indexer): a bare build is a localnet build. */
export const LOCALNET_DEFAULTS: Readonly<Record<string, string>> = {
    VITE_RPC_URL: 'http://127.0.0.1:8899',
    VITE_RPC_WS_URL: 'ws://127.0.0.1:8900',
    VITE_INDEXER_URL: 'http://127.0.0.1:8080',
    VITE_INDEXER_WS_URL: 'ws://127.0.0.1:8080/v1/ws',
};

function url(raw: string | undefined, name: string, schemes: readonly string[], problems: string[]): string {
    if (!raw) {
        problems.push(`${name} is required`);
        return '';
    }
    try {
        const u = new URL(raw);
        if (!schemes.includes(u.protocol)) problems.push(`${name} must use ${schemes.join(' or ')}`);
        if (u.username || u.password) problems.push(`${name} must not carry credentials`);
        return raw.replace(/\/+$/, '');
    } catch {
        problems.push(`${name} is not a URL`);
        return '';
    }
}

/** Parses the `VITE_*` variables. Throws `ConfigError` listing every problem. */
export function parseConfig(env: Readonly<Record<string, string | boolean | undefined>>): AppConfig {
    const problems: string[] = [];
    const raw = (k: string): string | undefined => {
        const v = env[k];
        return typeof v === 'string' && v.trim() !== '' ? v.trim() : undefined;
    };
    const clusterRaw = raw('VITE_CLUSTER') ?? 'localnet';
    const s = (k: string): string | undefined =>
        raw(k) ?? (clusterRaw === 'localnet' ? LOCALNET_DEFAULTS[k] : undefined);
    const cluster = (CLUSTERS as readonly string[]).includes(clusterRaw) ? (clusterRaw as Cluster) : null;
    if (!cluster) problems.push(`VITE_CLUSTER must be one of ${CLUSTERS.join(', ')}`);
    const secure = cluster === 'mainnet';
    const http = secure ? ['https:'] : ['https:', 'http:'];
    const ws = secure ? ['wss:'] : ['wss:', 'ws:'];
    const rpcUrl = url(s('VITE_RPC_URL'), 'VITE_RPC_URL', http, problems);
    const rpcWsUrl = url(s('VITE_RPC_WS_URL'), 'VITE_RPC_WS_URL', ws, problems);
    const indexerUrl = url(s('VITE_INDEXER_URL'), 'VITE_INDEXER_URL', http, problems);
    const indexerWsUrl = url(s('VITE_INDEXER_WS_URL'), 'VITE_INDEXER_WS_URL', ws, problems);
    const altRaw = s('VITE_PROTOCOL_ALT');
    let protocolAlt: Address | null = null;
    if (altRaw) {
        if (isAddress(altRaw)) protocolAlt = altRaw;
        else problems.push('VITE_PROTOCOL_ALT is not a base58 address');
    }
    const feeRaw = s('VITE_PRIORITY_FEE_MAX_MICROLAMPORTS') ?? '1000000';
    let priorityFeeMaxMicroLamports = 0n;
    if (/^\d{1,12}$/.test(feeRaw)) priorityFeeMaxMicroLamports = BigInt(feeRaw);
    else problems.push('VITE_PRIORITY_FEE_MAX_MICROLAMPORTS must be an integer (µlamports/CU)');
    const testVenueRaw = env.VITE_ENABLE_TEST_VENUE;
    const enableTestVenue = testVenueRaw === true || testVenueRaw === 'true';
    if (enableTestVenue && cluster !== 'localnet') {
        problems.push('VITE_ENABLE_TEST_VENUE is TEST-ONLY and requires VITE_CLUSTER=localnet');
    }
    const explorerTxUrl = s('VITE_EXPLORER_TX_URL') ?? 'https://explorer.solana.com/tx/{sig}';
    if (!explorerTxUrl.includes('{sig}')) problems.push('VITE_EXPLORER_TX_URL must contain {sig}');
    if (problems.length > 0 || !cluster) throw new ConfigError(problems);
    return {
        cluster,
        chain: `solana:${cluster}`,
        rpcUrl,
        rpcWsUrl,
        indexerUrl,
        indexerWsUrl,
        protocolAlt,
        priorityFeeMaxMicroLamports,
        enableTestVenue,
        explorerTxUrl,
    };
}
