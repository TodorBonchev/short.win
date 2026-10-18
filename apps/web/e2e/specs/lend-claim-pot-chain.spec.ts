/**
 * lend-claim-pot-chain (FRONTEND.md §3.6 Claim, KI66-R4; ECON §9.4 R3A-2, §9.5; ARCH I-P6; the
 * owner's eighth round, KI-66 full). NEEDS the LEND track and the test-venue build (the mock venue):
 * `@lend`.
 *
 * Mock market `chain` is lent by wallet `chainAbsent` (C) alone and `chainBorrower` borrows 15M of
 * its tokens from the seed on. The spec rebases the pool's shares on the validator, then has C stay
 * away while its unconverted shares earn more realised SOL than C's credit, and the claimer A's
 * claim in the app sends C's pot-only sync first:
 *
 * 1. From Node: C withdraws the free tokens down to 12,000 (fewer than a thousandth of the claims);
 *    a 1,000× gap settles the borrowed tokens S1, which rolls the epoch with a rebase record
 *    (`num = 1`, `1/1,000`): C now holds old-epoch shares behind it and never converts them.
 * 2. A confirms the app's separate pool cleanup (only `sweep_trader`), then lends as many tokens
 *    in the new epoch (worth C's at the new price); the borrower borrows
 *    again (once the EMA admits an open); rent is credited to A's shares and C's for 300 s; A takes
 *    part of its tokens out; the borrower closes and pays the rent in, realised on the shares still
 *    there: C's unconverted shares get more than C's credit (a surplus after the record), A less
 *    (a shortfall the empty pot cannot pay).
 * 3. In the app, A's claim reads the other lenders from the indexer, judges C with the WASM
 *    `previewTouchPot` on the record from C's epoch and bundles `[sync_lender(C, 1), claim_sol]`:
 *    the sync carries the record read-only and C's owner unsigned; the review shows the sync, its
 *    disclosure and the simulated payout; the landed claim pays A's full shortfall from the pot; C's
 *    shares, epoch, lock and the record stay, its forward chain is open (`fwd ≠ 0`) and reached the
 *    share epoch.
 */
import {
    CLAIM_SOL_DISCRIMINATOR,
    CLAIM_SOL_EVENT_DISCRIMINATOR,
    findShareEpochPda,
    getClaimSolEventDecoder,
    getLenderSyncedEventDecoder,
    getShareEpochDecoder,
    LENDER_SYNCED_EVENT_DISCRIMINATOR,
    type LenderPosition,
    podU128ToBigInt,
    SWEEP_TRADER_DISCRIMINATOR,
    SYNC_LENDER_DISCRIMINATOR,
} from '@shortwin/client';
import { dataValue, expect, need, requireTrading, T, test } from '../fixtures.ts';
import {
    closeShortFromNode,
    eventsOf,
    gapSettle,
    intentOf,
    landedTransaction,
    lenderPosition,
    lenderPositionAddress,
    lendFromNode,
    marketAccount,
    marketVaultAccount,
    openShortWhenAllowed,
    shortPosition,
    simulateClaimSol,
    simulatePotSync,
    withdrawFreeDownTo,
    withdrawTokensFromNode,
} from '../validator/actions.ts';
import { getAccount, waitForClock } from '../validator/chain.ts';
import { signerFor } from '../validator/keys.ts';

/** Free tokens C leaves in the pool before the gap: < 15M/1,000 of the borrowed, so the shares rebase. */
const KEEP_TOKENS = 12_000n;
/**
 * A 1,000× gap: S1 for the borrowed tokens, and C's 12,000 tokens worth ≈ 12 SOL after it — dear
 * enough that the rent of the second position leaves A a shortfall above the claim's 10,000-lamport
 * dust threshold (`CLAIM_DUST_LAMPORTS`, the owner's decision of 2026-10-11; at 100× and 90 s it was ≈ 470, at
 * 1,000× and 240 s ≈ 12,000).
 */
const GAP_BPS = 9_990_000;
/** A's deposit after the rebase (as many tokens as C kept) and the position borrowed again (87.5 %). */
const CLAIMER_TOKENS = 12_000n;
const BORROW_TOKENS = 21_000n;
/** Seconds of rent credited to A and C's unconverted shares before A takes tokens out. */
const RENT_S = 300n;
/** What A takes out before the rent is paid in (within its allowance). */
const WITHDRAW_TOKENS = 1_000n;
/** `FWD_HOLD` (`shortwin_math::pool`): held at the record, the forward chain walked. */
const FWD_HOLD = 1;
/** `Market.flags`: the trader source is complete after the post-settlement cleanup. */
const WIPE_TRADER_SWEPT = 1 << 7;

test.describe.configure({ mode: 'serial' });

/** What the first test measured, for the second (one worker, in order). */
let setup: {
    /** C's position before A's claim (old-epoch shares behind the record). */
    readonly absent: LenderPosition;
    /** The record C stands behind and its `conv_rem`. */
    readonly recordEpoch: number;
    readonly convRem: bigint;
    /** A's claim alone (simulated): its own realised SOL, the pot empty. */
    readonly alone: bigint;
    /** What C's pot-only sync moves (simulated). */
    readonly surplus: bigint;
} | null = null;

test.describe('lend: a claim syncs an absent lender behind a rebase record first (KI66-R4)', { tag: '@lend' }, () => {
    test.beforeAll(async ({ state }) => {
        if (state.program !== 'test-venue' || !state.positions) return;
        const open = await shortPosition(state.positions.chain.position);
        test.skip(!open, 'the chain market is spent (a reused validator): run without SHORTWIN_E2E_REUSE');
    });

    test.describe('the rebase and the absent lender’s surplus', () => {
        test.use({ wallet: 'chainClaimer' });

        test('a gap rebases the shares behind C; rent realised after the record leaves C a surplus and A a shortfall', async ({
            app,
            state,
        }) => {
            requireTrading(state);
            test.setTimeout(900_000);
            const m = state.markets.chain;
            const p = need(state.positions?.chain, 'chain position');
            const unit = 10n ** BigInt(m.decimals);
            const c = state.wallets.chainAbsent.address;

            // 1. The rebase: C keeps a thousandth's worth of free tokens; the gap settles the rest S1.
            const epoch0 = (await marketAccount(m)).shareEpoch;
            await withdrawFreeDownTo('chainAbsent', m, state.tokenAccounts.chain.absent, KEEP_TOKENS * unit);
            const g = await gapSettle(m, p, GAP_BPS);
            expect(g.settle.condition).toBe(1);
            expect([g.rolled.epoch, g.rolled.num, g.rolled.den]).toEqual([epoch0, 1, 1_000n]);
            const mk = await marketAccount(m);
            expect(mk.shareEpoch).toBe(epoch0 + 1);
            expect([mk.tokensFree, mk.borrowedQ]).toEqual([KEEP_TOKENS * unit, 0n]);
            expect(await shortPosition(p.position)).toBeNull();
            const c0 = need(await lenderPosition(m.market, c), 'C lender position');
            expect(c0.epoch).toBe(epoch0);
            expect(c0.fwd).toBe(0);

            // The dust flush requires its pending trader source to be swept before a new lender
            // joins, even though C still has shares. The app reviews that cleanup separately:
            // a later reward claim may vest, so a deposit must not roll the cleanup back.
            await app.goto(`/m/${m.mint}?panel=lend&action=deposit`, 'market');
            await app.connectWallet();
            const panel = app.id(T.lendPanel);
            await expect(panel.getByTestId(T.lendSweepNotice)).toBeVisible();
            const cleanup = panel.getByTestId(T.lendCleanupSubmit);
            await expect(cleanup).toBeEnabled();
            await cleanup.click();
            expect(await app.simulated()).toBe('review');
            await expect(app.modal).toContainText('Clean up pending lender income');
            await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'true');
            await expect(app.inModal(T.txInstruction('0'))).toHaveAttribute('data-name', 'SweepTrader');
            await expect(app.inModal(T.txInstruction('1'))).toHaveCount(0);
            await app.a11y('surviving-share pool cleanup review');
            const cleanupSignature = await app.signAndConfirm();
            await app.closeModal();

            const cleanupLanded = need(await landedTransaction(cleanupSignature), 'landed pool cleanup');
            expect(cleanupLanded.err).toBeNull();
            expect(intentOf(cleanupLanded).map((ix) => Array.from(ix.data.subarray(0, 8)))).toEqual([
                Array.from(SWEEP_TRADER_DISCRIMINATOR),
            ]);
            const cleaned = await marketAccount(m);
            expect(cleaned.flags & WIPE_TRADER_SWEPT).toBe(WIPE_TRADER_SWEPT);
            expect([cleaned.shareEpoch, podU128ToBigInt(cleaned.totalShares)]).toEqual([
                mk.shareEpoch,
                podU128ToBigInt(mk.totalShares),
            ]);
            await expect(cleanup).toHaveCount(0);

            // 2. A lends in the new epoch; the position borrows again; rent for RENT_S; A takes part out.
            await lendFromNode('chainClaimer', m, state.tokenAccounts.chain.claimer, CLAIMER_TOKENS * unit);
            const borrower = await signerFor('chainBorrower');
            const p2 = await openShortWhenAllowed(borrower, m, BORROW_TOKENS * unit, 2n);
            const open = need(await shortPosition(p2.position), 'the second position');
            await waitForClock(open.tOpen + RENT_S, (Number(RENT_S) + 120) * 1_000);
            await withdrawTokensFromNode('chainClaimer', m, state.tokenAccounts.chain.claimer, WITHDRAW_TOKENS * unit);
            await closeShortFromNode(borrower, m, p2);
            expect(await shortPosition(p2.position)).toBeNull();

            // C never came back: its position is still the old epoch's, no chain yet.
            const absent = need(await lenderPosition(m.market, c), 'C lender position');
            expect([absent.epoch, absent.shares, absent.fwd]).toEqual([c0.epoch, c0.shares, 0]);
            const vault = need(await marketVaultAccount(m), 'chain market vault');
            expect(vault.unmatched).toBe(0n);
            // A's claim alone pays its own realised SOL; C's sync would move a surplus to the pot.
            const alone = await simulateClaimSol('chainClaimer', m);
            // A's shortfall (credited, neither its own realised SOL nor the empty pot pays it): above
            // the claim's 10,000-lamport dust threshold, so the claim looks for syncs.
            const touchedA = (await simulatePotSync('chainClaimer', m, [])).lender;
            const shortA = BigInt(touchedA.pending) - (touchedA.real - touchedA.claimed);
            console.log(`[lend-claim-pot-chain] A's credit ${touchedA.pending}, alone ${alone}, shortfall ${shortA}`);
            expect(shortA).toBeGreaterThanOrEqual(10_000n);
            const sim = await simulatePotSync('chainAbsent', m, [epoch0]);
            const surplus = sim.vault.unmatched - vault.unmatched;
            expect(surplus).toBeGreaterThan(0n);
            expect([sim.lender.epoch, sim.lender.shares, sim.lender.fwd]).toEqual([
                absent.epoch,
                absent.shares,
                FWD_HOLD,
            ]);
            expect(sim.lender.fwdEpoch).toBe(mk.shareEpoch);
            const [recordAddress] = await findShareEpochPda({ market: m.market, epoch: epoch0 });
            const record = getShareEpochDecoder().decode(need(await getAccount(recordAddress), 'the record').data);
            setup = { absent, recordEpoch: epoch0, convRem: record.convRem, alone, surplus };
        });
    });

    test.describe('the claimer', () => {
        test.use({ wallet: 'chainClaimer' });

        test('claims with [sync_lender(C) unsigned, read-only record, claim_sol]: paid in full, C unconverted', async ({
            app,
            state,
        }) => {
            requireTrading(state);
            const s = need(setup, 'the first test');
            const m = state.markets.chain;
            const c = state.wallets.chainAbsent.address;
            const vault0 = need(await marketVaultAccount(m), 'chain market vault');
            const before = need(await lenderPosition(m.market, app.walletAddress), 'A lender position');

            await app.goto(`/m/${m.mint}?panel=lend&action=claim`, 'market');
            await app.connectWallet();
            const panel = app.id(T.lendPanel);
            await expect(panel).toHaveAttribute('data-action', 'claim');
            const claimable = panel.getByTestId(T.claimValue('claimable'));
            await expect(claimable).toHaveAttribute('data-value', /^\d+$/, { timeout: 60_000 });
            await expect(panel.getByTestId(T.claimValue('pot'))).toHaveAttribute('data-value', '0');
            // The panel's preview is the claim alone (the pot as it is).
            expect(await dataValue(claimable)).toBe(s.alone);
            await app.a11y('claim tab with a shortfall an absent lender behind a rebase can fund');

            await panel.getByTestId(T.claimSubmit).click();
            expect(await app.simulated()).toBe('review');
            await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'true');
            for (const [i, name] of ['SyncLender', 'ClaimSol'].entries()) {
                await expect(app.inModal(T.txInstruction(String(i)))).toHaveAttribute('data-program', 'short.win');
                await expect(app.inModal(T.txInstruction(String(i)))).toHaveAttribute('data-name', name);
            }
            await expect(app.inModal(T.txInstruction('2'))).toHaveCount(0);
            await expect(app.inModal(T.disclosure('lender.potSyncs'))).toBeVisible();
            await expect(app.inModal(T.disclosure('lender.claimCloses'))).toHaveCount(0);
            const lenderPda = await lenderPositionAddress(m.market, app.walletAddress);
            const simClaimed = await app.delta(lenderPda, 'claimed');
            expect(simClaimed).toBeGreaterThan(s.alone);
            await app.a11y('claim review with a pot-only sync first');
            const signature = await app.signAndConfirm();
            await app.closeModal();

            // Exactly the bundle: C's pot-only sync through its record, then the claim.
            const landed = need(await landedTransaction(signature), 'landed claim');
            expect(landed.err).toBeNull();
            const intent = intentOf(landed);
            expect(intent.map((ix) => Array.from(ix.data.subarray(0, 8)))).toEqual([
                Array.from(SYNC_LENDER_DISCRIMINATOR),
                Array.from(CLAIM_SOL_DISCRIMINATOR),
            ]);
            const sync = need(intent[0], 'the sync');
            expect(Array.from(sync.data.subarray(8))).toEqual([1]); // max_epochs = 1: the record of C's epoch
            expect(sync.accounts).toContain(await lenderPositionAddress(m.market, c));
            // KI65-REV-SEC1: C's owner rides along unsigned (sync_lender's `owner`, index 3).
            expect(sync.accounts[3]).toBe(c);
            expect(landed.signers).not.toContain(c);
            expect(landed.signers).toContain(app.walletAddress);
            // The record goes read-only (the pot-only touch writes no record, KI-66 full).
            const [record] = await findShareEpochPda({ market: m.market, epoch: s.recordEpoch });
            expect(sync.accounts.at(-1)).toBe(record);
            expect(landed.writable).not.toContain(record);

            const [synced] = await eventsOf(
                signature,
                LENDER_SYNCED_EVENT_DISCRIMINATOR,
                getLenderSyncedEventDecoder(),
            );
            expect(need(synced, 'LenderSynced event').owner).toBe(c);
            const [claimEv] = await eventsOf(signature, CLAIM_SOL_EVENT_DISCRIMINATOR, getClaimSolEventDecoder());
            const ev = need(claimEv, 'ClaimSol event');
            const after = need(await lenderPosition(m.market, app.walletAddress), 'A lender position');
            expect(ev.solPaid).toBe(after.claimed - before.claimed);
            expect(ev.solPaid).toBe(simClaimed);
            // The pot paid the rest: the claim alone stops at A's own realised SOL.
            expect(ev.potDraw).toBeGreaterThan(0n);
            expect(ev.solPaid).toBe(s.alone + ev.potDraw);
            expect(ev.potDraw).toBeLessThanOrEqual(s.surplus);
            // A's full shortfall: nothing it was credited stays unpaid, but for rounding carries.
            expect(after.pending).toBeGreaterThanOrEqual(0n);
            expect(after.pending).toBeLessThanOrEqual(2n);
            const vault1 = need(await marketVaultAccount(m), 'chain market vault');
            expect(vault1.unmatched).toBe(vault0.unmatched + s.surplus - ev.potDraw);
            expect(vault1.lenderSol).toBe(vault0.lenderSol - ev.solPaid);
            console.log(
                `[lend-claim-pot-chain] alone ${s.alone}, C's surplus ${s.surplus}, pot draw ${ev.potDraw}, paid ${ev.solPaid}, A's pending left ${after.pending}`,
            );

            // C is not converted: shares, epoch, lock and the record's conv_rem stay; its forward
            // chain holds and reached the share epoch; nothing realised beyond its credit is left.
            const cl = need(await lenderPosition(m.market, c), 'C lender position');
            expect([cl.shares, cl.epoch, cl.lock, cl.lockTerm, cl.lockUntil]).toEqual([
                s.absent.shares,
                s.absent.epoch,
                s.absent.lock,
                s.absent.lockTerm,
                s.absent.lockUntil,
            ]);
            expect(cl.fwd).toBe(FWD_HOLD);
            const mk = await marketAccount(m);
            expect(cl.fwdEpoch).toBe(mk.shareEpoch);
            expect(cl.fwdShares).toBe(s.absent.shares / 1_000n);
            expect(cl.real - cl.claimed).toBeLessThanOrEqual(cl.pending > 0n ? BigInt(cl.pending) : 0n);
            const rec = getShareEpochDecoder().decode(need(await getAccount(record), 'the record').data);
            expect(rec.convRem).toBe(s.convRem);
            expect(podU128ToBigInt(mk.totalShares)).toBeGreaterThan(after.shares);
            // A second sync of C moves nothing more (idempotent).
            const again = await simulatePotSync('chainAbsent', m, [s.recordEpoch]);
            expect(again.vault.unmatched).toBe(vault1.unmatched);
        });
    });
});
