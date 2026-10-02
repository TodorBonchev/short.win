/**
 * frame — clickjacking (FRONTEND.md §9.3; M9-SEC-6). The production build served by `vite preview`
 * (this suite's server) must not run inside another page: the server sends `X-Frame-Options: DENY`
 * and `frame-ancestors 'none'`, and — for a host that ignores `dist/_headers` — the app itself
 * refuses to mount when framed and shows `frame-guard` instead (no wallet button, no Sign button).
 *
 * Uses Playwright's own `test` (not the app fixtures): the browser logs the refused frame as an
 * error, which the fixtures would count as a CSP violation of the app.
 */
import { expect, test } from '@playwright/test';
import { T } from '../../src/config/testids.ts';
import { URLS } from '../env.ts';

const victim = (path: string) =>
    `<!doctype html><title>attacker</title><iframe id="victim" src="${URLS.app}${path}" width="1200" height="800" style="opacity:0.05"></iframe>`;

test.describe('framing', { tag: '@scaffold' }, () => {
    test('the server sends X-Frame-Options: DENY and frame-ancestors none', async ({ request }) => {
        for (const path of ['/', '/markets']) {
            const res = await request.get(`${URLS.app}${path}`);
            expect(res.ok()).toBe(true);
            expect(res.headers()['x-frame-options']).toBe('DENY');
            expect(res.headers()['content-security-policy']).toContain("frame-ancestors 'none'");
        }
    });

    test('a framing page gets no app — the browser refuses, and so does the app on a header-less host', async ({
        page,
    }) => {
        // 1. The headers are honoured: the frame never renders the app.
        await page.setContent(victim('/markets'));
        const frame = page.frameLocator('#victim');
        await page.waitForTimeout(3_000);
        await expect(frame.locator(`[data-testid="${T.statusBar}"]`)).toHaveCount(0);
        await expect(frame.locator(`[data-testid="${T.walletButton}"]`)).toHaveCount(0);

        // 2. A host that ignores `_headers` (S3, GitHub Pages, plain nginx): strip them; the app's
        //    own guard renders a notice instead of the app.
        await page.route(`${URLS.app}/markets`, async (route) => {
            const res = await route.fetch();
            const headers = { ...res.headers() };
            delete headers['x-frame-options'];
            delete headers['content-security-policy'];
            await route.fulfill({ response: res, headers });
        });
        await page.setContent(victim('/markets'));
        await expect(frame.locator(`[data-testid="${T.frameGuard}"]`)).toBeVisible({ timeout: 30_000 });
        await expect(frame.locator(`[data-testid="${T.walletButton}"]`)).toHaveCount(0);
        await expect(frame.locator(`[data-testid="${T.statusBar}"]`)).toHaveCount(0);

        // Opened directly, the same build runs.
        await page.unrouteAll();
        await page.goto(`${URLS.app}/markets`);
        await expect(page.locator(`[data-testid="${T.statusBar}"]`)).toBeVisible({ timeout: 30_000 });
        await expect(page.locator(`[data-testid="${T.frameGuard}"]`)).toHaveCount(0);
    });
});
