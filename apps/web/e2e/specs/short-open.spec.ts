/**
 * short-open (FRONTEND.md §3.7, §4 `borrower.liquidation`, §5.3, §10.2). NEEDS the SHORT track's
 * short panel and venue layer (on MARKETS' market page): `@short`.
 *
 * 1. Every preview field of §3.7 is present with its raw `data-value`; the review's simulated
 *    position equals the preview (`q`, `s` = proceeds, `m` = margin, `v` = proceeds + margin) and the
 *    market's free tokens drop by `q`; the borrower disclosure's checkbox gates Sign; the open lands.
 * 2. Blocked state: on the coverage-paused market the panel shows `short-blocked-reason` with
 *    `data-code="CoveragePaused"` and the `market.coveragePause` disclosure (the page's banner; the
 *    panel links to it, O-2), and nothing opens.
 */
import { DISCLOSURES } from '../../src/config/disclosures.ts';
import { dataValue, expect, need, requireTrading, T, test } from '../fixtures.ts';
import { shortPosition } from '../validator/actions.ts';
import { getBalance } from '../validator/chain.ts';

test.use({ wallet: 'trader' });

const Q_TOKENS = 400_000n;

/** Fields with a raw integer `data-value` (§11.4); `tier-terms` and `caps` are composite. */
const INTEGER_FIELDS = [
    'q',
    'proceeds',
    'margin',
    'open-fee',
    'total-cost',
    'cr-open',
    'cr-ema',
    'liq-price',
    'rent-rate',
    'min-rent',
    'max-duration',
    'deadline',
] as const;

test.describe('short: open', { tag: '@short' }, () => {
    test('every preview field, the borrower disclosure gate, the simulated position equals the preview', async ({
        app,
        state,
    }) => {
        requireTrading(state);
        const m = state.markets.main;
        const q = Q_TOKENS * 10n ** BigInt(m.decimals);
        await app.goto(`/m/${m.mint}?panel=short`, 'market');
        await app.connectWallet();
        const panel = app.id(T.shortPanel);
        await expect(panel).toHaveAttribute('data-mint', m.mint);
        await expect(panel.getByTestId(T.shortBlocked)).toHaveCount(0);

        await panel.getByTestId(T.shortQInput).fill(Q_TOKENS.toString());
        await expect(panel.getByTestId(T.shortSlippageInput)).toBeVisible();
        for (const f of INTEGER_FIELDS) {
            await expect(panel.getByTestId(T.shortPreview(f)), f).toHaveAttribute('data-value', /^-?\d+$/);
        }
        for (const f of ['tier-terms', 'caps'] as const)
            await expect(panel.getByTestId(T.shortPreview(f))).toBeVisible();
        await expect(panel.getByTestId(T.shortCapWarning)).toHaveCount(0);
        expect(await dataValue(panel.getByTestId(T.shortPreview('q')))).toBe(q);
        const proceeds = await dataValue(panel.getByTestId(T.shortPreview('proceeds')));
        const margin = await dataValue(panel.getByTestId(T.shortPreview('margin')));
        const crOpen = await dataValue(panel.getByTestId(T.shortPreview('cr-open')));
        expect(proceeds).toBeGreaterThan(0n);
        expect(margin).toBeGreaterThan(0n);
        expect(crOpen).toBeGreaterThan(0n);
        const liq = panel.getByTestId(T.disclosure('borrower.liquidation'));
        await expect(liq).toBeVisible();
        await expect(liq).toContainText('S1 — the price jumped past your vault');
        await app.a11y('market page, short panel');

        const ack = panel.getByTestId(T.disclosureAck('borrower.liquidation'));
        if (await ack.count()) await ack.check();
        await panel.getByTestId(T.shortSubmit).click();
        expect(await app.simulated()).toBe('review');
        await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'true');
        await expect(app.inModal(T.txSign)).toBeDisabled();

        // The new position's PDA has a random nonce: find its rows by field.
        const exclude = [app.walletAddress, m.market];
        const dq = await app.deltaOfNew('q', exclude);
        const position = dq.address;
        expect(dq.change).toBe(q);
        expect(await app.delta(position, 's')).toBe(proceeds);
        expect(await app.delta(position, 'm')).toBe(margin);
        expect(await app.delta(position, 'v')).toBe(proceeds + margin);
        // The market's free tokens drop by q (§5.3 names the field `tokens_free`; `tokensFree` accepted).
        const free = (await app.deltas()).find(
            (d) => d.address === m.market && (d.field === 'tokens_free' || d.field === 'tokensFree'),
        );
        expect(free?.change).toBe(-q);
        expect(await app.delta(app.walletAddress, 'lamports')).toBeLessThan(0n);
        await app.a11y('open review', `[data-testid="${T.txModal}"]`);

        await app.ackInReview('borrower.liquidation');
        await expect(app.inModal(T.txSign)).toBeEnabled();
        const before = await getBalance(app.walletAddress);
        await app.signAndConfirm();
        const p = need(await shortPosition(position), 'the opened position');
        expect(p.q).toBe(q);
        expect(p.s).toBe(proceeds);
        expect(p.m).toBe(margin);
        expect(await getBalance(app.walletAddress)).toBeLessThan(before);
    });

    test('blocked: new shorts paused for keeper coverage', async ({ app, state }) => {
        requireTrading(state);
        const m = state.markets.paused;
        await app.goto(`/m/${m.mint}?panel=short`, 'market');
        await app.connectWallet();
        const panel = app.id(T.shortPanel);
        const blocked = panel.getByTestId(T.shortBlocked);
        await expect(blocked).toBeVisible();
        await expect(blocked).toHaveAttribute('data-code', 'CoveragePaused');
        await expect(app.id(T.disclosure('market.coveragePause')).first()).toContainText(
            DISCLOSURES['market.coveragePause'].body[0] ?? '',
        );
        // Shown once, as the page's banner; the panel links to it (the thirteenth round, O-2).
        await expect(panel.getByTestId(T.disclosure('market.coveragePause'))).toHaveCount(0);
        await expect(panel.getByTestId(T.shortCoverageLink)).toBeVisible();
        const submit = panel.getByTestId(T.shortSubmit);
        if (await submit.count()) await expect(submit).toBeDisabled();
        await app.a11y('short panel, coverage paused');
    });
});
