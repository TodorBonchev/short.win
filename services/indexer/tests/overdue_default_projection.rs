//! Actual program event ABI -> distinct default projection/API/rebuild, no fake trade valuation.
mod api_support;
use anchor_lang::{AnchorSerialize, Discriminator};
use api_support::*;
use serde_json::Value;
use shortwin_indexer::ingest::decode::shortwin_events::{actor_of, decode_shortwin_event};
use shortwin_indexer::ingest::project::{self, EventIn, Item};
use shortwin_indexer::ingest::store::ChainKey;
use solana_pubkey::Pubkey;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn overdue_foreclosure_replays_as_nominal_token_loss_and_actual_collateral_distribution() {
    let f = Fixture::seeded().await;
    let market: Pubkey = MA.parse().unwrap();
    let borrower: Pubkey = B1.parse().unwrap();
    let keeper: Pubkey = K1.parse().unwrap();
    let position: Pubkey = P1.parse().unwrap();
    let slot = 2_200;
    let (default_at, collateral, q): (i64, String, String) =
        sqlx::query_as("SELECT deadline+86400,v::text,q::text FROM positions WHERE position=$1")
            .bind(P1)
            .fetch_one(&f.admin)
            .await
            .unwrap();
    let event = shortwin::events::OverdueCollateralDefault {
        market,
        slot: slot as u64,
        ts: ts(slot),
        position,
        borrower,
        keeper,
        q_lost: q.parse().unwrap(),
        default_at,
        collateral: collateral.parse().unwrap(),
        keeper_payment: 50_000,
        lender_amount: 1_500_000,
        borrower_amount: 0,
        rent_due: 6_000,
        credited_retired: 4_800,
        vested: 0,
        vest_flushed: 5_000_100,
        share_epoch_after: 0,
        position_account_refund: 2_000_000,
    };
    let mut bytes = shortwin::events::OverdueCollateralDefault::DISCRIMINATOR.to_vec();
    event.serialize(&mut bytes).unwrap();
    let decoded = decode_shortwin_event(&bytes).expect("actual program and current IDL agree");
    assert_eq!(decoded.kind, "OverdueCollateralDefault");
    assert_eq!(decoded.trailing, 0);
    assert_eq!(
        actor_of(decoded.kind, &decoded.data, &borrower),
        Some(keeper)
    );
    assert!(decode_shortwin_event(&bytes[..bytes.len() - 1]).is_err());
    let keeper_settlements_before: Option<i64> = sqlx::query_scalar(
        "SELECT sum(settlements)::bigint FROM v_keeper_activity WHERE keeper=$1",
    )
    .bind(K1)
    .fetch_one(&f.admin)
    .await
    .unwrap();
    // A near-wipe can flush vest without rolling the share epoch. The typed default
    // event must clear this last-known vest snapshot even with no EpochRolled event.
    sqlx::query("UPDATE markets SET vest_bal=5000000 WHERE market=$1")
        .bind(MA)
        .execute(&f.admin)
        .await
        .unwrap();
    let before:(String,String,String)=sqlx::query_as("SELECT tokens_free::text,lender_sol::text,protocol_fees::text FROM markets WHERE market=$1")
        .bind(MA).fetch_one(&f.admin).await.unwrap();
    let id = insert_event(
        &f.admin,
        100,
        slot,
        decoded.kind,
        Some(MA),
        Some(P1),
        decoded.data.clone(),
    )
    .await;
    let(signature,tx_index):(String,i32)=sqlx::query_as("UPDATE events SET actor=$2,invocation='overdue_quote_default' WHERE id=$1 RETURNING signature::text,tx_index")
        .bind(id).bind(K1).fetch_one(&f.admin).await.unwrap();
    let item = Item::Event(EventIn {
        signature: signature.clone(),
        key: ChainKey {
            slot,
            tx_index,
            ix_index: 0,
            ev_source: 0,
            ev_seq: 0,
        },
        block_time: Some(ts(slot)),
        fee_payer: keeper,
        invocation: "overdue_quote_default".into(),
        kind: decoded.kind.into(),
        market: Some(market),
        ev_ts: ts(slot),
        data: decoded.data,
        derived: None,
    });
    let mut conn = f.admin.acquire().await.unwrap();
    let mut invalid = item.clone();
    let Item::Event(invalid_event) = &mut invalid else {
        unreachable!()
    };
    invalid_event.data["q_lost"] = serde_json::json!("1");
    assert!(project::apply(&mut conn, &[invalid], false).await.is_err());
    let status: String = sqlx::query_scalar("SELECT status FROM positions WHERE position=$1")
        .bind(P1)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(status, "open");
    project::apply(&mut conn, std::slice::from_ref(&item), true)
        .await
        .unwrap();
    project::apply(&mut conn, &[item], false).await.unwrap();
    drop(conn);
    let row: Value = sqlx::query_scalar("SELECT to_jsonb(p) FROM positions p WHERE position=$1")
        .bind(P1)
        .fetch_one(&f.admin)
        .await
        .unwrap();
    assert_eq!(row["status"], "defaulted");
    assert_eq!(row["exit_kind"], "OverdueCollateralDefault");
    assert_eq!(row["borrower_payout"].as_u64(), Some(0));
    assert_eq!(row["keeper_paid"].as_u64(), Some(50_000));
    for key in [
        "buyback_cost",
        "b_live_exit",
        "rent_paid",
        "rent_protocol",
        "penalty",
        "expiry_fee",
        "token_surplus",
        "settle_condition",
        "borrower_pnl",
    ] {
        assert!(row[key].is_null(), "{key}: {row}");
    }
    let default_row: Value =
        sqlx::query_scalar("SELECT to_jsonb(d) FROM overdue_defaults d WHERE position=$1")
            .bind(P1)
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert_eq!(default_row["q_lost"].as_u64(), Some(100_000_000));
    assert_eq!(default_row["lender_amount"].as_u64(), Some(1_500_000));
    assert_eq!(default_row["rent_due"].as_u64(), Some(6_000));
    assert_eq!(
        default_row["position_account_refund"].as_u64(),
        Some(2_000_000)
    );
    let after:(String,String,String,String,i64,String,String)=sqlx::query_as("SELECT tokens_free::text,lender_sol::text,protocol_fees::text,borrowed_q::text,open_positions::bigint,total_notional_open::text,open_vault_lamports::text FROM markets WHERE market=$1")
        .bind(MA).fetch_one(&f.admin).await.unwrap();
    assert_eq!(
        after.0, before.0,
        "lost token principal was not repaid to inventory"
    );
    assert_eq!(
        after.1.parse::<u64>().unwrap(),
        before.1.parse::<u64>().unwrap() + 1_500_000
    );
    assert_eq!(after.2, before.2, "no protocol cut inferred from rent due");
    let vest_after_default: Option<String> =
        sqlx::query_scalar("SELECT vest_bal::text FROM markets WHERE market=$1")
            .bind(MA)
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert_eq!(vest_after_default.as_deref(), Some("0"));
    let rolls: i64 =
        sqlx::query_scalar("SELECT count(*) FROM events WHERE signature=$1 AND kind='EpochRolled'")
            .bind(&signature)
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert_eq!(rolls, 0, "flush projection does not depend on a roll");
    assert_eq!(
        (
            after.3.as_str(),
            after.4,
            after.5.as_str(),
            after.6.as_str()
        ),
        ("0", 0, "0", "0")
    );
    let fees: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT fee_type,recipient,amount::text FROM fees WHERE position=$1 AND signature=$2",
    )
    .bind(P1)
    .bind(&signature)
    .fetch_all(&f.admin)
    .await
    .unwrap();
    assert_eq!(
        fees,
        vec![(
            "overdue_keeper_reward".into(),
            "keeper".into(),
            "50000".into()
        )]
    );
    let fake_settlements: i64 =
        sqlx::query_scalar("SELECT count(*) FROM settlements WHERE position=$1")
            .bind(P1)
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert_eq!(fake_settlements, 0);
    let stats:(i64,String,String)=sqlx::query_as("SELECT overdue_collateral_defaults,overdue_principal_tokens_lost::text,overdue_lender_collateral::text FROM v_market_exits WHERE market=$1")
        .bind(MA).fetch_one(&f.admin).await.unwrap();
    assert_eq!(stats, (1, "100000000".into(), "1500000".into()));
    // Rebuild from retained, typed decoded events, including earlier opens/margin,
    // reproduces exactly the same terminal row and collateral ledgers.
    project::rebuild_market(&f.admin, &market).await.unwrap();
    let replayed: Value =
        sqlx::query_scalar("SELECT to_jsonb(d) FROM overdue_defaults d WHERE position=$1")
            .bind(P1)
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert_eq!(replayed, default_row);
    let replayed_vest: Option<String> =
        sqlx::query_scalar("SELECT vest_bal::text FROM markets WHERE market=$1")
            .bind(MA)
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert_eq!(replayed_vest.as_deref(), Some("0"));
    let app = f.router(relaxed());
    let contract = Contract::load();
    let response = get(&app, &format!("/v1/positions/{P1}")).await;
    contract.check("getPosition", "overdue default", &response);
    assert_eq!(response.body["position"]["status"], "defaulted");
    assert_eq!(
        response.body["position"]["exit"]["kind"],
        "OverdueCollateralDefault"
    );
    assert_eq!(
        response.body["position"]["exit"]["overdue_default"]["q_lost"],
        "100000000"
    );
    assert!(response.body["position"]["exit"]["borrower_pnl"].is_null());
    assert!(response.body["settlement"].is_null());
    assert_eq!(
        response.body["position"]["exit"]["overdue_default"]["vest_flushed"],
        "5000100"
    );
    let api_default = response.body["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "OverdueCollateralDefault")
        .unwrap();
    assert_eq!(api_default["data"]["vest_flushed"], "5000100");
    let response = get(&app, &format!("/v1/markets/{TA}/liquidations")).await;
    contract.check(
        "listMarketLiquidations",
        "distinct overdue default",
        &response,
    );
    let first = &response.body["data"][0];
    assert_eq!(first["kind"], "overdue_default");
    assert_eq!(first["lender_amount"], "1500000");
    assert!(first["buyback_cost"].is_null());
    assert!(first["settle_condition"].is_null());
    let response = get(
        &app,
        &format!("/v1/markets/{TA}/positions?status=defaulted"),
    )
    .await;
    contract.check("listMarketPositions", "defaulted filter", &response);
    assert_eq!(response.body["data"].as_array().unwrap().len(), 1);
    let response = get(&app, "/v1/keepers").await;
    contract.check("listKeepers", "distinct default keeper activity", &response);
    let keeper_row = response.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["keeper"] == K1)
        .unwrap();
    assert_eq!(keeper_row["overdue_defaults"], 1);
    assert_eq!(
        keeper_row["settlements"].as_i64(),
        Some(keeper_settlements_before.unwrap_or(0))
    );
    let response = get(&app, &format!("/v1/positions/{P2}")).await;
    contract.check("getPosition", "historical Close unaffected", &response);
    assert_eq!(response.body["position"]["exit"]["kind"], "Close");
    assert_eq!(response.body["position"]["exit"]["buyback_cost"], "470000");
    drop(app);
    f.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rolling_default_record_prefunding_is_old_lender_income_on_rebuild() {
    let f = Fixture::seeded().await;
    let market: Pubkey = MA.parse().unwrap();
    let keeper: Pubkey = K1.parse().unwrap();
    let slot = 2_201;
    // Normalize the API fixture's enriched account snapshots to its retained event ledger
    // before comparing incremental projection with a full event rebuild.
    project::rebuild_market(&f.admin, &market).await.unwrap();
    let before: String = sqlx::query_scalar("SELECT lender_sol::text FROM markets WHERE market=$1")
        .bind(MA)
        .fetch_one(&f.admin)
        .await
        .unwrap();
    let event = shortwin::events::EpochRolled {
        market,
        slot: slot as u64,
        ts: ts(slot),
        epoch: 0,
        acc_end: 0,
        num: 1,
        den: 1000,
        vest_flushed: 100,
    };
    let mut bytes = shortwin::events::EpochRolled::DISCRIMINATOR.to_vec();
    event.serialize(&mut bytes).unwrap();
    let decoded = decode_shortwin_event(&bytes).unwrap();
    // Decoder's separate exact-ABI regression derives777 only from the record->vault CPI.
    let derived = serde_json::json!({"record_donation":"777"});
    let id = insert_event(
        &f.admin,
        199,
        slot,
        decoded.kind,
        Some(MA),
        None,
        decoded.data.clone(),
    )
    .await;
    let (signature,tx_index):(String,i32)=sqlx::query_as(
        "UPDATE events SET actor=$2,invocation='overdue_quote_default',derived=$3 WHERE id=$1 RETURNING signature::text,tx_index")
        .bind(id).bind(K1).bind(&derived).fetch_one(&f.admin).await.unwrap();
    let item = Item::Event(EventIn {
        signature,
        key: ChainKey {
            slot,
            tx_index,
            ix_index: 0,
            ev_source: 0,
            ev_seq: 0,
        },
        block_time: Some(ts(slot)),
        fee_payer: keeper,
        invocation: "overdue_quote_default".into(),
        kind: decoded.kind.into(),
        market: Some(market),
        ev_ts: ts(slot),
        data: decoded.data,
        derived: Some(derived),
    });
    let mut conn = f.admin.acquire().await.unwrap();
    project::apply(&mut conn, std::slice::from_ref(&item), true)
        .await
        .unwrap();
    let duplicate = project::apply(&mut conn, &[item], true).await.unwrap();
    assert!(duplicate.rebuild_markets.contains(&market));
    drop(conn);
    let after: (String, Option<String>, i64) = sqlx::query_as(
        "SELECT lender_sol::text,vest_bal::text,share_epoch::bigint FROM markets WHERE market=$1",
    )
    .bind(MA)
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert_eq!(
        after.0.parse::<u64>().unwrap(),
        before.parse::<u64>().unwrap() + 777
    );
    assert_eq!(after.1.as_deref(), Some("777"));
    assert_eq!(after.2, 1);
    project::rebuild_market(&f.admin, &market).await.unwrap();
    let replayed: (String, Option<String>, i64) = sqlx::query_as(
        "SELECT lender_sol::text,vest_bal::text,share_epoch::bigint FROM markets WHERE market=$1",
    )
    .bind(MA)
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert_eq!(replayed, after);
    f.stop().await;
}
