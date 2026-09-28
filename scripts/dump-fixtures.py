#!/usr/bin/env python3
"""Dump the Raydium mainnet fixtures of the program tests (ARCHITECTURE §1.2, §8.2; M3).

Writes, under ``tests/fixtures/``:

- ``programs/raydium_amm_v4.so``, ``programs/raydium_cpmm.so``: the deployed ELFs (the
  ProgramData account after its 45-byte header, cut at the ELF's own end; every byte after it
  is zero on mainnet and is re-created by the loader in ``tests/litesvm/src/fork.rs``);
- ``accounts/raydium/*.bin``: the raw data of every account the Raydium tests load — the two
  program accounts and their ProgramData headers (T-LATCH), the Fartcoin AMM v4 pool and its
  reversed-order sibling, five CPMM pools (USELESS; a creator-fee pool with the fee on WSOL as
  token 0; a Token-2022 pool; a creator-fee pool with WSOL as token 1; a pool charging the
  creator fee on both tokens), their vaults, mints, LP mints and observations, both vault
  authorities, the AMM v4 ``AmmConfig`` and every CPMM ``AmmConfig``;
- ``manifest.json``: one entry per account (address, owner, lamports, executable, rent epoch,
  space, **slot**, sha256, purpose, decoded fields) and one snapshot record per fetch;
- ``SHA256SUMS``: every fixture file (CI: ``cd tests/fixtures && sha256sum -c SHA256SUMS``).

All venue *state* accounts are read in **one** ``getMultipleAccounts`` call, so pools, vaults
and fee counters are one consistent mainnet state at one slot (pinned in the manifest).
Programs are fetched separately (their deploy slot is in the ProgramData header).

Usage (Python 3.14, standard library only)::

    scripts/dump-fixtures.py            # = check: offline; every file against the manifest,
                                        #   the ProgramData reconstructions, the cross-references
    scripts/dump-fixtures.py fetch      # fetch what is missing (idempotent: a complete, valid
                                        #   fixture set is left byte-for-byte unchanged)
    scripts/dump-fixtures.py fetch --refresh   # re-dump everything at a new slot
    scripts/dump-fixtures.py fetch --rpc URL   # default: api.mainnet-beta, then publicnode

Entries of the manifest this script does not manage (e.g. the Pyth dump) are kept verbatim.
A refresh replaces the pinned bytes: the reader unit tests
(``crates/shortwin-venue-state/src/raydium_{v4,cpmm}.rs``) assert the decoded values of this
dump and move with it (the manifest's ``decoded`` fields list the new ones).
Exit status: 0 ok, 1 a check failed; an RPC or consistency error while fetching exits with its
message.
"""

import argparse
import base64
import datetime as dt
import hashlib
import json
import os
import struct
import sys
import time
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURES = os.path.normpath(os.path.join(HERE, "..", "tests", "fixtures"))
MANIFEST = os.path.join(FIXTURES, "manifest.json")
SUMS = os.path.join(FIXTURES, "SHA256SUMS")
ACCOUNT_DIR = "accounts/raydium"
PROGRAM_DIR = "programs"

RPCS = ["https://api.mainnet-beta.solana.com", "https://solana-rpc.publicnode.com"]

# ---- constants (R-RAY §1.1, §2) ------------------------------------------------------------

LOADER_UPGRADEABLE = "BPFLoaderUpgradeab1e11111111111111111111111"
TOKEN = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
TOKEN_2022 = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
WSOL = "So11111111111111111111111111111111111111112"
SYSTEM = "11111111111111111111111111111111"
AMM_V4 = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"
CPMM = "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C"
AMM_V4_AUTHORITY = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1"
AMM_V4_CONFIG = "9DCxsMizn3H1hprZ7xWe6LDzeUeZBksYFpBWBtSf1PQX"
CPMM_AUTHORITY = "GpMZbSM2GgvTKHJirzeGfMFoaZ8UR2X7F4v8vHTvxFbL"
CPMM_POOL_DISC = bytes.fromhex("f7ede3f5d7c3de46")
CPMM_CONFIG_DISC = bytes.fromhex("daf42168cbcb2b6f")
CPMM_OBSERVATION_LEN = 4075

# (name, program, ProgramData, sha256 of the ELF with trailing zeros trimmed per R-RAY §1.3)
PROGRAMS = [
    (
        "raydium_amm_v4",
        AMM_V4,
        "A7ZG7ByDi8DpzT9Ab7CiXhvgYTJQmaDPJkMDoPitaCQV",
        "d4cee0cd4f78f69d05f5d645fc6d120dff830f231162d35cf2b2dae1df7b2c9f",
    ),
    (
        "raydium_cpmm",
        CPMM,
        "DMawCQzbgNTmbzaESc7o6pvL1KAeetY8zA7jNpzntHhU",
        "bdaff73e7ed9fc75f091d732987d36476d7378d32c47495fe9eef0a930dbf8cc",
    ),
]

# (prefix, pool address, purpose). R-RAY §2.1, §2.4, §6.4, §12.3; ARCH §8.2.
V4_POOLS = [
    (
        "ray_v4_fartcoin",
        "Bzc9NZfMqkXR6fz1DBph7BDf9BroyEf6pnzESP7v5iiw",
        "Fartcoin's main AMM v4 pool (R-RAY §2.1, §6.4; ARCH §2.5 example): WSOL is the coin, "
        "swap fee 25/10,000, status 6; the M3 fork E2E pool and T-RAY-MATH's AMM v4 venue",
    ),
    (
        "ray_v4_fartcoin_rev",
        "9GWMrDgHzWqT5wLZSwKbsFud63q93vEuA5TE6Qt9txVs",
        "Fartcoin's tiny reversed-order AMM v4 pool (R-RAY §2.1; ARCH §8.2): WSOL is the pc, "
        "and need_take_pnl is non-zero on both sides, so reserves = vault - need_take_pnl bite",
    ),
]
CPMM_POOLS = [
    (
        "cpmm_useless",
        "Q2sPHPdUWFMg7M7wwrQKLrn619cAucfRsmhVJffodSp",
        "USELESS CPMM pool (R-RAY §2.4, §6.4): WSOL token 0, config 0 (0.25 %), creator fee "
        "off; the M3 fork E2E pool",
    ),
    (
        "cpmm_creator_fee",
        "FQd8z3Dy6NRxZgx152SUmYL9AyyqAbYCPtSmy4spEgnH",
        "creator-fee CPMM pool (R-RAY §2.4, §3.5 E): WSOL token 0, config 0, creator fee 0.05 % "
        "on token 0 (creator_fee_on 1) -> on the input of buys, on the output of sells; large "
        "uncollected creator counter",
    ),
    (
        "cpmm_token2022",
        "7ZRtFcgfxKcUfdACiWf9abqBs9mVy569VeDLdLxb7Ztx",
        "largest Token-2022 CPMM pool (R-RAY §2.4 `7ZRtFcgf…`): WSOL token 0 (SPL), token 1 "
        "under Token-2022, config 0, creator fee off",
    ),
    (
        "cpmm_wsol_token1",
        "3Cv9uPpaSqQD55oiiegpC5SBm9664kqbJoB5K7YBQfGb",
        "deepest creator-fee CPMM pool with WSOL as token 1 (creator_fee_on 2 = WSOL; config 10: "
        "0.50 % + 0.40 %): the reversed side order and a second config",
    ),
    (
        "cpmm_creator_both",
        "FrZnJAFt8HtvsvERXMLmXTfJTEkRbkwitTKTwS7SzRXf",
        "deepest CPMM pool with WSOL token 0 and creator_fee_on 0 (BothToken: the creator fee is "
        "always on the input), config 0",
    ),
]

# ---- base58 ---------------------------------------------------------------------------------

B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def b58encode(b):
    n = int.from_bytes(b, "big")
    s = ""
    while n:
        n, r = divmod(n, 58)
        s = B58[r] + s
    return "1" * (len(b) - len(b.lstrip(b"\0"))) + s


def b58decode(s):
    n = 0
    for c in s:
        n = n * 58 + B58.index(c)
    body = n.to_bytes((n.bit_length() + 7) // 8, "big") if n else b""
    return b"\0" * (len(s) - len(s.lstrip("1"))) + body


def key_at(d, off):
    return b58encode(d[off : off + 32])


def u64_at(d, off):
    return struct.unpack_from("<Q", d, off)[0]


def u16_at(d, off):
    return struct.unpack_from("<H", d, off)[0]


def sha256(b):
    return hashlib.sha256(b).hexdigest()


# ---- RPC ------------------------------------------------------------------------------------


class Rpc:
    def __init__(self, urls):
        self.urls = urls

    def call(self, method, params):
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
        last = None
        for url in self.urls:
            for attempt in range(6):
                req = urllib.request.Request(
                    url, data=body, headers={"Content-Type": "application/json"}
                )
                try:
                    with urllib.request.urlopen(req, timeout=120) as r:
                        out = json.loads(r.read())
                except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as e:
                    last = e
                    time.sleep(2 * (attempt + 1))
                    continue
                err = out.get("error")
                if err and err.get("code") in (429, -32429, -32005):
                    last = err
                    time.sleep(3 * (attempt + 1))
                    continue
                if err:
                    raise SystemExit(f"rpc {method} on {url}: {err}")
                self.used = url
                return out["result"]
        raise SystemExit(f"rpc {method} failed on every endpoint: {last}")

    def multiple(self, keys):
        res = self.call("getMultipleAccounts", [keys, {"encoding": "base64"}])
        return res["context"]["slot"], res["value"]


def decode_value(v):
    return {
        "owner": v["owner"],
        "lamports": v["lamports"],
        "executable": v["executable"],
        "rent_epoch": v["rentEpoch"],
        "data": base64.b64decode(v["data"][0]),
    }


# ---- decoders (for the manifest's "decoded" fields and the cross-checks) ---------------------


def decode_token_account(d):
    return {
        "mint": key_at(d, 0),
        "owner": key_at(d, 32),
        "amount": u64_at(d, 64),
        "delegate": key_at(d, 76) if struct.unpack_from("<I", d, 72)[0] else None,
        "state": d[108],
        "is_native": u64_at(d, 113) if struct.unpack_from("<I", d, 109)[0] else None,
    }


def tlv_types(d):
    """Token-2022 extension types of a mint (R-MINT §3.2)."""
    out = []
    if len(d) <= 166:
        return out
    off = 166
    while off + 4 <= len(d):
        t, n = struct.unpack_from("<HH", d, off)
        if t == 0:
            break
        out.append(t)
        off += 4 + n
    return out


def decode_mint(d, owner):
    out = {
        "program": owner,
        "supply": u64_at(d, 36),
        "decimals": d[44],
        "mint_authority": key_at(d, 4) if struct.unpack_from("<I", d, 0)[0] else None,
        "freeze_authority": key_at(d, 50) if struct.unpack_from("<I", d, 46)[0] else None,
    }
    if owner == TOKEN_2022:
        out["extension_types"] = tlv_types(d)
    return out


def decode_amm_info(d):
    return {
        "status": u64_at(d, 0),
        "nonce": u64_at(d, 8),
        "coin_decimals": u64_at(d, 32),
        "pc_decimals": u64_at(d, 40),
        "pnl_numerator": u64_at(d, 160),
        "pnl_denominator": u64_at(d, 168),
        "swap_fee_numerator": u64_at(d, 176),
        "swap_fee_denominator": u64_at(d, 184),
        "need_take_pnl_coin": u64_at(d, 192),
        "need_take_pnl_pc": u64_at(d, 200),
        "pool_open_time": u64_at(d, 224),
        "coin_vault": key_at(d, 336),
        "pc_vault": key_at(d, 368),
        "coin_vault_mint": key_at(d, 400),
        "pc_vault_mint": key_at(d, 432),
        "lp_mint": key_at(d, 464),
        "amm_owner": key_at(d, 688),
        "lp_amount": u64_at(d, 720),
    }


def decode_pool_state(d):
    return {
        "amm_config": key_at(d, 8),
        "pool_creator": key_at(d, 40),
        "token_0_vault": key_at(d, 72),
        "token_1_vault": key_at(d, 104),
        "lp_mint": key_at(d, 136),
        "token_0_mint": key_at(d, 168),
        "token_1_mint": key_at(d, 200),
        "token_0_program": key_at(d, 232),
        "token_1_program": key_at(d, 264),
        "observation_key": key_at(d, 296),
        "auth_bump": d[328],
        "status": d[329],
        "mint_0_decimals": d[331],
        "mint_1_decimals": d[332],
        "lp_supply": u64_at(d, 333),
        "protocol_fees_token_0": u64_at(d, 341),
        "protocol_fees_token_1": u64_at(d, 349),
        "fund_fees_token_0": u64_at(d, 357),
        "fund_fees_token_1": u64_at(d, 365),
        "open_time": u64_at(d, 373),
        "recent_epoch": u64_at(d, 381),
        "creator_fee_on": d[389],
        "enable_creator_fee": d[390],
        "creator_fees_token_0": u64_at(d, 397),
        "creator_fees_token_1": u64_at(d, 405),
    }


def decode_amm_config(d):
    return {
        "bump": d[8],
        "disable_create_pool": d[9],
        "index": u16_at(d, 10),
        "trade_fee_rate": u64_at(d, 12),
        "protocol_fee_rate": u64_at(d, 20),
        "fund_fee_rate": u64_at(d, 28),
        "create_pool_fee": u64_at(d, 36),
        "creator_fee_rate": u64_at(d, 108),
    }


def elf_len(elf):
    """The ELF file's own length: the furthest byte any header, section or segment names."""
    if elf[:4] != b"\x7fELF" or elf[4] != 2 or elf[5] != 1:
        raise SystemExit("ProgramData does not hold a 64-bit little-endian ELF")
    (e_phoff, e_shoff) = struct.unpack_from("<QQ", elf, 0x20)
    (e_phentsize, e_phnum, e_shentsize, e_shnum) = struct.unpack_from("<HHHH", elf, 0x36)
    end = max(64, e_phoff + e_phnum * e_phentsize, e_shoff + e_shnum * e_shentsize)
    for i in range(e_phnum):
        o = e_phoff + i * e_phentsize
        p_offset, _, _, p_filesz = struct.unpack_from("<QQQQ", elf, o + 8)
        end = max(end, p_offset + p_filesz)
    for i in range(e_shnum):
        o = e_shoff + i * e_shentsize
        sh_type = struct.unpack_from("<I", elf, o + 4)[0]
        sh_offset, sh_size = struct.unpack_from("<QQ", elf, o + 24)
        if sh_type != 8:  # SHT_NOBITS occupies no file bytes
            end = max(end, sh_offset + sh_size)
    return end


# ---- manifest -------------------------------------------------------------------------------


def load_manifest():
    with open(MANIFEST) as f:
        return json.load(f)


def managed(entry):
    return entry.get("group") in ("raydium_programs", "raydium_state")


def write_manifest(m):
    text = json.dumps(m, indent=2, ensure_ascii=False) + "\n"
    with open(MANIFEST, "w") as f:
        f.write(text)


def write_sums(paths):
    """SHA256SUMS: the unmanaged lines as they are, then every managed file, sorted."""
    keep = []
    if os.path.exists(SUMS):
        with open(SUMS) as f:
            for line in f:
                p = line.rstrip("\n").split("  ", 1)[-1]
                if not (p.startswith(ACCOUNT_DIR + "/") or p.startswith(PROGRAM_DIR + "/")):
                    keep.append(line.rstrip("\n"))
    lines = keep + [f"{sha256(read_file(p))}  {p}" for p in sorted(paths)]
    with open(SUMS, "w") as f:
        f.write("\n".join(lines) + "\n")


def read_file(rel):
    with open(os.path.join(FIXTURES, rel), "rb") as f:
        return f.read()


def write_file(rel, data):
    path = os.path.join(FIXTURES, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)


def now_utc():
    return dt.datetime.now(dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


# ---- fetch ----------------------------------------------------------------------------------


def fetch_programs(rpc):
    """Program accounts, ProgramData headers and ELFs; returns (entries, files, snapshot)."""
    entries, files = [], []
    slot_before = rpc.call("getSlot", [])
    for name, program, program_data, trimmed_expected in PROGRAMS:
        res = rpc.call("getAccountInfo", [program, {"encoding": "base64"}])
        prog = decode_value(res["value"])
        pd_res = rpc.call("getAccountInfo", [program_data, {"encoding": "base64"}])
        pd = decode_value(pd_res["value"])
        d = pd["data"]
        if prog["owner"] != LOADER_UPGRADEABLE or not prog["executable"]:
            raise SystemExit(f"{program}: not an upgradeable program")
        if prog["data"][:4] != b"\x02\0\0\0" or b58encode(prog["data"][4:36]) != program_data:
            raise SystemExit(f"{program}: program account does not name {program_data}")
        if pd["owner"] != LOADER_UPGRADEABLE or d[:4] != b"\x03\0\0\0":
            raise SystemExit(f"{program_data}: not a ProgramData account")
        elf = d[45:]
        n = elf_len(elf)
        if any(elf[n:]):
            raise SystemExit(f"{program_data}: non-zero bytes after the ELF's end ({n})")
        trimmed = elf.rstrip(b"\0")
        deploy_slot = u64_at(d, 4)
        authority = key_at(d, 13) if d[12] == 1 else None
        prog_file = f"{ACCOUNT_DIR}/{name}_program.bin"
        header_file = f"{ACCOUNT_DIR}/{name}_program_data_header.bin"
        elf_file = f"{PROGRAM_DIR}/{name}.so"
        write_file(prog_file, prog["data"])
        write_file(header_file, d[:45])
        write_file(elf_file, elf[:n])
        files += [prog_file, header_file, elf_file]
        entries.append(
            {
                "name": f"{name}_program",
                "group": "raydium_programs",
                "purpose": f"the {name} program account (upgradeable loader; names its ProgramData). "
                "tests/litesvm/src/fork.rs deploys the ELF at the mainnet address with it",
                "address": program,
                "owner": prog["owner"],
                "lamports": prog["lamports"],
                "executable": True,
                "rent_epoch": prog["rent_epoch"],
                "space": len(prog["data"]),
                "slot": res["context"]["slot"],
                "sha256": sha256(prog["data"]),
                "file": prog_file,
                "decoded": {"program_data": program_data},
            }
        )
        entries.append(
            {
                "name": f"{name}_program_data",
                "group": "raydium_programs",
                "purpose": f"{name} ProgramData: the deployed ELF (T-RAY-*) and the header the "
                "venue-upgrade latch reads (T-LATCH on the dumped ProgramData, ARCH §8.2, §9 M3). "
                "Stored as header ‖ ELF; the account is header ‖ ELF ‖ zeros up to `space`, whose "
                "sha256 is `sha256` (fork.rs rebuilds and checks it)",
                "address": program_data,
                "owner": pd["owner"],
                "lamports": pd["lamports"],
                "executable": False,
                "rent_epoch": pd["rent_epoch"],
                "space": len(d),
                "slot": pd_res["context"]["slot"],
                "sha256": sha256(d),
                "files": {"header": header_file, "elf": elf_file},
                "elf_len": n,
                "elf_sha256": sha256(elf[:n]),
                "elf_trimmed_len": len(trimmed),
                "elf_trimmed_sha256": sha256(trimmed),
                "elf_trimmed_sha256_expected": trimmed_expected,
                "decoded": {
                    "deploy_slot": deploy_slot,
                    "upgrade_authority": authority,
                    "e_flags": struct.unpack_from("<I", elf, 0x30)[0],
                },
            }
        )
        if sha256(trimmed) != trimmed_expected:
            print(
                f"WARNING: {name}: trimmed ELF sha256 {sha256(trimmed)} differs from R-RAY §1.3 "
                f"{trimmed_expected} (upgraded since the research?)",
                file=sys.stderr,
            )
    snapshot = {
        "group": "raydium_programs",
        "slot": slot_before,
        "fetched_at": now_utc(),
        "rpc": rpc.used,
        "method": "getAccountInfo (base64) of each program and ProgramData account; "
        "`solana program dump -um <program> out.so` gives the same ELF bytes followed by the "
        "zero padding",
    }
    return entries, files, snapshot


def fetch_state(rpc):
    """Every venue-state account in one getMultipleAccounts call (one slot)."""
    # 1. The pools, to learn their derived accounts.
    roots = [a for _, a, _ in V4_POOLS] + [a for _, a, _ in CPMM_POOLS]
    _, vals = rpc.multiple(roots)
    derived = {}
    for (prefix, addr, _), v in zip(V4_POOLS + CPMM_POOLS, vals):
        if v is None:
            raise SystemExit(f"pool {addr} does not exist")
        d = decode_value(v)["data"]
        derived[addr] = decode_amm_info(d) if v["owner"] == AMM_V4 else decode_pool_state(d)
    # 2. Every CPMM AmmConfig (the 21 live configs, R-RAY §2.5).
    cfgs = rpc.call(
        "getProgramAccounts",
        [
            CPMM,
            {
                "encoding": "base64",
                "dataSlice": {"offset": 10, "length": 2},
                "filters": [
                    {"dataSize": 236},
                    {"memcmp": {"offset": 0, "bytes": b58encode(CPMM_CONFIG_DISC)}},
                ],
            },
        ],
    )
    configs = sorted(
        (struct.unpack("<H", base64.b64decode(c["account"]["data"][0]))[0], c["pubkey"])
        for c in cfgs
    )

    # 3. The account list, with names and purposes (first name wins for shared accounts).
    plan = []  # (name, address, purpose)
    seen = {}

    def add(name, addr, purpose):
        if addr in seen:
            return
        seen[addr] = name
        plan.append((name, addr, purpose))

    add("mint_wsol", WSOL, "SPL Token native mint (WSOL): the SOL side of every Raydium pool")
    add("ray_v4_authority", AMM_V4_AUTHORITY,
        "AMM v4 vault / LP authority `create_program_address([\"amm authority\", 254])` "
        "(a system account; swaps pass it read-only)")
    add("ray_v4_amm_config", AMM_V4_CONFIG,
        "AMM v4 AmmConfig (pnl_owner, create_pool_fee; R-RAY §1.1): not read by swaps or by "
        "the route predicates, kept for admin-action tests")
    add("cpmm_authority", CPMM_AUTHORITY,
        "CPMM vault / LP authority PDA [\"vault_and_lp_mint_auth_seed\"] (a system account)")
    for prefix, addr, purpose in V4_POOLS:
        f = derived[addr]
        sol_coin = f["coin_vault_mint"] == WSOL
        tok = f["pc_vault_mint"] if sol_coin else f["coin_vault_mint"]
        add(f"{prefix}_pool", addr, f"AmmInfo. {purpose}")
        add(f"{prefix}_coin_vault", f["coin_vault"],
            f"coin vault of {prefix} ({'WSOL' if sol_coin else 'token'}); reserves = amount - need_take_pnl_coin")
        add(f"{prefix}_pc_vault", f["pc_vault"],
            f"pc vault of {prefix} ({'token' if sol_coin else 'WSOL'}); reserves = amount - need_take_pnl_pc")
        add("mint_fartcoin" if prefix.startswith("ray_v4_fartcoin") else f"{prefix}_token_mint", tok,
            "the token mint of the AMM v4 Fartcoin pools (SPL Token)")
        add(f"{prefix}_lp_mint", f["lp_mint"], f"LP mint of {prefix} (supply for the locked share λ)")
    for prefix, addr, purpose in CPMM_POOLS:
        f = derived[addr]
        add(f"{prefix}_pool", addr, f"PoolState. {purpose}")
        add(f"{prefix}_token_0_vault", f["token_0_vault"],
            f"token 0 vault of {prefix}; reserves = amount - (protocol + fund + creator)_0")
        add(f"{prefix}_token_1_vault", f["token_1_vault"],
            f"token 1 vault of {prefix}; reserves = amount - (protocol + fund + creator)_1")
        for side in ("token_0_mint", "token_1_mint"):
            if f[side] != WSOL:
                add(f"{prefix}_token_mint", f[side], f"the token mint of {prefix}")
        add(f"{prefix}_lp_mint", f["lp_mint"], f"LP mint of {prefix} (supply for the locked share λ)")
        add(f"{prefix}_observation", f["observation_key"],
            f"ObservationState of {prefix} (written by every swap; mandatory swap account)")
    for index, addr in configs:
        add(f"cpmm_config_{index:02}", addr,
            f"CPMM AmmConfig index {index} (R-RAY §2.5): fee variants for T-RAY-MATH / T-RAY-FEE")
    if len(plan) > 100:
        raise SystemExit(f"{len(plan)} accounts do not fit one getMultipleAccounts call")

    # 4. One call, one slot.
    slot, vals = rpc.multiple([a for _, a, _ in plan])
    block_time = None
    try:
        block_time = rpc.call("getBlockTime", [slot])
    except SystemExit as e:
        print(f"WARNING: getBlockTime({slot}): {e}", file=sys.stderr)
    entries, files = [], []
    for (name, addr, purpose), v in zip(plan, vals):
        if v is None:
            raise SystemExit(f"{name} {addr} does not exist at slot {slot}")
        a = decode_value(v)
        d = a["data"]
        decoded = None
        if a["owner"] == AMM_V4 and len(d) == 752:
            decoded = decode_amm_info(d)
        elif a["owner"] == CPMM and d[:8] == CPMM_POOL_DISC:
            decoded = decode_pool_state(d)
        elif a["owner"] == CPMM and d[:8] == CPMM_CONFIG_DISC:
            decoded = decode_amm_config(d)
        elif (a["owner"] in (TOKEN, TOKEN_2022) and len(d) == 82) or (
            a["owner"] == TOKEN_2022 and len(d) > 165 and d[165] == 1
        ):
            decoded = decode_mint(d, a["owner"])
        elif a["owner"] in (TOKEN, TOKEN_2022) and len(d) >= 165:
            decoded = decode_token_account(d)
        elif a["owner"] == CPMM and len(d) == CPMM_OBSERVATION_LEN:
            decoded = {"pool_id": key_at(d, 11)}
        # The pools must still name what step 1 derived (all these keys are immutable).
        if addr in derived:
            for k, want in derived[addr].items():
                if k.endswith(("vault", "mint", "_key", "config", "program")) and decoded[k] != want:
                    raise SystemExit(f"{name}: {k} changed between the two reads")
        rel = f"{ACCOUNT_DIR}/{name}.bin"
        write_file(rel, d)
        files.append(rel)
        e = {
            "name": name,
            "group": "raydium_state",
            "purpose": purpose,
            "address": addr,
            "owner": a["owner"],
            "lamports": a["lamports"],
            "executable": a["executable"],
            "rent_epoch": a["rent_epoch"],
            "space": len(d),
            "slot": slot,
            "sha256": sha256(d),
            "file": rel,
        }
        if decoded is not None:
            e["decoded"] = decoded
        entries.append(e)
    snapshot = {
        "group": "raydium_state",
        "slot": slot,
        "block_time": block_time,
        "block_time_utc": dt.datetime.fromtimestamp(block_time, dt.UTC).strftime(
            "%Y-%m-%dT%H:%M:%SZ") if block_time else None,
        "fetched_at": now_utc(),
        "rpc": rpc.used,
        "method": "one getMultipleAccounts call (base64) over every account below, so all of "
        "them are the mainnet state at `slot`; the CPMM configs were listed first with "
        "getProgramAccounts(CPMM, dataSize 236)",
    }
    return entries, files, snapshot


def cmd_fetch(args):
    m = load_manifest()
    have = {s["group"] for s in m.get("snapshots", [])}
    groups = ["raydium_programs", "raydium_state"]
    todo = [g for g in groups if args.refresh or g not in have or check(m, only=g)]
    if not todo:
        print("fixtures complete and valid; nothing to fetch (use --refresh to re-dump)")
        return 0
    rpc = Rpc([args.rpc] if args.rpc else RPCS)
    for g in todo:
        entries, files, snap = fetch_programs(rpc) if g == "raydium_programs" else fetch_state(rpc)
        m["accounts"] = [e for e in m["accounts"] if e.get("group") != g] + entries
        m["snapshots"] = [s for s in m.get("snapshots", []) if s["group"] != g] + [snap]
        print(f"{g}: {len(entries)} accounts at slot {snap['slot']}")
    m["description"] = (
        "Mainnet accounts, programs and transactions dumped for the program tests (ARCHITECTURE "
        "§1.2, §8.2). Each entry records how it was fetched; SHA256SUMS pins the bytes (CI: cd "
        "tests/fixtures && sha256sum -c SHA256SUMS). The Raydium entries (groups raydium_programs "
        "and raydium_state) are written by scripts/dump-fixtures.py; `snapshots` records the "
        "slot and RPC of each group."
    )
    write_manifest(m)
    managed_files = []
    for e in m["accounts"]:
        if managed(e):
            managed_files += [e["file"]] if "file" in e else list(e["files"].values())
    write_sums(managed_files)
    problems = check(load_manifest())
    for p in problems:
        print("FAIL:", p, file=sys.stderr)
    return 1 if problems else 0


# ---- check ----------------------------------------------------------------------------------


def check(m, only=None):
    """Offline checks; returns a list of problems (empty = ok)."""
    problems = []
    entries = [e for e in m["accounts"] if managed(e) and (only is None or e["group"] == only)]
    if not entries:
        return [f"no entries for {only or 'the Raydium groups'}"]
    by_addr = {e["address"]: e for e in m["accounts"] if managed(e)}
    sums = {}
    if os.path.exists(SUMS):
        with open(SUMS) as f:
            for line in f:
                h, p = line.rstrip("\n").split("  ", 1)
                sums[p] = h
    for e in entries:
        try:
            if "file" in e:
                d = read_file(e["file"])
                if sha256(d) != e["sha256"] or len(d) != e["space"]:
                    problems.append(f"{e['file']}: bytes differ from the manifest")
                if sums.get(e["file"]) != sha256(d):
                    problems.append(f"{e['file']}: missing or wrong in SHA256SUMS")
            else:
                header = read_file(e["files"]["header"])
                elf = read_file(e["files"]["elf"])
                full = header + elf + bytes(e["space"] - len(header) - len(elf))
                if sha256(full) != e["sha256"]:
                    problems.append(f"{e['name']}: header ‖ ELF ‖ zeros != the mainnet account")
                if sha256(elf) != e["elf_sha256"] or len(elf) != e["elf_len"]:
                    problems.append(f"{e['files']['elf']}: ELF differs from the manifest")
                if sha256(elf.rstrip(b"\0")) != e["elf_trimmed_sha256_expected"]:
                    problems.append(f"{e['name']}: trimmed ELF hash != R-RAY §1.3")
                if u64_at(header, 4) != e["decoded"]["deploy_slot"]:
                    problems.append(f"{e['name']}: deploy slot differs from the header")
                for p in e["files"].values():
                    if sums.get(p) != sha256(read_file(p)):
                        problems.append(f"{p}: missing or wrong in SHA256SUMS")
        except FileNotFoundError as ex:
            problems.append(f"{e['name']}: {ex}")
    if only in (None, "raydium_state"):
        # Cross-references: every account a pool names is in the fixture set with the
        # owner the swap requires.
        for e in entries:
            dec = e.get("decoded") or {}
            if e["owner"] == AMM_V4 and "coin_vault" in dec:
                for k in ("coin_vault", "pc_vault", "coin_vault_mint", "pc_vault_mint", "lp_mint"):
                    if dec[k] not in by_addr:
                        problems.append(f"{e['name']}: {k} {dec[k]} is not a fixture")
                for k, mk in (("coin_vault", "coin_vault_mint"), ("pc_vault", "pc_vault_mint")):
                    v = by_addr.get(dec[k], {}).get("decoded", {})
                    if v.get("mint") != dec[mk] or v.get("owner") != AMM_V4_AUTHORITY:
                        problems.append(f"{e['name']}: {k} mint/authority mismatch")
            if e["owner"] == CPMM and "token_0_vault" in dec:
                for k in ("amm_config", "token_0_vault", "token_1_vault", "token_0_mint",
                          "token_1_mint", "lp_mint", "observation_key"):
                    if dec[k] not in by_addr:
                        problems.append(f"{e['name']}: {k} {dec[k]} is not a fixture")
                for i in "01":
                    v = by_addr.get(dec[f"token_{i}_vault"], {})
                    if (v.get("decoded", {}).get("mint") != dec[f"token_{i}_mint"]
                            or v.get("decoded", {}).get("owner") != CPMM_AUTHORITY
                            or v.get("owner") != dec[f"token_{i}_program"]):
                        problems.append(f"{e['name']}: token_{i} vault mint/authority/program mismatch")
        slots = {e["slot"] for e in entries if e["group"] == "raydium_state"}
        if len(slots) > 1:
            problems.append(f"raydium_state spans several slots: {sorted(slots)}")
    return problems


def cmd_check(_args):
    problems = check(load_manifest())
    for p in problems:
        print("FAIL:", p, file=sys.stderr)
    if not problems:
        n = sum(1 for e in load_manifest()["accounts"] if managed(e))
        print(f"ok: {n} Raydium fixture entries match their files, SHA256SUMS and each other")
    return 1 if problems else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd")
    sub.add_parser("check", help="offline verification (default)")
    f = sub.add_parser("fetch", help="dump from mainnet what is missing or invalid")
    f.add_argument("--refresh", action="store_true", help="re-dump every group at a new slot")
    f.add_argument("--rpc", help="JSON-RPC endpoint (default: mainnet-beta, then publicnode)")
    args = ap.parse_args()
    if args.cmd == "fetch":
        return cmd_fetch(args)
    return cmd_check(args)


if __name__ == "__main__":
    sys.exit(main())
