"""Negative release-packet guards; no RPC, signing, ELF execution or deployment."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("mainnet_release", ROOT / "scripts/prepare-mainnet-release.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class MainnetReleaseTests(unittest.TestCase):
    def setUp(self):
        self.spec = json.loads((ROOT / release.SPEC).read_text())
        self.inputs = {key: json.loads((ROOT / path).read_text()) for key, path in release.INPUTS.items()}

    def test_current_non_executable_spec_matches_primary_inputs(self):
        release.validate_spec(self.spec, self.inputs)
        self.assertFalse(self.spec["execute"])
        self.assertFalse(self.spec["readyForActivation"])
        self.assertTrue(self.spec["initializationIntent"]["opensPaused"])

    def test_execution_and_activation_flags_cannot_be_enabled(self):
        for key in ["execute", "readyForActivation"]:
            changed = copy.deepcopy(self.spec)
            changed[key] = True
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "cannot enable"):
                release.validate_spec(changed, self.inputs)

    def test_other_reviewed_rebuild_cannot_replace_duration_artifact(self):
        other = "b12fefdcf9c73fa8c708847bba162e9aa54e77d46cc8e1a340b12d95c9b2e0a9"
        changed = copy.deepcopy(self.spec)
        changed["programSha256"] = other
        with self.assertRaisesRegex(ValueError, "exact primary"):
            release.validate_spec(changed, self.inputs)
        self.inputs["reviewManifest"]["ownProgram"]["elfSha256"] = other
        with self.assertRaisesRegex(ValueError, "exact primary"):
            release.validate_spec(self.spec, self.inputs)

    def test_mismatched_router_and_wrong_mainnet_genesis_are_rejected(self):
        self.inputs["publicRoles"]["genesisHash"] = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
        with self.assertRaisesRegex(ValueError, "role genesis"):
            release.validate_spec(self.spec, self.inputs)
        self.inputs["publicRoles"]["genesisHash"] = None
        self.inputs["publicRoles"]["jupiterReview"]["deploySlot"] += 1
        with self.assertRaisesRegex(ValueError, "role/router"):
            release.validate_spec(self.spec, self.inputs)

    def test_activation_cannot_hide_or_mark_a_gate_complete(self):
        for mutation in ["rename", "complete", "remove"]:
            changed = copy.deepcopy(self.spec)
            if mutation == "rename":
                changed["activationGates"][0]["id"] = "unrelated-check"
            elif mutation == "complete":
                changed["activationGates"][0]["status"] = "passed"
            else:
                changed["activationGates"].pop()
            with self.subTest(mutation=mutation), self.assertRaisesRegex(ValueError, "gate"):
                release.validate_spec(changed, self.inputs)

    def test_increased_caps_or_added_service_slots_require_new_release_review(self):
        changed = copy.deepcopy(self.spec)
        changed["releaseBounds"]["maxMarketBookedNotionalLamports"] *= 2
        self.inputs["policy"]["marketNotionalCapLamports"] *= 2
        with self.assertRaisesRegex(ValueError, "new explicit release review"):
            release.validate_spec(changed, self.inputs)
        self.inputs["policy"]["marketNotionalCapLamports"] //= 2
        changed = copy.deepcopy(self.spec)
        changed["releaseBounds"]["exactQuantityHistorySlots"] = 2
        self.inputs["service"]["max_jobs"] = 2
        with self.assertRaisesRegex(ValueError, "new explicit release review"):
            release.validate_spec(changed, self.inputs)

    def test_public_role_export_excludes_keyfile_paths_and_unknown_fields(self):
        roles = copy.deepcopy(self.inputs["publicRoles"])
        roles["keyfiles"] = {"reporter": "/dummy/private/reporter.json"}
        roles["unexpectedSecret"] = "dummy-marker"
        public = release.public_roles(roles)
        self.assertNotIn("keyfiles", public)
        self.assertNotIn("unexpectedSecret", public)
        self.assertNotIn("dummy-marker", json.dumps(public))

    def test_wrong_program_bytes_are_rejected_without_an_output_packet(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "unqualified.so"
            path.write_bytes(b"\x7fELF" + b"\0" * 128)
            with self.assertRaisesRegex(ValueError, "exact primary qualified ELF"):
                release.read_program(path)
            self.assertEqual([file.name for file in Path(folder).iterdir()], ["unqualified.so"])

    def test_program_input_symlink_is_not_followed(self):
        with tempfile.TemporaryDirectory() as folder:
            file = Path(folder) / "unqualified.so"
            file.write_bytes(b"\x7fELF")
            link = Path(folder) / "link.so"
            link.symlink_to(file)
            with self.assertRaises(OSError):
                release.read_program(link)


if __name__ == "__main__":
    unittest.main()
