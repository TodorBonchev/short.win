"""M6R-1 and M6-SEC-1 (the owner's decisions of 2026-09-30; ECONOMICS.md #105, §1.2, §5.2, §6.1, §8.2, §10.1,
§12.12 (12); ARCH §2.5, §3.4, §4.6, §4.7): third-party liquidity on canonical PumpSwap pools, and the graduation's
post-change warm-up price seed.

The facts (R-VEN Update H11): the LP a canonical PumpSwap pool mints at creation is burned by migrate_v2, but anyone
can deposit into the pool and withdraw again -- boost vault funded or not -- and VQR scales with the deposit
(floor(VQR*base_in/base)); a deposit or withdrawal moves sqrt(X*(Y + VQR)) without moving the price and can be wrapped
around another instruction, exactly as on Raydium.  The graduation (M6 as built, ECONOMICS.md #104) reads the pool's
lp_supply as its anchor, and its post-change warm-up re-seeds P_ema from the pool's own bucket lows.

The rules compared (Market subclasses; "#105" is the sim's Market, the owner's decisions as the sims build them):
  "M6 as built" (Built104): no locked depth on PumpSwap -- the caps' X0 = min(X_now, L_slow/sqrt(P_ema)) without the
      D term, no cap on the exit reference liquidity, the graduation's L_slow capped at the lp_supply anchor only --
      and the warm-up price seed bounded upward only (M4X-1);
  "#105": M6R-1 -- D = floor(lambda_bps*isqrt(X*(Y + VQR))/1e4), lambda_bps = floor((lp_supply - lp_mint.supply)*1e4/
      lp_supply) (the burned LP's depth), the caps' X0 also <= D/(lambda_min*sqrt(P_ema)), the reference's EMA terms
      capped at floor(D*1e4/lambda_min), and L_slow capped at D during a PumpSwap warm-up (Market._warmup_slow_cap:
      the first PumpSwap observation after the transition) -- and M6-SEC-1 -- the seed falls by at most the largest
      v0.4.1 step down per integration (x11/12 at dt = 1);
  ablations: "M6-SEC-1 only" (NoD: #105 without the D rules) and "M6R-1 only" (NoDown: #105 with the seed unbounded
      downward); the historical spot leg at L_live (Spot100, before FITS-2 (a), #101); and OWNER-M6D-1's option (a)
      (LamPamm, an analysis rule, not built: lambda_min := 100 % in PumpSwap sizing only, so the caps' D term reads
      D itself).
The spot leg at max(L_live, L_ema') (FITS-2 (a)) and the reference guard were venue-independent already; the keeper's
per-second liquidity-anomaly coverage (FITS-2 (b); switch_warmup.Coverage "new") is the keeper's, on any venue.
Parts:
  1  M6-SEC-1: one low recorded in a graduation's warm-up second (a [sell, crank_price, buy] next to the keeper's
     crank), on ECON §10.1's rows; the undisturbed exit, the as-built delay and #105's; a genuine dump right after the
     transition (nothing is forced when the seed lags a fall).
  2  M6R-1 at the graduation: a deposit between migrate_v2 and the transition crank raises the lp_supply anchor;
     L_slow after the transition and after the first PumpSwap observation, the carried position's exit, a new open's
     cap after the boost window.
  3  M6R-1 in steady state on a PumpSwap pool (market C: the fresh graduate after its boost, Tier C): (a) a deposit
     held k x the pool for H and pulled at a genuine pump (the reference cap); (b) an open while the deposit is held
     (sizing: the D term); (c) a deposit wrapped around the open (sizing and margin are not inflatable in one
     transaction, under either rule); (d) a pool with 50 % third-party LP: the withdraw-wrapped liquidation and the
     withdraw drag under the keeper's coverage, #100's spot leg at L_live against the rule as adopted (FITS-2);
     (e) M6D-1: (b)'s held deposit sizes the open (up to lambda/lambda_min = 2x the honest cap under #105) and
     leaves in the second a genuine pump starts -- L_slow (tau 1 h) keeps the reference at the cap, every
     liquidation fails its exit bound and the position gaps into S1 (lenders below Q's value, the borrower zeroed);
     option (a) -- lambda_min = 100 % in PumpSwap sizing -- for the owner.
Deterministic; standard library only.

    python3 pumpswap_lp.py            # every part (redirect to ../out/attacks/pumpswap_lp.out)
    python3 pumpswap_lp.py 1 3        # some parts
"""
import copy
import sys
from dataclasses import replace

from common import *
import switch_warmup as SW


# ------------------------------------------------------------------------------------------------------------
# Rules
# ------------------------------------------------------------------------------------------------------------
def _warmup_unbounded_down(self, t):
    """Market._integrate_warmup before M6-SEC-1 (M4R with M4X-1): P_ema = min(post-change lows, the step up)."""
    if t > self.ema_t:
        T = self.tier
        k = EMA_WARMUP_BUCKETS - self.warm
        ents = self.ring[len(self.ring) - k:] if k else []
        dt = min(t - self.ema_t, T.ema_dtmax_s)
        up = (self.ema * T.ema_tau_s + self.ema * (BPS + T.ema_clamp_bps) // BPS * dt) // (T.ema_tau_s + dt)
        p = min([e[5] for e in ents] + [self.b_lo, up])
        H = sorted([e[8] for e in ents] + [self.b_lhi])
        n = len(H)
        l = min(H[n - 1], H[(n - 1) // 2] * (BPS + T.ema_clamp_bps) // BPS)
        self.ema, self.liq = p, l
        self.ema_t = t
        self.ema_count += 1
        self.warm -= 1
        self.ring.append(self._entry(t, p, l, self.b_lo, self.b_hi, self.b_llo, self.b_lhi))
        if len(self.ring) > self.P.ring_len:
            del self.ring[0]
        self._guard_check()
    self.lb_sec, self.lb_lo, self.lb_hi = self.b_sec, self.b_lo, self.b_hi
    self.b_sec = None


class NoD(Market):
    """#105 without M6R-1's locked-depth rules on PumpSwap (the D term of the caps, the reference cap, the warm-up's
    L_slow cap): M6 as built plus M6-SEC-1."""

    def locked_depth_venue(self):
        return self.venue.kind == "ray"

    def _warmup_slow_cap(self):
        pass


class NoDown(Market):
    """#105 without M6-SEC-1: the warm-up seed falls to the lowest post-change bucket low at once (M6 as built plus
    M6R-1)."""
    _integrate_warmup = _warmup_unbounded_down


class Built104(NoD):
    """M6 as built (ECONOMICS.md #104): neither M6R-1 nor M6-SEC-1."""
    _integrate_warmup = _warmup_unbounded_down


class Spot100(SW.Spot100):
    """#100's liquidation spot leg at L_live (before FITS-2 (a), #101) with #105's other rules."""


class LamPamm(Market):
    """OWNER-M6D-1's option (a), an analysis rule (not built): lambda_min := 100 % on a canonical PumpSwap pool's
    sizing -- the caps' X0 <= D/sqrt(P_ema), the burned LP's depth itself (a canonical pool's lock is its burned
    creation LP, so lambda_min has no eligibility role there); Raydium keeps pool_lock_min_bps.  The program's tier
    bound would read D the same way (the sim's tiers are fixed).  The reference cap keeps floor(D*1e4/lambda_min):
    capping it at D as well gives every part 3 (e) row the same exit second and price (checked; the lenders' P&L
    within 0.1 point, the borrower's share within 1), and the cap at D/lambda_min keeps V2-14's bound on a
    withdraw-wrapped due liquidation."""

    def conservative_x(self):
        v = self.venue
        if not (isinstance(v, CPPool) and v.canonical):
            return super().conservative_x()
        P = self.P
        self.P = replace(P, pool_lock_min_bps=BPS)
        try:
            return super().conservative_x()
        finally:
            self.P = P


RULES = {"built": Built104, "nod": NoD, "nodown": NoDown, "r105": Market, "s100": Spot100, "lam100": LamPamm}
RULE_NAME = {"built": "M6 as built (#104)", "nod": "M6-SEC-1 only", "nodown": "M6R-1 only", "r105": "#105",
             "s100": "#100's spot leg at L_live",
             "lam100": "sizing at D on PumpSwap (OWNER-M6D-1 (a), analysis)"}


def hdr(cols):
    print("| " + " | ".join(cols) + " |")
    print("|" + "---|" * len(cols))


def row(cells):
    print("| " + " | ".join(str(c) for c in cells) + " |")


# ------------------------------------------------------------------------------------------------------------
# PumpSwap LP moves (R-VEN Update H11: both vaults, VQR and lp_supply scale; the burned creation LP stays)
# ------------------------------------------------------------------------------------------------------------
def unlocked_bps(v):
    return (v.lp_units - v.lp_locked) * BPS // v.lp_units if v.lp_units else 0


def withdraw_unlocked(v):
    """Every third-party LP unit out (floor): sqrt(X*(Y + VQR)) falls to about D."""
    u = unlocked_bps(v)
    if u > 0:
        v.remove_liquidity(u)


def wrapped(v, move):
    """The pool one instruction sees inside an LP round trip: 'wd' = every third-party LP unit withdrawn, 'depK' = K x
    the pool deposited (price unchanged)."""
    if move is None:
        return v
    w = copy.deepcopy(v)
    if move == "wd":
        withdraw_unlocked(w)
    elif move.startswith("dep"):
        w.add_liquidity(BPS * int(move[3:] or "1"))
    else:
        raise ValueError(move)
    return w


def restore(v, w):
    """After an instruction ran on the wrapped copy w of v (possibly trading), the round trip closes: the LP moved out
    (in) goes back (out) at the new reserve ratio (VQR with it)."""
    if w is v:
        return
    v.X = w.X * v.lp_units // w.lp_units
    v.Y = w.Y * v.lp_units // w.lp_units
    v.vq = w.vq * v.lp_units // w.lp_units


def obs_wrapped(m, t, move):
    v = m.venue
    w = wrapped(v, move)
    m.advance(t)
    m.venue = w
    m.observe(t)
    m.venue = v
    restore(v, w)


def try_liq(m, pos, t, move=None):
    v = m.venue
    w = wrapped(v, move)
    m.venue = w
    r = m.try_exit(pos, t)
    m.venue = v
    restore(v, w)
    return r


def sell_wrapped_obs(m, t, f):
    """[sell to f x the spot, crank_price, buy the same tokens back] in second t (the attacker's low); returns the
    round-trip cost (lamports)."""
    v = m.venue
    m.advance(t)
    d = -move_size(v, int(v.spot_wad() * f))
    if v.vq > 0:
        d = min(d, v.X * v.Y // v.vq * 99 // 100)      # a payout stays within the real quote reserve
    if d <= 0:
        m.observe(t)
        return 0
    got = v.sell(d)
    m.observe(t)
    return v.buy(d) - got


def exit_cell(r, t0, p1):
    if r is None or r["kind"] == "open":
        return "open"
    return f"{r['kind']} +{r['t'] - t0}s @{r['spot'] / p1:.2f}x"


def lender_cell(pos, r):
    if r is None or r["kind"] == "open":
        return "-"
    return f"{r['tokens'] / pos.Q:.0%} + {sol(r['lender_sol']):.4f}"


# ------------------------------------------------------------------------------------------------------------
# Part 1: M6-SEC-1
# ------------------------------------------------------------------------------------------------------------
T_GRAD = TE + DEFAULT.migration_delay_s        # the transition second of GRAD_SCENARIOS (the keeper's crank)


def grad_run(rule, key, sc, events=(), size=1.0):
    m = RULES[rule](MARKETS[key](), tier_of(key), DEFAULT)
    pos, r, p1 = run(m, size, replace(sc, events=tuple(sc.events) + tuple(events)))
    return m, pos, r, p1


def low_event(t, f, cost):
    def ev(m):
        if isinstance(m.venue, CPPool):
            cost.append(sell_wrapped_obs(m, t, f))
    return (t, ev)


def seed_at_gate(rule, key, sc, events):
    """P_ema / the pool's spot at the 8th warm-up integration (the gate), no position exit (size tiny)."""
    m = RULES[rule](MARKETS[key](), tier_of(key), DEFAULT)
    pos = m.open(max(1, m.max_open_q() // 1000), 0)
    evs = sorted(tuple(sc.events) + tuple(events), key=lambda e: e[0])
    ei = 0
    for t in range(1, T_GRAD + 12):
        while ei < len(evs) and evs[ei][0] <= t:
            evs[ei][1](m)
            ei += 1
        m.step_to(t)
        if t > T_GRAD and m.warm == 0:
            return m.ema / m.venue.spot_wad(), t - T_GRAD
    return None, None


def part1():
    print("## 1. M6-SEC-1: one low in a graduation's warm-up second")
    print()
    print("ECON §10.1's graduation rows (migration 2 s after the completion, the keeper cranking every second): in the")
    print(f"k-th second after the transition (k = 1: the first post-change bucket) the attacker sends [sell to f x the")
    print("pool's spot, crank_price, buy back] next to the keeper's crank, so that bucket's low is f x the price.  As built")
    print("the warm-up seed is the lowest post-change bucket low (bounded upward only), so one such low sets P_ema for the")
    print("rest of the warm-up and the EMA leg then climbs back at most one v0.4.1 step (x7/6) a second; with M6-SEC-1")
    print("the seed also falls by at most one step (x11/12) per integration.  Cells: the exit (seconds after the")
    print("completion, price), lenders' tokens back + SOL; the attacker's round trip (SOL, fees at the pool's tier).")
    print()
    hdr(["market", "scenario", "attack", "M6 as built (#104)", "#105", "attacker round trip (SOL)"])
    cases = [("D@80%", GRAD_SCENARIOS[0]), ("D@90%", GRAD_SCENARIOS[2]), ("D@97%", GRAD_SCENARIOS[1])]
    for key, sc in cases:
        base = {}
        for rule in ("built", "r105"):
            m, pos, r, p1 = grad_run(rule, key, sc)
            base[rule] = (pos, r, p1)
        row([key, sc.name, "none (honest)"] +
            [f"{exit_cell(base[x][1], TE, base[x][2])}; {lender_cell(base[x][0], base[x][1])}" for x in ("built", "r105")]
            + ["-"])
        for f in (0.5, 0.25):
            for k in (1, 4, 8):
                cells, costs = [], []
                for rule in ("built", "r105"):
                    cost = []
                    m, pos, r, p1 = grad_run(rule, key, sc, (low_event(T_GRAD + k, f, cost),))
                    cells.append(f"{exit_cell(r, TE, p1)}; {lender_cell(pos, r)}")
                    costs.append(cost[0] if cost else 0)
                row([key, sc.name, f"low at {f:g}x in second {k}"] + cells + [f"{sol(costs[-1]):.3f}"])
    print()
    print("The seed at the gate (P_ema / the pool's spot at the 8th warm-up integration; no position exit), a low of f x")
    print("in the first post-change second, and a genuine dump of the pool to f x (held) right after the transition:")
    print()
    hdr(["market", "move", "M6 as built (#104)", "#105"])
    for key, sc in (("D@80%", GRAD_SCENARIOS[0]), ("D@97%", GRAD_SCENARIOS[0])):
        for label, mk in (("low at 0.5x in second 1", lambda: (low_event(T_GRAD + 1, 0.5, []),)),
                          ("low at 0.25x in second 1", lambda: (low_event(T_GRAD + 1, 0.25, []),)),
                          ("low at 0.1x in second 1", lambda: (low_event(T_GRAD + 1, 0.1, []),)),
                          ("genuine dump to 0.5x from second 1", lambda: dump_events(0.5)),
                          ("genuine dump to 0.25x from second 1", lambda: dump_events(0.25))):
            cells = []
            for rule in ("built", "r105"):
                g, s = seed_at_gate(rule, key, sc, mk())
                cells.append("-" if g is None else f"{g:.3f} (gate +{s} s)")
            row([key, label] + cells)
    print()
    print("The genuine dump, positions at the cap held on (nothing may be forced: the spot leg reads the live price):")
    print()
    hdr(["market", "move", "M6 as built (#104)", "#105"])
    for key in ("D@80%", "D@97%"):
        for f in (0.5, 0.25):
            cells = []
            for rule in ("built", "r105"):
                m, pos, r, p1 = grad_run(rule, key, replace(GRAD_SCENARIOS[0], window=600), dump_events(f))
                cells.append(f"{exit_cell(r, TE, p1)}; P_ema/P at the end {m.ema / m.venue.spot_wad():.3f}")
            row([key, f"completion, then a genuine dump to {f:g}x from the first post-change second"] + cells)
    print()


def dump_events(f):
    """Other traders dump the pool to f x its price in the first post-change second and keep it there 600 s."""
    state = {}

    def first(m):
        if isinstance(m.venue, CPPool):
            state["p"] = m.venue.spot_wad()
            m.venue.move_to_price(int(state["p"] * f))

    def keep(m):
        if isinstance(m.venue, CPPool) and "p" in state:
            m.venue.move_to_price(int(state["p"] * f))
    return ((T_GRAD + 1, first),) + tuple((T_GRAD + 1 + i, keep) for i in range(1, 600))


# ------------------------------------------------------------------------------------------------------------
# Part 2: M6R-1 at the graduation
# ------------------------------------------------------------------------------------------------------------
def dep_at_transition(k, withdraw_at=0):
    """[migrate_v2, deposit k x the pool, crank_price (the transition)] in the migration second; the deposit is withdrawn
    `withdraw_at` s later (0: in the next transaction of the same second)."""
    state = {}

    def at_t(m):
        m.advance(T_GRAD)
        p = m.migrated_pool
        if p is None:
            return
        p.add_liquidity(BPS * k)
        state["pool"] = p
        m.observe(T_GRAD)                  # the attacker's crank: the transition reads lp_supply
        if withdraw_at == 0:
            withdraw_unlocked(p)

    def later(m):
        if "pool" in state:
            withdraw_unlocked(state["pool"])
    evs = [(T_GRAD, at_t)]
    if withdraw_at:
        evs.append((T_GRAD + withdraw_at, later))
    return tuple(evs)


def slow_trace(rule, key, events, probe=(0, 1, 9, 900, 3600)):
    """L_slow / sqrt(X*(Y + VQR)) of the pool at T_GRAD + s for s in probe (no position exit: size tiny), and the cap
    q / X of an open right after the boost window (T_GRAD + 900 s), relative to the honest run's."""
    m = RULES[rule](MARKETS[key](), tier_of(key), DEFAULT)
    pos = m.open(max(1, m.max_open_q() // 1000), 0)
    evs = sorted(tuple(GRAD_SCENARIOS[0].events) + tuple(events), key=lambda e: e[0])
    ei, out, q900 = 0, {}, None
    for t in range(1, T_GRAD + max(probe) + 1):
        while ei < len(evs) and evs[ei][0] <= t:
            evs[ei][1](m)
            ei += 1
        s = t - T_GRAD
        if TE - 60 <= t <= T_GRAD + 60 or t % 30 == 0 or s in probe or s == 900:
            m.step_to(t)                   # the keeper: every second around the graduation, else every 30 s
        else:
            m.advance(t)
            continue
        if s in probe and isinstance(m.venue, CPPool):
            out[s] = m.liq_slow / m.venue.sqrt_k()
        if s == 900 and isinstance(m.venue, CPPool):
            q900 = m.max_open_q() / m.venue.X
    return out, q900


def part2():
    print("## 2. M6R-1 at the graduation: a deposit between migrate_v2 and the transition")
    print()
    print("The attacker sends [migrate_v2, deposit k x the new pool, crank_price] (the transition reads the inflated")
    print("lp_supply as its anchor: L_slow = min(the curve's L_slow, anchor)), then withdraws.  As built L_slow keeps the")
    print("curve's value, ~1.35x the pool's depth, and decays at tau_slow = 1 h; with M6R-1 the first PumpSwap observation")
    print("caps it at the pool's locked depth D (the burned creation LP).  Cells: L_slow / sqrt(X*(Y + VQR)) at the")
    print("transition second, 1 s, 9 s (the gate), 900 s and 3600 s after it; the cap q/X of an open 900 s after the")
    print("transition (the boost window; the boost is spent by then).")
    print()
    hdr(["market", "move", "rule", "L_slow / depth at +0 / +1 / +9 / +900 / +3600 s", "open cap q/X at +900 s"])
    for key in ("D@80%",):
        for label, ev in (("none (honest)", ()), ("deposit 1x, withdrawn in the same second", dep_at_transition(1)),
                          ("deposit 3x, withdrawn in the same second", dep_at_transition(3)),
                          ("deposit 3x, held 60 s", dep_at_transition(3, 60))):
            for rule in ("built", "r105"):
                tr, q = slow_trace(rule, key, ev)
                row([key, label, RULE_NAME[rule], " / ".join(f"{tr.get(s, 0):.3f}" for s in (0, 1, 9, 900, 3600)),
                     "-" if q is None else f"{100 * q:.3f} %"])
    print()
    print("The carried positions' exits under the same deposit (ECON §10.1 rows; the keeper cranking every second):")
    print()
    hdr(["market", "scenario", "move", "M6 as built (#104)", "#105"])
    for key, sc in (("D@80%", GRAD_SCENARIOS[0]), ("D@90%", GRAD_SCENARIOS[1]), ("D@97%", GRAD_SCENARIOS[1])):
        for label, ev in (("none (honest)", ()), ("deposit 3x, withdrawn in the same second", dep_at_transition(3))):
            cells = []
            for rule in ("built", "r105"):
                m, pos, r, p1 = grad_run(rule, key, sc, ev)
                cells.append(f"{exit_cell(r, TE, p1)}; {lender_cell(pos, r)}")
            row([key, sc.name, label] + cells)
    print()


# ------------------------------------------------------------------------------------------------------------
# Part 3: M6R-1 in steady state
# ------------------------------------------------------------------------------------------------------------
def pool_c(tp_bps=0):
    """Market C's pool (the fresh graduate after its boost, lambda = 1), with tp_bps x the pool of third-party LP."""
    p = MARKETS["C"]()
    if tp_bps:
        p.add_liquidity(tp_bps)
    return p


def steady_market(rule, tp_bps=0, tier="C", size=1.0):
    pool = pool_c(tp_bps)
    m = RULES[rule](pool, TIERS[tier], DEFAULT)
    pos = m.open(max(1, int(m.max_open_q() * size)), 0)
    t = 0
    while t < TE:
        t += 1
        m.step_to(t)
    return m, pos, t


def held_run(rule, k, hold_s, top=3.0, dur=120, keeper="new", horizon=600):
    """A position at the cap on C (Tier C), the EMA converged; an LP deposits k x the pool at +1 s and holds it
    hold_s seconds (L_ema, L_slow follow the deeper pool), then withdraws in the second a genuine pump to top x over
    dur s starts; the keeper on its coverage duties (switch_warmup.Coverage).  Returns the exit cell and the reference
    liquidity L_b / sqrt(X*(Y + VQR)) at the pump's start."""
    m, pos, t0 = steady_market(rule)
    pool = m.venue
    p0 = pool.spot_wad()
    cov = SW.Coverage(keeper)
    lb = None
    for s in range(1, hold_s + horizon + 1):
        t = t0 + s
        if s == 1 and k:
            pool.add_liquidity(BPS * k)
        if s == hold_s + 1 and k:
            withdraw_unlocked(pool)
        if s > hold_s:
            pool.move_to_price(int(p0 * (1 + (top - 1) * min(s - hold_s, dur) / dur)))
        view = (pool.spot_wad(), pool.sqrt_k())
        if s == hold_s + 1:
            l_e = min(m.liq, m.ring_medians()[2]) if m.guard else m.liq
            l_ref = max(l_e, m.liq_slow)
            cap = m.ref_depth_cap()
            lb = max(pool.sqrt_k(), l_ref if cap is None else min(l_ref, cap)) / pool.sqrt_k()
        if cov.due(m, pos, t):
            m.step_to(t)
            cov.observed(t)
            r = m.try_exit(pos, t)
            if r is not None:
                return f"{r['kind']} +{s - hold_s} s @{r['spot'] / p0:.2f}x", lb
        else:
            m.advance(t)
        cov.check_foreign(m, t, view)
    return "open", lb


def sizing_run(rule, k, hold_s, wrap=None):
    """C's market; an LP deposits k x the pool and holds it hold_s seconds (the keeper cranking every 30 s); then an open
    at the cap -- with the deposit still in (wrap None) or wrapped in [deposit, open, withdraw] (wrap 'dep3') --
    after which every third-party LP unit leaves.  Returns (q / X of the pool after the withdrawal, the cap on the
    honest pool)."""
    m = RULES[rule](pool_c(), TIERS["C"], DEFAULT)
    honest_x = m.venue.X
    t = 0
    while t < TE:
        t += 1
        m.step_to(t)
    pool = m.venue
    if k:
        pool.add_liquidity(BPS * k)
    for s in range(1, hold_s + 1):
        if s % 30 == 0:
            m.step_to(t + s)
        else:
            m.advance(t + s)
    t += hold_s + 1
    m.step_to(t)
    w = wrapped(pool, wrap)
    m.venue = w
    q = m.max_open_q()
    m.venue = pool
    restore(pool, w)
    withdraw_unlocked(pool)
    return q / pool.X, honest_x


def sized_held_run(rule, k, size=0.95, top=5.0, dur=120, hold_s=3600, pull=True, horizon=600):
    """M6D-1: C's market (Tier C), the EMA converged; an LP deposits k x the pool and holds it hold_s seconds (the
    keeper cranking every 30 s: L_ema and L_slow follow the deeper pool); then an open of `size` x the cap on the
    deepened pool -- sized on the held deposit, the (b) bound -- and in the next second a genuine pump to top x over
    dur s starts, the deposit pulled (every third-party LP unit) in its first second (pull) or kept; the #101 keeper on
    its coverage duties (per second near the threshold).  Returns (the exit cell, q / the honest cap, the lenders'
    P&L / Q's value at the exit's spot, the borrower's share of V)."""
    m = RULES[rule](pool_c(), TIERS["C"], DEFAULT)
    t = 0
    while t < TE:
        t += 1
        m.step_to(t)
    pool = m.venue
    honest = m.max_open_q()
    if k:
        pool.add_liquidity(int(BPS * k))
    for s in range(1, hold_s + 1):
        if s % 30 == 0:
            m.step_to(t + s)
        else:
            m.advance(t + s)
    t += hold_s + 1
    m.step_to(t)
    pos = m.open(max(1, int(m.max_open_q() * size)), t)
    p0 = pool.spot_wad()
    cov = SW.Coverage("new")
    for s in range(1, horizon + 1):
        tt = t + s
        if s == 1 and k and pull:
            withdraw_unlocked(pool)
        pool.move_to_price(int(p0 * (1 + (top - 1) * min(s, dur) / dur)))
        view = (pool.spot_wad(), pool.sqrt_k())
        if cov.due(m, pos, tt):
            m.step_to(tt)
            cov.observed(tt)
            r = m.try_exit(pos, tt)
            if r is not None:
                qv = pos.Q * r["spot"] // WAD
                lp = r["lender_sol"] - (pos.Q - r["tokens"]) * r["spot"] // WAD
                return (f"{r['kind']} +{s} s @{r['spot'] / p0:.2f}x", pos.Q / honest, lp / qv, r["borrower"] / pos.V)
        else:
            m.advance(tt)
        cov.check_foreign(m, tt, view)
    return "open", pos.Q / honest, None, None


def drag_run(rule, keeper, hold, attack, tp_bps=BPS, w=30, dur=60, horizon=300, att_from=None, size=1.0):
    """FITS-2 on PumpSwap (steady state): C's pool with tp_bps x the pool of third-party LP, which the attacker holds
    (lambda = 1/(1 + tp)); a position at the cap, the EMA converged; the keeper on its coverage duties; in every second
    from att_from, before the keeper's tick, the attacker sends [withdraw all, crank_price, deposit] (the drag) and
    [withdraw all, liquidate, deposit]; flat w s, then a genuine pump to hold x over dur s, held."""
    m, pos, t0 = steady_market(rule, tp_bps, size=size)
    pool = m.venue
    p0 = pool.spot_wad()
    cov = SW.Coverage(keeper)
    if att_from is None:
        att_from = 2 if keeper == "new" else 1
    for s in range(1, w + dur + horizon + 1):
        t = t0 + s
        if s > w:
            pool.move_to_price(int(p0 * (1 + (hold - 1) * min(s - w, dur) / dur)))
        view = (pool.spot_wad(), pool.sqrt_k())
        a = attack and s >= att_from
        if a:
            obs_wrapped(m, t, "wd")
            r = try_liq(m, pos, t, "wd")
            if r is not None:
                return f"{r['kind']} +{s - w} s by the attacker @{r['spot'] / p0:.3f}x"
        if cov.due(m, pos, t):
            m.step_to(t)
            cov.observed(t)
            r = m.try_exit(pos, t)
            if r is not None:
                return f"{r['kind']} +{s - w} s by the keeper @{r['spot'] / p0:.3f}x"
        elif not a:
            m.advance(t)
        cov.check_foreign(m, t, view)
    return "open"


def drag_threshold(rule, keeper, tp_bps=BPS, lo=1.2, hi=3.5, tol=0.005, size=1.0):
    """The lowest hold level at which the honest keeper liquidates (no attacker)."""
    while hi - lo > tol:
        mid = (lo + hi) / 2
        if drag_run(rule, keeper, mid, False, tp_bps, size=size).startswith("liquidation"):
            hi = mid
        else:
            lo = mid
    return hi


def part3():
    print("## 3. M6R-1 in steady state on a PumpSwap pool (market C: the fresh graduate after its boost, Tier C)")
    print()
    print("(a) A deposit of k x the pool held for H and pulled in the second a genuine 3x/120 s pump starts (the keeper on")
    print("its coverage duties, #101 keeper).  L_ema and L_slow follow the deeper pool while it is held; the exit reference")
    print("L_b = max(L_live, min(max(L_ema', L_slow), cap)) then credits the pulled depth unless the cap floor(D*1e4/")
    print("lambda_min) binds.  Cells: the exit (seconds after the pump starts, price); L_b / the pool's depth at the pump's")
    print("first second.")
    print()
    hdr(["deposit", "held", "M6 as built (#104)", "#105"])
    for k, hold_s in ((0, 600), (1, 600), (1, 3600), (3, 600), (3, 3600), (7, 3600)):
        cells = []
        for rule in ("built", "r105"):
            c, lb = held_run(rule, k, hold_s)
            cells.append(f"{c}; L_b/depth {lb:.2f}" if lb is not None else c)
        row([f"{k}x the pool" if k else "none", f"{hold_s} s"] + cells)
    print()
    print("(b), (c) An open at the cap after the deposit was held for H (still in during the open), or wrapped around the")
    print("open alone; every third-party LP unit then leaves.  Cells: the position's q / X of the pool after the")
    print("withdrawal (the honest open: 1x); a deposit wrapped around the open inflates nothing (the minimum drops X_now,")
    print("and L_slow, D and the ring's EMA state do not move within the transaction), a held one pushes L_slow, which")
    print("the D term bounds at lambda/lambda_min of the burned LP's depth:")
    print()
    hdr(["deposit", "held / wrapped", "M6 as built (#104)", "#105"])
    base = sizing_run("r105", 0, 0)[0]
    for k, hold_s, wrap in ((0, 0, "dep3"), (1, 3600, None), (3, 3600, None), (7, 3600, None), (3, 14400, None),
                            (7, 14400, None)):
        cells = []
        for rule in ("built", "r105"):
            q, _hx = sizing_run(rule, k, hold_s, wrap)
            cells.append(f"{100 * q:.3f} % ({q / base:.2f}x)")
        row([f"{k or 3}x the pool", "wrapped around the open" if wrap else f"held {hold_s} s"] + cells)
    print()
    print("(d) FITS-2 on PumpSwap: C's pool with as much third-party LP again (lambda 50 %), which the attacker holds; a")
    print("position at a fraction of the cap; flat 30 s, then a genuine pump to a hold level just below the honest")
    print("threshold (healthy at the pool's depth).  From its first second the attacker sends [withdraw all, crank_price,")
    print("deposit] (the withdraw drag: its attacker-only seconds pull L_ema to D) and [withdraw all, liquidate, deposit]")
    print("every second.  The liquidation spot leg at L_live (#100) or at max(L_live, L_ema') (FITS-2 (a), #101), each")
    print("with the #99 keeper (per second only near a threshold, else every 30 s) or the #101 keeper (FITS-2 (b): per")
    print("second for 10 s after a foreign bucket liquidity, extended while it recurs); every column with #105's D rules.")
    print("Cells: the attacked outcome at each hold level (the honest keeper leaves every one of them open):")
    print()
    combos = (("s100", "old"), ("r105", "old"), ("s100", "new"), ("r105", "new"))
    names = {("s100", "old"): "spot leg at L_live, #99 keeper (#100)",
             ("r105", "old"): "spot leg at max(L_live, L_ema'), #99 keeper",
             ("s100", "new"): "spot leg at L_live, #101 keeper",
             ("r105", "new"): "spot leg at max(L_live, L_ema'), #101 keeper (as adopted)"}
    hdr(["size (of the cap)", "honest threshold", "hold"] + [names[c] for c in combos])
    for size in (0.5, 0.25):
        th = drag_threshold("r105", "new", size=size)
        for dh in (0.005, 0.01, 0.02, 0.04):
            hold = round(th - dh, 3)
            row([f"{size:g}", f"{th:.3f}x", f"{hold:.3f}x"] + [drag_run(r, k, hold, True, size=size) for r, k in combos])
    print()
    part3e()


def part3e():
    print("(e) M6D-1: (b)'s held deposit sizes the open, then leaves at a genuine pump.  An LP deposits k x the pool")
    print("and holds it 1 h (a crank every 30 s); a position of `size` x the cap is opened on the deepened pool (sized")
    print("on the held deposit: X0's D term allows lambda/lambda_min of the burned LP's depth, 2x at lambda_min 50 %);")
    print("in the next second a genuine pump to top x over 120 s starts and the LP withdraws every third-party unit in")
    print("its first second (or keeps it: the control).  L_slow (tau 1 h) still credits the deposit, so the reference")
    print("L_b stays at the cap floor(D*1e4/lambda_min) while the pool is back at D; the #101 keeper tries every")
    print("covered second.  Cells: the exit (seconds after the pump starts, price); q / the honest cap; the lenders'")
    print("P&L against Q's value at the exit's spot; the borrower's share of V.")
    print()
    rules = ("built", "r105", "lam100")
    hdr(["deposit (third-party share)", "held, then", "pump", "size (of the cap)"] + [RULE_NAME[r] for r in rules])
    cases = [(0, True, 5.0, 0.95), (3, False, 5.0, 0.95), (3, True, 3.0, 0.95), (3, True, 5.0, 0.95),
             (3, True, 10.0, 0.95), (3, True, 5.0, 0.75), (3, True, 5.0, 0.5), (7, True, 5.0, 0.95),
             (1, True, 5.0, 0.95), (0.5, True, 5.0, 0.95), (0.25, True, 5.0, 0.95), (0.075, True, 5.0, 0.95)]
    for k, pull, top, size in cases:
        cells = []
        for rule in rules:
            c, qr, lp, bw = sized_held_run(rule, k, size=size, top=top, pull=pull)
            cells.append(c if lp is None else f"{c}; q {qr:.2f}x; lenders {100 * lp:+.1f} % of Q; borrower "
                                              f"{100 * bw:.0f} % of V")
        dep = "none" if not k else f"{k:g}x the pool ({100 * k / (1 + k):.0f} %)"
        row([dep, "-" if not k else ("pulled at the pump" if pull else "kept (control)"), f"{top:g}x / 120 s",
             f"{size:g}"] + cells)
    print()


PARTS = {"1": part1, "2": part2, "3": part3}

if __name__ == "__main__":
    want = sys.argv[1:] or list(PARTS)
    for p in want:
        PARTS[p]()
