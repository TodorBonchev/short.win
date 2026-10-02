/**
 * Playwright global setup (FRONTEND.md §10.2): the E2E validator and its seed, the optional real
 * indexer (profile `full`), the app built for the seeded protocol ALT and its preview server.
 * `SHORTWIN_E2E_REUSE=1` reuses a healthy validator with its `.e2e/state.json`, the app build when
 * its sources are unchanged (`sourceFingerprint`) and a running preview (for quick local iterations
 * with `SHORTWIN_E2E_KEEP=1`).
 *
 * The validator and the preview are started here rather than by Playwright's `webServer`, because
 * the app's build depends on the seed (the protocol ALT's address) and `webServer` starts first.
 */
import { existsSync, mkdirSync } from 'node:fs';
import { E2E_DIR, ENV, PATHS } from './env.ts';
import { appBuiltFor, appUp, buildApp, startPreview } from './validator/app.ts';
import { startIndexer } from './validator/indexer.ts';
import { seed } from './validator/seed.ts';
import { type E2EState, loadState } from './validator/state.ts';
import { startValidator, validatorHealthy } from './validator/validator.ts';

export default async function globalSetup(): Promise<void> {
    mkdirSync(E2E_DIR, { recursive: true });
    let state: E2EState;
    if (ENV.reuse && existsSync(PATHS.state) && (await validatorHealthy())) {
        state = loadState();
        console.log(`[e2e] reusing the running validator and its seed of ${state.createdAt}`);
    } else {
        await startValidator();
        state = await seed();
    }
    if (ENV.profile === 'full') await startIndexer();
    // A reused build must be current (same ALT, same sources); a fresh build restarts the preview,
    // which serves the file list it found at start.
    const rebuilt = !(ENV.reuse && appBuiltFor(state.protocolAlt));
    if (rebuilt) buildApp(state.protocolAlt);
    if (rebuilt || !(ENV.reuse && (await appUp()))) await startPreview();
}
