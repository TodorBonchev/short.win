#!/usr/bin/env python3
"""Materialize an additive read-only route capture for the actual-SBF replayer.

It validates all recorded HTTP/RPC response hashes and decoded account hashes.
It never changes the historical fixture manifest or fetches/sends a transaction.
The candidate is not promoted merely because it can be materialized.
"""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path


def digest(data):
    return hashlib.sha256(data).hexdigest()


def evidence_bytes(path):
    if path.is_file():
        return gzip.decompress(path.read_bytes()) if path.suffix == ".gz" else path.read_bytes()
    compressed = Path(str(path) + ".gz")
    if compressed.is_file():
        return gzip.decompress(compressed.read_bytes())
    raise FileNotFoundError(path)


def load(path):
    return json.loads(evidence_bytes(path))


def reconstruct_snapshot(capture, qualification, account_manifest):
    accounts, slots = {}, {}
    for receipt in sorted(qualification["requests"], key=lambda r: r["started_at"]):
        request = receipt.get("request", {})
        if request.get("method") != "getMultipleAccounts":
            continue
        response = load(capture / receipt["file"])["result"]
        addresses = request["params"][0]
        values = response["value"]
        if len(addresses) != len(values):
            raise ValueError("raw RPC account count differs from recorded request")
        for address, account in zip(addresses, values):
            accounts[address] = account
            slots[address] = response["context"]["slot"]
    return {"accounts": accounts, "account_context_slots": slots,
            "snapshot_slot": account_manifest["snapshot_slot"],
            "snapshot_block_time": account_manifest["snapshot_block_time"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--own-program-sha256", required=True, help="Explicit reviewed ELF hash for this replay; no historical default")
    args = parser.parse_args()
    capture = args.capture.resolve()
    qualification = load(capture / "qualification.json")
    account_manifest = load(capture / "accounts.manifest.json")
    if not qualification.get("whole_q_quote_available"):
        raise ValueError("capture lacks a whole-q buy quote")
    for receipt in qualification["requests"]:
        filename = Path(receipt["file"])
        if filename.is_absolute() or ".." in filename.parts:
            raise ValueError("response path escaped capture")
        if digest(evidence_bytes(capture / filename)) != receipt["sha256"]:
            raise ValueError(f"raw response hash mismatch: {filename}")
    # Derive address/value binding from recorded requests and exact RPC replies,
    # including in a clean clone where large replies are stored as .json.gz.
    snapshot = reconstruct_snapshot(capture, qualification, account_manifest)
    materialized = []
    for entry in account_manifest["accounts"]:
        address = entry["key"]
        account = snapshot["accounts"][address]
        if entry["missing"]:
            if account is not None:
                raise ValueError(f"missing account unexpectedly populated: {address}")
            continue
        if account["data"][1] != "base64":
            raise ValueError("only raw base64 account snapshots are supported")
        if snapshot["account_context_slots"][address] != entry["context_slot"]:
            raise ValueError(f"account context slot mismatch: {address}")
        data = base64.b64decode(account["data"][0], validate=True)
        if digest(data) != entry["sha256"] or len(data) != entry["length"]:
            raise ValueError(f"decoded account hash/length mismatch: {address}")
        for rpc_field, manifest_field in [("owner", "owner"), ("lamports", "lamports"), ("executable", "executable"), ("rentEpoch", "rentEpoch")]:
            if account[rpc_field] != entry[manifest_field]:
                raise ValueError(f"account metadata mismatch: {address} {rpc_field}")
        materialized.append((entry, account, data))
    # Refuse overwrite so previous evidence and failed replay attempts survive.
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "accounts").mkdir()
    candidate = {"schema": 1, "name": capture.name, "base_mint": qualification["mint"],
                 "quantity": qualification["quantity"], "context_slot": snapshot["snapshot_slot"],
                 "block_time": snapshot["snapshot_block_time"], "own_program_sha256": args.own_program_sha256,
                 "scope": "non-atomic current mainnet snapshot candidate, no baseline promotion",
                 "capture_files": [], "accounts": [], "lookup_tables": []}
    for filename in ["qualification.json", "accounts.manifest.json"]:
        candidate["capture_files"].append({"file": filename, "sha256": digest(evidence_bytes(capture / filename))})
    for entry, account, data in materialized:
        filename = f"accounts/{entry['key']}.bin"
        (args.output / filename).write_bytes(data)
        candidate["accounts"].append({"address": entry["key"], "file": filename, "sha256": entry["sha256"],
                                      "owner": account["owner"], "lamports": account["lamports"],
                                      "executable": account["executable"], "rent_epoch": account["rentEpoch"],
                                      "context_slot": snapshot["account_context_slots"][entry["key"]]})
    for side in ["sell", "buy"]:
        receipt = qualification[side]
        raw = evidence_bytes(capture / receipt["response_file"])
        if digest(raw) != receipt["response_sha256"]:
            raise ValueError(f"selected {side} response hash mismatch")
        filename = f"{side}.json"
        (args.output / filename).write_bytes(raw)
        candidate[side] = {"file": filename, "sha256": digest(raw)}
        for address in json.loads(raw).get("addressesByLookupTableAddress", {}):
            if address not in candidate["lookup_tables"]:
                candidate["lookup_tables"].append(address)
    (args.output / "candidate.json").write_text(json.dumps(candidate, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"candidate": str(args.output / "candidate.json"), "sha256": digest((args.output / "candidate.json").read_bytes()), "account_count": len(materialized)}, sort_keys=True))


if __name__ == "__main__":
    main()
