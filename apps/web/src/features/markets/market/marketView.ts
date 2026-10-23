/**
 * One view of a market for the market page (FRONTEND.md §3.4) from its two sources (FE-D1): the
 * indexer's `MarketDetail` (analytics, history, symbol) and the chain's `Market` account (the state
 * that gates opens: flag bits, venue, tier, the EMA warm-up, the market's own rent kink). With
 * both, analytics come from the indexer's consistent snapshot and the token's identity and gating
 * state from the route and the chain: the mint is always the route's, and decimals, venue, tier,
 * flags, creator and token program are the chain's whenever its `Market` is loaded (M9-SEC-3: an
 * indexer must not be able to rename the token or rescale its prices). A detail for another mint
 * is not used at all (`useMarketData`); without the indexer the view is the chain's alone.
 */
import { type Market, podU128ToBigInt } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { big, type MarketDetail, type MarketFlags, type TierName, type VenueName } from '../../../api/types.ts';
import { displayTokenMetadata } from '../../../config/devnetSmokeToken.ts';
import { config } from '../../../config/env.ts';
import { flagsFromBits, venueName } from '../shared/marketFlags.ts';

export interface MarketView {
    readonly source: 'indexer' | 'chain';
    readonly mint: Address;
    readonly symbol: string | null;
    readonly name: string | null;
    readonly decimals: number | null;
    readonly venue: VenueName;
    readonly tier: TierName;
    readonly flags: MarketFlags;
    readonly createdTs: number | null;
    /** `Market.graduated_at` (pool age basis, O13); `null` when unknown. */
    readonly graduatedAt: number | null;
    readonly tokensFree: bigint;
    readonly borrowedQ: bigint;
    /** `T + ΣB_Q`. */
    readonly inventory: bigint;
    /** Utilisation in bps; `null` from the chain alone (the page computes it with the WASM math). */
    readonly utilBps: number | null;
    readonly openPositions: number;
    readonly totalNotionalOpen: bigint;
    readonly emaPriceWad: bigint | null;
    readonly spotWad: bigint | null;
    readonly oiLamports: bigint | null;
    readonly tvlLamports: bigint | null;
    readonly tvlUsd: number | null;
    readonly lenderCount: number | null;
    readonly creator: string | null;
    readonly coinCreator: string | null;
    readonly tokenProgram: string | null;
    /** `Market.ema_warm` (chain only). */
    readonly emaWarm: number | null;
    /** The market's own kink (`Market.util_kink_bps`, R3A-9; chain only). */
    readonly utilKinkBps: number | null;
    readonly detail: MarketDetail | null;
}

const TIERS: readonly TierName[] = ['A', 'B', 'C', 'D'];

export function tierName(code: number): TierName {
    return TIERS[code] ?? 'C';
}

const pos = (v: bigint): number | null => (v > 0n ? Number(v) : null);

export function viewFromChain(mint: Address, m: Market): MarketView {
    const ema = podU128ToBigInt(m.emaPrice);
    const label = displayTokenMetadata(config.cluster, mint, null, null);
    return {
        source: 'chain',
        mint,
        symbol: label.symbol,
        name: label.name,
        decimals: m.decimals,
        venue: venueName(m.venue),
        tier: tierName(m.tier),
        flags: flagsFromBits(m.flags),
        createdTs: pos(m.createdAt),
        graduatedAt: pos(m.graduatedAt),
        tokensFree: m.tokensFree,
        borrowedQ: m.borrowedQ,
        inventory: m.tokensFree + m.borrowedQ,
        utilBps: null,
        openPositions: m.openPositions,
        totalNotionalOpen: m.totalNotionalOpen,
        emaPriceWad: ema > 0n ? ema : null,
        spotWad: null,
        oiLamports: null,
        tvlLamports: null,
        tvlUsd: null,
        lenderCount: null,
        creator: m.creator,
        coinCreator: null,
        tokenProgram: m.tokenProgram,
        emaWarm: m.emaWarm,
        utilKinkBps: m.utilKinkBps,
        detail: null,
    };
}

export function viewFromIndexer(d: MarketDetail, chain: Market | null, mint: Address): MarketView {
    const label = displayTokenMetadata(config.cluster, mint, d.symbol, d.name);
    return {
        source: 'indexer',
        mint,
        symbol: label.symbol,
        name: label.name,
        decimals: chain ? chain.decimals : (d.decimals ?? null),
        venue: chain ? venueName(chain.venue) : d.venue,
        tier: chain ? tierName(chain.tier) : d.tier,
        flags: chain ? flagsFromBits(chain.flags) : d.flags,
        createdTs: d.created_ts,
        graduatedAt: chain ? pos(chain.graduatedAt) : d.graduated_at,
        tokensFree: big(d.tokens_free),
        borrowedQ: big(d.borrowed_q),
        inventory: big(d.inventory_tokens),
        utilBps: d.util_bps,
        openPositions: d.open_positions,
        totalNotionalOpen: big(d.total_notional_open),
        emaPriceWad: d.ema_price === null ? null : big(d.ema_price),
        spotWad: d.spot_wad === null ? null : big(d.spot_wad),
        oiLamports: big(d.oi_lamports),
        tvlLamports: big(d.tvl_lamports),
        tvlUsd: d.tvl_usd,
        lenderCount: d.lender_count,
        creator: chain ? chain.creator : d.creator,
        coinCreator: d.coin_creator,
        tokenProgram: chain ? chain.tokenProgram : d.token_program,
        emaWarm: chain?.emaWarm ?? null,
        utilKinkBps: chain?.utilKinkBps ?? null,
        detail: d,
    };
}
