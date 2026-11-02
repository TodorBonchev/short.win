"""Offline guards for non-executable production setup proposals."""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("production_proposal", HERE / "verify-production-proposal.py")
proposal = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = proposal
spec.loader.exec_module(proposal)


class ProductionProposalTests(unittest.TestCase):
    def setUp(self):
        self.roles = json.loads((HERE / "production-roles.template.json").read_text())

    def test_unassigned_proposal_cannot_pass_assignment_gate(self):
        result = proposal.verify_roles(self.roles)
        self.assertFalse(result["complete_public_identity_assignment"])
        self.assertFalse(result["execution_enabled"])
        with self.assertRaisesRegex(ValueError, "unassigned"):
            proposal.verify_roles(self.roles, require_assigned=True)

    def test_execute_cannot_be_enabled_in_review_manifest(self):
        self.roles["execute"] = True
        with self.assertRaisesRegex(ValueError, "cannot enable execution"):
            proposal.verify_roles(self.roles)

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


if __name__ == "__main__":
    unittest.main()
