/**
 * mobile-guide — the owner's thirteenth-round help on a phone (FRONTEND.md §3.3, §3.4, §3.11, §4.3;
 * project `<profile>-phone`: 390 × 844, touch, DPR 3). Real taps; axe at 0 on every state.
 *
 * - the intro strip on the markets list's first screen; Hide; How it works from the More sheet;
 * - a glossary ⓘ opens a bottom sheet over a scrim; Escape returns focus; a tap on the scrim closes;
 * - in a market card, a tap on the sheet's text, its ✕ or the scrim (over another card) opens no
 *   market, and focus returns to the ⓘ;
 * - O-2: in the trade sheet the coverage link says it closes the sheet, closes it and focuses the
 *   page's banner;
 * - O-1: the deposit sheet's risks and acknowledgement after the preview.
 *
 * Writes `e2e/screenshots/phone/` PNGs with `SHORTWIN_E2E_SCREENSHOTS=update`. Signs nothing.
 */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { E2E_DIR, WEB_DIR } from '../env.ts';
import { type App, E2E_WALLET_NAME, expect, requireTrading, T, test } from '../fixtures.ts';

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

test.describe('phone guide', { tag: ['@markets', '@guide'] }, () => {
    test.use({ wallet: 'phone' });

    test('the intro strip, Hide, and How it works from the More sheet', async ({ app, page }) => {
        await app.goto('/markets', 'markets');
        const strip = app.id(T.marketsIntro);
        await expect(strip).toBeVisible();
        await expect(strip).toBeInViewport();
        await app.a11y('phone markets with the intro strip');
        await shot(page, 'markets-intro');

        await app.id(T.navMore).tap();
        const row = app.id(T.moreHowItWorks);
        await expect(row).toBeVisible();
        await row.tap();
        await expect(app.id(T.page('how-it-works'))).toBeVisible();
        await expect(app.id(T.navMore)).toHaveAttribute('aria-current', 'true');
        const fits = await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
        expect(fits, 'How it works fits the screen').toBe(true);
        await app.a11y('phone how it works');
        await shot(page, 'how-it-works');

        await app.goto('/markets', 'markets');
        await app.id(T.marketsIntroDismiss).tap();
        await expect(app.id(T.marketsIntro)).toHaveCount(0);
        await page.reload();
        await expect(app.id(T.marketsTable)).toBeVisible();
        await expect(app.id(T.marketsIntro)).toHaveCount(0);
    });

    test('a glossary ⓘ opens a bottom sheet; Escape returns focus, the scrim closes it', async ({
        app,
        page,
        state,
    }) => {
        requireTrading(state);
        await app.goto(`/m/${state.markets.main.mint}`, 'market');
        const tip = app.id(T.glossaryTip('rentApr')).first();
        await tip.tap();
        const sheet = app.id(T.glossaryPopover('rentApr'));
        await expect(sheet).toBeVisible();
        // A bottom sheet: its bottom edge on the screen's.
        const box = await sheet.boundingBox();
        const vh = page.viewportSize()?.height ?? 0;
        expect(Math.round((box?.y ?? 0) + (box?.height ?? 0))).toBe(vh);
        await app.a11y('phone glossary sheet');
        await shot(page, 'glossary-open');
        await page.keyboard.press('Escape');
        await expect(sheet).toHaveCount(0);
        await expect(tip).toBeFocused();
        await tip.tap();
        await expect(sheet).toBeVisible();
        await page.touchscreen.tap(20, 80);
        await expect(sheet).toHaveCount(0);
    });

    test('a card’s ⓘ: its sheet and its scrim open nothing under them; focus returns to the ⓘ', async ({
        app,
        page,
        state,
    }) => {
        requireTrading(state);
        await app.goto('/markets', 'markets');
        const row = app.id(T.marketsRow(state.markets.paused.mint));
        await expect(row).toBeVisible();
        const tip = row.getByTestId(T.glossaryTip('flagCoveragePaused'));
        await tip.scrollIntoViewIfNeeded();
        await tip.tap();
        const sheet = app.id(T.glossaryPopover('flagCoveragePaused'));
        await expect(sheet).toBeVisible();
        // A tap on the definition's text: nothing opens.
        await sheet.locator('p').first().tap();
        await expect(sheet).toBeVisible();
        await expect(page).toHaveURL(/\/markets/);
        // A tap on the scrim over a market card: it closes, the card does not open, focus is back.
        const rowBox = await row.boundingBox();
        const sheetBox = await sheet.boundingBox();
        const y = (rowBox?.y ?? 0) + 8;
        expect(y, 'the card is above the sheet').toBeLessThan(sheetBox?.y ?? 0);
        expect(y).toBeGreaterThan(0);
        await page.touchscreen.tap((rowBox?.x ?? 0) + (rowBox?.width ?? 0) / 2, y);
        await expect(sheet).toHaveCount(0);
        await page.waitForTimeout(300);
        await expect(page).toHaveURL(/\/markets/);
        await expect(tip).toBeFocused();
        // Its ✕ closes it without opening the card either.
        await tip.tap();
        await expect(sheet).toBeVisible();
        await sheet.getByTestId(T.glossaryClose).tap();
        await expect(sheet).toHaveCount(0);
        await page.waitForTimeout(300);
        await expect(page).toHaveURL(/\/markets/);
        await expect(tip).toBeFocused();
    });

    test('O-2: in the trade sheet the coverage link closes it and focuses the banner', async ({ app, page, state }) => {
        requireTrading(state);
        await app.goto(`/m/${state.markets.paused.mint}`, 'market');
        await tapConnect(app);
        await app.id(T.marketActionShort).tap();
        await expect(page.locator('.trade-panel')).toHaveAttribute('data-sheet', 'open');
        const panel = app.id(T.shortPanel);
        await expect(panel.getByTestId(T.shortBlocked)).toHaveAttribute('data-code', 'CoveragePaused');
        await expect(panel.getByTestId(T.disclosure('market.coveragePause'))).toHaveCount(0);
        const link = panel.getByTestId(T.shortCoverageLink);
        await expect(link).toHaveText('Close and read the full coverage notice');
        await app.a11y('phone trade sheet with the coverage link');
        await shot(page, 'coverage-link');
        await link.tap();
        await expect(page.locator('.trade-panel')).not.toHaveAttribute('data-sheet', 'open');
        await expect(page).not.toHaveURL(/sheet=trade/);
        const banner = app.id(T.marketBanner('coverage-paused'));
        await expect(banner).toBeFocused();
        await expect(banner).toBeInViewport();
        await shot(page, 'coverage-banner-focused');
    });
});

test.describe('phone guide: the deposit order', { tag: ['@lend', '@guide'] }, () => {
    test.use({ wallet: 'phone' });

    test('O-1: the deposit sheet puts the risks and the acknowledgement after the preview', async ({
        app,
        page,
        state,
    }) => {
        await app.goto(`/m/${state.markets.main.mint}`, 'market');
        await tapConnect(app);
        await app.id(T.marketActionLend).tap();
        const panel = app.id(T.lendPanel);
        await expect(panel).toBeVisible();
        await panel.getByTestId(T.lendAmountInput).fill('1');
        await expect(panel.getByTestId(T.lendPreview('shares'))).toHaveAttribute('data-value', /^\d+$/);
        const ack = panel.getByTestId(T.disclosureAck('lender.solRepayment'));
        await ack.scrollIntoViewIfNeeded();
        const [preview, risks, ackBox] = await Promise.all([
            panel.getByTestId(T.lendPreview('shares')).boundingBox(),
            panel.getByTestId(T.disclosure('lender.risks')).boundingBox(),
            ack.boundingBox(),
        ]);
        expect((preview?.y ?? 0) < (risks?.y ?? 0)).toBe(true);
        expect((risks?.y ?? 0) < (ackBox?.y ?? 0)).toBe(true);
        await app.a11y('phone deposit sheet, new order');
        await shot(page, 'lend-deposit-order');
    });
});
