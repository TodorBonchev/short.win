/**
 * Fixtures for the MARKETS page tests: OpenAPI-shaped indexer responses (services/indexer/openapi.yaml)
 * and decoded chain accounts (only the fields the pages read; cast to the generated types).
 */
import { bigIntToPodU128, type GlobalConfig, type Market, type OracleCache, type VenueGuard } from '@shortwin/client';
import type { MaybeAccount } from '@solana/kit';
import { type Address, address, getAddressEncoder } from '@solana/kit';
import type {
    ExitEvent,
    GlobalStats,
    IngestStatus,
    LenderPosition,
    MarketDetail,
    MarketFlags,
    MarketSummary,
    VenueStatus,
} from '../../../api/types.ts';
import { TOKEN_2022_PROGRAM, TOKEN_PROGRAM } from '../../../config/programs.ts';

export const MINT_A = address('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
export const MINT_B = address('DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263');
export const MINT_C = address('EKpQGSJtjMFqKZ9KQanSqYXRcF8fBopzLHYxdM65zcjm');
export const MINT_D = address('JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN');
export const CREATOR = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
export const LENDER = address('5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK');
export const SIG_A = '5VERv8NMvzbJMEkV8xnrLkEaWRtSz9CosKDYjCJjBRnbJLgp8uirBgmQpjKhoR4tjF3ZpRzrFmBV6UjKdiSZkQUW';
export const SIG_B = '3AsdoALgZFuq2oUVWrDYhg2pNeaLJKPLf8hU2mQ6U8qJxeJ6hsrhKqfpRV5ix3UvQvtaVuaKvpNfdVY3ehrHJ6Wm';

export const NOW = Math.floor(Date.now() / 1000);

export const NO_FLAGS: MarketFlags = {
    paused_opens: false,
    paused_coverage: false,
    mint_unsafe: false,
    route_broken: false,
    wipe_live: false,
};

export function summary(over: Partial<MarketSummary> & { mint: string }): MarketSummary {
    return {
        market: over.mint,
        symbol: 'TEST',
        name: 'Test token',
        decimals: 6,
        venue: 'pump_swap',
        tier: 'C',
        created_ts: NOW - 3 * 86_400,
        flags: NO_FLAGS,
        tokens_free: '750000000000',
        borrowed_q: '250000000000',
        inventory_tokens: '1000000000000',
        util_bps: 2_500,
        open_positions: 3,
        ema_price: '28000000000000',
        price_ts: NOW - 5,
        oi_lamports: '12500000000',
        tvl_lamports: '40000000000',
        tvl_usd: 5_600,
        ...over,
    };
}

export function detail(over: Partial<MarketDetail> = {}): MarketDetail {
    return {
        ...summary({ mint: MINT_A }),
        creator: CREATOR,
        coin_creator: CREATOR,
        token_program: TOKEN_PROGRAM,
        bonding_curve: null,
        canonical_pool: null,
        ray_pool: null,
        graduated_at: NOW - 3 * 86_400,
        total_shares: '1000000000000',
        share_epoch: 0,
        total_notional_open: '9000000000',
        open_vault_lamports: '30000000000',
        lender_sol: '120000000',
        protocol_fees: '20000000',
        lender_count: 7,
        lender_value_lamports: '41000000000',
        inventory_value_lamports: '28000000000',
        spot_wad: '29000000000000',
        route_chains: { no_route_since: 0, pool_fail_since: 0 },
        fees: [
            { fee_type: 'open_fee', recipient: 'protocol', n: 3, total: '15000000', last_24h: '5000000' },
            { fee_type: 'rent_lender', recipient: 'lenders', n: 40, total: '90000000', last_24h: '30000000' },
        ],
        exits: {
            positions: 9,
            open: 3,
            closes: 4,
            liquidations_cr: 1,
            liquidations_expired: 0,
            expiries: 0,
            settlements_s1: 0,
            settlements_s2: 1,
            settlements_s3: 0,
            exits: 6,
            avg_duration_s: 7_200,
            close_rate_bps: 6_666,
            liquidation_rate_bps: 1_666,
            expiry_rate_bps: 0,
            settlement_rate_bps: 1_666,
            avg_open_impact_bps: 120,
            avg_exit_slippage_bps: 80,
            borrower_pnl: '-150000000',
            borrower_pnl_known_exits: 6,
            borrower_pnl_unknown_exits: 0,
        },
        lender_loss: {
            settlements: 1,
            token_value: '2000000000',
            lender_amount: '1800000000',
            lender_pnl: '-200000000',
            realised_lender_loss: '200000000',
            loss_events: 1,
            loss_bps_of_value: 1_000,
            loss_bps_of_notional: 800,
            realised_lender_loss_s1: '0',
            realised_lender_loss_s2: '200000000',
            realised_lender_loss_s3: '0',
        },
        creator_exposure: {
            coin_creator: CREATOR,
            coin_creator_shares: '200000000000',
            coin_creator_share_bps: 2_000,
            coin_creator_deposited_tokens: '200000000000',
            creator_fee_income_lamports: '3500000',
            shortwin_volume_lamports: '900000000',
        },
        coverage: {
            gaps: 2,
            gap_s_total: 30,
            gap_s_max: 20,
            gaps_24h: 1,
            gap_s_24h: 10,
            coverage_pauses: 0,
            coverage_paused_s: 0,
            paused_coverage: false,
            observations_24h: 86_000,
            failed_observations_24h: 3,
        },
        quarantined_events: 0,
        tier_terms: null,
        ...over,
    };
}

export function lender(owner: string, over: Partial<LenderPosition> = {}): LenderPosition {
    return {
        market: MINT_A,
        mint: MINT_A,
        owner,
        shares: '500000000000',
        share_bps: 5_000,
        epoch: 0,
        lock: '0',
        lock_until: 0,
        deposited_tokens: '500000000000',
        withdrawn_tokens: '0',
        sol_paid: '10000000',
        pot_draw: '0',
        deficit_covered: '0',
        deposits: 1,
        withdrawals: 0,
        claims: 1,
        is_coin_creator: false,
        is_market_creator: false,
        first_ts: NOW - 86_400,
        last_ts: NOW - 3_600,
        ...over,
    };
}

export function exitEvent(signature: string, over: Partial<ExitEvent> = {}): ExitEvent {
    return {
        signature,
        slot: 1_000,
        tx_index: 0,
        ix_index: 0,
        ts: NOW - 600,
        finalized: true,
        kind: 'liquidation',
        market: MINT_A,
        mint: MINT_A,
        position: LENDER,
        keeper: CREATOR,
        q: '1000000000',
        buyback_cost: '400000000',
        lender_amount: null,
        keeper_paid: '10000000',
        borrower_payout: '50000000',
        settle_condition: null,
        expired: false,
        ...over,
    };
}

export const INGEST_OK: IngestStatus = {
    confirmed_slot: 1_000,
    finalized_slot: 968,
    latest_block_time: NOW,
    lag_s: 1,
    streams: [{ stream: 'shortwin', newest_slot: 1_000, backfill_complete: true }],
    rollups_complete_before: NOW - 3_600,
    quarantined: 0,
    layout_warnings: 0,
    missing: 0,
    logs_truncated: 0,
};

export function stats(over: Partial<GlobalStats> = {}): GlobalStats {
    return {
        markets: 4,
        markets_with_positions: 2,
        tvl_lamports: '100000000000',
        tvl_usd: 14_000,
        oi_lamports: '20000000000',
        notional_open_lamports: '15000000000',
        open_positions: 5,
        lender_sol: '0',
        protocol_fees_unswept: '0',
        protocol_revenue: '0',
        lender_fee_income: '0',
        keeper_income: '0',
        closes: 0,
        liquidations: 0,
        expiries: 0,
        settlements_s1: 0,
        settlements_s2: 0,
        settlements_s3: 0,
        realised_lender_loss: '0',
        settlement_lender_pnl: '0',
        unique_lenders: 0,
        unique_borrowers: 0,
        repeat_lenders: 0,
        repeat_borrowers: 0,
        avg_duration_s: null,
        coverage_gap_s: 42,
        creator_fee_income: '0',
        sol_usd: 140,
        inventory_value_lamports: '0',
        util_bps: 2_000,
        rental_fees: '0',
        avg_open_impact_bps: null,
        avg_exit_slippage_bps: null,
        ...over,
    };
}

export function venues(over: Partial<VenueStatus> = {}): VenueStatus {
    return {
        venue_enabled: 0b1111,
        route_broken_bits: 0,
        paused_opens: false,
        guards: [0, 1, 2, 3, 4, 5].map((venue) => ({
            venue,
            name:
                (['pump', 'pump_amm', 'pump_fees', 'raydium_v4', 'raydium_cpmm', 'jupiter'] as const)[venue] ??
                'unknown',
            upgraded: false,
            guard_slot: '100',
            programdata_slot: '100',
            flagged_at: null,
            last_ts: NOW - 60,
        })),
        extras: [],
        admin_changes: [],
        ...over,
    };
}

// ---- chain accounts ---------------------------------------------------------------------------

export function exists<T extends object>(addr: Address, data: T): MaybeAccount<T> {
    return {
        exists: true,
        address: addr,
        data,
        executable: false,
        lamports: 1n as never,
        programAddress: addr,
        space: 0n,
    } as unknown as MaybeAccount<T>;
}

export function missing<T extends object>(addr: Address): MaybeAccount<T> {
    return { exists: false, address: addr } as MaybeAccount<T>;
}

export function chainMarket(over: Partial<Market> = {}): Market {
    return {
        version: 1,
        flags: 0,
        venue: 3,
        tier: 2,
        decimals: 6,
        emaWarm: 0,
        createdAt: BigInt(NOW - 3 * 86_400),
        graduatedAt: BigInt(NOW - 3 * 86_400),
        mint: MINT_A,
        tokenProgram: TOKEN_PROGRAM,
        creator: CREATOR,
        tokensFree: 750_000_000_000n,
        borrowedQ: 250_000_000_000n,
        openPositions: 3,
        totalNotionalOpen: 9_000_000_000n,
        emaPrice: bigIntToPodU128(28_000_000_000_000n),
        utilKinkBps: 8_000,
        ...over,
    } as unknown as Market;
}

const TIER = (base: number) => ({
    mBps: 0,
    crOpenBps: 30_000,
    crLiqBps: 15_000,
    penaltyBps: 800,
    maxDurationS: 604_800,
    impactPosBps: 500,
    impactMktBps: 2_000,
    maxLiqSlipBps: 1_500,
    rateBaseBps: base,
    rateSlope1Bps: 2_000,
    rateSlope2Bps: 10_000,
    minRentS: 3_600,
});

export function globalConfig(over: Partial<GlobalConfig> = {}): GlobalConfig {
    return {
        flags: 0,
        venueEnabled: 0b1111,
        venueRouteBroken: 0,
        params: {
            marketCreationFee: 20_000_000n,
            thetaAUsdE6: 500_000_000_000n,
            thetaBUsdE6: 100_000_000_000n,
            oracleStaleS: 600,
        },
        tiers: [TIER(500), TIER(700), TIER(1_000), TIER(1_500)],
        kink: { utilKinkBps: 8_000, version: 1 },
        ...over,
    } as unknown as GlobalConfig;
}

/** `globalConfig()` with every parameter the How it works page and the disclosures quote. */
export function fullGlobalConfig(): GlobalConfig {
    const base = globalConfig();
    return {
        ...base,
        params: {
            ...base.params,
            openFeeBps: 10,
            rentProtocolBps: 1_000,
            penaltyLenderBps: 6_000,
            penaltyProtocolBps: 1_000,
            penaltyKeeperBps: 3_000,
            expiryFeeBps: 100,
            settleKeeperFixed: 10_000_000n,
            settleKeeperBps: 100,
            settleKeeperCap: 500_000_000n,
            s3WaitS: 600,
            lockGraceS: 86_400,
            lumpVestS: 259_200,
        },
        tiers: base.tiers.map((t, i) => ({
            ...t,
            maxDurationS: [2_592_000, 1_209_600, 604_800, 259_200][i] ?? 604_800,
        })),
    } as GlobalConfig;
}

export function oracleCache(): OracleCache {
    return {
        price: 14_000_000_000n,
        conf: 0n,
        expo: -8,
        publishTime: BigInt(NOW - 20),
        updatedSlot: 1n,
        bump: 1,
    } as unknown as OracleCache;
}

export function venueGuard(upgraded = 0): VenueGuard {
    return {
        upgraded,
        guardSlot: [10n, 11n, 12n, 13n, 14n, 15n],
        flaggedSlot: [0n, 0n, 0n, 0n, 0n, 0n],
        flaggedAt: [0n, 0n, 0n, 0n, 0n, 0n],
    } as unknown as VenueGuard;
}

// ---- mint bytes -------------------------------------------------------------------------------

/** A base mint: 82 bytes, initialized, optional freeze authority. */
export function mintBytes({ freeze = false, decimals = 6 } = {}): Uint8Array {
    const d = new Uint8Array(82);
    d[36] = 0x40;
    d[37] = 0x42;
    d[38] = 0x0f; // supply 1,000,000
    d[44] = decimals;
    d[45] = 1;
    if (freeze) {
        d[46] = 1;
        d.set(getAddressEncoder().encode(CREATOR), 50);
    }
    return d;
}

/** A Token-2022 mint with the given TLV entries. */
export function mint2022Bytes(entries: readonly (readonly [number, number])[]): Uint8Array {
    const out: number[] = [...mintBytes()];
    while (out.length < 166) out.push(0);
    out[165] = 1;
    for (const [t, len] of entries) {
        out.push(t & 0xff, t >> 8, len & 0xff, len >> 8);
        for (let i = 0; i < len; i++) out.push(0);
    }
    return new Uint8Array(out);
}

export { TOKEN_2022_PROGRAM, TOKEN_PROGRAM };
