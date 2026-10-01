#!/usr/bin/env python3
"""
short.win -- keeper cost model (ECONOMICS §5.8, R3B-5; O17).  Deterministic arithmetic on MEASURED
inputs (M7: ARCH §9 "keeper cost measurement ... feeding back into ECON §5.8"; U27 closed).

Inputs:
  * base fee 5,000 lamports per signature: mainnet getFeeForMessage, slot 450,897,283.
  * priority price scenarios (micro-lamports per CU): 0 / 1,000 / 10,000 / 100,000 (the O17 ceiling).
    Mainnet getRecentPrioritizationFees on pAMM, a PumpSwap pool and AMM v4 (slots 450,897,105-255):
    the per-slot MINIMUM landed fee was 0 in >= 75 % of slots, p90 0-1,943, max 1.31 M.
  * priority is paid on the REQUESTED limit, which the keeper sets to ceil(simulated x 1.1); a
    transaction costs 5,000 x signatures + ceil(requested CU x price / 10^6) (the keeper's fee model).
  * CU per instruction: the real keeper's own transactions, measured per venue by
    services/keeper-litesvm `m7_cost::m7_keeper_cost_inputs` (tests/bench/m7_keeper/results.json,
    `cu.*.requested_cu`, the costliest measured), in place of the v0.4 estimates 35 k / 90 k / 220 k.
    The attacker's LP-wrapped crank stays an estimate (~150 k CU, the M4G-2 reproducer's).
  * batching: k crank_price instructions per transaction (one signature).  Measured (T-SIZE, the
    keeper's builder): v1 (4,096 B, 64 inline addresses) holds 8 PumpSwap / 9 AMM v4 / 9 CPMM / 11
    curve cranks, and a batch of 6 fits every venue and a mix (the keeper's default `batch_v1` = 6);
    v0 through the protocol ALT holds 3 PumpSwap / 4 AMM v4 / 4 CPMM / 5 curve cranks (the fallback).
  * cadence: the keeper's own, measured (quiet: one crank per 30 s = 2,880 a day; near a threshold:
    one a second; an armed position 43 sol_settle calls; a foreign price deviation 116 cranks at the
    harness's 2 ticks a second, ~150 at 2.5 slots a second; an isolated LP-wrapped trigger 8 cranks
    above the control; a PumpSwap warm-up 8 cranks).
  * revenue: SPEC §6.1 rent at 80 % utilisation on the beta cap of 25 SOL open notional per market
    (ADOPTED Q21); protocol share 15 %.
  * O17 budget: `per_market_daily_lamports` 30,000,000 (0.03 SOL, PROPOSED default, O17's upper
    bound); the per-market budget is charged with the market's share of its crank transactions
    (a batch's fee split equally among its markets).

    python3 keeper_cost.py
"""
import json
import math
import os

DAY = 86_400
BASE = 5_000
LAMPORTS = 10**9
PRICES = (0, 1_000, 10_000, 100_000)
VENUES = ("pumpswap", "pump_curve", "raydium_cpmm", "raydium_amm_v4")
NAMES = {"pumpswap": "PumpSwap", "pump_curve": "curve", "raydium_cpmm": "CPMM", "raydium_amm_v4": "AMM v4"}
# Requested CU (ceil(consumed x 1.1)), the costliest measured per kind (tests/bench/m7_keeper/results.json).
CU = {
    "pumpswap": {"crank": 41_159, "sol_settle_keep": 64_119, "sol_settle_settle": 74_158,
                 "liquidate": 212_255, "expire": 208_348},
    "pump_curve": {"crank": 37_365, "sol_settle_keep": 62_700, "sol_settle_settle": 72_721,
                   "liquidate": 191_938, "expire": 188_919},
    "raydium_cpmm": {"crank": 32_826, "sol_settle_keep": 65_640, "sol_settle_settle": 77_388,
                     "liquidate": 125_282, "expire": 117_479},
    "raydium_amm_v4": {"crank": 32_762, "sol_settle_keep": 57_648, "sol_settle_settle": 67_713,
                       "liquidate": 105_268, "expire": 101_001},
}
LP_WRAP_CU = 150_000             # the attacker's [deposit, crank_price, withdraw] (estimate, M4G-2)
V1_K = 6                         # the keeper's `batch_v1`
V1_MAX = {"pumpswap": 8, "pump_curve": 11, "raydium_cpmm": 9, "raydium_amm_v4": 9}
V0_K = {"pumpswap": 3, "pump_curve": 5, "raydium_cpmm": 4, "raydium_amm_v4": 4}
QUIET_PER_DAY = DAY // 30        # 2,880
ARMED_SETTLES = 43
BURST_CRANKS = 116               # 150 at 2.5 slots a second
LIQ_TRIGGER_CRANKS = 8
LIQ_COVER_S = 10
BUDGET = 30_000_000              # per_market_daily_lamports (PROPOSED)
SET_COVERAGE_PAUSE_CU = 8_791
CRANK_SOL_USD_CU = 7_212
CRANK_SOL_USD_PER_DAY = 1_560    # a feed publishing every 55 s
RATE80 = {"A": 5_000, "B": 10_000, "C": 20_000, "D": 40_000}   # APR bps at 80 % utilisation


def tx_cost(cu_total, price, signatures=1):
    """What the keeper pays for one transaction: base fees plus priority on the requested limit."""
    return signatures * BASE + math.ceil(cu_total * price / 1_000_000)


def per_market_crank(venue, price, k=1):
    """One market's share of a batch of k cranks of `venue` (lamports)."""
    return tx_cost(k * CU[venue]["crank"], price) / k


def targeted_day(venue, price, k, f):
    """SOL per market-day: per-second cranks for a share f of the day, one per 30 s otherwise."""
    c = per_market_crank(venue, price, k)
    return DAY * (f * c + (1 - f) * c / 30) / LAMPORTS


def table(headers, rows):
    out = ["| " + " | ".join(headers) + " |", "|" + "|".join("---" for _ in headers) + "|"]
    out += ["| " + " | ".join(str(c) for c in r) + " |" for r in rows]
    return "\n".join(out)


def cross_check():
    """The constants above equal tests/bench/m7_keeper/results.json (when this runs in the repository)."""
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "tests", "bench",
                        "m7_keeper", "results.json")
    if not os.path.exists(path):
        return "results.json not found: constants not cross-checked"
    d = json.load(open(path))
    kinds = {"crank": "crank", "sol_settle_keep": "sol_settle_keep", "sol_settle_settle": "sol_settle_settle",
             "liquidate": "liquidate", "expire": "expire"}
    for v in VENUES:
        for mine, theirs in kinds.items():
            got = d["cu"][v][theirs]["requested_cu"][1]
            assert CU[v][mine] == got, (v, mine, CU[v][mine], got)
        assert V1_MAX[v] == d["crank_batches"][v]["v1_max"], v
        assert V0_K[v] == d["crank_batches"][v]["v0_protocol_alt_max"], v
        for i, price in enumerate(PRICES):
            for f_i, f in enumerate((0, 0.01, 0.05, 0.2, 1)):
                for key, k in (("unbatched", 1), ("v1_k6", V1_K), ("v0_alt", V0_K[v])):
                    want = d["cost_per_market_day_sol"][v][key][str(price)][f_i]
                    assert abs(targeted_day(v, price, k, f) - want) < 6e-6, (v, key, price, f)
    cad = d["cadence"]
    assert (cad["quiet_cranks_per_day"], cad["armed_sol_settle_calls_per_episode"], cad["price_burst_cranks_per_trigger"],
            cad["liquidity_trigger_extra_cranks"], cad["pumpswap_warmup_cranks"]) == \
        (QUIET_PER_DAY, ARMED_SETTLES, BURST_CRANKS, LIQ_TRIGGER_CRANKS, 8)
    g = d["global_lines"]
    assert g["set_coverage_pause"]["requested_cu"][1] == SET_COVERAGE_PAUSE_CU
    assert g["crank_sol_usd"]["requested_cu"][1] == CRANK_SOL_USD_CU
    assert g["crank_sol_usd_per_day"] == CRANK_SOL_USD_PER_DAY
    return "constants cross-checked against tests/bench/m7_keeper/results.json (" + d["builds"]["shortwin_so_sha256"][:8] + ")"


def main():
    note = cross_check()
    print(f"<!-- {note} -->\n")
    print("### Measured inputs: requested CU per instruction (ceil(simulated x 1.1), what the priority fee is paid on) "
          "and crank batches\n")
    rows = []
    for v in VENUES:
        c = CU[v]
        rows.append([NAMES[v], f"{c['crank']:,}", f"{c['sol_settle_keep']:,} / {c['sol_settle_settle']:,}",
                     f"{c['liquidate']:,} / {c['expire']:,}", V1_MAX[v], V0_K[v]])
    print(table(["venue", "crank_price", "sol_settle (keep / settles)", "liquidate / expire", "v1 max cranks",
                 "v0 + protocol ALT cranks"], rows))

    print("\n### Cost of continuous coverage: one crank per market per second (SOL per market per day)\n")
    rows = []
    for v in VENUES:
        for price in PRICES:
            cells = [NAMES[v], f"{price:,}"]
            for k in (1, V0_K[v], V1_K, V1_MAX[v]):
                cells.append(f"{per_market_crank(v, price, k) * DAY / LAMPORTS:.4f}")
            rows.append(cells)
    print(table(["venue", "priority price (uL/CU)", "unbatched (k = 1)", "v0 + ALT (venue's k)", "v1 k = 6",
                 "v1 max k"], rows))

    print("\n### Targeted coverage (duties 1-2): per-second cranks for a share f of the day, else one per 30 s; "
          "v1 k = 6 (unbatched in brackets); SOL per market per day\n")
    rows = []
    for v in VENUES:
        for f in (0.0, 0.01, 0.05, 0.20, 1.0):
            cells = [NAMES[v], f"{100 * f:g} %"]
            for price in PRICES:
                cells.append(f"{targeted_day(v, price, V1_K, f):.4f} ({targeted_day(v, price, 1, f):.4f})")
            rows.append(cells)
    print(table(["venue", "f", "0 uL/CU", "1,000 uL/CU", "10,000 uL/CU", "100,000 uL/CU"], rows))

    print("\n### Episode costs (unbatched, the costliest venue: PumpSwap; SOL per episode)\n")
    rows = []
    v = "pumpswap"
    for price in PRICES:
        armed = ARMED_SETTLES * tx_cost(CU[v]["sol_settle_keep"], price)
        burst = BURST_CRANKS * tx_cost(CU[v]["crank"], price)
        liq = tx_cost(CU[v]["liquidate"], price) + 1_000_000        # + a Jito tip of 0.001 SOL (assumption)
        warm = 8 * tx_cost(CU[v]["crank"], price)
        rows.append([f"{price:,}", f"{armed / LAMPORTS:.5f}", f"{burst / LAMPORTS:.5f}", f"{liq / LAMPORTS:.5f}",
                     f"{warm / LAMPORTS:.5f}"])
    print(table(["priority price (uL/CU)", f"one armed position ({ARMED_SETTLES} x sol_settle)",
                 f"one price deviation ({BURST_CRANKS} x crank)", "one liquidation (+0.001 SOL tip)",
                 "a PumpSwap warm-up (8 cranks)"], rows))

    print("\n### Global lines (the protocol, not a market's budget; SOL)\n")
    rows = []
    for price in PRICES:
        sol_usd = CRANK_SOL_USD_PER_DAY * tx_cost(CRANK_SOL_USD_CU, price)
        pause = tx_cost(SET_COVERAGE_PAUSE_CU, price, signatures=2)
        rows.append([f"{price:,}", f"{sol_usd / LAMPORTS:.4f}", f"{pause / LAMPORTS:.8f}"])
    print(table(["priority price (uL/CU)", f"crank_sol_usd, a day ({CRANK_SOL_USD_PER_DAY:,} calls)",
                 "set_coverage_pause, one call (two signatures)"], rows))

    print("\n### Liquidity-anomaly coverage (duty 2, #101): the keeper's cost against the attacker's (SOL; a "
          f"market with open positions; per-second coverage for liq_cover_s = {LIQ_COVER_S} s, measured "
          f"{LIQ_TRIGGER_CRANKS} cranks above the control per isolated trigger; PumpSwap cranks)\n")
    rows = []
    for price in PRICES:
        att = tx_cost(LP_WRAP_CU, price)
        k1 = tx_cost(CU[v]["crank"], price)
        k6 = per_market_crank(v, price, V1_K)
        rows.append([f"{price:,}", f"{att / LAMPORTS:.7f}", f"{LIQ_TRIGGER_CRANKS * k1 / LAMPORTS:.6f}",
                     f"{LIQ_TRIGGER_CRANKS * k1 / att:.1f}x / {LIQ_TRIGGER_CRANKS * k6 / att:.1f}x",
                     f"{k1 / att:.2f}x / {k6 / att:.2f}x"])
    print(table(["priority price (uL/CU)", "attacker: one LP-wrapped crank",
                 f"keeper: one isolated trigger ({LIQ_TRIGGER_CRANKS} cranks)",
                 "isolated trigger, keeper / attacker (unbatched / k = 6)",
                 "sustained, per second, keeper / attacker (unbatched / k = 6)"], rows))

    print("\n### The O17 budget (0.03 SOL per market-day): what exhausts it (OWNER-M7-2). Triggers every 11 s on top "
          "of the quiet cadence of a market with a healthy position; the day's budget then puts the market in "
          "degraded mode (opens paused until the UTC day rolls over + 600 s; exits continue). LP-wrapped "
          "triggers need a pool with LP accounting: PumpSwap and Raydium only (the pump curve has none; M7-SEC-2)\n")
    rows = []
    for vv in VENUES:
        if vv == "pump_curve":
            continue
        for price in PRICES:
            cells = [NAMES[vv], f"{price:,}"]
            for k in (1, V1_K):
                crank = per_market_crank(vv, price, k)
                quiet = QUIET_PER_DAY * crank
                left = BUDGET - quiet
                if left <= 0:
                    cells.append("quiet cadence alone exceeds the budget")
                    continue
                n = math.ceil(left / (LIQ_TRIGGER_CRANKS * crank))   # the trigger that spends the last lamport
                att = n * tx_cost(LP_WRAP_CU, price)
                cells.append(f"{n:,} triggers, {n * 11 / 3600:.1f} h, attacker {att / LAMPORTS:.4f} SOL")
            cells.append(f"{QUIET_PER_DAY * per_market_crank(vv, price, V1_K) / BUDGET * 100:.0f} % / "
                         f"{QUIET_PER_DAY * per_market_crank(vv, price, 1) / BUDGET * 100:.0f} %")
            rows.append(cells)
    print(table(["venue", "priority price (uL/CU)", "unbatched keeper", "batched keeper (k = 6)",
                 "quiet day's share of the budget (k = 6 / unbatched)"], rows))

    print("\n### Price bursts (duty 2) after M7-SEC-2: a burst needs a reverted >= 2 % excursion, a price round "
          f"trip of ~1 % of the token reserve (2 phi delta of fees), against the keeper's {BURST_CRANKS} cranks "
          "(PumpSwap cranks; SOL)\n")
    rows = []
    for depth in (100, 1_000):
        for phi_bps in (25, 100):
            att = 2 * phi_bps / 10_000 * 0.0099 * depth * LAMPORTS + BASE
            cells = [f"{depth:,} SOL", f"{phi_bps} bps", f"{att / LAMPORTS:.5f}"]
            for price in (0, 100_000):
                keeper = BURST_CRANKS * tx_cost(CU["pumpswap"]["crank"], price)
                n = math.ceil((BUDGET - QUIET_PER_DAY * tx_cost(CU["pumpswap"]["crank"], price)) / keeper)
                cells.append(f"{keeper / LAMPORTS:.5f} ({att / keeper:.1f}x; {n} bursts = {n * att / LAMPORTS:.3f} SOL"
                             " to spend the day's budget)")
            rows.append(cells)
    print(table(["pool SOL reserve", "fee one way", "attacker: one round trip",
                 "keeper unbatched at 0 uL/CU (attacker / keeper; budget)",
                 "keeper unbatched at 100,000 uL/CU (attacker / keeper; budget)"], rows))
    print("\n(As built at M7, before M7-SEC-2: one plain crank_price after anyone's honest >= 2 % move, "
          "5,000 lamports, started the same burst.)")

    print("\n### Revenue of a market at the beta cap (25 SOL open notional, 80 % utilisation; SOL per day)\n")
    rows = []
    for tier, apr in RATE80.items():
        rent = 25 * apr / 10_000 / 365
        rows.append([tier, f"{apr / 100:.0f} %", f"{rent:.4f}", f"{0.15 * rent:.4f}", f"{0.85 * rent:.4f}"])
    print(table(["tier", "rent APR at 80 %", "rent (all)", "protocol share (15 %)", "lender share (85 %)"], rows))

    print("\n### Break-even: open notional per market at which the protocol's rent share pays for continuous "
          "per-second coverage (v1 k = 6, PumpSwap cranks)\n")
    rows = []
    for price in (1_000, 10_000, 100_000):
        cost = per_market_crank("pumpswap", price, V1_K) * DAY / LAMPORTS
        cells = [f"{price:,}", f"{cost:.4f}"]
        for tier, apr in RATE80.items():
            cells.append(f"{cost / (0.15 * apr / 10_000 / 365):,.0f} SOL")
        rows.append(cells)
    print(table(["priority price (uL/CU)", "coverage cost (SOL/day)", "A", "B", "C", "D"], rows))


if __name__ == "__main__":
    main()
