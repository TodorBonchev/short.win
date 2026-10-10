/**
 * Playwright 1.63 (FRONTEND.md §10.2–§10.3): the specs in `e2e/specs` against the production build
 * served from `.e2e/dist`, a local `solana-test-validator` seeded by `e2e/validator/seed.ts`, the
 * injected Wallet Standard test wallet and (profile `chain`) the routed indexer mock.
 *
 * Browser: `$SHORTWIN_CHROMIUM`, else the system `/usr/bin/chromium`, else Playwright's Chromium in
 * the user cache (`npx playwright install chromium`; never a root install).
 *
 * The specs share one validator and run serially (one worker): they are independent of their
 * order (each spec group signs with its own seeded wallet), not of each other's chain writes to
 * the shared markets. Tags: `@scaffold` runs against the scaffold alone; `@markets`, `@lend`,
 * `@short` need that track's pages (FRONTEND.md §12.3) — `--grep @scaffold` before they land.
 * `@responsive` (the six-viewport capture, FRONTEND.md §10.5) is left out of the default run; it runs
 * with `SHORTWIN_E2E_RESPONSIVE=1` or a `SHORTWIN_SHOTS_DIR` (`--grep @responsive` for it alone).
 *
 * Projects (M10-RESP, FRONTEND.md §10.2): `<profile>` — the desktop (1440 × 900), every spec but
 * `mobile-*.spec.ts`; `<profile>-phone` — a 390 × 844 phone (mobile viewport, touch, DPR 3, an iPhone
 * user agent; Chromium's emulation), the `mobile-*.spec.ts` specs only (real taps, axe at 0). Both
 * are in the default run; `--project <profile>` runs one.
 */
import { defineConfig, devices } from '@playwright/test';
import { chromiumPath, ENV, PATHS, URLS } from './e2e/env.ts';

const executablePath = chromiumPath();

export default defineConfig({
    testDir: './e2e/specs',
    outputDir: PATHS.results,
    globalSetup: './e2e/global-setup.ts',
    globalTeardown: './e2e/global-teardown.ts',
    fullyParallel: false,
    workers: 1,
    grepInvert: ENV.responsive ? undefined : /@responsive/,
    // Validator round trips, a 20 s stale-review wait and real confirmations: generous budgets.
    timeout: 180_000,
    expect: { timeout: 30_000 },
    retries: ENV.ci ? 1 : 0,
    forbidOnly: ENV.ci,
    reporter: [['list'], ['html', { outputFolder: PATHS.report, open: 'never' }]],
    use: {
        baseURL: URLS.app,
        viewport: { width: 1440, height: 900 },
        colorScheme: 'dark',
        locale: 'en-US',
        timezoneId: 'UTC',
        trace: 'retain-on-failure',
        screenshot: 'only-on-failure',
        video: 'off',
        actionTimeout: 30_000,
        navigationTimeout: 60_000,
    },
    projects: [
        {
            name: ENV.profile,
            testIgnore: /mobile-.*\.spec\.ts/,
            use: {
                ...devices['Desktop Chrome'],
                viewport: { width: 1440, height: 900 },
                launchOptions: executablePath ? { executablePath } : {},
            },
        },
        {
            name: `${ENV.profile}-phone`,
            testMatch: /mobile-.*\.spec\.ts/,
            use: {
                browserName: 'chromium',
                viewport: { width: 390, height: 844 },
                isMobile: true,
                hasTouch: true,
                deviceScaleFactor: 3,
                userAgent: devices['iPhone 13'].userAgent,
                launchOptions: executablePath ? { executablePath } : {},
            },
        },
    ],
});
