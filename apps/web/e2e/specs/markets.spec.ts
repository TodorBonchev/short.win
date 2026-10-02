/**
 * markets (FRONTEND.md §3.3 markets table, §6.4 live rows). NEEDS the MARKETS track's markets
 * table: `@markets`.
 *
 * The seeded markets are listed; the venue and tier filters and the sort headers drive the
 * indexer query; a `markets` summary pushed on the WebSocket patches its row in place; a row opens
 * its market with Enter; a searched mint without a market offers "Create market".
 */

import { getDepositInstructionAsync, SHORTWIN_PROGRAM_ADDRESS } from '@shortwin/client';
import { getCreateAssociatedTokenIdempotentInstructionAsync, getMintToInstruction } from '@solana-program/token';
import { expect, need, T, test } from '../fixtures.ts';
import { ata, eventAuthority, PROGRAMS, send } from '../validator/chain.ts';
import { signerFor } from '../validator/keys.ts';

test.use({ wallet: null });

test.describe('markets table', { tag: '@markets' }, () => {
    test('rows, filters and sort', async ({ app, state, indexerMock }) => {
        const { main, move, paused, t22 } = state.markets;
        await app.goto('/markets', 'markets');
        await expect(app.id(T.marketsTable)).toBeVisible();
        for (const m of [main, move, paused, t22]) await expect(app.id(T.marketsRow(m.mint))).toBeVisible();
        await app.a11y('markets table');

        await app.id(T.marketsFilterVenue).selectOption('none');
        await expect(app.id(T.marketsRow(t22.mint))).toBeVisible();
        await expect(app.id(T.marketsRow(main.mint))).toHaveCount(0);
        await app.id(T.marketsFilterVenue).selectOption({ index: 0 });
        await expect(app.id(T.marketsRow(main.mint))).toBeVisible();

        await app.id(T.marketsFilterTier).selectOption('C');
        await expect(app.id(T.marketsRow(main.mint))).toBeVisible();
        await app.id(T.marketsFilterTier).selectOption({ index: 0 });

        for (const key of ['oi', 'util', 'created', 'open_positions', 'tvl'] as const) {
            await app.id(T.marketsSort(key)).click();
            await expect(app.page).toHaveURL(new RegExp(`sort=${key}`));
            await expect(app.page.locator('th', { has: app.id(T.marketsSort(key)) })).toHaveAttribute(
                'aria-sort',
                /^(ascending|descending)$/,
            );
            await expect
                .poll(() => need(indexerMock, 'indexer mock').requests.some((r) => r.includes(`sort=${key}`)))
                .toBe(true);
        }
    });

    test('a markets summary on the WebSocket patches its row in place', async ({ app, state, indexerMock }) => {
        const move = state.markets.move;
        await app.goto('/markets', 'markets');
        await expect(app.id(T.statusWs)).toHaveAttribute('data-state', 'open');
        const row = app.id(T.marketsRow(move.mint));
        await expect(row).toBeVisible();
        const before = await row.innerText();
        // REST keeps serving the old summary: only the WebSocket can change the row.
        await need(indexerMock, 'indexer mock').freezeMarket(move.mint);

        // A new deposit changes the market's inventory and TVL on chain…
        const payer = await signerFor('payer');
        const lender = await signerFor('seedLender');
        const mintAuthority = await signerFor('mintAuthority');
        const src = await ata(lender.address, move.mint, PROGRAMS.token);
        const amount = 5_000_000n * 10n ** BigInt(move.decimals);
        await send(payer, [
            await getCreateAssociatedTokenIdempotentInstructionAsync({ payer, owner: lender.address, mint: move.mint }),
            getMintToInstruction({ mint: move.mint, token: src, mintAuthority, amount }),
        ]);
        await send(lender, [
            await getDepositInstructionAsync({
                owner: lender,
                mint: move.mint,
                ownerTokenAccount: src,
                tokenProgram: PROGRAMS.token,
                eventAuthority: await eventAuthority(),
                program: SHORTWIN_PROGRAM_ADDRESS,
                amount,
                minShares: 0n,
            }),
        ]);
        // …and the indexer pushes the new summary on `markets`: the row is patched in place.
        await need(indexerMock, 'indexer mock').pushMarket(move.mint);
        await expect.poll(() => row.innerText(), { timeout: 15_000 }).not.toBe(before);
    });

    test('Enter opens a market; a mint without a market offers Create market', async ({ app, state, page }) => {
        const main = state.markets.main;
        await app.goto('/markets', 'markets');
        const row = app.id(T.marketsRow(main.mint));
        await row.focus();
        await page.keyboard.press('Enter');
        await expect(page).toHaveURL(new RegExp(`/m/${main.mint}`));
        await expect(app.id(T.page('market'))).toBeVisible();
        await expect(app.id(T.marketMint)).toHaveAttribute('data-value', main.mint);

        const fresh = state.createMints.ok;
        await app.goto(`/markets?q=${fresh}`, 'markets');
        await expect(app.id(T.marketsEmpty)).toBeVisible();
        const link = app.id(T.marketsCreateLink);
        await expect(link).toHaveAttribute('href', new RegExp(`/markets/new\\?mint=${fresh}`));
        await link.click();
        await expect(app.id(T.page('create-market'))).toBeVisible();
        await expect(app.id(T.createMintInput)).toHaveValue(fresh);
    });
});
