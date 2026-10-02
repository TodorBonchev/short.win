import { describe, expect, it, vi } from 'vitest';
import { ApiError, createApiClient, keySegment } from './client.ts';

const MINT = 'So11111111111111111111111111111111111111112';

function fakeFetch(status: number, body: unknown, headers: Record<string, string> = {}) {
    return vi.fn(
        async (_url: RequestInfo | URL, _init?: RequestInit) =>
            new Response(typeof body === 'string' ? body : JSON.stringify(body), { status, headers }),
    );
}

describe('indexer client', () => {
    it('encodes the query, omits empty values and reads the slot headers', async () => {
        const f = fakeFetch(
            200,
            { data: [], next_cursor: null },
            { 'x-indexer-slot': '123', 'x-indexer-finalized-slot': '100' },
        );
        const metas: unknown[] = [];
        const api = createApiClient('https://api.short.win/', {
            fetchImpl: f as unknown as typeof fetch,
            onMeta: (m) => metas.push(m),
        });
        const r = await api.get('/v1/markets', { q: 'bonk & co', tier: undefined, sort: 'tvl', has_positions: true });
        expect(f.mock.calls[0]?.[0]).toBe('https://api.short.win/v1/markets?q=bonk+%26+co&sort=tvl&has_positions=true');
        expect(f.mock.calls[0]?.[1]?.credentials).toBe('omit');
        expect(r.meta.indexerSlot).toBe(123);
        expect(r.meta.finalizedSlot).toBe(100);
        expect(metas).toHaveLength(1);
    });

    it('maps problem+json errors with Retry-After', async () => {
        const f = fakeFetch(
            503,
            { type: 'about:blank', title: 'Unavailable', status: 503, code: 'unavailable', detail: 'busy' },
            { 'retry-after': '3' },
        );
        const api = createApiClient('http://x', { fetchImpl: f as unknown as typeof fetch });
        const e = await api.get('/v1/stats').catch((x: unknown) => x);
        expect(e).toBeInstanceOf(ApiError);
        expect((e as ApiError).code).toBe('unavailable');
        expect((e as ApiError).retryAfterS).toBe(3);
        expect((e as ApiError).transient).toBe(true);
    });

    it('maps a 404 without a body and a network failure', async () => {
        const api404 = createApiClient('http://x', { fetchImpl: fakeFetch(404, '') as unknown as typeof fetch });
        const e = (await api404.get(`/v1/markets/${MINT}`).catch((x: unknown) => x)) as ApiError;
        expect(e.code).toBe('not_found');
        expect(e.transient).toBe(false);
        const down = createApiClient('http://x', {
            fetchImpl: (async () => {
                throw new TypeError('Failed to fetch');
            }) as unknown as typeof fetch,
        });
        const n = (await down.get('/v1/stats').catch((x: unknown) => x)) as ApiError;
        expect(n.code).toBe('network');
        expect(n.transient).toBe(true);
    });

    it('refuses paths outside /v1 and non-base58 path keys', async () => {
        const api = createApiClient('http://x', { fetchImpl: fakeFetch(200, {}) as unknown as typeof fetch });
        await expect(api.get('/admin')).rejects.toThrow(/must start with \/v1\//);
        expect(() => keySegment('../../etc')).toThrow(ApiError);
        expect(keySegment(MINT)).toBe(MINT);
    });
});
