/**
 * lend-claim-sync (FRONTEND.md §3.6 Claim, §3.8 the Lending page, FE-18; ECON §9.4; R3A-2). NEEDS
 * the LEND track and the test-venue build (the mock venue): `@lend`.
 *
 * Mock market `pot` is lent by wallet `potClaimer` alone and `potBorrower` holds a position there
 * from the seed on, so rent is credited to `potClaimer` (`acc`) every second. Then `potLate` lends
 * nine times as much and the borrower closes: the rent is paid in and realised (`acc_r`) across
 * both lenders' shares, so `potLate` holds realised SOL beyond its entitlement — a surplus that
 * moves to the pot only when it is touched — and `potClaimer`'s entitlement is mostly not realised
 * on its own position (a shortfall the empty pot cannot pay).
 *
 * Before `potLate` lends, `potClaimer` is the market's only lender: its shortfall (rent the open
 * short has not paid in) is one no sync can fund, so — the owner's decision of 2026-10-11, "only
 * when syncs can pay" — Lend → Claim and the Lending page's row are the disabled "Review claim ·
 * 0 SOL" with "Nothing to claim yet" (the note still names the whole shortfall), and "Claim all" is
 * not shown.
 *
 * Lend → Claim says so ("… now · syncs may add up to …"). Then the claim alone lands from Node:
 * `potClaimer` has nothing claimable left, only the shortfall. The Lending page's row (the owner's
 * decision of 2026-10-11: a row claims like the panel) offers the claim through the syncs —
 * "Review claim · 0 SOL now · syncs may add up to …", enabled, counted by "Claim all" — reads the
 * other lenders from the indexer, judges each with `previewTouch` and bundles
 * `[sync_lender(potLate), claim_sol]`: the landed transaction is exactly that, the sync moves the
 * surplus to the pot, the claim draws it and pays exactly what the review simulated, where the
 * claim alone would now pay nothing.
 */
import {
    CLAIM_SOL_DISCRIMINATOR,
    CLAIM_SOL_EVENT_DISCRIMINATOR,
    getClaimSolEventDecoder,
    getLenderSyncedEventDecoder,
    LENDER_SYNCED_EVENT_DISCRIMINATOR,
    SYNC_LENDER_DISCRIMINATOR,
} from '@shortwin/client';
import { dataValue, expect, need, requireTrading, T, test } from '../fixtures.ts';
import {
    accrue,
    claimSolFromNode,
    closeShortFromNode,
    eventsOf,
    intentOf,
    landedTransaction,
    lenderPosition,
    lenderPositionAddress,
    lendFromNode,
    marketVaultAccount,
    shortPosition,
    simulateClaimSol,
    simulatePotSync,
    simulateSyncLender,
} from '../validator/actions.ts';
import { sleep, waitForClock } from '../validator/chain.ts';
import { signerFor } from '../validator/keys.ts';

test.use({ wallet: 'potClaimer' });

/** `potLate`'s deposit: nine times `potClaimer`'s (seed.ts), so ~90 % of the realised rent is its. */
const LATE_TOKENS = 5_400_000n;
/** Seconds of rent credited to `potClaimer` alone before `potLate` lends (at least). */
const ALONE_S = 30n;
/**
 * …and lamports (at least): its shortfall after the close (most of it: `potLate` realises nine
 * tenths of the rent paid in) stays well above the claim's 10,000-lamport dust threshold
 * (`CLAIM_DUST_LAMPORTS`, the owner's decision of 2026-10-11), under which no sync is looked for.
 */
const ALONE_CREDIT = 40_000n;

test.describe('lend: a claim syncs other lenders first (FE-18)', { tag: '@lend' }, () => {
    test('the pot cannot pay the claim: after the claim alone, the Lending page’s row claims through [sync_lender(other), claim_sol] and pays what the review showed', async ({
        app,
        state,
    }) => {
        requireTrading(state);
        const m = state.markets.pot;
        const p = need(state.positions?.pot, 'pot position');
        const open = await shortPosition(p.position);
        test.skip(!open, 'the pot market is spent (a reused validator): run without SHORTWIN_E2E_REUSE');
        const late = state.wallets.potLate.address;

        // Rent credited to potClaimer alone (the market's prologue books it; a touch simulated by a
        // third party reads it), then potLate lends, then the rent is paid in.
        await waitForClock(need(open, 'pot position').tOpen + ALONE_S);
        for (let i = 0; ; i++) {
            await accrue(m);
            const { lender } = await simulatePotSync('potClaimer', m, []);
            console.log(`[pot] potClaimer credited ${lender.pending} lamports (+${i * 5} s)`);
            if (BigInt(lender.pending) >= ALONE_CREDIT) break;
            expect(i, `potClaimer's credit ${lender.pending} after ${i * 5} s more`).toBeLessThan(24);
            await sleep(5_000);
        }
        // The only lender's shortfall: no sync can fund it, so nothing is offered (owner, 2026-10-11).
        await app.goto(`/m/${m.mint}?panel=lend&action=claim`, 'market');
        await app.connectWallet();
        const only = app.id(T.lendPanel);
        await expect(only).toHaveAttribute('data-action', 'claim');
        await expect(only.getByTestId(T.claimValue('claimable'))).toHaveAttribute('data-value', '0', {
            timeout: 60_000,
        });
        await expect(only.getByTestId(T.lendClaimReason)).toHaveText('Nothing to claim yet', { timeout: 30_000 });
        const onlySubmit = only.getByTestId(T.claimSubmit);
        await expect(onlySubmit).toBeDisabled();
        await expect(onlySubmit).toHaveText('Review claim · 0 SOL');
        await expect(onlySubmit).not.toHaveAttribute('aria-busy', 'true');
        await expect(only.getByTestId(T.claimSubmitSyncs)).toHaveCount(0);
        await expect(only).toContainText('of what is credited to you cannot be claimed now');
        await app.a11y('claim tab: the only lender’s shortfall, no sync to fund it');
        await app.goto('/lend', 'lend');
        await app.connectWallet();
        const onlyRow = app.id(T.lenderRow(m.market));
        await expect(onlyRow.getByTestId(T.lenderClaimReason(m.market))).toHaveText('Nothing to claim yet', {
            timeout: 60_000,
        });
        await expect(onlyRow.getByTestId(T.lenderClaim(m.market))).toBeDisabled();
        await expect(onlyRow.getByTestId(T.lenderClaim(m.market))).toHaveText('Review claim · 0 SOL');
        await expect(app.id(T.lenderClaimAll)).toHaveCount(0);

        await lendFromNode('potLate', m, state.tokenAccounts.pot.late, LATE_TOKENS * 10n ** BigInt(m.decimals));
        await closeShortFromNode(await signerFor('potBorrower'), m, p);
        expect(await shortPosition(p.position)).toBeNull();
        const vault0 = need(await marketVaultAccount(m), 'pot market vault');
        expect(vault0.unmatched).toBe(0n);
        // What the claim alone would pay (simulated on the validator; nothing lands).
        const alone = await simulateClaimSol('potClaimer', m);
        expect(alone).toBeGreaterThan(0n);
        // What a touch of potLate alone moves to the pot (simulated): no rent accrues any more, so
        // the landed sync moves exactly this.
        const surplus = (await simulateSyncLender('potLate', m)).unmatched - vault0.unmatched;
        expect(surplus).toBeGreaterThan(0n);

        // Lend → Claim: the claim alone's preview, and that syncs may add to it (O-5).
        await app.goto(`/m/${m.mint}?panel=lend&action=claim`, 'market');
        await app.connectWallet();
        const panel = app.id(T.lendPanel);
        await expect(panel).toHaveAttribute('data-action', 'claim');
        const claimable = panel.getByTestId(T.claimValue('claimable'));
        await expect(claimable).toHaveAttribute('data-value', /^[1-9]\d*$/, { timeout: 60_000 });
        await expect(panel.getByTestId(T.claimValue('pot'))).toHaveAttribute('data-value', '0');
        // The panel's preview is the claim alone (the pot as it is).
        expect(await dataValue(claimable)).toBe(alone);
        // The search for syncs runs before the label offers them: potLate's surplus can fund it.
        await expect(panel.getByTestId(T.claimSubmitSyncs)).toContainText('syncs may add up to', { timeout: 30_000 });
        await app.a11y('claim tab with a shortfall the pot cannot pay');

        // The claim alone lands (from Node): potClaimer's own realised SOL is paid, the rest is short.
        const before = need(await lenderPosition(m.market, app.walletAddress), 'potClaimer lender position');
        await claimSolFromNode('potClaimer', m);
        const mid = need(await lenderPosition(m.market, app.walletAddress), 'potClaimer lender position');
        expect(mid.claimed - before.claimed).toBe(alone);
        expect(await simulateClaimSol('potClaimer', m)).toBe(0n);
        const vaultMid = need(await marketVaultAccount(m), 'pot market vault');
        expect(vaultMid.lenderSol).toBe(vault0.lenderSol - alone);

        // The Lending page: the row has nothing claimable now, yet its Claim is the panel's — the
        // claim through the syncs, enabled, in the panel's words; "Claim all" counts the row.
        await app.goto('/lend', 'lend');
        await app.connectWallet();
        const row = app.id(T.lenderRow(m.market));
        await expect(row.getByTestId(T.lenderCell('claimable'))).toHaveAttribute('data-value', '0', {
            timeout: 60_000,
        });
        const claim = row.getByTestId(T.lenderClaim(m.market));
        await expect(claim).toBeEnabled({ timeout: 30_000 });
        await expect(claim).toHaveAttribute('data-lines', '2');
        await expect(claim).toContainText('Review claim · 0 SOL now');
        const syncsLine = row.getByTestId(T.claimSubmitSyncs);
        await expect(syncsLine).toContainText('syncs may add up to');
        await expect(row.getByTestId(T.lenderClaimReason(m.market))).toHaveCount(0);
        await expect(app.id(T.lenderClaimAll)).toBeVisible();
        await app.a11y('the Lending page: a row with a shortfall only');
        // "syncs may add up to {g} SOL": {g} is min(shortfall, what the picked syncs move), the most
        // the claim can pay.
        const g = need(/up to ([\d.]+) SOL/.exec((await syncsLine.textContent()) ?? '')?.[1], 'the shortfall');

        await claim.click();
        expect(await app.simulated()).toBe('review');
        await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'true');
        for (const [i, name] of ['SyncLender', 'ClaimSol'].entries()) {
            await expect(app.inModal(T.txInstruction(String(i)))).toHaveAttribute('data-program', 'short.win');
            await expect(app.inModal(T.txInstruction(String(i)))).toHaveAttribute('data-name', name);
        }
        await expect(app.inModal(T.txInstruction('2'))).toHaveCount(0);
        const lenderPda = await lenderPositionAddress(m.market, app.walletAddress);
        const simClaimed = await app.delta(lenderPda, 'claimed');
        expect(simClaimed).toBeGreaterThan(0n);
        // {g} is shown to 6 decimals, truncated: within 1,000 lamports of the shortfall.
        expect(simClaimed).toBeLessThan(BigInt(Math.round(Number(g) * 1e9)) + 1_000n);
        const signature = await app.signAndConfirm();
        await app.closeModal();

        // Exactly the bundle: the other lender's plain touch, then the claim.
        const landed = need(await landedTransaction(signature), 'landed claim');
        expect(landed.err).toBeNull();
        const intent = intentOf(landed);
        expect(intent.map((ix) => Array.from(ix.data.subarray(0, 8)))).toEqual([
            Array.from(SYNC_LENDER_DISCRIMINATOR),
            Array.from(CLAIM_SOL_DISCRIMINATOR),
        ]);
        expect(Array.from(intent[0]?.data.subarray(8) ?? [])).toEqual([0]); // max_epochs = 0
        const lateLender = await lenderPositionAddress(m.market, late);
        expect(intent[0]?.accounts).toContain(lateLender);
        // KI65-REV-SEC1: the other lender's owner rides along unsigned (sync_lender's `owner`, index
        // 3); a plain touch reads no rebase record, so it needs no signature.
        expect(intent[0]?.accounts[3]).toBe(late);
        expect(landed.signers).not.toContain(late);
        expect(landed.signers).toContain(app.walletAddress);

        const [synced] = await eventsOf(signature, LENDER_SYNCED_EVENT_DISCRIMINATOR, getLenderSyncedEventDecoder());
        expect(need(synced, 'LenderSynced event').owner).toBe(late);
        const [claimEv] = await eventsOf(signature, CLAIM_SOL_EVENT_DISCRIMINATOR, getClaimSolEventDecoder());
        const c = need(claimEv, 'ClaimSol event');
        const after = need(await lenderPosition(m.market, app.walletAddress), 'potClaimer lender position');
        // It pays what the review showed — all of it from the pot: the claim alone paid nothing more.
        expect(c.solPaid).toBe(after.claimed - mid.claimed);
        expect(c.solPaid).toBe(simClaimed);
        expect(c.potDraw).toBeGreaterThan(0n);
        expect(c.solPaid).toBe(c.potDraw);

        // The late lender's surplus left its position: nothing realised beyond its entitlement.
        const l = need(await lenderPosition(m.market, late), 'potLate lender position');
        expect(l.real - l.claimed).toBeLessThanOrEqual(l.pending > 0n ? BigInt(l.pending) : 0n);
        const vault1 = need(await marketVaultAccount(m), 'pot market vault');
        expect(vault1.lenderSol).toBe(vaultMid.lenderSol - c.solPaid);
        // The pot got the late lender's surplus and paid the draw; the rest stays for the next claim.
        expect(c.potDraw).toBeLessThanOrEqual(surplus);
        expect(vault1.unmatched).toBe(vault0.unmatched + surplus - c.potDraw);
    });
});
