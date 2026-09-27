"""R3B-7(a) / R3A-11(d): ring liquidity across a venue change.

At graduation the curve's sqrt(vT*vQ) (~5.7e12) is ~1.35x the new PumpSwap pool's (~4.2e12);
a Raydium fast-path switch may land on a pool with a quarter of the depth.  v0.3's ARCH text
carried every ring entry's L and lobs over; v0.4 (like the sims) rewrites them to the reset
liquidity.  Compared here: outcome of genuine moves after the change, and the number of seconds
after it during which the settlement box is wider than fv_dev_bps in L (which blocks any S2/S3
amount that depends on it).
"""
import sys
from common import *

S = sys.modules["shortwin_sim"]


class CarryOver(Market):
    def _rewrite_ring_liq(self, L):
        self.b_sec = None                    # bucket cleared, ring L / lobs carried over (v0.3 ARCH text)


def mk(key, carry):
    cls = CarryOver if carry else Market
    return cls(MARKETS[key](), tier_of(key), DEFAULT)


def box_wide_seconds(m, t0, t1):
    wide = 0
    for t in range(t0, t1):
        m.step_to(t)
        P_lo, P_hi, L_lo, L_hi = m.settle_box(t)
        if (L_hi - L_lo) * BPS > DEFAULT.fv_dev_bps * L_lo:
            wide += 1
    return wide


if __name__ == "__main__":
    print("| market | scenario | ring at the venue change | outcome | exit vs event | price at exit | lender P&L | "
          "seconds with the settlement box wider than 1 % in L (first 60 s after the change) |")
    print("|---|---|---|---|---|---|---|---|")
    cases = [("D@90%", S.GRAD_SCENARIOS[1]), ("D@90%", S.GRAD_SCENARIOS[2]), ("D@97%", S.GRAD_SCENARIOS[1]),
             ("RA", S.FAST_SCENARIOS[1]), ("RB", S.FAST_SCENARIOS[1]), ("RC", S.FAST_SCENARIOS[1])]
    for key, sc in cases:
        for carry in (True, False):
            m = mk(key, carry)
            pos, r, P1 = run(m, 1.0, sc)
            s = summarize(pos, r, P1)
            # box width right after the change, on a fresh run without the position's exit
            m2 = mk(key, carry)
            pos2 = m2.open(m2.max_open_q(), 0)
            t, ev = 0, sorted(sc.events, key=lambda e: e[0])
            ei, changed = 0, None
            venue0 = id(m2.venue)
            while t < sc.t_event + 400 and changed is None:
                t += 1
                while ei < len(ev) and ev[ei][0] <= t:
                    ev[ei][1](m2)
                    ei += 1
                m2.step_to(t)
                if id(m2.venue) != venue0:
                    changed = t
            wide = box_wide_seconds(m2, changed + 1, changed + 61) if changed else "n/a"
            print(f"| {key} | {sc.name} | {'carried over (v0.3 ARCH)' if carry else 'rewritten (v0.4.1)'} | {s['kind']} | "
                  f"{s['t'] - sc.t_event:+d}s | {s['mult']:.2f}x | {s['lender_pnl_pct']:+.1f}% | {wide} |")
