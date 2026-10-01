//! INGEST on LiteSVM-recorded sessions (INDEXER.md §3, §11.5): every committed session through
//! `rpc::to_raw_tx`, the decoder, the writer and the projections into an embedded PostgreSQL,
//! compared with the chain's end state; the catalog's coverage; duplicate and out-of-order
//! delivery; rebuilds; a simulated rollback through the finality tracker; the single-writer lock;
//! the NOTIFY fan-out; short.win's venue legs against their own events.

mod ingest_common;

use std::collections::{BTreeMap, BTreeSet};

use ingest_common::*;
use shortwin_indexer::ingest::finality::{self, BlockCheck, StatusSource, TxStatus};
use shortwin_indexer::ingest::raw::{Commitment, RawTx};
use shortwin_indexer::ingest::session::Session;
use shortwin_indexer::ingest::writer::Writer;
use shortwin_indexer::ingest::{self, replay, rpc};
use solana_signature::Signature;
use sqlx::PgPool;

/// The sessions the ordering, duplicate and rollback tests replay: a Raydium market's pool
/// adoption, venue legs and a liquidation; a SOL settlement whose full wipe rolls the epoch
/// through a pre-funded record (the §6.4 donation); governance, pauses and lender payouts.
const LIFECYCLE: &str = "raydium_switch_liquidation.jsonl";
const EPOCH_ROLL: &str = "epoch_roll_prefunded_record.jsonl";
const GOVERNANCE: &str = "admin_governance_and_pauses.jsonl";

fn raws(s: &Session, commitment: Commitment) -> Vec<RawTx> {
    s.txs
        .iter()
        .map(|t| rpc::to_raw_tx(&t.transaction, t.tx_index, commitment).expect("converts"))
        .collect()
}

/// Writes `txs` through one writer (no checkpoints), ending the batch after each `batch` writes.
async fn write_all(
    pool: &PgPool,
    s: &Session,
    txs: &[RawTx],
    batch: usize,
) -> ingest::IngestReport {
    let ids = replay::program_ids(s).expect("ids");
    let mut w = Writer::open(pool, ids, "replay").await.expect("writer");
    for chunk in txs.chunks(batch.max(1)) {
        for t in chunk {
            w.write(t).await.expect("write");
        }
        w.end_batch().await.expect("end batch");
    }
    w.close().await.expect("close")
}

async fn kinds(pool: &PgPool) -> BTreeMap<String, i64> {
    let rows: Vec<(String, i16, i64)> =
        sqlx::query_as("SELECT kind, ev_source, count(*) FROM events GROUP BY kind, ev_source")
            .fetch_all(pool)
            .await
            .expect("kinds");
    rows.into_iter()
        .map(|(k, src, n)| (if src == 1 { format!("{k}[L]") } else { k }, n))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_session_ingests_without_quarantine_and_matches_the_chain() {
    let pg = pg().await;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut trades = 0i64;
    let sessions = all_sessions();
    assert!(!sessions.is_empty());
    for (name, s) in &sessions {
        let pool = pg.fresh_db().await.expect("db");
        let report = ingest::ingest_session(&pool, s, 0..s.txs.len())
            .await
            .unwrap_or_else(|e| panic!("{name}: {e:#}"));
        // A recorded VM may execute one signed transaction twice from a restored state (a
        // test's rewind); the chain could not, and the writer keeps the first (§3.1).
        let distinct: BTreeSet<String> = raws(s, Commitment::Finalized)
            .iter()
            .map(|t| t.signature.to_string())
            .collect();
        assert_eq!(report.txs_seen, s.txs.len() as u64, "{name}");
        assert_eq!(report.txs_written, distinct.len() as u64, "{name}");
        assert_eq!(
            report.txs_written + report.txs_duplicate,
            s.txs.len() as u64,
            "{name}"
        );
        assert_eq!(report.quarantined, 0, "{name}");
        let q: i64 = sqlx::query_scalar("SELECT count(*) FROM ingest_quarantine")
            .fetch_one(&pool)
            .await
            .expect("quarantine");
        assert_eq!(q, 0, "{name}: quarantined payloads");
        let mismatches = end_state_mismatches(&pool, s).await;
        assert!(
            mismatches.is_empty(),
            "{name}: the index disagrees with the chain:\n{}",
            mismatches.join("\n")
        );
        seen.extend(kinds(&pool).await.into_keys());
        trades +=
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM venue_trades WHERE is_shortwin")
                .fetch_one(&pool)
                .await
                .expect("trades");
        // The ledger identity of §11.4 (6): protocol_fees = Σ protocol fees − Σ sweeps.
        let bad: Vec<String> = sqlx::query_scalar(
            "SELECT m.market::text FROM markets m \
             WHERE m.protocol_fees <> COALESCE((SELECT sum(amount) FROM fees f \
                 WHERE f.market = m.market AND f.recipient = 'protocol'), 0) \
               - COALESCE((SELECT sum(amount) FROM protocol_sweeps s WHERE s.market = m.market), 0)",
        )
        .fetch_all(&pool)
        .await
        .expect("identity");
        assert!(
            bad.is_empty(),
            "{name}: protocol_fees ≠ Σ protocol fees − Σ sweeps for {bad:?}"
        );
        pool.close().await;
    }
    // Every event the program emits appears at least once across the session set (§11.4 (12)).
    // `VenueRewardsClaimed` is never emitted (claim_venue_rewards is not built).
    let mut missing: Vec<&str> = shortwin_indexer::model::SHORTWIN_EVENTS
        .iter()
        .map(|s| s.name)
        .filter(|n| *n != "VenueRewardsClaimed" && *n != "CoverageGap")
        .filter(|n| !seen.contains(*n))
        .collect();
    for log_kind in [
        "CoverageGap[L]",
        "MintSafetyTripped[L]",
        "CurveCompleted[L]",
    ] {
        if !seen.contains(log_kind) {
            missing.push(log_kind);
        }
    }
    assert!(
        missing.is_empty(),
        "event kinds no session covers: {missing:?}"
    );
    assert!(trades > 0, "no short.win venue leg decoded");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn re_ingesting_writes_nothing_and_a_rebuild_reproduces_every_projection() {
    let pg = pg().await;
    for name in [LIFECYCLE, EPOCH_ROLL, GOVERNANCE] {
        let s = load(name);
        let pool = pg.fresh_db().await.expect("db");
        ingest::ingest_session(&pool, &s, 0..s.txs.len())
            .await
            .expect("ingest");
        let first = dump(&pool, &[]).await;
        // §11.4 (8): idempotent re-ingest.
        let again = ingest::ingest_session(&pool, &s, 0..s.txs.len())
            .await
            .expect("again");
        assert_eq!(again.txs_written, 0, "{name}");
        assert_eq!(again.txs_duplicate, s.txs.len() as u64, "{name}");
        assert_same(
            &first,
            &dump(&pool, &[]).await,
            &format!("{name}: re-ingest"),
        );
        // §11.4 (9): a rebuild of everything, then of each market alone.
        let n = ingest::rebuild(&pool, None).await.expect("rebuild");
        assert!(n >= 1, "{name}: no market rebuilt");
        assert_same(
            &first,
            &dump(&pool, &[]).await,
            &format!("{name}: rebuild(None)"),
        );
        let markets: Vec<String> = sqlx::query_scalar("SELECT market::text FROM markets")
            .fetch_all(&pool)
            .await
            .expect("markets");
        for m in markets {
            ingest::rebuild(&pool, Some(m.parse().expect("key")))
                .await
                .expect("rebuild one");
        }
        assert_same(
            &first,
            &dump(&pool, &[]).await,
            &format!("{name}: rebuild(market)"),
        );
        pool.close().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn out_of_order_and_duplicate_delivery_converge_to_the_in_order_database() {
    let pg = pg().await;
    for name in [LIFECYCLE, EPOCH_ROLL, GOVERNANCE] {
        let s = load(name);
        let txs = raws(&s, Commitment::Finalized);
        let in_order = pg.fresh_db().await.expect("db");
        write_all(&in_order, &s, &txs, 1).await;
        let want = dump(&in_order, &[]).await;

        // §11.4 (10): the second half first (a gap-fill writing history after newer events),
        // in one batch, then the first half.
        let gap = pg.fresh_db().await.expect("db");
        let half = txs.len() / 2;
        let mut reordered: Vec<RawTx> = txs[half..].to_vec();
        reordered.extend_from_slice(&txs[..half]);
        let report = write_all(&gap, &s, &reordered, txs.len()).await;
        assert!(
            report.markets_rebuilt >= 1,
            "{name}: the late history marked nothing"
        );
        assert_same(
            &want,
            &dump(&gap, &[]).await,
            &format!("{name}: second half first"),
        );

        // Reversed blocks of three, every transaction delivered twice, a batch per block.
        let shuffled = pg.fresh_db().await.expect("db");
        let mut blocks: Vec<Vec<RawTx>> = txs.chunks(3).map(<[RawTx]>::to_vec).collect();
        blocks.reverse();
        let mut doubled = Vec::new();
        for b in &blocks {
            for t in b {
                doubled.push(t.clone());
                doubled.push(t.clone());
            }
        }
        let report = write_all(&shuffled, &s, &doubled, 6).await;
        assert!(report.txs_duplicate >= txs.len() as u64, "{name}");
        assert_same(
            &want,
            &dump(&shuffled, &[]).await,
            &format!("{name}: shuffled and doubled"),
        );
        for p in [in_order, gap, shuffled] {
            p.close().await;
        }
    }
}

/// The cluster as the finality tracker sees it in the rollback test: everything up to
/// `finalized` is finalized in its slot (`slots`) except `abandoned`, which no node's status
/// knows and whose slots' finalized blocks do not carry; `refetch` holds what another fork
/// included (served at finalized).
struct Stub {
    finalized: u64,
    abandoned: BTreeSet<String>,
    refetch: BTreeMap<String, RawTx>,
    slots: BTreeMap<String, u64>,
}

impl StatusSource for Stub {
    async fn finalized_slot(&self) -> anyhow::Result<u64> {
        Ok(self.finalized)
    }

    async fn statuses(&self, sigs: &[Signature]) -> anyhow::Result<Vec<Option<TxStatus>>> {
        Ok(sigs
            .iter()
            .map(|s| {
                let s = s.to_string();
                (!self.abandoned.contains(&s)).then(|| TxStatus {
                    slot: self.slots.get(&s).copied().unwrap_or(0),
                    finalized: true,
                })
            })
            .collect())
    }

    async fn fetch_finalized(&self, sig: &Signature) -> anyhow::Result<Option<RawTx>> {
        Ok(self.refetch.get(&sig.to_string()).map(|t| {
            let mut t = t.clone();
            t.commitment = Commitment::Finalized;
            t
        }))
    }

    async fn block_check(&self, _slot: u64, sig: &Signature) -> anyhow::Result<BlockCheck> {
        Ok(if self.abandoned.contains(&sig.to_string()) {
            BlockCheck::Lacks
        } else {
            BlockCheck::Has
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rollback_at_confirmed_restores_the_database_of_the_surviving_transactions() {
    let pg = pg().await;
    let s = load(LIFECYCLE);
    let confirmed = raws(&s, Commitment::Confirmed);
    let finalized = raws(&s, Commitment::Finalized);
    let k = 8;
    let n = confirmed.len();
    assert!(n > 2 * k);
    let max_slot = confirmed.iter().map(|t| t.slot).max().expect("slots");

    // The database that only ever saw the first n − k transactions (finalized).
    let survivor = pg.fresh_db().await.expect("db");
    write_all(&survivor, &s, &finalized[..n - k], 1).await;
    let want = dump(&survivor, &[]).await;

    // §11.4 (11): everything ingested at confirmed, then the last k abandoned.
    let pool = pg.fresh_db().await.expect("db");
    write_all(&pool, &s, &confirmed, 1).await;
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ingest_txs WHERE commitment = 'confirmed'")
            .fetch_one(&pool)
            .await
            .expect("count");
    assert_eq!(pending, n as i64);
    let slots: BTreeMap<String, u64> = confirmed
        .iter()
        .map(|t| (t.signature.to_string(), t.slot))
        .collect();
    let stub = Stub {
        finalized: max_slot,
        abandoned: confirmed[n - k..]
            .iter()
            .map(|t| t.signature.to_string())
            .collect(),
        refetch: BTreeMap::new(),
        slots: slots.clone(),
    };
    let ids = replay::program_ids(&s).expect("ids");
    let mut w = Writer::open(&pool, ids, "ws_live").await.expect("writer");
    let r = finality::run_once(&mut w, &stub, 5)
        .await
        .expect("finality");
    w.close().await.expect("close");
    assert_eq!(r.promoted, (n - k) as u64);
    assert_eq!(r.rolled_back, k as u64);
    assert_same(&want, &dump(&pool, &[]).await, "rollback of the last k");
    let logged: i64 = sqlx::query_scalar("SELECT count(*) FROM ingest_rollbacks")
        .fetch_one(&pool)
        .await
        .expect("rollbacks");
    assert_eq!(logged, k as i64);
    let fin: i64 = sqlx::query_scalar("SELECT finalized_slot FROM ingest_state")
        .fetch_one(&pool)
        .await
        .expect("state");
    assert_eq!(fin, i64::try_from(max_slot).expect("slot"));

    // The status the node gives does not know the last k, but `getTransaction` at finalized
    // serves them in their slots (a node whose status lags): promoted in place, never rolled
    // back (M8-SEC-1), and the database equals the complete history.
    let full = pg.fresh_db().await.expect("db");
    write_all(&full, &s, &finalized, 1).await;
    let want_full = dump(&full, &[]).await;
    let pool2 = pg.fresh_db().await.expect("db");
    write_all(&pool2, &s, &confirmed, 1).await;
    let stub = Stub {
        finalized: max_slot,
        abandoned: confirmed[n - k..]
            .iter()
            .map(|t| t.signature.to_string())
            .collect(),
        refetch: confirmed[n - k..]
            .iter()
            .map(|t| (t.signature.to_string(), t.clone()))
            .collect(),
        slots: slots.clone(),
    };
    let mut w = Writer::open(&pool2, ids, "ws_live").await.expect("writer");
    let r = finality::run_once(&mut w, &stub, 256)
        .await
        .expect("finality");
    w.close().await.expect("close");
    assert_eq!((r.promoted, r.rolled_back, r.reingested), (n as u64, 0, 0));
    // The re-ingested rows say ws_live where the full history says replay: compare the rest.
    sqlx::query("UPDATE ingest_txs SET source = 'replay'")
        .execute(&pool2)
        .await
        .expect("source");
    assert_same(
        &want_full,
        &dump(&pool2, &[]).await,
        "rollback then re-ingest",
    );

    // A source's `Rollback { slot }`: every non-finalized transaction at or above the slot.
    let pool3 = pg.fresh_db().await.expect("db");
    write_all(&pool3, &s, &finalized[..n - k], 1).await;
    {
        let mut w = Writer::open(&pool3, ids, "ws_live").await.expect("writer");
        for t in &confirmed[n - k..] {
            w.write(t).await.expect("write");
        }
        let cut = confirmed[n - k].slot;
        let rb = finality::rollback_from_slot(&pool3, cut, "fork switch")
            .await
            .expect("rollback");
        assert!(rb.rolled_back >= 1);
        w.end_batch_force().await.expect("rebuild");
        w.close().await.expect("close");
        // Transactions of the cut slot that came before the abandoned ones were finalized.
        let left: i64 =
            sqlx::query_scalar("SELECT count(*) FROM ingest_txs WHERE commitment = 'confirmed'")
                .fetch_one(&pool3)
                .await
                .expect("count");
        assert_eq!(left, 0);
    }
    let check = pg.fresh_db().await.expect("db");
    write_all(&check, &s, &finalized[..n - k], 1).await;
    assert_same(
        &dump(&check, PROJECTIONS).await,
        &dump(&pool3, PROJECTIONS).await,
        "Rollback { slot }",
    );
    for p in [survivor, pool, full, pool2, pool3, check] {
        p.close().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_writer_per_database_and_the_events_notification() {
    let pg = pg().await;
    let pool = pg.fresh_db().await.expect("db");
    let s = load(GOVERNANCE);
    let ids = replay::program_ids(&s).expect("ids");
    let mut listener = sqlx::postgres::PgListener::connect_with(&pool)
        .await
        .expect("listener");
    listener
        .listen(shortwin_indexer::db::NOTIFY_EVENTS)
        .await
        .expect("listen");
    let mut w = Writer::open(&pool, ids, "replay")
        .await
        .expect("first writer");
    assert!(
        Writer::open(&pool, ids, "replay").await.is_err(),
        "a second writer must not start"
    );
    assert!(
        ingest::rebuild(&pool, None).await.is_err(),
        "no rebuild while a writer runs"
    );
    let txs = raws(&s, Commitment::Finalized);
    let mut with_events = 0;
    for t in &txs {
        if let ingest::writer::WriteOutcome::Written { events } = w.write(t).await.expect("write")
            && events > 0
        {
            with_events += 1;
        }
    }
    // One notification per committed transaction with events, carrying its max events.id
    // (delivered at commit; read after the fact, each within a bound).
    let mut last_id = None;
    for _ in 0..with_events {
        let n = tokio::time::timeout(std::time::Duration::from_secs(10), listener.recv())
            .await
            .expect("a notification within 10 s")
            .expect("a notification");
        last_id = Some(n.payload().parse::<i64>().expect("an events.id"));
    }
    w.close().await.expect("close");
    let max: i64 = sqlx::query_scalar("SELECT max(id) FROM events")
        .fetch_one(&pool)
        .await
        .expect("max");
    assert_eq!(last_id, Some(max));
    // The lock is free again.
    Writer::open(&pool, ids, "replay")
        .await
        .expect("a writer after close")
        .close()
        .await
        .expect("close");
    // The listener holds a pooled connection: release it before closing the pool.
    drop(listener);
    pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn short_win_venue_legs_match_their_own_events() {
    // §7.2: an open's sale = the trade's net SOL; an exit's buyback = the buy's SOL in.
    let pg = pg().await;
    let mut venues: BTreeSet<String> = BTreeSet::new();
    for (name, s) in all_sessions() {
        let pool = pg.fresh_db().await.expect("db");
        ingest::ingest_session(&pool, &s, 0..s.txs.len())
            .await
            .expect("ingest");
        let rows: Vec<(String, String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT t.venue, t.shortwin_invocation, t.side, t.sol_amount::text, \
               CASE e.kind WHEN 'Open' THEN e.data->>'s' ELSE e.data->>'buyback_cost' END \
             FROM venue_trades t JOIN events e ON e.signature = t.signature \
               AND e.ix_index = t.ix_index AND e.kind IN ('Open', 'Close', 'Liquidate', 'Expire') \
             WHERE t.is_shortwin",
        )
        .fetch_all(&pool)
        .await
        .expect("legs");
        for (venue, inv, side, sol, leg) in rows {
            let want_side = if inv == "open_short" { "sell" } else { "buy" };
            assert_eq!(side, want_side, "{name}: {venue} {inv}");
            assert_eq!(
                Some(sol.clone()),
                leg,
                "{name}: {venue} {inv} SOL {sol} vs the event's"
            );
            venues.insert(venue);
        }
        pool.close().await;
    }
    let all: BTreeSet<String> = ["pump_curve", "pump_swap", "raydium_cpmm", "raydium_v4"]
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(
        venues, all,
        "every venue's leg is checked against its short.win event"
    );
}
