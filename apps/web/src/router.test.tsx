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
        [`/positions/${POSITION}`, T.page('position')],
        [`/m/${MINT}`, T.shortPanel],
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

    it('passes the lend action through and shows not-found for a bad mint', async () => {
        const { queryClient } = await renderAt(`/m/${MINT}?panel=lend&action=withdraw`);
        expect((await screen.findByTestId(T.lendPanel)).dataset.action).toBe('withdraw');
        queryClient.clear();
        const bad = await renderAt('/m/not-a-mint');
        expect(await screen.findByText(/Not found/)).toBeTruthy();
        bad.queryClient.clear();
    });
});
