/**
 * mobile-lend — lending on a phone (M10-RESP; FRONTEND.md §3.6, §3.8, §7.6, §10.2): project
 * `<profile>-phone` (390 × 844, touch, DPR 3, an iPhone user agent). Every step is a real tap
 * (`locator.tap()`, no DOM-click fallback) and axe (WCAG 2.2 A/AA) runs at 0 on every state visited.
 *
 * 1. The market page's Lend (action bar) opens the trade sheet on Lend → Deposit (`?sheet=trade`).
 *    The form foot says why the review cannot open yet; its reason focuses the SPEC §5.3
 *    acknowledgement; ticked, the deposit is reviewed and signed with Sign on screen without
 *    scrolling; Escape on the confirmed review closes it and the sheet (as Done does). Withdraw: a
 *    chip fills a share of the holding and the "≈ tokens" line follows.
 * 2. The lender dashboard shows the position as a card with Withdraw / Deposit links into the
 *    market's trade sheet; Claim says why it is disabled and "Claim all" is not offered while
 *    nothing is claimable (or, when another lender's sync can fund a shortfall, the row's Claim is
 *    the panel's, on two lines, and "Claim all" counts it), once the row's search has ended.
 * 3. With SOL claimable (rent paid in), "Claim all" sits in the action bar, on the first screen
 *    (the `claimer` wallet, read only: nothing is signed with it here).
 * 4. Without a wallet: the dashboard's and the panel's Connect wallet.
 *
 * Signs one deposit, with `phone` (the phone project's own wallet). Writes PNGs (CSS scale) to
 * `.e2e/screenshots/phone/`, or to the reviewed set `e2e/screenshots/phone/` with
 * `SHORTWIN_E2E_SCREENSHOTS=update`.
 */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import type { Locator, Page } from '@playwright/test';
import { E2E_DIR, WEB_DIR } from '../env.ts';
import { type App, E2E_WALLET_NAME, expect, need, requireTrading, T, test } from '../fixtures.ts';
import { payRentIn } from '../validator/actions.ts';

const DIR = join(
    process.env.SHORTWIN_E2E_SCREENSHOTS === 'update'
        ? join(WEB_DIR, 'e2e', 'screenshots')
        : join(E2E_DIR, 'screenshots'),
    'phone',
);
mkdirSync(DIR, { recursive: true });

/** The first screen, at CSS scale (what the user sees). */
async function shot(page: Page, name: string): Promise<void> {
    await expect(page.locator('.skeleton:visible')).toHaveCount(0, { timeout: 20_000 });
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

/** The element lies wholly inside the viewport (on the first screen, no scrolling). */
async function expectOnScreen(page: Page, el: Locator): Promise<void> {
    const box = need(await el.boundingBox(), 'a bounding box');
    const vp = need(page.viewportSize(), 'a viewport');
    expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.y + box.height).toBeLessThanOrEqual(vp.height);
}

test.describe('phone lending', { tag: '@lend' }, () => {
    test.describe.configure({ mode: 'serial' });

    test.describe('the phone wallet', () => {
        test.use({ wallet: 'phone' });

        test('Lend opens the trade sheet; a deposit is acknowledged, reviewed and signed', async ({
            app,
            page,
            state,
        }) => {
            const main = state.markets.main;
            // Connected on the lender page; the market page reconnects the authorised wallet.
            await app.goto('/lend', 'lend');
            await tapConnect(app);
            await app.goto(`/m/${main.mint}`, 'market');
            await expect(app.id(T.walletAddress)).toHaveAttribute('data-value', app.walletAddress);
            const panel = app.id(T.lendPanel);
            await expect(panel).toBeHidden();
            await app.id(T.marketActionLend).tap();
            await expect(page).toHaveURL(/[?&]sheet=trade/);
            await expect(page).toHaveURL(/[?&]panel=lend/);
            await expect(panel).toBeVisible();
            await expect(panel).toHaveAttribute('data-action', 'deposit');

            // The form foot says what is missing, first unmet first.
            const reason = app.id(T.lendDepositReason);
            await expect(reason).toContainText('Enter an amount');
            await expect(app.id(T.lendSubmit)).toBeDisabled();
            await app.a11y('phone lend sheet: deposit');
            await shot(page, 'trade-sheet-lend');

            const amount = panel.getByTestId(T.lendAmountInput);
            await amount.tap();
            await amount.fill('1000');
            await expect(reason).toContainText('Tick the acknowledgement above');
            // The reason is a button that brings the acknowledgement into view and focuses it.
            await reason.getByRole('button').tap();
            const ack = panel.getByTestId(T.disclosureAck('lender.solRepayment'));
            await expect(ack).toBeFocused();
            await expect(ack).toBeInViewport();
            await ack.tap();
            await expect(ack).toBeChecked();
            await expect(reason).toHaveText('');
            const submit = app.id(T.lendSubmit);
            await expect(submit).toBeEnabled();
            await expectOnScreen(page, submit);
            await app.a11y('phone lend sheet: deposit ready');
            await shot(page, 'trade-sheet-lend-ready');

            await submit.tap();
            expect(await app.simulated()).toBe('review');
            await app.inModal(T.disclosureAck('lender.solRepayment')).tap();
            const sign = app.inModal(T.txSign);
            await expect(sign).toBeEnabled();
            await expectOnScreen(page, sign);
            await app.a11y('phone deposit review');
            await shot(page, 'review-sheet-deposit');
            await sign.tap();
            await app.expectStage('confirmed', 90_000);
            // Escape closes the confirmed review and the sheet, as Done does (mobile-trade taps Done):
            // back on the market page, the toast whole.
            await expect(app.modal.getByRole('button', { name: 'Done' })).toBeVisible();
            await page.keyboard.press('Escape');
            await expect(app.modal).toHaveCount(0);
            await expect(page).not.toHaveURL(/[?&]sheet=trade/);
            await expect(panel).toBeHidden();
            await expect(app.id(T.marketActionLend)).toBeFocused();
            const toast = app.id(T.toast).first();
            await expect(toast).toBeInViewport();
            await expect(toast).toContainText('Confirmed');
            const toastLinks = toast.locator('.toast-links a');
            if ((await toastLinks.count()) > 0) await expect(toastLinks.first()).toBeVisible();

            // Withdraw: a chip fills a share of the holding; the "≈ tokens" line follows.
            await app.id(T.marketActionLend).tap();
            await expect(page).toHaveURL(/[?&]sheet=trade/);
            await expect(panel).toBeVisible();
            await panel.getByTestId(T.lendTab('withdraw')).tap();
            await expect(panel).toHaveAttribute('data-action', 'withdraw');
            const half = panel.getByTestId(T.withdrawPercent('50'));
            await half.tap();
            await expect(half).toHaveAttribute('aria-pressed', 'true');
            await expect(panel.getByTestId(T.withdrawSharesInput)).not.toHaveValue('');
            await expect(panel.getByTestId(T.withdrawApprox)).toContainText('≈');
            // In the full run another spec's one-off income (lend-claim) still vests in this pool:
            // leaving forfeits this lender's part, which takes its acknowledgement first. The reason
            // line says so and its button focuses the box.
            const withdrawReason = app.id(T.lendWithdrawReason);
            await expect(withdrawReason).toHaveText(/^$|Tick the acknowledgement/);
            if ((await withdrawReason.textContent())?.includes('Tick the acknowledgement')) {
                await withdrawReason.getByRole('button').tap();
                const forfeitAck = panel.getByTestId(T.disclosureAck('lender.exitForfeitsVest'));
                await expect(forfeitAck).toBeFocused();
                await forfeitAck.tap();
                await expect(forfeitAck).toBeChecked();
                await expect(withdrawReason).toHaveText('');
            }
            await expect(app.id(T.withdrawSubmit)).toBeEnabled();
            // axe judges a target by its visible part: bring the destination select out from under
            // the sticky form foot first (a scroll position, not a size: the select is 44 px).
            await panel.getByTestId(T.withdrawDestination).evaluate((el) => el.scrollIntoView({ block: 'center' }));
            await app.a11y('phone lend sheet: withdraw');
            await shot(page, 'trade-sheet-lend-withdraw');
        });

        test('the lender dashboard: a card with links into the trade sheet; no Claim all with nothing to claim', async ({
            app,
            page,
            state,
        }) => {
            const main = state.markets.main;
            await app.goto('/lend', 'lend');
            await tapConnect(app);
            const row = app.id(T.lenderRow(main.market));
            await expect(row).toBeVisible();
            // A card: the table's header leaves the screen (not the accessibility tree).
            await expect(app.id(T.lenderDashboard).locator('thead')).toHaveCSS('position', 'absolute');

            const withdraw = row.getByTestId(T.lenderWithdraw(main.market));
            const deposit = row.getByTestId(T.lenderDeposit(main.market));
            await expect(withdraw).toHaveAttribute('href', /[?&]sheet=trade/);
            await expect(withdraw).toHaveAttribute('href', /[?&]action=withdraw/);
            await expect(deposit).toHaveAttribute('href', /[?&]action=deposit/);

            const claimable = BigInt(
                need(await app.id(T.lenderKpi('claimable')).getAttribute('data-value'), 'claimable'),
            );
            // Nothing claimable now: the row's Claim is off — unless another lender's sync can fund
            // 10,000 lamports or more of what is credited, when it is the panel's claim, on two lines
            // (the owner's decisions of 2026-10-11, FRONTEND.md §3.8).
            const claim = row.getByTestId(T.lenderClaim(main.market));
            // The row's search for syncs has ended (owner, 2026-10-11: only when syncs can pay).
            await expect(claim).not.toHaveAttribute('aria-busy', 'true', { timeout: 30_000 });
            if (claimable === 0n && (await row.getByTestId(T.claimSubmitSyncs).count()) === 0) {
                await expect(app.id(T.lenderClaimAll)).toHaveCount(0);
                await expect(row.getByTestId(T.lenderClaimReason(main.market))).toHaveText('Nothing to claim yet');
                await expect(claim).toBeDisabled();
                await expect(claim).toHaveText('Review claim · 0 SOL');
            } else if (claimable === 0n) {
                await expect(claim).toBeEnabled();
                await expect(claim).toContainText('Review claim · 0 SOL now');
                await expect(app.id(T.lenderClaimAll)).toBeVisible();
            }
            await app.a11y('phone lender dashboard');
            await shot(page, 'lend-dashboard');

            // A row's Withdraw opens the market's trade sheet on Lend → Withdraw.
            await withdraw.tap();
            await expect(page).toHaveURL(new RegExp(`/m/${main.mint}`));
            await expect(page).toHaveURL(/[?&]sheet=trade/);
            await expect(app.id(T.lendPanel)).toBeVisible();
            await expect(app.id(T.lendPanel)).toHaveAttribute('data-action', 'withdraw');
        });
    });

    test.describe('the claimer wallet (read only)', () => {
        test.use({ wallet: 'claimer' });

        test('with SOL claimable, Claim all is in the action bar on the first screen', async ({ app, page, state }) => {
            requireTrading(state);
            const main = state.markets.main;
            await payRentIn(main, 200_000n * 10n ** BigInt(main.decimals));
            await app.goto('/lend', 'lend');
            await tapConnect(app);
            await expect(app.id(T.lenderRow(main.market))).toBeVisible();
            await expect(app.id(T.lenderKpi('claimable'))).toHaveAttribute('data-value', /^[1-9]\d*$/, {
                timeout: 90_000,
            });
            const all = app.id(T.lenderClaimAll);
            await expect(all).toBeVisible();
            await expect(all).toBeEnabled();
            await expect(page.locator('.action-bar').getByTestId(T.lenderClaimAll)).toHaveCount(1);
            await expectOnScreen(page, all);
            await app.a11y('phone lender dashboard with Claim all');
            await shot(page, 'lend-dashboard-claim-all');
        });
    });

    test.describe('no wallet', () => {
        test.use({ wallet: null });

        test('the dashboard and the lend sheet offer Connect wallet', async ({ app, page, state }) => {
            await app.goto('/lend', 'lend');
            const cta = app.id(T.connectCta('lend-dashboard'));
            await expect(cta).toBeVisible();
            await app.a11y('phone lender dashboard, no wallet');
            await shot(page, 'lend-dashboard-no-wallet');

            await app.goto(`/m/${state.markets.main.mint}?panel=lend&action=deposit&sheet=trade`, 'market');
            await expect(app.id(T.lendPanel)).toBeVisible();
            await expect(app.id(T.connectCta('lend'))).toBeVisible();
            await expect(app.id(T.lendDepositReason)).toContainText('Connect a wallet first');
            await expect(app.id(T.lendSubmit)).toBeDisabled();
            await app.a11y('phone lend sheet, no wallet');
            await shot(page, 'trade-sheet-lend-no-wallet');
        });
    });
});
