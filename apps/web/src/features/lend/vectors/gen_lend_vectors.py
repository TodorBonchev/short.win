#!/usr/bin/env python3
"""Lend-panel preview vectors (FRONTEND.md §10.1, LEND: "deposit / withdraw / claim previews
against pool_model-derived vectors through the WASM").

Drives the normative lender-pool model `docs/design/sim/pool_model.py` v0.4.1 (default switches,
continuous rent) through a lazy op mix — lenders are never touched by a `check()`, so they fall
share epochs behind, their locks expire untouched and full wipes leave parked lenders — and, at
sampled points, writes the state **as the program stores it** (the `Market` lender-pool fields
and flags, `MarketVault`, every `LenderPosition`, every `ShareEpoch` record) together with what
the model answers for each preview the lend panel shows, computed on deep copies:

  deposit AMOUNT     Pool.deposit                      -> shares minted (or the refusal)
  withdraw SHARES    Pool.withdraw                     -> tokens (or the refusal), and `sol`:
                     Pool.claim_sol right after it (the SOL part `withdraw` pays)
  claim              Pool.claim_sol                    -> lamports
  allowance          Pool.allowance_view               -> tokens
  touch              Pool._touch                       -> pending, real, claimed, shares, lock, epoch
  value              _touch, then token_value          -> tokens
  lock               _touch, then _lock_of             -> shares
  lockUntil          Pool.lock_until_for               -> unix seconds
  release            Pool._release                     -> lamports released at `now`

`apps/web/src/features/lend/previews.test.ts` rebuilds `PoolState` / `LenderState` /
`ShareEpochs` from these account-shaped records with the panel's own conversion
(`previews.ts`) and requires `@shortwin/math-wasm` to return exactly the model's numbers.

Mapping of the model to the accounts (ARCH §2.3.4, §2.3.6; `crates/shortwin-math/src/pool.rs`):
`sol_vault` -> `MarketVault.lender_sol`, `protocol` -> `protocol_fees`, `epoch` -> `share_epoch`,
`wipe_live` -> `Market.flags` bit 5, `wipe_swept` -> bit 7 (the model has no venue-reward bit:
`rewards_required = false`), `epochs[e] = [acc_end, num, den, acc_r_end, s_old, carry,
carry_r]` -> `ShareEpoch` with `den_log1000 = log_1000(den)`, the lock calendar `{day: term}` ->
`lock_cal[day mod 128]` with `cal_day` = the oldest bucket day (or today + 1 without buckets: the
program's `cal_advance` drops every day up to today), `lock_epoch = -1` -> `u32::MAX`. States whose
values do not fit the account fields (or whose calendar would not fit the 128-day ring) are not
sampled.

Run from anywhere (deterministic), then format with Biome:

    python3 apps/web/src/features/lend/vectors/gen_lend_vectors.py
    pnpm --filter web exec biome format --write src/features/lend/vectors/lend_vectors.json
"""
import copy
import json
import random
import sys
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[5]
sys.path.insert(0, str(ROOT / "docs" / "design" / "sim"))
import pool_model as pm  # noqa: E402  (v0.4.1)

SEED = 20261001
OPS = 1_200
SAMPLE_EVERY = 100
OUT = HERE / "lend_vectors.json"

U64 = 2**64 - 1
U128 = 2**128 - 1
I64 = (-(2**63), 2**63 - 1)
I128 = (-(2**127), 2**127 - 1)
U32_MAX = 2**32 - 1
DAY = pm.LOCK_BUCKET_S
BUCKETS = pm.LOCK_BUCKETS
FLAG_WIPE_LIVE = 1 << 5
FLAG_WIPE_TRADER_SWEPT = 1 << 7

# pool_model's refusal messages -> the program error names (ARCH §3.11; errors.rs `From<MathError>`)
REFUSALS = [
    ("deposit too small", "ZeroAmount"),
    ("share overflow", "MathOverflow"),
    ("WipeSweepPending", "WipeSweepPending"),
    ("negative SOL balance", "NegativePending"),
    ("not enough shares", "InsufficientShares"),
    ("insufficient unborrowed inventory", "InsufficientFreeInventory"),
    ("above pro-rata share", "AboveProRataAllowance"),
]


class OutOfRange(Exception):
    pass


def u(v, hi):
    if not 0 <= v <= hi:
        raise OutOfRange(v)
    return str(v)


def s(v, rng):
    if not rng[0] <= v <= rng[1]:
        raise OutOfRange(v)
    return str(v)


def refusal(e: ValueError) -> str:
    msg = str(e)
    for needle, name in REFUSALS:
        if needle in msg:
            return "!" + name
    raise e


def den_log(den: int) -> int:
    k = 0
    while den > 1:
        assert den % 1000 == 0, den
        den //= 1000
        k += 1
    return k


def dump_state(p: pm.Pool) -> dict:
    today = p.now // DAY
    days = sorted(p.lock_cal)
    cal_day = min(days[0], today + 1) if days else today + 1
    if days and days[-1] >= cal_day + BUCKETS:
        raise OutOfRange("calendar")
    lock_cal = {}
    for d, term in p.lock_cal.items():
        if term:
            lock_cal[str(d % BUCKETS)] = u(term, U128)
    flags = (FLAG_WIPE_LIVE if p.wipe_live else 0) | (FLAG_WIPE_TRADER_SWEPT if p.wipe_swept else 0)
    market = {
        "tokensFree": u(p.T, U64),
        "borrowedQ": u(p.B, U64),
        "totalShares": u(p.S, U128),
        "acc": s(p.acc, I128),
        "carry": s(p.carry, I128),
        "accR": u(p.acc_r, U128),
        "carryR": u(p.carry_r, U128),
        "dust": s(p.dust, I128),
        "receivable": u(p.receivable, U64),
        "maxRelease": s(p.max_release, I64),
        "vestBal": u(p.vest_bal, U64),
        "vestLast": s(p.vest_last, I64),
        "vestEnd": s(p.vest_end, I64),
        "shareEpoch": p.epoch,
        "flags": flags,
    }
    vault = {
        "lenderSol": u(p.sol_vault, U64),
        "unmatched": u(p.unmatched, U64),
        "protocolFees": u(p.protocol, U64),
        "pi": u(p.pi, U128),
        "lockNorm": u(p.lock_norm, U128),
        "bMin": u(p.b_min, U64),
        "piEpoch": p.pi_epoch,
        "calDay": str(cal_day),
        "lockCal": lock_cal,
    }
    lenders = {}
    for who, L in p.lenders.items():
        lenders[who] = {
            "shares": u(L.shares, U128),
            "snap": s(L.snap, I128),
            "pending": s(L.pending, I64),
            "snapR": u(L.snap_r, U128),
            "real": u(L.real, U64),
            "claimed": u(L.claimed, U64),
            "epoch": L.epoch,
            "lock": u(L.lock, U128),
            "lockPi": u(L.lock_pi, U128),
            "lockEpoch": U32_MAX if L.lock_epoch == -1 else L.lock_epoch,
            "lockTerm": u(L.lock_term, U128),
            "lockUntil": s(L.lock_until, I64),
        }
    records = []
    need = min([L.epoch for L in p.lenders.values()] + [p.epoch - 1 if p.wipe_live else p.epoch])
    for e in sorted(x for x in p.epochs if x >= need):
        acc_end, num, den, acc_r_end, s_old, carry, carry_r = p.epochs[e]
        records.append({
            "epoch": e,
            "accEnd": s(acc_end, I128),
            "accREnd": u(acc_r_end, U128),
            "sOld": u(s_old, U128),
            "carry": s(carry, I128),
            "carryR": u(carry_r, U128),
            "num": num,
            "denLog1000": den_log(den),
        })
    return {"market": market, "vault": vault, "lenders": lenders, "records": records}


def expectations(p: pm.Pool, rng: random.Random) -> list:
    out = []
    lenders = list(p.lenders)

    def run(fn):
        q = copy.deepcopy(p)
        try:
            return fn(q)
        except ValueError as e:
            return refusal(e)

    # deposits: an existing lender and a new one
    for who in [rng.choice(lenders) if lenders else None, "new"]:
        if who is None:
            continue
        for amount in (1, rng.choice([10**6, 10**9, 10**12, rng.randrange(1, 10**13)])):
            out.append({"lender": who, "op": "deposit", "amount": str(amount),
                        "result": run(lambda q, w=who, a=amount: str(q.deposit(w, a)))})
    for who in lenders:
        L = p.lenders[who]
        out.append({"lender": who, "op": "claim", "result": run(lambda q, w=who: str(q.claim_sol(w)))})
        out.append({"lender": who, "op": "allowance", "result": run(lambda q, w=who: str(q.allowance_view(w)))})

        def touched(q, w=who):
            T = q._touch(w)
            return {"pending": str(T.pending), "real": str(T.real), "claimed": str(T.claimed),
                    "shares": str(T.shares), "lock": str(T.lock), "epoch": T.epoch}

        out.append({"lender": who, "op": "touch", "result": run(touched)})
        out.append({"lender": who, "op": "value",
                    "result": run(lambda q, w=who: str(pm.token_value(q, q._touch(w))))})
        out.append({"lender": who, "op": "lock", "result": run(lambda q, w=who: str(q._lock_of(q._touch(w))))})
        if L.shares:
            cands = {1, L.shares, max(1, L.shares // 2)}
            q0 = copy.deepcopy(p)
            allow = q0.free_allowance(who)
            if q0.T + q0.B + pm.VA:
                cands.add(max(1, min(L.shares, allow * (q0.S + pm.VS) // (q0.T + q0.B + pm.VA))))
            for sh in sorted(cands):
                def wd(q, w=who, n=sh):
                    tokens = q.withdraw(w, n)
                    sol = q.claim_sol(w)
                    return {"tokens": str(tokens), "sol": str(sol)}
                out.append({"lender": who, "op": "withdraw", "shares": str(sh), "result": run(wd)})
    q = copy.deepcopy(p)
    out.append({"op": "lockUntil", "result": str(q.lock_until_for())})
    q = copy.deepcopy(p)
    before = q.vest_bal
    if q.S or not q.vest_bal:
        q._release()
        out.append({"op": "release", "result": str(before - q.vest_bal)})
    return out


def step(p: pm.Pool, rng: random.Random, users, pid):
    op = rng.choice(["dep", "dep", "dep", "wd", "wd", "wdmax", "borrow", "borrow", "accrue", "adv", "repay",
                     "repay", "settle", "claim", "cover", "sweep", "wipe", "drain"])
    try:
        if op == "dep":
            if p.wipe_live and not p.wipe_swept and rng.random() < 0.6:
                p.sweep_sol(rng.choice([0, rng.randrange(1, 10**9)]))
            p.deposit(rng.choice(users), rng.choice([10**6, 10**9, 10**12, rng.randrange(1, 10**13)]))
        elif op in ("wd", "wdmax"):
            w = rng.choice(users)
            L = p.lenders.get(w)
            if L and L.shares:
                if op == "wd":
                    n = rng.randrange(1, L.shares + 1)
                else:
                    allow = copy.deepcopy(p).free_allowance(w)
                    n = max(1, min(L.shares, allow * (p.S + pm.VS) // (p.T + p.B + pm.VA)))
                p.withdraw(w, n)
        elif op == "borrow" and p.T > 0:
            pid[0] += 1
            p.borrow(pid[0], rng.randrange(1, p.T + 1), rng.choice([3600, 86400, 7 * 86400, 30 * 86400]))
        elif op == "accrue":
            p.advance(rng.randrange(0, 900), rng.randrange(0, 10**15))
        elif op == "adv":
            p.advance(rng.choice([60, 3600, 4 * 3600, 6 * 3600 + 1, 86400, 3 * 86400]))
        elif op == "repay" and p.positions:
            k = rng.choice(list(p.positions))
            q, io, _pr, _dl = p.positions[k]
            owed = pm.cdiv(q * (p.index - io), pm.WAD)
            p.repay(k, owed if rng.random() < 0.8 else rng.randrange(0, owed + 1), rng.choice([0, rng.randrange(0, 10**9)]))
        elif op == "settle" and p.positions:
            k = rng.choice(list(p.positions))
            sol_in = rng.randrange(0, 10**11)
            p.settle(k, sol_in, rng.randrange(0, sol_in + 1) if rng.random() < 0.4 else 0)
        elif op == "claim":
            p.claim_sol(rng.choice(users))
        elif op == "cover":
            w = rng.choice(users)
            if w in p.lenders:
                L = copy.deepcopy(p)._touch(w)
                if L.pending < 0:
                    p.cover(w, -L.pending)
        elif op == "sweep":
            p.sweep_sol(rng.randrange(1, 10**9))
        elif op == "wipe" and p.T > 0 and p.S > 0:
            # a full wipe: borrow up to the cap, the free rest is withdrawn, everything settles S1
            pid[0] += 1
            qty = p.T * p.util_cap // pm.BPS
            if qty > 0 and (p.B + qty) * pm.BPS <= p.util_cap * (p.T + p.B):
                p.borrow(pid[0], qty, 86400)
            for w in list(p.lenders):
                L = p.lenders[w]
                if L.shares and p.T:
                    allow = copy.deepcopy(p).free_allowance(w)
                    n = min(L.shares, allow * (p.S + pm.VS) // (p.T + p.B + pm.VA))
                    if n:
                        try:
                            p.withdraw(w, n)
                        except ValueError:
                            pass
            if p.T == 0:
                for k in list(p.positions):
                    p.settle(k, rng.randrange(0, 10**10), 0)
        elif op == "drain" and p.positions and p.T > 0:
            # most free tokens leave, then a position settles small: forces rebases (S >> T + B)
            for w in list(p.lenders):
                L = p.lenders[w]
                if L.shares:
                    allow = copy.deepcopy(p).free_allowance(w)
                    n = min(L.shares, allow * (p.S + pm.VS) // (p.T + p.B + pm.VA))
                    if n:
                        try:
                            p.withdraw(w, n)
                        except ValueError:
                            pass
            k = max(p.positions, key=lambda x: p.positions[x][0])
            p.settle(k, rng.randrange(0, 10**6), 0)
    except ValueError:
        pass


def main():
    rng = random.Random(SEED)
    cases = []
    skipped = 0
    for run_i in range(3):
        p = pm.Pool("continuous", util_cap_bps=9_000)
        users = [f"u{i}" for i in range(4)]
        pid = [0]
        wipe_seen = False
        for n in range(OPS):
            step(p, rng, users, pid)
            # every SAMPLE_EVERY ops, and once right after each full wipe (parked lenders, the
            # post-wipe deposit gate)
            fresh_wipe = p.wipe_live and not wipe_seen
            wipe_seen = p.wipe_live
            if n % SAMPLE_EVERY == SAMPLE_EVERY - 1 or fresh_wipe:
                try:
                    state = dump_state(p)
                except OutOfRange:
                    skipped += 1
                    continue
                cases.append({
                    "name": f"run{run_i}_op{n}",
                    "now": str(p.now),
                    "lockGraceS": p.lock_grace_s,
                    **state,
                    "expect": expectations(p, rng),
                })
    behind = sum(1 for c in cases for L in c["lenders"].values() if L["epoch"] < c["market"]["shareEpoch"])
    wipes = sum(1 for c in cases if c["market"]["flags"] & FLAG_WIPE_LIVE)
    locks = sum(1 for c in cases if c["vault"]["lockCal"])
    vests = sum(1 for c in cases if c["market"]["vestBal"] != "0")
    doc = {
        "generator": "apps/web/src/features/lend/vectors/gen_lend_vectors.py",
        "source": "docs/design/sim/pool_model.py v0.4.1 (default switches, continuous rent), lazy op mix",
        "seed": SEED,
        "stats": {"cases": len(cases), "skipped": skipped, "lendersBehind": behind, "wipeLive": wipes,
                  "withLocks": locks, "withVest": vests},
        "cases": cases,
    }
    OUT.write_text(json.dumps(doc, separators=(",", ":")) + "\n")
    print(json.dumps(doc["stats"]))


if __name__ == "__main__":
    main()
