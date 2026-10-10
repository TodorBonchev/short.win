/**
 * The MARKETS pages' responsive behaviour (M10-RESP; FRONTEND.md §7.6, §10.1) in jsdom: the column
 * metadata the table cards and priorities read, the phone filter toggle and "Clear filters", the
 * market page's action bar and trade sheet (a URL key, modal semantics, Escape), a "Lend only"
 * shown once, the stat strip's priorities and "More stats", the all-zero settlements line, and the
 * address input attributes. Layout itself is CSS (the phone E2E and the responsive capture).
 */
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { T } from '../../config/testids.ts';
import * as accounts from '../../rpc/accounts.ts';
import { resetMedia, setMedia } from '../../test/media.ts';
import * as chainReads from './shared/chainReads.ts';
import {
    chainMarket,
    detail,
    exists,
    globalConfig,
    MINT_A,
    MINT_B,
    missing,
    oracleCache,
    summary,
} from './test/fixtures.ts';
import { mockIndexer, renderAt } from './test/harness.tsx';

vi.mock('../../rpc/accounts.ts', async (orig) => ({
    ...(await orig<typeof import('../../rpc/accounts.ts')>()),
    readMarket: vi.fn(),
    readGlobalConfig: vi.fn(),
    readVenueGuard: vi.fn(),
    readOracle: vi.fn(),
}));
vi.mock('./shared/chainReads.ts', async (orig) => ({
    ...(await orig<typeof import('./shared/chainReads.ts')>()),
    readRawAccount: vi.fn(),
}));

const POOLS_NONE = { mint: MINT_A, current: null, window: null, pools: [], events: [] };

function marketIndexer(over: Record<string, unknown> = {}) {
    return mockIndexer({
        [`/v1/markets/${MINT_A}`]: detail({ symbol: 'BONK', name: 'Bonk Inu' }),
        [`/v1/markets/${MINT_A}/pools`]: POOLS_NONE,
        [`/v1/markets/${MINT_A}/lenders`]: { data: [], next_cursor: null },
        ...over,
    });
}

beforeEach(() => {
    vi.mocked(accounts.readMarket).mockResolvedValue(exists(MINT_A, chainMarket()));
    vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, globalConfig()));
    vi.mocked(accounts.readVenueGuard).mockResolvedValue(missing(MINT_A));
    vi.mocked(accounts.readOracle).mockResolvedValue(exists(MINT_A, oracleCache()));
    vi.mocked(chainReads.readRawAccount).mockResolvedValue(null);
});

afterEach(() => {
    vi.unstubAllGlobals();
    resetMedia();
});

describe('markets list', () => {
    it('labels its cells for the cards and ranks its columns', async () => {
        mockIndexer({ '/v1/markets': { data: [summary({ mint: MINT_A, symbol: 'BONK' })], next_cursor: null } });
        const { queryClient } = await renderAt('/markets');
        const row = await screen.findByTestId(T.marketsRow(MINT_A));
        const cell = (col: string) => row.querySelector<HTMLElement>(`td[data-col="${col}"]`)?.dataset;
        expect(cell('token')?.card).toBe('title');
        expect(cell('venue')?.card).toBe('aside');
        expect(cell('tier')?.card).toBe('aside');
        expect(cell('price')?.label).toBe('Price');
        expect(cell('tvl')?.priority).toBe('1');
        expect(cell('util')?.priority).toBe('2');
        expect(cell('inventory')?.priority).toBe('3');
        expect(cell('created')?.priority).toBe('3');
        // No flags: an empty cell (a card leaves it out).
        expect(row.querySelector('td[data-col="flags"]')?.childElementCount).toBe(0);
        // The filter field: no autocapitalisation or autocorrection of a mint.
        const input = screen.getByTestId(T.marketsSearch);
        expect(input.getAttribute('autocapitalize')).toBe('none');
        expect(input.getAttribute('placeholder')).toBe('Filter this list');
        queryClient.clear();
    });

    it('counts the active filters on the phone toggle and clears them from an empty list', async () => {
        mockIndexer({
            '/v1/markets': (u: URL) => ({
                data: u.searchParams.get('tier') ? [] : [summary({ mint: MINT_B, symbol: 'WIF' })],
                next_cursor: null,
            }),
        });
        const { router, queryClient } = await renderAt('/markets?tier=A&venue=pump_swap');
        const toggle = await screen.findByTestId(T.marketsFiltersToggle);
        expect(toggle.textContent).toContain('Filters · 2');
        expect(toggle.getAttribute('aria-expanded')).toBe('false');
        await act(async () => {
            fireEvent.click(toggle);
        });
        expect(toggle.getAttribute('aria-expanded')).toBe('true');
        const clear = await screen.findByTestId(T.marketsClearFilters);
        await act(async () => {
            fireEvent.click(clear);
        });
        await waitFor(() => expect(router.state.location.search).toEqual({}));
        expect(await screen.findByTestId(T.marketsRow(MINT_B))).toBeTruthy();
        queryClient.clear();
    });
});

describe('market page', () => {
    it('opens the trade sheet from the action bar on a phone, modal, and Escape closes it', async () => {
        setMedia({ compact: true, phone: true, coarse: true });
        marketIndexer();
        const { router, queryClient } = await renderAt(`/m/${MINT_A}`);
        const short = await screen.findByTestId(T.marketActionShort);
        expect(short.getAttribute('aria-label')).toBe('Short BONK');
        expect(screen.getByTestId(T.marketActionLend).getAttribute('aria-label')).toBe('Lend BONK');
        const panel = screen.getByTestId(T.shortPanel).closest<HTMLElement>('.trade-panel');
        expect(panel?.getAttribute('role')).toBeNull();

        await act(async () => {
            fireEvent.click(screen.getByTestId(T.marketActionLend));
        });
        await waitFor(() => expect(router.state.location.search).toMatchObject({ sheet: 'trade', panel: 'lend' }));
        await waitFor(() => expect(panel?.getAttribute('data-sheet')).toBe('open'));
        expect(panel?.getAttribute('role')).toBe('dialog');
        expect(panel?.getAttribute('aria-modal')).toBe('true');
        const title = document.getElementById(panel?.getAttribute('aria-labelledby') ?? '');
        expect(title?.textContent).toBe('Trade BONK');
        expect(document.activeElement).toBe(title);
        expect(screen.getByTestId(T.marketHeader).closest('[inert]')).not.toBeNull();

        await act(async () => {
            fireEvent.keyDown(document, { key: 'Escape' });
        });
        await waitFor(() => expect(router.state.location.search).not.toHaveProperty('sheet'));
        expect(panel?.getAttribute('data-sheet')).toBeNull();
        expect(panel?.getAttribute('role')).toBeNull();
        expect(screen.getByTestId(T.marketHeader).closest('[inert]')).toBeNull();
        queryClient.clear();
    });

    it('ignores the sheet key on a desktop: the trade panel stays in the page', async () => {
        marketIndexer();
        const { queryClient } = await renderAt(`/m/${MINT_A}?sheet=trade`);
        const panel = (await screen.findByTestId(T.shortPanel)).closest<HTMLElement>('.trade-panel');
        expect(panel?.getAttribute('data-sheet')).toBeNull();
        expect(panel?.getAttribute('role')).toBeNull();
        queryClient.clear();
    });

    it('shows "Lend only" once: the venue badge carries the list-only flag', async () => {
        vi.mocked(accounts.readMarket).mockResolvedValue(exists(MINT_A, chainMarket({ venue: 0 })));
        marketIndexer({ [`/v1/markets/${MINT_A}`]: detail({ symbol: 'BONK', venue: 'none' }) });
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        const flag = await screen.findByTestId(T.marketFlag('list-only'));
        expect(within(flag).getByTestId(T.marketVenue).textContent).toBe('Lend only');
        const header = screen.getByTestId(T.marketHeader);
        expect(within(header).getAllByText('Lend only')).toHaveLength(1);
        queryClient.clear();
    });

    it('ranks the stats (rate first; inventory, OI and lenders behind "More stats" on a phone)', async () => {
        marketIndexer();
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        await screen.findByTestId(T.marketStat('inventory'));
        const strip = screen.getByTestId(T.marketStat('rate')).parentElement as HTMLElement;
        const order = [...strip.children].map((c) => c.querySelector('.stat-label')?.textContent);
        expect(order).toEqual([
            'Rate (APR)',
            'Utilisation',
            'TVL',
            'Size cap',
            'Inventory',
            'Open interest',
            'Lenders',
        ]);
        for (const k of ['inventory', 'oi', 'lenders'] as const) {
            expect(screen.getByTestId(T.marketStat(k)).dataset.priority).toBe('2');
        }
        const more = screen.getByTestId(T.marketStatsMore);
        expect(more.getAttribute('aria-controls')).toBe(strip.id);
        expect(strip.hasAttribute('data-expanded')).toBe(false);
        await act(async () => {
            fireEvent.click(more);
        });
        expect(more.getAttribute('aria-expanded')).toBe('true');
        expect(strip.hasAttribute('data-expanded')).toBe(true);
        queryClient.clear();
    });

    it('folds an all-zero settlements block into one line, and mutes zero values', async () => {
        const zero = {
            settlements: 0,
            token_value: '0',
            lender_amount: '0',
            lender_pnl: '0',
            realised_lender_loss: '0',
            loss_events: 0,
            loss_bps_of_value: null,
            loss_bps_of_notional: null,
            realised_lender_loss_s1: '0',
            realised_lender_loss_s2: '0',
            realised_lender_loss_s3: '0',
        };
        marketIndexer({ [`/v1/markets/${MINT_A}`]: detail({ symbol: 'BONK', lender_loss: zero }) });
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        const loss = await screen.findByTestId(T.marketLenderLoss);
        expect(loss.dataset.value).toBe('0');
        expect(loss.querySelector('summary')?.textContent).toBe('No settlements yet');
        expect(loss.querySelector('details')?.hasAttribute('open')).toBe(false);
        // A zero amount stays, quieter; the fixture's open fees are not zero.
        expect(loss.querySelectorAll('dd[data-zero]').length).toBe(9);
        expect(screen.getByTestId(T.marketFees).querySelector('tbody tr td[data-card="title"]')).toBeTruthy();
        queryClient.clear();
    });
});
