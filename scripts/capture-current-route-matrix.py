#!/usr/bin/env python3
"""Capture fresh public mainnet route inputs; never sign, simulate or send.

Recipes come from an explicit retained matrix and its historical qualification
manifests. They select public mints/quantities only: every build/account response
in the output is requested anew. Authentication may come only from the inherited
JUPITER_API_KEY; neither it nor request headers are recorded.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--matrix", type=Path, default=ROOT / "tests/runtime/current-routes.json")
    parser.add_argument("--resume", action="store_true", help="Keep completed captures and continue a partially captured output; never overwrite case evidence")
    args = parser.parse_args()
    args.output = args.output.resolve()
    args.matrix = args.matrix.resolve()
    args.output.mkdir(parents=True, exist_ok=args.resume)
    matrix = json.loads(args.matrix.read_bytes())
    collector = ROOT / "scripts/qualify-current-routes.py"
    report = {
        "schema": 1,
        "scope": "fresh non-atomic read-only mainnet quote/build and account capture; no execution qualification",
        "started_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "historical_recipe_matrix": str(args.matrix.relative_to(ROOT)),
        "historical_recipe_matrix_sha256": sha(args.matrix.read_bytes()),
        "collector_sha256": sha(collector.read_bytes()),
        "orchestrator_sha256": sha(Path(__file__).read_bytes()),
        "rpc_endpoint": "https://api.mainnet-beta.solana.com",
        "jupiter_endpoint": "https://api.jup.ag/swap/v2/build",
        "api_key_present": bool(os.environ.get("JUPITER_API_KEY")),
        "live_transactions_sent": 0,
        "cases": [],
    }
    env = dict(os.environ, SOLANA_RPC_URL=report["rpc_endpoint"])
    for mode in ("expected_pass", "expected_fail"):
        for case in matrix[mode]:
            recipe_path = ROOT / case["capture"] / "qualification.json"
            recipe = json.loads(recipe_path.read_bytes())
            output = args.output / case["name"]
            command = [sys.executable, str(collector), "--output", str(output),
                       "--mint", recipe["mint"], "--quantity", str(recipe["quantity"]),
                       "--program-id", recipe["program_id"],
                       "--borrower", recipe["binding"]["borrower"],
                       "--route-budgets", ",".join(map(str, recipe["request_account_budgets"])),
                       "--production-observation", "--max-spread-bps", "500"]
            if recipe.get("dexes"):
                command += ["--dexes", recipe["dexes"]]
            elif recipe.get("exclude_dexes"):
                command += ["--exclude-dexes", recipe["exclude_dexes"]]
            if recipe.get("diagnostic_ineligible_capture"):
                command += ["--capture-ineligible"]
            reused = output.exists()
            if reused:
                if not args.resume:
                    raise ValueError("case output already exists: " + str(output))
                returncode = 0 if (output / "qualification.json").is_file() else 1
            else:
                result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True)
                returncode = result.returncode
                output.mkdir(exist_ok=True)
                (output / "collector.stdout.log").write_bytes(result.stdout)
                (output / "collector.stderr.log").write_bytes(result.stderr)
            item = {"name": case["name"], "historical_expectation": mode,
                    "historical_recipe_sha256": sha(recipe_path.read_bytes()),
                    "command": command, "returncode": returncode,
                    "preserved_existing_case": reused,
                    "capture": str(output.relative_to(ROOT)),
                    "status": "captured" if returncode == 0 else "capture_failed"}
            qualification = output / "qualification.json"
            if qualification.is_file():
                q = json.loads(qualification.read_bytes())
                item.update({key: q[key] for key in ["snapshot_slot", "whole_q_quote_available",
                             "within_500bps_spread", "mint_eligible_plain82_rule"]})
                item["sell_labels"] = q["sell"]["labels"]
                item["buy_labels"] = q["buy"]["labels"] if q["buy"] else None
            report["cases"].append(item)
            write(args.output / "capture-summary.json", report)
            print(json.dumps(item), flush=True)
    report["finished_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    report["status"] = "captured" if all(c["returncode"] == 0 for c in report["cases"]) else "incomplete"
    write(args.output / "capture-summary.json", report)
    return 0 if report["status"] == "captured" else 1


if __name__ == "__main__":
    raise SystemExit(main())
