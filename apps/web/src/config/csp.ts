/**
 * The Content-Security-Policy and security headers (FRONTEND.md §9.3), computed from the
 * configuration so `connect-src` lists exactly the RPC and indexer origins. `vite.config.ts`
 * injects the policy as a `<meta>` into production builds and writes `dist/_headers` for hosts
 * that read it; the CDN must send the header form (with `frame-ancestors`, which a meta tag cannot
 * carry).
 */
import type { AppConfig } from './parseConfig.ts';
import { SECURITY_TXT_CONTENT_TYPE, SECURITY_TXT_PATH } from './securityTxt.ts';

type CspSources = Pick<AppConfig, 'rpcUrl' | 'rpcWsUrl' | 'indexerUrl' | 'indexerWsUrl' | 'cluster'>;

function origin(u: string): string {
    const x = new URL(u);
    return `${x.protocol}//${x.host}`;
}

/** The policy's directives, in order. `header` adds the header-only directives. */
export function cspDirectives(c: CspSources, header: boolean): [string, string[]][] {
    const connect = Array.from(
        new Set(["'self'", ...[c.rpcUrl, c.rpcWsUrl, c.indexerUrl, c.indexerWsUrl].map(origin)]),
    );
    const d: [string, string[]][] = [
        ['default-src', ["'none'"]],
        // 'wasm-unsafe-eval' compiles @shortwin/math-wasm; nothing else is evaluated.
        ['script-src', ["'self'", "'wasm-unsafe-eval'"]],
        ['style-src', ["'self'"]],
        ['img-src', ["'self'", 'data:']],
        ['font-src', ["'self'"]],
        ['connect-src', connect],
        ['manifest-src', ["'self'"]],
        ['worker-src', ["'self'"]],
        ['base-uri', ["'none'"]],
        ['form-action', ["'none'"]],
        ['object-src', ["'none'"]],
    ];
    if (header) d.push(['frame-ancestors', ["'none'"]]);
    if (c.cluster === 'mainnet') d.push(['upgrade-insecure-requests', []]);
    return d;
}

export function contentSecurityPolicy(c: CspSources, header: boolean): string {
    return cspDirectives(c, header)
        .map(([k, v]) => (v.length ? `${k} ${v.join(' ')}` : k))
        .join('; ');
}

/**
 * The anti-framing headers on their own (`vite preview`, which serves the E2E suite): a CSP header
 * with only `frame-ancestors` adds nothing else to the `<meta>` policy (policies intersect).
 */
export const ANTI_FRAMING_HEADERS: Readonly<Record<string, string>> = {
    'X-Frame-Options': 'DENY',
    'Content-Security-Policy': "frame-ancestors 'none'",
};

/**
 * What a deployed origin's response lacks of §9.3's headers (the deploy check,
 * `scripts/check-headers.ts`): an empty list when it sends them.
 */
export function missingSecurityHeaders(headers: { get(name: string): string | null }): string[] {
    const problems: string[] = [];
    const xfo = headers.get('x-frame-options');
    if (xfo?.trim().toUpperCase() !== 'DENY') problems.push(`X-Frame-Options must be DENY (got ${xfo ?? 'none'})`);
    const csp = headers.get('content-security-policy');
    if (!csp || !/(^|;)\s*frame-ancestors\s+'none'\s*(;|$)/.test(csp)) {
        problems.push(`Content-Security-Policy must carry frame-ancestors 'none' (got ${csp ?? 'none'})`);
    }
    if (headers.get('x-content-type-options')?.trim().toLowerCase() !== 'nosniff') {
        problems.push('X-Content-Type-Options must be nosniff');
    }
    if (!headers.get('strict-transport-security')) problems.push('Strict-Transport-Security is missing');
    return problems;
}

/** Response headers for the static host (`dist/_headers`). */
export function securityHeaders(c: CspSources): Record<string, string> {
    return {
        'Content-Security-Policy': contentSecurityPolicy(c, true),
        'X-Frame-Options': 'DENY',
        'X-Content-Type-Options': 'nosniff',
        'Referrer-Policy': 'no-referrer',
        'Permissions-Policy': 'camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=(), hid=()',
        'Cross-Origin-Opener-Policy': 'same-origin',
        'Strict-Transport-Security': 'max-age=63072000; includeSubDomains',
    };
}

/**
 * The `dist/_headers` file (Netlify / Cloudflare Pages syntax): the security headers on every path,
 * and `/.well-known/security.txt` as `text/plain; charset=utf-8` (RFC 9116 §3; M10 SECTXT-4).
 */
export function headersFile(c: CspSources): string {
    return [
        '/*',
        ...Object.entries(securityHeaders(c)).map(([k, v]) => `  ${k}: ${v}`),
        '',
        SECURITY_TXT_PATH,
        `  Content-Type: ${SECURITY_TXT_CONTENT_TYPE}`,
        '',
    ].join('\n');
}
