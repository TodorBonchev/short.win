/**
 * lend-post-wipe (FRONTEND.md §3.6 Deposit and Claim, FE-18; ARCH §3.3, §3.9; R4A-7, M2-SEC-4).
 * NEEDS the LEND track and the test-venue build (the mock venue): `@lend`.
 *
 * Mock market `wipe` is lent by wallet `wiped` alone and `wipeBorrower` holds its only position
 * (seed.ts). The spec drives an organic full wipe on the validator, then the app's lender flows
 * around it, in order on that one market (`mode: 'serial'`):
 *
 * 1. The wipe, from Node (the lender's and the keeper's part): `wiped` withdraws every free token, a
 *    gap far beyond the S1 boundary, then one `crank_price` + `sol_settle` per second until the last
 *    borrowed tokens settle S1. The epoch rolls into a live full-wipe record (`num = 0`, `S = 0`)
 *    and a plain deposit is refused `WipeSweepPending` (6084) — what the bundle is for.
 * 2. `wiped` claims its S1 SOL in the app while parked on the live record: `claim_sol` carries the
 *    record and pays the settlement's lender amount.
 * 3. SOL lands on the trader side (a donation); `rejoin` deposits in the app: the panel shows the
 *    sweep notice, the review lists `SweepTrader` then `Deposit`, the landed transaction is exactly
 *    that bundle, `TraderSwept` credits the wipe record, and the deposit mints at the initial rate
 *    and finalises the record.
 * 4. `wiped` claims the swept SOL through the final record.
 *
 * A reused validator whose `wipe` market is already spent skips the group (run without
 * SHORTWIN_E2E_REUSE).
 */
import {
    CLAIM_SOL_DISCRIMINATOR,
    CLAIM_SOL_EVENT_DISCRIMINATOR,
    type ClaimSolEvent,
    DEPOSIT_DISCRIMINATOR,
    DEPOSIT_EVENT_DISCRIMINATOR,
    findShareEpochPda,
    getClaimSolEventDecoder,
    getDepositEventDecoder,
    getTraderSweptEventDecoder,
    podU128ToBigInt,
    SHORTWIN_ERROR__WIPE_SWEEP_PENDING,
    SWEEP_TRADER_DISCRIMINATOR,
    TRADER_SWEPT_EVENT_DISCRIMINATOR,
} from '@shortwin/client';
import type { Address } from '@solana/kit';
import { getTransferSolInstruction } from '@solana-program/system';
import { type App, dataValue, expect, need, requireTrading, T, test } from '../fixtures.ts';
import {
    eventsOf,
    fullWipe,
    intentOf,
    landedTransaction,
    lenderPosition,
    lenderPositionAddress,
    lendFromNode,
    marketAccount,
    programErrorNumber,
    shortPosition,
    type WipeOutcome,
} from '../validator/actions.ts';
import { getBalance, getTokenAmount, LAMPORTS_PER_SOL, send } from '../validator/chain.ts';
import { signerFor } from '../validator/keys.ts';

/** `Market.flags` bits (programs/shortwin/src/state/market.rs). */
const WIPE_LIVE = 1 << 5;
const WIPE_TRADER_SWEPT = 1 << 7;
/** `TraderSwept.to` / `wipe_bits` (events.rs, sweep_trader.rs). */
const SWEEP_TO_WIPE_RECORD = 1;
const WIPE_BITS_TRADER_SWEPT = 1;
const WIPE_BITS_LIVE = 0x80;
/** `VS / VA` (ECON §9.2): shares per raw token on an empty pool. */
const INITIAL_RATE = 1_000n;
const DONATION = 1n * LAMPORTS_PER_SOL;
const REJOIN_TOKENS = 1_000n;

test.describe.configure({ mode: 'serial' });

/** What the earlier steps did, for the later ones (one worker, in order). */
let wipe: WipeOutcome | null = null;
let swept: bigint | null = null;

/** Claims in the claim tab of `mint` as the connected wallet; returns the landed `ClaimSol` and the panel's preview. */
async function claimInApp(app: App, mint: Address, market: Address): Promise<{ ev: ClaimSolEvent; shown: bigint }> {
    await app.goto(`/m/${mint}?panel=lend&action=claim`, 'market');
    await app.connectWallet();
    const panel = app.id(T.lendPanel);
    await expect(panel).toHaveAttribute('data-action', 'claim');
    const claimable = panel.getByTestId(T.claimValue('claimable'));
    await expect(claimable).toHaveAttribute('data-value', /^[1-9]\d*$/, { timeout: 60_000 });
    const shown = await dataValue(claimable);
    const before = need(await lenderPosition(market, app.walletAddress), 'wiped lender position');
    await panel.getByTestId(T.claimSubmit).click();
    expect(await app.simulated()).toBe('review');
    await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'true');
    await expect(app.inModal(T.txInstruction('0'))).toHaveAttribute('data-name', 'ClaimSol');
    await expect(app.inModal(T.txInstruction('1'))).toHaveCount(0);
    const lenderPda = await lenderPositionAddress(market, app.walletAddress);
    const simClaimed = await app.delta(lenderPda, 'claimed');
    const signature = await app.signAndConfirm();
    await app.closeModal();

    const landed = need(await landedTransaction(signature), 'landed claim');
    expect(landed.err).toBeNull();
    const [claim] = intentOf(landed);
    expect(Array.from(claim?.data.subarray(0, 8) ?? [])).toEqual(Array.from(CLAIM_SOL_DISCRIMINATOR));
    // The lender is one epoch behind: `claim_sol` carries the wipe record (its last account).
    const [record] = await findShareEpochPda({ market, epoch: before.epoch });
    expect(claim?.accounts.at(-1)).toBe(record);
    const [ev] = await eventsOf(signature, CLAIM_SOL_EVENT_DISCRIMINATOR, getClaimSolEventDecoder());
    const e = need(ev, 'ClaimSol event');
    const after = need(await lenderPosition(market, app.walletAddress), 'wiped lender position');
    expect(e.owner).toBe(app.walletAddress);
    expect(e.solPaid).toBe(after.claimed - before.claimed);
    expect(e.solPaid).toBe(simClaimed);
    return { ev: e, shown };
}

test.describe('lend: the post-wipe sweep bundle and claims through the wipe record (FE-18)', { tag: '@lend' }, () => {
    test.beforeAll(async ({ state }) => {
        if (state.program !== 'test-venue' || !state.positions) return;
        const open = await shortPosition(state.positions.wipe.position);
        test.skip(!open, 'the wipe market is spent (a reused validator): run without SHORTWIN_E2E_REUSE');
    });

    test.describe('the wipe', () => {
        test.use({ wallet: null });

        test('the only lender takes every free token, a gap settles the last borrowed tokens S1: a live wipe record', async ({
            state,
        }) => {
            requireTrading(state);
            test.setTimeout(420_000);
            const m = state.markets.wipe;
            const p = need(state.positions?.wipe, 'wipe position');
            const epoch0 = (await marketAccount(m)).shareEpoch;
            wipe = await fullWipe(m, 'wiped', state.tokenAccounts.wipe.lender, p);
            expect(wipe.withdrawn).toBeGreaterThan(0n);
            expect(wipe.settle.condition).toBe(1);
            expect(wipe.settle.position).toBe(p.position);
            expect(wipe.settle.lenderAmount).toBeGreaterThan(0n);
            expect(wipe.rolled.epoch).toBe(epoch0);
            expect(wipe.rolled.num).toBe(0);
            const mk = await marketAccount(m);
            expect(mk.shareEpoch).toBe(epoch0 + 1);
            expect(podU128ToBigInt(mk.totalShares)).toBe(0n);
            expect([mk.tokensFree, mk.borrowedQ]).toEqual([0n, 0n]);
            expect(mk.flags & WIPE_LIVE).toBe(WIPE_LIVE);
            expect(mk.flags & WIPE_TRADER_SWEPT).toBe(0);
            expect(await shortPosition(p.position)).toBeNull();

            // R4A-7: a deposit alone is refused until the post-wipe sweep runs (what the app bundles).
            const plain = lendFromNode('rejoin', m, state.tokenAccounts.wipe.rejoin, 10n ** BigInt(m.decimals));
            await expect(plain).rejects.toThrow();
            expect(programErrorNumber(await plain.catch((e: unknown) => e))).toBe(SHORTWIN_ERROR__WIPE_SWEEP_PENDING);
        });
    });

    test.describe('the wiped lender', () => {
        test.use({ wallet: 'wiped' });

        test('claims its S1 SOL while parked on the live wipe record', async ({ app, state }) => {
            requireTrading(state);
            const w = need(wipe, 'the wipe of the first test');
            const m = state.markets.wipe;
            const { ev, shown } = await claimInApp(app, m.mint, m.market);
            // The settlement's whole lender amount, less the per-share floor of its allocation.
            expect(ev.solPaid).toBeLessThanOrEqual(w.settle.lenderAmount);
            expect(ev.solPaid).toBeGreaterThanOrEqual(w.settle.lenderAmount - 1_000n);
            expect(ev.solPaid).toBe(shown);
            expect(ev.potDraw).toBe(0n);
        });
    });

    test.describe('a new lender', () => {
        test.use({ wallet: 'rejoin' });

        test('deposits through [sweep_trader, deposit]: the record gets the trader SOL, shares at the initial rate', async ({
            app,
            state,
        }) => {
            requireTrading(state);
            const m = state.markets.wipe;
            const source = state.tokenAccounts.wipe.rejoin;
            const raw = REJOIN_TOKENS * 10n ** BigInt(m.decimals);
            // SOL left on the trader side after the wipe (anyone's donation): the sweep's to recover.
            const payer = await signerFor('payer');
            await send(payer, [getTransferSolInstruction({ source: payer, destination: m.trader, amount: DONATION })]);
            const onTrader = (await getBalance(m.trader)) + (await getBalance(m.traderWsol));
            const epoch = (await marketAccount(m)).shareEpoch;

            await app.goto(`/m/${m.mint}?panel=lend&action=deposit`, 'market');
            await app.connectWallet();
            const panel = app.id(T.lendPanel);
            await expect(panel.getByTestId(T.lendSweepNotice)).toBeVisible();
            await app.selectTokenAccount(panel.getByTestId(T.lendTokenAccount), source);
            await panel.getByTestId(T.lendAmountInput).fill(REJOIN_TOKENS.toString());
            await expect(panel.getByTestId(T.lendPreview('shares'))).toHaveAttribute(
                'data-value',
                (raw * INITIAL_RATE).toString(),
            );
            await panel.getByTestId(T.disclosureAck('lender.solRepayment')).check();
            await app.a11y('deposit into a wiped pool (the sweep notice)');
            await panel.getByTestId(T.lendSubmit).click();

            expect(await app.simulated()).toBe('review');
            await expect(app.inModal(T.txSimStatus)).toHaveAttribute('data-ok', 'true');
            for (const [i, name] of ['SweepTrader', 'Deposit'].entries()) {
                await expect(app.inModal(T.txInstruction(String(i)))).toHaveAttribute('data-program', 'short.win');
                await expect(app.inModal(T.txInstruction(String(i)))).toHaveAttribute('data-name', name);
            }
            await expect(app.inModal(T.txInstruction('2'))).toHaveCount(0);
            expect(await app.delta(source, 'amount')).toBe(-raw);
            const lenderPda = await lenderPositionAddress(m.market, app.walletAddress);
            expect(await app.delta(lenderPda, 'shares')).toBe(raw * INITIAL_RATE);
            await app.ackInReview('lender.solRepayment');
            const before = need(await getTokenAmount(source), 'rejoin balance');
            const signature = await app.signAndConfirm();
            await app.closeModal();

            // The landed transaction is exactly the bundle, top level and in order.
            const landed = need(await landedTransaction(signature), 'landed deposit');
            expect(landed.err).toBeNull();
            const intent = intentOf(landed);
            expect(intent.map((ix) => Array.from(ix.data.subarray(0, 8)))).toEqual([
                Array.from(SWEEP_TRADER_DISCRIMINATOR),
                Array.from(DEPOSIT_DISCRIMINATOR),
            ]);
            // `sweep_trader` names the live wipe record (the latest one) as its last account.
            const [record] = await findShareEpochPda({ market: m.market, epoch: epoch - 1 });
            expect(intent[0]?.accounts.at(-1)).toBe(record);

            const [sweptEv] = await eventsOf(signature, TRADER_SWEPT_EVENT_DISCRIMINATOR, getTraderSweptEventDecoder());
            const s = need(sweptEv, 'TraderSwept event');
            expect(s.lamports).toBe(onTrader);
            expect(s.to).toBe(SWEEP_TO_WIPE_RECORD);
            expect(s.wipeBits).toBe(WIPE_BITS_TRADER_SWEPT | WIPE_BITS_LIVE);
            const [dep] = await eventsOf(signature, DEPOSIT_EVENT_DISCRIMINATOR, getDepositEventDecoder());
            const d = need(dep, 'Deposit event');
            expect(d.shares).toBe(raw * INITIAL_RATE);

            const mk = await marketAccount(m);
            expect(mk.flags & WIPE_LIVE).toBe(0);
            expect(mk.shareEpoch).toBe(epoch);
            expect(podU128ToBigInt(mk.totalShares)).toBe(raw * INITIAL_RATE);
            expect(mk.tokensFree).toBe(raw);
            expect(await getTokenAmount(source)).toBe(before - raw);
            expect(await getBalance(m.trader)).toBe(0n);
            const lp = need(await lenderPosition(m.market, app.walletAddress), 'rejoin lender position');
            expect([lp.shares, lp.epoch]).toEqual([raw * INITIAL_RATE, epoch]);
            swept = s.lamports;
        });
    });

    test.describe('the wiped lender, again', () => {
        test.use({ wallet: 'wiped' });

        test('claims the swept SOL through the final wipe record', async ({ app, state }) => {
            requireTrading(state);
            const sweptSol = need(swept, 'the sweep of the third test');
            const m = state.markets.wipe;
            const { ev, shown } = await claimInApp(app, m.mint, m.market);
            expect(ev.solPaid).toBeLessThanOrEqual(sweptSol);
            expect(ev.solPaid).toBeGreaterThanOrEqual(sweptSol - 1_000n);
            expect(ev.solPaid).toBe(shown);
            // Converted through the record: no shares left, in the current epoch.
            const lp = need(await lenderPosition(m.market, app.walletAddress), 'wiped lender position');
            expect(lp.shares).toBe(0n);
            expect(lp.epoch).toBe((await marketAccount(m)).shareEpoch);
        });
    });
});
