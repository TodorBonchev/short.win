/**
 * status (FRONTEND.md §3.9 protocol status; O17 degraded mode). NEEDS the MARKETS track's protocol
 * status page: `@markets`.
 *
 * The coverage-paused market is listed among the degraded markets; the venue programs' rows carry
 * the latch, enabled and route-broken state; the global pause and the indexer's health are shown.
 */
import { expect, T, test } from '../fixtures.ts';

test.use({ wallet: null });

const VENUE_ROWS = ['pump', 'pump_amm', 'pump_fees', 'raydium_v4', 'raydium_cpmm'] as const;

test.describe('protocol status', { tag: '@markets' }, () => {
    test('coverage-paused market listed, venue rows, global pause, indexer health', async ({ app, state }) => {
        await app.goto('/status', 'status');
        await expect(app.id(T.statusPage)).toBeVisible();
        await expect(app.id(T.statusPausedOpens)).toHaveAttribute('data-value', 'false');
        await expect(app.id(T.statusIndexerLag)).toHaveAttribute('data-state', 'live');

        await expect(app.id(T.statusDegraded)).toBeVisible();
        await expect(app.id(T.statusDegradedRow(state.markets.paused.mint))).toBeVisible();
        await expect(app.id(T.statusDegradedRow(state.markets.main.mint))).toHaveCount(0);

        await expect(app.id(T.statusVenues)).toBeVisible();
        for (const name of VENUE_ROWS) {
            const row = app.id(T.statusVenueRow(name));
            await expect(row, name).toBeVisible();
            // No venue was upgraded since initialize_config recorded the live ProgramData slots.
            await expect(row).toHaveAttribute('data-upgraded', 'false');
            await expect(row).toHaveAttribute('data-enabled', /^(true|false)$/);
            await expect(row).toHaveAttribute('data-route-broken', 'false');
        }
        await expect(app.id(T.statusVenueRow('raydium_v4'))).toHaveAttribute('data-enabled', 'true');
        await expect(app.id(T.statusRouteBroken)).toBeVisible();
        await app.a11y('protocol status');
    });
});
