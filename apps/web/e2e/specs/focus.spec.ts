/**
 * focus — Focus Not Obscured (Minimum), WCAG 2.2 2.4.11, on the desktop (FRONTEND.md §8, §10.2):
 * a Tab walk over the markets list (the intro strip shown), a market's Overview tab, the Lending
 * page and How it works; no focused control may sit entirely under the sticky header or the
 * status bar (e2e/focus-obscured.ts; partial covers are annotations). The phone's walk, with the
 * tab bar, the status strip and the market's action bar, is mobile-focus.spec.ts. Signs nothing.
 */

import { expect, T, test } from '../fixtures.ts';
import { expectFocusNeverObscured } from '../focus-obscured.ts';

test.describe('focus not obscured: markets', { tag: '@markets' }, () => {
    test.use({ wallet: 'trader' });

    test('the markets list with the intro strip', async ({ app, page, state }) => {
        await app.goto('/markets', 'markets');
        await expect(app.id(T.marketsIntro)).toBeVisible();
        await expect(app.id(T.marketsRow(state.markets.main.mint))).toBeVisible();
        await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
        await expectFocusNeverObscured(page, 'desktop /markets');
    });

    test("a market's Overview tab", async ({ app, page, state }) => {
        await app.goto(`/m/${state.markets.main.mint}`, 'market');
        await app.connectWallet();
        await expect(app.id(T.marketHeader)).toBeVisible();
        await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
        await expectFocusNeverObscured(page, 'desktop market overview');
    });
});

test.describe('focus not obscured: lending', { tag: '@lend' }, () => {
    test.use({ wallet: 'claimer' });

    test('the Lending page', async ({ app, page }) => {
        await app.goto('/lend', 'lend');
        await app.connectWallet();
        await expect(app.id(T.lenderDashboard)).toBeVisible();
        await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
        await expectFocusNeverObscured(page, 'desktop /lend');
    });
});

test.describe('focus not obscured: How it works', { tag: ['@markets', '@guide'] }, () => {
    test.use({ wallet: null });

    test('How it works', async ({ app, page }) => {
        await app.goto('/how-it-works', 'how-it-works');
        await expectFocusNeverObscured(page, 'desktop /how-it-works');
    });
});
