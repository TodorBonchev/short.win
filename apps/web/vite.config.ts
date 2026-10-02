/**
 * Vite 8 config (FRONTEND.md §2.2, §9.3). Production builds: the configuration is validated (a bad
 * VITE_* value fails the build), the Content-Security-Policy is injected as a <meta> and written with
 * the other security headers to dist/_headers for the static host, and no source is inlined. Source
 * maps are generated `hidden` (no `sourceMappingURL` in the bundle) and moved out of `dist/` into
 * `dist-sourcemaps/`, so a deploy of `dist/` does not publish them (M9-SEC-6). `vite preview` (the
 * E2E server) sends the anti-framing headers too.
 */
import { mkdirSync, renameSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv, type Plugin } from 'vite';
import { ANTI_FRAMING_HEADERS, contentSecurityPolicy, securityHeaders } from './src/config/csp.ts';
import { parseConfig } from './src/config/parseConfig.ts';

/** Where the production source maps go (outside the deployed `dist/`). */
const SOURCEMAPS_DIR = 'dist-sourcemaps';

function securityPlugin(env: Record<string, string>): Plugin {
    const cfg = parseConfig(env);
    return {
        name: 'shortwin-security',
        apply: 'build',
        transformIndexHtml: () => [
            {
                tag: 'meta',
                attrs: { 'http-equiv': 'Content-Security-Policy', content: contentSecurityPolicy(cfg, false) },
                injectTo: 'head-prepend',
            },
        ],
        generateBundle() {
            const lines = ['/*', ...Object.entries(securityHeaders(cfg)).map(([k, v]) => `  ${k}: ${v}`), ''];
            this.emitFile({ type: 'asset', fileName: '_headers', source: lines.join('\n') });
        },
        writeBundle(options, bundle) {
            const out = options.dir ?? 'dist';
            const maps = resolve(out, '..', SOURCEMAPS_DIR);
            for (const file of Object.keys(bundle)) {
                if (!file.endsWith('.map')) continue;
                const to = join(maps, file);
                mkdirSync(dirname(to), { recursive: true });
                renameSync(join(out, file), to);
            }
        },
    };
}

export default defineConfig(({ mode }) => {
    const env = loadEnv(mode, process.cwd(), 'VITE_');
    return {
        plugins: [react(), securityPlugin(env)],
        build: {
            target: 'es2023',
            sourcemap: 'hidden',
            assetsInlineLimit: 0,
        },
        server: { port: 5173, strictPort: true },
        preview: { port: 4173, strictPort: true, headers: { ...ANTI_FRAMING_HEADERS } },
    };
});
