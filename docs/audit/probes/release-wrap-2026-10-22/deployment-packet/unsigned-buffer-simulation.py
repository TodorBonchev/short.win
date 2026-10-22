#!/usr/bin/env python3
"""One bounded, unsigned devnet loader-account preparation simulation. Never signs/sends."""
import base64
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import struct
import subprocess

OUT = Path(__file__).resolve().parent
RPC = "https://api.devnet.solana.com"
GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
PAYER = "3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14"
PROGRAM = "5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK"
PROGRAMDATA = "5UUu4yttKrtDNegGbUgZBxubJPaoPrsuiybeCJ8BnqcF"
SYSTEM = "11111111111111111111111111111111"
LOADER = "BPFLoaderUpgradeab1e11111111111111111111111"
COMPUTE = "ComputeBudget111111111111111111111111111111"
RENT = "SysvarRent111111111111111111111111111111111"
CLOCK = "SysvarC1ock11111111111111111111111111111111"
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
ALLOWED = {"getGenesisHash", "getLatestBlockhash", "getMultipleAccounts", "getMinimumBalanceForRentExemption", "getFeeForMessage", "simulateTransaction"}

def b58decode(value):
    n = 0
    for ch in value:
        n = n * 58 + ALPHABET.index(ch)
    return b"\0" * (len(value) - len(value.lstrip("1"))) + (n.to_bytes((n.bit_length() + 7) // 8, "big") if n else b"")

def b58encode(value):
    n = int.from_bytes(value, "big")
    result = ""
    while n:
        n, rem = divmod(n, 58)
        result = ALPHABET[rem] + result
    return "1" * (len(value) - len(value.lstrip(b"\0"))) + result

def compact(n):
    encoded = bytearray()
    while True:
        byte = n & 127
        n >>= 7
        encoded.append(byte | (128 if n else 0))
        if not n:
            return bytes(encoded)

def instruction(program_index, accounts, data):
    return bytes([program_index]) + compact(len(accounts)) + bytes(accounts) + compact(len(data)) + data

def message(keys, required, readonly_unsigned, blockhash, instructions):
    decoded = [b58decode(key) for key in keys]
    assert all(len(key) == 32 for key in decoded)
    assert len(b58decode(blockhash)) == 32
    return bytes([required, 0, readonly_unsigned]) + compact(len(keys)) + b"".join(decoded) + b58decode(blockhash) + compact(len(instructions)) + b"".join(instructions)

def unsigned_tx(msg, required):
    return compact(required) + bytes(64 * required) + msg

def rpc_batch(label, calls):
    assert all(method in ALLOWED for _, method, _ in calls)
    requests = [{"jsonrpc": "2.0", "id": ident, "method": method, "params": params} for ident, method, params in calls]
    request_path = OUT / (label + "-request.json")
    request_path.write_text(json.dumps(requests, indent=2) + "\n")
    result = subprocess.run(["curl", "--fail-with-body", "--silent", "--show-error", "--max-time", "30", "--request", "POST", "--header", "Content-Type: application/json", "--data-binary", "@" + str(request_path), RPC], capture_output=True, text=True, check=True)
    responses = json.loads(result.stdout)
    assert isinstance(responses, list) and len(responses) == len(requests)
    byid = {response["id"]: response for response in responses}
    assert len(byid) == len(requests)
    for ident, _, _ in calls:
        assert ident in byid and "error" not in byid[ident], byid.get(ident)
    (OUT / (label + "-raw.json")).write_text(json.dumps({"observed_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(), "rpc": RPC, "responses": responses}, indent=2) + "\n")
    return byid

# Public placeholder derived from a label, not from any private key. It has no
# available signer and MUST NOT be used as the eventual deployment buffer.
args = argparse.ArgumentParser(description=__doc__)
args.add_argument("--buffer-public-key", help="Public address only, for a later already-created off-repo buffer signer. This script never opens its keypair file.")
options = args.parse_args()
buffer = options.buffer_public_key or b58encode(hashlib.sha256(b"short.win unsigned buffer simulation only 2026-10-22").digest())
assert len(b58decode(buffer)) == 32
state = rpc_batch("simulation-context", [
    ("genesis", "getGenesisHash", []),
    ("blockhash", "getLatestBlockhash", [{"commitment": "confirmed"}]),
    ("accounts", "getMultipleAccounts", [[buffer, PROGRAM, PROGRAMDATA], {"commitment": "confirmed", "encoding": "base64", "dataSlice": {"offset": 0, "length": 0}}]),
    ("rent_program", "getMinimumBalanceForRentExemption", [36, {"commitment": "confirmed"}]),
    ("rent_programdata", "getMinimumBalanceForRentExemption", [1644069, {"commitment": "confirmed"}]),
])
assert state["genesis"]["result"] == GENESIS
assert state["accounts"]["result"]["value"] == [None, None, None]
blockhash = state["blockhash"]["result"]["value"]["blockhash"]
program_rent = state["rent_program"]["result"]
programdata_rent = state["rent_programdata"]["result"]
assert isinstance(program_rent, int) and isinstance(programdata_rent, int)
compute_data = [bytes([3]) + struct.pack("<Q", 0), bytes([2]) + struct.pack("<I", 1_400_000)]

# Exact initial loader-account preparation shape used by the selected CLI path.
initial = message([PAYER, buffer, SYSTEM, LOADER, COMPUTE], 2, 3, blockhash, [
    instruction(2, [0, 1], struct.pack("<IQQ", 0, programdata_rent, 1644061) + b58decode(LOADER)),
    instruction(3, [1, 0], struct.pack("<I", 0)),
    *[instruction(4, [], data) for data in compute_data],
])
write_baseline = message([PAYER, buffer, LOADER, COMPUTE], 1, 2, blockhash, [
    instruction(2, [1, 0], struct.pack("<IIQ", 1, 0, 0)),
    *[instruction(3, [], data) for data in compute_data],
])
chunk_size = 1232 - len(unsigned_tx(write_baseline, 1)) - 1
assert chunk_size > 0
maximum_write_count = (1644024 + chunk_size - 1) // chunk_size
final = message([PAYER, PROGRAM, PROGRAMDATA, buffer, SYSTEM, LOADER, RENT, CLOCK, COMPUTE], 2, 5, blockhash, [
    instruction(4, [0, 1], struct.pack("<IQQ", 0, program_rent, 36) + b58decode(LOADER)),
    instruction(5, [0, 2, 1, 3, 6, 7, 4, 0], struct.pack("<IQ", 2, 1644024)),
    *[instruction(8, [], data) for data in compute_data],
])
initial_wire = unsigned_tx(initial, 2)
assert len(initial_wire) <= 1232
(OUT / "unsigned-initial-transaction.base64").write_text(base64.b64encode(initial_wire).decode() + "\n")
calls = [
    ("initial_fee", "getFeeForMessage", [base64.b64encode(initial).decode(), {"commitment": "confirmed"}]),
    ("write_fee", "getFeeForMessage", [base64.b64encode(write_baseline).decode(), {"commitment": "confirmed"}]),
    ("final_fee", "getFeeForMessage", [base64.b64encode(final).decode(), {"commitment": "confirmed"}]),
    ("initial_simulation", "simulateTransaction", [base64.b64encode(initial_wire).decode(), {"encoding": "base64", "sigVerify": False, "replaceRecentBlockhash": True, "commitment": "confirmed", "minContextSlot": state["accounts"]["result"]["context"]["slot"]}]),
]
result = rpc_batch("unsigned-simulation", calls)
fees = {name: result[name + "_fee"]["result"]["value"] for name in ["initial", "write", "final"]}
assert all(isinstance(fee, int) and fee >= 0 for fee in fees.values())
simulation = result["initial_simulation"]["result"]
summary = {
    "observed_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "scope": "unsigned create-buffer and initialize-buffer only; no ELF upload, final deploy, or program invocation",
    "rpc": RPC, "genesis": GENESIS, "transaction_version": "legacy (mirrors stock solana-cli 4.1.2)",
    "signatures": "two 64-byte zero placeholders; no signing",
    "sigVerify": False, "replaceRecentBlockhash": True,
    "simulation_buffer_public_key": buffer,
    "buffer_is_placeholder": options.buffer_public_key is None,
    "buffer_signer_verified_by_this_script": False,
    "payer": PAYER, "buffer_authority": PAYER,
    "buffer_space_bytes": 1644061, "buffer_funding_lamports": programdata_rent,
    "initial_transaction_wire_bytes": len(initial_wire),
    "simulation_context": simulation["context"],
    "simulation_error": simulation["value"]["err"],
    "simulation_units_consumed": simulation["value"].get("unitsConsumed"),
    "simulation_fee_lamports": simulation["value"].get("fee"),
    "quoted_fees_lamports": fees,
    "write_baseline_transaction_bytes": len(unsigned_tx(write_baseline, 1)),
    "cli_maximum_write_chunk_bytes": chunk_size,
    "maximum_upload_write_transactions_before_any_resigning": maximum_write_count,
    "one_pass_maximum_fees_lamports": fees["initial"] + fees["write"] * maximum_write_count + fees["final"],
    "final_deployment_simulated": False,
    "final_simulation_limitation": "No populated real buffer exists on chain. Full final loader deployment cannot be simulated against current devnet state before uploading it. Explicit compute price 0 causes the selected CLI to simulate initial preparation, the first write, and final deploy during a later approved deployment.",
    "broadcast_or_key_access": False,
}
(OUT / "unsigned-simulation-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))
assert summary["simulation_error"] is None, "Unsigned buffer preparation failed; inspect retained response."
