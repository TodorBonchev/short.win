-- Distinct fixed-term quote-native collateral foreclosure after locked 24h grace.
-- No token buyback, market valuation, rent payment, or S1/S2/S3 classification is invented.
ALTER TABLE positions DROP CONSTRAINT positions_status_check;
ALTER TABLE positions ADD CONSTRAINT positions_status_check
    CHECK (status IN ('open', 'closed', 'liquidated', 'expired', 'settled', 'defaulted'));
ALTER TABLE positions DROP CONSTRAINT positions_exit_kind_check;
ALTER TABLE positions ADD CONSTRAINT positions_exit_kind_check
    CHECK (exit_kind IN ('Close', 'Repay', 'Liquidate', 'Expire', 'SolSettle', 'OverdueCollateralDefault'));

CREATE TABLE overdue_defaults (
    position                 pubkey PRIMARY KEY REFERENCES positions (position) ON DELETE CASCADE,
    market                   pubkey NOT NULL REFERENCES markets (market) ON DELETE CASCADE,
    borrower                 pubkey NOT NULL,
    keeper                   pubkey NOT NULL,
    q_lost                   u64 NOT NULL CHECK (q_lost > 0),
    default_at               BIGINT NOT NULL,
    collateral               u64 NOT NULL,
    keeper_payment           u64 NOT NULL,
    lender_amount            u64 NOT NULL,
    borrower_amount          u64 NOT NULL CHECK (borrower_amount = 0),
    rent_due                 u64 NOT NULL,
    credited_retired         u64 NOT NULL,
    vested                   u64 NOT NULL,
    vest_flushed             u64 NOT NULL,
    share_epoch_after        u32 NOT NULL,
    position_account_refund  u64 NOT NULL,
    signature                signature NOT NULL,
    ix_index                 SMALLINT NOT NULL,
    ev_source                SMALLINT NOT NULL,
    ev_seq                   INTEGER NOT NULL,
    slot                     BIGINT NOT NULL,
    tx_index                 INTEGER NOT NULL,
    ts                       BIGINT NOT NULL CHECK (ts >= default_at),
    CHECK (keeper_payment + lender_amount = collateral),
    FOREIGN KEY (signature, ix_index, ev_source, ev_seq)
        REFERENCES events (signature, ix_index, ev_source, ev_seq) ON DELETE CASCADE
);
CREATE INDEX overdue_defaults_market ON overdue_defaults (market, slot DESC);

ALTER TABLE fees DROP CONSTRAINT fees_fee_type_check;
ALTER TABLE fees ADD CONSTRAINT fees_fee_type_check CHECK (fee_type IN (
    'creation_fee',
    'open_fee',
    'venue_rent',
    'rent_lender',
    'rent_protocol',
    'liq_penalty_lender',
    'liq_penalty_protocol',
    'liq_penalty_keeper',
    'liq_keeper_topup',
    'expiry_fee',
    'expiry_keeper_reward',
    'settle_keeper_reward',
    'trader_sweep_lenders',
    'trader_sweep_wipe',
    'trader_sweep_protocol',
    'venue_rewards_lenders',
    'venue_rewards_wipe',
    'venue_rewards_protocol',
    'overdue_keeper_reward'));

CREATE OR REPLACE VIEW v_market_exits AS
SELECT market,
       count(*)                                                                    AS positions,
       count(*) FILTER (WHERE status = 'open')                                     AS open,
       count(*) FILTER (WHERE status = 'closed')                                   AS closes,
       count(*) FILTER (WHERE status = 'liquidated'
                          AND NOT COALESCE(liquidated_expired, FALSE))             AS liquidations_cr,
       count(*) FILTER (WHERE status = 'liquidated'
                          AND COALESCE(liquidated_expired, FALSE))                 AS liquidations_expired,
       count(*) FILTER (WHERE status = 'expired')                                  AS expiries,
       count(*) FILTER (WHERE status = 'settled' AND settle_condition = 1)         AS settlements_s1,
       count(*) FILTER (WHERE status = 'settled' AND settle_condition = 2)         AS settlements_s2,
       count(*) FILTER (WHERE status = 'settled' AND settle_condition = 3)         AS settlements_s3,
       count(*) FILTER (WHERE status <> 'open')                                    AS exits,
       avg(duration_s) FILTER (WHERE status <> 'open')                             AS avg_duration_s,
       COALESCE(sum(penalty) FILTER (WHERE status = 'liquidated'), 0)              AS liquidation_penalties,
       COALESCE(sum(q) FILTER (WHERE status = 'liquidated'), 0)                    AS liquidated_tokens,
       div(count(*) FILTER (WHERE status = 'closed') * 10000,
           NULLIF(count(*) FILTER (WHERE status <> 'open'), 0))::BIGINT            AS close_rate_bps,
       div(count(*) FILTER (WHERE status = 'liquidated') * 10000,
           NULLIF(count(*) FILTER (WHERE status <> 'open'), 0))::BIGINT            AS liquidation_rate_bps,
       div(count(*) FILTER (WHERE status = 'expired') * 10000,
           NULLIF(count(*) FILTER (WHERE status <> 'open'), 0))::BIGINT            AS expiry_rate_bps,
       div(count(*) FILTER (WHERE status = 'settled') * 10000,
           NULLIF(count(*) FILTER (WHERE status <> 'open'), 0))::BIGINT            AS settlement_rate_bps,
       avg(sw_open_impact_bps(q, spot_wad_open, s))                                AS avg_open_impact_bps,
       avg(sw_exit_slippage_bps(buyback_cost, b_live_exit))
           FILTER (WHERE status IN ('closed', 'liquidated', 'expired'))            AS avg_exit_slippage_bps,
       CASE WHEN count(*) FILTER (WHERE status <> 'open' AND borrower_pnl IS NULL) > 0
            THEN NULL
            ELSE COALESCE(sum(borrower_pnl) FILTER (WHERE status <> 'open'), 0)
       END                                                                         AS borrower_pnl,
       count(*) FILTER (WHERE status <> 'open' AND borrower_pnl IS NOT NULL)         AS borrower_pnl_known_exits,
       count(*) FILTER (WHERE status <> 'open' AND borrower_pnl IS NULL)             AS borrower_pnl_unknown_exits
     , count(*) FILTER (WHERE exit_kind = 'OverdueCollateralDefault')                AS overdue_collateral_defaults
     , COALESCE(sum((SELECT d.q_lost FROM overdue_defaults d WHERE d.position = positions.position)), 0)
                                                                                     AS overdue_principal_tokens_lost
     , COALESCE(sum((SELECT d.lender_amount FROM overdue_defaults d WHERE d.position = positions.position)), 0)
                                                                                     AS overdue_lender_collateral
FROM positions
GROUP BY market;

-- Defaults are keeper attempts/activity, with their own count rather than S3.
CREATE OR REPLACE FUNCTION sw_keeper_attempt(name TEXT) RETURNS BOOLEAN
    LANGUAGE SQL IMMUTABLE PARALLEL SAFE AS $$
    SELECT name IN ('crank_price', 'liquidate', 'expire', 'sol_settle', 'sweep_trader',
                    'propose_pool', 'execute_pool_switch', 'set_coverage_pause', 'overdue_quote_default')
$$;

CREATE OR REPLACE VIEW v_keeper_activity AS
WITH exits AS (
    SELECT exit_keeper AS keeper,
           market,
           count(*) FILTER (WHERE status = 'liquidated') AS liquidations,
           count(*) FILTER (WHERE status = 'expired')    AS expiries,
           count(*) FILTER (WHERE status = 'settled')    AS settlements,
           count(*) FILTER (WHERE status = 'defaulted') AS overdue_defaults
    FROM positions
    WHERE exit_keeper IS NOT NULL
    GROUP BY exit_keeper, market
), rewards AS (
    SELECT recipient_key AS keeper, market, sum(amount) AS rewards_lamports
    FROM fees
    WHERE recipient = 'keeper' AND recipient_key IS NOT NULL
    GROUP BY recipient_key, market
)
SELECT COALESCE(e.keeper, r.keeper)          AS keeper,
       COALESCE(e.market, r.market)          AS market,
       COALESCE(e.liquidations, 0)           AS liquidations,
       COALESCE(e.expiries, 0)               AS expiries,
       COALESCE(e.settlements, 0)            AS settlements,
       COALESCE(r.rewards_lamports, 0)       AS rewards_lamports,
       COALESCE(e.overdue_defaults, 0)       AS overdue_defaults
FROM exits e
FULL JOIN rewards r ON r.keeper = e.keeper AND r.market = e.market;
