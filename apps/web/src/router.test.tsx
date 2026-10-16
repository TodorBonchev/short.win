/**
 * Smoke test of the shell and the route tree: every route renders inside the app shell, with the
 * header, search, wallet button and status bar present, against unreachable RPC / indexer endpoints.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router';
import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { IndexerLiveProvider } from './api/live.tsx';
import { ToastProvider } from './components/Toast.tsx';
import { config } from './config/env.ts';
import { T } from './config/testids.ts';
import { routeTree } from './router.tsx';
import { createAppClient } from './rpc/client.ts';
import { AppClientProvider } from './rpc/context.tsx';
import { closeMode } from './shell/sheetParam.ts';

const MINT = 'So11111111111111111111111111111111111111112';
const POSITION = '9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin';

class NoSocket {
    readyState = 0;
    onopen = null;
    onclose = null;
    onerror = null;
    onmessage = null;
    send() {}
    close() {}
    addEventListener() {}
    removeEventListener() {}
}

beforeEach(() => {
    // Nothing answers: requests stay pending (no retries to wait for), sockets never open.
    vi.stubGlobal('fetch', () => new Promise(() => {}));
    vi.stubGlobal('WebSocket', NoSocket);
});

afterEach(() => {
    vi.unstubAllGlobals();
});

async function renderAt(path: string) {
    const queryClient = new QueryClient();
    const router = createRouter({
        routeTree,
        context: { queryClient },
        history: createMemoryHistory({ initialEntries: [path] }),
    });
    const client = createAppClient(config);
    await act(async () => {
        render(
            <AppClientProvider client={client}>
                <QueryClientProvider client={queryClient}>
                    <IndexerLiveProvider>
                        <ToastProvider>
                            <RouterProvider router={router} />
                        </ToastProvider>
                    </IndexerLiveProvider>
                </QueryClientProvider>
            </AppClientProvider>,
        );
        await router.load();
    });
    return { router, queryClient };
}

describe('routes', () => {
    it.each([
        ['/markets', T.page('markets')],
        ['/markets/new', T.page('create-market')],
        ['/positions', T.page('positions')],
        ['/lend', T.page('lend')],
        ['/status', T.page('status')],
        ['/security', T.page('security')],
        ['/how-it-works', T.page('how-it-works')],
        [`/positions/${POSITION}`, T.page('position')],
        // The trade panel's default (Short or Lend) waits for the market's view (MarketPage.tsx); an explicit
        // ?panel= mounts at once.
        [`/m/${MINT}`, T.page('market')],
        [`/m/${MINT}?panel=lend&action=withdraw`, T.lendPanel],
    ])('%s renders inside the shell', async (path, testId) => {
        const { queryClient } = await renderAt(path);
        expect(await screen.findByTestId(testId)).toBeTruthy();
        if (path.startsWith('/m/')) expect(screen.getByTestId(T.page('market'))).toBeTruthy();
        expect(screen.getByTestId(T.header)).toBeTruthy();
        expect(screen.getByTestId(T.searchInput)).toBeTruthy();
        expect(screen.getByTestId(T.walletButton)).toBeTruthy();
        expect(screen.getByTestId(T.statusBar)).toBeTruthy();
        queryClient.clear();
    });

    it('redirects / to /markets and validates search params', async () => {
        const { router, queryClient } = await renderAt('/?x=1');
        expect(router.state.location.pathname).toBe('/markets');
        await act(async () => {
            await router.navigate({ to: '/markets', search: { tier: 'Z' as never, sort: 'tvl' } });
        });
        expect(router.state.location.search).toEqual({ sort: 'tvl' });
        queryClient.clear();
    });

    it('validates the sheet keys (M10-RESP): known values kept, unknown ones dropped', async () => {
        const { router, queryClient } = await renderAt(`/m/${MINT}?panel=lend&sheet=trade`);
        expect(router.state.location.search).toEqual({ panel: 'lend', sheet: 'trade' });
        await act(async () => {
            await router.navigate({
                to: '/m/$mint',
                params: { mint: MINT as never },
                search: { sheet: 'nope' as never },
            });
        });
        expect(router.state.location.search).toEqual({});
        await act(async () => {
            await router.navigate({
                to: '/positions/$position',
                params: { position: POSITION as never },
                search: { sheet: 'add-margin' },
            });
        });
        expect(router.state.location.search).toEqual({ sheet: 'add-margin' });
        await act(async () => {
            await router.navigate({
                to: '/positions/$position',
                params: { position: POSITION as never },
                search: { sheet: 'trade' as never },
            });
        });
        expect(router.state.location.search).toEqual({});
        queryClient.clear();
    });

    it.each([
        ['/status', true],
        ['/security', true],
        ['/markets', false],
    ])('the tab bar marks More as where %s is (%s)', async (path, current) => {
        const { queryClient } = await renderAt(path);
        expect(screen.getByTestId(T.navMore).hasAttribute('data-current')).toBe(current);
        expect(screen.getByTestId(T.navMore).getAttribute('aria-current')).toBe(current ? 'true' : null);
        queryClient.clear();
    });

    it('closes a sheet by going back only when opening it pushed the entry', () => {
        expect(closeMode({ sheet: true })).toBe('back');
        expect(closeMode({})).toBe('replace');
        expect(closeMode(undefined)).toBe('replace');
    });

    it('passes the lend action through and shows not-found for a bad mint', async () => {
        const { queryClient } = await renderAt(`/m/${MINT}?panel=lend&action=withdraw`);
        expect((await screen.findByTestId(T.lendPanel)).dataset.action).toBe('withdraw');
        queryClient.clear();
        const bad = await renderAt('/m/not-a-mint');
        expect(await screen.findByText(/Not found/)).toBeTruthy();
        bad.queryClient.clear();
    });
});
