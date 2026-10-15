/**
 * short-close-curve (design pump-final §8, §14 "Web"; FRONTEND.md §5.11, FE-3 closed). NEEDS the
 * SHORT track's pages and the dumped venue fixtures (pump's 2026-10-13 ELFs and state): `@short`.
 *
 * A close past the curve's `rT` on the validator, end to end. Node lists the dumped `near166` curve
 * (`create_market(Auto)`) and lends to it, and a third party trades the curve once (pump tops the
 * dumped curve up to the validator's higher rent on its first trade); after the 50 s warm-up the
 * `curve` wallet opens 1.5 M
 * tokens in the Short panel — `sell_v3` with the 10 v3 TradeExtras (`lists.ts`), the review's
 * simulation and the landing on pump's ELF; a third party then buys the curve down to `q/2`
 * (`buy_v3`). On the position page the close previews `B̂_v3` like any close (no straddle notice,
 * nothing in its way), the review simulates it, and one `close_short` buys `rT` on the curve and
 * `q − rT` from the would-be pool (pump's synthetic migration): the position is gone, the curve is
 * complete with `pcB = q − rT`, and the market moved to `PumpMigrating` in that instruction. axe at 0
 * on the panel, the position page and the close review; no CSP violation (the fixture's teardown).
 */
import { dataValue, expect, need, requireVenueFixtures, T, test } from '../fixtures.ts';
import { shortPosition } from '../validator/actions.ts';
import { waitForClock } from '../validator/chain.ts';
import { crowdBuy, curveState, listCurveMarket, marketVenue } from '../validator/curve.ts';

test.use({ wallet: 'curve' });

/** `q = vT/200` of `near166` at the dump, within every default cap (`synthetic_exit_it`). */
const Q_TOKENS = 1_500_000n;
const DECIMALS = 6n;
/** `market_warmup_s` (its compiled minimum, the seed's). */
const MARKET_WARMUP_S = 50n;
/** `VenueKind::PumpMigrating`. */
const PUMP_MIGRATING = 2;

test.describe('short: a close past the curve rT (pump v3)', { tag: '@short' }, () => {
    test('opens on near166, the crowd buys past q, one close completes the curve', async ({ app, state }) => {
        requireVenueFixtures(state);
        test.setTimeout(420_000);
        const m = await listCurveMarket();
        // The crowd trades first: the validator's rent (6,960 lamports a byte) is above mainnet's
        // (5,080), so pump tops the dumped curve's lamports up to its rent on the curve's first trade
        // (552,720 here), from that trade's user — an open would read it as venue rent beyond its
        // tolerance (ProceedsBelowMin). On mainnet a 166-byte curve holds its rent already.
        await crowdBuy(1_000_000_000n, 10n ** 9n);
        await waitForClock(m.createdAt + MARKET_WARMUP_S + 2n);
        const q = Q_TOKENS * 10n ** DECIMALS;

        // The open, on the curve (`sell_v3`).
        await app.goto(`/m/${m.mint}?panel=short`, 'market');
        await app.connectWallet();
        const panel = app.id(T.shortPanel);
        await expect(panel).toHaveAttribute('data-mint', m.mint);
        await expect(panel.getByTestId(T.shortBlocked)).toHaveCount(0);
        await panel.getByTestId(T.shortQInput).fill(Q_TOKENS.toString());
        await expect(panel.getByTestId(T.shortPreview('proceeds'))).toHaveAttribute('data-value', /^\d+$/);
        expect(await dataValue(panel.getByTestId(T.shortPreview('q')))).toBe(q);
        const ack = panel.getByTestId(T.disclosureAck('borrower.liquidation'));
        if (await ack.count()) await ack.check();
        await app.a11y('market page, short panel on the curve');
        await panel.getByTestId(T.shortSubmit).click();
        expect(await app.simulated()).toBe('review');
        const dq = await app.deltaOfNew('q', [app.walletAddress, m.market]);
        expect(dq.change).toBe(q);
        const position = dq.address;
        await app.ackInReview('borrower.liquidation');
        await app.signAndConfirm();
        expect(need(await shortPosition(position), 'the opened position').q).toBe(q);

        // A third party buys the curve down to q/2: closing now buys past rT.
        const open = await curveState();
        await crowdBuy(open.rt - q / 2n, 100n * 10n ** 9n);
        const straddled = await curveState();
        expect(straddled.rt).toBe(q / 2n);
        expect(straddled.complete).toBe(false);

        // The close: an ordinary close at B̂_v3 (FE-3 closed), one close_short.
        await app.goto(`/positions/${position}`, 'position');
        await expect(app.id(T.positionDetail)).toHaveAttribute('data-value', position);
        const buyback = app.id(T.closePreview('buyback'));
        await expect(buyback).toHaveAttribute('data-value', /^\d+$/);
        expect(await dataValue(buyback)).toBeGreaterThan(0n);
        await expect(app.id(T.closeSubmit)).toBeEnabled();
        await app.a11y('position detail, a close past the curve rT');
        await app.id(T.closeSubmit).click();
        expect(await app.simulated()).toBe('review');
        expect(await app.delta(position, 'q')).toBe(-q);
        await app.a11y('close review past rT', `[data-testid="${T.txModal}"]`);
        await expect(app.inModal(T.txSign)).toBeEnabled();
        await app.inModal(T.txSign).click();
        await expect.poll(async () => shortPosition(position), { timeout: 90_000 }).toBeNull();

        // The close completed the curve by synthetic migration and moved the market in that instruction.
        const done = await curveState();
        expect(done.complete).toBe(true);
        expect(done.rt).toBe(0n);
        expect(done.pcb).toBe(q - straddled.rt);
        expect(done.pcq).toBeGreaterThan(0n);
        expect(await marketVenue(m.market)).toBe(PUMP_MIGRATING);
    });
});
