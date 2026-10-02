/**
 * smoke (FRONTEND.md §10.2; runs against the scaffold — `@scaffold`): the shell, nav, search by
 * mint and by ticker, wallet connect / disconnect (and the silent reconnect after a reload), the
 * status bar's states, the WASM math module, and axe on every route.
 */
import { E2E_WALLET_NAME, expect, T, test } from '../fixtures.ts';

test.use({ wallet: 'trader' });

const ROUTES = [
    ['markets', 'markets'],
    ['positions', 'positions'],
    ['lend', 'lend'],
    ['status', 'status'],
] as const;

test.describe('smoke', { tag: '@scaffold' }, () => {
    test('shell, nav and the status bar', async ({ app, page, indexerMock }) => {
        await app.goto('/', 'markets');
        await expect(page).toHaveURL(/\/markets$/);
        await expect(app.id(T.clusterBadge)).toHaveText(/localnet/i);
        for (const [key, root] of ROUTES) {
            await app.id(T.nav(key)).click();
            await expect(app.id(T.page(root))).toBeVisible();
            await expect(app.id(T.nav(key))).toHaveAttribute('aria-current', 'page');
        }
        await expect(app.id(T.statusBar)).toBeVisible();
        await expect(app.id(T.statusRpc)).toHaveAttribute('data-state', 'ok');
        await expect(app.id(T.statusIndexer)).toHaveAttribute('data-state', 'live');
        await expect(app.id(T.statusMath)).toHaveAttribute('data-state', 'ready');
        // The routed indexer reports SOL/USD 150 (a real one reads the oracle, stale on localnet).
        if (indexerMock) await expect(app.id(T.statusSolUsd)).toContainText('150.00');
        // The live feed connects once a page subscribes a channel (idle on pages that do not).
        await expect(app.id(T.statusWs)).toHaveAttribute('data-state', /^(idle|connecting|open)$/);
        await expect(app.id(T.fallbackBanner)).toHaveCount(0);
    });

    test('search by mint and by ticker', async ({ app, page, state, indexerMock }) => {
        const main = state.markets.main;
        await app.goto('/markets', 'markets');
        // "/" focuses the search from anywhere outside a text field.
        await page.locator('body').click({ position: { x: 5, y: 300 } });
        await page.keyboard.press('/');
        await expect(app.id(T.searchInput)).toBeFocused();
        await page.keyboard.type(main.mint);
        await expect(app.id(T.searchResult(main.mint))).toBeVisible();
        await page.keyboard.press('Enter');
        await expect(page).toHaveURL(new RegExp(`/m/${main.mint}`));
        await expect(app.id(T.page('market'))).toBeVisible();

        // By ticker: the routed indexer labels the seeded mints. A real indexer (profile `full`)
        // has symbols only once its metadata refresher has run (INDEXER IX-16; FRONTEND FE-12).
        if (indexerMock) {
            await app.id(T.searchInput).fill(main.symbol.slice(0, 5));
            await expect(app.id(T.searchResults)).toBeVisible();
            await app.id(T.searchResult(main.mint)).click();
            await expect(page).toHaveURL(new RegExp(`/m/${main.mint}`));
        } else {
            test.info().annotations.push({ type: 'skipped-step', description: 'search by ticker (profile full)' });
        }

        // A valid mint the indexer does not know still offers "Open mint".
        await app.id(T.searchInput).fill(state.createMints.ok);
        await expect(app.id(T.searchResult(state.createMints.ok))).toBeVisible();
    });

    test('wallet connect, silent reconnect after a reload, disconnect', async ({ app, page }) => {
        await app.goto('/markets', 'markets');
        await app.id(T.walletButton).click();
        await expect(app.id(T.walletMenu)).toBeVisible();
        await expect(app.id(T.walletOption(E2E_WALLET_NAME))).toBeVisible();
        await app.id(T.walletOption(E2E_WALLET_NAME)).click();
        await expect(app.id(T.walletAddress)).toHaveAttribute('data-value', app.walletAddress);
        expect(await page.evaluate(() => window.__shortwinE2E?.connected())).toBe(true);

        await page.reload();
        await expect(app.id(T.walletAddress)).toHaveAttribute('data-value', app.walletAddress);

        await app.disconnectWallet();
        await page.reload();
        await expect(app.id(T.header)).toBeVisible();
        await expect(app.id(T.walletAddress)).toHaveCount(0);
    });

    test('axe on every route', async ({ app, state }) => {
        const pages = [
            ['/markets', 'markets'],
            ['/markets/new', 'create-market'],
            [`/m/${state.markets.main.mint}`, 'market'],
            ['/positions', 'positions'],
            ['/lend', 'lend'],
            ['/status', 'status'],
        ] as const;
        for (const [path, root] of pages) {
            await app.goto(path, root);
            await app.a11y(path);
        }
    });
});
