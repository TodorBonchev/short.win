"""Persistent local identity binding for history and operator financial storage."""
from contextlib import redirect_stdout
from dataclasses import asdict
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import uuid

SOURCE=Path(__file__).resolve().parents[1]/"src/shortwin_price_service.py"
SPEC=importlib.util.spec_from_file_location("operator_binding_service",SOURCE)
service=importlib.util.module_from_spec(SPEC);sys.modules[SPEC.name]=service;SPEC.loader.exec_module(service)
PROGRAM="BCPiXCqAQpDPbp7TcPVpbUeZmLk9HBJ2XxLBmCsfmFSZ"
OTHER_PROGRAM="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"

def settings():return service.Settings(60,2,5,30,100,50,1000,10000,8,1,1,1)
def binding():return {"schema":1,"program_id":PROGRAM,"operator_store_id":"161c54b1-353d-4de3-8514-eef860fc4347","operator_database":"/srv/shortwin/operator.sqlite3"}

class OperatorBindingTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name)
        self.path=self.root/'history.sqlite3';self.h=service.History(self.path,settings())
    def tearDown(self):self.h.close();self.temp.cleanup()
    def test_fresh_history_has_stable_distinct_uuid_and_exposes_unbound_state(self):
        state=self.h.queue_state();identity=state['history_instance_id']
        self.assertEqual(str(uuid.UUID(identity)),identity);self.assertEqual(uuid.UUID(identity).version,4)
        self.assertIsNone(state['operator_binding'])
        self.h.close();self.h=service.History(self.path,settings())
        self.assertEqual(self.h.queue_state(),state)
        other=service.History(self.root/'other.sqlite3',settings())
        try:self.assertNotEqual(other.history_instance_id(),identity)
        finally:other.close()
    def test_exact_duplicate_binding_is_durable_without_resetting_queue_or_allowances(self):
        self.h.db.execute("INSERT INTO metadata VALUES('financial_allowance_sentinel','4:1000000')");self.h.db.commit()
        revision=self.h.revision();first=self.h.bind_operator(binding())
        self.assertEqual(first['operator_binding'],binding());self.assertEqual(self.h.bind_operator(binding()),first)
        self.h.close();self.h=service.History(self.path,settings())
        self.assertEqual(self.h.bind_operator(binding()),first)
        self.assertEqual(self.h.queue_state()['operator_binding'],binding());self.assertEqual(self.h.revision(),revision)
        self.assertEqual(self.h.db.execute("SELECT value FROM metadata WHERE key='financial_allowance_sentinel'").fetchone()[0],'4:1000000')
    def test_changed_store_path_or_program_cannot_overwrite_binding_even_via_other_connection(self):
        self.h.bind_operator(binding());before=self.h.queue_state()
        changes=[{'operator_store_id':str(uuid.uuid4())},{'operator_database':'/srv/shortwin/recreated.sqlite3'},{'program_id':OTHER_PROGRAM}]
        other=service.History(self.path,settings())
        try:
            for change in changes:
                with self.subTest(change=change):
                    with self.assertRaisesRegex(ValueError,'persistent history binding'):
                        other.bind_operator(dict(binding(),**change))
                    self.assertEqual(self.h.queue_state(),before)
        finally:other.close()
    def test_replaced_history_cannot_reproduce_external_identity_anchor(self):
        old=self.h.bind_operator(binding());self.h.close()
        # Move only the closed history file, as an accidental replacement might;
        # the operator's separate persistent lock anchor still remembers old UUID.
        self.path.rename(self.root/'retained-history.sqlite3')
        self.h=service.History(self.path,settings());new=self.h.bind_operator(binding())
        self.assertEqual(new['operator_binding'],old['operator_binding'])
        self.assertNotEqual(new['history_instance_id'],old['history_instance_id'])
    def test_malformed_binding_and_corrupted_identity_are_not_repaired_or_overwritten(self):
        bad=[dict(binding(),schema=True),dict(binding(),schema=2),dict(binding(),extra=True),
             dict(binding(),program_id='not-a-public-key'),dict(binding(),operator_store_id='0'*36),
             dict(binding(),operator_store_id='00000000-0000-0000-0000-000000000000'),
             dict(binding(),operator_database='operator.sqlite3'),dict(binding(),operator_database='/srv/../operator.sqlite3'),
             dict(binding(),operator_database='/srv/operator\0.sqlite3')]
        state=self.h.queue_state()
        for payload in bad:
            with self.subTest(payload=payload):
                with self.assertRaises(ValueError):self.h.bind_operator(payload)
                self.assertEqual(self.h.queue_state(),state)
        self.h.db.execute("UPDATE metadata SET value='corrupt' WHERE key='history_instance_id'");self.h.db.commit()
        with self.assertRaisesRegex(ValueError,'canonical non-nil UUID'):self.h.bind_operator(binding())
        self.assertEqual(self.h.db.execute("SELECT value FROM metadata WHERE key='history_instance_id'").fetchone()[0],'corrupt')
        self.assertIsNone(self.h.operator_binding())
    def test_missing_uuid_in_existing_modeled_database_fails_without_recreation(self):
        import sqlite3
        self.h.bind_operator(binding());self.h.close()
        with sqlite3.connect(self.path) as db:
            db.execute("DELETE FROM metadata WHERE key='history_instance_id'")
            before=db.execute("SELECT key,value FROM metadata ORDER BY key").fetchall()
        with self.assertRaisesRegex(ValueError,'existing history instance identity is missing'):
            service.History(self.path,settings())
        with sqlite3.connect(self.path) as db:
            self.assertEqual(db.execute("SELECT key,value FROM metadata ORDER BY key").fetchall(),before)
            self.assertIsNone(db.execute("SELECT value FROM metadata WHERE key='history_instance_id'").fetchone())
    def test_trusted_cli_accepts_stdin_and_queue_state_returns_identical_binding(self):
        config=self.root/'settings.json';config.write_text(json.dumps(asdict(settings())))
        args=['--settings',str(config),'--database',str(self.path)]
        out=io.StringIO()
        with patch('sys.stdin',io.StringIO(json.dumps(binding()))),redirect_stdout(out):
            self.assertEqual(service.main(args+['bind-operator','-']),0)
        first=json.loads(out.getvalue());out=io.StringIO()
        with redirect_stdout(out):self.assertEqual(service.main(args+['queue-state']),0)
        state=json.loads(out.getvalue())
        self.assertEqual(state['history_instance_id'],first['history_instance_id'])
        self.assertEqual(state['operator_binding'],first['operator_binding'])

if __name__=='__main__':unittest.main()
