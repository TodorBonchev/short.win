/**
 * Deploy check (FRONTEND.md §9.3; M9-SEC-6): fetches a deployed origin and fails unless it sends
 * the anti-framing and other security headers — `dist/_headers` is a Netlify / Cloudflare Pages
 * convention, and a host that ignores it (S3 / CloudFront, GitHub Pages, plain nginx) would serve a
 * frameable app without them. Also checks that no source map is published next to the bundle.
 *
 *   node scripts/check-headers.ts https://short.win
 */
import { missingSecurityHeaders } from '../src/config/csp.ts';

async function main(): Promise<number> {
    const origin = process.argv[2];
    if (!origin) {
        console.error('usage: node scripts/check-headers.ts <https://origin>');
        return 2;
    }
    const problems: string[] = [];
    for (const path of ['/', '/markets']) {
        const res = await fetch(new URL(path, origin), { redirect: 'follow' });
        for (const p of missingSecurityHeaders(res.headers)) problems.push(`${path}: ${p}`);
        if (path === '/') {
            const html = await res.text();
            for (const m of html.matchAll(/src="([^"]+\.js)"/g)) {
                const map = await fetch(new URL(`${m[1]}.map`, origin));
                if (map.ok) problems.push(`${m[1]}.map is published (source maps belong in dist-sourcemaps/)`);
            }
        }
    }
    for (const p of problems) console.error(`✗ ${p}`);
    if (!problems.length) console.log(`✓ ${origin} sends the security headers and no source maps`);
    return problems.length ? 1 : 0;
}

process.exitCode = await main();
