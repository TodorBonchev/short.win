/**
 * lend-deposit (FRONTEND.md §3.6, §4 `lender.solRepayment`, §5.3, §10.2). NEEDS the LEND track's
 * lend panel and lender dashboard (and MARKETS' market page that renders the panel): `@lend`.
 *
 * SPEC §5.3's sentence verbatim on the deposit screen before signing, its required checkbox gating
 * the submit and again the review's Sign; the pool's unpaid rent (`Market.receivable`, credited by
 * a lender prologue just before — the seeded `margin` short pays rent into `main` — plus what the
 * deposit's own prologue credits since) and this deposit's part of it, with its level
 * (`lender.unpaidRent`, ECON §9.4; none on a release-build run, where nothing is shorted); the
 * simulated deltas (source token account, lender shares); the confirmed deposit on chain; the
 * dashboard row.
 */
import { podU128ToBigInt } from '@shortwin/client';
import { LENDER_SOL_REPAYMENT } from '../../src/config/disclosures.ts';
import { unpaidRentExposure } from '../../src/features/lend/previews.ts';
import { dataValue, expect, need, T, test } from '../fixtures.ts';
import {
    accrue,
    landedTransaction,
    lenderPosition,
    lenderPositionAddress,
    marketAccount,
} from '../validator/actions.ts';
import { getTokenAmount } from '../validator/chain.ts';

test.use({ wallet: 'lender' });

const AMOUNT_TOKENS = 1_000n;

test.describe('lend: deposit', { tag: '@lend' }, () => {
    test('SPEC §5.3 verbatim, Sign gated by its checkbox, simulated deltas, confirmed, dashboard', async ({
        app,
        state,
    }) => {
        const m = state.markets.main;
        const source = need(state.tokenAccounts.mainAta.lender, 'lender main ATA');
        const raw = AMOUNT_TOKENS * 10n ** BigInt(m.decimals);
        // The seeded short's rent credited up to now (test-venue build): the pool has unpaid rent.
        if (state.positions) await accrue(m);
        const market = await marketAccount(m);

        await app.goto(`/m/${m.mint}?panel=lend&action=deposit`, 'market');
        await app.connectWallet();
        const panel = app.id(T.lendPanel);
        await expect(panel).toHaveAttribute('data-mint', m.mint);
        await expect(panel).toHaveAttribute('data-action', 'deposit');

        // §4.1 rule 2: verbatim, on the deposit screen, before signing, with its own checkbox.
        const sol = panel.getByTestId(T.disclosure('lender.solRepayment'));
        await expect(sol).toBeVisible();
        await expect(sol).toContainText(LENDER_SOL_REPAYMENT);
        await expect(panel.getByTestId(T.disclosureAck('lender.solRepayment'))).not.toBeChecked();
        await expect(panel.getByTestId(T.disclosure('lender.risks'))).toBeVisible();

        await app.selectTokenAccount(panel.getByTestId(T.lendTokenAccount), source);
        await panel.getByTestId(T.lendAmountInput).fill(AMOUNT_TOKENS.toString());
        await expect(panel.getByTestId(T.lendPreview('shares'))).toHaveAttribute('data-value', /^[1-9]\d*$/);
        const previewShares = await dataValue(panel.getByTestId(T.lendPreview('shares')));

        // ECON §9.4: the unpaid rent the deposit meets — at least the stored receivable (the panel adds
        // the rent accrued since) — and this deposit's pro-rata part of it should none be paid.
        let unpaid = unpaidRentExposure(0n, podU128ToBigInt(market.totalShares), previewShares, raw, 0n);
        if (state.positions) {
            expect(market.receivable).toBeGreaterThan(0n);
            const row = panel.getByTestId(T.lendPreview('unpaid-rent'));
            const share = panel.getByTestId(T.lendPreview('unpaid-rent-share'));
            const of = (receivable: bigint) =>
                unpaidRentExposure(
                    receivable,
                    podU128ToBigInt(market.totalShares),
                    previewShares,
                    raw,
                    podU128ToBigInt(market.emaPrice),
                );
            // The two figures of one render (the panel's clock ticks every 5 s and the receivable with it).
            let shown = 0n;
            await expect
                .poll(async () => {
                    shown = await dataValue(row);
                    return (await dataValue(share)) === of(shown).exposure;
                })
                .toBe(true);
            expect(shown).toBeGreaterThanOrEqual(market.receivable);
            unpaid = of(shown);
            expect(unpaid.exposure).toBeGreaterThan(0n);
            expect(unpaid.exposure).toBeLessThanOrEqual(shown);
            await expect(panel.locator(`.lend-unpaid[data-level="${unpaid.level}"]`)).toHaveCount(1);
            const disclosure = panel.getByTestId(T.disclosure('lender.unpaidRent'));
            await expect(disclosure).toHaveCount(1);
            await expect(disclosure).toHaveJSProperty('tagName', unpaid.level === 'warn' ? 'SECTION' : 'DETAILS');
        } else {
            expect(market.receivable).toBe(0n);
            await expect(panel.getByTestId(T.lendPreview('unpaid-rent'))).toHaveCount(0);
            await expect(panel.getByTestId(T.disclosure('lender.unpaidRent'))).toHaveCount(0);
        }

        await expect(panel.getByTestId(T.lendSubmit)).toBeDisabled();
        await panel.getByTestId(T.disclosureAck('lender.solRepayment')).check();
        await expect(panel.getByTestId(T.lendSubmit)).toBeEnabled();
        await app.a11y('market page, lend panel (deposit)');
        await panel.getByTestId(T.lendSubmit).click();

        expect(await app.simulated()).toBe('review');
        await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'true');
        // The review repeats the sentence; acknowledgements are per transaction (never remembered).
        await expect(app.inModal(T.disclosure('lender.solRepayment'))).toContainText(LENDER_SOL_REPAYMENT);
        await expect(app.inModal(T.disclosureAck('lender.solRepayment'))).not.toBeChecked();
        await expect(app.inModal(T.txSign)).toBeDisabled();
        // The review repeats the unpaid-rent disclosure only when the deposit's part is material.
        await expect(app.inModal(T.disclosure('lender.unpaidRent'))).toHaveCount(unpaid.level === 'warn' ? 1 : 0);

        expect(await app.delta(source, 'amount')).toBe(-raw);
        const lenderPda = await lenderPositionAddress(m.market, app.walletAddress);
        const simShares = await app.delta(lenderPda, 'shares');
        // FE-D6: min_shares = ⌊preview·(1 − 0.5 %)⌋; the preview equals the program up to rent accrued since.
        expect(simShares).toBeGreaterThanOrEqual((previewShares * 995n) / 1000n);
        expect(simShares).toBeLessThanOrEqual((previewShares * 1005n) / 1000n);
        expect(await dataValue(app.inModal(T.txCu))).toBeGreaterThan(0n);
        expect(await dataValue(app.inModal(T.txFee))).toBeGreaterThanOrEqual(5_000n);
        await app.a11y('deposit review', `[data-testid="${T.txModal}"]`);

        await app.ackInReview('lender.solRepayment');
        const before = need(await getTokenAmount(source), 'source balance');
        const signature = await app.signAndConfirm();
        await expect(app.inModal(T.txExplorer)).toHaveAttribute('href', new RegExp(signature));

        const landed = need(await landedTransaction(signature as never), 'landed deposit');
        expect(landed.err).toBeNull();
        expect(await getTokenAmount(source)).toBe(before - raw);
        const lp = need(await lenderPosition(m.market, app.walletAddress), 'lender position');
        expect(lp.shares).toBeGreaterThanOrEqual((previewShares * 995n) / 1000n);
        await app.closeModal();
        // Focus never falls to <body>: the Deposit submit, disabled once the form resets after the
        // confirmed deposit, cannot take it back, so the page's main region does (Modal's focusBack).
        await expect(app.page.locator('#main')).toBeFocused();

        await app.goto('/lend', 'lend');
        await expect(app.id(T.lenderDashboard)).toBeVisible();
        const row = app.id(T.lenderRow(m.market));
        await expect(row).toBeVisible();
        await expect(row.getByTestId(T.lenderCell('shares'))).toHaveAttribute('data-value', lp.shares.toString());
        await app.a11y('lender dashboard');
    });
});
