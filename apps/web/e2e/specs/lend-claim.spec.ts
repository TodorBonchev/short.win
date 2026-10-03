/**
 * lend-claim (FRONTEND.md §3.6 Claim / Withdraw, §4 `lender.exitForfeitsVest`, §10.2). NEEDS the
 * LEND track's claim and withdraw tabs: `@lend`.
 *
 * Wallet `claimer` lent most of `main`'s inventory before the seeded positions opened, so rent
 * accrues to it continuously; a short opened from Node and closed 30 s later pays rent in (its
 * minimum rent's excess over what accrued vests, M10-8C-1).
 *
 * 1. SOL becomes claimable as rent accrues; the claim lands (the lender's `claimed` rises by the
 *    simulated amount).
 * 2. With unvested one-off income in the pool (a donation swept by `sweep_trader` vests over
 *    `lump_vest_s`), a withdrawal that leaves shares behind shows the vest-forfeit warning, and its
 *    checkbox gates Sign.
 */
import { DISCLOSURES } from '../../src/config/disclosures.ts';
import { dataValue, expect, need, requireTrading, T, test } from '../fixtures.ts';
import { donateAndSweep, lenderPosition, lenderPositionAddress, payRentIn } from '../validator/actions.ts';
import { LAMPORTS_PER_SOL } from '../validator/chain.ts';

test.use({ wallet: 'claimer' });

test.describe('lend: claim and the vest-forfeit warning', { tag: '@lend' }, () => {
    test('claimable SOL after rent accrues; the claim lands', async ({ app, state }) => {
        requireTrading(state);
        const m = state.markets.main;
        // Rent accrues every second on the open positions and is claimable once a borrower pays it
        // in: a short opened from Node and closed 30 s later pays at least the minimum rent; what it
        // accrued while open is claimable at once, the minimum rent's excess vests (M10-8C-1).
        await payRentIn(m, 200_000n * 10n ** BigInt(m.decimals));
        await app.goto(`/m/${m.mint}?panel=lend&action=claim`, 'market');
        await app.connectWallet();
        const panel = app.id(T.lendPanel);
        await expect(panel).toHaveAttribute('data-action', 'claim');
        const claimable = panel.getByTestId(T.claimValue('claimable'));
        await expect(claimable).toHaveAttribute('data-value', /^[1-9]\d*$/, { timeout: 90_000 });
        for (const f of ['vesting', 'pot'] as const) {
            await expect(panel.getByTestId(T.claimValue(f))).toHaveAttribute('data-value', /^\d+$/);
        }
        await app.a11y('claim tab');

        const before = need(await lenderPosition(m.market, app.walletAddress), 'claimer lender position');
        await panel.getByTestId(T.claimSubmit).click();
        expect(await app.simulated()).toBe('review');
        const lenderPda = await lenderPositionAddress(m.market, app.walletAddress);
        const simClaimed = await app.delta(lenderPda, 'claimed');
        expect(simClaimed).toBeGreaterThan(0n);
        await app.signAndConfirm();
        const after = need(await lenderPosition(m.market, app.walletAddress), 'claimer lender position');
        // Rent keeps accruing between the simulation and the landing: at least the simulated claim.
        expect(after.claimed - before.claimed).toBeGreaterThanOrEqual(simClaimed);
    });

    test('a partial withdrawal with unvested income shows the vest-forfeit warning; its checkbox gates Sign', async ({
        app,
        state,
    }) => {
        requireTrading(state);
        const m = state.markets.main;
        await donateAndSweep(m, 2n * LAMPORTS_PER_SOL);
        await app.goto(`/m/${m.mint}?panel=lend&action=withdraw`, 'market');
        await app.connectWallet();
        const panel = app.id(T.lendPanel);
        // Max = the pro-rata allowance: with tokens lent out some shares stay behind.
        await panel.getByTestId(T.withdrawMax).click();
        await expect(panel.getByTestId(T.withdrawPreview('allowance'))).toHaveAttribute('data-value', /^[1-9]\d*$/);
        const forfeit = panel.getByTestId(T.withdrawPreview('vesting-forfeit'));
        await expect(forfeit).toHaveAttribute('data-value', /^[1-9]\d*$/);
        const warning = panel.getByTestId(T.disclosure('lender.exitForfeitsVest'));
        await expect(warning).toBeVisible();
        await expect(warning).toContainText('of income still vesting');
        expect(await dataValue(forfeit)).toBeGreaterThan(0n);
        await app.a11y('withdraw tab with the vest-forfeit warning');
        // A panel may also ask for the acknowledgement before the review (the review asks again).
        const panelAck = panel.getByTestId(T.disclosureAck('lender.exitForfeitsVest'));
        if (await panelAck.count()) await panelAck.check();

        await panel.getByTestId(T.withdrawSubmit).click();
        expect(await app.simulated()).toBe('review');
        const ack = app.inModal(T.disclosureAck('lender.exitForfeitsVest'));
        await expect(ack).toBeVisible();
        await expect(app.inModal(T.disclosure('lender.exitForfeitsVest'))).toContainText(
            DISCLOSURES['lender.exitForfeitsVest'].ack ?? '',
        );
        await expect(app.inModal(T.txSign)).toBeDisabled();
        await ack.check();
        await expect(app.inModal(T.txSign)).toBeEnabled();
        // The shared market keeps its inventory for the other specs: review only.
        await app.closeModal();
    });
});
