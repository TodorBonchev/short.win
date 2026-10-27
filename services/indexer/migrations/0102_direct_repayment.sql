-- Borrower-delivered whole-debt token repayment. External token acquisition costs are
-- unknown: NULL economic PNL is preserved through the position API and aggregate views.
-- Retain status='closed' for successful borrower exits; exit_kind identifies the mechanism.
ALTER TABLE positions ADD COLUMN exit_kind TEXT
    CHECK (exit_kind IN ('Close', 'Repay', 'Liquidate', 'Expire', 'SolSettle'));

UPDATE positions SET exit_kind = CASE status
    WHEN 'closed' THEN 'Close' WHEN 'liquidated' THEN 'Liquidate'
    WHEN 'expired' THEN 'Expire' WHEN 'settled' THEN 'SolSettle' END
WHERE status <> 'open';

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
FROM positions
GROUP BY market;
