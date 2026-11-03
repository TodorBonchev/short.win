#!/usr/bin/env python3
"""Replay fresh or explicitly historical route captures against a reviewed Jupiter.

Consumes an already built native runner and explicit financial-program ELFs.
Per-process exit status, exact loaded hashes, logs and lifecycle invariants must
agree. No network reads, program builds or live transaction sends occur.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
JUP = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def checked_log(folder, transaction):
    relative = Path(transaction["log_file"])
    path = (folder / relative).resolve()
    if relative.is_absolute() or ".." in relative.parts or not path.is_relative_to(folder.resolve()):
        raise ValueError("runtime log escaped replay output")
    raw = path.read_bytes()
    if sha(raw) != transaction["log_sha256"]:
        raise ValueError("runtime log hash mismatch")
    return raw.decode()


def replay(runner, own, candidate, folder):
    started = time.time_ns()
    result = subprocess.run([str(runner), str(candidate), str(folder)], cwd=ROOT,
                            env=dict(os.environ, SHORTWIN_SIMPLE_SO=str(own)), capture_output=True)
    finished = time.time_ns()
    parent = folder.parent
    (parent / "stdout.log").write_bytes(result.stdout)
    (parent / "stderr.log").write_bytes(result.stderr)
    receipt = {"started_unix_ns": started, "finished_unix_ns": finished, "returncode": result.returncode,
               "terminated_by_signal": -result.returncode if result.returncode < 0 else None,
               "stdout_sha256": sha(result.stdout), "stderr_sha256": sha(result.stderr),
               "candidate_sha256": sha(candidate.read_bytes()), "native_runner_sha256": sha(runner.read_bytes()),
               "own_program_sha256": sha(own.read_bytes())}
    write_json(parent / "process.json", receipt)
    summary = folder / "summary.json"
    if not summary.is_file():
        raise ValueError("runner exited without a readable summary; returncode=" + str(result.returncode))
    actual = json.loads(summary.read_bytes())
    logs = {stage: checked_log(folder, transaction) for stage, transaction in actual.get("transactions", {}).items()}
    return actual, receipt, logs, sha(summary.read_bytes())


def validate_common(actual, receipt, candidate, review, runner_hash, own_hash):
    if (actual.get("own_program_sha256") != own_hash or actual.get("native_runner_sha256") != runner_hash
            or actual.get("manifest_sha256") != receipt["candidate_sha256"]
            or actual.get("program_id") != "BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh"
            or actual.get("live_transactions_sent") != 0
            or actual.get("review_provenance") != candidate["review_provenance"]
            or actual.get("reviewed_deployment") != candidate["reviewed_deployment"]
            or actual.get("execution_context") != candidate["execution_context"]):
        return False
    jupiter = actual.get("jupiter", {})
    return (jupiter.get("deploy_slot") == review["deploy_slot"]
            and jupiter.get("programdata_len") == review["programdata_len"]
            and jupiter.get("programdata_sha256") == review["programdata"]["sha256"]
            and jupiter.get("program_sha256") == review["program"]["sha256"]
            and jupiter.get("trimmed_elf_sha256") == review["elf_sha256"])


def validate_positive(actual, receipt, logs, case):
    stages = actual.get("transactions", {})
    valid = (receipt["returncode"] == 0 and actual.get("status") == "passed"
             and actual.get("functional_execution_passed") is True
             and actual.get("actual_snapshot_packets_fit") is True and actual.get("packet_fit_failures") == {}
             and actual.get("post_cpi_rollback") is True and actual.get("base_token_conservation") is True
             and actual.get("native_lamport_conservation") is True
             and actual.get("lifecycle_invariants") and all(actual["lifecycle_invariants"].values())
             and actual.get("synthetic_reference", {}).get("functional_policy_max_spread_bps") == 500)
    for name in ("post_swap_fee_bound_rollback", "open_sell", "full_buyback"):
        transaction = stages.get(name, {})
        valid = valid and transaction.get("lookup_mode") == "actual_snapshot_lookup_tables" and 0 < transaction.get("wire_bytes", 0) <= 1232
        valid = valid and ("Program " + JUP + " success") in logs.get(name, "").splitlines()
        valid = valid and (transaction.get("error") is not None if name == "post_swap_fee_bound_rollback" else transaction.get("error") is None)
    valid = valid and "FeeAboveMax" in logs.get("post_swap_fee_bound_rollback", "")
    setup = actual.get("setup_allowance", {})
    if case.get("requires_opening_setup_spend"):
        valid = valid and setup.get("opening_actual_actor_cost", 0) > 0
    pump = actual.get("closing_setup_policy") is not None
    if case.get("requires_zero_close_setup") or pump:
        valid = valid and setup.get("closing_signed_maximum") == setup.get("closing_actual_actor_cost") == 0
    if case.get("requires_setup_budget_rejections") or pump:
        rejected = actual.get("setup_budget_rejections", {})
        valid = valid and len(rejected) >= 4 and all(rejected.values())
    for transaction in stages.values():
        valid = valid and transaction.get("lookup_mode") == "actual_snapshot_lookup_tables" and 0 < transaction.get("wire_bytes", 0) <= 1232
    return bool(valid)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--matrix", type=Path, required=True)
    parser.add_argument("--reviewed-deployment", type=Path, required=True)
    parser.add_argument("--own-program", type=Path, required=True, action="append")
    parser.add_argument("--runner", type=Path, required=True)
    parser.add_argument("--overlay-historical", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    report = {"schema": 1, "status": "running", "scope": "actual-SBF saved-route compatibility; no live landing, history or duration qualification",
              "overlay_historical": args.overlay_historical, "live_transactions_sent": 0, "cases": []}
    status = 1
    try:
        helper_path = ROOT / "scripts/materialize-reviewed-route-candidate.py"
        spec = importlib.util.spec_from_file_location("reviewed_route_materializer", helper_path)
        helper = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(helper)
        review, _, review_hash = helper.load_review(args.reviewed_deployment)
        helper.review_tuple(review["predecessor"])
        matrix_raw = args.matrix.read_bytes()
        matrix = json.loads(matrix_raw)
        if matrix.get("schema") != 1 or matrix.get("program_id") != "BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh":
            raise ValueError("unsupported matrix/program identity")
        if not matrix.get("expected_pass") or matrix.get("pending"):
            raise ValueError("qualification matrix must contain positive cases and no pending cases")
        runner = args.runner.resolve()
        runner_hash = sha(runner.read_bytes())
        report.update({"native_runner_sha256": runner_hash, "matrix_sha256": sha(matrix_raw),
                       "review_manifest_sha256": review_hash, "reviewed_deployment": helper.review_tuple(review),
                       "own_programs": []})
        seen_hashes, names = set(), set()
        for mode in ("expected_pass", "expected_fail"):
            for case in matrix[mode]:
                name = case["name"]
                if Path(name).name != name or name in (".", "..", "predecessor-pin-refusal") or name in names:
                    raise ValueError("matrix case names must be unique simple filenames")
                names.add(name)
        for own_path in args.own_program:
            own = own_path.resolve()
            own_hash = sha(own.read_bytes())
            if own_hash in seen_hashes:
                raise ValueError("duplicate own-program ELF")
            seen_hashes.add(own_hash)
            report["own_programs"].append({"sha256": own_hash, "path": str(own)})
            own_folder = args.output / own_hash
            own_folder.mkdir()
            cases = [(mode, case) for mode in ("expected_pass", "expected_fail") for case in matrix[mode]]
            cases.append(("predecessor_pin_refusal", {"name": "predecessor-pin-refusal", "capture": matrix["expected_pass"][0]["capture"]}))
            for mode, case in cases:
                folder = own_folder / case["name"]
                folder.mkdir()
                capture = (ROOT / case["capture"]).resolve()
                if not capture.is_relative_to(ROOT / "tests/fixtures"):
                    raise ValueError("capture must remain within repository tests/fixtures")
                command = [sys.executable, str(helper_path), str(capture), str(folder / "candidate"),
                           "--own-program-sha256", own_hash, "--reviewed-deployment", str(args.reviewed_deployment.resolve())]
                if args.overlay_historical:
                    command.append("--overlay-historical")
                if mode == "predecessor_pin_refusal":
                    command.append("--predecessor-pin")
                materialized = subprocess.run(command, cwd=ROOT, capture_output=True)
                (folder / "materialize.stdout.log").write_bytes(materialized.stdout)
                (folder / "materialize.stderr.log").write_bytes(materialized.stderr)
                if materialized.returncode != 0:
                    raise ValueError("candidate materialization failed: " + case["name"])
                candidate_path = folder / "candidate/candidate.json"
                candidate = json.loads(candidate_path.read_bytes())
                actual, receipt, logs, summary_hash = replay(runner, own, candidate_path, folder / "replay")
                valid = validate_common(actual, receipt, candidate, review, runner_hash, own_hash)
                if mode == "expected_pass":
                    valid = valid and validate_positive(actual, receipt, logs, case)
                elif mode == "expected_fail":
                    transaction = actual.get("transactions", {}).get(case["transaction"], {})
                    valid = (valid and receipt["returncode"] == 1 and actual.get("status") == "failed"
                             and actual.get("rejected_route_protected_state_unchanged") is True
                             and case["error_substring"] in str(actual.get("error"))
                             and case["transaction_log_substring"] in logs.get(case["transaction"], "")
                             and transaction.get("lookup_mode") == "actual_snapshot_lookup_tables"
                             and 0 < transaction.get("wire_bytes", 0) <= 1232)
                else:
                    log = logs.get("predecessor_pin_initialize", "")
                    valid = (valid and receipt["returncode"] == 1 and actual.get("status") == "failed"
                             and actual.get("predecessor_pin_rejected") is True
                             and actual.get("predecessor_pin_protected_state_unchanged") is True
                             and "Custom(6039)" in str(actual.get("error")) and "RouteAccounts" in log
                             and "custom program error: 0x1797" in log and ("Program " + JUP + " invoke") not in log)
                report["cases"].append({"name": case["name"], "own_program_sha256": own_hash, "expected": mode,
                    "actual": actual.get("status"), "verified": bool(valid), "returncode": receipt["returncode"],
                    "summary_sha256": summary_hash, "process_receipt_sha256": sha((folder / "process.json").read_bytes())})
                write_json(args.output / "summary.json", report)
                if not valid:
                    report.setdefault("errors", []).append("route outcome differs from reviewed matrix: " + case["name"] + " / " + own_hash)
        report["status"] = "failed" if report.get("errors") else "passed"
        status = 1 if report.get("errors") else 0
    except Exception as error:
        report["status"] = "failed"
        report["error"] = str(error)
    finally:
        write_json(args.output / "summary.json", report)
        print(json.dumps(report, indent=2, sort_keys=True))
    return status


if __name__ == "__main__":
    raise SystemExit(main())
