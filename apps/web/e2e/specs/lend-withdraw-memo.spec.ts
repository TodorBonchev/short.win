/**
 * lend-withdraw-memo — the M9 exit's memo UX tests (FRONTEND.md §10.3; ARCH §3.3 M1-SEC-7; T-MEMO
 * on mainnet's Token-2022 and spl-memo v3, which the E2E validator runs since M10). NEEDS the LEND
 * track's withdraw tab: `@lend`.
 *
 * Wallets `memo` and `memoRace` each lent into the list-only Token-2022 market `t22` (seed) and own
 * a Token-2022 ATA with the MemoTransfer extension: required (`memo`) or not (`memoRace`). The
 * withdrawal destination is the owner's ATA (the panel's default, §3.6).
 *
 * 1. Withdrawing to the memo-required ATA shows `withdraw-memo-notice`; the transaction passes
 *    `memo_program` (spl-memo v3, CPI'd right before the token transfer) and lands; the ATA's
 *    balance rises by the previewed tokens.
 * 2. Race: memos become required (from Node) after the review simulated. Every Token-2022
 *    withdrawal passes `memo_program` (TMEMO-9; the program CPIs it only when the destination
 *    requires memos), so Refresh still simulates, and the withdrawal lands with the memo CPI'd;
 *    re-entry shows the notice. (Before M10 the plan passed it only for a destination read as
 *    memo-required: Refresh failed with `MemoProgramRequired` 6095 and Retry rebuilt the plan.)
 */

import type { Address } from '@solana/kit';
import { DISCLOSURES } from '../../src/config/disclosures.ts';
import { type App, dataValue, expect, need, T, test } from '../fixtures.ts';
import { ensureLent, landedTransaction, setRequireMemos } from '../validator/actions.ts';
import { getTokenAmount, PROGRAMS } from '../validator/chain.ts';

const MEMO_TEXT = DISCLOSURES['token.memoRequired'].body[0] ?? '';
/** Re-lent (from the wallet's own source account) when a previous run on a reused validator withdrew it all. */
const LEND = 1_000n * 10n ** 6n;

async function openWithdraw(app: App, mint: string) {
    await app.goto(`/m/${mint}?panel=lend&action=withdraw`, 'market');
    await app.connectWallet();
    const panel = app.id(T.lendPanel);
    await expect(panel).toHaveAttribute('data-action', 'withdraw');
    return panel;
}

/** Fills the pro-rata allowance and returns the previewed tokens. */
async function fillMax(app: App): Promise<bigint> {
    const panel = app.id(T.lendPanel);
    await panel.getByTestId(T.withdrawMax).click();
    const tokens = panel.getByTestId(T.withdrawPreview('tokens'));
    await expect(tokens).toHaveAttribute('data-value', /^[1-9]\d*$/);
    return dataValue(tokens);
}

/** The memo program ran inside the withdrawal, before Token-2022's transfer (M1-SEC-7). */
function memoCpiBeforeTransfer(logs: readonly string[]): boolean {
    const memo = logs.findIndex((l) => l.startsWith(`Program ${PROGRAMS.memo} invoke [2]`));
    const transfer = logs.findIndex((l, i) => i > memo && l.startsWith(`Program ${PROGRAMS.token2022} invoke [2]`));
    return memo >= 0 && transfer > memo;
}

test.describe('lend: withdraw to a memo-required account (§10.3)', { tag: '@lend' }, () => {
    test.describe('memo required', () => {
        test.use({ wallet: 'memo' });

        test('shows the notice, passes memo_program and lands; the ATA rises by the preview', async ({
            app,
            state,
        }) => {
            const t22 = state.markets.t22;
            const ata = state.tokenAccounts.memo.ata;
            await ensureLent('memo', t22, state.tokenAccounts.memo.source, LEND);
            const panel = await openWithdraw(app, t22.mint);
            const notice = panel.getByTestId(T.withdrawMemoNotice);
            await expect(notice).toBeVisible();
            await expect(notice).toContainText(MEMO_TEXT);
            await app.a11y('withdraw tab with the memo notice');
            const tokens = await fillMax(app);
            await panel.getByTestId(T.withdrawSubmit).click();

            expect(await app.simulated()).toBe('review');
            expect(await app.delta(ata, 'amount')).toBe(tokens);
            const before = need(await getTokenAmount(ata), 'ATA balance');
            const signature = await app.signAndConfirm();
            const landed = need(await landedTransaction(signature), 'landed withdrawal');
            expect(landed.err).toBeNull();
            expect(memoCpiBeforeTransfer(landed.logs)).toBe(true);
            expect(await getTokenAmount(ata)).toBe(before + tokens);
        });
    });

    test.describe('race', () => {
        test.use({ wallet: 'memoRace' });

        test('memos required before Sign: the plan already passes memo_program, Refresh simulates, it lands with the memo', async ({
            app,
            state,
        }) => {
            const t22 = state.markets.t22;
            const ata: Address = state.tokenAccounts.memoRace.ata;
            await setRequireMemos('memoRace', ata, false);
            await ensureLent('memoRace', t22, state.tokenAccounts.memoRace.source, LEND);
            try {
                const panel = await openWithdraw(app, t22.mint);
                await expect(panel.getByTestId(T.withdrawMemoNotice)).toHaveCount(0);
                const tokens = await fillMax(app);
                await panel.getByTestId(T.withdrawSubmit).click();
                expect(await app.simulated()).toBe('review');

                // TMEMO-9: memos become required after the review; the plan already names spl-memo, so
                // Refresh re-simulates the same plan and it still passes.
                await setRequireMemos('memoRace', ata, true);
                await app.inModal(T.txRetry).click();
                expect(await app.simulated()).toBe('review');
                expect(await app.delta(ata, 'amount')).toBe(tokens);
                const before = need(await getTokenAmount(ata), 'ATA balance');
                const signature = await app.signAndConfirm();
                const landed = need(await landedTransaction(signature), 'landed withdrawal');
                expect(landed.err).toBeNull();
                expect(memoCpiBeforeTransfer(landed.logs)).toBe(true);
                expect(await getTokenAmount(ata)).toBe(before + tokens);
                await app.closeModal();

                // Re-entry (lent again, since Max withdrew it all): the panel re-reads the destination.
                await ensureLent('memoRace', t22, state.tokenAccounts.memoRace.source, LEND);
                await app.goto(`/m/${t22.mint}?panel=lend&action=withdraw`, 'market');
                await expect(panel.getByTestId(T.withdrawMemoNotice)).toBeVisible();
            } finally {
                await setRequireMemos('memoRace', ata, false);
            }
        });
    });
});
