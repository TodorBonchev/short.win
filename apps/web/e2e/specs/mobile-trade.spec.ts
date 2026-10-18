/**
 * mobile-trade — opening, managing and reading a short on a phone (M10-RESP; FRONTEND.md §7.6,
 * §10.2): project `<profile>-phone` (390 × 844, touch, DPR 3, an iPhone user agent). Every step is
 * a real tap (`locator.tap()`, no DOM-click fallback); axe (WCAG 2.2 A/AA) runs at 0 on each state,
 * scoped to the TRADE track's own surfaces (the short panel, the review, the positions pages).
 *
 * 1. Short: the size, the reason line under the submit naming the acknowledgement (its button
 *    focuses the box), the review sheet with Sign on screen without scrolling, the open signed and
 *    confirmed, the success toast at the top of the screen. The panel opens from the market page's
 *    action bar (`market-action-short`) as the trade sheet (`?sheet=trade`).
 * 2. The new position: its card on /positions, the card's Add margin opening the position page
 *    with the Manage sheet at that form (focused heading), ✕ closing it in place, the action bar's
 *    Close opening it at Close and the browser's Back closing it.
 * 3. No wallet: the panel's foot offers Connect wallet over the disabled submit and says why.
 *
 * Writes the phone PNGs (CSS scale) to `.e2e/screenshots/phone/`, or to the reviewed set
 * `e2e/screenshots/phone/` with `SHORTWIN_E2E_SCREENSHOTS=update`. Signs one `open_short` as the
 * phone project's own wallet (`phone`).
 */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import type { Locator, Page } from '@playwright/test';
import { E2E_DIR, WEB_DIR } from '../env.ts';
import { type App, E2E_WALLET_NAME, expect, requireTrading, T, test } from '../fixtures.ts';
import { freshOpenPrice } from '../validator/actions.ts';

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
    const approved = await app.page.evaluate(
        (a) => window.localStorage.getItem('shortwin-e2e-wallet:authorized') === a,
        app.walletAddress,
    );
    if (approved) {
        await expect(address).toHaveAttribute('data-value', app.walletAddress, { timeout: 15_000 });
        return;
    }
    if (await address.isVisible()) return;
    await app.id(T.walletButton).tap();
    await expect(app.id(T.walletMenu)).toBeVisible();
    await app.id(T.walletOption(E2E_WALLET_NAME)).tap();
    await expect(address).toHaveAttribute('data-value', app.walletAddress);
}

/** Opens the Short panel through the market's action bar: the trade sheet (FRONTEND.md §3.4). */
async function openShortPanel(app: App): Promise<Locator> {
    await app.id(T.marketActionShort).tap();
    await expect(app.page).toHaveURL(/[?&]sheet=trade/);
    await expect(app.page.locator('.trade-panel')).toHaveAttribute('data-sheet', 'open');
    const panel = app.id(T.shortPanel);
    await expect(panel).toBeVisible();
    // The sheet slides in (`--t-sheet`): a tap while it moves is aimed at a moving box, and
    // Playwright then scrolls the sheet to reach it (the fields' labels under the sheet head).
    await app.page.evaluate(() =>
        Promise.all(
            document
                .getAnimations()
                .filter((a) => a.playState === 'running' && a.effect?.getTiming().iterations !== Infinity)
                .map((a) => a.finished.catch(() => undefined)),
        ).then(() => undefined),
    );
    return panel;
}

/** The element's top edge is in the upper part of the screen (the phone's toasts sit under the header). */
async function inUpperScreen(page: Page, l: Locator): Promise<void> {
    const box = await l.boundingBox();
    const h = page.viewportSize()?.height ?? 0;
    expect(box, 'element has a box').not.toBeNull();
    expect(box?.y ?? h).toBeLessThan(h / 3);
}

test.describe('phone: short', { tag: '@short' }, () => {
    test.use({ wallet: 'phone' });
    test.describe.configure({ timeout: 300_000 });

    test('open a short, then manage it from its card and its page', async ({ app, page, state }) => {
        requireTrading(state);
        const m = state.markets.main;
        // Connect on the positions page; the market page then reconnects the approved wallet.
        await app.goto('/positions', 'positions');
        await tapConnect(app);
        await freshOpenPrice(m);
        await app.goto(`/m/${m.mint}?panel=short`, 'market');
        await tapConnect(app);
        const panel = await openShortPanel(app);

        // 1. The size, then the reason line that points at the acknowledgement.
        const size = panel.getByTestId(T.shortQInput);
        await size.tap();
        await size.fill('250000');
        await expect(panel.getByTestId(T.shortPreview('margin'))).toHaveAttribute('data-value', /^\d+$/);
        const reason = panel.getByTestId(T.shortSubmitReason);
        await expect(reason).toHaveText('Tick “I understand…” above');
        const submit = panel.getByTestId(T.shortSubmit);
        await expect(submit).toBeDisabled();
        await expect(submit).toBeInViewport();
        await app.a11y('phone short panel', `[data-testid="${T.shortPanel}"]`);
        await shot(page, 'trade-sheet-short');
        await reason.getByRole('button').tap();
        const ack = panel.getByTestId(T.disclosureAck('borrower.liquidation'));
        await expect(ack).toBeFocused();
        await ack.tap();
        await expect(ack).toBeChecked();
        await expect(reason).toHaveText('');
        await expect(submit).toBeEnabled();

        // The review sheet: Sign on the first screen, gated by its own acknowledgement.
        await submit.tap();
        expect(await app.simulated()).toBe('review');
        await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'true');
        const sign = app.inModal(T.txSign);
        await expect(sign).toBeInViewport();
        await expect(sign).toBeDisabled();
        const position = (await app.deltaOfNew('q', [app.walletAddress, m.market])).address;
        await app.a11y('phone open review', `[data-testid="${T.txModal}"]`);
        await shot(page, 'review-sheet');
        await app.inModal(T.disclosureAck('borrower.liquidation')).tap();
        await expect(sign).toBeEnabled();
        await expect(sign).toBeInViewport();
        await sign.tap();
        await app.expectStage('confirmed', 90_000);
        const toast = app.id(T.toast).filter({ hasText: 'Open short' });
        await expect(toast).toBeVisible();
        await expect(toast).toContainText('Confirmed');
        // View position: in the confirmed review, and in the toast — which, over the open sheet, is
        // the one-line strip in the sheet's head (its links show once the sheet closes).
        await expect(app.inModal(T.txConfirmedLink)).toHaveAttribute('href', `/positions/${position}`);
        await expect(toast.locator('a', { hasText: 'View position' })).toHaveAttribute(
            'href',
            `/positions/${position}`,
        );
        // Done closes the review and the trade sheet with it (Back-or-replace, like ✕): the user is
        // on the market page, focus on the Short that opened the sheet, and the whole toast — its
        // View position link — at the top of the screen; its own button dismisses it.
        await app.modal.getByRole('button', { name: 'Done' }).tap();
        await expect(app.modal).toHaveCount(0);
        await expect(page).not.toHaveURL(/sheet=trade/);
        await expect(page.locator('.trade-panel')).not.toHaveAttribute('data-sheet', 'open');
        await expect(app.id(T.marketActionShort)).toBeFocused();
        await expect(toast).toBeVisible();
        await expect(toast.locator('a', { hasText: 'View position' })).toBeVisible();
        await inUpperScreen(page, toast);
        await shot(page, 'toast-short-open');
        await toast.getByRole('button', { name: 'Dismiss' }).tap();
        await expect(toast).toHaveCount(0);

        // 2. Its card on the positions page.
        await app.id(T.nav('positions')).tap();
        await expect(app.id(T.page('positions'))).toBeVisible();
        const card = app.id(T.positionRow(position));
        await expect(card).toBeVisible();
        await expect(app.id(T.positionsTotals)).toBeVisible();
        await expect(card.getByTestId(T.positionCell('risk'))).toBeVisible();
        await expect(card.getByTestId(T.positionCell('cr'))).toBeVisible();
        // A card, not a table row: one column wide, its cells stacked.
        const cardBox = await card.boundingBox();
        expect(cardBox?.width ?? 0).toBeLessThanOrEqual(390);
        await app.a11y('phone positions', `[data-testid="${T.page('positions')}"]`);
        await shot(page, 'positions');

        // The card's Add margin: the position page with the Manage sheet at that form.
        await card.getByTestId(T.positionAddMargin).tap();
        await expect(page).toHaveURL(new RegExp(`/positions/${position}\\?sheet=add-margin`));
        const manage = page.locator('.manage-panel');
        await expect(manage).toHaveAttribute('data-sheet', 'open');
        await expect(manage).toHaveAttribute('role', 'dialog');
        await expect(page.locator('#add-margin-title')).toBeFocused();
        await expect(app.id(T.addMarginInput)).toBeInViewport();
        await expect(app.id(T.addMarginReason)).toHaveText('Enter an amount of SOL');
        await app.a11y('phone manage sheet (add margin)', '.manage-panel');
        await shot(page, 'manage-sheet');

        // ✕ closes it in place (it was opened by a link, not by this page: the URL is replaced).
        await app.id(T.manageSheetClose).tap();
        await expect(manage).not.toHaveAttribute('data-sheet', 'open');
        await expect(page).toHaveURL(new RegExp(`/positions/${position}(#[\\w-]+)?$`));
        await expect(app.id(T.positionActionClose)).toBeInViewport();
        await app.a11y('phone position', `[data-testid="${T.page('position')}"]`);
        await shot(page, 'position');

        // The action bar's Close: the sheet at Close; the browser's Back closes it.
        await app.id(T.positionActionClose).tap();
        await expect(page).toHaveURL(/[?&]sheet=close/);
        await expect(manage).toHaveAttribute('data-sheet', 'open');
        await expect(page.locator('#close-title')).toBeFocused();
        await expect(app.id(T.closeSlippageInput)).toBeInViewport();
        await expect(app.id(T.closeSubmit)).toBeEnabled();
        await app.a11y('phone manage sheet (close)', '.manage-panel');
        await shot(page, 'manage-sheet-close');
        await page.goBack();
        await expect(page).toHaveURL(new RegExp(`/positions/${position}(#[\\w-]+)?$`));
        await expect(manage).not.toHaveAttribute('data-sheet', 'open');
        await expect(app.id(T.positionDetail)).toBeVisible();
    });
});

test.describe('phone: short without a wallet', { tag: '@short' }, () => {
    test.use({ wallet: null });

    test('the panel offers Connect wallet over the disabled submit', async ({ app, page, state }) => {
        requireTrading(state);
        await app.goto(`/m/${state.markets.main.mint}?panel=short`, 'market');
        const panel = await openShortPanel(app);
        const cta = panel.getByTestId(T.connectCta('short'));
        await expect(cta).toBeVisible();
        await expect(cta).toBeEnabled();
        await expect(panel.getByTestId(T.shortSubmit)).toBeDisabled();
        await expect(panel.getByTestId(T.shortSubmit)).toHaveText('Connect a wallet to short');
        await expect(panel.getByTestId(T.shortSubmitReason)).toHaveText('Connect a wallet first');
        await cta.tap();
        await expect(cta).toHaveAttribute('aria-expanded', 'true');
        await expect(panel.getByTestId(T.walletOpenIn('phantom'))).toBeVisible();
        await app.a11y('phone short panel without a wallet', `[data-testid="${T.shortPanel}"]`);
        await shot(page, 'trade-sheet-no-wallet');
    });

    test('positions without a wallet: Connect wallet, no empty table', async ({ app, page }) => {
        await app.goto('/positions', 'positions');
        await expect(app.id(T.connectCta('positions'))).toBeVisible();
        await expect(app.id(T.positionsTable)).toHaveCount(0);
        await app.a11y('phone positions without a wallet', `[data-testid="${T.page('positions')}"]`);
        await shot(page, 'positions-no-wallet');
    });
});
