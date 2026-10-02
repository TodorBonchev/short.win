/**
 * lend-cpi-guard — the M9 exit's CPI Guard UX tests (FRONTEND.md §10.3; ARCH §3.3, §9 M9; FE-1).
 * NEEDS the LEND track's lend panel (on MARKETS' market page): `@lend`.
 *
 * On the list-only Token-2022 market `t22`, wallet `guard` holds three accounts of the mint: its
 * ATA (plain), `guarded` (CpiGuard `lock_cpi` on) and `race` (the CpiGuard extension, off).
 *
 * 1. The guarded source shows `lend-cpi-guard-notice` with the `token.cpiGuard` text and disables
 *    the plain deposit; selecting the ATA re-enables it.
 * 2. Opting into the bracket (`lend-cpi-guard-bracket`) yields a review listing DisableCpiGuard /
 *    Deposit / EnableCpiGuard (`tx-instruction-<i>`) and deposits through exactly that bracket — all
 *    top-level, owner-signed, one transaction (checked again on the landed transaction) — and the
 *    account still reads `lock_cpi = true` afterwards.
 * 3. Race: CPI Guard is turned on (from Node) on the unguarded `race` account after the review
 *    simulated; Refresh re-simulates and fails with Token-2022 error 42 (`tx-error`
 *    `data-kind="token"`, `data-code="42"`), Sign stays disabled; Cancel and re-entry re-read the
 *    account and show the notice.
 */

import type { Locator } from '@playwright/test';
import { DEPOSIT_DISCRIMINATOR } from '@shortwin/client';
import { DISCLOSURES, LENDER_SOL_REPAYMENT } from '../../src/config/disclosures.ts';
import { type App, expect, need, T, test } from '../fixtures.ts';
import { intentOf, landedTransaction, setCpiGuard, tokenExtensions } from '../validator/actions.ts';
import { getTokenAmount, PROGRAMS } from '../validator/chain.ts';

test.use({ wallet: 'guard' });

const AMOUNT = '10';

async function openDeposit(app: App, mint: string): Promise<Locator> {
    await app.goto(`/m/${mint}?panel=lend&action=deposit`, 'market');
    await app.connectWallet();
    const panel = app.id(T.lendPanel);
    await expect(panel).toHaveAttribute('data-action', 'deposit');
    await expect(panel.getByTestId(T.disclosure('lender.solRepayment'))).toContainText(LENDER_SOL_REPAYMENT);
    return panel;
}

const CPI_GUARD_TEXT = DISCLOSURES['token.cpiGuard'].body[0] ?? '';

test.describe('lend: CPI Guard (§10.3)', { tag: '@lend' }, () => {
    test('a CPI-Guarded source blocks the plain deposit; the ATA re-enables it', async ({ app, state }) => {
        const t22 = state.markets.t22;
        const { ata, guarded } = state.tokenAccounts.guard;
        expect((await tokenExtensions(guarded)).lockCpi).toBe(true);
        const panel = await openDeposit(app, t22.mint);
        const select = panel.getByTestId(T.lendTokenAccount);

        await app.selectTokenAccount(select, guarded);
        await panel.getByTestId(T.lendAmountInput).fill(AMOUNT);
        await panel.getByTestId(T.disclosureAck('lender.solRepayment')).check();
        const notice = panel.getByTestId(T.lendCpiGuardNotice);
        await expect(notice).toBeVisible();
        await expect(notice).toHaveAttribute('data-state', 'blocked');
        await expect(notice).toContainText(CPI_GUARD_TEXT);
        await expect(panel.getByTestId(T.lendSubmit)).toBeDisabled();
        await expect(panel.getByTestId(T.lendCpiGuardBracket)).not.toBeChecked();
        await app.a11y('lend panel with the CPI Guard notice');

        await app.selectTokenAccount(select, ata);
        await expect(panel.getByTestId(T.lendCpiGuardNotice)).toHaveCount(0);
        await expect(panel.getByTestId(T.lendSubmit)).toBeEnabled();
    });

    test('the opt-in bracket deposits through DisableCpiGuard / deposit / EnableCpiGuard; the guard stays on', async ({
        app,
        state,
    }) => {
        const t22 = state.markets.t22;
        const { guarded } = state.tokenAccounts.guard;
        const raw = BigInt(AMOUNT) * 10n ** BigInt(t22.decimals);
        const panel = await openDeposit(app, t22.mint);
        await app.selectTokenAccount(panel.getByTestId(T.lendTokenAccount), guarded);
        await panel.getByTestId(T.lendAmountInput).fill(AMOUNT);
        await panel.getByTestId(T.disclosureAck('lender.solRepayment')).check();
        await expect(panel.getByTestId(T.lendSubmit)).toBeDisabled();

        await panel.getByTestId(T.lendCpiGuardBracket).check();
        await expect(panel.getByTestId(T.lendCpiGuardNotice)).toHaveAttribute('data-state', 'bracket');
        await expect(panel.getByTestId(T.lendSubmit)).toBeEnabled();
        await panel.getByTestId(T.lendSubmit).click();

        expect(await app.simulated()).toBe('review');
        await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'true');
        // The review lists the bracket's three instructions, decoded from their bytes (§5.3, §10.3).
        const steps = [
            ['Token-2022', 'DisableCpiGuard'],
            ['short.win', 'Deposit'],
            ['Token-2022', 'EnableCpiGuard'],
        ] as const;
        for (const [i, [program, name]] of steps.entries()) {
            await expect(app.inModal(T.txInstruction(String(i)))).toHaveAttribute('data-program', program);
            await expect(app.inModal(T.txInstruction(String(i)))).toHaveAttribute('data-name', name);
        }
        await expect(app.inModal(T.txInstruction(String(steps.length)))).toHaveCount(0);
        await expect(app.inModal(T.disclosure('token.cpiGuard'))).toContainText(CPI_GUARD_TEXT);
        expect(await app.delta(guarded, 'amount')).toBe(-raw);
        await app.ackInReview('lender.solRepayment');
        const before = need(await getTokenAmount(guarded), 'guarded balance');
        const signature = await app.signAndConfirm();

        // The landed transaction is exactly the bracket, top level and in order (FE-1).
        const landed = need(await landedTransaction(signature), 'landed bracket');
        expect(landed.err).toBeNull();
        const intent = intentOf(landed);
        expect(intent.map((ix) => ix.program)).toEqual([PROGRAMS.token2022, PROGRAMS.shortwin, PROGRAMS.token2022]);
        expect(Array.from(intent[0]?.data ?? [])).toEqual([34, 1]); // CpiGuardExtension · Disable
        expect(Array.from(intent[1]?.data.subarray(0, 8) ?? [])).toEqual(Array.from(DEPOSIT_DISCRIMINATOR));
        expect(Array.from(intent[2]?.data ?? [])).toEqual([34, 0]); // CpiGuardExtension · Enable
        expect(await getTokenAmount(guarded)).toBe(before - raw);
        expect((await tokenExtensions(guarded)).lockCpi).toBe(true);
    });

    test('race: CPI Guard turned on before Sign → mapped error 42, Sign disabled; re-entry shows the notice', async ({
        app,
        state,
    }) => {
        const t22 = state.markets.t22;
        const { ata, race } = state.tokenAccounts.guard;
        await setCpiGuard('guard', race, false);
        try {
            const panel = await openDeposit(app, t22.mint);
            const select = panel.getByTestId(T.lendTokenAccount);
            await app.selectTokenAccount(select, race);
            await panel.getByTestId(T.lendAmountInput).fill(AMOUNT);
            await panel.getByTestId(T.disclosureAck('lender.solRepayment')).check();
            await expect(panel.getByTestId(T.lendCpiGuardNotice)).toHaveCount(0);
            await panel.getByTestId(T.lendSubmit).click();
            expect(await app.simulated()).toBe('review');

            await setCpiGuard('guard', race, true);
            await app.inModal(T.txRetry).click();
            expect(await app.simulated()).toBe('failed');
            await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'false');
            const err = app.inModal(T.txError);
            await expect(err).toHaveAttribute('data-kind', 'token');
            await expect(err).toHaveAttribute('data-code', '42');
            await expect(err).toContainText(/CPI Guard/);
            await app.ackInReview('lender.solRepayment');
            await expect(app.inModal(T.txSign)).toBeDisabled();
            expect(await app.walletSigned()).toEqual([]);

            await app.closeModal();
            // Re-entry re-reads the account: the guarded source now shows the notice.
            await app.selectTokenAccount(select, ata);
            await app.selectTokenAccount(select, race);
            await expect(panel.getByTestId(T.lendCpiGuardNotice)).toBeVisible();
            await expect(panel.getByTestId(T.lendCpiGuardNotice)).toHaveAttribute('data-state', 'blocked');
            await expect(panel.getByTestId(T.lendSubmit)).toBeDisabled();
        } finally {
            await setCpiGuard('guard', race, false);
        }
    });
});
