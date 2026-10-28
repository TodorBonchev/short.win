#!/usr/bin/env python3
"""Replay the additive pinned route matrix; no network reads or transaction sends."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
WORK = ROOT / "simple"

def sha(data):
    return hashlib.sha256(data).hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--matrix", type=Path, default=WORK / "tests/runtime/current-routes.json")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    own = Path(os.environ.get("SHORTWIN_SIMPLE_SO", WORK / "target/deploy/shortwin_simple.so")).resolve()
    own_hash = sha(own.read_bytes())
    matrix = json.loads(args.matrix.read_bytes())
    report = {"schema": 1, "own_program_sha256": own_hash, "matrix_sha256": sha(args.matrix.read_bytes()),
              "scope": "additive non-atomic snapshot compatibility; no live execution, history provenance or 24h qualification",
              "live_transactions_sent": 0, "cases": [], "pending": matrix.get("pending", []), "status": "running"}
    status = 1
    try:
        subprocess.run(["cargo", "build", "-p", "shortwin-simple-runtime-tests", "--example", "qualify_current_route", "--locked"], cwd=WORK, check=True)
        metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=WORK))
        runner = Path(metadata["target_directory"]) / "debug/examples/qualify_current_route"
        report["native_runner_sha256"] = sha(runner.read_bytes())
        for mode in ["expected_pass", "expected_fail"]:
            for case in matrix[mode]:
                folder = args.output / case["name"]
                folder.mkdir()
                capture = (ROOT / case["capture"]).resolve()
                if not capture.is_relative_to(ROOT / "docs/audit"):
                    raise ValueError("capture must remain within archived audit evidence")
                subprocess.run([sys.executable, str(WORK / "scripts/materialize-current-route-candidate.py"),
                                str(capture), str(folder / "candidate"), "--own-program-sha256", own_hash], check=True)
                env = dict(os.environ, SHORTWIN_SIMPLE_SO=str(own))
                result = subprocess.run([str(runner), str(folder / "candidate/candidate.json"), str(folder / "replay")],
                                        cwd=WORK, env=env, capture_output=True)
                (folder / "stdout.log").write_bytes(result.stdout)
                (folder / "stderr.log").write_bytes(result.stderr)
                actual = json.loads((folder / "replay/summary.json").read_bytes())
                valid = actual["own_program_sha256"] == own_hash
                if mode == "expected_pass":
                    valid = valid and result.returncode == 0 and actual["status"] == "passed" and actual.get("actual_snapshot_packets_fit") is True
                    valid = valid and actual.get("post_cpi_rollback") is True and actual.get("base_token_conservation") is True
                    valid = valid and actual.get("synthetic_reference", {}).get("functional_policy_max_spread_bps") == 500
                    setup = actual.get("setup_allowance", {})
                    if case.get("requires_opening_setup_spend"):
                        valid = valid and setup.get("opening_actual_actor_cost", 0) > 0
                    if case.get("requires_zero_close_setup"):
                        valid = valid and setup.get("closing_signed_maximum") == setup.get("closing_actual_actor_cost") == 0
                    if case.get("requires_setup_budget_rejections"):
                        rejected = actual.get("setup_budget_rejections", {})
                        valid = valid and len(rejected) >= 4 and all(rejected.values())
                else:
                    log = folder / "replay" / actual["transactions"][case["transaction"]]["log_file"]
                    valid = valid and result.returncode == 1 and actual["status"] == "failed" and case["error_substring"] in actual["error"] and case["transaction_log_substring"] in log.read_text()
                report["cases"].append({"name": case["name"], "expected": mode, "actual": actual["status"],
                                        "verified": valid, "summary_sha256": sha((folder / "replay/summary.json").read_bytes()),
                                        "stdout_sha256": sha(result.stdout), "stderr_sha256": sha(result.stderr)})
                if not valid:
                    raise ValueError("route outcome differs from pinned matrix: " + case["name"])
        report["status"] = "passed"
        status = 0
    except Exception as error:
        report["status"] = "failed"
        report["error"] = str(error)
    finally:
        (args.output / "summary.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        print(json.dumps(report, indent=2, sort_keys=True))
    return status

if __name__ == "__main__":
    raise SystemExit(main())
