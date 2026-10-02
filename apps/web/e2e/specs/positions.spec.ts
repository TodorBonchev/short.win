/**
 * positions (FRONTEND.md §3.8 positions dashboard; SPEC §8). NEEDS the SHORT track's positions
 * page: `@short`.
 *
 * Wallet `holder` has a seeded open position on the `move` market: its row shows every field of
 * §3.8 with raw values that match the chain; after a large price move (from Node) the CR falls and
 * the risk badge changes; the time left counts down to the deadline. The price is moved back
 * afterwards (the `move` market is shared with short-close).
 */

import { dataValue, expect, need, requireTrading, T, test } from '../fixtures.ts';
import { movePrice, shortPosition } from '../validator/actions.ts';

test.use({ wallet: 'holder' });

const CELLS = ['market', 'q', 'vault', 'cr', 'liq-price', 'pnl', 'rent', 'time-left', 'risk', 'status'] as const;
/** `riskLevel` of src/components/Badge.tsx (§3.8 words it safe / watch / at risk / liquidatable). */
// `below`: the live leg alone is below the liquidation CR (M9-UX-4).
const RISKS = /^(safe|watch|danger|below|critical)$/;

test.describe('positions dashboard', { tag: '@short' }, () => {
    test('row fields, the risk badge after a price move, time left', async ({ app, state }) => {
        requireTrading(state);
        const pos = need(state.positions, 'seeded positions').holder;
        const chainPos = need(await shortPosition(pos.position), 'holder position');
        await app.goto('/positions', 'positions');
        await app.connectWallet();
        await expect(app.id(T.positionsTable)).toBeVisible();
        await expect(app.id(T.positionsFilter)).toBeVisible();
        const row = app.id(T.positionRow(pos.position));
        await expect(row).toHaveAttribute('data-status', 'open');
        for (const c of CELLS) await expect(row.getByTestId(T.positionCell(c)), c).toBeVisible();
        const cell = (c: (typeof CELLS)[number]) => row.getByTestId(T.positionCell(c));
        expect(await dataValue(cell('q'))).toBe(chainPos.q);
        expect(await dataValue(cell('vault'))).toBe(chainPos.v);
        await expect(cell('cr')).toHaveAttribute('data-value', /^[1-9]\d*$/);
        await expect(cell('liq-price')).toHaveAttribute('data-value', /^[1-9]\d*$/);
        await expect(cell('pnl')).toHaveAttribute('data-value', /^-?\d+$/);
        await expect(cell('rent')).toHaveAttribute('data-value', /^\d+$/);
        await expect(cell('risk')).toHaveAttribute('data-risk', RISKS);
        const left = await dataValue(cell('time-left'));
        expect(left).toBeGreaterThan(0n);
        expect(left).toBeLessThanOrEqual(chainPos.deadline - chainPos.tOpen);
        // The countdown moves.
        await expect.poll(async () => dataValue(cell('time-left')), { timeout: 15_000 }).toBeLessThan(left);
        await app.a11y('positions dashboard');

        const crBefore = await dataValue(cell('cr'));
        const riskBefore = await cell('risk').getAttribute('data-risk');
        await movePrice(state.markets.move, 8_000);
        try {
            await expect.poll(async () => dataValue(cell('cr')), { timeout: 30_000 }).toBeLessThan(crBefore);
            await expect(cell('risk')).not.toHaveAttribute('data-risk', riskBefore ?? '', { timeout: 30_000 });
            await expect(cell('risk')).toHaveAttribute('data-risk', RISKS);
        } finally {
            // Back to about where it was: P·1.8 then ·(1 − 4,444/10⁴) ≈ P.
            await movePrice(state.markets.move, -4_444);
        }

        // The row's actions (§11.4: "row / detail action").
        await expect(row.getByTestId(T.positionAddMargin)).toBeVisible();
        await expect(row.getByTestId(T.positionClose)).toBeVisible();
        await app.goto(`/positions/${pos.position}`, 'position');
        await expect(app.id(T.positionDetail)).toHaveAttribute('data-value', pos.position);
    });
});
