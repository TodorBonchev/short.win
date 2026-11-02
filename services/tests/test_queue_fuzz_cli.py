"""Bounded CLI/durable-completion and intentional-abort checks for queue fuzz."""
import hashlib
import importlib.util
from collections import Counter
import json
from pathlib import Path
import subprocess
import sys
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

WORKER=Path(__file__).resolve().parents[1]/"queue_lifecycle_fuzz.py"
class QueueFuzzCliTests(unittest.TestCase):
    def invoke(self,out,*args):
        return subprocess.run([sys.executable,str(WORKER),'--seed','103410000','--out',str(out),*args],capture_output=True,text=True,timeout=30)
    def verify_hashes(self,out):
        summary=json.loads((out/'summary.json').read_text());completion=json.loads((out/'completion.json').read_text())
        self.assertEqual(completion['summary_sha256'],hashlib.sha256((out/'summary.json').read_bytes()).hexdigest())
        self.assertEqual(completion['log_sha256'],hashlib.sha256((out/'run.log').read_bytes()).hexdigest())
        return summary,completion
    def test_bounded_cases_have_real_counts_and_exact_durable_hashes(self):
        with tempfile.TemporaryDirectory() as folder:
            out=Path(folder)/'run';result=self.invoke(out,'--cases','2')
            self.assertEqual(result.returncode,0,result.stderr);s,c=self.verify_hashes(out)
            self.assertTrue(s['finished_cleanly']);self.assertEqual(s['completed_cases'],2);self.assertEqual(c['exit_code'],0)
            for key in ['failures','anomalies','setup_failures','invariant_failures']:self.assertEqual(s[key],0)
            for key in ['protected_active','protected_pending','protected_reference','protected_intent','retirements','cas_rejections','restarts','cold_history_checks']:
                self.assertGreater(s['coverage'][key],0,key)
            self.assertFalse((out/'working.sqlite3').exists())
            old=(out/'run.log').read_bytes();self.assertNotEqual(self.invoke(out,'--cases','1').returncode,0);self.assertEqual((out/'run.log').read_bytes(),old)
    def test_duration_finishes_inflight_case_without_borrowing_time(self):
        with tempfile.TemporaryDirectory() as folder:
            out=Path(folder)/'run';result=self.invoke(out,'--duration-seconds','1')
            self.assertEqual(result.returncode,0,result.stderr);s,c=self.verify_hashes(out)
            self.assertGreaterEqual(s['wall_seconds'],1);self.assertGreaterEqual(c['elapsed_seconds'],1)
            self.assertGreater(s['completed_cases'],0);self.assertTrue(s['finished_cleanly'])
    def test_failure_preserves_raw_sqlite_bundle_before_connection_close(self):
        spec=importlib.util.spec_from_file_location('queue_fuzz_preservation_test',WORKER)
        worker=importlib.util.module_from_spec(spec);sys.modules[spec.name]=worker;spec.loader.exec_module(worker)
        with tempfile.TemporaryDirectory() as folder:
            out=Path(folder)
            with patch.object(worker,'protected',side_effect=AssertionError('injected lifecycle fault')):
                with self.assertRaisesRegex(AssertionError,'injected lifecycle fault'):
                    worker.run_case(103410000,out,Counter())
            crash=out/'failure-sqlite';hashes=json.loads((crash/'SHA256SUMS.json').read_text())
            self.assertEqual(set(hashes),{'working.sqlite3','working.sqlite3-wal','working.sqlite3-shm'})
            for name,digest in hashes.items():
                self.assertEqual(hashlib.sha256((crash/name).read_bytes()).hexdigest(),digest,name)
            self.assertTrue((out/'working.sqlite3').exists())
            # Open a disposable recovery copy so examining the retained bundle
            # cannot checkpoint or rewrite the preserved raw WAL/SHM evidence.
            recovery=out/'recovery';recovery.mkdir()
            for name in hashes:(recovery/name).write_bytes((crash/name).read_bytes())
            with sqlite3.connect(recovery/'working.sqlite3') as db:
                self.assertEqual(db.execute('PRAGMA integrity_check').fetchone()[0],'ok')
                self.assertEqual(db.execute('SELECT count(*) FROM observations').fetchone()[0],25)
                self.assertEqual(db.execute("SELECT value FROM metadata WHERE key='financial_allowance_sentinel'").fetchone()[0],'4:1000000')
            for name,digest in hashes.items():
                self.assertEqual(hashlib.sha256((crash/name).read_bytes()).hexdigest(),digest,name)
    def test_supervisor_stop_is_partial_nonzero_with_readable_evidence(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);stop=root/'stop';stop.write_text('stop');out=root/'run'
            result=self.invoke(out,'--duration-seconds','60','--stop-file',str(stop))
            self.assertEqual(result.returncode,3,result.stderr);s,c=self.verify_hashes(out)
            self.assertFalse(s['finished_cleanly']);self.assertTrue(s['stopped']);self.assertEqual(c['exit_code'],3)
            self.assertTrue((out/'stopped.json').exists());self.assertEqual(s['completed_cases'],0)

if __name__=='__main__':unittest.main()
