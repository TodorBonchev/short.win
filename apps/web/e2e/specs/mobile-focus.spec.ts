/**
 * mobile-focus — Focus Not Obscured (Minimum), WCAG 2.2 2.4.11, on a phone (FRONTEND.md §8,
 * §10.2; project `<profile>-phone`: 390 × 844, touch, DPR 3): a Tab walk (a phone with a keyboard,
 * or switch access) over the markets list (the intro strip shown), a market's Overview tab with
 * its action bar, the Lending page and How it works. No focused control may sit entirely under the
 * sticky header, the tab bar, the status strip or the action bar (e2e/focus-obscured.ts; partial
 * covers are annotations). Connects as `phone` by taps; signs nothing.
 */

import { type App, E2E_WALLET_NAME, expect, T, test } from '../fixtures.ts';
import { expectFocusNeverObscured } from '../focus-obscured.ts';

/** Connects the injected wallet through the header popup, by taps. */
async function tapConnect(app: App): Promise<void> {
    const address = app.id(T.walletAddress);
    if (await address.isVisible()) return;
    await app.id(T.walletButton).tap();
    await expect(app.id(T.walletMenu)).toBeVisible();
    await app.id(T.walletOption(E2E_WALLET_NAME)).tap();
    await expect(address).toHaveAttribute('data-value', app.walletAddress);
}

test.describe('phone focus not obscured: markets', { tag: '@markets' }, () => {
    test.use({ wallet: 'phone' });

    test('the markets list with the intro strip', async ({ app, page, state }) => {
        await app.goto('/markets', 'markets');
        await expect(app.id(T.marketsIntro)).toBeVisible();
        await expect(app.id(T.marketsRow(state.markets.main.mint))).toBeVisible();
        await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
        await expectFocusNeverObscured(page, 'phone /markets');
    });

    test("a market's Overview tab, with the action bar", async ({ app, page, state }) => {
        await app.goto(`/m/${state.markets.main.mint}`, 'market');
        await tapConnect(app);
        await expect(app.id(T.marketActionShort)).toBeVisible();
        await expect(app.id(T.marketActionLend)).toBeVisible();
        await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
        await expectFocusNeverObscured(page, 'phone market overview');
    });
});

test.describe('phone focus not obscured: lending', { tag: '@lend' }, () => {
    test.use({ wallet: 'phone' });

    test('the Lending page', async ({ app, page }) => {
        await app.goto('/lend', 'lend');
        await tapConnect(app);
        await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
        await expectFocusNeverObscured(page, 'phone /lend');
    });
});

test.describe('phone focus not obscured: How it works', { tag: ['@markets', '@guide'] }, () => {
    test.use({ wallet: null });

    test('How it works', async ({ app, page }) => {
        await app.goto('/how-it-works', 'how-it-works');
        await expectFocusNeverObscured(page, 'phone /how-it-works');
    });
});
