#!/usr/bin/env python3
"""Unsigned first-missing-write simulation and fee quotes; never signs or sends."""
import base64
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
BUFFER = "2JGjQoiGyDLTsqy9186Ct7kN4zdG2ULb8nARjN7ddgcL"
SYSTEM = "11111111111111111111111111111111"
LOADER = "BPFLoaderUpgradeab1e11111111111111111111111"
COMPUTE = "ComputeBudget111111111111111111111111111111"
RENT = "SysvarRent111111111111111111111111111111111"
CLOCK = "SysvarC1ock11111111111111111111111111111111"
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"

def b58decode(value):
    number = 0
    for ch in value:
        number = number * 58 + ALPHABET.index(ch)
    return b"\0" * (len(value) - len(value.lstrip("1"))) + (number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b"")

def compact(number):
    result = bytearray()
    while True:
        byte = number & 127
        number >>= 7
        result.append(byte | (128 if number else 0))
        if not number:
            return bytes(result)

def instruction(program_index, accounts, data):
    return bytes([program_index]) + compact(len(accounts)) + bytes(accounts) + compact(len(data)) + data

def message(keys, signatures, readonly_unsigned, blockhash, instructions):
    decoded = [b58decode(key) for key in keys]
    assert all(len(key) == 32 for key in decoded) and len(b58decode(blockhash)) == 32
    return bytes([signatures, 0, readonly_unsigned]) + compact(len(keys)) + b"".join(decoded) + b58decode(blockhash) + compact(len(instructions)) + b"".join(instructions)

def batch(label, calls):
    assert all(method in {"getGenesisHash", "getLatestBlockhash", "getFeeForMessage", "simulateTransaction"} for _, method, _ in calls)
    requests = [{"jsonrpc": "2.0", "id": ident, "method": method, "params": params} for ident, method, params in calls]
    (OUT / (label + "-request.json")).write_text(json.dumps(requests, indent=2) + "\n")
    response = subprocess.run(["curl", "--fail-with-body", "--silent", "--show-error", "--max-time", "30", "--request", "POST", "--header", "Content-Type: application/json", "--data-binary", "@" + str(OUT / (label + "-request.json")), RPC], capture_output=True, text=True, check=True)
    raw = json.loads(response.stdout)
    assert isinstance(raw, list) and len(raw) == len(calls)
    byid = {item["id"]: item for item in raw}
    assert len(byid) == len(calls) and all("error" not in item for item in raw)
    (OUT / (label + "-raw.json")).write_text(json.dumps({"observed_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(), "rpc": RPC, "responses": raw}, indent=2) + "\n")
    return byid

progress = json.loads((OUT / "recovery-public-summary.json").read_text())
artifact = (OUT / "shortwin-d6f4e092.so").read_bytes()
assert len(artifact) == 1644024 and hashlib.sha256(artifact).hexdigest() == progress["accepted_artifact_sha256"]
assert progress["remaining_write_chunks"] > 0 and progress["buffer_authority"] == PAYER
index = progress["first_remaining_chunk_index"]
offset = index * 960
chunk = artifact[offset:offset + 960]
context = batch("recovery-simulation-context", [
    ("genesis", "getGenesisHash", []),
    ("blockhash", "getLatestBlockhash", [{"commitment": "confirmed"}]),
])
assert context["genesis"]["result"] == GENESIS
blockhash = context["blockhash"]["result"]["value"]["blockhash"]
compute = [bytes([3]) + struct.pack("<Q", 0), bytes([2]) + struct.pack("<I", 1400000)]
write_message = message([PAYER, BUFFER, LOADER, COMPUTE], 1, 2, blockhash, [
    instruction(2, [1, 0], struct.pack("<IIQ", 1, offset, len(chunk)) + chunk),
    *[instruction(3, [], data) for data in compute],
])
final_message = message([PAYER, PROGRAM, PROGRAMDATA, BUFFER, SYSTEM, LOADER, RENT, CLOCK, COMPUTE], 2, 5, blockhash, [
    instruction(4, [0, 1], struct.pack("<IQQ", 0, progress["program_rent_lamports"], 36) + b58decode(LOADER)),
    instruction(5, [0, 2, 1, 3, 6, 7, 4, 0], struct.pack("<IQ", 2, 1644024)),
    *[instruction(8, [], data) for data in compute],
])
unsigned_write = compact(1) + bytes(64) + write_message
assert len(unsigned_write) <= 1232
result = batch("recovery-unsigned-simulation", [
    ("write_fee", "getFeeForMessage", [base64.b64encode(write_message).decode(), {"commitment": "confirmed"}]),
    ("final_fee", "getFeeForMessage", [base64.b64encode(final_message).decode(), {"commitment": "confirmed"}]),
    ("write_simulation", "simulateTransaction", [base64.b64encode(unsigned_write).decode(), {"encoding": "base64", "sigVerify": False, "replaceRecentBlockhash": True, "commitment": "confirmed", "minContextSlot": progress["context"]["slot"]}]),
])
write_fee = result["write_fee"]["result"]["value"]
final_fee = result["final_fee"]["result"]["value"]
assert isinstance(write_fee, int) and isinstance(final_fee, int)
sim = result["write_simulation"]["result"]
fees = progress["remaining_write_chunks"] * write_fee + final_fee
summary = {
    "observed_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "scope": "Unsigned simulation of exactly the first remaining loader Write; no buffer creation, final deploy, signing or broadcast.",
    "rpc": RPC, "genesis": GENESIS, "buffer": BUFFER, "payer_and_buffer_authority": PAYER,
    "chunk_index": index, "offset": offset, "chunk_bytes": len(chunk),
    "legacy_transaction_wire_bytes": len(unsigned_write),
    "signature": "one zero 64-byte placeholder; no signing", "sigVerify": False, "replaceRecentBlockhash": True,
    "context": sim["context"], "simulation_error": sim["value"]["err"],
    "units_consumed": sim["value"].get("unitsConsumed"), "simulation_fee_lamports": sim["value"].get("fee"),
    "fresh_quoted_write_fee_lamports": write_fee, "fresh_quoted_final_fee_lamports": final_fee,
    "remaining_write_count_at_progress_snapshot": progress["remaining_write_chunks"],
    "estimated_one_pass_remaining_fees_lamports": fees,
    "estimated_additional_payer_debit_lamports": fees + progress["program_rent_lamports"],
    "estimated_payer_balance_after_success_lamports": progress["payer_balance_lamports"] - fees - progress["program_rent_lamports"],
    "final_deployment_simulated": False,
    "simulation_limit": "This verifies one loader write against present devnet state. The incomplete buffer prevents a meaningful successful final deployment simulation; remaining writes must land before the CLI's final stage simulation.",
    "signed_sent_retried_reclaimed": False,
}
(OUT / "recovery-simulation-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))
assert summary["simulation_error"] is None
