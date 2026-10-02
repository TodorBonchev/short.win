/**
 * The Kit client (FRONTEND.md §5.1, §6.5): `@solana/kit` 8.3 with `@solana/kit-plugin-rpc`'s
 * connection (RPC + subscriptions) and `@solana/kit-plugin-wallet`'s Wallet Standard state
 * (`walletWithoutSigner`: the transaction pipeline passes the connected signer explicitly, so
 * nothing signs implicitly). No web3.js v1, no wallet-adapter (ARCH §1.1).
 */
import { createClient } from '@solana/kit';
import { solanaRpcConnection } from '@solana/kit-plugin-rpc';
import { walletWithoutSigner } from '@solana/kit-plugin-wallet';
import type { AppConfig } from '../config/parseConfig.ts';

export function createAppClient(cfg: AppConfig) {
    return createClient()
        .use(solanaRpcConnection({ rpcUrl: cfg.rpcUrl, rpcSubscriptionsUrl: cfg.rpcWsUrl }))
        .use(walletWithoutSigner({ chain: cfg.chain, storageKey: 'shortwin:wallet' }));
}

export type AppClient = ReturnType<typeof createAppClient>;
export type AppRpc = AppClient['rpc'];
export type AppRpcSubscriptions = AppClient['rpcSubscriptions'];
