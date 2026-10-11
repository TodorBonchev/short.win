/**
 * guide — the owner's thirteenth-round help on a desktop (FRONTEND.md §3.3, §3.4, §3.11, §4.3):
 *
 * - the markets list's intro strip: three parts, its links, Hide remembered across a reload;
 * - the glossary: a header's ⓘ opens by click and by keyboard (never on hover), Escape returns
 *   focus, a tap outside closes; a stat's ⓘ on the market page;
 * - How it works: reached from the strip's risk link (the heading focused), from the status bar;
 *   its numbers from the chain's config; the approved risk texts without a checkbox;
 * - O-2: the coverage-paused market's Short panel links to the banner instead of repeating it, and
 *   the link focuses the banner;
 * - O-1: the deposit form's order (risks and the acknowledgement after the preview, before the
 *   submit).
 *
 * Read-only (nothing is signed). Writes `70-…`–`75-…` PNGs (to `e2e/screenshots/` with
 * `SHORTWIN_E2E_SCREENSHOTS=update`) and runs axe on each state.
 */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { E2E_DIR, WEB_DIR } from '../env.ts';
import { expect, requireTrading, T, test } from '../fixtures.ts';

const DIR =
    process.env.SHORTWIN_E2E_SCREENSHOTS === 'update'
        ? join(WEB_DIR, 'e2e', 'screenshots')
        : join(E2E_DIR, 'screenshots');
mkdirSync(DIR, { recursive: true });

/** As `screens.spec.ts`: settled, then the whole document (or the viewport). */
async function shot(page: Page, name: string, opts: { fullPage?: boolean } = {}): Promise<void> {
    await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
    const size = page.viewportSize();
    const extra =
        opts.fullPage === false || !size
            ? 0
            : await page.evaluate(() => Math.max(0, document.documentElement.scrollHeight - window.innerHeight));
    if (size && extra > 0)
        await page.setViewportSize({ width: size.width, height: Math.min(size.height + extra, 6_000) });
    await page.screenshot({ path: join(DIR, `${name}.png`), animations: 'disabled' });
    if (size && extra > 0) await page.setViewportSize(size);
}

test.describe('guide: newcomer help', { tag: ['@markets', '@guide'] }, () => {
    test.use({ wallet: 'trader' });

    test('the intro strip: three parts, its links, Hide remembered across a reload', async ({ app, page, state }) => {
        await app.goto('/markets', 'markets');
        const strip = app.id(T.marketsIntro);
        await expect(strip).toBeVisible();
        await expect(strip.getByRole('heading', { level: 3 })).toHaveText([
            'Short a token',
            'Lend a token',
            'What can go wrong',
        ]);
        await expect(app.id(T.marketsIntroLink('short-risks'))).toHaveAttribute('href', '/how-it-works#short-risks');
        await expect(app.id(T.marketsIntroLink('lend-risks'))).toHaveAttribute('href', '/how-it-works#lend-risks');
        // Above the list, which still starts on the first screen.
        await expect(app.id(T.marketsRow(state.markets.main.mint))).toBeInViewport();
        await app.a11y('markets with the intro strip');
        await shot(page, '70-markets-intro', { fullPage: false });

        await app.id(T.marketsIntroDismiss).click();
        await expect(strip).toHaveCount(0);
        await expect(page.getByRole('heading', { level: 1, name: 'Markets' })).toBeFocused();
        await expect(page.getByRole('status').filter({ hasText: 'Introduction hidden.' })).toHaveCount(1);
        await page.reload();
        await expect(app.id(T.marketsTable)).toBeVisible();
        await expect(app.id(T.marketsIntro)).toHaveCount(0);
    });

    test('the glossary: by click and by keyboard, never on hover; Escape returns focus', async ({
        app,
        page,
        state,
    }) => {
        requireTrading(state);
        await app.goto('/markets', 'markets');
        const tip = app.id(T.glossaryTip('tvl'));
        await expect(tip).toBeVisible();
        await tip.hover();
        await expect(app.id(T.glossaryPopover('tvl'))).toHaveCount(0);
        await tip.click();
        const pop = app.id(T.glossaryPopover('tvl'));
        await expect(pop).toBeVisible();
        await expect(tip).toHaveAttribute('aria-expanded', 'true');
        await expect(pop).toContainText('Total value in the market, in SOL');
        // A click outside closes it.
        await page.getByRole('heading', { level: 1, name: 'Markets' }).click();
        await expect(pop).toHaveCount(0);
        // The keyboard: focus, Enter, Escape (focus back on the ⓘ).
        await tip.focus();
        await page.keyboard.press('Enter');
        await expect(pop).toBeVisible();
        await page.keyboard.press('Escape');
        await expect(pop).toHaveCount(0);
        await expect(tip).toBeFocused();
        await expect(tip).toHaveAttribute('aria-expanded', 'false');

        // A stat's ⓘ on the market page; the definition is placed beside it, on screen.
        await app.goto(`/m/${state.markets.main.mint}`, 'market');
        const rate = app.id(T.glossaryTip('rentApr')).first();
        await rate.click();
        const def = app.id(T.glossaryPopover('rentApr'));
        await expect(def).toHaveAttribute('data-placed', '');
        await expect(def).toBeInViewport({ ratio: 1 });
        await app.a11y('market page, glossary open');
        await shot(page, '71-glossary-open', { fullPage: false });
    });

    test('How it works: from the strip’s risk link and the status bar, with the live numbers', async ({
        app,
        page,
    }) => {
        await app.goto('/markets', 'markets');
        await app.id(T.marketsIntroLink('short-risks')).click();
        await expect(app.id(T.page('how-it-works'))).toBeVisible();
        await expect(page).toHaveURL(/\/how-it-works#short-risks$/);
        await expect(app.id(T.howItWorksSection('short-risks'))).toBeFocused();
        // The approved texts verbatim, without a checkbox (nothing is signed here).
        await expect(app.id(T.disclosure('borrower.liquidation'))).toContainText('How this short can end');
        await expect(app.id(T.disclosure('lender.solRepayment'))).toBeVisible();
        await expect(app.id(T.page('how-it-works')).locator('input[type="checkbox"]')).toHaveCount(0);
        await expect(app.id(T.page('how-it-works'))).toContainText(/Open fee: \d[\d.]*% of the position’s notional/);
        await app.a11y('how it works');
        await page.evaluate(() => window.scrollTo(0, 0));
        await shot(page, '72-how-it-works');

        await app.goto('/security', 'security');
        await app.id(T.nav('how-it-works')).click();
        await expect(app.id(T.page('how-it-works'))).toBeVisible();
    });

    test('O-2: the paused market’s Short panel links to the coverage banner and focuses it', async ({
        app,
        page,
        state,
    }) => {
        requireTrading(state);
        await app.goto(`/m/${state.markets.paused.mint}?panel=short`, 'market');
        await app.connectWallet();
        const panel = app.id(T.shortPanel);
        await expect(panel.getByTestId(T.shortBlocked)).toHaveAttribute('data-code', 'CoveragePaused');
        // The note stays; the disclosure the banner shows is not repeated in the panel.
        await expect(panel.getByTestId(T.disclosure('market.coveragePause'))).toHaveCount(0);
        const link = panel.getByTestId(T.shortCoverageLink);
        await expect(link).toHaveText('Read the full coverage notice');
        await app.a11y('short panel with the coverage link');
        await shot(page, '73-short-coverage-link', { fullPage: false });
        await link.click();
        const banner = app.id(T.marketBanner('coverage-paused'));
        await expect(banner).toBeFocused();
        await expect(banner).toBeInViewport();
    });
});

test.describe('guide: the deposit order', { tag: ['@lend', '@guide'] }, () => {
    // A wallet that holds the main token (its source account has a balance).
    test.use({ wallet: 'errors' });

    test('O-1: the deposit form puts the risks and the acknowledgement after the preview', async ({
        app,
        page,
        state,
    }) => {
        const main = state.markets.main;
        await page.setViewportSize({ width: 1440, height: 1800 });
        await app.goto(`/m/${main.mint}?panel=lend&action=deposit`, 'market');
        await app.connectWallet();
        const panel = app.id(T.lendPanel);
        await panel.getByTestId(T.lendAmountInput).fill('1');
        await expect(panel.getByTestId(T.lendPreview('shares'))).toHaveAttribute('data-value', /^\d+$/);
        // Document order of the form's parts (the reading order: no CSS reorders, FRONTEND.md §7.6).
        const order = await panel.evaluate(
            (root, ids) => {
                const all = [...root.querySelectorAll('[data-testid]')];
                return ids.map((id) => all.findIndex((e) => e.getAttribute('data-testid') === id));
            },
            [
                T.lendAmountInput,
                T.lendSlippageInput,
                T.lendPreview('shares'),
                T.disclosure('lender.risks'),
                T.disclosure('lender.solRepayment'),
                T.lendSubmit,
            ],
        );
        expect(order.every((n) => n >= 0)).toBe(true);
        expect([...order].sort((a, b) => a - b)).toEqual(order);
        await app.a11y('deposit form, new order');
        await panel.screenshot({ path: join(DIR, '74-lend-deposit-order.png'), animations: 'disabled' });
    });
});
