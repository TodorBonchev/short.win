/**
 * mobile-shell — the app shell on a phone (M10-RESP; FRONTEND.md §7.6, §10.2): project
 * `<profile>-phone` (390 × 844, touch, DPR 3, an iPhone user agent). Every step is a real tap
 * (`locator.tap()`, no DOM-click fallback) and axe (WCAG 2.2 A/AA) runs at 0 on every state visited.
 *
 * - the bottom tab bar reaches Markets, Positions and Lend (`aria-current` follows), More reaches
 *   Status and the security policy;
 * - the status strip expands to every status item;
 * - the search overlay keeps a mixed-case mint as typed and its result opens the market;
 * - without a wallet, the wallet popup offers the three open-in-wallet links for this very page
 *   (hrefs only: never followed); connected, the menu has Copy and Explorer.
 *
 * Writes the shell's phone PNGs (CSS scale) to `.e2e/screenshots/phone/`, or to the reviewed set
 * `e2e/screenshots/phone/` with `SHORTWIN_E2E_SCREENSHOTS=update`. Signs nothing; connects as
 * `phone` (the phone project's own wallet).
 */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { browseLink, MOBILE_WALLETS } from '../../src/wallet/deepLinks.ts';
import { E2E_DIR, WEB_DIR } from '../env.ts';
import { type App, E2E_WALLET_NAME, expect, T, test } from '../fixtures.ts';

const DIR = join(
    process.env.SHORTWIN_E2E_SCREENSHOTS === 'update'
        ? join(WEB_DIR, 'e2e', 'screenshots')
        : join(E2E_DIR, 'screenshots'),
    'phone',
);
mkdirSync(DIR, { recursive: true });

/** The first screen, at CSS scale (what the user sees). */
async function shot(page: Page, name: string): Promise<void> {
    await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
    await page.screenshot({ path: join(DIR, `${name}.png`), animations: 'disabled', scale: 'css' });
}

/** Connects the injected wallet through the header popup, by taps. */
async function tapConnect(app: App): Promise<void> {
    const address = app.id(T.walletAddress);
    if (await address.isVisible()) return;
    await app.id(T.walletButton).tap();
    await expect(app.id(T.walletMenu)).toBeVisible();
    await app.id(T.walletOption(E2E_WALLET_NAME)).tap();
    await expect(address).toHaveAttribute('data-value', app.walletAddress);
}

/** axe on the shell only (header with the tab bar, the status strip). */
async function shellA11y(app: App, label: string): Promise<void> {
    await app.a11y(`${label} (header)`, `[data-testid="${T.header}"]`);
    await app.a11y(`${label} (status bar)`, `[data-testid="${T.statusBar}"]`);
}

test.describe('phone shell', { tag: '@scaffold' }, () => {
    test.use({ wallet: 'phone' });

    test('the tab bar, More, Status and Security', async ({ app, page }) => {
        await app.goto('/markets', 'markets');
        expect(await page.evaluate(() => window.matchMedia('(pointer: coarse)').matches)).toBe(true);
        await expect(app.id(T.navMore)).toBeVisible();
        await expect(app.id(T.nav('status'))).toBeHidden();
        for (const key of ['positions', 'lend', 'markets'] as const) {
            await app.id(T.nav(key)).tap();
            await expect(app.id(T.page(key))).toBeVisible();
            await expect(app.id(T.nav(key))).toHaveAttribute('aria-current', 'page');
            await app.a11y(`phone ${key}`);
        }
        await shot(page, 'markets');

        await app.id(T.navMore).tap();
        await expect(app.id(T.moreSheet)).toBeVisible();
        await expect(app.id(T.navMore)).toHaveAttribute('aria-expanded', 'true');
        await expect(app.id(T.moreSolUsd)).toBeVisible();
        await app.a11y('phone more sheet');
        await shot(page, 'more');
        await app.id(T.moreStatus).tap();
        await expect(app.id(T.page('status'))).toBeVisible();
        await expect(app.id(T.moreSheet)).toBeHidden();
        // The status and security pages' own content is the MARKETS track's (mobile-markets); here
        // the shell around them.
        await shellA11y(app, 'phone status');

        // From the markets list again: until the MARKETS track's phone pass, /status is laid out
        // wider than a phone (the visual viewport pans, so a tap there can miss the tab bar).
        await app.goto('/markets', 'markets');
        await app.id(T.navMore).tap();
        await app.id(T.moreSecurity).tap();
        await expect(app.id(T.page('security'))).toBeVisible();
        await shellA11y(app, 'phone security');
    });

    test('the status strip expands to every status item', async ({ app, page }) => {
        await app.goto('/markets', 'markets');
        const summary = app.id(T.statusSummary);
        await expect(summary).toBeVisible();
        await expect(summary).toHaveAttribute('data-state', /^(ok|warn|bad)$/);
        await expect(app.id(T.statusIndexer)).toBeHidden();
        await summary.tap();
        await expect(summary).toHaveAttribute('aria-expanded', 'true');
        await expect(app.id(T.statusIndexer)).toBeVisible();
        await expect(app.id(T.statusRpc)).toBeVisible();
        await expect(app.id(T.nav('security'))).toBeVisible();
        await app.a11y('phone status expanded');
        await shot(page, 'status-expanded');
        await summary.tap();
        await expect(app.id(T.statusIndexer)).toBeHidden();
        // Escape from inside the list closes it and gives focus back to the summary (not <body>).
        await summary.tap();
        await app.id(T.nav('security')).focus();
        await page.keyboard.press('Escape');
        await expect(app.id(T.statusIndexer)).toBeHidden();
        await expect(summary).toBeFocused();
    });

    test('the search overlay keeps a mint as typed and opens its market', async ({ app, page, state }) => {
        const main = state.markets.main;
        await app.goto('/markets', 'markets');
        await expect(app.id(T.searchInput)).toBeHidden();
        // Cancel closes the overlay and gives focus back to the Search button.
        await app.id(T.searchToggle).tap();
        await expect(app.id(T.searchInput)).toBeVisible();
        await app.id(T.searchClose).tap();
        await expect(app.id(T.searchInput)).toBeHidden();
        await expect(app.id(T.searchToggle)).toBeFocused();

        await app.id(T.searchToggle).tap();
        const input = app.id(T.searchInput);
        await expect(input).toBeVisible();
        await expect(input).toBeFocused();
        await expect(input).toHaveAttribute('autocapitalize', 'none');
        await input.pressSequentially(main.mint);
        await expect(input).toHaveValue(main.mint);
        await expect(app.id(T.searchResult(main.mint))).toBeVisible();
        await app.a11y('phone search overlay');
        await shot(page, 'search');
        await app.id(T.searchResult(main.mint)).tap();
        await expect(page).toHaveURL(new RegExp(`/m/${main.mint}`));
        await expect(app.id(T.page('market'))).toBeVisible();
    });

    test('connected: the wallet sheet has the account actions', async ({ app, page }) => {
        await app.goto('/markets', 'markets');
        await tapConnect(app);
        await app.id(T.walletButton).tap();
        await expect(app.id(T.walletMenu)).toBeVisible();
        await expect(app.id(T.walletCopyAddress)).toBeVisible();
        await expect(app.id(T.walletExplorer)).toHaveAttribute('href', new RegExp(app.walletAddress));
        await expect(app.id(T.walletMyPositions)).toBeVisible();
        // A bottom sheet with its own title and ✕.
        await expect(app.id(T.walletMenu).getByRole('heading', { name: 'Wallet' })).toBeVisible();
        await expect(app.id(T.walletSheetClose)).toBeVisible();
        await app.a11y('phone wallet connected');
        await shot(page, 'wallet-connected');
        // Back closes it (its own history entry) and stays on the page; focus is back on the button.
        await page.goBack();
        await expect(app.id(T.walletMenu)).toHaveCount(0);
        await expect(page).toHaveURL(/\/markets(\?|$)/);
        await expect(app.id(T.walletButton)).toBeFocused();
        // ✕ closes it too, and pops that entry: one Back from /markets later leaves nothing behind.
        await app.id(T.walletButton).tap();
        await expect(app.id(T.walletMenu)).toBeVisible();
        await app.id(T.walletSheetClose).tap();
        await expect(app.id(T.walletMenu)).toHaveCount(0);
        await expect(app.id(T.walletButton)).toBeFocused();
        // A menu link replaces the sheet's entry: Back from /lend returns to /markets, popup closed.
        await app.id(T.walletButton).tap();
        await app.id(T.walletMyLending).tap();
        await expect(app.id(T.page('lend'))).toBeVisible();
        await expect(app.id(T.walletMenu)).toHaveCount(0);
        await page.goBack();
        await expect(page).toHaveURL(/\/markets(\?|$)/);
        await expect(app.id(T.page('markets'))).toBeVisible();
        await expect(app.id(T.walletMenu)).toHaveCount(0);
    });

    test('not found offers a way back', async ({ app, page }) => {
        await app.goto('/no-such-page');
        await expect(app.id(T.notFoundCta)).toBeVisible();
        await app.a11y('phone not found');
        await shot(page, 'not-found');
        await app.id(T.notFoundCta).tap();
        await expect(app.id(T.page('markets'))).toBeVisible();
    });
});

test.describe('phone without a wallet', { tag: '@scaffold' }, () => {
    test.use({ wallet: null });

    test('the wallet popup opens this page in a wallet app (links only, never followed)', async ({
        app,
        page,
        state,
    }) => {
        await app.goto(`/m/${state.markets.main.mint}?panel=lend&action=deposit`, 'market');
        await expect(app.id(T.walletButton)).toHaveAccessibleName('Connect wallet');
        await app.id(T.walletButton).tap();
        const menu = app.id(T.walletMenu);
        await expect(menu).toBeVisible();
        await expect(menu).toHaveAttribute('role', 'dialog');
        await expect(menu.locator('[role="menu"]')).toHaveCount(0);
        const url = page.url();
        const origin = new URL(url).origin;
        for (const w of MOBILE_WALLETS) {
            await expect(app.id(T.walletOpenIn(w))).toHaveAttribute('href', browseLink(w, url, origin));
        }
        await expect(app.id(T.walletReload)).toBeVisible();
        await app.a11y('phone wallet without a wallet');
        await shot(page, 'wallet-no-wallet');
    });
});
