-- Preserve full canonical event ordering in configuration history and position lifecycles.
-- Owner: INGEST.

ALTER TABLE protocol_params_history
    ADD COLUMN IF NOT EXISTS ev_source SMALLINT NOT NULL DEFAULT -1,
    ADD COLUMN IF NOT EXISTS ev_seq INTEGER NOT NULL DEFAULT -1;

ALTER TABLE protocol_params_history DROP CONSTRAINT protocol_params_history_pkey;
ALTER TABLE protocol_params_history
    ADD PRIMARY KEY (signature, ix_index, ev_source, ev_seq);
DROP INDEX protocol_params_history_order;
CREATE INDEX protocol_params_history_order ON protocol_params_history (
    slot, tx_index, ix_index, ev_source, ev_seq
);
ALTER TABLE protocol_params_history
    ALTER COLUMN ev_source DROP DEFAULT,
    ALTER COLUMN ev_seq DROP DEFAULT;

-- A position PDA is reusable after a terminal exit. The old projection retained only one row per
-- PDA, so an upgrade cannot prove which Open produced that row when a transaction contains more
-- than one lifecycle. Do not guess an event key: discard these derived rows and rebuild them from
-- canonical events below.
ALTER TABLE overdue_defaults DROP CONSTRAINT overdue_defaults_position_fkey;
DELETE FROM overdue_defaults;
DELETE FROM settlements;
DELETE FROM positions;

ALTER TABLE positions DROP CONSTRAINT positions_pkey;
ALTER TABLE positions
    ADD COLUMN opened_ix_index SMALLINT NOT NULL,
    ADD COLUMN opened_ev_source SMALLINT NOT NULL,
    ADD COLUMN opened_ev_seq INTEGER NOT NULL,
    ADD COLUMN is_current BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN exit_tx_index INTEGER,
    ADD COLUMN exit_ix_index SMALLINT,
    ADD COLUMN exit_ev_source SMALLINT,
    ADD COLUMN exit_ev_seq INTEGER,
    ADD PRIMARY KEY (
        position, open_signature, opened_ix_index, opened_ev_source, opened_ev_seq
    ),
    ADD CONSTRAINT positions_noncurrent_terminal
        CHECK (is_current OR status <> 'open'),
    ADD CONSTRAINT positions_exit_key_complete
        CHECK (
            (exit_slot IS NULL AND exit_tx_index IS NULL AND exit_ix_index IS NULL
             AND exit_ev_source IS NULL AND exit_ev_seq IS NULL)
            OR
            (exit_slot IS NOT NULL AND exit_tx_index IS NOT NULL AND exit_ix_index IS NOT NULL
             AND exit_ev_source IS NOT NULL AND exit_ev_seq IS NOT NULL)
        );
CREATE UNIQUE INDEX positions_current_pda ON positions (position) WHERE is_current;
CREATE UNIQUE INDEX positions_lifecycle_order ON positions (
    position, opened_slot, opened_tx_index, opened_ix_index, opened_ev_source, opened_ev_seq
);

ALTER TABLE settlements DROP CONSTRAINT settlements_pkey;
ALTER TABLE settlements
    ADD COLUMN open_signature signature NOT NULL,
    ADD COLUMN opened_ix_index SMALLINT NOT NULL,
    ADD COLUMN opened_ev_source SMALLINT NOT NULL,
    ADD COLUMN opened_ev_seq INTEGER NOT NULL,
    ADD PRIMARY KEY (
        position, open_signature, opened_ix_index, opened_ev_source, opened_ev_seq
    ),
    ADD FOREIGN KEY (
        position, open_signature, opened_ix_index, opened_ev_source, opened_ev_seq
    ) REFERENCES positions (
        position, open_signature, opened_ix_index, opened_ev_source, opened_ev_seq
    ) ON DELETE CASCADE;

ALTER TABLE overdue_defaults DROP CONSTRAINT overdue_defaults_pkey;
ALTER TABLE overdue_defaults
    ADD COLUMN open_signature signature NOT NULL,
    ADD COLUMN opened_ix_index SMALLINT NOT NULL,
    ADD COLUMN opened_ev_source SMALLINT NOT NULL,
    ADD COLUMN opened_ev_seq INTEGER NOT NULL,
    ADD PRIMARY KEY (
        position, open_signature, opened_ix_index, opened_ev_source, opened_ev_seq
    ),
    ADD FOREIGN KEY (
        position, open_signature, opened_ix_index, opened_ev_source, opened_ev_seq
    ) REFERENCES positions (
        position, open_signature, opened_ix_index, opened_ev_source, opened_ev_seq
    ) ON DELETE CASCADE;

-- The per-market exit view now joins a default to its immutable lifecycle rather than only its
-- reusable PDA.
CREATE OR REPLACE VIEW v_market_exits AS
SELECT p.market,
       count(*)                                                                    AS positions,
       count(*) FILTER (WHERE p.status = 'open')                                   AS open,
       count(*) FILTER (WHERE p.status = 'closed')                                 AS closes,
       count(*) FILTER (WHERE p.status = 'liquidated'
                          AND NOT COALESCE(p.liquidated_expired, FALSE))           AS liquidations_cr,
       count(*) FILTER (WHERE p.status = 'liquidated'
                          AND COALESCE(p.liquidated_expired, FALSE))               AS liquidations_expired,
       count(*) FILTER (WHERE p.status = 'expired')                                AS expiries,
       count(*) FILTER (WHERE p.status = 'settled' AND p.settle_condition = 1)     AS settlements_s1,
       count(*) FILTER (WHERE p.status = 'settled' AND p.settle_condition = 2)     AS settlements_s2,
       count(*) FILTER (WHERE p.status = 'settled' AND p.settle_condition = 3)     AS settlements_s3,
       count(*) FILTER (WHERE p.status <> 'open')                                  AS exits,
       avg(p.duration_s) FILTER (WHERE p.status <> 'open')                         AS avg_duration_s,
       COALESCE(sum(p.penalty) FILTER (WHERE p.status = 'liquidated'), 0)          AS liquidation_penalties,
       COALESCE(sum(p.q) FILTER (WHERE p.status = 'liquidated'), 0)                AS liquidated_tokens,
       div(count(*) FILTER (WHERE p.status = 'closed') * 10000,
           NULLIF(count(*) FILTER (WHERE p.status <> 'open'), 0))::BIGINT          AS close_rate_bps,
       div(count(*) FILTER (WHERE p.status = 'liquidated') * 10000,
           NULLIF(count(*) FILTER (WHERE p.status <> 'open'), 0))::BIGINT          AS liquidation_rate_bps,
       div(count(*) FILTER (WHERE p.status = 'expired') * 10000,
           NULLIF(count(*) FILTER (WHERE p.status <> 'open'), 0))::BIGINT          AS expiry_rate_bps,
       div(count(*) FILTER (WHERE p.status = 'settled') * 10000,
           NULLIF(count(*) FILTER (WHERE p.status <> 'open'), 0))::BIGINT          AS settlement_rate_bps,
       avg(sw_open_impact_bps(p.q, p.spot_wad_open, p.s))                          AS avg_open_impact_bps,
       avg(sw_exit_slippage_bps(p.buyback_cost, p.b_live_exit))
           FILTER (WHERE p.status IN ('closed', 'liquidated', 'expired'))          AS avg_exit_slippage_bps,
       CASE WHEN count(*) FILTER (WHERE p.status <> 'open' AND p.borrower_pnl IS NULL) > 0
            THEN NULL
            ELSE COALESCE(sum(p.borrower_pnl) FILTER (WHERE p.status <> 'open'), 0)
       END                                                                         AS borrower_pnl,
       count(*) FILTER (WHERE p.status <> 'open' AND p.borrower_pnl IS NOT NULL)   AS borrower_pnl_known_exits,
       count(*) FILTER (WHERE p.status <> 'open' AND p.borrower_pnl IS NULL)       AS borrower_pnl_unknown_exits,
       count(*) FILTER (WHERE p.exit_kind = 'OverdueCollateralDefault')            AS overdue_collateral_defaults,
       COALESCE(sum(d.q_lost), 0)                                                   AS overdue_principal_tokens_lost,
       COALESCE(sum(d.lender_amount), 0)                                            AS overdue_lender_collateral
FROM positions p
LEFT JOIN overdue_defaults d
  ON d.position = p.position
 AND (d.open_signature, d.opened_ix_index, d.opened_ev_source, d.opened_ev_seq)
     = (p.open_signature, p.opened_ix_index, p.opened_ev_source, p.opened_ev_seq)
GROUP BY p.market;

-- Both projections are derived. The writer repairs these markers before accepting a new batch;
-- rollups now refuse to advance while either marker is set. Rewind completed hourly buckets so
-- the canonical replay also repairs previously omitted lifecycle flows.
UPDATE protocol_state SET needs_rebuild = TRUE WHERE id;
UPDATE markets SET needs_rebuild = TRUE;
UPDATE rollup_state
SET complete_before = LEAST(
        complete_before,
        COALESCE((SELECT (min(ev_ts) / 3600) * 3600 FROM events WHERE market IS NOT NULL),
                 complete_before)
    ),
    updated_at = now()
WHERE name IN ('market_stats_hourly', 'global_stats_hourly');
