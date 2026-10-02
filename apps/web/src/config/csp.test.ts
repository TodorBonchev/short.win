import { describe, expect, it } from 'vitest';
import { contentSecurityPolicy, headersFile, securityHeaders } from './csp.ts';
import { parseConfig } from './parseConfig.ts';
import { SECURITY_TXT_CONTENT_TYPE, SECURITY_TXT_PATH } from './securityTxt.ts';

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

    it('dist/_headers sends the security headers everywhere and security.txt as UTF-8 plain text', () => {
        // `vite.config.ts` emits exactly this; `vite preview` (the E2E server) ignores the file.
        const blocks = new Map<string, Record<string, string>>();
        let path = '';
        for (const line of headersFile(mainnet).split('\n')) {
            if (line === '') continue;
            if (!line.startsWith('  ')) {
                path = line;
                expect(blocks.has(path)).toBe(false);
                blocks.set(path, {});
                continue;
            }
            const m = /^ {2}([A-Za-z-]+): (.+)$/.exec(line);
            expect(m, line).not.toBeNull();
            const block = blocks.get(path);
            if (m?.[1] && m[2] && block) block[m[1]] = m[2];
        }
        expect([...blocks.keys()]).toEqual(['/*', SECURITY_TXT_PATH]);
        expect(blocks.get('/*')).toEqual(securityHeaders(mainnet));
        expect(blocks.get(SECURITY_TXT_PATH)).toEqual({ 'Content-Type': SECURITY_TXT_CONTENT_TYPE });
        expect(SECURITY_TXT_CONTENT_TYPE).toBe('text/plain; charset=utf-8');
    });
});
