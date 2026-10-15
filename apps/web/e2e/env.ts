/**
 * The E2E environment (FRONTEND.md §10.2): ports, paths and the profile, shared by the
 * validator bootstrap, the seed, global setup / teardown, the Playwright config and the fixtures.
 * Everything lives under `apps/web/.e2e/` (gitignored; on disk, never /tmp).
 *
 * Variables (all optional):
 * - `SHORTWIN_E2E_PROFILE`   `chain` (default: the indexer is a Playwright-routed mock built from the
 *                            chain) | `full` (a real `shortwin-indexer` against a user-space Postgres)
 * - `SHORTWIN_E2E_PROGRAM`   `test-venue` (default; the mock venue drives trading) | `release`
 * - `SHORTWIN_E2E_VENUES`    `1` (default): load the dumped venue programs and accounts of
 *                            tests/fixtures; `0` skips them (faster start, no real-venue specs)
 * - `SHORTWIN_BUNDLED_SPL`    `1`: keep the validator's built-in Token-2022 instead of mainnet's build
 *                            from tests/fixtures (group `spl_programs`; as the LiteSVM harness's opt-out)
 *                            and skip the SPL pin check (the built-in spl-memo v3 is mainnet's either way)
 * - `SHORTWIN_E2E_REUSE`     `1`: reuse a running E2E validator, its seed and the app build
 * - `SHORTWIN_E2E_KEEP`      `1`: leave the validator and the preview server running after the run
 * - `SHORTWIN_E2E_RESPONSIVE` `1`: include the `@responsive` six-viewport capture (also on when
 *                            `SHORTWIN_SHOTS_DIR` is set: its output directory; FRONTEND.md §10.5)
 * - `SHORTWIN_E2E_MWA`       `1` (default): build the app with the Mobile Wallet Adapter on
 *                            (`VITE_ENABLE_MOBILE_WALLET_ADAPTER`, FRONTEND.md §5.4 (6)); `0` builds it
 *                            off (the app is rebuilt: `sourceFingerprint` hashes the build's `VITE_*`)
 * - `SHORTWIN_CHROMIUM`      browser executable (default `/usr/bin/chromium` when it exists, else
 *                            Playwright's own Chromium from the user cache); `playwright` forces the latter
 * - `SHORTWIN_SO`, `MOCK_CPAMM_SO`  program ELFs (defaults under target/)
 * - `SOLANA_BIN`             directory of `solana-test-validator` (default: the Agave install)
 * - `SHORTWIN_INDEXER_BIN`, `SHORTWIN_PG_INSTALL_DIR`  the `full` profile's binaries
 */
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));

/** `apps/web`. */
export const WEB_DIR = resolve(here, '..');
/** The repository root. */
export const REPO_ROOT = resolve(WEB_DIR, '..', '..');
/** Everything the E2E writes (gitignored). */
export const E2E_DIR = join(WEB_DIR, '.e2e');

export const PATHS = {
    ledger: join(E2E_DIR, 'ledger'),
    keys: join(E2E_DIR, 'keys'),
    accounts: join(E2E_DIR, 'accounts'),
    logs: join(E2E_DIR, 'logs'),
    dist: join(E2E_DIR, 'dist'),
    pg: join(E2E_DIR, 'pg'),
    state: join(E2E_DIR, 'state.json'),
    runtime: join(E2E_DIR, 'runtime.json'),
    report: join(E2E_DIR, 'report'),
    results: join(E2E_DIR, 'test-results'),
    fixtures: join(REPO_ROOT, 'tests', 'fixtures'),
} as const;

/** Dedicated ports, so a developer validator (8899) and `vite preview` (4173) can run alongside. */
export const PORTS = {
    rpc: 18899,
    ws: 18900,
    faucet: 19900,
    gossip: 18901,
    dynamicRange: '18910-18960',
    preview: 14173,
    indexer: 18080,
    postgres: 15432,
} as const;

export const URLS = {
    rpc: `http://127.0.0.1:${PORTS.rpc}`,
    ws: `ws://127.0.0.1:${PORTS.ws}`,
    app: `http://127.0.0.1:${PORTS.preview}`,
    indexer: `http://127.0.0.1:${PORTS.indexer}`,
    indexerWs: `ws://127.0.0.1:${PORTS.indexer}/v1/ws`,
} as const;

export type Profile = 'chain' | 'full';

function flag(name: string, dflt: boolean): boolean {
    const v = process.env[name];
    if (v === undefined || v === '') return dflt;
    return v === '1' || v.toLowerCase() === 'true';
}

export const ENV = {
    profile: (process.env.SHORTWIN_E2E_PROFILE === 'full' ? 'full' : 'chain') as Profile,
    program: process.env.SHORTWIN_E2E_PROGRAM === 'release' ? ('release' as const) : ('test-venue' as const),
    venues: flag('SHORTWIN_E2E_VENUES', true),
    bundledSpl: flag('SHORTWIN_BUNDLED_SPL', false),
    reuse: flag('SHORTWIN_E2E_REUSE', false),
    keep: flag('SHORTWIN_E2E_KEEP', false),
    responsive: flag('SHORTWIN_E2E_RESPONSIVE', false) || !!process.env.SHORTWIN_SHOTS_DIR,
    /** O-6: the Mobile Wallet Adapter in the app build (`SHORTWIN_E2E_MWA=0`: the flag-off build). */
    mwa: flag('SHORTWIN_E2E_MWA', true),
    ci: flag('CI', false),
} as const;

/**
 * The program ELFs the validator loads (FRONTEND.md §10.2). The `complete_curve` helper is retired
 * with pump's v3 exits (design pump-final §8, D2), so the validator no longer deploys it.
 */
export function programPaths(): { shortwin: string; mock: string } {
    const shortwin =
        process.env.SHORTWIN_SO ??
        (ENV.program === 'release'
            ? join(REPO_ROOT, 'target', 'deploy', 'shortwin.so')
            : join(REPO_ROOT, 'target', 'test-venue', 'deploy', 'shortwin.so'));
    return {
        shortwin,
        mock: process.env.MOCK_CPAMM_SO ?? join(REPO_ROOT, 'target', 'deploy', 'mock_cpamm.so'),
    };
}

/** `solana-test-validator` (Agave 4.1.2; never a root install). */
export function validatorBinary(): string {
    const dir =
        process.env.SOLANA_BIN ?? join(homedir(), '.local', 'share', 'solana', 'install', 'active_release', 'bin');
    const bin = join(dir, 'solana-test-validator');
    return existsSync(bin) ? bin : 'solana-test-validator';
}

/**
 * The browser: `$SHORTWIN_CHROMIUM`, else the system Chromium when present, else `undefined`
 * (Playwright's own Chromium, installed into the user cache with `npx playwright install chromium`).
 */
export function chromiumPath(): string | undefined {
    const env = process.env.SHORTWIN_CHROMIUM;
    if (env === 'playwright') return undefined;
    if (env) return env;
    return existsSync('/usr/bin/chromium') ? '/usr/bin/chromium' : undefined;
}

/** The `VITE_*` configuration of the E2E app build (FRONTEND.md §2.1, §10.2). */
export function appBuildEnv(protocolAlt: string | null): Record<string, string> {
    return {
        VITE_CLUSTER: 'localnet',
        VITE_RPC_URL: URLS.rpc,
        VITE_RPC_WS_URL: URLS.ws,
        VITE_INDEXER_URL: URLS.indexer,
        VITE_INDEXER_WS_URL: URLS.indexerWs,
        VITE_PROTOCOL_ALT: protocolAlt ?? '',
        VITE_PRIORITY_FEE_MAX_MICROLAMPORTS: '1000000',
        VITE_ENABLE_TEST_VENUE: ENV.program === 'test-venue' ? 'true' : 'false',
        VITE_ENABLE_MOBILE_WALLET_ADAPTER: ENV.mwa ? 'true' : 'false',
        VITE_EXPLORER_TX_URL: `https://explorer.solana.com/tx/{sig}?cluster=custom&customUrl=${encodeURIComponent(URLS.rpc)}`,
    };
}
