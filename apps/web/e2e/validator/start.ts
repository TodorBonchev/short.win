/**
 * Standalone bootstrap for manual work against the E2E environment:
 *
 *   node e2e/validator/start.ts          # validator + seed (+ the app with --app), left running
 *   node e2e/validator/start.ts --app    # also build the app and serve it on :14173
 *   node e2e/validator/start.ts --env    # also write .env.e2e.local for `pnpm --filter web dev:e2e`
 *   node e2e/validator/fund.ts <address> # SOL and every seeded mint for a browser wallet
 *   node e2e/validator/stop.ts           # stop everything
 *
 * Then `SHORTWIN_E2E_REUSE=1 pnpm --filter web e2e` runs the specs against it without reseeding.
 * The seeded addresses are in `.e2e/state.json`; the generated keys in `.e2e/keys/`.
 */
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { appBuildEnv, ENV, URLS, WEB_DIR } from '../env.ts';
import { appBuiltFor, buildApp, startPreview } from './app.ts';
import { startIndexer } from './indexer.ts';
import { seed } from './seed.ts';
import { startValidator } from './validator.ts';

const withApp = process.argv.includes('--app');
const withEnv = process.argv.includes('--env');

/** `.env.e2e.local` (gitignored): the E2E configuration for `vite --mode e2e` (the dev server). */
function writeDevEnv(protocolAlt: string | null): string {
    const path = join(WEB_DIR, '.env.e2e.local');
    const lines = Object.entries(appBuildEnv(protocolAlt)).map(([k, v]) => `${k}=${v}`);
    writeFileSync(
        path,
        `# Written by e2e/validator/start.ts --env (the E2E validator's endpoints).\n${lines.join('\n')}\n`,
    );
    return path;
}

try {
    await startValidator();
    const state = await seed();
    if (ENV.profile === 'full') await startIndexer();
    if (withEnv) console.log(`[e2e] wrote ${writeDevEnv(state.protocolAlt)}`);
    if (withApp) {
        if (!appBuiltFor(state.protocolAlt)) buildApp(state.protocolAlt);
        await startPreview();
    }
    console.log(
        `[e2e] ready: RPC ${URLS.rpc}${withApp ? `, app ${URLS.app}` : ''}; stop with node e2e/validator/stop.ts`,
    );
    process.exit(0);
} catch (e) {
    console.error(e);
    process.exit(1);
}
