"""Real SQLite queue retirement under trusted finalized snapshot contracts."""
import copy
import importlib.util
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path
from dataclasses import asdict

SOURCE = Path(__file__).resolve().parents[1] / "src/shortwin_price_service.py"
SPEC = importlib.util.spec_from_file_location("queue_lifecycle_service", SOURCE)
service = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = service
SPEC.loader.exec_module(service)
MARKET = "kXYTnbz7MJ3g1d1cKX5wfZLy8KmN7Cctx4oePy5ndB2"
POLICY = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"

def job(q=100, active=False): return service.Job(MARKET, POLICY, MINT, q, active)
def settings(): return service.Settings(60, 2, 5, 30, 100, 50, 1000, 10000, 8, 1, 1, 1)
def reference(item=None, ts=1000, slot=2000):
    return {"job": asdict(item or job()), "source_slot":str(slot), "source_ts":ts,"max_age_seconds":30,"max_age_slots":100}

class QueueLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.path=Path(self.temp.name)/"queue.sqlite3"
        self.history=service.History(self.path,settings());self.history.enqueue(job(),600)
    def tearDown(self): self.history.close();self.temp.cleanup()
    def envelope(self, ts=1000, slot=2000, **updates):
        result={"schema":1,"complete":True,"commitment":"finalized","publisher_lock":True,
                "context_slot":str(slot),"clock_slot":str(slot),"clock_ts":ts,
                "expected_queue_revision":str(self.history.revision()),"active_jobs":[],"references":[],
                "pending_publications":[],"fulfilled_requests":[]}
        result.update(updates);return result
    def apply(self, **updates):
        payload=self.envelope(**updates);return self.history.reconcile_queue(payload,payload["clock_ts"])
    def test_unused_warm_timeout_releases_one_slot_without_touching_financial_metadata(self):
        ttl=4*60+30
        self.history.db.execute("INSERT INTO metadata VALUES('new-reference-accounts','4')");self.history.db.commit()
        self.assertEqual(self.apply(ts=600+ttl)["retired"],[])
        result=self.apply(ts=601+ttl,slot=2001)
        self.assertEqual(len(result["retired"]),1);self.assertEqual(self.history.jobs(),[])
        self.assertTrue(self.history.enqueue(job(101),932))
        self.assertEqual(self.history.db.execute("SELECT value FROM metadata WHERE key='new-reference-accounts'").fetchone()[0],"4")
    def test_both_reference_clocks_must_expire_before_retiring_resolved_debt(self):
        self.history.sync_loans([job()],1000)
        self.assertFalse(self.apply(ts=1031,slot=2050,references=[reference()])["retired"])
        self.assertFalse(self.history.jobs()[0].active_loan)
        self.assertEqual(len(self.apply(ts=1032,slot=2101,references=[reference()])["retired"]),1)
    def test_exact_reference_freshness_boundaries_are_retained(self):
        self.assertFalse(self.apply(ts=1030,slot=2100,references=[reference()])["retired"])
        self.assertEqual(len(self.apply(ts=1031,slot=2101,references=[reference()])["retired"]),1)
    def test_slot_expiry_alone_is_not_enough_and_active_debt_always_wins(self):
        self.assertFalse(self.apply(ts=1020,slot=2101,references=[reference()])["retired"])
        self.assertFalse(self.apply(ts=1031,slot=2102,active_jobs=[asdict(job())],references=[reference()])["retired"])
        self.assertTrue(self.history.jobs()[0].active_loan)
    def test_pending_publication_survives_restart_until_explicitly_reconciled(self):
        self.assertFalse(self.apply(pending_publications=[asdict(job())])["retired"])
        self.history.close();self.history=service.History(self.path,settings())
        self.assertFalse(self.apply(ts=1100,slot=2200,pending_publications=[asdict(job())])["retired"])
        self.assertEqual(len(self.apply(ts=1101,slot=2201)["retired"]),1)
    def test_new_generation_cannot_reuse_old_private_history_or_old_fulfillment(self):
        for ts in range(840,1001,5): self.history.record(job(),ts,ts+1000,100,105,{"raw":ts})
        before=self.history.db.execute("SELECT count(*) FROM observations").fetchone()[0]
        self.assertEqual(self.history.reference(job(),0,2000,1000)["status"],"Finite")
        self.apply(ts=1000,slot=2000)
        self.assertEqual(self.history.reference(job(),0,2000,1000)["status"],"Unknown")
        self.assertTrue(self.history.enqueue(job(),1001));self.assertEqual(self.history.generation(job()),1)
        self.history.record(job(),1001,2001,100,105)
        self.assertEqual(self.history.reference(job(),0,2001,1001)["status"],"Unknown")
        self.assertEqual(self.history.db.execute("SELECT count(*) FROM observations WHERE generation=0").fetchone()[0],before)
        old_ack={"job":asdict(job()),"generation":0,"requested_at":999999}
        self.assertFalse(self.apply(ts=1002,slot=2002,fulfilled_requests=[old_ack])["retired"])
        current_ack=dict(old_ack,generation=1,requested_at=1001)
        self.assertEqual(len(self.apply(ts=1003,slot=2003,fulfilled_requests=[current_ack])["retired"]),1)
    def test_stale_tick_cannot_write_into_readmitted_cohort(self):
        self.apply();self.history.enqueue(job(),1001)
        with self.assertRaisesRegex(ValueError,"generation changed"):
            self.history.record(job(),1002,2002,100,105,expected_generation=0)
        self.assertEqual(self.history.db.execute("SELECT count(*) FROM observations").fetchone()[0],0)
    def test_revision_cas_rejects_enqueue_or_observation_races_atomically(self):
        stale=self.envelope()
        other=service.History(self.path,settings())
        try: other.enqueue(job(),1000)
        finally: other.close()
        before=self.history.queue_state()
        with self.assertRaisesRegex(ValueError,"revision changed"):
            self.history.reconcile_queue(stale,1000)
        self.assertEqual(self.history.queue_state(),before)
        stale=self.envelope();self.history.record(job(),1000,2000,100,105)
        with self.assertRaisesRegex(ValueError,"revision changed"):
            self.history.reconcile_queue(stale,1000)
    def test_active_capacity_failure_rolls_back_demotions_and_retirements(self):
        self.history.sync_loans([job()],1000)
        before=self.history.queue_state()
        with self.assertRaisesRegex(ValueError,"cannot fit"):
            self.apply(ts=1001,slot=2001,active_jobs=[asdict(job(101))],references=[reference()])
        self.assertEqual(self.history.queue_state(),before)
        self.assertFalse(self.history.enqueue(job(101,True),1001))
    def test_incomplete_unlocked_future_or_stale_snapshot_never_mutates_queue(self):
        changes=[{"complete":False},{"publisher_lock":False},{"commitment":"confirmed"},
                 {"context_slot":"0"},{"clock_slot":"1999"},{"clock_slot":"2001"},
                 {"references":[reference(ts=1001)]},{"references":[reference(slot=2001)]},
                 {"active_jobs":[asdict(job()),asdict(job(101))]}]
        for change in changes:
            with self.subTest(change=change):
                before=self.history.queue_state()
                with self.assertRaises(ValueError): self.apply(**change)
                self.assertEqual(self.history.queue_state(),before)
        missing=self.envelope();missing.pop("references")
        with self.assertRaises(ValueError): self.history.reconcile_queue(missing,1000)
        with self.assertRaisesRegex(ValueError,"stale"):
            self.history.reconcile_queue(self.envelope(),1031)
    def test_snapshot_order_same_slot_conflict_and_exact_replay_are_durable(self):
        payload=self.envelope(active_jobs=[asdict(job())]);result=self.history.reconcile_queue(payload,1000)
        self.history.close();self.history=service.History(self.path,settings())
        replay=self.history.reconcile_queue(payload,1000)
        self.assertTrue(replay["idempotent"]);self.assertEqual(replay["revision"],result["revision"])
        for change in [{"context_slot":"1999"},{"clock_ts":999},{"pending_publications":[asdict(job())]}]:
            altered=copy.deepcopy(payload);altered.update(change);altered["expected_queue_revision"]=str(self.history.revision())
            with self.assertRaises(ValueError):self.history.reconcile_queue(altered,altered["clock_ts"])
    def test_cached_snapshot_retry_rejects_enqueue_or_record_after_prior_application(self):
        payload=self.envelope(active_jobs=[asdict(job())])
        self.history.reconcile_queue(payload,1000)
        self.history.enqueue(job(),1001)
        before=self.history.queue_state()
        with self.assertRaisesRegex(ValueError,"revision changed since prior"):
            self.history.reconcile_queue(payload,1001)
        self.assertEqual(self.history.queue_state(),before)
        payload=self.envelope(ts=1002,slot=2002,active_jobs=[asdict(job())])
        self.history.reconcile_queue(payload,1002)
        self.history.record(job(),1003,2003,100,105)
        before=self.history.queue_state()
        with self.assertRaisesRegex(ValueError,"revision changed since prior"):
            self.history.reconcile_queue(payload,1003)
        self.assertEqual(self.history.queue_state(),before)
    def test_full_active_queue_and_pending_intent_cannot_be_replaced(self):
        self.history.sync_loans([job()],1000)
        self.assertFalse(self.apply(active_jobs=[asdict(job())])["retired"])
        self.assertFalse(self.history.enqueue(job(101),1000))
        self.apply(ts=1100,slot=2200)
        self.assertTrue(self.history.enqueue(job(101),1100))
        self.assertFalse(self.apply(ts=1101,slot=2201)["retired"])
    def test_old_model_database_is_preserved_without_schema_migration(self):
        path=Path(self.temp.name)/"old.sqlite3";db=sqlite3.connect(path)
        db.execute("CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
        db.execute("INSERT INTO metadata VALUES('model_hash','old-v2-model')")
        db.execute("CREATE TABLE original_evidence(value TEXT)");db.execute("INSERT INTO original_evidence VALUES('preserve-me')");db.commit();db.close()
        with self.assertRaisesRegex(ValueError,"preserve the old database"):
            service.History(path,settings())
        db=sqlite3.connect(path)
        try:
            self.assertEqual({r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")},{"metadata","original_evidence"})
            self.assertEqual(db.execute("SELECT value FROM original_evidence").fetchone()[0],"preserve-me")
        finally:db.close()

if __name__=='__main__':unittest.main()
