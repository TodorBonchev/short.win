/**
 * add-margin (FRONTEND.md §3.8 position detail, §5.3, §5.10). NEEDS the SHORT track's position
 * detail page: `@short`.
 *
 * Wallet `margin` holds a seeded position on `main`. Adding 0.5 SOL previews a higher CR; the
 * review shows the position's `m` and `v` rising by the amount; after confirmation the page's CR
 * matches the preview (within the rent accrued meanwhile).
 */

import { dataValue, expect, need, requireTrading, T, test } from '../fixtures.ts';
import { shortPosition } from '../validator/actions.ts';

test.use({ wallet: 'margin' });

const AMOUNT_SOL = '0.5';
const AMOUNT = 500_000_000n;

test.describe('short: add margin', { tag: '@short' }, () => {
    test('CR preview and the confirmed CR', async ({ app, state }) => {
        requireTrading(state);
        const pos = need(state.positions, 'seeded positions').margin;
        const before = need(await shortPosition(pos.position), 'margin position');
        await app.goto(`/positions/${pos.position}`, 'position');
        await app.connectWallet();
        await expect(app.id(T.positionDetail)).toHaveAttribute('data-value', pos.position);
        const cr = app.id(T.positionCell('cr'));
        await expect(cr).toHaveAttribute('data-value', /^[1-9]\d*$/);
        const crBefore = await dataValue(cr);

        await app.id(T.addMarginInput).fill(AMOUNT_SOL);
        const preview = app.id(T.addMarginPreviewCr);
        await expect.poll(async () => dataValue(preview), { timeout: 15_000 }).toBeGreaterThan(crBefore);
        const crPreview = await dataValue(preview);
        await app.a11y('position detail, add margin');

        await app.id(T.addMarginSubmit).click();
        expect(await app.simulated()).toBe('review');
        expect(await app.delta(pos.position, 'v')).toBe(AMOUNT);
        expect(await app.delta(pos.position, 'm')).toBe(AMOUNT);
        expect(await app.delta(app.walletAddress, 'lamports')).toBeLessThanOrEqual(-AMOUNT);
        await app.signAndConfirm();
        await app.closeModal();

        const after = need(await shortPosition(pos.position), 'margin position');
        expect(after.v - before.v).toBe(AMOUNT);
        // CR = vault (less rent due) over the buyback; rent accrues per second: ±0.1 %.
        await expect
            .poll(
                async () => {
                    const v = await dataValue(cr);
                    return v >= (crPreview * 999n) / 1000n && v <= (crPreview * 1001n) / 1000n;
                },
                { timeout: 30_000 },
            )
            .toBe(true);
    });
});
