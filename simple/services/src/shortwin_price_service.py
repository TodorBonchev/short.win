#!/usr/bin/env python3
"""Trusted exact-quantity Jupiter observations; no wallet or transaction submission.

All amounts are raw integer units. This is the explicitly trusted price source,
not an independent oracle. SQLite history is local operator evidence.
"""
from __future__ import annotations

import argparse
import base64
from dataclasses import asdict, dataclass
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import sys
import time
from typing import Callable
import urllib.error
import urllib.parse
import urllib.request
import uuid

WSOL = "So11111111111111111111111111111111111111112"
BUILD_URL = "https://api.jup.ag/swap/v2/build"
JUPITER_PROGRAM = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"
REQUEST_ROUTE_ACCOUNT_BUDGET = 16
FALLBACK_ROUTE_ACCOUNT_BUDGET = 32
MAX_ROUTE_ACCOUNTS = 40
MAX_ROUTE_DATA_BYTES = 768
EXCLUDED_DEXES = "Scorch"
ROUTE_V2 = bytes([187, 100, 250, 204, 49, 196, 175, 20])
MODEL_VERSION = "shortwin-simple-exact-q-v3-generational-queue"
U64_MAX = (1 << 64) - 1


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def digest(value: object) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def integer(value: object, name: str, minimum: int = 0, maximum: int = U64_MAX) -> int:
    if isinstance(value, bool) or not isinstance(value, (int, str)):
        raise ValueError(f"{name} must be an integer")
    if isinstance(value, str) and (not value.isascii() or not value.isdecimal()):
        raise ValueError(f"{name} must be an unsigned decimal integer")
    number = int(value)
    if not minimum <= number <= maximum:
        raise ValueError(f"{name} outside [{minimum}, {maximum}]")
    return number


def public_key(value: object, name: str) -> str:
    ordered = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    if not isinstance(value, str) or not 32 <= len(value) <= 44 or not set(value) <= set(ordered):
        raise ValueError(f"{name} must be a base58 public key string")
    number = 0
    for character in value:
        number = number * 58 + ordered.index(character)
    decoded_length = (number.bit_length() + 7) // 8 + len(value) - len(value.lstrip("1"))
    if decoded_length != 32:
        raise ValueError(f"{name} must decode to exactly 32 bytes")
    return value


def stable_uuid(value: object, name: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{name} must be a canonical non-nil UUID")
    try:
        parsed = uuid.UUID(value)
    except (ValueError, AttributeError) as error:
        raise ValueError(f"{name} must be a canonical non-nil UUID") from error
    if str(parsed) != value or parsed.int == 0:
        raise ValueError(f"{name} must be a canonical non-nil UUID")
    return value


@dataclass(frozen=True)
class Settings:
    epoch_seconds: int
    history_epochs: int
    max_sample_gap_seconds: int
    reference_max_age_seconds: int
    reference_max_age_slots: int
    slippage_bps: int
    max_spread_bps: int
    max_buy_lamports: int
    max_ask_requests: int
    max_jobs: int
    jobs_per_tick: int
    request_timeout_seconds: int
    request_interval_ms: int = 1000

    def __post_init__(self) -> None:
        bounds = {"epoch_seconds": (1, 300), "history_epochs": (2, 8),
                  "max_sample_gap_seconds": (1, self.epoch_seconds),
                  "reference_max_age_seconds": (1, 60), "reference_max_age_slots": (1, 1000),
                  "slippage_bps": (0, 1000), "max_spread_bps": (0, 10000),
                  "max_buy_lamports": (1, U64_MAX), "max_ask_requests": (1, 32),
                  "max_jobs": (1, 4096), "jobs_per_tick": (1, self.max_jobs),
                  "request_timeout_seconds": (1, 30), "request_interval_ms": (1000, 60000)}
        for key, (low, high) in bounds.items():
            value = getattr(self, key)
            if not isinstance(value, int):
                raise ValueError(f"{key} must be a JSON integer")
            integer(value, key, low, high)

    @property
    def model_hash(self) -> str:
        # Match immutable Policy.price_policy_hash before enabling opens.
        return digest({"model": MODEL_VERSION, "settings": asdict(self),
                       "route_profile": {"request_account_budgets": [REQUEST_ROUTE_ACCOUNT_BUDGET, FALLBACK_ROUTE_ACCOUNT_BUDGET],
                                         "maximum_total_accounts": MAX_ROUTE_ACCOUNTS,
                                         "maximum_instruction_data_bytes": MAX_ROUTE_DATA_BYTES,
                                         "program": JUPITER_PROGRAM, "swap_mode": "ExactIn",
                                         "transaction_version": 0, "header": "ordinary-route-v2-zero-fees",
                                          "observation_ask": "first-actual-full-q-within-live-spread",
                                          "exclude_dexes": EXCLUDED_DEXES}})


def verify_policy(settings: Settings, policy: dict) -> None:
    """Check decoded immutable on-chain terms before a publisher signs anything."""
    if not isinstance(policy, dict):
        raise ValueError("decoded policy must be an object")
    terms = policy.get("terms", policy)
    if not isinstance(terms, dict):
        raise ValueError("decoded policy terms must be an object")
    fields = {"historyEpochSeconds": settings.epoch_seconds,
              "minimumHistoryEpochs": settings.history_epochs,
              "referenceMaxAgeSeconds": settings.reference_max_age_seconds,
              "referenceMaxAgeSlots": settings.reference_max_age_slots,
              "maxSpreadBps": settings.max_spread_bps}
    for name, expected in fields.items():
        if integer(terms.get(name), name) != expected:
            raise ValueError(f"immutable policy {name} differs from this history model")
    model_hash = terms.get("pricePolicyHash")
    if isinstance(model_hash, list) and len(model_hash) == 32:
        model_hash = bytes(integer(item, "policy hash byte", 0, 255) for item in model_hash).hex()
    if model_hash != settings.model_hash:
        raise ValueError("immutable policy pricePolicyHash differs from this history model")


def read_json(path: Path) -> object:
    return json.loads(sys.stdin.read() if str(path) == "-" else path.read_text())


@dataclass(frozen=True)
class Job:
    market: str
    policy: str
    mint: str
    quantity: int
    active_loan: bool = False

    def __post_init__(self) -> None:
        for name in ("market", "policy", "mint"):
            public_key(getattr(self, name), name)
        object.__setattr__(self, "quantity", integer(self.quantity, "quantity", 1))
        if not isinstance(self.active_loan, bool):
            raise ValueError("active_loan must be boolean")
        if self.mint == WSOL:
            raise ValueError("WSOL is collateral, not a base-token market")

    @property
    def key(self) -> tuple[str, str, str]:
        return self.market, self.policy, str(self.quantity)


@dataclass(frozen=True)
class Quote:
    amount: int
    output: int
    guaranteed_output: int
    raw_hash: str
    raw: dict


class Unavailable(RuntimeError):
    """Missing, malformed, stale, or unsuccessful observations are Unknown."""


class RouteUnavailable(Unavailable):
    """A bounded alternate account budget may provide a supported route."""


class Jupiter:
    def __init__(self, settings: Settings, taker: str, api_key: str,
                 transport: Callable[[dict], dict] | None = None, *,
                 clock: Callable[[], float] | None = None, sleeper: Callable[[float], None] | None = None,
                 rate_gate: Callable[[], None] | None = None) -> None:
        self.settings, self.taker, self.api_key, self.transport = settings, taker, api_key, transport
        self.clock, self.sleeper = clock or time.monotonic, sleeper or time.sleep
        self.rate_gate, self.last_request = rate_gate, None

    def _get(self, params: dict) -> dict:
        if self.transport:
            return self.transport(params)
        headers = {"Accept": "application/json"}
        if self.api_key:
            headers["x-api-key"] = self.api_key
        request = urllib.request.Request(BUILD_URL + "?" + urllib.parse.urlencode(params), headers=headers)
        try:
            with urllib.request.urlopen(request, timeout=self.settings.request_timeout_seconds) as response:
                raw = response.read(2_000_001)
                if len(raw) > 2_000_000:
                    raise Unavailable("Jupiter response exceeds bounded size")
                return json.loads(raw)
        except urllib.error.HTTPError as error:
            # 400 includes no-route responses. Do not amplify rate limits,
            # authentication failures, provider outages or connection failures.
            if error.code == 400:
                raise RouteUnavailable("Jupiter has no route for this request budget") from None
            raise Unavailable(f"Jupiter read failed: HTTP {error.code}") from None
        except (OSError, ValueError) as error:
            # Never retain URLs, headers, credentials or provider error bodies.
            raise Unavailable(f"Jupiter read failed: {type(error).__name__}") from None

    def quote(self, input_mint: str, output_mint: str, amount: int) -> Quote:
        last = None
        for budget in (REQUEST_ROUTE_ACCOUNT_BUDGET, FALLBACK_ROUTE_ACCOUNT_BUDGET):
            try:
                return self._quote(input_mint, output_mint, amount, budget)
            except RouteUnavailable as error:
                last = error
        raise last or Unavailable("no supported bounded route")

    def _quote(self, input_mint: str, output_mint: str, amount: int, account_budget: int) -> Quote:
        integer(amount, "quote amount", 1)
        params = {"inputMint": input_mint, "outputMint": output_mint,
                  "amount": str(amount), "taker": self.taker,
                  "slippageBps": str(self.settings.slippage_bps),
                  "wrapAndUnwrapSol": "false", "maxAccounts": str(account_budget), "transactionVersion": "0", "excludeDexes": EXCLUDED_DEXES}
        # /swap/v2/build is ExactIn. Do not send an ignored ExactOut flag.
        if self.rate_gate:
            self.rate_gate()
        else:
            now = self.clock()
            if self.last_request is not None:
                self.sleeper(max(0, self.settings.request_interval_ms/1000 - (now-self.last_request)))
            self.last_request = self.clock()
        raw = self._get(params)
        try:
            if not isinstance(raw, dict) or raw.get("error"):
                raise ValueError("no successful quote")
            if (raw["inputMint"], raw["outputMint"], raw["swapMode"]) != (input_mint, output_mint, "ExactIn"):
                raise ValueError("quote domain mismatch")
            if integer(raw["inAmount"], "inAmount", 1) != amount:
                raise ValueError("exact input mismatch")
            output = integer(raw["outAmount"], "outAmount", 1)
            guaranteed = integer(raw["otherAmountThreshold"], "otherAmountThreshold", 1)
            if guaranteed > output:
                raise ValueError("threshold exceeds expected output")
            if integer(raw["slippageBps"], "slippageBps", 0, 1000) != self.settings.slippage_bps:
                raise ValueError("slippage mismatch")
            if not raw.get("swapInstruction") or not isinstance(raw.get("routePlan"), list) or not raw["routePlan"]:
                raise ValueError("missing executable route witness")
            if any(not isinstance(step, dict) or not isinstance(step.get("swapInfo", {}), dict) for step in raw["routePlan"]):
                raise ValueError("malformed route plan witness")
            if any(step.get("swapInfo", {}).get("label") in EXCLUDED_DEXES.split(",") for step in raw["routePlan"]):
                raise ValueError("route uses an excluded incompatible DEX")
            swap = raw["swapInstruction"]
            if (not isinstance(swap, dict) or swap.get("programId") != JUPITER_PROGRAM
                    or not isinstance(swap.get("accounts"), list) or not 1 <= len(swap["accounts"]) <= MAX_ROUTE_ACCOUNTS
                    or not isinstance(swap.get("data"), str) or len(swap["data"]) > (MAX_ROUTE_DATA_BYTES+2)//3*4
                    or not 8 <= len(base64.b64decode(swap["data"], validate=True)) <= MAX_ROUTE_DATA_BYTES):
                raise ValueError("route lies outside the current program/client envelope")
            data = base64.b64decode(swap["data"], validate=True)
            # Match the program's supported scalar envelope before any quote
            # becomes a trusted price. Shared/legacy/ExactOut responses are Unknown.
            if (len(data) < 39 or data[:8] != ROUTE_V2
                    or int.from_bytes(data[8:16], "little") != amount
                    or int.from_bytes(data[16:24], "little") != output
                    or int.from_bytes(data[24:26], "little") != self.settings.slippage_bps
                    or data[26:30] != bytes(4)):
                raise ValueError("unsupported or mismatched ordinary RouteV2 header")
            steps = int.from_bytes(data[30:34], "little")
            if not 1 <= steps <= 8 or len(data)-34 < steps*5:
                raise ValueError("route step envelope mismatch")
            if guaranteed != (output * (10000-self.settings.slippage_bps)+9999)//10000:
                raise ValueError("threshold does not match signed route slippage")
        except (KeyError, TypeError, ValueError) as error:
            raise RouteUnavailable(f"Jupiter quote rejected: {error}") from None
        return Quote(amount, output, guaranteed, digest(raw), raw)

    def observe(self, job: Job, previous_ask: int | None = None) -> tuple[int, int, dict]:
        sale = self.quote(job.mint, WSOL, job.quantity)
        # A fresh executable ask need not be numerically minimal. Reuse the
        # last successful budget when still within today's live spread. At most
        # one capped growth probe is needed because the initial budget >= bid.
        cap = min(self.settings.max_buy_lamports,
                  sale.guaranteed_output * (10000+self.settings.max_spread_bps)//10000)
        ask = self.buy(job, max(previous_ask or 0, sale.guaranteed_output), cap,
                       retain_first=True)
        return sale.guaranteed_output, ask.amount, {
            "sale": {"hash": sale.raw_hash, "response": sale.raw},
            "buy": {"hash": ask.raw_hash, "response": ask.raw}}

    def buy(self, job: Job, initial: int, maximum: int | None = None, *,
            retain_first: bool = False) -> Quote:
        """Return only an actually quoted ExactIn budget guaranteeing all q tokens.

        Router monotonicity is not assumed. Binary probes improve a retained
        successful candidate; interpolation never creates an executable ask.
        """
        cap = self.settings.max_buy_lamports
        if maximum is not None:
            cap = min(cap, integer(maximum, "maximum input", 1))
        started = self.clock()

        def probe(budget: int) -> Quote:
            result = self.quote(WSOL, job.mint, budget)
            if self.clock() - started > self.settings.reference_max_age_seconds:
                raise Unavailable("acquisition search outlived reference freshness")
            return result

        amount = min(max(1, integer(initial, "initial budget", 1)), cap)
        used, low, best = 0, 0, None
        while used < self.settings.max_ask_requests:
            quote = probe(amount)
            used += 1
            if quote.guaranteed_output >= job.quantity:
                best = quote
                if retain_first:
                    return best
                break
            low = amount
            if amount == cap:
                raise Unavailable("no full-token acquisition within configured SOL budget")
            amount = min(cap, amount * 2)
        if best is None:
            raise Unavailable("bounded acquisition search exhausted")
        while used < self.settings.max_ask_requests and best.amount - low > 1:
            amount = (best.amount + low) // 2
            quote = probe(amount)
            used += 1
            if quote.guaranteed_output >= job.quantity:
                best = quote
            else:
                low = amount
        return best


class RpcClock:
    def __init__(self, url: str, timeout: int = 10) -> None:
        self.url, self.timeout = url, timeout

    def _call(self, method: str, params: list) -> object:
        request = urllib.request.Request(self.url, data=canonical({"jsonrpc": "2.0", "id": 1,
            "method": method, "params": params}), headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                body = response.read(1_000_001)
                if len(body) > 1_000_000:
                    raise Unavailable("RPC response exceeds bounded size")
                result = json.loads(body)
            if result.get("error") or "result" not in result:
                raise ValueError("RPC returned no result")
            return result["result"]
        except (OSError, TypeError, ValueError, urllib.error.HTTPError) as error:
            raise Unavailable(f"RPC read failed: {type(error).__name__}") from None

    def read(self) -> tuple[int, int]:
        slot = integer(self._call("getSlot", [{"commitment": "confirmed"}]), "RPC slot", 1)
        timestamp = integer(self._call("getBlockTime", [slot]), "RPC block time", 1, (1 << 63) - 1)
        return slot, timestamp


class History:
    def __init__(self, path: str | Path, settings: Settings) -> None:
        self.settings = settings
        self.db = sqlite3.connect(path)
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("PRAGMA synchronous=FULL")
        exists = self.db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='metadata'").fetchone()
        prior = self.db.execute("SELECT value FROM metadata WHERE key='model_hash'").fetchone() if exists else None
        if prior and prior[0] != settings.model_hash:
            self.db.close()
            raise ValueError("history model changed: preserve the old database; use a new database and requalify history")
        if prior and self.db.execute("SELECT 1 FROM metadata WHERE key='history_instance_id'").fetchone() is None:
            self.db.close()
            raise ValueError("existing history instance identity is missing; preserve this database for diagnosis")
        self.db.executescript("""
        CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS jobs(market TEXT NOT NULL,policy TEXT NOT NULL,q TEXT NOT NULL,
          mint TEXT NOT NULL,active INTEGER NOT NULL,due INTEGER NOT NULL,failures INTEGER NOT NULL DEFAULT 0,
          generation INTEGER NOT NULL,PRIMARY KEY(market,policy,q));
        CREATE TABLE IF NOT EXISTS identities(market TEXT NOT NULL,policy TEXT NOT NULL,q TEXT NOT NULL,
          mint TEXT NOT NULL,generation INTEGER NOT NULL,admitted_at INTEGER NOT NULL,PRIMARY KEY(market,policy,q));
        CREATE TABLE IF NOT EXISTS publication_requests(market TEXT NOT NULL,policy TEXT NOT NULL,q TEXT NOT NULL,
          requested_at INTEGER NOT NULL,PRIMARY KEY(market,policy,q));
        CREATE TABLE IF NOT EXISTS observations(market TEXT NOT NULL,policy TEXT NOT NULL,q TEXT NOT NULL,
          generation INTEGER NOT NULL,ts INTEGER NOT NULL,slot TEXT NOT NULL,bid TEXT NOT NULL,ask TEXT NOT NULL,finite INTEGER NOT NULL,
          evidence TEXT NOT NULL,reason TEXT NOT NULL,PRIMARY KEY(market,policy,q,generation,ts));
        CREATE TABLE IF NOT EXISTS retirements(market TEXT NOT NULL,policy TEXT NOT NULL,q TEXT NOT NULL,
          generation INTEGER NOT NULL,context_slot TEXT NOT NULL,clock_slot TEXT NOT NULL,clock_ts INTEGER NOT NULL,
          snapshot_hash TEXT NOT NULL,body TEXT NOT NULL,PRIMARY KEY(market,policy,q,generation));
        """)
        with self.db:
            self.db.execute("INSERT OR IGNORE INTO metadata VALUES('model_hash',?)", (settings.model_hash,))
            self.db.execute("INSERT OR IGNORE INTO metadata VALUES('queue_revision','0')")
            self.db.execute("INSERT OR IGNORE INTO metadata VALUES('history_instance_id',?)", (str(uuid.uuid4()),))

    def history_instance_id(self) -> str:
        row = self.db.execute("SELECT value FROM metadata WHERE key='history_instance_id'").fetchone()
        if row is None:
            raise ValueError("history instance identity is missing; preserve this database for diagnosis")
        return stable_uuid(row[0], "history instance identity")

    def operator_binding(self) -> dict | None:
        row = self.db.execute("SELECT value FROM metadata WHERE key='operator_binding'").fetchone()
        return self._validate_operator_binding(json.loads(row[0])) if row else None

    @staticmethod
    def _validate_operator_binding(payload: object) -> dict:
        fields = {"schema", "program_id", "operator_store_id", "operator_database"}
        if not isinstance(payload, dict) or set(payload) != fields or type(payload.get("schema")) is not int or payload["schema"] != 1:
            raise ValueError("exact schema-1 operator binding required")
        public_key(payload["program_id"], "binding program_id")
        stable_uuid(payload["operator_store_id"], "binding operator_store_id")
        path = payload["operator_database"]
        if not isinstance(path, str) or not path.startswith("/") or len(path) > 4096 or "\0" in path or os.path.normpath(path) != path:
            raise ValueError("binding operator_database must be an absolute normalized path")
        return dict(payload)

    def bind_operator(self, payload: object) -> dict:
        """Trusted local immutable identity binding; neither signs nor moves funds."""
        binding = self._validate_operator_binding(payload)
        self.db.execute("BEGIN IMMEDIATE")
        try:
            identity = self.history_instance_id()
            prior = self.operator_binding()
            if prior is not None and prior != binding:
                raise ValueError("operator binding differs from the persistent history binding; preserve both databases")
            if prior is None:
                self.db.execute("INSERT INTO metadata VALUES('operator_binding',?)", (canonical(binding).decode(),))
            self.db.commit()
            return {"schema": 1, "history_instance_id": identity, "operator_binding": binding}
        except BaseException:
            self.db.rollback()
            raise

    def revision(self) -> int:
        return int(self.db.execute("SELECT value FROM metadata WHERE key='queue_revision'").fetchone()[0])

    def _bump_revision(self) -> None:
        value = integer(self.revision()+1, "queue revision", 1, U64_MAX)
        self.db.execute("UPDATE metadata SET value=? WHERE key='queue_revision'", (str(value),))

    def generation(self, job: Job) -> int | None:
        row = self.db.execute("SELECT mint,generation FROM jobs WHERE market=? AND policy=? AND q=?", job.key).fetchone()
        if row and row[0] != job.mint:
            raise ValueError("a market-policy-quantity identity cannot change mint")
        return int(row[1]) if row else None

    def queue_state(self) -> dict:
        self.db.execute("BEGIN")
        try:
            result = {"schema": 1, "revision": str(self.revision()), "jobs": self.job_rows(),
                      "history_instance_id": self.history_instance_id(), "operator_binding": self.operator_binding()}
            last = self.db.execute("SELECT value FROM metadata WHERE key='last_reconcile_context'").fetchone()
            result["last_context_slot"] = last[0] if last else "0"
            self.db.commit()
            return result
        except BaseException:
            self.db.rollback()
            raise

    def job_rows(self) -> list[dict]:
        return [dict(asdict(job), quantity=str(job.quantity), generation=self.generation(job),
                     publication_requested_at=self.publication_requested_at(job)) for job in self.jobs()]

    def close(self) -> None:
        self.db.close()

    def rate_gate(self, clock: Callable[[], float] = time.time,
                  sleeper: Callable[[float], None] = time.sleep) -> None:
        """Reserve one provider request across processes sharing this database."""
        now = clock()
        try:
            self.db.execute("BEGIN IMMEDIATE")
            previous = self.db.execute("SELECT value FROM metadata WHERE key='last_request_at'").fetchone()
            scheduled = max(now, float(previous[0]) + self.settings.request_interval_ms/1000) if previous else now
            if scheduled-now > self.settings.reference_max_age_seconds:
                raise Unavailable("shared provider request queue exceeds freshness budget")
            self.db.execute("INSERT OR REPLACE INTO metadata VALUES('last_request_at',?)", (str(scheduled),))
            self.db.commit()
        except BaseException:
            self.db.rollback()
            raise
        sleeper(max(0, scheduled-now))

    def enqueue(self, job: Job, now: int) -> bool:
        self.db.execute("BEGIN IMMEDIATE")
        try:
            accepted = self._enqueue(job, now)
            if accepted:
                if not job.active_loan:
                    self.db.execute("INSERT OR REPLACE INTO publication_requests VALUES(?,?,?,?)", (*job.key, now))
                self._bump_revision()
            self.db.commit()
            return accepted
        except BaseException:
            self.db.rollback()
            raise

    def _enqueue(self, job: Job, now: int) -> bool:
        integer(now, "now", 1, (1 << 63) - 1)
        existing = self.db.execute("SELECT mint,active FROM jobs WHERE market=? AND policy=? AND q=?", job.key).fetchone()
        if existing:
            if existing[0] != job.mint:
                raise ValueError("a market-policy-quantity identity cannot change mint")
            self.db.execute("UPDATE jobs SET active=MAX(active,?) WHERE market=? AND policy=? AND q=?", (int(job.active_loan), *job.key))
            return True
        if self.db.execute("SELECT count(*) FROM jobs").fetchone()[0] >= self.settings.max_jobs:
            return False
        identity = self.db.execute("SELECT mint,generation FROM identities WHERE market=? AND policy=? AND q=?", job.key).fetchone()
        if identity and identity[0] != job.mint:
            raise ValueError("a retired market-policy-quantity identity cannot change mint")
        generation = integer(int(identity[1])+1 if identity else 0, "generation", 0, (1 << 63)-1)
        self.db.execute("INSERT OR REPLACE INTO identities VALUES(?,?,?,?,?,?)", (*job.key, job.mint, generation, now))
        self.db.execute("INSERT INTO jobs(market,policy,q,mint,active,due,generation) VALUES(?,?,?,?,?,?,?)", (*job.key, job.mint, int(job.active_loan), now, generation))
        return True

    def plan(self, now: int) -> list[Job]:
        rows = self.db.execute("SELECT market,policy,mint,q,active FROM jobs WHERE due<=? ORDER BY active DESC,due,market,policy,q LIMIT ?",
                               (now, self.settings.jobs_per_tick)).fetchall()
        return [Job(market, policy, mint, int(q), bool(active)) for market, policy, mint, q, active in rows]

    def jobs(self) -> list[Job]:
        rows = self.db.execute("SELECT market,policy,mint,q,active FROM jobs ORDER BY active DESC,due,market,policy,q LIMIT ?",
                               (self.settings.max_jobs,)).fetchall()
        return [Job(market, policy, mint, int(q), bool(active)) for market, policy, mint, q, active in rows]

    def publication_requested_at(self, job: Job) -> int:
        row = self.db.execute("SELECT requested_at FROM publication_requests WHERE market=? AND policy=? AND q=?", job.key).fetchone()
        return int(row[0]) if row else 0

    def sync_loans(self, jobs: list[Job], now: int) -> None:
        """Legacy trusted priority addition only; cannot demote, retire or evict."""
        if len(jobs) > self.settings.max_jobs or len({job.key for job in jobs}) != len(jobs):
            raise ValueError("active-loan snapshot exceeds capacity or has duplicate identities")
        self.db.execute("BEGIN IMMEDIATE")
        try:
            for job in jobs:
                if not self._enqueue(Job(job.market, job.policy, job.mint, job.quantity, True), now):
                    raise ValueError("active-loan queue capacity exhausted; trusted reconciliation required")
            self._bump_revision()
            self.db.commit()
        except BaseException:
            self.db.rollback()
            raise

    def reconcile_queue(self, payload: dict, now: int) -> dict:
        """Trusted finalized complete snapshot under the operator publisher lock.

        This local contract is not an RPC proof. A public request cannot retire
        anything, and a mismatched queue revision or snapshot fails atomically.
        """
        required = {"schema", "complete", "commitment", "publisher_lock", "context_slot", "clock_slot", "clock_ts", "expected_queue_revision", "active_jobs", "references", "pending_publications", "fulfilled_requests"}
        if not isinstance(payload, dict) or set(payload) != required:
            raise ValueError("complete versioned queue snapshot field set required")
        if type(payload["schema"]) is not int or payload["schema"] != 1 or payload["complete"] is not True or payload["commitment"] != "finalized" or payload["publisher_lock"] is not True:
            raise ValueError("trusted complete finalized snapshot under publisher lock required")
        context = integer(payload["context_slot"], "snapshot context slot", 1)
        clock_slot = integer(payload["clock_slot"], "snapshot final clock slot", context, context)
        clock_ts = integer(payload["clock_ts"], "snapshot final clock timestamp", 1, (1 << 63)-1)
        integer(now, "local wall time", 1, (1 << 63)-1)
        if abs(now-clock_ts) > self.settings.reference_max_age_seconds:
            raise ValueError("snapshot clock is stale relative to local wall time")
        expected_revision = integer(payload["expected_queue_revision"], "expected queue revision")

        def snapshot_job(value: object) -> Job:
            if not isinstance(value, dict):
                raise ValueError("snapshot job must be an object")
            try:
                return Job(**value)
            except TypeError as error:
                raise ValueError("snapshot job field set is invalid") from error

        def jobs(values: object, name: str, maximum: int) -> dict:
            if not isinstance(values, list) or len(values) > maximum:
                raise ValueError(name+" must be a bounded complete list")
            result = {}
            for value in values:
                if not isinstance(value, dict):
                    raise ValueError(name+" contains an invalid job")
                item = snapshot_job(value)
                if item.key in result:
                    raise ValueError(name+" contains duplicate identities")
                result[item.key] = item
            return result
        active = jobs(payload["active_jobs"], "active jobs", self.settings.max_jobs)
        pending = jobs(payload["pending_publications"], "pending publications", 4096)
        refs = {}
        values = payload["references"]
        if not isinstance(values, list) or len(values) > 10000:
            raise ValueError("complete bounded finite reference list required")
        for value in values:
            if not isinstance(value, dict) or set(value) != {"job", "source_slot", "source_ts", "max_age_seconds", "max_age_slots"}:
                raise ValueError("finite reference fields required")
            item = snapshot_job(value["job"])
            if item.key in refs:
                raise ValueError("duplicate finite reference identity")
            source_slot = integer(value["source_slot"], "reference source slot", 1, context)
            source_ts = integer(value["source_ts"], "reference source timestamp", 1, clock_ts)
            age_seconds = integer(value["max_age_seconds"], "reference max age seconds", 1, 60)
            age_slots = integer(value["max_age_slots"], "reference max age slots", 1, 1000)
            refs[item.key] = (item, clock_ts-source_ts > age_seconds and clock_slot-source_slot > age_slots)
        acknowledgments = {}
        values = payload["fulfilled_requests"]
        if not isinstance(values, list) or len(values) > 4096:
            raise ValueError("bounded fulfilled request list required")
        for value in values:
            if not isinstance(value, dict) or set(value) != {"job", "generation", "requested_at"}:
                raise ValueError("generation-bound fulfillment fields required")
            item = snapshot_job(value["job"])
            generation = integer(value["generation"], "acknowledged generation", 0, (1 << 63)-1)
            timestamp = integer(value["requested_at"], "acknowledged request", 0, (1 << 63)-1)
            key = (*item.key, generation)
            if key in acknowledgments:
                raise ValueError("duplicate fulfilled request identity")
            acknowledgments[key] = (item, timestamp)
        fingerprint = digest(payload)
        self.db.execute("BEGIN IMMEDIATE")
        try:
            def meta(name: str, default: str = "0") -> str:
                row = self.db.execute("SELECT value FROM metadata WHERE key=?", (name,)).fetchone()
                return row[0] if row else default
            previous = int(meta("last_reconcile_context"))
            if context == previous and fingerprint == meta("last_reconcile_hash", ""):
                result = json.loads(meta("last_reconcile_result"))
                if self.revision() != int(result["revision"]):
                    raise ValueError("queue revision changed since prior snapshot application")
                result.update(idempotent=True)
                self.db.commit()
                return result
            if context <= previous or clock_slot < int(meta("last_reconcile_clock_slot")) or clock_ts < int(meta("last_reconcile_clock_ts")):
                raise ValueError("snapshot is stale, out of order or conflicts at the same context slot")
            if self.revision() != expected_revision:
                raise ValueError("queue revision changed while snapshot was collected")
            for collection in [active, pending, {key: value[0] for key, value in refs.items()}]:
                for item in collection.values():
                    existing = self.db.execute("SELECT mint FROM identities WHERE market=? AND policy=? AND q=?", item.key).fetchone()
                    if existing and existing[0] != item.mint:
                        raise ValueError("trusted snapshot mint differs from admitted identity")
            retired, blocked = [], []
            ttl = (self.settings.history_epochs+2)*self.settings.epoch_seconds+self.settings.reference_max_age_seconds
            for item in self.jobs():
                generation = self.generation(item)
                if item.key in active:
                    self.db.execute("UPDATE jobs SET active=1 WHERE market=? AND policy=? AND q=?", item.key)
                    blocked.append({"job": asdict(item), "reason": "active_debt"})
                    continue
                self.db.execute("UPDATE jobs SET active=0 WHERE market=? AND policy=? AND q=?", item.key)
                reason = "pending_publication" if item.key in pending else None
                if reason is None and item.key in refs and not refs[item.key][1]:
                    reason = "finite_reference_not_expired_in_both_clocks"
                requested = self.publication_requested_at(item)
                acknowledged = acknowledgments.get((*item.key, generation))
                fulfilled = acknowledged is not None and acknowledged[0].mint == item.mint and acknowledged[1] >= requested
                if reason is None and requested and not fulfilled and (requested > clock_ts or clock_ts-requested <= ttl):
                    reason = "unfulfilled_warm_intent"
                if reason:
                    blocked.append({"job": asdict(item), "reason": reason})
                    continue
                rows = self.db.execute("SELECT ts,slot,bid,ask,finite,evidence,reason FROM observations WHERE market=? AND policy=? AND q=? AND generation=? ORDER BY ts", (*item.key, generation)).fetchall()
                body = {"job": asdict(item), "generation": generation, "requested_at": requested,
                        "retained_observation_count": len(rows), "retained_observations_digest": digest(rows)}
                self.db.execute("INSERT INTO retirements VALUES(?,?,?,?,?,?,?,?,?)", (*item.key, generation, str(context), str(clock_slot), clock_ts, fingerprint, json.dumps(body, sort_keys=True)))
                self.db.execute("DELETE FROM jobs WHERE market=? AND policy=? AND q=?", item.key)
                retired.append(body)
            for item in active.values():
                if not self._enqueue(Job(item.market, item.policy, item.mint, item.quantity, True), now):
                    raise ValueError("complete active-debt set cannot fit without evicting a protected identity")
            self._bump_revision()
            result = {"schema": 1, "revision": str(self.revision()), "context_slot": str(context),
                      "clock_slot": str(clock_slot), "clock_ts": clock_ts, "snapshot_hash": fingerprint,
                      "retired": retired, "blocked": blocked, "active_identities": len(active),
                      "scheduled_identities": len(self.jobs()), "idempotent": False}
            for key, value in {"last_reconcile_context": str(context), "last_reconcile_clock_slot": str(clock_slot),
                               "last_reconcile_clock_ts": str(clock_ts), "last_reconcile_hash": fingerprint,
                               "last_reconcile_result": json.dumps(result, sort_keys=True)}.items():
                self.db.execute("INSERT OR REPLACE INTO metadata VALUES(?,?)", (key, value))
            self.db.commit()
            return result
        except BaseException:
            self.db.rollback()
            raise

    def record(self, job: Job, timestamp: int, slot: int, bid: int, ask: int,
               evidence: dict | None = None, reason: str = "", expected_generation: int | None = None) -> bool:
        integer(timestamp, "timestamp", 1, (1 << 63) - 1); integer(slot, "slot")
        finite = bid > 0 and ask >= bid and slot > 0
        if finite and (ask - bid) * 10000 > bid * self.settings.max_spread_bps:
            finite, reason = False, "spread exceeds reviewed policy"
        bid, ask = (integer(bid, "bid", 1), integer(ask, "ask", 1)) if finite else (0, 0)
        self.db.execute("BEGIN IMMEDIATE")
        try:
            generation = self.generation(job)
            if generation is None or (expected_generation is not None and generation != expected_generation):
                raise ValueError("observation identity is retired or its admission generation changed")
            previous = self.db.execute("SELECT ts,slot FROM observations WHERE market=? AND policy=? AND q=? AND generation=? ORDER BY ts DESC LIMIT 1", (*job.key, generation)).fetchone()
            if previous and (timestamp <= previous[0] or slot <= int(previous[1])):
                raise ValueError("observations must advance source timestamp and slot")
            self.db.execute("INSERT INTO observations VALUES(?,?,?,?,?,?,?,?,?,?,?)", (*job.key, generation, timestamp, str(slot), str(bid), str(ask), int(finite), json.dumps(evidence or {}, sort_keys=True), reason[:200]))
            self.db.execute("UPDATE jobs SET due=?,failures=CASE WHEN ? THEN 0 ELSE failures+1 END WHERE market=? AND policy=? AND q=? AND generation=?", (timestamp + max(1, self.settings.max_sample_gap_seconds//2), int(finite), *job.key, generation))
            cutoff = timestamp - (self.settings.history_epochs + 2) * self.settings.epoch_seconds
            self.db.execute("DELETE FROM observations WHERE market=? AND policy=? AND q=? AND generation=? AND ts<?", (*job.key, generation, cutoff))
            self._bump_revision()
            self.db.commit()
            return finite
        except BaseException:
            self.db.rollback()
            raise

    def previous_ask(self, job: Job) -> int | None:
        generation = self.generation(job)
        if generation is None:
            return None
        row = self.db.execute("SELECT ask FROM observations WHERE market=? AND policy=? AND q=? AND generation=? AND finite=1 ORDER BY ts DESC LIMIT 1", (*job.key, generation)).fetchone()
        return int(row[0]) if row else None

    def reference(self, job: Job, onchain_sequence: int, current_slot: int, current_ts: int) -> dict:
        integer(onchain_sequence, "onchain sequence", 0, U64_MAX-1)
        integer(current_slot, "current slot", 1); integer(current_ts, "current timestamp", 1, (1 << 63)-1)
        s = self.settings
        generation = self.generation(job)
        latest = self.db.execute("SELECT ts,slot,bid,ask,finite,evidence FROM observations WHERE market=? AND policy=? AND q=? AND generation=? ORDER BY ts DESC LIMIT 1", (*job.key, generation)).fetchone() if generation is not None else None
        empty = {"quantity": str(job.quantity), "sequence": str(onchain_sequence + 1), "source_slot": str(current_slot),
                 "source_ts": current_ts, "finite": False, "live_bid_lower": "0", "live_ask_upper": "0",
                 "history_ask_upper_max": "0", "history_bid_lower_max": "0", "acquisition_reference": "0",
                 "completed_epochs": 0, "history_start_ts": 0, "history_end_ts": 0, "evidence_hash": "00"*32}

        def unknown(reason: str) -> dict:
            return {"market": job.market, "policy": job.policy, "model_hash": s.model_hash,
                    "generation": generation, "status": "Unknown", "reason": reason, "args": empty}

        if not latest or not latest[4]:
            return unknown("missing or unsuccessful latest observation")
        ts, slot, bid, ask = int(latest[0]), int(latest[1]), int(latest[2]), int(latest[3])
        if not (0 <= current_ts-ts <= s.reference_max_age_seconds and 0 <= current_slot-slot <= s.reference_max_age_slots):
            return unknown("latest observation is stale or ahead of current clock")
        epoch_start = ts // s.epoch_seconds * s.epoch_seconds
        start, end = epoch_start - s.history_epochs*s.epoch_seconds, epoch_start-1
        rows = self.db.execute("SELECT ts,slot,bid,ask,finite,evidence FROM observations WHERE market=? AND policy=? AND q=? AND generation=? AND ts>=? AND ts<=? ORDER BY ts", (*job.key, generation, start, end)).fetchall()
        if start < 0 or not rows or any(not row[4] for row in rows):
            return unknown("completed history is missing or contains Unknown")
        gap = s.max_sample_gap_seconds
        if rows[0][0]-start > gap or end-rows[-1][0] > gap or any(b[0]-a[0] > gap for a,b in zip(rows,rows[1:])):
            return unknown("completed epochs have a sampling gap")
        for first in range(start, epoch_start, s.epoch_seconds):
            samples = [row for row in rows if first <= row[0] < first+s.epoch_seconds]
            if not samples or samples[0][0]-first > gap or first+s.epoch_seconds-1-samples[-1][0] > gap:
                return unknown("a completed epoch lacks boundary coverage")
        history_ask, history_bid = max(int(row[3]) for row in rows), max(int(row[2]) for row in rows)
        witness = {"model": MODEL_VERSION, "model_hash": s.model_hash, "generation": generation, "job": asdict(job),
                   "source": [ts,slot,bid,ask,json.loads(latest[5])], "history": rows}
        payload = dict(empty, source_slot=str(slot), source_ts=ts, finite=True, live_bid_lower=str(bid),
                       live_ask_upper=str(ask), history_ask_upper_max=str(history_ask),
                       history_bid_lower_max=str(history_bid), acquisition_reference=str(min(ask,history_ask)),
                       completed_epochs=s.history_epochs, history_start_ts=start, history_end_ts=end,
                       evidence_hash=digest(witness))
        return {"market": job.market, "policy": job.policy, "model_hash": s.model_hash,
                "generation": generation, "status": "Finite", "evidence": witness, "args": payload}


def tick(history: History, jupiter: Jupiter, clock: Callable[[], tuple[int, int]], now: int,
         wall_clock: Callable[[], float] = time.time) -> list[dict]:
    output = []
    for job in history.plan(now):
        # If a usable RPC clock is unavailable, stop without fabricating source
        # time. Previously published references naturally expire on-chain.
        generation = history.generation(job)
        if generation is None:
            continue
        slot, timestamp = clock()
        try:
            if abs(wall_clock()-timestamp) > history.settings.reference_max_age_seconds:
                raise Unavailable("RPC block clock is stale relative to local wall clock")
            bid, ask, evidence = jupiter.observe(job, history.previous_ask(job))
            after_slot, after_ts = clock()
            if not (0 <= after_ts-timestamp <= history.settings.reference_max_age_seconds
                    and 0 <= after_slot-slot <= history.settings.reference_max_age_slots):
                raise Unavailable("quote request outlived reference freshness limits")
            finite = history.record(job, timestamp, slot, bid, ask, evidence, expected_generation=generation)
            output.append({"job": asdict(job), "finite": finite})
        except Unavailable as error:
            history.record(job, timestamp, slot, 0, 0, reason=str(error), expected_generation=generation)
            output.append({"job": asdict(job), "finite": False, "reason": str(error)})
    return output


def capacity_report(settings: Settings) -> dict:
    calls = 2 * (1 + settings.max_ask_requests)
    history_calls = min(calls, 6)
    minimum_cycle = settings.max_jobs * history_calls * settings.request_interval_ms / 1000
    # Request spacing alone is a lower bound. Leave half the sampling gap for
    # provider/RPC latency, transaction work, other API use and scheduler delay.
    sample_target = max(1, settings.max_sample_gap_seconds//2)
    return {"identity_capacity_configured": settings.max_jobs, "requests_per_identity_worst_case": history_calls,
             "execution_buy_http_requests_maximum": 2*settings.max_ask_requests,
             "observation_requests_steady": 2, "observation_requests_maximum": min(calls, 6),
             "maximum_http_requests_per_quote_probe": 2,
             "observation_spacing_cycle_steady_seconds": settings.max_jobs*2*settings.request_interval_ms/1000,
             "observation_spacing_cycle_maximum_seconds": settings.max_jobs*min(calls, 6)*settings.request_interval_ms/1000,
            "spacing_only_cycle_seconds": minimum_cycle, "target_sampling_seconds": sample_target,
            "spacing_fits_target": minimum_cycle <= sample_target,
            "nominal_identities_with_half_gap_margin": int(sample_target*1000 // (history_calls*settings.request_interval_ms)),
            "measured_latency_and_sustained_coverage": "pending",
            "ready_for_mainnet": False,
            "reason": "Measured provider/RPC latency and active-loan coverage are required; this is a spacing lower bound"}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--settings", required=True, type=Path)
    parser.add_argument("--database", type=Path, default=Path("price-history.sqlite3"))
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("model-hash")
    sub.add_parser("capacity")
    check_policy = sub.add_parser("check-policy"); check_policy.add_argument("policy", type=Path)
    enqueue = sub.add_parser("enqueue"); enqueue.add_argument("jobs", type=Path)
    sync = sub.add_parser("sync-loans"); sync.add_argument("jobs", type=Path)
    sub.add_parser("plan")
    sub.add_parser("jobs")
    sub.add_parser("queue-state")
    bind = sub.add_parser("bind-operator"); bind.add_argument("binding", type=Path)
    reconcile = sub.add_parser("reconcile-queue"); reconcile.add_argument("snapshot", type=Path)
    observe = sub.add_parser("tick"); observe.add_argument("--taker", required=True)
    buy = sub.add_parser("build-buy"); buy.add_argument("job", type=Path)
    buy.add_argument("--taker", required=True); buy.add_argument("--maximum-input", required=True, type=int)
    sell = sub.add_parser("build-sell"); sell.add_argument("job", type=Path)
    sell.add_argument("--taker", required=True); sell.add_argument("--minimum-output", required=True, type=int)
    reference = sub.add_parser("reference"); reference.add_argument("job", type=Path)
    reference.add_argument("--onchain-sequence", required=True, type=int)
    reference.add_argument("--current-slot", required=True, type=int)
    reference.add_argument("--current-ts", required=True, type=int)
    args = parser.parse_args(argv)
    settings = Settings(**json.loads(args.settings.read_text()))
    if args.command == "model-hash":
        print(settings.model_hash)
        return 0
    if args.command == "capacity":
        print(json.dumps(capacity_report(settings), sort_keys=True, indent=2))
        return 0
    if args.command == "check-policy":
        verify_policy(settings, read_json(args.policy))
        print(json.dumps({"valid": True, "model_hash": settings.model_hash}, sort_keys=True))
        return 0
    history = History(args.database, settings)
    try:
        now = int(time.time())
        if args.command in ("enqueue", "sync-loans"):
            jobs = read_json(args.jobs)
            if not isinstance(jobs, list) or len(jobs) > settings.max_jobs:
                raise ValueError("bounded job list required")
            if args.command == "sync-loans":
                history.sync_loans([Job(**item) for item in jobs], now)
                result = {"active_identities": len(jobs)}
            else:
                result = [{"job": item, "accepted": history.enqueue(Job(**item), now)} for item in jobs]
        elif args.command == "bind-operator":
            result = history.bind_operator(read_json(args.binding))
        elif args.command == "queue-state":
            result = history.queue_state()
        elif args.command == "reconcile-queue":
            result = history.reconcile_queue(read_json(args.snapshot), now)
        elif args.command in ("plan", "jobs"):
            selected = history.plan(now) if args.command == "plan" else history.jobs()
            result = [dict(asdict(job), quantity=str(job.quantity), generation=history.generation(job),
                           publication_requested_at=history.publication_requested_at(job)) for job in selected]
        elif args.command == "tick":
            rpc, key = os.environ.get("SOLANA_RPC_URL"), os.environ.get("JUPITER_API_KEY", "")
            if not rpc:
                raise ValueError("SOLANA_RPC_URL environment variable is required")
            if not key and settings.request_interval_ms < 2000:
                raise ValueError("keyless provider access requires request_interval_ms >= 2000")
            result = tick(history, Jupiter(settings, args.taker, key, rate_gate=history.rate_gate),
                          RpcClock(rpc, settings.request_timeout_seconds).read, now)
        elif args.command in ("build-buy", "build-sell"):
            key = os.environ.get("JUPITER_API_KEY", "")
            if not key and settings.request_interval_ms < 2000:
                raise ValueError("keyless provider access requires request_interval_ms >= 2000")
            job = Job(**read_json(args.job))
            jupiter = Jupiter(settings, args.taker, key, rate_gate=history.rate_gate)
            if args.command == "build-buy":
                quote = jupiter.buy(job, history.previous_ask(job) or args.maximum_input, args.maximum_input)
            else:
                quote = jupiter.quote(job.mint, WSOL, job.quantity)
                if quote.guaranteed_output < integer(args.minimum_output, "minimum output", 1):
                    raise Unavailable("fresh sale does not guarantee signed minimum proceeds")
            result = {"status": "Ready", "input_amount": str(quote.amount),
                      "build": quote.raw, "evidence_hash": quote.raw_hash, "model_hash": settings.model_hash}
        else:
            result = history.reference(Job(**read_json(args.job)), args.onchain_sequence,
                                       args.current_slot, args.current_ts)
        print(json.dumps(result, sort_keys=True, indent=2))
        return 0
    finally:
        history.close()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, Unavailable, sqlite3.Error) as error:
        raise SystemExit(f"price service stopped without signing or sending: {error}") from None
