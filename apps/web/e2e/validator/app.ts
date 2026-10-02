/**
 * The app under test (FRONTEND.md §10.2): `vite build` with the E2E configuration
 * (`VITE_CLUSTER=localnet`, `VITE_ENABLE_TEST_VENUE=true`, the E2E ports, the seeded protocol ALT)
 * into `.e2e/dist`, served by `vite preview` on :14173. The production build is what is tested:
 * the CSP `<meta>`, hashed assets and the WASM module exactly as shipped.
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { appBuildEnv, PATHS, PORTS, REPO_ROOT, URLS, WEB_DIR } from '../env.ts';
import { isAlive, readRuntime, spawnLogged, stopPid, updateRuntime, waitFor } from './process.ts';

const vite = join(WEB_DIR, 'node_modules', '.bin', 'vite');

/** What a build depends on besides its `VITE_*` values: a reused build is current only if none changed. */
const SOURCE_INPUTS = ['src', 'public', 'index.html', 'vite.config.ts', 'package.json', 'tsconfig.json'];

function walk(path: string, out: string[]): void {
    if (!existsSync(path)) return;
    if (statSync(path).isDirectory()) {
        for (const e of readdirSync(path).sort()) walk(join(path, e), out);
    } else {
        out.push(path);
    }
}

/** SHA-256 over the app's sources, its build configuration, the WASM stamp and the E2E `VITE_*` values. */
export function sourceFingerprint(protocolAlt: string | null): string {
    const h = createHash('sha256');
    const files: string[] = [];
    for (const input of SOURCE_INPUTS) walk(join(WEB_DIR, input), files);
    files.push(join(REPO_ROOT, 'packages', 'math-wasm', 'pkg', '.inputs-sha256'));
    for (const f of files) {
        if (!existsSync(f)) continue;
        h.update(relative(REPO_ROOT, f)).update('\0').update(readFileSync(f)).update('\0');
    }
    h.update(JSON.stringify(appBuildEnv(protocolAlt)));
    return h.digest('hex');
}

export function buildApp(protocolAlt: string | null): void {
    const wasm = join(REPO_ROOT, 'packages', 'math-wasm', 'pkg', 'shortwin_math_wasm_bg.wasm');
    if (!existsSync(wasm)) {
        throw new Error(`missing ${wasm}: run \`pnpm --filter @shortwin/math-wasm build\` first (docs/DEVELOPMENT.md)`);
    }
    console.log(`[e2e] vite build -> .e2e/dist (protocol ALT ${protocolAlt ?? 'none'})`);
    const r = spawnSync(vite, ['build', '--outDir', PATHS.dist, '--emptyOutDir', '--logLevel', 'warn'], {
        cwd: WEB_DIR,
        env: { ...process.env, ...appBuildEnv(protocolAlt) },
        stdio: 'inherit',
    });
    if (r.status !== 0) throw new Error(`vite build failed (exit ${r.status ?? r.signal})`);
    updateRuntime({ appBuiltForAlt: protocolAlt, appBuiltFromSources: sourceFingerprint(protocolAlt) });
}

/** Whether `.e2e/dist` was built for this ALT from the current sources (`SHORTWIN_E2E_REUSE`). */
export function appBuiltFor(protocolAlt: string | null): boolean {
    const rt = readRuntime();
    return (
        existsSync(join(PATHS.dist, 'index.html')) &&
        rt.appBuiltForAlt === protocolAlt &&
        rt.appBuiltFromSources === sourceFingerprint(protocolAlt)
    );
}

export async function appUp(): Promise<boolean> {
    try {
        const r = await fetch(URLS.app, { signal: AbortSignal.timeout(2_000) });
        return r.ok;
    } catch {
        return false;
    }
}

export async function startPreview(): Promise<void> {
    const prev = readRuntime();
    if (isAlive(prev.previewPid)) await stopPid(prev.previewPid);
    const child = spawnLogged(
        vite,
        ['preview', '--outDir', PATHS.dist, '--host', '127.0.0.1', '--port', String(PORTS.preview), '--strictPort'],
        'preview.log',
        { cwd: WEB_DIR },
    );
    if (!child.pid) throw new Error('vite preview did not start');
    updateRuntime({ previewPid: child.pid });
    await waitFor('vite preview (see .e2e/logs/preview.log)', appUp, 30_000);
    console.log(`[e2e] app at ${URLS.app}`);
}

export async function stopPreview(): Promise<void> {
    await stopPid(readRuntime().previewPid);
    updateRuntime({ previewPid: undefined });
}
