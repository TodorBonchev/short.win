/**
 * Vite 8 config (FRONTEND.md §2.2, §9.3). Production builds: the configuration is validated (a bad
 * VITE_* value fails the build), the Content-Security-Policy is injected as a <meta> and written with
 * the other security headers to dist/_headers for the static host, and no source is inlined. Source
 * maps are generated `hidden` (no `sourceMappingURL` in the bundle) and moved out of `dist/` into
 * `dist-sourcemaps/`, so a deploy of `dist/` does not publish them (M9-SEC-6). `vite preview` (the
 * E2E server) sends the anti-framing headers too.
 *
 * `public/.well-known/security.txt` (RFC 9116; M10 SECTXT-4) is checked by every build (fields and
 * formats), and only a `VITE_CLUSTER=mainnet` build also refuses its `OWNER-FILL` placeholders and
 * an `Expires` 30 days or less (or more than a year) ahead; `dist/_headers` (`headersFile`) serves
 * it as `text/plain; charset=utf-8`.
 */
import { mkdirSync, readFileSync, renameSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import react from '@vitejs/plugin-react';
import { defaultClientConditions, defineConfig, loadEnv, type Plugin } from 'vite';
import { ANTI_FRAMING_HEADERS, contentSecurityPolicy, headersFile } from './src/config/csp.ts';
import { parseConfig } from './src/config/parseConfig.ts';
import { SECURITY_TXT_PATH, securityTxtProblems } from './src/config/securityTxt.ts';

/** Where the production source maps go (outside the deployed `dist/`). */
const SOURCEMAPS_DIR = 'dist-sourcemaps';

function securityPlugin(env: Record<string, string>): Plugin {
    const cfg = parseConfig(env);
    let publicDir = 'public';
    return {
        name: 'shortwin-security',
        apply: 'build',
        configResolved(resolved) {
            publicDir = resolved.publicDir;
        },
        buildStart() {
            const file = join(publicDir, SECURITY_TXT_PATH);
            const production = cfg.cluster === 'mainnet';
            const problems = securityTxtProblems(readFileSync(file, 'utf8'), { now: new Date(), production });
            if (problems.length > 0) {
                const why = production ? ' (a mainnet build needs the owner values)' : '';
                this.error(`${file}${why}:\n- ${problems.join('\n- ')}`);
            }
        },
        transformIndexHtml: () => [
            {
                tag: 'meta',
                attrs: { 'http-equiv': 'Content-Security-Policy', content: contentSecurityPolicy(cfg, false) },
                injectTo: 'head-prepend',
            },
        ],
        generateBundle() {
            this.emitFile({ type: 'asset', fileName: '_headers', source: headersFile(cfg) });
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
        // The workspace's @shortwin/client resolves to its TypeScript sources, not its dist/.
        resolve: { conditions: ['@shortwin/source', ...defaultClientConditions] },
        build: {
            target: 'es2023',
            sourcemap: 'hidden',
            assetsInlineLimit: 0,
        },
        server: { port: 5173, strictPort: true },
        preview: { port: 4173, strictPort: true, headers: { ...ANTI_FRAMING_HEADERS } },
    };
});
