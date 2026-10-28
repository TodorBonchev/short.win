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

WSOL = "So11111111111111111111111111111111111111112"
BUILD_URL = "https://api.jup.ag/swap/v2/build"
JUPITER_PROGRAM = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"
REQUEST_ROUTE_ACCOUNT_BUDGET = 32
MAX_ROUTE_ACCOUNTS = 48
MAX_ROUTE_DATA_BYTES = 768
MODEL_VERSION = "shortwin-simple-exact-q-v1"
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
                       "route_profile": {"request_account_budget": REQUEST_ROUTE_ACCOUNT_BUDGET,
                                         "maximum_total_accounts": MAX_ROUTE_ACCOUNTS,
                                         "maximum_instruction_data_bytes": MAX_ROUTE_DATA_BYTES,
                                         "program": JUPITER_PROGRAM, "swap_mode": "ExactIn",
                                         "transaction_version": 0}})


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
        alphabet = set("123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz")
        for key in (self.market, self.policy, self.mint):
            if not isinstance(key, str) or not 32 <= len(key) <= 44 or not set(key) <= alphabet:
                raise ValueError("market, policy, mint must be base58 public key strings")
            number = 0
            ordered = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
            for character in key:
                number = number * 58 + ordered.index(character)
            decoded_length = (number.bit_length() + 7) // 8 + len(key) - len(key.lstrip("1"))
            if decoded_length != 32:
                raise ValueError("public key must decode to exactly 32 bytes")
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
        except (OSError, ValueError, urllib.error.HTTPError) as error:
            # Never retain URLs, headers, credentials or provider error bodies.
            raise Unavailable(f"Jupiter read failed: {type(error).__name__}") from None

    def quote(self, input_mint: str, output_mint: str, amount: int) -> Quote:
        integer(amount, "quote amount", 1)
        params = {"inputMint": input_mint, "outputMint": output_mint,
                  "amount": str(amount), "taker": self.taker,
                  "slippageBps": str(self.settings.slippage_bps),
                  "wrapAndUnwrapSol": "false", "maxAccounts": str(REQUEST_ROUTE_ACCOUNT_BUDGET), "transactionVersion": "0"}
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
            swap = raw["swapInstruction"]
            if (not isinstance(swap, dict) or swap.get("programId") != JUPITER_PROGRAM
                    or not isinstance(swap.get("accounts"), list) or not 1 <= len(swap["accounts"]) <= MAX_ROUTE_ACCOUNTS
                    or not isinstance(swap.get("data"), str) or len(swap["data"]) > (MAX_ROUTE_DATA_BYTES+2)//3*4
                    or not 8 <= len(base64.b64decode(swap["data"], validate=True)) <= MAX_ROUTE_DATA_BYTES):
                raise ValueError("route lies outside the current program/client envelope")
        except (KeyError, TypeError, ValueError) as error:
            raise Unavailable(f"Jupiter quote rejected: {error}") from None
        return Quote(amount, output, guaranteed, digest(raw), raw)

    def observe(self, job: Job, previous_ask: int | None = None) -> tuple[int, int, dict]:
        sale = self.quote(job.mint, WSOL, job.quantity)
        ask = self.buy(job, previous_ask or sale.guaranteed_output)
        return sale.guaranteed_output, ask.amount, {
            "sale": {"hash": sale.raw_hash, "response": sale.raw},
            "buy": {"hash": ask.raw_hash, "response": ask.raw}}

    def buy(self, job: Job, initial: int, maximum: int | None = None) -> Quote:
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
        self.db.executescript("""
        CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS jobs(market TEXT NOT NULL,policy TEXT NOT NULL,q TEXT NOT NULL,
          mint TEXT NOT NULL,active INTEGER NOT NULL,due INTEGER NOT NULL,failures INTEGER NOT NULL DEFAULT 0,
          PRIMARY KEY(market,policy,q));
        CREATE TABLE IF NOT EXISTS observations(market TEXT NOT NULL,policy TEXT NOT NULL,q TEXT NOT NULL,
          ts INTEGER NOT NULL,slot TEXT NOT NULL,bid TEXT NOT NULL,ask TEXT NOT NULL,finite INTEGER NOT NULL,
          evidence TEXT NOT NULL,reason TEXT NOT NULL,PRIMARY KEY(market,policy,q,ts));
        """)
        prior = self.db.execute("SELECT value FROM metadata WHERE key='model_hash'").fetchone()
        if prior and prior[0] != settings.model_hash:
            self.db.close()
            raise ValueError("history model changed: use a new database and requalify its history")
        with self.db:
            self.db.execute("INSERT OR IGNORE INTO metadata VALUES('model_hash',?)", (settings.model_hash,))

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
        with self.db:
            return self._enqueue(job, now)

    def _enqueue(self, job: Job, now: int) -> bool:
        integer(now, "now", 1, (1 << 63) - 1)
        existing = self.db.execute("SELECT mint,active FROM jobs WHERE market=? AND policy=? AND q=?", job.key).fetchone()
        if existing:
            if existing[0] != job.mint:
                raise ValueError("a market-policy-quantity identity cannot change mint")
            # Duplicate discovery cannot demote existing debt. A complete
            # trusted indexer refresh is required to remove debt priority.
            self.db.execute("UPDATE jobs SET active=MAX(active,?) WHERE market=? AND policy=? AND q=?",
                            (int(job.active_loan), *job.key))
            return True
        count = self.db.execute("SELECT count(*) FROM jobs").fetchone()[0]
        if count >= self.settings.max_jobs:
            if not job.active_loan:
                return False
            victim = self.db.execute("SELECT market,policy,q FROM jobs WHERE active=0 ORDER BY due DESC,market,policy,q LIMIT 1").fetchone()
            if victim is None:
                return False
            self.db.execute("DELETE FROM jobs WHERE market=? AND policy=? AND q=?", victim)
            self.db.execute("DELETE FROM observations WHERE market=? AND policy=? AND q=?", victim)
        self.db.execute("INSERT INTO jobs(market,policy,q,mint,active,due) VALUES(?,?,?,?,?,?)",
                        (*job.key, job.mint, int(job.active_loan), now))
        return True

    def plan(self, now: int) -> list[Job]:
        rows = self.db.execute("SELECT market,policy,mint,q,active FROM jobs WHERE due<=? ORDER BY active DESC,due,market,policy,q LIMIT ?",
                               (now, self.settings.jobs_per_tick)).fetchall()
        return [Job(market, policy, mint, int(q), bool(active)) for market, policy, mint, q, active in rows]

    def jobs(self) -> list[Job]:
        rows = self.db.execute("SELECT market,policy,mint,q,active FROM jobs ORDER BY active DESC,due,market,policy,q LIMIT ?",
                               (self.settings.max_jobs,)).fetchall()
        return [Job(market, policy, mint, int(q), bool(active)) for market, policy, mint, q, active in rows]

    def sync_loans(self, jobs: list[Job], now: int) -> None:
        """Trusted complete indexer snapshot only; never accept a public payload."""
        if len(jobs) > self.settings.max_jobs or len({job.key for job in jobs}) != len(jobs):
            raise ValueError("active-loan snapshot exceeds capacity or has duplicate identities")
        with self.db:
            self.db.execute("UPDATE jobs SET active=0")
            for job in jobs:
                if not self._enqueue(Job(job.market, job.policy, job.mint, job.quantity, True), now):
                    raise ValueError("active-loan queue capacity exhausted")

    def record(self, job: Job, timestamp: int, slot: int, bid: int, ask: int,
               evidence: dict | None = None, reason: str = "") -> bool:
        integer(timestamp, "timestamp", 1, (1 << 63) - 1); integer(slot, "slot")
        identity = self.db.execute("SELECT mint FROM jobs WHERE market=? AND policy=? AND q=?", job.key).fetchone()
        if not identity or identity[0] != job.mint:
            raise ValueError("observation identity is not in the bounded queue")
        finite = bid > 0 and ask >= bid and slot > 0
        if finite and (ask - bid) * 10000 > bid * self.settings.max_spread_bps:
            finite, reason = False, "spread exceeds reviewed policy"
        bid, ask = (integer(bid, "bid", 1), integer(ask, "ask", 1)) if finite else (0, 0)
        with self.db:
            previous = self.db.execute("SELECT ts,slot FROM observations WHERE market=? AND policy=? AND q=? ORDER BY ts DESC LIMIT 1", job.key).fetchone()
            if previous and (timestamp <= previous[0] or slot <= int(previous[1])):
                raise ValueError("observations must advance source timestamp and slot")
            self.db.execute("INSERT INTO observations VALUES(?,?,?,?,?,?,?,?,?,?)",
                            (*job.key, timestamp, str(slot), str(bid), str(ask), int(finite),
                             json.dumps(evidence or {}, sort_keys=True), reason[:200]))
            self.db.execute("UPDATE jobs SET due=?,failures=CASE WHEN ? THEN 0 ELSE failures+1 END WHERE market=? AND policy=? AND q=?",
                            (timestamp + max(1, self.settings.max_sample_gap_seconds//2), int(finite), *job.key))
            cutoff = timestamp - (self.settings.history_epochs + 2) * self.settings.epoch_seconds
            self.db.execute("DELETE FROM observations WHERE market=? AND policy=? AND q=? AND ts<?", (*job.key, cutoff))
        return finite

    def previous_ask(self, job: Job) -> int | None:
        row = self.db.execute("SELECT ask FROM observations WHERE market=? AND policy=? AND q=? AND finite=1 ORDER BY ts DESC LIMIT 1", job.key).fetchone()
        return int(row[0]) if row else None

    def reference(self, job: Job, onchain_sequence: int, current_slot: int, current_ts: int) -> dict:
        integer(onchain_sequence, "onchain sequence", 0, U64_MAX-1)
        integer(current_slot, "current slot", 1); integer(current_ts, "current timestamp", 1, (1 << 63)-1)
        s = self.settings
        latest = self.db.execute("SELECT ts,slot,bid,ask,finite,evidence FROM observations WHERE market=? AND policy=? AND q=? ORDER BY ts DESC LIMIT 1", job.key).fetchone()
        empty = {"quantity": str(job.quantity), "sequence": str(onchain_sequence + 1), "source_slot": str(current_slot),
                 "source_ts": current_ts, "finite": False, "live_bid_lower": "0", "live_ask_upper": "0",
                 "history_ask_upper_max": "0", "history_bid_lower_max": "0", "acquisition_reference": "0",
                 "completed_epochs": 0, "history_start_ts": 0, "history_end_ts": 0, "evidence_hash": "00"*32}

        def unknown(reason: str) -> dict:
            return {"market": job.market, "policy": job.policy, "model_hash": s.model_hash,
                    "status": "Unknown", "reason": reason, "args": empty}

        if not latest or not latest[4]:
            return unknown("missing or unsuccessful latest observation")
        ts, slot, bid, ask = int(latest[0]), int(latest[1]), int(latest[2]), int(latest[3])
        if not (0 <= current_ts-ts <= s.reference_max_age_seconds and 0 <= current_slot-slot <= s.reference_max_age_slots):
            return unknown("latest observation is stale or ahead of current clock")
        epoch_start = ts // s.epoch_seconds * s.epoch_seconds
        start, end = epoch_start - s.history_epochs*s.epoch_seconds, epoch_start-1
        rows = self.db.execute("SELECT ts,slot,bid,ask,finite,evidence FROM observations WHERE market=? AND policy=? AND q=? AND ts>=? AND ts<=? ORDER BY ts", (*job.key, start, end)).fetchall()
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
        witness = {"model": MODEL_VERSION, "model_hash": s.model_hash, "job": asdict(job),
                   "source": [ts,slot,bid,ask,json.loads(latest[5])], "history": rows}
        payload = dict(empty, source_slot=str(slot), source_ts=ts, finite=True, live_bid_lower=str(bid),
                       live_ask_upper=str(ask), history_ask_upper_max=str(history_ask),
                       history_bid_lower_max=str(history_bid), acquisition_reference=str(min(ask,history_ask)),
                       completed_epochs=s.history_epochs, history_start_ts=start, history_end_ts=end,
                       evidence_hash=digest(witness))
        return {"market": job.market, "policy": job.policy, "model_hash": s.model_hash,
                "status": "Finite", "evidence": witness, "args": payload}


def tick(history: History, jupiter: Jupiter, clock: Callable[[], tuple[int, int]], now: int,
         wall_clock: Callable[[], float] = time.time) -> list[dict]:
    output = []
    for job in history.plan(now):
        # If a usable RPC clock is unavailable, stop without fabricating source
        # time. Previously published references naturally expire on-chain.
        slot, timestamp = clock()
        try:
            if abs(wall_clock()-timestamp) > history.settings.reference_max_age_seconds:
                raise Unavailable("RPC block clock is stale relative to local wall clock")
            bid, ask, evidence = jupiter.observe(job, history.previous_ask(job))
            after_slot, after_ts = clock()
            if not (0 <= after_ts-timestamp <= history.settings.reference_max_age_seconds
                    and 0 <= after_slot-slot <= history.settings.reference_max_age_slots):
                raise Unavailable("quote request outlived reference freshness limits")
            finite = history.record(job, timestamp, slot, bid, ask, evidence)
            output.append({"job": asdict(job), "finite": finite})
        except Unavailable as error:
            history.record(job, timestamp, slot, 0, 0, reason=str(error))
            output.append({"job": asdict(job), "finite": False, "reason": str(error)})
    return output


def capacity_report(settings: Settings) -> dict:
    calls = 1 + settings.max_ask_requests
    minimum_cycle = settings.max_jobs * calls * settings.request_interval_ms / 1000
    # Request spacing alone is a lower bound. Leave half the sampling gap for
    # provider/RPC latency, transaction work, other API use and scheduler delay.
    sample_target = max(1, settings.max_sample_gap_seconds//2)
    return {"identity_capacity_configured": settings.max_jobs, "requests_per_identity_worst_case": calls,
            "spacing_only_cycle_seconds": minimum_cycle, "target_sampling_seconds": sample_target,
            "spacing_fits_target": minimum_cycle <= sample_target,
            "nominal_identities_with_half_gap_margin": int(sample_target*1000 // (calls*settings.request_interval_ms)),
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
        elif args.command in ("plan", "jobs"):
            selected = history.plan(now) if args.command == "plan" else history.jobs()
            result = [dict(asdict(job), quantity=str(job.quantity)) for job in selected]
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
