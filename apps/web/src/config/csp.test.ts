import { describe, expect, it } from 'vitest';
import {
    contentSecurityPolicy,
    headersFile,
    MWA_CONNECT_SOURCES,
    MWA_SPINNER_ATTR_HASH,
    MWA_SPINNER_STYLE_HASH,
    securityHeaders,
} from './csp.ts';
import { parseConfig } from './parseConfig.ts';
import { SECURITY_TXT_CONTENT_TYPE, SECURITY_TXT_PATH } from './securityTxt.ts';

const MAINNET_ENV = {
    VITE_CLUSTER: 'mainnet',
    VITE_RPC_URL: 'https://rpc.example/?key=public',
    VITE_RPC_WS_URL: 'wss://rpc.example',
    VITE_INDEXER_URL: 'https://api.short.win',
    VITE_INDEXER_WS_URL: 'wss://api.short.win/v1/ws',
};
/** The default build: the Mobile Wallet Adapter on (FRONTEND.md §5.4 (6), §9.3). */
const mainnet = parseConfig(MAINNET_ENV);
const mainnetOff = parseConfig({ ...MAINNET_ENV, VITE_ENABLE_MOBILE_WALLET_ADAPTER: 'false' });
const localnet = parseConfig({});
const localnetOff = parseConfig({ VITE_ENABLE_MOBILE_WALLET_ADAPTER: 'false' });

/** The policies before the Mobile Wallet Adapter existed, byte for byte (the flag-off builds keep them). */
const MAINNET_OFF =
    "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:; font-src 'self'; " +
    "connect-src 'self' https://rpc.example wss://rpc.example https://api.short.win wss://api.short.win; " +
    "manifest-src 'self'; worker-src 'self'; base-uri 'none'; form-action 'none'; object-src 'none'; upgrade-insecure-requests";
const LOCALNET_OFF =
    "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:; font-src 'self'; " +
    "connect-src 'self' http://127.0.0.1:8899 ws://127.0.0.1:8900 http://127.0.0.1:8080 ws://127.0.0.1:8080; " +
    "manifest-src 'self'; worker-src 'self'; base-uri 'none'; form-action 'none'; object-src 'none'";
/** mobile.md §2.1: the flag-on policy for the mainnet fixture, exactly and in this order. */
const MAINNET_ON =
    "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; " +
    "style-src-elem 'self' 'sha256-9y+0omBcAXmVSpYuadfUp/k8GxCIDiw2wrEyqM1cQnc='; " +
    "style-src-attr 'unsafe-hashes' 'sha256-fpg83YZxyBh7zNCICwdKi5D7eUkxQ9wk3mpUiNPhP3g='; " +
    "img-src 'self' data:; font-src 'self'; " +
    "connect-src 'self' https://rpc.example wss://rpc.example https://api.short.win wss://api.short.win ws://localhost:* http://localhost; " +
    "manifest-src 'self'; worker-src 'self'; base-uri 'none'; form-action 'none'; object-src 'none'; " +
    'upgrade-insecure-requests';

describe('content security policy', () => {
    it('allows only self, wasm compilation and the configured RPC / indexer origins', () => {
        for (const cfg of [mainnet, mainnetOff]) {
            const csp = contentSecurityPolicy(cfg, false);
            expect(csp).toContain("default-src 'none'");
            expect(csp).toContain("script-src 'self' 'wasm-unsafe-eval'");
            expect(csp).toContain(
                "connect-src 'self' https://rpc.example wss://rpc.example https://api.short.win wss://api.short.win",
            );
            expect(csp).not.toMatch(/unsafe-inline|'unsafe-eval'/);
            expect(csp).not.toContain('key=public');
            expect(csp).toContain('upgrade-insecure-requests');
            expect(csp).not.toContain('frame-ancestors');
        }
    });

    it('with the Mobile Wallet Adapter off: exactly the policy before it, no localhost, no -elem / -attr', () => {
        expect(mainnetOff.enableMobileWalletAdapter).toBe(false);
        expect(contentSecurityPolicy(mainnetOff, false)).toBe(MAINNET_OFF);
        expect(contentSecurityPolicy(localnetOff, false)).toBe(LOCALNET_OFF);
        for (const cfg of [mainnetOff, localnetOff]) {
            const csp = contentSecurityPolicy(cfg, true);
            expect(csp).not.toContain('localhost');
            expect(csp).not.toMatch(/style-src-elem|style-src-attr|sha256-/);
        }
    });

    it('with the Mobile Wallet Adapter on (the default): the loopback sources and the two spinner hashes only', () => {
        expect(mainnet.enableMobileWalletAdapter).toBe(true);
        const csp = contentSecurityPolicy(mainnet, false);
        expect(csp).toBe(MAINNET_ON);
        const connect = csp.split('; ').find((d) => d.startsWith('connect-src ')) ?? '';
        expect(connect.endsWith(` ${MWA_CONNECT_SOURCES.join(' ')}`)).toBe(true);
        expect(connect.endsWith('ws://localhost:* http://localhost')).toBe(true);
        expect(csp).toContain(`style-src-elem 'self' ${MWA_SPINNER_STYLE_HASH};`);
        expect(csp).toContain(`style-src-attr 'unsafe-hashes' ${MWA_SPINNER_ATTR_HASH};`);
        expect(csp.match(/sha256-/g)).toHaveLength(2);
        expect(csp).not.toMatch(/unsafe-inline|'unsafe-eval'|frame-src|child-src|fonts\.g/);
        // Chromium does not upgrade loopback requests: UIR stays (§9.3; a device check, FE-32).
        expect(csp).toContain('upgrade-insecure-requests');
        // Localnet: the same additions, its own origins, no UIR.
        const local = contentSecurityPolicy(localnet, false);
        expect(local).toContain(
            "connect-src 'self' http://127.0.0.1:8899 ws://127.0.0.1:8900 http://127.0.0.1:8080 ws://127.0.0.1:8080 ws://localhost:* http://localhost;",
        );
        expect(local).not.toContain('upgrade-insecure-requests');
    });

    it('forbids framing in the header form (clickjacking)', () => {
        for (const cfg of [mainnet, mainnetOff]) {
            const h = securityHeaders(cfg);
            expect(h['Content-Security-Policy']).toContain("frame-ancestors 'none'");
            expect(h['X-Frame-Options']).toBe('DENY');
            expect(h['Referrer-Policy']).toBe('no-referrer');
        }
    });

    it('keeps Permissions-Policy and COOP identical in both flag states, never listing local-network', () => {
        const on = securityHeaders(mainnet);
        const off = securityHeaders(mainnetOff);
        expect(on['Permissions-Policy']).toBe(off['Permissions-Policy']);
        expect(on['Cross-Origin-Opener-Policy']).toBe(off['Cross-Origin-Opener-Policy']);
        expect(on['Cross-Origin-Opener-Policy']).toBe('same-origin');
        for (const h of [on, off]) {
            // Their default allowlist is 'self', which the Mobile Wallet Adapter needs.
            expect(h['Permissions-Policy']).not.toMatch(/local-network|loopback-network|local-network-access/);
        }
        // Only the CSP differs.
        const { 'Content-Security-Policy': cspOn, ...restOn } = on;
        const { 'Content-Security-Policy': cspOff, ...restOff } = off;
        expect(restOn).toEqual(restOff);
        // The header form adds frame-ancestors (before UIR), in both states.
        expect(cspOn).toBe(
            MAINNET_ON.replace('; upgrade-insecure-requests', "; frame-ancestors 'none'; upgrade-insecure-requests"),
        );
        expect(cspOff).toBe(
            MAINNET_OFF.replace('; upgrade-insecure-requests', "; frame-ancestors 'none'; upgrade-insecure-requests"),
        );
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
        // The default (flag-on) build's file carries the flag-on policy.
        expect(blocks.get('/*')?.['Content-Security-Policy']).toContain('ws://localhost:* http://localhost');
        expect(blocks.get('/*')?.['Content-Security-Policy']).toContain(MWA_SPINNER_STYLE_HASH);
        expect(blocks.get(SECURITY_TXT_PATH)).toEqual({ 'Content-Type': SECURITY_TXT_CONTENT_TYPE });
        expect(SECURITY_TXT_CONTENT_TYPE).toBe('text/plain; charset=utf-8');
    });
});
