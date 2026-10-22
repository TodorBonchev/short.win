#!/usr/bin/env python3
"""Bounded public recovery inspection. No signers, signing, or broadcast methods."""
import base64
import datetime
import hashlib
import json
from pathlib import Path
import subprocess

OUT = Path(__file__).resolve().parent
RPC = "https://api.devnet.solana.com"
GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
PAYER = "3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14"
PROGRAM = "5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK"
PROGRAMDATA = "5UUu4yttKrtDNegGbUgZBxubJPaoPrsuiybeCJ8BnqcF"
BUFFER = "2JGjQoiGyDLTsqy9186Ct7kN4zdG2ULb8nARjN7ddgcL"
LOADER = "BPFLoaderUpgradeab1e11111111111111111111111"
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"

def b58encode(raw):
    n = int.from_bytes(raw, "big")
    value = ""
    while n:
        n, rem = divmod(n, 58)
        value = ALPHABET[rem] + value
    return "1" * (len(raw) - len(raw.lstrip(b"\0"))) + value

requests = [
    {"jsonrpc": "2.0", "id": "genesis", "method": "getGenesisHash", "params": []},
    {"jsonrpc": "2.0", "id": "accounts", "method": "getMultipleAccounts", "params": [[PAYER, PROGRAM, PROGRAMDATA, BUFFER], {"commitment": "confirmed", "encoding": "base64"}]},
    {"jsonrpc": "2.0", "id": "program_rent", "method": "getMinimumBalanceForRentExemption", "params": [36, {"commitment": "confirmed"}]},
    {"jsonrpc": "2.0", "id": "programdata_rent", "method": "getMinimumBalanceForRentExemption", "params": [1644069, {"commitment": "confirmed"}]},
]
assert all(request["method"] in {"getGenesisHash", "getMultipleAccounts", "getMinimumBalanceForRentExemption"} for request in requests)
(OUT / "recovery-state-request.json").write_text(json.dumps(requests, indent=2) + "\n")
result = subprocess.run(["curl", "--fail-with-body", "--silent", "--show-error", "--max-time", "30", "--max-filesize", "3000000", "--request", "POST", "--header", "Content-Type: application/json", "--data-binary", "@" + str(OUT / "recovery-state-request.json"), RPC], capture_output=True, text=True, check=True)
raw = json.loads(result.stdout)
assert isinstance(raw, list) and len(raw) == len(requests)
responses = {response["id"]: response for response in raw}
assert len(responses) == len(requests) and all("error" not in response for response in raw)
assert responses["genesis"]["result"] == GENESIS
accounts = responses["accounts"]["result"]["value"]
assert len(accounts) == 4 and accounts[1:3] == [None, None]
payer, _, _, buffer = accounts
assert payer["owner"] == "11111111111111111111111111111111" and payer["space"] == 0 and payer["executable"] is False
assert buffer["owner"] == LOADER and buffer["space"] == 1644061 and buffer["executable"] is False
assert buffer["data"][1] == "base64"
buffer_data = base64.b64decode(buffer["data"][0], validate=True)
assert len(buffer_data) == 1644061
assert int.from_bytes(buffer_data[:4], "little") == 1 and buffer_data[4] == 1
authority = b58encode(buffer_data[5:37])
assert authority == PAYER
observed = datetime.datetime.now(datetime.timezone.utc).isoformat()
(OUT / "recovery-state-raw.json").write_text(json.dumps({"observed_at_utc": observed, "rpc": RPC, "responses": raw}, indent=2) + "\n")

# Comparison measures upload progress against the already accepted public file.
# It is not an ELF validation, rebuild, security review, or qualification result.
expected = (OUT / "shortwin-d6f4e092.so").read_bytes()
assert len(expected) == 1644024 and hashlib.sha256(expected).hexdigest() == "d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc"
payload = buffer_data[37:]
chunk_size = 960
equal_chunks = []
remaining_chunks = []
already_zero_chunks = []
matching_nonzero_chunks = []
unexpected_nonzero_mismatch_chunks = []
for index, offset in enumerate(range(0, len(expected), chunk_size)):
    want = expected[offset:offset + chunk_size]
    have = payload[offset:offset + len(want)]
    if have == want:
        equal_chunks.append(index)
        (matching_nonzero_chunks if any(want) else already_zero_chunks).append(index)
    else:
        remaining_chunks.append(index)
        if any(have):
            unexpected_nonzero_mismatch_chunks.append(index)
assert len(equal_chunks) + len(remaining_chunks) == 1713
program_rent = responses["program_rent"]["result"]
programdata_rent = responses["programdata_rent"]["result"]
assert isinstance(program_rent, int) and isinstance(programdata_rent, int)
prior_fee_quote = json.loads((OUT / "unsigned-simulation-summary.json").read_text())["quoted_fees_lamports"]
incremental_fee_estimate = len(remaining_chunks) * prior_fee_quote["write"] + prior_fee_quote["final"]
incremental_expected_debit = program_rent + incremental_fee_estimate
debit_so_far = 25000000000 - payer["lamports"]
summary = {
    "observed_at_utc": observed, "rpc": RPC, "genesis": GENESIS,
    "context": responses["accounts"]["result"]["context"],
    "payer": PAYER, "payer_balance_lamports": payer["lamports"],
    "program": PROGRAM, "programdata": PROGRAMDATA,
    "program_absent": True, "programdata_absent": True,
    "buffer": BUFFER, "buffer_owner": buffer["owner"], "buffer_executable": False,
    "buffer_space_bytes": buffer["space"], "buffer_payload_bytes": len(payload),
    "buffer_authority": authority, "buffer_lamports": buffer["lamports"],
    "buffer_payload_sha256": hashlib.sha256(payload).hexdigest(),
    "accepted_artifact_sha256": hashlib.sha256(expected).hexdigest(),
    "whole_payload_equals_artifact": payload == expected,
    "upload_progress_basis": "CLI-shaped 960-byte chunk comparison against the accepted ELF; equality counts include untouched all-zero chunks.",
    "maximum_total_chunks": 1713, "equal_chunks": len(equal_chunks),
    "matching_nonzero_chunks": len(matching_nonzero_chunks),
    "already_zero_equal_chunks": len(already_zero_chunks),
    "remaining_write_chunks": len(remaining_chunks),
    "first_remaining_chunk_index": remaining_chunks[0] if remaining_chunks else None,
    "first_remaining_byte_offset": remaining_chunks[0] * chunk_size if remaining_chunks else None,
    "unexpected_nonzero_mismatch_chunks": unexpected_nonzero_mismatch_chunks,
    "matching_payload_bytes": sum(a == b for a, b in zip(payload, expected)),
    "total_payload_bytes": len(expected),
    "program_rent_lamports": program_rent, "programdata_rent_lamports": programdata_rent,
    "buffer_fully_funds_current_programdata_rent": buffer["lamports"] >= programdata_rent,
    "original_payer_balance_lamports": 25000000000,
    "payer_debit_so_far_lamports": debit_so_far,
    "debit_minus_buffer_balance_lamports": debit_so_far - buffer["lamports"],
    "balance_delta_limit": "Residual is consistent with transaction fees only if no unrelated payer activity occurred; it is not a signature-by-signature fee audit.",
    "signed_sent_retried_reclaimed": False,
}
(OUT / "recovery-public-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))
