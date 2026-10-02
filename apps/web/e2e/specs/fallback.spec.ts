/**
 * fallback — the indexer is unreachable (FRONTEND.md §6.5, §6.6; FE-D1). NEEDS the MARKETS track's
 * market page (rendered from the chain) and the LEND track's deposit and dashboard: `@markets @lend`.
 *
 * Every indexer request fails (connection refused) and its WebSocket closes: the shell shows the
 * fallback banner and `status-indexer` `down`; the market page renders from RPC; a deposit still
 * simulates, signs and lands; the lender dashboard lists the position from `getProgramAccounts`.
 */

import { expect, need, T, test } from '../fixtures.ts';
import { lenderPosition } from '../validator/actions.ts';
import { getTokenAmount } from '../validator/chain.ts';

test.use({ wallet: 'lender', indexer: 'down' });

test.describe('indexer down: RPC fallback', { tag: ['@markets', '@lend'] }, () => {
    test('banner, market page from the chain, a deposit still works, the dashboard from RPC', async ({
        app,
        state,
    }) => {
        const m = state.markets.main;
        const source = need(state.tokenAccounts.mainAta.lender, 'lender main ATA');
        await app.goto(`/m/${m.mint}?panel=lend&action=deposit`, 'market');
        await expect(app.id(T.fallbackBanner)).toBeVisible();
        await expect(app.id(T.statusIndexer)).toHaveAttribute('data-state', 'down');
        await expect(app.id(T.marketHeader)).toBeVisible();
        await expect(app.id(T.marketMint)).toHaveAttribute('data-value', m.mint);
        await expect(app.id(T.marketVenue)).toBeVisible();
        await app.a11y('market page from the chain');

        await app.connectWallet();
        const panel = app.id(T.lendPanel);
        await app.selectTokenAccount(panel.getByTestId(T.lendTokenAccount), source);
        await panel.getByTestId(T.lendAmountInput).fill('100');
        await panel.getByTestId(T.disclosureAck('lender.solRepayment')).check();
        await panel.getByTestId(T.lendSubmit).click();
        expect(await app.simulated()).toBe('review');
        await app.ackInReview('lender.solRepayment');
        const before = need(await getTokenAmount(source), 'source balance');
        await app.signAndConfirm();
        expect(await getTokenAmount(source)).toBe(before - 100n * 10n ** BigInt(m.decimals));
        await app.closeModal();

        const lp = need(await lenderPosition(m.market, app.walletAddress), 'lender position');
        await app.goto('/lend', 'lend');
        await expect(app.id(T.fallbackBanner)).toBeVisible();
        const row = app.id(T.lenderRow(m.market));
        await expect(row).toBeVisible();
        await expect(row.getByTestId(T.lenderCell('shares'))).toHaveAttribute('data-value', lp.shares.toString());
    });
});
