/**
 * Renders a route of the real route tree inside the app shell (as `router.test.tsx` does) against a
 * mocked indexer: `fetch` answers indexer GETs from a path → body table and leaves every other
 * request (RPC, the .wasm) pending. The chain is mocked per test file at `rpc/accounts.ts`,
 * `shared/chainReads.ts` and `venues/index.ts`.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router';
import { act, render } from '@testing-library/react';
import { vi } from 'vitest';
import { IndexerLiveProvider } from '../../../api/live.tsx';
import { ToastProvider } from '../../../components/Toast.tsx';
import { config } from '../../../config/env.ts';
import { initMathSync } from '../../../math/load.ts';
import { routeTree } from '../../../router.tsx';
import { createAppClient } from '../../../rpc/client.ts';
import { AppClientProvider } from '../../../rpc/context.tsx';
import { readRepoBytes } from '../../../test/repo.ts';
import { INGEST_OK, stats, venues } from './fixtures.ts';

export const INDEXER = new URL(config.indexerUrl).origin;

export type Handler = unknown | ((url: URL) => unknown);

/** A response with a status other than 200: `{ status, body }`. */
export class Reply {
    readonly status: number;
    readonly body: unknown;

    constructor(status: number, body: unknown) {
        this.status = status;
        this.body = body;
    }
}

export function problem(status: number, code: string): Reply {
    return new Reply(status, { type: 'about:blank', title: code, status, code, detail: code });
}

export const DEFAULT_HANDLERS: Readonly<Record<string, Handler>> = {
    '/v1/status': INGEST_OK,
    '/v1/stats': stats(),
    '/v1/venues': venues(),
    '/v1/admin/config': { paused_opens: false },
};

export interface MockIndexer {
    /** Every indexer request, in order. */
    readonly calls: URL[];
    /** Requests to `path` so far. */
    callsTo(path: string): URL[];
}

export function mockIndexer(handlers: Readonly<Record<string, Handler>>): MockIndexer {
    const calls: URL[] = [];
    const table = { ...DEFAULT_HANDLERS, ...handlers };
    vi.stubGlobal(
        'fetch',
        vi.fn((input: RequestInfo | URL) => {
            const url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url);
            if (url.origin !== INDEXER) return new Promise(() => {});
            calls.push(url);
            const h = table[url.pathname];
            const r = typeof h === 'function' ? (h as (u: URL) => unknown)(url) : h;
            const reply = r === undefined ? problem(404, 'not_found') : r instanceof Reply ? r : new Reply(200, r);
            return Promise.resolve(
                new Response(JSON.stringify(reply.body), {
                    status: reply.status,
                    headers: {
                        'content-type': 'application/json',
                        'x-indexer-slot': '1000',
                        'x-indexer-finalized-slot': '968',
                    },
                }),
            );
        }),
    );
    return { calls, callsTo: (path) => calls.filter((u) => u.pathname === path) };
}

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

let mathReady = false;

/** Initialises the real WASM math from `packages/math-wasm/pkg` once per test file. */
export function loadMathForTests(): void {
    if (mathReady) return;
    initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
    mathReady = true;
}

export async function renderAt(path: string) {
    vi.stubGlobal('WebSocket', NoSocket);
    loadMathForTests();
    const queryClient = new QueryClient();
    const router = createRouter({
        routeTree,
        context: { queryClient },
        history: createMemoryHistory({ initialEntries: [path] }),
    });
    const client = createAppClient(config);
    let view: ReturnType<typeof render> | undefined;
    await act(async () => {
        view = render(
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
    return { router, queryClient, view };
}
