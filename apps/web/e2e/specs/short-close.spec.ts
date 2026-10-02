/**
 * short-close (FRONTEND.md §3.8 position detail, §5.3, §10.2). NEEDS the SHORT track's position
 * detail page: `@short`.
 *
 * Wallet `closer` holds a seeded position on the `move` market. After a price move on the mock pool
 * (from Node: someone buys the token) the close preview's P&L falls; closing lands, the position
 * account is gone and the wallet's SOL changes by the simulated payout.
 */

import { dataValue, expect, need, requireTrading, T, test } from '../fixtures.ts';
import { movePrice, shortPosition } from '../validator/actions.ts';
import { getBalance } from '../validator/chain.ts';

test.use({ wallet: 'closer' });

test.describe('short: close', { tag: '@short' }, () => {
    test('P&L after a price move, close, payout delta', async ({ app, state }) => {
        requireTrading(state);
        const pos = need(state.positions, 'seeded positions').closer;
        const move = state.markets.move;
        await app.goto(`/positions/${pos.position}`, 'position');
        await app.connectWallet();
        await expect(app.id(T.positionDetail)).toHaveAttribute('data-value', pos.position);
        const pnl = app.id(T.closePreview('pnl'));
        await expect(pnl).toHaveAttribute('data-value', /^-?\d+$/);
        const pnlBefore = await dataValue(pnl);
        for (const f of ['buyback', 'rent', 'payout'] as const) {
            await expect(app.id(T.closePreview(f))).toHaveAttribute('data-value', /^\d+$/);
        }

        await movePrice(move, 300);
        // The preview follows the venue accounts (§3.7: recomputed on every new slot).
        await expect.poll(async () => dataValue(pnl), { timeout: 30_000 }).toBeLessThan(pnlBefore);
        const payout = await dataValue(app.id(T.closePreview('payout')));
        await expect(app.id(T.closeStraddleNotice)).toHaveCount(0);
        await app.a11y('position detail');

        await app.id(T.closeSubmit).click();
        expect(await app.simulated()).toBe('review');
        expect(await app.delta(pos.position, 'q')).toBe(-BigInt(pos.q));
        const simWallet = await app.delta(app.walletAddress, 'lamports');
        expect(simWallet).toBeGreaterThan(0n);
        const before = await getBalance(app.walletAddress);
        // Once the close lands the page shows the closed position and drops the close form with its
        // review: wait for the chain, not for the review's `confirmed` stage.
        await expect(app.inModal(T.txSign)).toBeEnabled();
        await app.inModal(T.txSign).click();
        await expect.poll(async () => shortPosition(pos.position), { timeout: 90_000 }).toBeNull();
        const actual = (await getBalance(app.walletAddress)) - before;
        // The payout plus the position's rent refund, less the fee; rent accrues for a few more seconds.
        const tolerance = 1_000_000n;
        expect(actual - simWallet).toBeLessThanOrEqual(tolerance);
        expect(simWallet - actual).toBeLessThanOrEqual(tolerance);
        expect(actual).toBeGreaterThanOrEqual(payout - tolerance - 10_000n);
    });
});
