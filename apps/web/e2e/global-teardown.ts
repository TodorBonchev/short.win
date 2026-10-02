/**
 * Playwright global teardown: stops the preview server, the indexer and its Postgres (profile
 * `full`) and the validator — unless `SHORTWIN_E2E_KEEP=1` (then `node e2e/validator/stop.ts`
 * stops them later). The ledger and logs stay in `.e2e/` for inspection.
 */
import { ENV } from './env.ts';
import { stopPreview } from './validator/app.ts';
import { stopIndexer } from './validator/indexer.ts';
import { stopValidator } from './validator/validator.ts';

export default async function globalTeardown(): Promise<void> {
    if (ENV.keep) {
        console.log('[e2e] SHORTWIN_E2E_KEEP=1: leaving the validator and the preview running');
        return;
    }
    await stopPreview();
    await stopIndexer();
    await stopValidator();
}
