"""Offline guards for non-executable production setup proposals."""
import copy
import base64
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("production_proposal", HERE / "verify-production-proposal.py")
proposal = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = proposal
spec.loader.exec_module(proposal)


def unassigned_roles():
    """Keep refusal fixtures independent of the current public role assignments."""
    roles = json.loads((HERE / "production-roles.template.json").read_text())
    roles["assignmentStatus"] = "unassigned"
    roles["genesisHash"] = None
    roles["rolePublicKeys"] = {name: None for name in roles["rolePublicKeys"]}
    return roles


class ProductionProposalTests(unittest.TestCase):
    def setUp(self):
        self.roles = unassigned_roles()

    def test_unassigned_proposal_cannot_pass_assignment_gate(self):
        result = proposal.verify_roles(self.roles)
        self.assertFalse(result["complete_public_identity_assignment"])
        self.assertFalse(result["execution_enabled"])
        with self.assertRaisesRegex(ValueError, "unassigned"):
            proposal.verify_roles(self.roles, require_assigned=True)

    def test_generated_public_assignment_passes_identity_gate_without_execution(self):
        roles = json.loads((HERE / "production-roles.template.json").read_text())
        result = proposal.verify_roles(roles, require_assigned=True)
        self.assertEqual(roles["assignmentStatus"], "assigned-for-review")
        self.assertEqual(roles["genesisHash"], proposal.MAINNET_GENESIS)
        self.assertEqual(len(set(roles["rolePublicKeys"].values())), 5)
        self.assertTrue(result["complete_public_identity_assignment"])
        self.assertEqual(result["missing_public_identity_fields"], [])
        self.assertFalse(result["execution_enabled"])
        self.assertFalse(roles["execute"])

    def test_execute_cannot_be_enabled_in_review_manifest(self):
        self.roles["execute"] = True
        with self.assertRaisesRegex(ValueError, "cannot enable execution"):
            proposal.verify_roles(self.roles)

    def test_role_manifest_rejects_other_chain_genesis_even_when_key_shaped(self):
        self.roles["genesisHash"] = proposal.service.JUPITER_PROGRAM
        with self.assertRaisesRegex(ValueError, "canonical mainnet-beta"):
            proposal.verify_roles(self.roles)
        self.roles["genesisHash"] = proposal.MAINNET_GENESIS
        self.assertIn("reporter", proposal.verify_roles(self.roles)["missing_public_identity_fields"])

    def test_local_placeholder_program_is_not_production_assignment(self):
        self.roles["programId"] = "kXYTnbz7MJ3g1d1cKX5wfZLy8KmN7Cctx4oePy5ndB2"
        with self.assertRaisesRegex(ValueError, "local placeholder"):
            proposal.verify_roles(self.roles)

    def test_zero_or_malformed_public_key_is_rejected(self):
        for key in ["11111111111111111111111111111111", "not a public key", "<choose reporter>"]:
            with self.subTest(key=key):
                roles = copy.deepcopy(self.roles)
                roles["rolePublicKeys"]["reporter"] = key
                with self.assertRaises(ValueError):
                    proposal.verify_roles(roles)

    def test_reporter_cannot_be_governance(self):
        # A public canonical program address is only synthetic validation input.
        key = proposal.service.JUPITER_PROGRAM
        self.roles["rolePublicKeys"].update(reporter=key, governance=key)
        with self.assertRaisesRegex(ValueError, "reporter must differ"):
            proposal.verify_roles(self.roles)

    def test_both_profiles_match_actual_service_and_reject_drift(self):
        for prefix in ["production", "keyless"]:
            with self.subTest(prefix=prefix):
                policy = json.loads((HERE / f"{prefix}-policy.proposed.json").read_text())
                settings = proposal.service.Settings(**json.loads((HERE / f"{prefix}-service.proposed.json").read_text()))
                proposal.verify_terms(policy)
                proposal.service.verify_policy(settings, policy)
                policy["pricePolicyHash"] = "01" * 32
                with self.assertRaisesRegex(ValueError, "pricePolicyHash differs"):
                    proposal.service.verify_policy(settings, policy)


class ReviewedJupiterTests(unittest.TestCase):
    """Small synthetic receipts exercise integrity checks, never RPC or SBF."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.roles = unassigned_roles()
        self.roles["jupiterReview"].update(deploySlot=42, programDataLength=52, elfSha256="pending")
        self.pin = self.roles["jupiterReview"]
        alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
        number = 0
        for character in self.pin["programData"]:
            number = number * 58 + alphabet.index(character)
        program = (2).to_bytes(4, "little") + number.to_bytes(32, "big")
        elf = b"\x7fELF\x01\x02\x03"
        pd = (3).to_bytes(4, "little") + (42).to_bytes(8, "little") + bytes(33) + elf
        self.pin["elfSha256"] = self.sha(elf)
        folder = "tests/fixtures/review"
        deployment = {"schema": 1, "network": "mainnet-beta", "live_transactions_sent": 0,
                      "program_id": self.pin["programId"], "programdata_address": self.pin["programData"],
                      "deploy_slot": 42, "programdata_len": len(pd), "elf_sha256": self.sha(elf),
                      "elf_trimmed_length": len(elf), "padded_elf_sha256": self.sha(elf),
                      "observed_at": "2026-11-03T00:00:00Z",
                      "clock": {"address": proposal.CLOCK, "slot": 43, "unix_timestamp": 1793664000},
                      "genesis_hash": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d", "requests": []}
        values = []
        for name, address, data, owner, executable in (
                ("program", self.pin["programId"], program, "BPFLoaderUpgradeab1e11111111111111111111111", True),
                ("programdata", self.pin["programData"], pd, "BPFLoaderUpgradeab1e11111111111111111111111", False),
                ("clock_account", "SysvarC1ock11111111111111111111111111111111", (43).to_bytes(8, "little") + bytes(24) + (1793664000).to_bytes(8, "little"), "Sysvar1111111111111111111111111111111111111", False)):
            entry = self.put(folder + "/" + name + ".bin", data)
            deployment[name] = {"file": name + ".bin", "sha256": entry["sha256"], "address": address,
                                "owner": owner, "executable": executable, "lamports": 1, "rent_epoch": 0, "context_slot": 43}
            values.append({"data": [base64.b64encode(data).decode(), "base64"], "owner": owner,
                           "executable": executable, "lamports": 1, "rentEpoch": 0})
        self.put(folder + "/jupiter.so", elf)
        for name, request, response in (
                ("genesis", {"method": "getGenesisHash", "params": []}, {"result": deployment["genesis_hash"]}),
                ("accounts", {"method": "getMultipleAccounts", "params": [[self.pin["programId"], self.pin["programData"], deployment["clock_account"]["address"]], {"commitment": "finalized", "encoding": "base64"}]}, {"result": {"context": {"slot": 43}, "value": values}})):
            raw = json.dumps(response).encode()
            stored = self.put(folder + "/" + name + ".json.gz", gzip.compress(raw, mtime=0))
            deployment["requests"].append({"file": name + ".json.gz", "sha256": self.sha(raw),
                                            "stored_sha256": stored["sha256"], "status": 200, "request": request})
        matrix = {"schema": 1, "program_id": self.roles["programId"], "pending": [],
                  "expected_pass": [{"name": "positive-" + str(i)} for i in range(6)],
                  "expected_fail": [{"name": "negative-" + str(i)} for i in range(2)]}
        matrix_entry = self.put_json("tests/runtime/matrix.json", matrix)
        deployment_entry = self.put_json(folder + "/deployment.json", deployment)
        own_hash = "01" * 32
        suite = {"schema": 1, "status": "passed", "live_transactions_sent": 0, "overlay_historical": False,
                 "review_manifest_sha256": deployment_entry["sha256"],
                 "reviewed_deployment": {key: deployment[key] for key in ("deploy_slot", "programdata_len", "elf_sha256")},
                 "matrix_sha256": matrix_entry["sha256"], "own_programs": [{"sha256": own_hash}], "cases": []}
        for mode in ("expected_pass", "expected_fail"):
            for case in matrix[mode]:
                positive = mode == "expected_pass"
                suite["cases"].append({"name": case["name"], "expected": mode, "own_program_sha256": own_hash,
                                       "verified": True, "actual": "passed" if positive else "failed", "returncode": 0 if positive else 1})
        suite["cases"].append({"name": "predecessor-pin-refusal", "expected": "predecessor_pin_refusal",
                               "own_program_sha256": own_hash, "verified": True, "actual": "failed", "returncode": 1})
        qualification = self.put_json(folder + "/qualification/summary.json", suite)
        qualification.update(matrixFile=matrix_entry["file"], matrixSha256=matrix_entry["sha256"], overlayHistorical=False)
        sources = [self.put(path, b"synthetic source binding\n") for path in (
            "programs/shortwin-simple/src/route.rs", "programs/shortwin-simple/src/initialization.rs",
            "tests/runtime/examples/qualify_current_route.rs", "scripts/materialize-reviewed-route-candidate.py",
            "scripts/qualify-reviewed-route-suite.py")]
        self.review = {"schemaVersion": 1, "status": "reviewed-offline-compatibility", "execute": False,
                       "qualifiedAtUtc": "2026-11-03T01:00:00Z", "jupiterReview": self.pin,
                       "ownProgram": {"programId": self.roles["programId"], "elfSha256": own_hash},
                       "sourceFiles": sources, "deployment": deployment_entry, "qualifications": [qualification]}
        self.save_review()

    @staticmethod
    def sha(raw):
        return hashlib.sha256(raw).hexdigest()

    def put(self, filename, raw):
        path = self.root / filename
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return {"file": filename, "sha256": self.sha(raw)}

    def put_json(self, filename, value):
        return self.put(filename, (json.dumps(value, sort_keys=True) + "\n").encode())

    def save_review(self):
        self.put_json("config/jupiter-reviewed-deployment.json", self.review)

    def test_saved_compatibility_does_not_enable_execution_or_complete_roles(self):
        result = proposal.verify_reviewed_jupiter(self.roles, self.root)
        self.assertEqual(result["status"], "reviewed-offline-compatibility")
        self.assertFalse(result["execution_enabled"])
        self.assertFalse(proposal.verify_roles(self.roles)["complete_public_identity_assignment"])

    def test_changed_source_is_rejected(self):
        (self.root / self.review["sourceFiles"][0]["file"]).write_bytes(b"changed source")
        with self.assertRaisesRegex(ValueError, "evidence hash mismatch"):
            proposal.verify_reviewed_jupiter(self.roles, self.root)

    def test_changed_role_pin_is_rejected(self):
        self.roles["jupiterReview"]["deploySlot"] += 1
        with self.assertRaisesRegex(ValueError, "pin differs"):
            proposal.verify_reviewed_jupiter(self.roles, self.root)

    def test_changed_retained_programdata_is_rejected(self):
        (self.root / "tests/fixtures/review/programdata.bin").write_bytes(b"changed program bytes")
        with self.assertRaisesRegex(ValueError, "evidence hash mismatch"):
            proposal.verify_reviewed_jupiter(self.roles, self.root)

    def test_failed_summary_is_rejected_even_with_updated_file_digest(self):
        entry = self.review["qualifications"][0]
        actual = json.loads((self.root / entry["file"]).read_bytes())
        actual["status"] = "failed"
        entry["sha256"] = self.put_json(entry["file"], actual)["sha256"]
        self.save_review()
        with self.assertRaisesRegex(ValueError, "did not pass"):
            proposal.verify_reviewed_jupiter(self.roles, self.root)

    def test_missing_case_is_rejected_even_with_passed_summary(self):
        entry = self.review["qualifications"][0]
        actual = json.loads((self.root / entry["file"]).read_bytes())
        actual["cases"].pop()
        entry["sha256"] = self.put_json(entry["file"], actual)["sha256"]
        self.save_review()
        with self.assertRaisesRegex(ValueError, "case coverage"):
            proposal.verify_reviewed_jupiter(self.roles, self.root)

    def test_populated_pin_without_manifest_is_rejected(self):
        (self.root / "config/jupiter-reviewed-deployment.json").unlink()
        with self.assertRaisesRegex(ValueError, "requires retained"):
            proposal.verify_reviewed_jupiter(self.roles, self.root)


if __name__ == "__main__":
    unittest.main()
