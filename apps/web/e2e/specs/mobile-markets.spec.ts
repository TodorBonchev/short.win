/**
 * mobile-markets — the MARKETS pages on a phone (M10-RESP; FRONTEND.md §7.6, §10.2): project
 * `<profile>-phone` (390 × 844, touch, DPR 3, an iPhone user agent). Every step is a real tap
 * (`locator.tap()`, no DOM-click fallback) and axe (WCAG 2.2 A/AA) runs at 0 on every state visited.
 *
 * - the markets list is cards; the sort select reorders them; the filters fold behind a toggle;
 *   a card opens its market;
 * - the market page: the tablist scrolls sideways to Events by taps; "More stats"; the tier sheet;
 *   the action bar opens the trade sheet (`?sheet=trade`) and Back closes it;
 * - create market: Paste is offered for the mint; a rejected token has no Review;
 * - /status and /security fit the screen.
 *
 * Writes the phone PNGs (CSS scale) to `.e2e/screenshots/phone/`, or to the reviewed set
 * `e2e/screenshots/phone/` with `SHORTWIN_E2E_SCREENSHOTS=update`. Signs nothing; connects as
 * `phone` (the phone project's own wallet) only where a page needs a wallet to show its state.
 */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { E2E_DIR, WEB_DIR } from '../env.ts';
import { type App, expect, T, test } from '../fixtures.ts';

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

/** The page is laid out at the screen's width: nothing pans sideways. */
async function fitsTheScreen(page: Page): Promise<void> {
    const m = await page.evaluate(() => ({
        doc: document.documentElement.scrollWidth,
        inner: window.innerWidth,
        scale: window.visualViewport?.scale ?? 1,
    }));
    expect(m.doc, 'document wider than the screen').toBeLessThanOrEqual(m.inner);
    expect(m.scale).toBe(1);
}

/** The rows' mints in their order on screen. */
async function rowMints(app: App): Promise<string[]> {
    return app
        .id(T.marketsTable)
        .locator('tbody tr[data-testid]')
        .evaluateAll((rows) => rows.map((r) => (r.getAttribute('data-testid') ?? '').replace(/^markets-row-/, '')));
}

test.describe('phone markets', { tag: '@markets' }, () => {
    test.use({ wallet: 'phone' });

    test('the markets list: cards, sort, filters, and a card opens its market', async ({ app, page, state }) => {
        const { main } = state.markets;
        await app.goto('/markets', 'markets');
        const row = app.id(T.marketsRow(main.mint));
        await expect(row).toBeVisible();
        // Card mode: each row is a grid, its header off screen (still in the accessibility tree).
        expect(await row.evaluate((r) => getComputedStyle(r).display)).toBe('grid');
        await expect(app.id(T.marketsTable).locator('thead')).toHaveCSS('position', 'absolute');
        await fitsTheScreen(page);
        await app.a11y('phone markets cards');
        await shot(page, 'markets-cards');

        // The sort select and the direction button (card mode) drive the same sort as the headers.
        const sort = app.id(T.tableSort(T.marketsTable));
        await expect(sort).toBeVisible();
        await expect(sort).toHaveValue('tvl');
        const byTvl = await rowMints(app);
        await app.id(T.tableSortDir(T.marketsTable)).tap();
        await expect(page).toHaveURL(/order=asc/);
        await expect(app.id(T.tableSortDir(T.marketsTable))).toHaveAttribute('data-order', 'asc');
        await expect(app.id(T.tableSortDir(T.marketsTable))).toContainText('Ascending');
        await expect.poll(() => rowMints(app)).not.toEqual(byTvl);
        // The (clipped) header's sort buttons stay keyboard reachable; focus shows the header row,
        // so no Tab stop is ever invisible (WCAG 2.4.7 / 2.4.11).
        await app.id(T.tableSortDir(T.marketsTable)).focus();
        await page.keyboard.press('Tab');
        const focused = await page.evaluate(() => {
            const a = document.activeElement as HTMLElement | null;
            const r = a?.getBoundingClientRect();
            const head = a?.closest('thead');
            return {
                testId: a?.getAttribute('data-testid') ?? '',
                w: r?.width ?? 0,
                h: r?.height ?? 0,
                clip: head ? getComputedStyle(head).clip : 'auto',
            };
        });
        // A sort button, or (since the thirteenth round) a header's glossary ⓘ before it.
        expect(focused.testId).toMatch(/^(markets-sort-|glossary-tip-)/);
        expect(focused.w).toBeGreaterThan(8);
        expect(focused.h).toBeGreaterThan(8);
        expect(focused.clip).toBe('auto');
        await page.keyboard.press('Shift+Tab');
        await sort.selectOption('open_positions');
        await expect(page).toHaveURL(/sort=open_positions/);

        // The filters fold behind "Filters"; an active one is counted on the toggle.
        const toggle = app.id(T.marketsFiltersToggle);
        await expect(app.id(T.marketsFilterVenue)).toBeHidden();
        await toggle.tap();
        await expect(toggle).toHaveAttribute('aria-expanded', 'true');
        await expect(app.id(T.marketsFilterVenue)).toBeVisible();
        await app.id(T.marketsFilterTier).selectOption('C');
        await expect(toggle).toContainText('Filters · 1');
        await app.a11y('phone markets filters open');
        await shot(page, 'markets-filters');
        await app.id(T.marketsFilterTier).selectOption({ index: 0 });

        // An empty filtered list offers to clear the filters.
        await app.id(T.marketsSearch).fill('zzzz-no-such-token');
        await expect(app.id(T.marketsEmpty)).toBeVisible();
        await app.id(T.marketsClearFilters).tap();
        await expect(app.id(T.marketsSearch)).toHaveValue('');
        await expect(row).toBeVisible();

        await row.tap();
        await expect(page).toHaveURL(new RegExp(`/m/${main.mint}`));
        await expect(app.id(T.page('market'))).toBeVisible();
    });

    test('the market page: tabs, more stats, the tier sheet and the trade sheet', async ({ app, page, state }) => {
        test.skip(state.program !== 'test-venue', 'needs the test-venue build (mock venue)');
        const { main } = state.markets;
        await app.goto(`/m/${main.mint}`, 'market');
        await expect(app.id(T.marketSource)).toHaveAttribute('data-value', 'indexer');
        // One column; the trade panel waits behind the action bar.
        await expect(app.id(T.shortPanel)).toBeHidden();
        await expect(app.id(T.marketActionShort)).toBeVisible();
        await expect(app.id(T.marketActionShort)).toHaveAccessibleName(/^Short /);
        await fitsTheScreen(page);
        await app.a11y('phone market');
        await shot(page, 'market');

        // More stats: inventory, open interest and lenders behind a toggle.
        await expect(app.id(T.marketStat('inventory'))).toBeHidden();
        await app.id(T.marketStatsMore).tap();
        await expect(app.id(T.marketStatsMore)).toHaveAttribute('aria-expanded', 'true');
        await expect(app.id(T.marketStat('inventory'))).toBeVisible();
        await expect(app.id(T.marketStat('lenders'))).toBeVisible();

        // The tablist scrolls sideways; a tap on Events selects it and keeps it in view.
        const events = app.id(T.marketTab('events'));
        await events.scrollIntoViewIfNeeded();
        await events.tap();
        await expect(events).toHaveAttribute('aria-selected', 'true');
        await expect(events).toBeInViewport();
        // The mock indexer has no events for a market: the empty state, under the (screen-reader) header.
        await expect(page.getByText('No events yet.')).toBeVisible();
        await expect(app.id(T.marketEvents)).toHaveCount(1);
        await fitsTheScreen(page);
        await app.a11y('phone market events');
        await shot(page, 'market-events');
        await app.id(T.marketTab('lenders')).tap();
        await expect(app.id(T.marketLenders)).toBeVisible();
        await fitsTheScreen(page);
        await app.a11y('phone market lenders');

        // "Why this tier": a bottom sheet with a close button.
        await app.id(T.marketTier).tap();
        await expect(app.id(T.marketTierWhy)).toBeVisible();
        await expect(app.id(T.marketTierWhy)).toBeInViewport();
        await app.a11y('phone tier sheet');
        await shot(page, 'market-tier-why');
        await app.id(T.marketTierWhyClose).tap();
        await expect(app.id(T.marketTierWhy)).toHaveCount(0);
        await expect(app.id(T.marketTier)).toBeFocused();

        // The action bar opens the trade sheet at Short; Back closes it and stays on the market.
        await app.id(T.marketActionShort).tap();
        await expect(page).toHaveURL(/sheet=trade/);
        await expect(app.id(T.shortPanel)).toBeVisible();
        const sheet = page.locator('.trade-panel[data-sheet="open"]');
        await expect(sheet).toHaveAttribute('role', 'dialog');
        await expect(sheet).toHaveAttribute('aria-modal', 'true');
        await expect(app.id(T.tradeSheetClose)).toBeVisible();
        await app.a11y('phone trade sheet');
        await shot(page, 'market-trade-sheet');
        await page.goBack();
        await expect(page).not.toHaveURL(/sheet=trade/);
        await expect(app.id(T.shortPanel)).toBeHidden();
        await expect(app.id(T.page('market'))).toBeVisible();

        // Lend opens the same sheet at Lend; ✕ closes it and focus returns to the action bar.
        await app.id(T.marketActionLend).tap();
        await expect(page).toHaveURL(/sheet=trade/);
        await expect(page).toHaveURL(/panel=lend/);
        await expect(app.id(T.lendPanel)).toBeVisible();
        await app.id(T.tradeSheetClose).tap();
        await expect(page).not.toHaveURL(/sheet=trade/);
        await expect(app.id(T.lendPanel)).toBeHidden();
        await expect(app.id(T.marketActionLend)).toBeFocused();
    });

    test('create market: Paste for the mint, and a rejected token has no Review', async ({ app, page, state }) => {
        await app.goto('/markets/new', 'create-market');
        await expect(app.id(T.createMintInput)).toHaveAttribute('autocapitalize', 'none');
        await expect(app.id(T.createMintPaste)).toBeVisible();
        // The form foot is the page's bottom bar: Review and its reason on the first screen.
        await expect(app.id(T.createSubmit)).toBeInViewport();
        await expect(app.id(T.createSubmitReason)).toContainText('mint');
        await fitsTheScreen(page);
        await app.a11y('phone create market');
        await shot(page, 'create-market');

        await app.goto(`/markets/new?mint=${state.createMints.badExtension}`, 'create-market');
        await expect(app.id(T.createVerdict)).toHaveAttribute('data-verdict', 'reject');
        await expect(app.id(T.createSubmit)).toHaveCount(0);
        await expect(page.getByText('Cannot be listed', { exact: true })).toBeVisible();
        await fitsTheScreen(page);
        await app.a11y('phone create market rejected');
        await shot(page, 'create-market-rejected');
    });

    test('status and security fit the screen', async ({ app, page, state }) => {
        await app.goto('/status', 'status');
        await expect(app.id(T.statusDegradedRow(state.markets.paused.mint))).toBeVisible();
        await fitsTheScreen(page);
        await app.a11y('phone status');
        await shot(page, 'status');

        await app.goto('/security', 'security');
        await expect(app.id(T.page('security')).locator('a[href^="mailto:"]').first()).toBeVisible();
        await fitsTheScreen(page);
        await app.a11y('phone security');
        await shot(page, 'security');

        await app.goto(`/m/${state.createMints.ok}`, 'market');
        await expect(app.id(T.marketNotFound)).toBeVisible();
        await fitsTheScreen(page);
        await app.a11y('phone market not listed');
    });
});
