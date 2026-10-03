/**
 * screens — a visual pass over every page (FRONTEND.md §7, §8, §10.2): each route and state a user
 * reaches, rendered by the production build against the seeded validator, saved as a full-page PNG
 * and scanned with axe. Read-only: reviews are opened and cancelled, nothing is signed.
 *
 * The PNGs go to `.e2e/screenshots/` (gitignored); `SHORTWIN_E2E_SCREENSHOTS=update` writes the
 * committed reference set in `e2e/screenshots/` instead (docs/DEVELOPMENT.md "Web E2E").
 */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { E2E_DIR, WEB_DIR } from '../env.ts';
import { type App, expect, need, requireTrading, requireVenueFixtures, T, test } from '../fixtures.ts';

const DIR =
    process.env.SHORTWIN_E2E_SCREENSHOTS === 'update'
        ? join(WEB_DIR, 'e2e', 'screenshots')
        : join(E2E_DIR, 'screenshots');
mkdirSync(DIR, { recursive: true });

/**
 * Waits for the page to settle (no skeleton rows left), then saves `<name>.png`. The shell keeps
 * the header and the status bar fixed and scrolls `main` (FRONTEND.md §3.1), so a full capture
 * grows the viewport to `main`'s scroll height for the shot and restores it afterwards.
 */
async function shot(page: Page, name: string, opts: { fullPage?: boolean } = {}): Promise<void> {
    await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
    const size = page.viewportSize();
    const extra =
        opts.fullPage === false || !size
            ? 0
            : await page.evaluate(() => {
                  const main = document.querySelector('main');
                  return main ? Math.max(0, main.scrollHeight - main.clientHeight) : 0;
              });
    if (size && extra > 0)
        await page.setViewportSize({ width: size.width, height: Math.min(size.height + extra, 6_000) });
    await page.screenshot({ path: join(DIR, `${name}.png`), animations: 'disabled' });
    if (size && extra > 0) await page.setViewportSize(size);
}

async function marketTab(app: App, tab: Parameters<typeof T.marketTab>[0]): Promise<void> {
    await app.id(T.marketTab(tab)).click();
    await expect(app.id(T.marketTab(tab))).toHaveAttribute('aria-selected', 'true');
}

test.describe('screens: browsing', { tag: ['@screens', '@markets'] }, () => {
    test.use({ wallet: 'trader' });

    test('markets, search, status, not found', async ({ app, page, state }) => {
        await app.goto('/markets', 'markets');
        await expect(app.id(T.marketsRow(state.markets.main.mint))).toBeVisible();
        await shot(page, '01-markets');

        await app.goto(`/markets?q=${state.createMints.ok}`, 'markets');
        await expect(app.id(T.marketsCreateLink)).toBeVisible();
        await shot(page, '02-markets-search-no-market');

        await app.goto('/status', 'status');
        await expect(app.id(T.statusDegradedRow(state.markets.paused.mint))).toBeVisible();
        await shot(page, '03-status');

        await app.goto('/m/not-a-mint');
        await expect(page.getByText(/Not found/)).toBeVisible();
        await shot(page, '04-not-found');
        await app.a11y('not found');
    });

    test('security policy (M10 Q3)', async ({ app, page }) => {
        await app.goto('/security', 'security');
        const policy = app.id(T.page('security'));
        await expect(policy.locator('a[href="mailto:security@short.win"]').first()).toBeVisible();
        for (const name of ['Scope', 'Reporting a vulnerability', 'What happens next', 'Safe harbour', 'Audits']) {
            await expect(policy.getByRole('region', { name })).toBeVisible();
        }
        await expect(policy.getByRole('region', { name: 'What happens next' })).toContainText('best effort');
        await shot(page, '60-security');
        await app.a11y('security policy');
        // The status bar links to it from every page.
        await app.goto('/markets', 'markets');
        await app.id(T.nav('security')).click();
        await expect(app.id(T.page('security'))).toBeVisible();
    });

    test('create market: ok, rejected, pump curve', async ({ app, page, state }) => {
        await app.goto(`/markets/new?mint=${state.createMints.ok}`, 'create-market');
        await app.connectWallet();
        await expect(app.id(T.createVerdict)).toHaveAttribute('data-verdict', 'ok');
        await expect(app.id(T.createCost)).toHaveAttribute('data-value', /^\d+$/);
        await shot(page, '05-create-market-ok');

        await app.goto(`/markets/new?mint=${state.createMints.badExtension}`, 'create-market');
        await expect(app.id(T.createVerdict)).toHaveAttribute('data-verdict', 'reject');
        await shot(page, '06-create-market-rejected');

        requireVenueFixtures(state);
        await app.goto(`/markets/new?mint=${need(state.venueMints, 'venue mints').pumpCurve}`, 'create-market');
        await expect(app.id(T.createVenue)).toBeVisible();
        await shot(page, '07-create-market-pump-curve');
    });

    test('market page: every tab, the tier popover, a paused and a list-only market', async ({ app, page, state }) => {
        requireTrading(state);
        const main = state.markets.main;
        await app.goto(`/m/${main.mint}`, 'market');
        await expect(app.id(T.marketSource)).toHaveAttribute('data-value', 'indexer');
        await expect(app.id(T.shortPanel)).toBeVisible();
        await shot(page, '10-market-overview');

        for (const tab of ['positions', 'lenders', 'liquidations', 'pools', 'coverage', 'events'] as const) {
            await marketTab(app, tab);
            await shot(page, `11-market-tab-${tab}`);
            await app.a11y(`market tab ${tab}`);
        }

        await app.id(T.marketTier).click();
        await expect(app.id(T.marketTierWhy)).toBeVisible();
        await shot(page, '12-market-tier-why', { fullPage: false });
        await app.a11y('tier popover');
        await page.keyboard.press('Escape');

        await app.goto(`/m/${state.markets.paused.mint}`, 'market');
        await expect(app.id(T.marketBanner('coverage-paused'))).toBeVisible();
        await shot(page, '13-market-coverage-paused');

        await app.goto(`/m/${state.markets.t22.mint}`, 'market');
        await expect(app.id(T.marketFlag('list-only'))).toBeVisible();
        await shot(page, '14-market-list-only');
    });

    test('the laptop minimum: 1024 px', async ({ app, page, state }) => {
        await page.setViewportSize({ width: 1024, height: 720 });
        await app.goto(`/m/${state.markets.main.mint}`, 'market');
        await app.connectWallet();
        await expect(app.id(T.shortPanel)).toBeVisible();
        // No horizontal page scroll at the minimum width (FRONTEND.md §7.3).
        const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
        expect(overflow, 'horizontal overflow at 1024 px').toBeLessThanOrEqual(0);
        await shot(page, '15-market-1024');
        await app.goto('/markets', 'markets');
        await expect(app.id(T.marketsRow(state.markets.main.mint))).toBeVisible();
        // The widest table fits too: no sideways scroll inside it either.
        const tableOverflow = await app
            .id(T.marketsTable)
            .evaluate((t) => t.scrollWidth - (t.parentElement?.clientWidth ?? t.scrollWidth));
        expect(tableOverflow, 'markets table overflow at 1024 px').toBeLessThanOrEqual(0);
        await shot(page, '16-markets-1024');
    });
});

test.describe('screens: short', { tag: ['@screens', '@short'] }, () => {
    test.use({ wallet: 'trader' });

    test('the short panel with a preview, and its review', async ({ app, page, state }) => {
        requireTrading(state);
        const main = state.markets.main;
        await app.goto(`/m/${main.mint}?panel=short`, 'market');
        await app.connectWallet();
        const panel = app.id(T.shortPanel);
        await panel.getByTestId(T.shortQInput).fill('250000');
        await expect(panel.getByTestId(T.shortPreview('margin'))).toHaveAttribute('data-value', /^\d+$/);
        await shot(page, '20-short-preview');
        await panel.getByTestId(T.disclosureAck('borrower.liquidation')).check();
        await panel.getByTestId(T.shortSubmit).click();
        expect(await app.simulated()).toBe('review');
        await expect(app.inModal(T.txInstruction('0'))).toHaveAttribute('data-name', 'OpenShort');
        await shot(page, '21-short-review', { fullPage: false });
        await app.a11y('open-short review');
        await app.closeModal();

        await app.goto(`/m/${state.markets.paused.mint}?panel=short`, 'market');
        await expect(app.id(T.shortBlocked)).toHaveAttribute('data-code', 'CoveragePaused');
        await shot(page, '22-short-blocked');
    });
});

test.describe('screens: positions', { tag: ['@screens', '@short'] }, () => {
    test.use({ wallet: 'holder' });

    test('the dashboard and a position', async ({ app, page, state }) => {
        requireTrading(state);
        const holder = need(state.positions, 'positions').holder;
        await app.goto('/positions', 'positions');
        await app.connectWallet();
        await expect(app.id(T.positionRow(holder.position))).toBeVisible();
        await expect(app.id(T.positionCell('cr')).first()).toHaveAttribute('data-value', /^\d+$/);
        await shot(page, '30-positions');

        await app.goto(`/positions/${holder.position}`, 'position');
        await expect(app.id(T.positionDetail)).toHaveAttribute('data-value', holder.position);
        await expect(app.id(T.closePreview('payout'))).toBeVisible();
        await shot(page, '31-position-detail');
    });
});

test.describe('screens: lend', { tag: ['@screens', '@lend'] }, () => {
    test.use({ wallet: 'claimer' });

    test('deposit, withdraw, claim and the lender dashboard', async ({ app, page, state }) => {
        const main = state.markets.main;
        for (const action of ['deposit', 'withdraw', 'claim'] as const) {
            await app.goto(`/m/${main.mint}?panel=lend&action=${action}`, 'market');
            await app.connectWallet();
            await expect(app.id(T.lendPanel)).toHaveAttribute('data-action', action);
            if (action === 'deposit') await app.id(T.lendAmountInput).fill('1000');
            if (action === 'claim')
                await expect(app.id(T.claimValue('claimable'))).toHaveAttribute('data-value', /^\d+$/);
            await shot(page, `40-lend-${action}`);
        }
        await app.goto('/lend', 'lend');
        await expect(app.id(T.lenderRow(main.market))).toBeVisible();
        await shot(page, '41-lender-dashboard');
    });
});

test.describe('screens: CPI Guard and memo', { tag: ['@screens', '@lend'] }, () => {
    test.use({ wallet: 'guard' });

    test('the CPI Guard notice and the bracket', async ({ app, page, state }) => {
        const t22 = state.markets.t22;
        await app.goto(`/m/${t22.mint}?panel=lend&action=deposit`, 'market');
        await app.connectWallet();
        const panel = app.id(T.lendPanel);
        await app.selectTokenAccount(panel.getByTestId(T.lendTokenAccount), state.tokenAccounts.guard.guarded);
        await panel.getByTestId(T.lendAmountInput).fill('5');
        await expect(panel.getByTestId(T.lendCpiGuardNotice)).toHaveAttribute('data-state', 'blocked');
        await shot(page, '42-lend-cpi-guard-notice');
        await panel.getByTestId(T.lendCpiGuardBracket).check();
        await panel.getByTestId(T.disclosureAck('lender.solRepayment')).check();
        await panel.getByTestId(T.lendSubmit).click();
        expect(await app.simulated()).toBe('review');
        await expect(app.inModal(T.txInstruction('0'))).toHaveAttribute('data-name', 'DisableCpiGuard');
        await shot(page, '43-lend-cpi-guard-bracket-review', { fullPage: false });
        await app.closeModal();
    });
});

test.describe('screens: indexer down', { tag: ['@screens', '@markets'] }, () => {
    test.use({ wallet: 'trader', indexer: 'down' });

    test('the market page from the chain', async ({ app, page, state }) => {
        await app.goto(`/m/${state.markets.main.mint}`, 'market');
        await expect(app.id(T.fallbackBanner)).toBeVisible();
        await expect(app.id(T.marketSource)).toHaveAttribute('data-value', 'chain');
        await shot(page, '50-fallback-market');
        await app.goto('/markets', 'markets');
        await expect(app.id(T.marketsOpenMint)).toBeVisible();
        await shot(page, '51-fallback-markets');
    });
});
