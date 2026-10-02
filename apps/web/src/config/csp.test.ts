import { describe, expect, it } from 'vitest';
import { contentSecurityPolicy, securityHeaders } from './csp.ts';
import { parseConfig } from './parseConfig.ts';

const mainnet = parseConfig({
    VITE_CLUSTER: 'mainnet',
    VITE_RPC_URL: 'https://rpc.example/?key=public',
    VITE_RPC_WS_URL: 'wss://rpc.example',
    VITE_INDEXER_URL: 'https://api.short.win',
    VITE_INDEXER_WS_URL: 'wss://api.short.win/v1/ws',
});

describe('content security policy', () => {
    it('allows only self, wasm compilation and the configured RPC / indexer origins', () => {
        const csp = contentSecurityPolicy(mainnet, false);
        expect(csp).toContain("default-src 'none'");
        expect(csp).toContain("script-src 'self' 'wasm-unsafe-eval'");
        expect(csp).toContain(
            "connect-src 'self' https://rpc.example wss://rpc.example https://api.short.win wss://api.short.win",
        );
        expect(csp).not.toMatch(/unsafe-inline|'unsafe-eval'/);
        expect(csp).not.toContain('key=public');
        expect(csp).toContain('upgrade-insecure-requests');
        expect(csp).not.toContain('frame-ancestors');
    });

    it('forbids framing in the header form (clickjacking)', () => {
        const h = securityHeaders(mainnet);
        expect(h['Content-Security-Policy']).toContain("frame-ancestors 'none'");
        expect(h['X-Frame-Options']).toBe('DENY');
        expect(h['Referrer-Policy']).toBe('no-referrer');
    });
});
