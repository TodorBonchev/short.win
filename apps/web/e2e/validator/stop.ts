/** Stops every process an E2E run left behind (`SHORTWIN_E2E_KEEP=1` or `start.ts`). */
import { stopPreview } from './app.ts';
import { stopIndexer } from './indexer.ts';
import { stopValidator } from './validator.ts';

await stopPreview();
await stopIndexer();
await stopValidator();
console.log('[e2e] stopped');
