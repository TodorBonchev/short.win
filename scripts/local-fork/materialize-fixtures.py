#!/usr/bin/env python3
"""Write hash-checked historical mainnet accounts for a localhost-only validator.

Offline materialization makes no network requests and reads no signer files.
--verify-local-rpc performs read-only checks against 127.0.0.1:8899. The explicitly
synthetic accounts are Jupiter's empty event-authority placeholder and three
public demo token allocations. Program metadata is preserved byte for byte.
"""

import argparse
import base64
import gzip
import hashlib
import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path
import urllib.request


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests/fixtures"
LOADER = "BPFLoaderUpgradeab1e11111111111111111111111"
JUPITER = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"
JUPITER_DATA = "4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT"
EVENT_AUTHORITY = "D8cy77BBepLMngZx6ZukaTff5hCt1HrWyKk3Hnd9oitf"
LOOPBACK_SHIM = r"""/* Agave test-validator 4.1 binds RPC to wildcard addresses.
 * Restrict only this process's wildcard sockets to localhost via LD_PRELOAD. */
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <dlfcn.h>
#include <netinet/in.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static int (*original_bind)(int, const struct sockaddr *, socklen_t);
__attribute__((constructor)) static void initialize_bind(void) {
    original_bind = dlsym(RTLD_NEXT, "bind");
    if (!original_bind) _exit(127);
}

int bind(int fd, const struct sockaddr *address, socklen_t length) {
    if (address && address->sa_family == AF_INET && length >= sizeof(struct sockaddr_in)) {
        struct sockaddr_in local;
        memcpy(&local, address, sizeof(local));
        if (local.sin_addr.s_addr == htonl(INADDR_ANY)) {
            local.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
            return original_bind(fd, (const struct sockaddr *)&local, sizeof(local));
        }
    }
    if (address && address->sa_family == AF_INET6 && length >= sizeof(struct sockaddr_in6)) {
        struct sockaddr_in6 local;
        memcpy(&local, address, sizeof(local));
        if (IN6_IS_ADDR_UNSPECIFIED(&local.sin6_addr)) {
            local.sin6_addr = in6addr_loopback;
            return original_bind(fd, (const struct sockaddr *)&local, sizeof(local));
        }
    }
    return original_bind(fd, address, length);
}
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def checked(path, expected):
    data = path.read_bytes()
    if sha(data) != expected:
        raise ValueError(f"Hash mismatch: {path}")
    return data


def public_key_bytes(address):
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    value = 0
    for char in address:
        value = value * 58 + alphabet.index(char)
    result = bytes(len(address) - len(address.lstrip("1")))
    result += value.to_bytes((value.bit_length() + 7) // 8, "big")
    if len(result) != 32:
        raise ValueError(f"Invalid public address: {address}")
    return result


def public_key_string(data):
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    value = int.from_bytes(data, "big")
    result = ""
    while value:
        value, digit = divmod(value, 58)
        result = alphabet[digit] + result
    return "1" * (len(data) - len(data.lstrip(b"\0"))) + result


def verify_local_rpc(out):
    """Pin a fresh genesis, or validate executable identity when resuming it."""
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, req, fp, code, msg, headers, newurl):
            raise ValueError("Local RPC redirects are forbidden")

    opener = urllib.request.build_opener(NoRedirect)

    def rpc(method, params=None):
        request = urllib.request.Request(
            "http://127.0.0.1:8899",
            data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method,
                             "params": params or []}).encode(),
            headers={"Content-Type": "application/json"},
        )
        with opener.open(request, timeout=10) as response:
            result = json.load(response)
        if "error" in result:
            raise ValueError(f"Local RPC {method}: {result['error']}")
        return result["result"]

    def account(address):
        value = rpc("getAccountInfo", [address, {"encoding": "base64", "commitment": "confirmed"}])["value"]
        if value is None:
            raise ValueError(f"Missing local account: {address}")
        return value, base64.b64decode(value["data"][0], validate=True)

    if rpc("getHealth") != "ok":
        raise ValueError("Local validator is not healthy")
    genesis = rpc("getGenesisHash")
    receipt_path = out / "runtime-verification.json"
    previous = json.loads(receipt_path.read_text()) if receipt_path.exists() else None
    same_genesis = previous is not None and previous.get("genesis_hash") == genesis
    dumps = [json.loads(p.read_text()) for p in sorted((out / "accounts").glob("*.json"))]
    if len(dumps) != 16:
        raise ValueError("Materialize the 16 checked fixture accounts before verification")
    by_address = {dump["pubkey"]: dump["account"] for dump in dumps}
    # These executable accounts must stay exact even after users change pools,
    # vaults and holder balances by sending legitimate local demo transactions.
    executable_addresses = [JUPITER, JUPITER_DATA,
        "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C",
        "DMawCQzbgNTmbzaESc7o6pvL1KAeetY8zA7jNpzntHhU"]
    checked_accounts = []
    addresses = executable_addresses if same_genesis else list(by_address)
    for address in addresses:
        wanted = by_address[address]
        actual, data = account(address)
        changes = {}
        for field in ("lamports", "owner", "executable", "data", "rentEpoch"):
            if actual[field] != wanted[field]:
                if (address != "So11111111111111111111111111111111111111112"
                        or field not in ("lamports", "rentEpoch")):
                    raise ValueError(f"Local fixture differs: {address} {field}")
                changes[field] = {"captured": wanted[field], "validator": actual[field]}
        checked_accounts.append({
            "address": address, "data_len": len(data), "data_sha256": sha(data),
            "owner": actual["owner"], "executable": actual["executable"],
            "validator_normalization": changes,
        })

    own_address = "BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh"
    own_data_address = "H8CxcBVdTmGaebHwoXjMphqRU7vwK2iuDpZvtrK92d9m"
    own, program = account(own_address)
    own_data, data = account(own_data_address)
    authority = "9hSR6S7WPtxmTojgo6GG3k4yDPecgJY292j7xrsUGWBu"
    elf = (ROOT / "target/deploy/shortwin_simple.so").read_bytes()
    if (own["owner"] != LOADER or not own["executable"]
            or program != (2).to_bytes(4, "little") + public_key_bytes(own_data_address)
            or own_data["owner"] != LOADER or own_data["executable"]
            or data[:4] != (3).to_bytes(4, "little") or data[12] != 1
            or public_key_string(data[13:45]) != authority or data[45:] != elf):
        raise ValueError("Local lending program/upgrade authority/ELF mismatch")
    if same_genesis:
        if previous.get("own_elf_sha256") != sha(elf):
            raise ValueError("Current ELF differs from the verified genesis receipt")
        baseline = {record["address"]: record for record in previous["verified_accounts"]}
        for address, wanted in by_address.items():
            record = baseline.get(address, {})
            if (record.get("data_sha256") != sha(base64.b64decode(wanted["data"][0]))
                    or record.get("owner") != wanted["owner"]
                    or record.get("executable") != wanted["executable"]):
                raise ValueError(f"Original genesis verification is missing/invalid: {address}")
        checked_accounts = previous["verified_accounts"]
    _, clock_data = account("SysvarC1ock11111111111111111111111111111111")
    now = datetime.now(timezone.utc).isoformat()
    receipt = {
        "health": "ok", "genesis_hash": genesis,
        "slot": rpc("getSlot", [{"commitment": "confirmed"}]),
        "snapshot_rpc_slot": previous.get("snapshot_rpc_slot") if same_genesis else rpc("getSlot", [{"commitment": "confirmed"}]),
        "own_programdata": own_data_address, "own_upgrade_authority": authority,
        "own_elf_sha256": sha(elf), "jupiter_deploy_slot": 459_479_624,
        "jupiter_elf_sha256": "5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b",
        "clock": {"slot": int.from_bytes(clock_data[:8], "little"),
                  "unix_timestamp": int.from_bytes(clock_data[32:40], "little", signed=True)},
        "verification_mode": "same-genesis resume" if same_genesis else "fresh genesis",
        "checked_at": now,
        "initial_verified_at": previous.get("initial_verified_at", "pre-bootstrap receipt") if same_genesis else now,
        "verified_accounts": checked_accounts,
    }
    temporary = receipt_path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(receipt, indent=2) + "\n")
    temporary.replace(receipt_path)
    print(json.dumps({key: receipt[key] for key in
                      ("health", "genesis_hash", "slot", "verification_mode")}, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "target/local-fork")
    parser.add_argument("--loopback-shim", action="store_true",
                        help="Build the process-local bind shim needed by Agave 4.1 RPC")
    parser.add_argument("--verify-local-rpc", action="store_true",
                        help="Read only: verify the local running validator and pin its genesis")
    args = parser.parse_args()
    out = args.out.resolve()
    if not out.is_relative_to(ROOT / "target/local-fork"):
        raise ValueError("Output must remain within target/local-fork")
    if args.verify_local_rpc:
        verify_local_rpc(out)
        return

    manifest = json.loads(checked(
        FIXTURES / "manifest.json",
        "b3bc9cba8fe93f9ffe62089a486ce8e4b448bb5e0002dfd86f0d5714dbc2ee28",
    ))
    corpus_hashes = dict(
        (name.strip(), digest)
        for digest, name in (
            line.split(maxsplit=1)
            for line in (FIXTURES / "SHA256SUMS").read_text().splitlines()
            if line.strip()
        )
    )
    records = []
    dumps = {}

    def add(name, address, account, data, source):
        account = dict(account)
        account["data"] = [base64.b64encode(data).decode("ascii"), "base64"]
        dumps[f"{address}.json"] = {"pubkey": address, "account": account}
        records.append({
            "name": name, "address": address, "source": source,
            "owner": account["owner"], "lamports": account["lamports"],
            "executable": account["executable"], "rentEpoch": account["rentEpoch"],
            "data_len": len(data), "data_sha256": sha(data),
        })

    for entry in manifest["accounts"]:
        if "file" in entry:
            data = checked(FIXTURES / entry["file"], entry["sha256"])
        else:
            header = entry["files"]["header"]
            data = checked(FIXTURES / header, corpus_hashes[header])
            data += checked(FIXTURES / entry["files"]["elf"], entry["elf_sha256"])
            if len(data) > entry["space"]:
                raise ValueError(f"ProgramData exceeds captured space: {entry['name']}")
            data += bytes(entry["space"] - len(data))
        if len(data) != entry["space"] or sha(data) != entry["sha256"]:
            raise ValueError(f"Reconstructed account mismatch: {entry['name']}")
        add(entry["name"], entry["address"], {
            "lamports": entry["lamports"], "owner": entry["owner"],
            "executable": entry["executable"], "rentEpoch": entry["rent_epoch"],
        }, data, f"historical mainnet snapshot slot {entry['slot']}")

    capture = checked(
        FIXTURES / "jupiter-2026-10-27/getMultipleAccounts.json.gz",
        "0df3a42a4915ab894f8bf4ab98c8adc9f6291899494bd70eef74a84a61a1aa60",
    )
    raw = gzip.decompress(capture)
    if sha(raw) != "c611ab92f91fd6bf4ce415054a1e72cad3a3abe1bef5c2bb22f1e574d7a3c3ff":
        raise ValueError("Jupiter raw RPC hash mismatch")
    rpc = json.loads(raw)
    if rpc["result"]["context"]["slot"] != 461_997_164:
        raise ValueError("Jupiter snapshot slot mismatch")
    values = rpc["result"]["value"]
    if len(values) != 3:
        raise ValueError("Unexpected Jupiter capture account count")
    expected = [
        ("jupiter_program", JUPITER, 36, True,
         "77969e6c064ab31f68250951941ad20690dfc23e97870e63006ccdad39c8a47e"),
        ("jupiter_programdata", JUPITER_DATA, 2_892_269, False,
         "21e28c59e3b0a5536e48d31a1ca7346514a2bc61d70dc1388c920d7e3be35c1b"),
    ]
    for value, (name, address, size, executable, digest) in zip(values, expected):
        data = base64.b64decode(value["data"][0], validate=True)
        if (value["owner"] != LOADER or value["executable"] != executable
                or len(data) != size or sha(data) != digest):
            raise ValueError(f"Jupiter account mismatch: {name}")
        if name == "jupiter_programdata":
            if int.from_bytes(data[4:12], "little") != 459_479_624:
                raise ValueError("Jupiter deployment slot mismatch")
            if sha(data[45:].rstrip(b"\0")) != "5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b":
                raise ValueError("Jupiter ELF hash mismatch")
        add(name, address, {
            k: value[k] for k in ("lamports", "owner", "executable", "rentEpoch")
        }, data, "historical mainnet snapshot slot 461997164")

    add("jupiter_event_authority", EVENT_AUTHORITY, {
        "lamports": 1, "owner": "11111111111111111111111111111111",
        "executable": False, "rentEpoch": 0,
    }, b"", "synthetic empty event authority, matching tests/runtime/src/jupiter.rs")

    # Public, deterministic demo allocations. These do not represent captured
    # token holders or historical balances, and never modify any snapshot bytes.
    for name, address, owner in [
        ("demo_user_base_tokens", "DPMrEubpoCHgKHtEmGGZ8Xi72AHxJf29Bw9ruDYZSsYw",
         "3wpYnGqceZ8DzN3guiTd9rrYkWTwTHCChBSuo6cvkXTG"),
        ("demo_lender_base_tokens", "5uk3cPJ5Gra4JuYTrebjXzhJrmAF8FZcq5MKHKiQQyDg",
         "FR5pWwinRBn35GNhg7bsvw8Q13kRept2pm561DwZCQzT"),
        ("demo_quote_base_tokens", "4Yk9HoDSfJv9QcmJbLcXdWVgS7nfvdUqiVcvbSu8VBru",
         "GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB"),
    ]:
        data = bytearray(165)
        data[:32] = public_key_bytes("Dz9mQ9NzkBcCsuGPFJ3r1bS4wgqKMHBPiVuniW8Mbonk")
        data[32:64] = public_key_bytes(owner)
        data[64:72] = (10_000_000_000_000).to_bytes(8, "little")
        data[108] = 1  # SPL Token AccountState::Initialized; no optional authorities.
        add(name, address, {
            "lamports": 10_000_000, "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "executable": False, "rentEpoch": 0,
        }, bytes(data), "SYNTHETIC demo token allocation: 10000000000000 raw units; public test owner")

    accounts = out / "accounts"
    accounts.mkdir(parents=True, exist_ok=True)
    unexpected = {p.name for p in accounts.iterdir()} - dumps.keys()
    if unexpected:
        raise ValueError(f"Unexpected files in account directory: {sorted(unexpected)}")
    for filename, value in dumps.items():
        (accounts / filename).write_text(json.dumps(value, separators=(",", ":")) + "\n")
    own_elf = ROOT / "target/deploy/shortwin_simple.so"
    receipt = {
        "scope": "Local-only historical mainnet snapshot fork; no network access or signing",
        "execution_slot": 461_997_165,
        "own_program_id": "BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh",
        "own_program_sha256": sha(own_elf.read_bytes()),
        "jupiter_deploy_slot": 459_479_624,
        "accounts": records,
    }
    (out / "fixtures.receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    if args.loopback_shim:
        source = out / "bind-loopback.c"
        source.write_text(LOOPBACK_SHIM)
        compiled = out / "bind-loopback.so.tmp"
        subprocess.run(["cc", "-shared", "-fPIC", "-O2", "-Wall", "-Wextra",
                        "-o", str(compiled), str(source), "-ldl"], check=True)
        compiled.replace(out / "bind-loopback.so")
    print(json.dumps({"accounts": len(dumps), "directory": str(accounts),
                      "receipt": str(out / "fixtures.receipt.json")}, indent=2))


if __name__ == "__main__":
    main()
