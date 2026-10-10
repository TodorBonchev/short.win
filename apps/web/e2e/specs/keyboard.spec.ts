/**
 * keyboard — a deposit and an open completed with the keyboard only (FRONTEND.md §8, WCAG 2.2
 * 2.1.1). NEEDS the LEND and SHORT tracks' panels: `@lend @short`.
 *
 * No mouse: focus moves with Tab, checkboxes toggle with Space, buttons activate with Enter, the
 * wallet menu opens from the header button. The review dialog keeps focus inside itself.
 */

import { type App, E2E_WALLET_NAME, expect, need, requireTrading, T, test } from '../fixtures.ts';
import { getTokenAmount } from '../validator/chain.ts';

test.use({ wallet: 'keyboard' });

async function connectWithKeyboard(app: App): Promise<void> {
    await app.tabTo(T.walletButton);
    await app.page.keyboard.press('Enter');
    await expect(app.id(T.walletMenu)).toBeVisible();
    await app.tabTo(T.walletOption(E2E_WALLET_NAME));
    await app.page.keyboard.press('Enter');
    await expect(app.id(T.walletAddress)).toHaveAttribute('data-value', app.walletAddress);
    // Focus comes back to the header button after a pick, and after Escape (never to <body>).
    await expect(app.id(T.walletButton)).toBeFocused();
    await app.page.keyboard.press('Enter');
    await expect(app.id(T.walletMenu)).toBeVisible();
    await app.page.keyboard.press('Escape');
    await expect(app.id(T.walletMenu)).toHaveCount(0);
    await expect(app.id(T.walletButton)).toBeFocused();
}

/** Inside the review: tick `ack`, then Sign — Tab, Space, Tab, Enter. */
async function signWithKeyboard(app: App, ack: Parameters<typeof T.disclosureAck>[0]): Promise<void> {
    expect(await app.simulated()).toBe('review');
    await app.tabTo(T.disclosureAck(ack), { inModal: true });
    await app.page.keyboard.press('Space');
    await expect(app.inModal(T.disclosureAck(ack))).toBeChecked();
    await app.tabTo(T.txSign, { inModal: true });
    await app.page.keyboard.press('Enter');
    await app.expectStage('confirmed', 90_000);
}

test.describe('keyboard only', () => {
    test('a deposit', { tag: '@lend' }, async ({ app, state }) => {
        const m = state.markets.main;
        const source = need(state.tokenAccounts.mainAta.keyboard, 'keyboard main ATA');
        await app.goto(`/m/${m.mint}?panel=lend&action=deposit`, 'market');
        await connectWithKeyboard(app);
        await app.tabTo(T.lendAmountInput);
        await app.page.keyboard.type('25');
        await app.tabTo(T.disclosureAck('lender.solRepayment'));
        await app.page.keyboard.press('Space');
        await app.tabTo(T.lendSubmit);
        const before = need(await getTokenAmount(source), 'source balance');
        await app.page.keyboard.press('Enter');
        await signWithKeyboard(app, 'lender.solRepayment');
        expect(await getTokenAmount(source)).toBe(before - 25n * 10n ** BigInt(m.decimals));
        await app.page.keyboard.press('Escape');
        await expect(app.modal).toHaveCount(0);
    });

    test('an open', { tag: '@short' }, async ({ app, state }) => {
        requireTrading(state);
        const m = state.markets.main;
        await app.goto(`/m/${m.mint}?panel=short`, 'market');
        await connectWithKeyboard(app);
        await app.tabTo(T.shortQInput);
        await app.page.keyboard.type('200000');
        await expect(app.id(T.shortPreview('q'))).toHaveAttribute('data-value', /^[1-9]\d*$/);
        const ack = app.id(T.shortPanel).getByTestId(T.disclosureAck('borrower.liquidation'));
        if (await ack.count()) {
            await app.tabTo(T.disclosureAck('borrower.liquidation'));
            await app.page.keyboard.press('Space');
        }
        await app.tabTo(T.shortSubmit);
        await app.page.keyboard.press('Enter');
        await signWithKeyboard(app, 'borrower.liquidation');
        await expect(app.inModal(T.txSignature)).toHaveAttribute('data-value', /^[1-9A-HJ-NP-Za-km-z]{64,88}$/);
    });
});
