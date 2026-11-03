#!/usr/bin/env python3
"""Bind a saved route capture to an explicit retained Jupiter deployment review.

The default verifies the capture already contains the reviewed program bytes.
--overlay-historical instead replaces only Jupiter Program/ProgramData and the
VM execution Clock; all route, mint, DEX and lookup-table evidence stays saved
historical input. Neither mode fetches, signs, sends or promotes a transaction.
"""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
JUP = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"
JUP_PD = "4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT"
LOADER = "BPFLoaderUpgradeab1e11111111111111111111111"
CLOCK = "SysvarC1ock11111111111111111111111111111111"
B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def checked(root, entry, file_key="file", hash_key="sha256"):
    relative = Path(entry[file_key])
    path = (root / relative).resolve()
    if relative.is_absolute() or ".." in relative.parts or not path.is_relative_to(root.resolve()):
        raise ValueError("review evidence path escapes manifest directory")
    stored = path.read_bytes()
    if entry.get("stored_sha256") and sha(stored) != entry["stored_sha256"]:
        raise ValueError("stored review evidence hash mismatch: " + str(relative))
    raw = gzip.decompress(stored) if relative.suffix == ".gz" else stored
    if sha(raw) != entry[hash_key]:
        raise ValueError("review evidence hash mismatch: " + str(relative))
    return raw


def pubkey_bytes(value):
    number = 0
    for character in value:
        number = number * 58 + B58.index(character)
    raw = number.to_bytes((number.bit_length() + 7) // 8, "big")
    raw = bytes(len(value) - len(value.lstrip("1"))) + raw
    if len(raw) != 32:
        raise ValueError("invalid canonical public key")
    return raw


def review_tuple(value):
    result = {key: value[key] for key in ("deploy_slot", "programdata_len", "elf_sha256")}
    if (type(result["deploy_slot"]) is not int or not 0 < result["deploy_slot"] < 2**64
            or type(result["programdata_len"]) is not int or not 45 < result["programdata_len"] < 2**32
            or not isinstance(result["elf_sha256"], str) or len(result["elf_sha256"]) != 64
            or result["elf_sha256"] != result["elf_sha256"].lower()
            or len(bytes.fromhex(result["elf_sha256"])) != 32 or int(result["elf_sha256"], 16) == 0):
        raise ValueError("invalid reviewed deployment tuple")
    return result


def load_review(path):
    path = path.resolve()
    manifest_raw = path.read_bytes()
    value = json.loads(manifest_raw)
    if value.get("schema") != 1 or value.get("program_id") != JUP or value.get("programdata_address") != JUP_PD:
        raise ValueError("review must bind canonical Jupiter Program and ProgramData")
    root = path.parent
    program, pd = (checked(root, value[name]) for name in ("program", "programdata"))
    observations, contexts, clock_index, genesis_checked = {}, {}, None, False
    for receipt in value["requests"]:
        response = json.loads(checked(root, receipt))
        if receipt.get("status") != 200 or response.get("error"):
            raise ValueError("review source RPC request did not succeed")
        request, result = receipt["request"], response["result"]
        if request["method"] == "getMultipleAccounts":
            addresses = request["params"][0]
            if len(addresses) != len(result["value"]):
                raise ValueError("review source RPC account count mismatch")
            for index, (address, account) in enumerate(zip(addresses, result["value"])):
                observations[address] = account
                contexts[address] = result["context"]["slot"]
                if address == CLOCK and receipt["file"] == value["clock"]["source_file"]:
                    clock_index = index
        elif request["method"] == "getGenesisHash":
            if result != value.get("genesis_hash") or result != "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d":
                raise ValueError("review RPC genesis identity is not mainnet-beta")
            genesis_checked = True
    if not genesis_checked:
        raise ValueError("review is missing retained mainnet-beta genesis evidence")
    for name, executable in (("program", True), ("programdata", False)):
        account = value[name]
        address = JUP if name == "program" else JUP_PD
        if account.get("address", address) != address or account["owner"] != LOADER or account["executable"] is not executable:
            raise ValueError("reviewed account has invalid loader owner/executable flag")
        for field in ("lamports", "rent_epoch", "context_slot"):
            if type(account[field]) is not int or not 0 <= account[field] < 2**64:
                raise ValueError("invalid reviewed account field: " + field)
        observation = observations.get(address)
        if not observation or observation["data"][1] != "base64" or base64.b64decode(observation["data"][0], validate=True) != (program if name == "program" else pd):
            raise ValueError("reviewed bytes differ from canonical source RPC address binding")
        if (any(observation[rpc] != account[manifest] for rpc, manifest in
                (("owner", "owner"), ("lamports", "lamports"), ("executable", "executable"), ("rentEpoch", "rent_epoch")))
                or contexts[address] != account["context_slot"]):
            raise ValueError("reviewed metadata differs from canonical source RPC address binding")
    if len(program) != 36 or program[:4] != struct.pack("<I", 2) or program[4:] != pubkey_bytes(JUP_PD):
        raise ValueError("canonical Jupiter Program does not bind canonical ProgramData")
    if len(pd) < 45 or pd[:4] != struct.pack("<I", 3) or pd[12] not in (0, 1):
        raise ValueError("invalid canonical Jupiter ProgramData state")
    elf = pd[45:].rstrip(bytes(1))
    actual = {"deploy_slot": struct.unpack("<Q", pd[4:12])[0], "programdata_len": len(pd), "elf_sha256": sha(elf)}
    if elf[:4] != b"\x7fELF" or review_tuple(value) != actual:
        raise ValueError("review tuple does not match exact retained Jupiter bytes")
    clock = value["clock"]
    if clock.get("address", CLOCK) != CLOCK:
        raise ValueError("execution Clock evidence must identify canonical Clock")
    source = checked(root, clock, "source_file", "source_sha256")
    response = json.loads(source)
    result = response.get("result", response)
    account = result.get("value", result)
    if isinstance(account, list):
        index = clock_index
        if type(index) is not int or not 0 <= index < len(account):
            raise ValueError("Clock array evidence requires canonical retained request-address binding")
        if "source_account_index" in clock and clock["source_account_index"] != index:
            raise ValueError("explicit Clock index differs from canonical retained request-address binding")
        account = account[index]
    if account != observations.get(CLOCK):
        raise ValueError("Clock evidence differs from canonical retained request-address binding")
    if account.get("owner") != "Sysvar1111111111111111111111111111111111111" or account.get("executable") is not False:
        raise ValueError("Clock evidence has invalid sysvar owner/executable flag")
    data = account["data"]
    if data[1] != "base64":
        raise ValueError("Clock evidence must use raw base64")
    clock_data = base64.b64decode(data[0], validate=True)
    if len(clock_data) != 40:
        raise ValueError("Clock evidence must contain exact 40-byte Clock")
    if "clock_account" in value:
        clock_account = value["clock_account"]
        if (checked(root, clock_account) != clock_data or clock_account.get("address") != CLOCK
                or clock_account["context_slot"] != contexts[CLOCK]
                or any(account[rpc] != clock_account[manifest] for rpc, manifest in
                       (("owner", "owner"), ("lamports", "lamports"), ("executable", "executable"), ("rentEpoch", "rent_epoch")))):
            raise ValueError("retained Clock binary/metadata differs from canonical source RPC")
    slot, _, _, _, timestamp = struct.unpack("<QqQQq", clock_data)
    if clock["slot"] != slot or clock["unix_timestamp"] != timestamp or slot <= actual["deploy_slot"] or timestamp <= 0:
        raise ValueError("execution Clock differs from retained observation or precedes deployment")
    if "context_slot" in clock and clock["context_slot"] != result["context"]["slot"]:
        raise ValueError("Clock RPC context slot mismatch")
    if any(value[name]["context_slot"] < actual["deploy_slot"] for name in ("program", "programdata")):
        raise ValueError("reviewed program account observation predates deployment")
    return value, {"program": program, "programdata": pd}, sha(manifest_raw)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--own-program-sha256", required=True)
    parser.add_argument("--reviewed-deployment", type=Path, required=True)
    parser.add_argument("--overlay-historical", action="store_true")
    parser.add_argument("--predecessor-pin", action="store_true", help="Real onchain initialization must refuse the retained predecessor review")
    args = parser.parse_args()
    capture = args.capture.resolve()
    if not capture.is_relative_to(ROOT / "tests/fixtures"):
        raise ValueError("capture must remain within repository tests/fixtures")
    review, blobs, review_hash = load_review(args.reviewed_deployment)
    pin = review_tuple(review)
    if args.predecessor_pin:
        pin = review_tuple(review["predecessor"])
        if pin["deploy_slot"] >= review["deploy_slot"]:
            raise ValueError("predecessor review must have an older deployment slot")
    subprocess.run([sys.executable, str(ROOT / "scripts/materialize-current-route-candidate.py"),
                    str(capture), str(args.output), "--own-program-sha256", args.own_program_sha256], check=True)
    candidate_path = args.output / "candidate.json"
    candidate = json.loads(candidate_path.read_bytes())
    historical_context = {"slot": candidate["context_slot"], "unix_timestamp": candidate["block_time"]}
    for name, address in (("program", JUP), ("programdata", JUP_PD)):
        entries = [account for account in candidate["accounts"] if account["address"] == address]
        if len(entries) != 1:
            raise ValueError("capture must contain exactly one canonical Jupiter " + name)
        account, observed = entries[0], review[name]
        captured = checked(args.output, account)
        if args.overlay_historical:
            (args.output / account["file"]).write_bytes(blobs[name])
            account.update({key: observed[key] for key in ("sha256", "owner", "lamports", "executable", "rent_epoch", "context_slot")})
        elif (captured != blobs[name] or account["owner"] != observed["owner"]
              or account["executable"] is not observed["executable"]):
            raise ValueError("fresh capture Jupiter bytes/loader metadata differ from reviewed deployment; explicit historical overlay required")
    if args.overlay_historical:
        candidate["context_slot"] = review["clock"]["slot"]
        candidate["block_time"] = review["clock"]["unix_timestamp"]
    if candidate["context_slot"] <= review["deploy_slot"]:
        raise ValueError("candidate execution slot precedes reviewed deployment")
    scope = ("historical route/account regression with current reviewed Jupiter bytes and synthetic current VM Clock"
             if args.overlay_historical else "fresh non-atomic route/account capture with exact current reviewed Jupiter bytes and synthetic snapshot VM Clock")
    candidate["scope"] = scope
    candidate["reviewed_deployment"] = pin
    candidate["execution_context"] = {"slot": candidate["context_slot"], "unix_timestamp": candidate["block_time"],
                                      "synthetic_vm_clock": True, "source": "reviewed current Clock observation" if args.overlay_historical else "captured maximum account slot and getBlockTime"}
    candidate["review_provenance"] = {"scope": scope, "overlay_historical": args.overlay_historical,
        "review_manifest_sha256": review_hash, "source_capture": str(capture.relative_to(ROOT)),
        "source_capture_context": historical_context, "reviewed_current_clock": review["clock"],
        "overlaid_accounts": [JUP, JUP_PD] if args.overlay_historical else [],
        "current_deployment": review_tuple(review), "live_transactions_sent": 0,
        "limitations": ["Historical overlay is compatibility evidence, not fresh mainnet route/account evidence."] if args.overlay_historical else []}
    if args.predecessor_pin:
        candidate["replay_mode"] = "predecessor_pin_initialization_refusal"
    write_json(candidate_path, candidate)
    print(json.dumps({"candidate": str(candidate_path), "sha256": sha(candidate_path.read_bytes()), "scope": scope}, sort_keys=True))


if __name__ == "__main__":
    main()
