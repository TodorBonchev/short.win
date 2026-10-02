/**
 * Response types of the indexer API (`services/indexer/openapi.yaml`, OpenAPI 3.1; INDEXER.md §9).
 * Hand-written from the document: u64 / u128 / i128 / sums travel as decimal strings
 * (INDEXER.md §4.2) and are converted with `big()` where they are used, never with `Number()`.
 * `types.test.ts` checks the required-field lists of the main schemas against openapi.yaml.
 */

export type Pubkey = string;
export type Signature = string;
/** Unsigned 64-bit integer as a decimal string. */
export type U64 = string;
/** Unsigned 128-bit integer as a decimal string (WAD prices, shares). */
export type U128 = string;
/** Signed integer as a decimal string. */
export type I128 = string;
/** Non-negative sum as a decimal string; may exceed u64. */
export type BigNum = string;

export type VenueName =
    | 'none'
    | 'pump_curve'
    | 'pump_migrating'
    | 'pump_swap'
    | 'raydium_v4'
    | 'raydium_cpmm'
    | 'mock'
    | 'unknown';
export type TierName = 'A' | 'B' | 'C' | 'D';
export type PositionStatus = 'open' | 'closed' | 'liquidated' | 'expired' | 'settled';
export type SettleCondition = 'S1' | 'S2' | 'S3';
export type MarketSort = 'tvl' | 'oi' | 'util' | 'created' | 'open_positions';
export type SortOrder = 'asc' | 'desc';

/** A decimal-string integer as a bigint. */
export function big(s: string | null | undefined): bigint {
    return s === null || s === undefined || s === '' ? 0n : BigInt(s);
}

export interface Page<T> {
    readonly data: readonly T[];
    readonly next_cursor: string | null;
}

export interface Problem {
    readonly type: string;
    readonly title: string;
    readonly status: number;
    readonly detail?: string;
    readonly code:
        | 'bad_request'
        | 'invalid_pubkey'
        | 'invalid_signature'
        | 'invalid_cursor'
        | 'limit_out_of_range'
        | 'range_too_large'
        | 'not_found'
        | 'rate_limited'
        | 'timeout'
        | 'unavailable'
        | 'internal';
}

export interface ChainRef {
    readonly signature: Signature;
    readonly slot: number;
    readonly tx_index: number;
    readonly ix_index: number;
    readonly ts: number;
    readonly finalized: boolean;
}

export interface IngestStatus {
    readonly confirmed_slot: number;
    readonly finalized_slot: number;
    readonly latest_block_time: number | null;
    readonly lag_s: number | null;
    readonly streams: readonly { stream: string; newest_slot: number | null; backfill_complete: boolean }[];
    readonly rollups_complete_before: number;
    readonly quarantined: number;
    readonly layout_warnings: number;
    readonly missing: number;
    readonly logs_truncated: number;
}

export interface MarketFlags {
    readonly paused_opens: boolean;
    readonly paused_coverage: boolean;
    readonly mint_unsafe: boolean;
    readonly route_broken: boolean;
    readonly wipe_live: boolean;
}

export interface MarketSummary {
    readonly market: Pubkey;
    readonly mint: Pubkey;
    readonly symbol: string | null;
    readonly name: string | null;
    readonly decimals: number | null;
    readonly venue: VenueName;
    readonly tier: TierName;
    readonly created_ts: number;
    readonly flags: MarketFlags;
    readonly tokens_free: U64;
    readonly borrowed_q: U64;
    readonly inventory_tokens: BigNum;
    readonly util_bps: number;
    readonly open_positions: number;
    readonly ema_price: U128 | null;
    readonly price_ts: number | null;
    readonly oi_lamports: BigNum;
    readonly tvl_lamports: BigNum;
    readonly tvl_usd: number | null;
}

export type FeeType =
    | 'creation_fee'
    | 'open_fee'
    | 'venue_rent'
    | 'rent_lender'
    | 'rent_protocol'
    | 'liq_penalty_lender'
    | 'liq_penalty_protocol'
    | 'liq_penalty_keeper'
    | 'liq_keeper_topup'
    | 'expiry_fee'
    | 'expiry_keeper_reward'
    | 'settle_keeper_reward'
    | 'trader_sweep_lenders'
    | 'trader_sweep_wipe'
    | 'trader_sweep_protocol'
    | 'venue_rewards_lenders'
    | 'venue_rewards_wipe'
    | 'venue_rewards_protocol';

export interface FeeLine {
    readonly fee_type: FeeType;
    readonly recipient: 'protocol' | 'lenders' | 'keeper' | 'venue';
    readonly n: number;
    readonly total: BigNum;
    readonly last_24h: BigNum;
}

export interface ExitCounts {
    readonly positions: number;
    readonly open: number;
    readonly closes: number;
    readonly liquidations_cr: number;
    readonly liquidations_expired: number;
    readonly expiries: number;
    readonly settlements_s1: number;
    readonly settlements_s2: number;
    readonly settlements_s3: number;
    readonly exits: number;
    readonly avg_duration_s: number | null;
    readonly close_rate_bps: number | null;
    readonly liquidation_rate_bps: number | null;
    readonly expiry_rate_bps: number | null;
    readonly settlement_rate_bps: number | null;
    readonly avg_open_impact_bps: number | null;
    readonly avg_exit_slippage_bps: number | null;
    readonly borrower_pnl: I128;
}

export interface LenderLoss {
    readonly settlements: number;
    readonly token_value: BigNum;
    readonly lender_amount: BigNum;
    readonly lender_pnl: I128;
    readonly realised_lender_loss: BigNum;
    readonly loss_events: number;
    readonly loss_bps_of_value: number | null;
    readonly loss_bps_of_notional: number | null;
    readonly realised_lender_loss_s1: BigNum;
    readonly realised_lender_loss_s2: BigNum;
    readonly realised_lender_loss_s3: BigNum;
}

export interface CreatorExposure {
    readonly coin_creator: Pubkey | null;
    readonly coin_creator_shares: U128;
    readonly coin_creator_share_bps: number | null;
    readonly coin_creator_deposited_tokens: U64;
    readonly creator_fee_income_lamports: BigNum;
    readonly shortwin_volume_lamports: BigNum;
}

export interface CoverageStats {
    readonly gaps: number;
    readonly gap_s_total: number;
    readonly gap_s_max: number | null;
    readonly gaps_24h: number;
    readonly gap_s_24h: number;
    readonly coverage_pauses: number;
    readonly coverage_paused_s: number;
    readonly paused_coverage: boolean;
    readonly observations_24h: number;
    readonly failed_observations_24h: number;
}

export interface MarketDetail extends MarketSummary {
    readonly creator: Pubkey;
    readonly coin_creator: Pubkey | null;
    readonly token_program: Pubkey;
    readonly bonding_curve: Pubkey | null;
    readonly canonical_pool: Pubkey | null;
    readonly ray_pool: Pubkey | null;
    readonly graduated_at: number | null;
    readonly total_shares: U128;
    readonly share_epoch: number;
    readonly total_notional_open: U64;
    readonly open_vault_lamports: U64;
    readonly lender_sol: U64;
    readonly protocol_fees: U64;
    readonly lender_count: number;
    readonly lender_value_lamports: BigNum;
    readonly inventory_value_lamports: BigNum;
    readonly spot_wad: U128 | null;
    readonly route_chains: { readonly no_route_since: number; readonly pool_fail_since: number };
    readonly fees: readonly FeeLine[];
    readonly exits: ExitCounts;
    readonly lender_loss: LenderLoss;
    readonly creator_exposure: CreatorExposure;
    readonly coverage: CoverageStats;
    readonly quarantined_events: number;
    readonly tier_terms: Readonly<Record<string, unknown>> | null;
}

export interface MarketCoverage {
    readonly mint: Pubkey;
    readonly stats: CoverageStats;
    readonly recent_gaps: readonly (ChainRef & { gap_s: number; open_positions: number })[];
    readonly pauses: readonly {
        paused_at: number;
        resumed_at: number;
        paused_s: number;
        ongoing: boolean;
        by: Pubkey | null;
    }[];
}

export interface MarketPools {
    readonly mint: Pubkey;
    readonly current: Pubkey | null;
    readonly window: { candidate: Pubkey; d_cand: U128; window_ends: number } | null;
    readonly pools: readonly {
        pool: Pubkey;
        venue: VenueName | null;
        first_seen_ts: number;
        last_d: U128;
        proposals: number;
        adopted_ts: number | null;
        retired_ts: number | null;
        is_current: boolean;
    }[];
    readonly events: readonly IndexerEvent[];
}

export interface PositionExit {
    readonly ts: number;
    readonly signature: Signature;
    readonly keeper: Pubkey | null;
    readonly buyback_cost: U64 | null;
    readonly rent_paid: U64 | null;
    readonly penalty: U64 | null;
    readonly expiry_fee: U64 | null;
    readonly keeper_paid: U64 | null;
    readonly borrower_payout: U64;
    readonly settle_condition: SettleCondition | null;
    readonly borrower_pnl: I128;
    readonly duration_s: number;
}

export interface Position {
    readonly position: Pubkey;
    readonly market: Pubkey;
    readonly mint: Pubkey;
    readonly borrower: Pubkey;
    readonly nonce: U64;
    readonly status: PositionStatus;
    readonly tier: TierName;
    readonly venue: VenueName;
    readonly q: U64;
    readonly s: U64;
    readonly m: U64;
    readonly margin_added: U64;
    readonly v: U64;
    readonly n_open: U64;
    readonly fee_open: U64;
    readonly entry_price: U128;
    readonly deadline: number;
    readonly opened_ts: number;
    readonly open_signature: Signature;
    readonly armed: boolean;
    readonly exit: PositionExit | null;
}

export interface Settlement extends ChainRef {
    readonly position: Pubkey;
    readonly market: Pubkey;
    readonly mint: Pubkey;
    readonly keeper: Pubkey;
    readonly condition: SettleCondition;
    readonly q: U64;
    readonly n_open: U64;
    readonly expired: boolean;
    readonly b_live: U64;
    readonly b_rs: U64;
    readonly b_ema: U64;
    readonly p_lo: U128;
    readonly p_hi: U128;
    readonly fv: U64;
    readonly k_s: U64;
    readonly lender_amount: U64;
    readonly lump: U64;
    readonly borrower_amount: U64;
    readonly ema_age_s: number;
    readonly p_ref: U128;
    readonly p_ref_source: 'crank_clamped' | 'box_point' | 'box_mid';
    readonly token_value: U64;
    readonly lender_pnl: I128;
    readonly realised_lender_loss: U64;
}

export interface PositionDetail {
    readonly position: Position;
    readonly events: readonly IndexerEvent[];
    readonly settlement: Settlement | null;
}

export interface ExitEvent extends ChainRef {
    readonly kind: 'liquidation' | 'expiry' | 'settlement';
    readonly market: Pubkey;
    readonly mint: Pubkey;
    readonly position: Pubkey;
    readonly keeper: Pubkey;
    readonly q: U64;
    readonly buyback_cost: U64 | null;
    readonly lender_amount: U64 | null;
    readonly keeper_paid: U64;
    readonly borrower_payout: U64;
    readonly settle_condition: SettleCondition | null;
    readonly expired: boolean;
}

export interface LenderPosition {
    readonly market: Pubkey;
    readonly mint: Pubkey;
    readonly owner: Pubkey;
    readonly shares: U128;
    readonly share_bps: number | null;
    readonly epoch: number;
    readonly lock: U128;
    readonly lock_until: number;
    readonly deposited_tokens: U64;
    readonly withdrawn_tokens: U64;
    readonly sol_paid: U64;
    readonly pot_draw: U64;
    readonly deficit_covered: U64;
    readonly deposits: number;
    readonly withdrawals: number;
    readonly claims: number;
    readonly is_coin_creator: boolean;
    readonly is_market_creator: boolean;
    readonly first_ts: number;
    readonly last_ts: number;
}

export interface LenderDetail {
    readonly owner: Pubkey;
    readonly positions: readonly LenderPosition[];
    readonly totals: { readonly markets: number; readonly sol_paid: BigNum };
}

export type EventKind =
    | 'MarketCreated'
    | 'Deposit'
    | 'Withdraw'
    | 'ClaimSol'
    | 'DeficitCovered'
    | 'LenderSynced'
    | 'Open'
    | 'Close'
    | 'Liquidate'
    | 'Expire'
    | 'SolSettle'
    | 'SettleArmed'
    | 'SettleDisarmed'
    | 'EpochRolled'
    | 'ProtocolFeesSwept'
    | 'MarginAdded'
    | 'PriceCranked'
    | 'VenueStateChanged'
    | 'RouteChainChanged'
    | 'PoolProposed'
    | 'PoolSwitched'
    | 'PoolSwitchCancelled'
    | 'TraderSwept'
    | 'VenueRewardsClaimed'
    | 'MintSafetyTripped'
    | 'VenueUpgradeFlagged'
    | 'VenueRevalidated'
    | 'CoverageGap'
    | 'CoveragePauseSet'
    | 'ConfigProposed'
    | 'ConfigApplied'
    | 'ConfigCancelled'
    | 'PauseSet'
    | 'VenueDisabled'
    | 'RouteBrokenSet'
    | 'VenueExtrasSet'
    | 'CurveCompleted'
    | 'CurveAlreadyComplete';

export interface IndexerEvent extends ChainRef {
    readonly id: number;
    readonly kind: EventKind;
    readonly invocation: string;
    readonly source: 'cpi' | 'log';
    readonly market: Pubkey | null;
    readonly data: Readonly<Record<string, unknown>>;
}

export interface PriceSnapshot extends ChainRef {
    readonly spot_wad: U128;
    readonly ema_price: U128;
    readonly ema_liq: U128;
    readonly liq_slow: U128;
    readonly ema_count: U64;
    readonly tier: TierName;
    readonly util_bps: number;
    readonly vest_bal: U64;
    readonly cranker: Pubkey;
}

export interface VenueTrade extends ChainRef {
    readonly venue: VenueName;
    readonly pool: Pubkey;
    readonly mint: Pubkey;
    readonly side: 'buy' | 'sell';
    readonly token_amount: U64;
    readonly sol_amount: U64;
    readonly price_wad: U128 | null;
    readonly fee_creator_lamports: U64 | null;
    readonly user: Pubkey;
    readonly is_shortwin: boolean;
    readonly shortwin_invocation: string | null;
}

export interface Candle {
    readonly t: number;
    readonly open: U128;
    readonly high: U128;
    readonly low: U128;
    readonly close: U128;
    readonly volume_sol: BigNum;
    readonly volume_tokens: BigNum;
    readonly trades: number;
}

export interface GlobalStats {
    readonly markets: number;
    readonly markets_with_positions: number;
    readonly tvl_lamports: BigNum;
    readonly tvl_usd: number | null;
    readonly oi_lamports: BigNum;
    readonly notional_open_lamports: BigNum;
    readonly open_positions: number;
    readonly lender_sol: BigNum;
    readonly protocol_fees_unswept: BigNum;
    readonly protocol_revenue: BigNum;
    readonly lender_fee_income: BigNum;
    readonly keeper_income: BigNum;
    readonly closes: number;
    readonly liquidations: number;
    readonly expiries: number;
    readonly settlements_s1: number;
    readonly settlements_s2: number;
    readonly settlements_s3: number;
    readonly realised_lender_loss: BigNum;
    readonly settlement_lender_pnl: I128;
    readonly unique_lenders: number;
    readonly unique_borrowers: number;
    readonly repeat_lenders: number;
    readonly repeat_borrowers: number;
    readonly avg_duration_s: number | null;
    readonly coverage_gap_s: number;
    readonly creator_fee_income: BigNum;
    readonly sol_usd: number | null;
    readonly inventory_value_lamports: BigNum;
    readonly util_bps: number | null;
    readonly rental_fees: BigNum;
    readonly avg_open_impact_bps: number | null;
    readonly avg_exit_slippage_bps: number | null;
}

export interface ProtocolConfig {
    readonly as_of_slot: number;
    readonly source: 'initialize_config' | 'apply_config' | 'disable_venue' | 'account';
    readonly governance: Pubkey;
    readonly pause_authority: Pubkey;
    readonly coverage_authority: Pubkey | null;
    readonly fee_recipient: Pubkey;
    readonly timelock_s: number;
    readonly venue_enabled: number;
    readonly paused_opens: boolean;
    readonly route_broken_bits: number;
    readonly params: Readonly<Record<string, unknown>>;
    readonly tiers: readonly Readonly<Record<string, unknown>>[];
    readonly ema: Readonly<Record<string, unknown>>;
    readonly util_kink_bps: number;
    readonly pyth_sol_usd: Pubkey;
    readonly pyth_feed_id: string;
}

export interface VenueGuardRow {
    readonly venue: number;
    readonly name: 'pump' | 'pump_amm' | 'pump_fees' | 'raydium_v4' | 'raydium_cpmm' | 'jupiter' | 'unknown';
    readonly upgraded: boolean;
    readonly guard_slot: U64;
    readonly programdata_slot: U64;
    readonly flagged_at: number | null;
    readonly last_ts: number;
}

export interface VenueStatus {
    readonly venue_enabled: number | null;
    readonly route_broken_bits: number;
    readonly paused_opens: boolean;
    readonly guards: readonly VenueGuardRow[];
    readonly extras: readonly {
        venue: number;
        ix: number;
        version: number;
        keys: readonly string[];
        stable_version: number;
    }[];
    readonly admin_changes: readonly {
        account: Pubkey;
        field: string;
        venue: string;
        old_value: string | null;
        new_value: string;
        observed_slot: number;
    }[];
}

export interface SearchHit {
    readonly mint: Pubkey;
    readonly market: Pubkey;
    readonly symbol: string | null;
    readonly name: string | null;
    readonly match: 'mint' | 'symbol' | 'name';
}

export interface KeeperActivity {
    readonly keeper: Pubkey;
    readonly liquidations: number;
    readonly expiries: number;
    readonly settlements: number;
    readonly rewards_lamports: BigNum;
    readonly markets: number;
    readonly failures: number;
}

// ---- WebSocket (INDEXER.md §9.3) ----------------------------------------------------------

export type WsChannel =
    | 'markets'
    | 'liquidations'
    | 'admin'
    | 'coverage'
    | 'stats'
    | `market:${string}`
    | `position:${string}`
    | `lender:${string}`
    | `trades:${string}`;

export type WsServerMessage =
    | { readonly type: 'subscribed' | 'unsubscribed'; readonly channels: readonly WsChannel[] }
    | { readonly type: 'event'; readonly channel: WsChannel; readonly id: number; readonly data: IndexerEvent }
    | { readonly type: 'market'; readonly channel: WsChannel; readonly data: MarketSummary }
    | { readonly type: 'trade'; readonly channel: WsChannel; readonly data: VenueTrade }
    | { readonly type: 'stats'; readonly channel: WsChannel; readonly data: GlobalStats }
    | { readonly type: 'rollback'; readonly slot: number }
    | { readonly type: 'pong' | 'heartbeat' }
    | {
          readonly type: 'error';
          readonly code: 'bad_message' | 'too_many_subscriptions' | 'rate_limited' | 'lagged' | 'unavailable';
          readonly message: string;
      };
