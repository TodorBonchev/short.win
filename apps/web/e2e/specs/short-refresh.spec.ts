/** Two explicit wallet transactions recover a stale, positionless market across actual chain seconds. */
import { COVERAGE_GAP_S } from '../../src/venues/constants.ts';
import { expect, requireTrading, T, test } from '../fixtures.ts';
import { marketAccount, setCoveragePause } from '../validator/actions.ts';
import { clock, waitForClock } from '../validator/chain.ts';

test.use({ wallet: 'trader' });

test('short: explicitly record and integrate an idle price before opening', { tag: '@short' }, async ({
    app,
    state,
}) => {
    requireTrading(state);
    // This seeded market has liquidity and no positions. Restore its coverage flag for other specs.
    const m = state.markets.paused;
    expect((await marketAccount(m)).borrowedQ).toBe(0n);
    await setCoveragePause(m, false);
    try {
        const before = await marketAccount(m);
        const clearReady = before.coverageClearedAt + 5n;
        const staleAt = before.emaTs + COVERAGE_GAP_S + 1n;
        await waitForClock(clearReady > staleAt ? clearReady : staleAt);
        await app.goto(`/m/${m.mint}?panel=short`, 'market');
        await app.connectWallet();
        const panel = app.id(T.shortPanel);
        await expect(panel.getByTestId(T.shortPriceRefreshStep)).toHaveAttribute('data-step', 'record');
        await expect(panel.getByTestId(T.shortSubmit)).toBeDisabled();
        const startClock = await clock();

        await panel.getByTestId(T.shortPriceRefresh).click();
        expect(await app.simulated()).toBe('review');
        await expect(app.modal).toContainText('Record price observation');
        await expect(app.modal).toContainText('sign each separately');
        await app.signAndConfirm();
        const recorded = await marketAccount(m);
        expect(recorded.bSec).toBeGreaterThanOrEqual(startClock.unixTimestamp);
        expect(recorded.lbSec).toBeLessThan(recorded.bSec);
        await app.modal.getByRole('button', { name: 'Done', exact: true }).click();

        // Reload proves the second step comes from confirmed chain state, not an in-memory wizard.
        await app.goto(`/m/${m.mint}?panel=short`, 'market');
        await expect(panel.getByTestId(T.shortPriceRefreshStep)).toHaveAttribute('data-step', 'integrate');
        await expect(panel.getByTestId(T.shortSubmit)).toBeDisabled();
        await panel.getByTestId(T.shortPriceRefresh).click();
        expect(await app.simulated()).toBe('review');
        await expect(app.modal).toContainText('Update smoothed price');
        await app.signAndConfirm();
        const integrated = await marketAccount(m);
        expect(integrated.lbSec).toBe(recorded.bSec);
        expect(integrated.emaTs).toBeGreaterThan(recorded.bSec);
        expect(integrated.borrowedQ).toBe(0n);
        await app.modal.getByRole('button', { name: 'Done', exact: true }).click();
        await expect(panel.getByTestId(T.shortPriceRefresh)).toHaveCount(0);
        await expect(panel.getByTestId(T.shortBlocked)).toHaveCount(0);
        await app.a11y('short panel after explicit price refresh');
    } finally {
        await setCoveragePause(m, true);
    }
});
