/**
 * tx-errors — the transaction pipeline's failure paths (FRONTEND.md §5.3–§5.9, §10.2; FE-2, FE-D2,
 * FE-D3), driven through the deposit flow. NEEDS the LEND track's lend panel: `@lend`.
 *
 * - a wallet rejection maps to `wallet-rejected` (Retry, not an error banner) and nothing lands;
 * - a failing simulation never enables Sign and shows the mapped error with its `data-code`;
 * - a review older than 20 s is re-simulated at Sign and shown again — the wallet is not asked;
 * - a wallet that changes an instruction (`tamperNext`) is refused: nothing is sent;
 * - a wallet that only raises the priority fee (`raiseFeeNext`) is accepted and lands with it.
 */
import type { Locator } from '@playwright/test';
import { type App, dataValue, expect, need, T, test } from '../fixtures.ts';
import { landedTransaction, setCpiGuard, signatureKnown } from '../validator/actions.ts';
import { getTokenAmount } from '../validator/chain.ts';
import type { MarketInfo } from '../validator/state.ts';

test.use({ wallet: 'errors' });

/** Opens the deposit panel of `m`, fills `amount` from `source`, ticks SPEC §5.3 and submits. */
async function depositReview(app: App, m: MarketInfo, source: string, amount: string): Promise<Locator> {
    await app.goto(`/m/${m.mint}?panel=lend&action=deposit`, 'market');
    await app.connectWallet();
    const panel = app.id(T.lendPanel);
    await app.selectTokenAccount(panel.getByTestId(T.lendTokenAccount), source as never);
    await panel.getByTestId(T.lendAmountInput).fill(amount);
    await panel.getByTestId(T.disclosureAck('lender.solRepayment')).check();
    await panel.getByTestId(T.lendSubmit).click();
    return panel;
}

test.describe('transaction errors', { tag: '@lend' }, () => {
    test('a wallet rejection maps to wallet-rejected; nothing lands; Retry re-simulates', async ({ app, state }) => {
        const m = state.markets.main;
        const source = need(state.tokenAccounts.mainAta.errors, 'errors main ATA');
        const before = await getTokenAmount(source);
        await depositReview(app, m, source, '5');
        expect(await app.simulated()).toBe('review');
        await app.ackInReview('lender.solRepayment');
        await app.walletRejectNext();
        await app.inModal(T.txSign).click();
        await app.expectStage('failed');
        await expect(app.inModal(T.txError)).toHaveAttribute('data-kind', 'wallet-rejected');
        expect(await getTokenAmount(source)).toBe(before);
        expect(await app.walletSigned()).toEqual([]);
        await app.inModal(T.txRetry).click();
        expect(await app.simulated()).toBe('review');
        await app.closeModal();
    });

    test('a failing simulation never enables Sign; the mapped error carries its code', async ({ app, state }) => {
        const t22 = state.markets.t22;
        const { race } = state.tokenAccounts.errors;
        await setCpiGuard('errors', race, false);
        try {
            await depositReview(app, t22, race, '5');
            expect(await app.simulated()).toBe('review');
            // The source turns CPI-Guarded after the review: Token-2022 refuses the program's transfer.
            await setCpiGuard('errors', race, true);
            await app.inModal(T.txRetry).click();
            expect(await app.simulated()).toBe('failed');
            await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'false');
            const err = app.inModal(T.txError);
            await expect(err).toHaveAttribute('data-kind', 'token');
            await expect(err).toHaveAttribute('data-code', '42');
            await app.ackInReview('lender.solRepayment');
            await expect(app.inModal(T.txSign)).toBeDisabled();
            // Retry simulates again and still fails: Sign never becomes enabled.
            await app.inModal(T.txRetry).click();
            expect(await app.simulated()).toBe('failed');
            await expect(app.inModal(T.txSign)).toBeDisabled();
            expect(await app.walletSigned()).toEqual([]);
            await app.a11y('review with a failed simulation', `[data-testid="${T.txModal}"]`);
            await app.closeModal();
        } finally {
            await setCpiGuard('errors', race, false);
        }
    });

    test('a review older than 20 s is re-simulated at Sign and shown again before the wallet is asked', async ({
        app,
        page,
        state,
    }) => {
        const m = state.markets.main;
        const source = need(state.tokenAccounts.mainAta.errors, 'errors main ATA');
        await depositReview(app, m, source, '5');
        expect(await app.simulated()).toBe('review');
        await app.ackInReview('lender.solRepayment');
        await page.waitForTimeout(21_000);
        await app.inModal(T.txSign).click();
        // STALE_REVIEW_MS: back to simulating, then the review with fresh numbers — not the wallet.
        expect(await app.simulated()).toBe('review');
        expect(await app.walletSigned()).toEqual([]);
        await expect(app.inModal(T.txSign)).toBeEnabled();
        const before = need(await getTokenAmount(source), 'source balance');
        await app.signAndConfirm();
        expect(await getTokenAmount(source)).toBe(before - 5n * 10n ** BigInt(m.decimals));
        expect(await app.walletSigned()).toHaveLength(1);
    });

    test('a wallet that changes an instruction is refused and nothing is sent', async ({ app, state }) => {
        const m = state.markets.main;
        const source = need(state.tokenAccounts.mainAta.errors, 'errors main ATA');
        const before = await getTokenAmount(source);
        await depositReview(app, m, source, '5');
        expect(await app.simulated()).toBe('review');
        await app.ackInReview('lender.solRepayment');
        await app.walletTamperNext('data');
        await app.inModal(T.txSign).click();
        await app.expectStage('failed');
        // §5.8: an integrity refusal (TxErrorKind `integrity`).
        await expect(app.inModal(T.txError)).toHaveAttribute('data-kind', 'integrity');
        const signed = await app.walletSigned();
        expect(signed).toHaveLength(1);
        expect(signed[0]?.tampered).toBe(true);
        expect(await signatureKnown(need(signed[0], 'signed record').signature)).toBe(false);
        expect(await getTokenAmount(source)).toBe(before);
    });

    test('a wallet that only raises the priority fee is accepted (FE-2) and lands with it', async ({ app, state }) => {
        const m = state.markets.main;
        const source = need(state.tokenAccounts.mainAta.errors, 'errors main ATA');
        await depositReview(app, m, source, '5');
        expect(await app.simulated()).toBe('review');
        const limit = await dataValue(app.inModal(T.txCu));
        // M9-SEC-8: the review names the wallet and the account that will sign.
        await expect(app.inModal(T.txWallet)).toHaveAttribute('data-value', app.walletAddress);
        await expect(app.inModal(T.txWallet)).toHaveAttribute('data-sends-itself', 'false');
        await app.ackInReview('lender.solRepayment');
        await app.walletRaiseFeeNext(50_000);
        const signature = await app.signAndConfirm();
        const landed = need(await landedTransaction(signature), 'landed deposit');
        expect(landed.err).toBeNull();
        expect(landed.computeUnitPrice).toBe(50_000n);
        expect(BigInt(landed.computeUnitLimit ?? 0)).toBe(limit);
        const fee = 5_000n + (limit * 50_000n + 999_999n) / 1_000_000n;
        expect(landed.fee).toBe(fee);
        // FE-2 / M9-SEC-4: the review shows the fee as the wallet signed it.
        await expect(app.inModal(T.txFinalFee)).toHaveAttribute('data-value', fee.toString());
        const signed = await app.walletSigned();
        expect(signed.at(-1)?.raisedFeeTo).toBe('50000');
    });
});
