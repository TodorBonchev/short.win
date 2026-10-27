//! Borrower-delivered repayment must close debt without inventing acquisition cost or PNL.
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
async fn token_repayment_closes_ledgers_and_preserves_unknown_economic_pnl() {
    let f = Fixture::seeded().await;
    let market: Pubkey = MA.parse().unwrap();
    let borrower: Pubkey = B1.parse().unwrap();
    let position: Pubkey = P1.parse().unwrap();
    let slot = 2_000;
    let event = shortwin::events::Repay {
        market,
        slot: u64::try_from(slot).unwrap(),
        ts: ts(slot),
        position,
        borrower,
        q: 100_000_000,
        rent: 6_000,
        rent_protocol: 1_200,
        borrower_payout: 1_544_000,
    };
    let mut bytes = shortwin::events::Repay::DISCRIMINATOR.to_vec();
    event.serialize(&mut bytes).unwrap();
    let decoded = decode_shortwin_event(&bytes).expect("program and IDL agree");
    assert_eq!(decoded.kind, "Repay");
    assert_eq!(decoded.trailing, 0);
    assert_eq!(
        actor_of("Repay", &decoded.data, &Pubkey::new_unique()),
        Some(borrower)
    );
    let before: (String, String, String) = sqlx::query_as(
        "SELECT tokens_free::text, lender_sol::text, protocol_fees::text FROM markets WHERE market = $1",
    ).bind(MA).fetch_one(&f.admin).await.unwrap();
    let id = insert_event(
        &f.admin,
        99,
        slot,
        "Repay",
        Some(MA),
        Some(P1),
        decoded.data.clone(),
    )
    .await;
    let (signature, tx_index): (String, i32) = sqlx::query_as(
        "UPDATE events SET actor = $2, invocation = 'repay_short' WHERE id = $1 RETURNING signature::text, tx_index",
    ).bind(id).bind(B1).fetch_one(&f.admin).await.unwrap();
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
        fee_payer: borrower,
        invocation: "repay_short".into(),
        kind: "Repay".into(),
        market: Some(market),
        ev_ts: ts(slot),
        data: decoded.data,
        derived: None,
    });
    let mut conn = f.admin.acquire().await.unwrap();
    project::apply(&mut conn, std::slice::from_ref(&item), true)
        .await
        .unwrap();
    // Reapplying an already-exited position cannot restore tokens or charge rent twice.
    project::apply(&mut conn, &[item], false).await.unwrap();
    drop(conn);
    let row: Value = sqlx::query_scalar("SELECT to_jsonb(p) FROM positions p WHERE position = $1")
        .bind(P1)
        .fetch_one(&f.admin)
        .await
        .unwrap();
    assert_eq!(row["status"], "closed");
    assert_eq!(row["exit_kind"], "Repay");
    assert_eq!(row["borrower_payout"].as_u64(), Some(1_544_000));
    assert_eq!(row["rent_paid"].as_u64(), Some(6_000));
    assert_eq!(row["rent_protocol"].as_u64(), Some(1_200));
    for name in [
        "buyback_cost",
        "b_live_exit",
        "borrower_pnl",
        "exit_keeper",
        "keeper_paid",
    ] {
        assert!(row[name].is_null(), "{name}: {row}");
    }
    let after: (String, String, String, String, i64, String, String) = sqlx::query_as(
        "SELECT tokens_free::text, lender_sol::text, protocol_fees::text, borrowed_q::text, open_positions::bigint, \
         total_notional_open::text, open_vault_lamports::text FROM markets WHERE market = $1",
    ).bind(MA).fetch_one(&f.admin).await.unwrap();
    assert_eq!(
        after.0.parse::<u64>().unwrap(),
        before.0.parse::<u64>().unwrap() + event.q
    );
    assert_eq!(
        after.1.parse::<u64>().unwrap(),
        before.1.parse::<u64>().unwrap() + 4_800
    );
    assert_eq!(
        after.2.parse::<u64>().unwrap(),
        before.2.parse::<u64>().unwrap() + 1_200
    );
    assert_eq!(
        (
            after.3.as_str(),
            after.4,
            after.5.as_str(),
            after.6.as_str()
        ),
        ("0", 0, "0", "0")
    );
    let fees: Vec<(String, String)> = sqlx::query_as(
        "SELECT fee_type, amount::text FROM fees WHERE position = $1 AND signature = $2 ORDER BY fee_type",
    ).bind(P1).bind(row["exit_signature"].as_str().unwrap()).fetch_all(&f.admin).await.unwrap();
    assert_eq!(
        fees,
        vec![
            ("rent_lender".into(), "4800".into()),
            ("rent_protocol".into(), "1200".into())
        ]
    );
    let totals: (Option<String>, i64, i64) = sqlx::query_as(
        "SELECT borrower_pnl::text, borrower_pnl_known_exits, borrower_pnl_unknown_exits FROM v_market_exits WHERE market = $1",
    ).bind(MA).fetch_one(&f.admin).await.unwrap();
    assert_eq!(
        totals,
        (None, 4, 1),
        "a partial known sum is not the market's total PNL"
    );

    let app = f.router(relaxed());
    let contract = Contract::load();
    let response = get(&app, &format!("/v1/positions/{P1}")).await;
    contract.check("getPosition", "Repay position", &response);
    assert_eq!(response.body["position"]["exit"]["kind"], "Repay");
    assert!(response.body["position"]["exit"]["borrower_pnl"].is_null());
    assert!(response.body["position"]["exit"]["buyback_cost"].is_null());
    let response = get(&app, &format!("/v1/markets/{TA}")).await;
    contract.check(
        "getMarket",
        "market with unknown acquisition costs",
        &response,
    );
    assert!(response.body["exits"]["borrower_pnl"].is_null());
    assert_eq!(response.body["exits"]["borrower_pnl_known_exits"], 4);
    assert_eq!(response.body["exits"]["borrower_pnl_unknown_exits"], 1);
    let response = get(&app, &format!("/v1/positions/{P2}")).await;
    contract.check("getPosition", "ordinary Close remains unchanged", &response);
    assert_eq!(response.body["position"]["exit"]["kind"], "Close");
    assert_eq!(response.body["position"]["exit"]["buyback_cost"], "470000");
    assert_eq!(response.body["position"]["exit"]["borrower_pnl"], "-1000");
    drop(app);
    f.stop().await;
}
