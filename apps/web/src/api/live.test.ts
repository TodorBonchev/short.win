import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';
import type { ApiResult } from './client.ts';
import { qk } from './keys.ts';
import { applyWsMessage } from './live.tsx';
import type { MarketSummary, Page } from './types.ts';

const MINT = 'So11111111111111111111111111111111111111112';
const meta = { indexerSlot: 1, finalizedSlot: 1, receivedAt: 0 };

function summary(util_bps: number): MarketSummary {
    return {
        market: MINT,
        mint: MINT,
        symbol: 'X',
        name: null,
        decimals: 6,
        venue: 'pump_swap',
        tier: 'C',
        created_ts: 0,
        flags: {
            paused_opens: false,
            paused_coverage: false,
            mint_unsafe: false,
            route_broken: false,
            wipe_live: false,
        },
        tokens_free: '0',
        borrowed_q: '0',
        inventory_tokens: '0',
        util_bps,
        open_positions: 0,
        ema_price: null,
        price_ts: null,
        oi_lamports: '0',
        tvl_lamports: '0',
        tvl_usd: null,
    };
}

describe('applyWsMessage', () => {
    it('patches a market row in every cached markets page', () => {
        const qc = new QueryClient();
        const key = qk.markets({ sort: 'tvl' });
        qc.setQueryData<ApiResult<Page<MarketSummary>>>(key, {
            data: { data: [summary(10)], next_cursor: null },
            meta,
        });
        applyWsMessage(qc, { type: 'market', channel: 'markets', data: summary(9000) });
        expect(qc.getQueryData<ApiResult<Page<MarketSummary>>>(key)?.data.data[0]?.util_bps).toBe(9000);
    });

    it('invalidates the market’s queries on its events and everything on a rollback', () => {
        const qc = new QueryClient();
        qc.setQueryData(qk.market(MINT), { data: {}, meta });
        qc.setQueryData(qk.stats(), { data: {}, meta });
        applyWsMessage(qc, { type: 'event', channel: `market:${MINT}`, id: 1, data: {} as never });
        expect(qc.getQueryState(qk.market(MINT))?.isInvalidated).toBe(true);
        expect(qc.getQueryState(qk.stats())?.isInvalidated).toBe(false);
        applyWsMessage(qc, { type: 'rollback', slot: 5 });
        expect(qc.getQueryState(qk.stats())?.isInvalidated).toBe(true);
    });
});
