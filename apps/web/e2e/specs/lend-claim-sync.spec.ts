/**
 * lend-claim-sync (FRONTEND.md §3.6 Claim, FE-18; ECON §9.4; R3A-2). NEEDS the LEND track and the
 * test-venue build (the mock venue): `@lend`.
 *
 * Mock market `pot` is lent by wallet `potClaimer` alone and `potBorrower` holds a position there
 * from the seed on, so rent is credited to `potClaimer` (`acc`) every second. Then `potLate` lends
 * nine times as much and the borrower closes: the rent is paid in and realised (`acc_r`) across
 * both lenders' shares, so `potLate` holds realised SOL beyond its entitlement — a surplus that
 * moves to the pot only when it is touched — and `potClaimer`'s entitlement is mostly not realised
 * on its own position (a shortfall the empty pot cannot pay).
 *
 * The claim in the app reads the other lenders from the indexer, judges each with `previewTouch`
 * and bundles `[sync_lender(potLate), claim_sol]`: the landed transaction is exactly that, the sync
 * moves the surplus to the pot, the claim draws it, and the claimer is paid more than the same claim
 * alone pays (simulated on the validator first).
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
    simulateSyncLender,
} from '../validator/actions.ts';
import { waitForClock } from '../validator/chain.ts';
import { signerFor } from '../validator/keys.ts';

test.use({ wallet: 'potClaimer' });

/** `potLate`'s deposit: nine times `potClaimer`'s (seed.ts), so ~90 % of the realised rent is its. */
const LATE_TOKENS = 5_400_000n;
/** Seconds of rent credited to `potClaimer` alone before `potLate` lends (at least). */
const ALONE_S = 30n;

test.describe('lend: a claim syncs other lenders first (FE-18)', { tag: '@lend' }, () => {
    test('the pot cannot pay the claim: [sync_lender(other), claim_sol] lands and pays more than the claim alone', async ({
        app,
        state,
    }) => {
        requireTrading(state);
        const m = state.markets.pot;
        const p = need(state.positions?.pot, 'pot position');
        const open = await shortPosition(p.position);
        test.skip(!open, 'the pot market is spent (a reused validator): run without SHORTWIN_E2E_REUSE');
        const late = state.wallets.potLate.address;

        // Rent credited to potClaimer alone, then potLate lends, then the rent is paid in.
        await waitForClock(need(open, 'pot position').tOpen + ALONE_S);
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

        await app.goto(`/m/${m.mint}?panel=lend&action=claim`, 'market');
        await app.connectWallet();
        const panel = app.id(T.lendPanel);
        await expect(panel).toHaveAttribute('data-action', 'claim');
        const claimable = panel.getByTestId(T.claimValue('claimable'));
        await expect(claimable).toHaveAttribute('data-value', /^[1-9]\d*$/, { timeout: 60_000 });
        await expect(panel.getByTestId(T.claimValue('pot'))).toHaveAttribute('data-value', '0');
        // The panel's preview is the claim alone (the pot as it is).
        expect(await dataValue(claimable)).toBe(alone);
        await app.a11y('claim tab with a shortfall the pot cannot pay');

        const before = need(await lenderPosition(m.market, app.walletAddress), 'potClaimer lender position');
        await panel.getByTestId(T.claimSubmit).click();
        expect(await app.simulated()).toBe('review');
        await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'true');
        for (const [i, name] of ['SyncLender', 'ClaimSol'].entries()) {
            await expect(app.inModal(T.txInstruction(String(i)))).toHaveAttribute('data-program', 'short.win');
            await expect(app.inModal(T.txInstruction(String(i)))).toHaveAttribute('data-name', name);
        }
        await expect(app.inModal(T.txInstruction('2'))).toHaveCount(0);
        const lenderPda = await lenderPositionAddress(m.market, app.walletAddress);
        const simClaimed = await app.delta(lenderPda, 'claimed');
        expect(simClaimed).toBeGreaterThan(alone);
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

        const [synced] = await eventsOf(signature, LENDER_SYNCED_EVENT_DISCRIMINATOR, getLenderSyncedEventDecoder());
        expect(need(synced, 'LenderSynced event').owner).toBe(late);
        const [claimEv] = await eventsOf(signature, CLAIM_SOL_EVENT_DISCRIMINATOR, getClaimSolEventDecoder());
        const c = need(claimEv, 'ClaimSol event');
        const after = need(await lenderPosition(m.market, app.walletAddress), 'potClaimer lender position');
        expect(c.solPaid).toBe(after.claimed - before.claimed);
        expect(c.solPaid).toBe(simClaimed);
        // The pot paid the difference: the claim alone stops at the claimer's own realised SOL.
        expect(c.potDraw).toBeGreaterThan(0n);
        expect(c.solPaid).toBe(alone + c.potDraw);

        // The late lender's surplus left its position: nothing realised beyond its entitlement.
        const l = need(await lenderPosition(m.market, late), 'potLate lender position');
        expect(l.real - l.claimed).toBeLessThanOrEqual(l.pending > 0n ? BigInt(l.pending) : 0n);
        const vault1 = need(await marketVaultAccount(m), 'pot market vault');
        expect(vault1.lenderSol).toBe(vault0.lenderSol - c.solPaid);
        // The pot got the late lender's surplus and paid the draw; the rest stays for the next claim.
        expect(c.potDraw).toBeLessThanOrEqual(surplus);
        expect(vault1.unmatched).toBe(vault0.unmatched + surplus - c.potDraw);
    });
});
