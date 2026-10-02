/**
 * create-market (FRONTEND.md §3.5; ARCH §3.2 check 1; FE-D5). NEEDS the MARKETS track's
 * create-market page and the SHORT track's venue layer (`detectVenue`, `raydiumCandidates`):
 * `@markets`.
 *
 * 1. Safety verdicts computed in the browser from the mint account: ok, freeze authority
 *    (`MintFreezeAuthority`), a disallowed Token-2022 extension (`MintDisallowedExtension`), and a
 *    mint that already has a market (`existing`, linking to it).
 * 2. A list-only listing through the pipeline: cost, the `market.create` disclosure, the creator's
 *    SOL in the review, the confirmed market, navigation to `/m/<mint>`.
 * 3. Venue detection on the dumped mainnet accounts (SHORTWIN_E2E_VENUES=1): a live pump curve, and
 *    Raydium AMM v4 / CPMM candidates for mints without a pump venue.
 */

import type { Address } from '@solana/kit';
import { DISCLOSURES } from '../../src/config/disclosures.ts';
import { dataValue, expect, need, requireVenueFixtures, T, test } from '../fixtures.ts';
import { getAccount, getBalance, marketKeys, PROGRAMS } from '../validator/chain.ts';
import { fixtureAddress, rentExempt } from '../validator/genesis.ts';

test.use({ wallet: 'creator' });

test.describe('create market', { tag: '@markets' }, () => {
    test('safety verdicts: ok, freeze authority, disallowed extension, existing', async ({ app, state }) => {
        const { ok, freeze, badExtension } = state.createMints;
        const cases: [Address, string, string | null][] = [
            [ok, 'ok', null],
            [freeze, 'reject', 'MintFreezeAuthority'],
            [badExtension, 'reject', 'MintDisallowedExtension'],
            [state.markets.main.mint, 'existing', null],
        ];
        for (const [mint, verdict, reason] of cases) {
            await app.goto(`/markets/new?mint=${mint}`, 'create-market');
            await expect(app.id(T.createMintInput)).toHaveValue(mint);
            await expect(app.id(T.createVerdict)).toHaveAttribute('data-verdict', verdict);
            if (reason) await expect(app.id(T.createReason(reason))).toBeVisible();
            if (verdict === 'reject') {
                const submit = app.id(T.createSubmit);
                if (await submit.count()) await expect(submit).toBeDisabled();
            }
            if (verdict === 'existing') {
                await expect(app.id(T.createExisting)).toHaveAttribute('href', new RegExp(`/m/${mint}`));
            }
        }
        await app.a11y('create market, existing');

        // Typing a mint re-checks it.
        await app.goto('/markets/new', 'create-market');
        await app.id(T.createMintInput).fill(freeze);
        await expect(app.id(T.createVerdict)).toHaveAttribute('data-verdict', 'reject');
        await expect(app.id(T.createReason('MintFreezeAuthority'))).toBeVisible();
    });

    test('a list-only listing through the pipeline', async ({ app, state, page }) => {
        const mint = state.createMints.listable;
        const keys = await marketKeys(mint, PROGRAMS.token);
        expect(await getAccount(keys.market)).toBeNull();
        await app.goto(`/markets/new?mint=${mint}`, 'create-market');
        await app.connectWallet();
        await expect(app.id(T.createVerdict)).toHaveAttribute('data-verdict', 'ok');
        await expect(app.id(T.createVenue)).toHaveAttribute('data-value', 'none');
        const cost = await dataValue(app.id(T.createCost));
        // ≥ the creation fee (ECON §15: 0.005 SOL) plus the rent of Market (3,104 B) and MarketVault (2,200 B).
        expect(cost).toBeGreaterThan(5_000_000n + rentExempt(3_104) + rentExempt(2_200));
        await expect(app.id(T.disclosure('market.create')).first()).toContainText(
            DISCLOSURES['market.create'].body[0] ?? '',
        );
        await app.a11y('create market, ok');

        await app.id(T.createSubmit).click();
        expect(await app.simulated()).toBe('review');
        await expect(app.inModal(T.disclosure('market.create'))).toBeVisible();
        const sim = await app.delta(app.walletAddress, 'lamports');
        expect(-sim).toBeGreaterThanOrEqual(cost);
        const before = await getBalance(app.walletAddress);
        // On confirmation the page moves to the new market (§3.5 step 5), closing the review.
        await expect(app.inModal(T.txSign)).toBeEnabled();
        await app.inModal(T.txSign).click();
        await expect(page).toHaveURL(new RegExp(`/m/${mint}`), { timeout: 90_000 });
        expect(await getAccount(keys.market)).not.toBeNull();
        // The landed cost is the simulated one (up to the signature fee, if the simulation omits it).
        const spent = before - (await getBalance(app.walletAddress));
        expect(spent - -sim).toBeLessThanOrEqual(10_000n);
        expect(-sim - spent).toBeLessThanOrEqual(10_000n);
        await expect(app.id(T.page('market'))).toBeVisible();
        await expect(app.id(T.marketFlag('list-only'))).toBeVisible();
    });

    test('venue detection on the dumped mainnet accounts', async ({ app, state }) => {
        requireVenueFixtures(state);
        const v = need(state.venueMints, 'venue fixture mints');
        await app.goto(`/markets/new?mint=${v.pumpCurve}`, 'create-market');
        await expect(app.id(T.createVenue)).toHaveAttribute('data-value', 'pump_curve');

        for (const [mint, pool] of [
            [v.raydiumV4, fixtureAddress('ray_v4_fartcoin_pool')],
            [v.raydiumCpmm, fixtureAddress('cpmm_useless_pool')],
        ] as const) {
            await app.goto(`/markets/new?mint=${mint}`, 'create-market');
            await expect(app.id(T.createPools)).toBeVisible();
            const row = app.id(T.createPoolRow(pool));
            await expect(row).toBeVisible();
            await expect(row).toHaveAttribute('data-eligible', /^(true|false)$/);
        }
        await app.a11y('create market, Raydium candidates');
    });
});
