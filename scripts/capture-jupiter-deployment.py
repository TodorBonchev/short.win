#!/usr/bin/env python3
"""Retain a finalized mainnet Jupiter/Clock observation. Read-only RPC only."""
import argparse
import base64
import datetime
import gzip
import hashlib
import json
from pathlib import Path
import struct
import urllib.request

RPC = "https://api.mainnet-beta.solana.com"
PROGRAM = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"
PROGRAMDATA = "4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT"
CLOCK = "SysvarC1ock11111111111111111111111111111111"
LOADER = "BPFLoaderUpgradeab1e11111111111111111111111"
MAINNET = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def decode_key(key):
    value = 0
    for char in key:
        value = value * 58 + ALPHABET.index(char)
    return bytes(len(key) - len(key.lstrip("1"))) + value.to_bytes((value.bit_length() + 7) // 8, "big")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    receipts = []

    def request(name, method, params):
        payload = {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
        started = datetime.datetime.now(datetime.timezone.utc).isoformat()
        req = urllib.request.Request(RPC, data=json.dumps(payload).encode(), headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=30) as response:
            raw = response.read(12_000_001)
            if len(raw) > 12_000_000:
                raise ValueError("RPC observation exceeds limit")
            status = response.status
        result = json.loads(raw)
        if status != 200 or "error" in result or "result" not in result:
            raise ValueError("RPC observation failed")
        filename = name + ".json.gz"
        compressed = gzip.compress(raw, mtime=0)
        (args.output / filename).write_bytes(compressed)
        receipts.append({"file": filename, "sha256": sha(raw), "stored_sha256": sha(compressed),
                         "started_at": started, "finished_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                         "rpc": RPC, "status": status, "request": payload})
        return result["result"], filename, sha(raw)

    genesis, _, _ = request("genesis", "getGenesisHash", [])
    if genesis != MAINNET:
        raise ValueError("RPC is not canonical mainnet")
    snapshot, raw_file, raw_hash = request("jupiter-clock", "getMultipleAccounts", [
        [PROGRAM, PROGRAMDATA, CLOCK], {"encoding": "base64", "commitment": "finalized"}])
    values = snapshot["value"]
    if len(values) != 3 or any(value is None for value in values):
        raise ValueError("Missing canonical account")
    data = []
    for value in values:
        if value["data"][1] != "base64":
            raise ValueError("Expected raw base64 account data")
        data.append(base64.b64decode(value["data"][0], validate=True))
    program, pd, clock = data
    if not (values[0]["owner"] == values[1]["owner"] == LOADER
            and values[0]["executable"] is True and values[1]["executable"] is False
            and len(program) == 36 and program[:4] == struct.pack("<I", 2)
            and program[4:] == decode_key(PROGRAMDATA)
            and len(pd) >= 45 and pd[:4] == struct.pack("<I", 3) and pd[12] in (0, 1)
            and pd[45:49] == b"\x7fELF"):
        raise ValueError("Invalid canonical Jupiter loader binding")
    if len(clock) != 40 or values[2]["owner"] != "Sysvar1111111111111111111111111111111111111":
        raise ValueError("Invalid Clock observation")
    slot, _, _, _, unix_timestamp = struct.unpack("<QqQQq", clock)
    deploy_slot = struct.unpack_from("<Q", pd, 4)[0]
    if not (0 < deploy_slot < slot == snapshot["context"]["slot"]):
        raise ValueError("Clock/bank/deployment ordering mismatch")
    elf = pd[45:].rstrip(bytes([0]))
    result = {"schema": 1, "program_id": PROGRAM, "programdata_address": PROGRAMDATA,
              "network": "mainnet-beta", "genesis_hash": genesis,
              "observed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "deploy_slot": deploy_slot, "programdata_len": len(pd), "elf_sha256": sha(elf),
              "elf_trimmed_length": len(elf), "padded_elf_sha256": sha(pd[45:]),
              "scope": "Finalized same-bank read-only mainnet identity; not compatibility approval or deployment authorization",
              "live_transactions_sent": 0, "requests": receipts}
    for name, key, value, binary in zip(["program", "programdata", "clock_account"], [PROGRAM, PROGRAMDATA, CLOCK], values, data):
        filename = name + ".bin"
        (args.output / filename).write_bytes(binary)
        result[name] = {"address": key, "file": filename, "sha256": sha(binary),
                        "context_slot": snapshot["context"]["slot"],
                        **{field: value[field] for field in ["owner", "lamports", "executable", "rentEpoch"]}}
        result[name]["rent_epoch"] = result[name].pop("rentEpoch")
    result["clock"] = {"address": CLOCK, "slot": slot, "unix_timestamp": unix_timestamp,
                       "context_slot": snapshot["context"]["slot"],
                       "source_file": raw_file, "source_sha256": raw_hash}
    (args.output / "jupiter.so").write_bytes(elf)
    (args.output / "deployment.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(json.dumps({key: result[key] for key in ["observed_at", "deploy_slot", "programdata_len", "elf_sha256", "live_transactions_sent"]}))


if __name__ == "__main__":
    main()
