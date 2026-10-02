/**
 * The markets table against a mocked indexer (FRONTEND.md §3.3, §10.1): the request it sends, the
 * rows and their cells, server-side sorting through the headers, keyset paging, filters, live row
 * patches from the `markets` channel, the create link for a mint without a market, and the
 * indexer-down fallback.
 */
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { applyWsMessage } from '../../api/live.tsx';
import { T } from '../../config/testids.ts';
import { MINT_A, MINT_B, MINT_C, MINT_D, NO_FLAGS, summary } from './test/fixtures.ts';
import { mockIndexer, problem, renderAt } from './test/harness.tsx';

afterEach(() => {
    vi.unstubAllGlobals();
});

const BONK_A = summary({ mint: MINT_A, symbol: 'BONK', util_bps: 2_500 });
const BONK_B = summary({
    mint: MINT_B,
    symbol: 'BONK',
    venue: 'raydium_cpmm',
    tier: 'B',
    util_bps: 9_200,
    flags: { ...NO_FLAGS, paused_coverage: true },
});
const WIF = summary({
    mint: MINT_C,
    symbol: 'WIF',
    venue: 'pump_curve',
    tier: 'D',
    flags: { ...NO_FLAGS, route_broken: true },
});

describe('MarketsPage', () => {
    it('requests TVL-sorted markets and renders the dense rows', async () => {
        const api = mockIndexer({ '/v1/markets': { data: [BONK_A, BONK_B, WIF], next_cursor: null } });
        const { queryClient } = await renderAt('/markets');
        const table = await screen.findByTestId(T.marketsTable);
        const req = api.callsTo('/v1/markets')[0];
        expect(req?.searchParams.get('sort')).toBe('tvl');
        expect(req?.searchParams.get('order')).toBe('desc');
        expect(req?.searchParams.get('limit')).toBe('100');
        await within(table).findByTestId(T.marketsRow(MINT_A));
        expect(within(table).getAllByRole('row')).toHaveLength(4);
        // Duplicate tickers carry the mint's suffix; unique ones do not.
        expect(screen.getByTestId(T.marketsRow(MINT_A)).textContent).toContain(MINT_A.slice(-4));
        expect(screen.getByTestId(T.marketsRow(MINT_B)).textContent).toContain(MINT_B.slice(-4));
        expect(screen.getByTestId(T.marketsRow(MINT_C)).querySelector('.mk-dup')).toBeNull();
        // Flags with words, venue and tier badges, utilisation.
        const b = screen.getByTestId(T.marketsRow(MINT_B));
        expect(b.textContent).toContain('Coverage paused');
        expect(b.textContent).toContain('Raydium CPMM');
        expect(b.textContent).toContain('92');
        expect(screen.getByTestId(T.marketsRow(MINT_C)).textContent).toContain('Route broken');
        // TVL sorted by default: aria-sort on its header.
        expect(screen.getByTestId(T.marketsSort('tvl')).closest('th')?.getAttribute('aria-sort')).toBe('descending');
        queryClient.clear();
    });

    it('sorts on the server through the header buttons, toggling the order', async () => {
        const api = mockIndexer({ '/v1/markets': { data: [BONK_A], next_cursor: null } });
        const { router, queryClient } = await renderAt('/markets');
        await screen.findByTestId(T.marketsRow(MINT_A));
        await act(async () => {
            fireEvent.click(screen.getByTestId(T.marketsSort('oi')));
        });
        await waitFor(() =>
            expect(api.callsTo('/v1/markets').some((u) => u.searchParams.get('sort') === 'oi')).toBe(true),
        );
        expect(router.state.location.search).toMatchObject({ sort: 'oi', order: 'desc' });
        expect(screen.getByTestId(T.marketsSort('oi')).closest('th')?.getAttribute('aria-sort')).toBe('descending');
        await act(async () => {
            fireEvent.click(screen.getByTestId(T.marketsSort('oi')));
        });
        await waitFor(() =>
            expect(
                api
                    .callsTo('/v1/markets')
                    .some((u) => u.searchParams.get('sort') === 'oi' && u.searchParams.get('order') === 'asc'),
            ).toBe(true),
        );
        expect(screen.getByTestId(T.marketsSort('oi')).closest('th')?.getAttribute('aria-sort')).toBe('ascending');
        queryClient.clear();
    });

    it('pages with the keyset cursor and patches live rows on every loaded page', async () => {
        const api = mockIndexer({
            '/v1/markets': (u: URL) =>
                u.searchParams.get('cursor') === 'c1'
                    ? { data: [BONK_B], next_cursor: null }
                    : { data: [BONK_A, WIF], next_cursor: 'c1' },
        });
        const { queryClient } = await renderAt('/markets');
        await screen.findByTestId(T.marketsRow(MINT_A));
        expect(screen.queryByTestId(T.marketsRow(MINT_B))).toBeNull();
        await act(async () => {
            fireEvent.click(screen.getByTestId(T.marketsLoadMore));
        });
        await screen.findByTestId(T.marketsRow(MINT_B));
        expect(api.callsTo('/v1/markets').at(-1)?.searchParams.get('cursor')).toBe('c1');
        expect(screen.getAllByRole('row')).toHaveLength(4);
        await waitFor(() => expect(screen.queryByTestId(T.marketsLoadMore)).toBeNull());
        // A `markets` channel summary patches the row in place (here on the second page).
        await act(async () => {
            applyWsMessage(queryClient, {
                type: 'market',
                channel: 'markets',
                data: { ...BONK_B, util_bps: 4_100, open_positions: 77 },
            });
        });
        await waitFor(() => expect(screen.getByTestId(T.marketsRow(MINT_B)).textContent).toContain('41%'));
        expect(screen.getByTestId(T.marketsRow(MINT_B)).textContent).toContain('77');
        queryClient.clear();
    });

    it('filters by venue, tier and open positions', async () => {
        const api = mockIndexer({ '/v1/markets': { data: [BONK_A], next_cursor: null } });
        const { router, queryClient } = await renderAt('/markets');
        await screen.findByTestId(T.marketsRow(MINT_A));
        await act(async () => {
            fireEvent.change(screen.getByTestId(T.marketsFilterVenue), { target: { value: 'raydium_cpmm' } });
        });
        await act(async () => {
            fireEvent.change(screen.getByTestId(T.marketsFilterTier), { target: { value: 'B' } });
        });
        await act(async () => {
            fireEvent.click(screen.getByTestId(T.marketsFilterActive));
        });
        await waitFor(() => {
            const last = api.callsTo('/v1/markets').at(-1);
            expect(last?.searchParams.get('venue')).toBe('raydium_cpmm');
            expect(last?.searchParams.get('tier')).toBe('B');
            expect(last?.searchParams.get('has_positions')).toBe('true');
        });
        expect(router.state.location.search).toMatchObject({ venue: 'raydium_cpmm', tier: 'B', active: true });
        queryClient.clear();
    });

    it('searches by mint or ticker and offers to create a market for an unlisted mint', async () => {
        const api = mockIndexer({
            '/v1/markets': (u: URL) => ({ data: u.searchParams.get('q') ? [] : [BONK_A], next_cursor: null }),
        });
        const { router, queryClient } = await renderAt('/markets');
        await screen.findByTestId(T.marketsRow(MINT_A));
        expect(screen.getByTestId(T.marketsCreateLink).getAttribute('href')).toBe('/markets/new');
        await act(async () => {
            fireEvent.change(screen.getByTestId(T.marketsSearch), { target: { value: MINT_D } });
        });
        await waitFor(() =>
            expect(api.callsTo('/v1/markets').some((u) => u.searchParams.get('q') === MINT_D)).toBe(true),
        );
        expect(router.state.location.search).toMatchObject({ q: MINT_D });
        expect(await screen.findByTestId(T.marketsEmpty)).toBeTruthy();
        await waitFor(() =>
            expect(screen.getByTestId(T.marketsCreateLink).getAttribute('href')).toBe(`/markets/new?mint=${MINT_D}`),
        );
        queryClient.clear();
    });

    it('keeps the filter input and the URL in step when the URL changes elsewhere', async () => {
        mockIndexer({ '/v1/markets': { data: [BONK_A], next_cursor: null } });
        const { router, queryClient } = await renderAt('/markets');
        await screen.findByTestId(T.marketsRow(MINT_A));
        await act(async () => {
            fireEvent.change(screen.getByTestId(T.marketsSearch), { target: { value: 'bon' } });
        });
        await waitFor(() => expect(router.state.location.search).toMatchObject({ q: 'bon' }));
        // A navigation from elsewhere (a link, Back) wins over the input's last value.
        await act(async () => {
            await router.navigate({ to: '/markets', search: { q: 'wif' } });
        });
        await waitFor(() => expect((screen.getByTestId(T.marketsSearch) as HTMLInputElement).value).toBe('wif'));
        await act(async () => {
            await new Promise((r) => setTimeout(r, 400));
        });
        expect(router.state.location.search).toMatchObject({ q: 'wif' });
        queryClient.clear();
    });

    it('replaces the list with "open a mint" when the indexer fails', async () => {
        mockIndexer({ '/v1/markets': problem(500, 'internal') });
        const { router, queryClient } = await renderAt('/markets');
        const input = await screen.findByTestId(T.marketsOpenMint);
        expect(screen.getByTestId(T.page('markets')).querySelector('.error-state')?.textContent).toContain('internal');
        await act(async () => {
            fireEvent.change(input, { target: { value: MINT_D } });
        });
        await act(async () => {
            fireEvent.submit(input.closest('form') as HTMLFormElement);
        });
        await waitFor(() => expect(router.state.location.pathname).toBe(`/m/${MINT_D}`));
        queryClient.clear();
    });
});
