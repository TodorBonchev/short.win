#!/usr/bin/env python3
"""
short.win -- keeper cost model (ECONOMICS §5.8, R3B-5).  Deterministic arithmetic.

Inputs (primary sources where available, else labelled estimates):
  * base fee 5,000 lamports per signature: mainnet getFeeForMessage, slot 450,897,283.
  * priority price scenarios (micro-lamports per CU): 0 / 1,000 / 10,000 / 100,000.  Mainnet
    getRecentPrioritizationFees on pAMM, a PumpSwap pool and AMM v4 (slots 450,897,105-255):
    the per-slot MINIMUM landed fee was 0 in >= 75 % of slots, p90 0-1,943, max 1.31 M.
  * CU per instruction: ARCH §6.1 estimates (UNVERIFIED until T-CU): crank_price 35 k,
    sol_settle 90 k, liquidate 220 k.
  * batching: k crank_price instructions per transaction (one signature).  v0.4.1 (R4B-9): a v0
    transaction (1,232 B) with the protocol ALT fits only k <= 4 PumpSwap / CPMM cranks (6 inline
    keys each: ~209 B + 213 B per market) or k <= 5 curve cranks; keeper batches are v1
    transactions (4,096 B, 64 inline addresses, no ALT): ~6 shared keys + 6 per market, so
    k <= 9; the tables use k = 6 (v1) and show k = 4 (v0 fallback).
  * revenue: SPEC §6.1 rent at 80 % utilisation on the beta cap of 25 SOL open notional per
    market (ADOPTED Q21); protocol share 15 %.
  * liquidity-anomaly coverage (duty 2, #101, FITS-2 (b)): on a market with open positions a foreign
    bucket whose liquidity (llo / lhi) deviates from the keeper's own sqrt(X*Y) by more than
    burst_dev_bps starts per-SECOND coverage of the next liq_cover_s (10) seconds, extended while
    deviations recur; the attacker's trigger is one LP-wrapped crank, [deposit, crank_price, withdraw]
    (~150 k CU, the M4G-2 reproducer's estimate).  Sustained wrapping costs both sides one transaction
    a second; an isolated trigger buys liq_cover_s keeper cranks.  M4F-1's reverted liquidity bursts
    (a 60-s per-SLOT burst, 150 cranks, on any market) are shown for comparison (M4G-2).

    python3 keeper_cost.py
"""
DAY = 86_400
BASE = 5_000
LAMPORTS = 10**9
CU = {"crank_price": 35_000, "sol_settle": 90_000, "liquidate": 220_000, "lp_wrap": 150_000}
LIQ_COVER_S = 10       # duty 2 (#101): per-second coverage after a foreign liquidity deviation (liq_cover_s)
BURST_SLOTS = 150      # M4F-1's reverted liquidity burst: every slot for 60 s
PRICES = (0, 1_000, 10_000, 100_000)
RATE80 = {"A": 5_000, "B": 10_000, "C": 20_000, "D": 40_000}   # APR bps at 80 % utilisation


def tx_cost(n_ix, cu_each, price, signatures=1):
    return signatures * BASE + n_ix * cu_each * price // 1_000_000


def per_market_second(price, k=1):
    return tx_cost(k, CU["crank_price"], price) / k


def table(headers, rows):
    out = ["| " + " | ".join(headers) + " |", "|" + "|".join("---" for _ in headers) + "|"]
    out += ["| " + " | ".join(str(c) for c in r) + " |" for r in rows]
    return "\n".join(out)


def main():
    print("### Cost of continuous coverage: one crank per market per second (SOL per market per day)\n")
    rows = []
    for price in PRICES:
        rows.append([f"{price:,}"] + [f"{per_market_second(price, k) * DAY / LAMPORTS:.4f}" for k in (1, 4, 6, 9)])
    print(table(["priority price (micro-lamports/CU)", "unbatched (k = 1)", "4 per v0 tx (ALT)", "6 per v1 tx",
                 "9 per v1 tx (64-address limit)"], rows))
    print("\n### Transaction size of a crank batch (bytes; v0 limit 1,232 with the protocol ALT, v1 limit 4,096 "
          "and 64 inline addresses)\n")
    rows = []
    for k in (3, 4, 5, 6, 9):
        v0_ps = 209 + 213 * k
        v0_cv = 209 + 180 * k
        keys_v1 = 6 + 6 * k
        v1 = 65 + 3 + 1 + 32 * keys_v1 + 32 + 1 + 20 + k * 21
        rows.append([k, f"{v0_ps:,}{' (too large)' if v0_ps > 1232 else ''}", f"{v0_cv:,}{' (too large)' if v0_cv > 1232 else ''}",
                     f"{v1:,} B, {keys_v1} keys{' (too many keys)' if keys_v1 > 64 else ''}"])
    print(table(["k", "v0 PumpSwap / CPMM", "v0 curve", "v1 (all keys inline)"], rows))

    print("\n### Targeted coverage (ECON §5.8 duties): per-second cranks only for a fraction f of the day, "
          "else one crank per 30 s; batched k = 6 (v1 transactions; the v0 fallback k = 4 costs 1.5x the base-fee part); "
          "SOL per market per day\n")
    rows = []
    for f in (0.0, 0.01, 0.05, 0.20, 1.0):
        cells = [f"{100 * f:g} %"]
        for price in (1_000, 10_000, 100_000):
            c = per_market_second(price, 6)
            cells.append(f"{DAY * (f * c + (1 - f) * c / 30) / LAMPORTS:.4f}")
        rows.append(cells)
    print(table(["share of the day near a threshold (f)", "1,000 uL/CU", "10,000 uL/CU", "100,000 uL/CU"], rows))

    print("\n### Event costs (unbatched; SOL per event)\n")
    rows = []
    for price in (1_000, 10_000, 100_000):
        armed = 45 * tx_cost(1, CU["sol_settle"], price)          # one sol_settle per second until S1/S2 (>= 40 updates)
        burst = 150 * tx_cost(1, CU["crank_price"], price)         # crank every slot for 60 s after a deviating observation
        liq = tx_cost(1, CU["liquidate"], price) + 1_000_000       # + a Jito tip of 0.001 SOL (assumption)
        rows.append([f"{price:,}", f"{armed / LAMPORTS:.5f}", f"{burst / LAMPORTS:.5f}", f"{liq / LAMPORTS:.5f}"])
    print(table(["priority price (uL/CU)", "one armed position (45 x sol_settle)", "one deviation burst (150 slots x crank)",
                 "one liquidation (+0.001 SOL tip)"], rows))

    print("\n### Liquidity-anomaly coverage (duty 2, #101): the keeper's cost against the attacker's (SOL; a market with "
          "open positions; per-second coverage for liq_cover_s = 10 s after a foreign bucket liquidity deviation, "
          "extended while they recur)\n")
    rows = []
    for price in (0, 1_000, 10_000, 100_000):
        att = tx_cost(1, CU["lp_wrap"], price)                       # one [deposit, crank_price, withdraw]
        k1 = tx_cost(1, CU["crank_price"], price)                     # one keeper crank, unbatched
        k6 = per_market_second(price, 6)                              # batched k = 6 (v1)
        old = BURST_SLOTS * k1                                        # M4F-1's per-slot burst per trigger
        rows.append([f"{price:,}", f"{att / LAMPORTS:.7f}", f"{LIQ_COVER_S * k1 / LAMPORTS:.6f}",
                     f"{LIQ_COVER_S * k1 / att:.1f}x / {LIQ_COVER_S * k6 / att:.1f}x",
                     f"{k1 / att:.2f}x / {k6 / att:.2f}x", f"{old / LAMPORTS:.5f} ({old / att:.0f}x)"])
    print(table(["priority price (uL/CU)", "attacker: one LP-wrapped crank", "keeper: one isolated trigger (10 cranks)",
                 "isolated trigger, keeper / attacker (unbatched / k = 6)",
                 "sustained, per second, keeper / attacker (unbatched / k = 6)",
                 "M4F-1's reverted per-slot burst per trigger (keeper / attacker)"], rows))
    print("\n### The same per market-day: an attacker that keeps the keeper busiest per transaction (one trigger every "
          "liq_cover_s + 1 s, the window lapsing in between), one a minute, and one every second (unbatched keeper; "
          "SOL per market per day, keeper / attacker)\n")
    rows = []
    for price in (0, 10_000, 100_000):
        att = tx_cost(1, CU["lp_wrap"], price)
        k1 = tx_cost(1, CU["crank_price"], price)
        cells = [f"{price:,}"]
        for n_trig, secs in ((DAY // (LIQ_COVER_S + 1), LIQ_COVER_S), (DAY // 60, LIQ_COVER_S), (DAY, 1)):
            cells.append(f"{n_trig * secs * k1 / LAMPORTS:.4f} / {n_trig * att / LAMPORTS:.4f}")
        cells.append(f"{DAY // 60 * BURST_SLOTS * k1 / LAMPORTS:.4f} / {DAY // 60 * att / LAMPORTS:.4f}")
        rows.append(cells)
    print(table(["priority price (uL/CU)", "every 11 s", "every 60 s", "every second (sustained, 1:1)",
                 "M4F-1's burst, one trigger a minute"], rows))

    print("\n### Revenue of a market at the beta cap (25 SOL open notional, 80 % utilisation; SOL per day)\n")
    rows = []
    for tier, apr in RATE80.items():
        rent = 25 * apr / 10_000 / 365
        rows.append([tier, f"{apr / 100:.0f} %", f"{rent:.4f}", f"{0.15 * rent:.4f}", f"{0.85 * rent:.4f}"])
    print(table(["tier", "rent APR at 80 %", "rent (all)", "protocol share (15 %)", "lender share (85 %)"], rows))

    print("\n### Break-even: open notional per market at which the protocol's rent share pays for continuous "
          "per-second coverage (k = 6)\n")
    rows = []
    for price in (1_000, 10_000):
        cost = per_market_second(price, 6) * DAY / LAMPORTS
        cells = [f"{price:,}", f"{cost:.4f}"]
        for tier, apr in RATE80.items():
            cells.append(f"{cost / (0.15 * apr / 10_000 / 365):,.0f} SOL")
        rows.append(cells)
    print(table(["priority price (uL/CU)", "coverage cost (SOL/day)", "A", "B", "C", "D"], rows))


if __name__ == "__main__":
    main()
