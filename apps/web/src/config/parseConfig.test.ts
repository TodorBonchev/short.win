import { describe, expect, it } from 'vitest';
import { ConfigError, parseConfig } from './parseConfig.ts';

describe('parseConfig', () => {
    it('defaults to a localnet build', () => {
        const c = parseConfig({});
        expect(c.cluster).toBe('localnet');
        expect(c.chain).toBe('solana:localnet');
        expect(c.rpcUrl).toBe('http://127.0.0.1:8899');
        expect(c.indexerWsUrl).toBe('ws://127.0.0.1:8080/v1/ws');
        expect(c.enableTestVenue).toBe(false);
        expect(c.enableMobileWalletAdapter).toBe(true);
        expect(c.protocolAlt).toBeNull();
    });

    it('requires every URL off localnet and https/wss on mainnet', () => {
        expect(() => parseConfig({ VITE_CLUSTER: 'mainnet' })).toThrow(ConfigError);
        try {
            parseConfig({
                VITE_CLUSTER: 'mainnet',
                VITE_RPC_URL: 'http://rpc.example',
                VITE_RPC_WS_URL: 'wss://rpc.example',
                VITE_INDEXER_URL: 'https://api.short.win',
                VITE_INDEXER_WS_URL: 'wss://api.short.win/v1/ws',
            });
            expect.unreachable();
        } catch (e) {
            expect((e as ConfigError).problems).toEqual(['VITE_RPC_URL must use https:']);
        }
    });

    it('refuses credentials in URLs, a bad ALT and the test venue off localnet', () => {
        try {
            parseConfig({
                VITE_CLUSTER: 'devnet',
                VITE_RPC_URL: 'https://user:secret@rpc.example',
                VITE_RPC_WS_URL: 'wss://rpc.example',
                VITE_INDEXER_URL: 'https://api.example',
                VITE_INDEXER_WS_URL: 'wss://api.example/v1/ws',
                VITE_PROTOCOL_ALT: 'not-an-address',
                VITE_ENABLE_TEST_VENUE: 'true',
            });
            expect.unreachable();
        } catch (e) {
            const p = (e as ConfigError).problems;
            expect(p).toContain('VITE_RPC_URL must not carry credentials');
            expect(p).toContain('VITE_PROTOCOL_ALT is not a base58 address');
            expect(p).toContain('VITE_ENABLE_TEST_VENUE is TEST-ONLY and requires VITE_CLUSTER=localnet');
        }
    });

    it('turns the Mobile Wallet Adapter on by default and refuses anything but true or false', () => {
        expect(parseConfig({}).enableMobileWalletAdapter).toBe(true);
        for (const v of ['true', ' true ', '', true] as const) {
            expect(parseConfig({ VITE_ENABLE_MOBILE_WALLET_ADAPTER: v }).enableMobileWalletAdapter, String(v)).toBe(
                true,
            );
        }
        for (const v of ['false', ' false', false] as const) {
            expect(parseConfig({ VITE_ENABLE_MOBILE_WALLET_ADAPTER: v }).enableMobileWalletAdapter, String(v)).toBe(
                false,
            );
        }
        for (const v of ['off', '0', 'yes', 'TRUE']) {
            try {
                parseConfig({ VITE_ENABLE_MOBILE_WALLET_ADAPTER: v });
                expect.unreachable(v);
            } catch (e) {
                expect(e, v).toBeInstanceOf(ConfigError);
                expect((e as ConfigError).problems).toEqual([
                    'VITE_ENABLE_MOBILE_WALLET_ADAPTER must be true or false',
                ]);
            }
        }
        // Every cluster accepts both values, mainnet included.
        const mainnet = {
            VITE_CLUSTER: 'mainnet',
            VITE_RPC_URL: 'https://rpc.example',
            VITE_RPC_WS_URL: 'wss://rpc.example',
            VITE_INDEXER_URL: 'https://api.short.win',
            VITE_INDEXER_WS_URL: 'wss://api.short.win/v1/ws',
        };
        expect(parseConfig(mainnet).enableMobileWalletAdapter).toBe(true);
        expect(parseConfig({ ...mainnet, VITE_ENABLE_MOBILE_WALLET_ADAPTER: 'false' }).enableMobileWalletAdapter).toBe(
            false,
        );
    });

    it('parses the priority-fee ceiling and the test venue on localnet', () => {
        const c = parseConfig({ VITE_PRIORITY_FEE_MAX_MICROLAMPORTS: '250000', VITE_ENABLE_TEST_VENUE: 'true' });
        expect(c.priorityFeeMaxMicroLamports).toBe(250_000n);
        expect(c.enableTestVenue).toBe(true);
    });

    it('defaults transaction explorer links to the selected cluster', () => {
        const urls = {
            VITE_RPC_URL: 'https://rpc.example',
            VITE_RPC_WS_URL: 'wss://rpc.example',
            VITE_INDEXER_URL: 'https://api.example',
            VITE_INDEXER_WS_URL: 'wss://api.example/v1/ws',
        };
        expect(parseConfig({ ...urls, VITE_CLUSTER: 'devnet' }).explorerTxUrl).toBe(
            'https://explorer.solana.com/tx/{sig}?cluster=devnet',
        );
        for (const cluster of ['mainnet', 'localnet']) {
            expect(parseConfig({ ...urls, VITE_CLUSTER: cluster }).explorerTxUrl).toBe(
                'https://explorer.solana.com/tx/{sig}',
            );
        }
    });

    it('preserves an explicitly configured transaction explorer template', () => {
        const explorerTxUrl = 'https://explorer.example/tx/{sig}?network=custom';
        const config = parseConfig({
            VITE_CLUSTER: 'devnet',
            VITE_RPC_URL: 'https://rpc.example',
            VITE_RPC_WS_URL: 'wss://rpc.example',
            VITE_INDEXER_URL: 'https://api.example',
            VITE_INDEXER_WS_URL: 'wss://api.example/v1/ws',
            VITE_EXPLORER_TX_URL: explorerTxUrl,
        });
        expect(config.explorerTxUrl).toBe(explorerTxUrl);
    });
});
