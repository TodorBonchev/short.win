/**
 * The hand-written API types against openapi.yaml: for each schema below, the TypeScript key set
 * (checked by the compiler through `Record<keyof T, true>`) equals the schema's `required` list.
 */
import { describe, expect, it } from 'vitest';
import { readRepo } from '../test/repo.ts';
import type { IngestStatus, LenderPosition, MarketSummary, Position } from './types.ts';

function required(yaml: string, schema: string): string[] {
    const start = yaml.indexOf(`\n    ${schema}:\n`);
    if (start < 0) throw new Error(`schema ${schema} not found`);
    const block = yaml.slice(start, start + 4000);
    const m = /required: \[([^\]]*)\]/s.exec(block);
    if (!m?.[1]) throw new Error(`no required list in ${schema}`);
    return m[1]
        .split(',')
        .map((s) => s.trim().replace(/^'|'$/g, ''))
        .filter(Boolean)
        .sort();
}

const marketSummary: Record<keyof MarketSummary, true> = {
    market: true,
    mint: true,
    symbol: true,
    name: true,
    decimals: true,
    venue: true,
    tier: true,
    created_ts: true,
    flags: true,
    tokens_free: true,
    borrowed_q: true,
    inventory_tokens: true,
    util_bps: true,
    open_positions: true,
    ema_price: true,
    price_ts: true,
    oi_lamports: true,
    tvl_lamports: true,
    tvl_usd: true,
};

const position: Record<keyof Position, true> = {
    position: true,
    market: true,
    mint: true,
    borrower: true,
    nonce: true,
    status: true,
    tier: true,
    venue: true,
    q: true,
    s: true,
    m: true,
    margin_added: true,
    v: true,
    n_open: true,
    fee_open: true,
    entry_price: true,
    deadline: true,
    opened_ts: true,
    open_signature: true,
    armed: true,
    exit: true,
};

const lender: Record<keyof LenderPosition, true> = {
    market: true,
    mint: true,
    owner: true,
    shares: true,
    share_bps: true,
    epoch: true,
    lock: true,
    lock_until: true,
    deposited_tokens: true,
    withdrawn_tokens: true,
    sol_paid: true,
    pot_draw: true,
    deficit_covered: true,
    deposits: true,
    withdrawals: true,
    claims: true,
    is_coin_creator: true,
    is_market_creator: true,
    first_ts: true,
    last_ts: true,
};

const ingest: Record<keyof IngestStatus, true> = {
    confirmed_slot: true,
    finalized_slot: true,
    latest_block_time: true,
    lag_s: true,
    streams: true,
    rollups_complete_before: true,
    quarantined: true,
    layout_warnings: true,
    missing: true,
    logs_truncated: true,
};

describe('API types match services/indexer/openapi.yaml', () => {
    const yaml = readRepo('services/indexer/openapi.yaml');
    it.each([
        ['MarketSummary', marketSummary],
        ['Position', position],
        ['LenderPosition', lender],
        ['IngestStatus', ingest],
    ] as const)('%s', (name, keys) => {
        expect(Object.keys(keys).sort()).toEqual(required(yaml, name));
    });
});
