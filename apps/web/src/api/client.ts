/**
 * The indexer REST client (INDEXER.md §9; FRONTEND.md §6.1). GET only; every path parameter is
 * validated before the request (base58 for keys and signatures), query values are encoded by
 * `URLSearchParams`, errors are RFC 9457 `problem+json` mapped to `ApiError`, and each response
 * carries the indexer's slot headers for the staleness indicator.
 */

import { isAddress } from '@solana/kit';
import { cleanIndexerText } from '../lib/displayText.ts';
import type { Problem } from './types.ts';

export interface ApiMeta {
    /** `X-Indexer-Slot`: the newest slot the indexer has applied (confirmed). */
    readonly indexerSlot: number | null;
    /** `X-Indexer-Finalized-Slot`. */
    readonly finalizedSlot: number | null;
    /** Wall clock when the response arrived (ms). */
    readonly receivedAt: number;
}

export interface ApiResult<T> {
    readonly data: T;
    readonly meta: ApiMeta;
}

export class ApiError extends Error {
    readonly status: number;
    readonly code: Problem['code'] | 'network' | 'invalid_response';
    readonly retryAfterS: number | null;

    constructor(
        status: number,
        code: Problem['code'] | 'network' | 'invalid_response',
        message: string,
        retryAfterS: number | null = null,
    ) {
        super(message);
        this.name = 'ApiError';
        this.status = status;
        this.code = code;
        this.retryAfterS = retryAfterS;
    }

    /** 429 / 503 / network: worth retrying after `retryAfterS`. */
    get transient(): boolean {
        return this.status === 429 || this.status === 503 || this.code === 'network' || this.code === 'timeout';
    }
}

export type QueryValue = string | number | boolean | null | undefined;

const SIG_RE = /^[1-9A-HJ-NP-Za-km-z]{64,88}$/;

/** A path segment that must be a base58 address. */
export function keySegment(k: string): string {
    if (!isAddress(k)) throw new ApiError(400, 'invalid_pubkey', `not a base58 address: ${k.slice(0, 64)}`);
    return k;
}

/** A path segment that must be a base58 signature. */
export function signatureSegment(s: string): string {
    if (!SIG_RE.test(s)) throw new ApiError(400, 'invalid_signature', 'not a base58 signature');
    return s;
}

function headerInt(h: Headers, name: string): number | null {
    const v = h.get(name);
    if (v === null || !/^\d+$/.test(v)) return null;
    return Number(v);
}

export interface ApiClient {
    readonly baseUrl: string;
    get<T>(path: string, query?: Readonly<Record<string, QueryValue>>, signal?: AbortSignal): Promise<ApiResult<T>>;
}

export interface ApiClientOptions {
    readonly fetchImpl?: typeof fetch;
    /** Called with every successful response's meta (feeds the staleness indicator). */
    readonly onMeta?: (meta: ApiMeta) => void;
}

export function createApiClient(baseUrl: string, opts: ApiClientOptions = {}): ApiClient {
    const base = baseUrl.replace(/\/+$/, '');
    const fetchImpl = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
    return {
        baseUrl: base,
        async get<T>(path: string, query?: Readonly<Record<string, QueryValue>>, signal?: AbortSignal) {
            if (!path.startsWith('/v1/')) throw new Error(`indexer path must start with /v1/: ${path}`);
            const qs = new URLSearchParams();
            for (const [k, v] of Object.entries(query ?? {})) {
                if (v !== undefined && v !== null && v !== '') qs.set(k, String(v));
            }
            const url = `${base}${path}${qs.size ? `?${qs.toString()}` : ''}`;
            let res: Response;
            try {
                res = await fetchImpl(url, {
                    method: 'GET',
                    headers: { Accept: 'application/json' },
                    credentials: 'omit',
                    referrerPolicy: 'no-referrer',
                    ...(signal ? { signal } : {}),
                });
            } catch (e) {
                if (e instanceof DOMException && e.name === 'AbortError') throw e;
                throw new ApiError(0, 'network', 'The indexer is unreachable.');
            }
            const meta: ApiMeta = {
                indexerSlot: headerInt(res.headers, 'x-indexer-slot'),
                finalizedSlot: headerInt(res.headers, 'x-indexer-finalized-slot'),
                receivedAt: Date.now(),
            };
            if (!res.ok) {
                const retry = headerInt(res.headers, 'retry-after');
                let problem: Partial<Problem> = {};
                try {
                    problem = (await res.json()) as Partial<Problem>;
                } catch {
                    // not problem+json
                }
                throw new ApiError(
                    res.status,
                    problem.code ?? (res.status === 404 ? 'not_found' : 'internal'),
                    problem.detail ?? problem.title ?? `HTTP ${res.status}`,
                    retry,
                );
            }
            let data: T;
            try {
                // Symbols and names are untrusted metadata: normalised once, here (M9-SEC-5).
                data = cleanIndexerText((await res.json()) as T);
            } catch {
                throw new ApiError(res.status, 'invalid_response', 'The indexer returned invalid JSON.');
            }
            opts.onMeta?.(meta);
            return { data, meta };
        },
    };
}
