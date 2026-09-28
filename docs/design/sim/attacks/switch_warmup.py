"""M4R: post-change confirmation of the observation state after a Raydium venue change (ECON §5.2, §10.3,
§10.5, §12.12; ARCH §2.6 rule 4, §4.4; changelog #96; PROPOSED, owner item O18).

The problem.  After a fast-path adoption the M4 rule as built seeded the exits from single-instruction samples:
L_new = min(L_rec, L_live, floor(D*1e4/lambda_min)) and a P_ema frozen during the halt.  An unlocked LP of the
candidate withdraws around a sampling instruction (L_new = D), then holds L_ema there with one withdraw-wrapped
crank per second (directional aggregation freezes an EMA with one observation on its own side every second) and
liquidates positions within ~q/x of cr_liq that are healthy on the pool as it trades (M4R-1); the window floor
judged at the stale P_ema admits a pool whose real locked SOL is floor*sqrt(P_live/P_ema) after a dump (M4R-2).

The rule (the program's; ../shortwin_sim.py Market.adopt_pool / _integrate_warmup / gated / exit_reference /
window_floor_ok):
  * the adoption reads no sample: L_ema and every ring entry's L := the anchor cap = floor(D*1e4/lambda_min),
    L_slow = min(L_slow, D), P_ema and the ring prices carried; ema_warm := 8, ema_guard := 1;
  * each of the next 8 integrations re-seeds, over the post-change entries only, P_ema = min of the bucket
    price lows and of the largest v0.4.1 step from the previous P_ema (M4X-1: the seed rises no faster than the
    v0.4.1 EMA can) and L_ema = min(max lhi, floor(lomed(lhi)*(1e4 + C)/1e4)) -- no blend, L_slow untouched;
  * while ema_warm > 0 on an executable venue liquidate / expire fail (EmaWarmingUp), sol_settle evaluates
    nothing and Raydium opens are refused; a halted venue is never gated;
  * while ema_guard = 1 the exit reference reads L_b = max(L_live, min(L_ema, L_mlo), L_slow) and L_slow
    integrates min(lobs, L_mlo) (M4X-2); the guard clears at the first integration after the warm-up with
    L_ema <= 1.01 * L_mlo;
  * a proposal that opens or improves a window judges the window floor at min(P_ema, P_cand); the execution at
    P_ema, every failure cancels (as built; a genuine dump inside the window is adopted at P_ema, M4X-3).
Compared with "M4 as built" (venue_switch.BuiltM4).  Analysis rules: "M4R without the guard" (part 4), "M4R with
L_slow unguarded" (part 4c: the rule before M4X-2), "M4R without the step bound" (part 8d: before M4X-1).

Three run engines, ported from the M4R redesign's harnesses (the designers' fast-path loops, integer-exact;
with the rules replaced by the sim's):
  W  run_fast_path (designer "warmup-gate"): the M4R-1 reproducer -- a 100-SOL fully locked AMM v4 incumbent at
     RA's price (Tier C), a CPMM candidate at m x the pre-halt price; after the adoption, per second: the honest
     keeper's crank and liquidation attempt, and the attacker's actors (wrapped cranks, wrapped liquidations,
     atomic price bundles, held LP / price moves).
  P  switch_run (designer "invariant-seed"): incumbents R100 (100 SOL fully locked at RC's price, Tier C), RA,
     RC; attacker a1 = the LiteSVM reproducer's (execution withdraw-wrapped, then a withdraw-wrapped crank and
     liquidation every second after the keeper), a2 = every sample withdraw-wrapped and the wrapped liquidation
     first in each second.  A failed attacker liquidation keeps its prologue's observation (an upper bound on
     the attacker: on chain the attacker bundles a wrapped crank_price instead).
  M  run_plan (designer "multi-sample"): a 60-s pre-halt period (optionally at pre_mult x), the same incumbent
     at lambda 99.78 %, the LP-holder's wrapped crank and liquidation after the keeper every second.
Parts:
  1  M4R-1 grid (W): q 0.40 / 0.74 % of the incumbent's X, four LP-wrap variants of the two sampling
     instructions, the per-second withdraw-wrapped freeze and wrapped liquidation, 120 s.
  2  lambda < lambda_min candidates under the M4R-1 attacker (W; P on a fine grid).
  3  Honest depth and box after the warm-up (W): thinner, deeper and lambda < lambda_min candidates.
  4  Capital deposit freeze (P: RA -> 630 / 314 SOL; W: near-threshold and the big position); 4c the freeze held
     for 10-30 min before a genuine pump (M4X-2).
  5  Withdraw contest while the guard is set (P).
  6  M4R-2: a dump during the halt (P); a dump inside the collection window (M4X-3).
  7  Genuine moves (W, P, M).
  8  Degraded mode: no keeper during the warm-up (W, M); the steady-state variant for comparison (pre-existing);
     8d atomic price bundles in attacker-only seconds, the attacker's best pump sizes (M4X-1).
  9  Stale-high P_ema after a mild dip (M).
  10 Targeted random search (W): 1,200 runs, seed 23.
  selftest: an honest switch reaches L_ema / sqrt(X*Y) = 1.000 at the gate, the box is narrow after it and the
  guard clears at the gate.
Deterministic; standard library only.

    python3 switch_warmup.py            # every part (redirect to ../out/attacks/switch_warmup.out)
    python3 switch_warmup.py 1 4 10     # some parts
"""
import copy
import math
import random
import sys

from common import *  # noqa: F401,F403
from venue_switch import BuiltM4

TIER_C = TIERS["C"]


class NoGuard(Market):
    """Analysis rule: M4R without the reference guard (the exit reference reads L_ema as in v0.4.1)."""

    def mark_restart(self):
        super().mark_restart()
        self.guard = False


class NoBound(Market):
    """Analysis rule: M4R before M4X-1 -- the warm-up price seed is the min of the post-change bucket lows alone,
    with no step bound (one attacker-only warm-up seeds any pumped price)."""

    def _integrate_warmup(self, t):
        if t > self.ema_t:
            T = self.tier
            k = EMA_WARMUP_BUCKETS - self.warm
            ents = self.ring[len(self.ring) - k:] if k else []
            p = min([e[5] for e in ents] + [self.b_lo])
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


class NoSlowGuard(Market):
    """Analysis rule: M4R before M4X-2 -- L_slow integrates lobs even while the guard is set, so a deposit-frozen
    L_ema climbs into L_slow, which the exit reference reads unguarded."""

    def _integrate(self, t):
        if self.warm > 0 or not self.guard:
            return super()._integrate(t)
        n0 = self.ema_count
        self.guard = False
        super()._integrate(t)
        self.guard = True
        if self.ema_count != n0:
            self._guard_check()


RULE_CLS = {"built": BuiltM4, "m4r": Market, "noguard": NoGuard, "nobound": NoBound}
RULE_NAME = {"built": "M4 as built", "m4r": "M4R", "noguard": "M4R without the guard (analysis)",
             "nobound": "M4R without the step bound (analysis; before M4X-1)",
             "noslow": "M4R with L_slow unguarded (analysis; before M4X-2)"}
RULES2 = ("built", "m4r")


def hdr(cols):
    print("| " + " | ".join(cols) + " |")
    print("|" + "---|" * len(cols))


def row(cells):
    print("| " + " | ".join(str(c) for c in cells) + " |")


# ======================================================================================================
# Engine W (warmup-gate's run_fast_path)
# ======================================================================================================
def lp_withdraw_all(v):
    """Withdraw every unlocked LP unit; returns the LP units to re-deposit."""
    units = v.lp_units - v.lp_locked
    if units <= 0:
        return 0
    v.X -= v.X * units // v.lp_units
    v.Y -= v.Y * units // v.lp_units
    v.lp_units -= units
    return units


def lp_deposit(v, units):
    """Deposit `units` LP units at the pool's current ratio (reserves scale proportionally)."""
    if units <= 0:
        return
    v.X += v.X * units // v.lp_units
    v.Y += v.Y * units // v.lp_units
    v.lp_units += units


def lp_withdraw_raw(v, units):
    v.X -= v.X * units // v.lp_units
    v.Y -= v.Y * units // v.lp_units
    v.lp_units -= units


class Wrap:
    """Context: an LP move around one instruction ('wd' = withdraw every unlocked LP, 'dep' = deposit as much
    again as the LP counter (L doubles), None = nothing)."""

    def __init__(self, v, move):
        self.v, self.move, self.u = v, move, 0

    def __enter__(self):
        if self.move == "wd":
            self.u = lp_withdraw_all(self.v)
        elif self.move == "dep":
            self.u = self.v.lp_units
            lp_deposit(self.v, self.u)
        return self

    def __exit__(self, *exc):
        if self.move == "wd":
            lp_deposit(self.v, self.u)
        elif self.move == "dep":
            lp_withdraw_raw(self.v, self.u)
        return False


P_BASE_X, P_BASE_Y = 150_000_000_000_000, 2_500 * LAMPORTS_PER_SOL      # RA's price


def pool_at(sol_, lock_bps, fee_ppm=2500, out_share_ppm=120_000, name="AMMv4"):
    """A Raydium pool with `sol_` SOL real at the base price and locked share lock_bps."""
    Y = int(sol_ * LAMPORTS_PER_SOL)
    X = P_BASE_X * Y // P_BASE_Y
    return RaydiumPool(X, Y, fee_ppm=fee_ppm, out_share_ppm=out_share_ppm, name=name, lock_bps=lock_bps)


TH = 600          # the incumbent halts at TH (position opened at 0, EMA converged)


class Cfg:
    def __init__(self, **kw):
        self.inc_sol = 100.0
        self.inc_lock = 10_000
        self.inc_fee = 2500
        self.cand_sol = 52.0
        self.cand_lock = 5000
        self.cand_fee = 2500
        self.cand_mult = 1.97          # candidate price at adoption / pre-halt price
        self.q_bps = 40                # position q, bps of the incumbent's X (reproducer: 40)
        self.rec_move = None           # LP move around the recording proposal
        self.exec_move = None          # LP move around the execution
        self.keeper = lambda s: True   # s = seconds after the adoption: honest keeper cranks + tries exits
        self.att_crank = None          # the attacker's per-second wrapped crank: None | "wd" | "dep"
        self.att_liq = None            # the attacker's per-second wrapped exit attempt: None | "wd" | "dep"
        self.att_price = None          # per-second atomic price bundle around a crank: None | ("pump"|"dump", k)
        self.held = None               # (s0, s1, move): a genuine LP move held from s0 to s1 after the adoption
        self.held_price = None         # (s0, s1, k): price held at k x the path from s0 to s1 (attacker-only)
        self.path = None               # s -> price multiple of the pre-halt price after the adoption (None: flat)
        self.secs = 120
        self.new_halt_at = None        # the adopted pool halts s seconds after the adoption
        self.observe_from = 1          # nobody observes before this many seconds after the adoption
        for k, v in kw.items():
            setattr(self, k, v)


def proposal_floor(rule, m, c):
    """The window floor of a proposal: M4 as built at P_ema; M4R at min(P_ema, P_cand) (M4R-2)."""
    if rule == "built":
        return position_floor_ok(c.locked_depth(), m.ema)
    return window_floor_ok(c.locked_depth(), m.ema, c.spot_wad())


def box_wide(m):
    P_lo, P_hi, L_lo, L_hi = m.settle_box(m.t)
    return (L_hi - L_lo) * BPS > DEFAULT.fv_dev_bps * L_lo


def run_fast_path(rule, cfg):
    """Returns a dict: adoption second (relative to the halt), exit (kind, seconds after adoption, who, spot
    multiple), L_ema/sqrt(XY) right after the adoption and at the end of the warm-up, borrower/lender P&L."""
    inc = pool_at(cfg.inc_sol, cfg.inc_lock, fee_ppm=cfg.inc_fee)
    m = RULE_CLS[rule](inc, TIER_C, DEFAULT)
    X0 = inc.X
    Q = X0 * cfg.q_bps // BPS
    pos = m.open(Q, 0)
    p0 = inc.spot_wad()
    for t in range(1, 61):
        m.step_to(t)
    for t in range(61, TH - 10, 30):
        m.step_to(t)
    for t in range(TH - 10, TH):
        m.step_to(t)
    inc.disabled = True
    c = pool_at(cfg.cand_sol, cfg.cand_lock, fee_ppm=cfg.cand_fee, out_share_ppm=160_000, name="CPMM")
    c.move_to_price(int(p0 * cfg.cand_mult))
    out = dict(adopt=None, exit=None, who=None, t_exit=None, mult=None, l_adopt=None, l_warm=None,
               p_warm=None, t_warm=None, cancelled=0, pos=pos, m=m, cand=c, Q=Q,
               wide_warm=0, wide_after=0, q_x=Q / c.X, t_clear=None)
    P = m.P
    t = TH
    l_rec = None
    fp_t0 = None
    t_adopt = None
    while t < TH + 1200 and t_adopt is None:
        t += 1
        m.step_to(t)                                   # the keeper's crank of the halted incumbent (chain)
        if m.pool_fail_since is None:
            continue
        armed = t >= m.pool_fail_since + P.pool_fast_route_s
        if fp_t0 is None and armed:
            snap = (c.X, c.Y)
            with Wrap(c, cfg.rec_move):
                ok = proposal_floor(rule, m, c)
                if ok:
                    l_rec = c.sqrt_k()
            c.X, c.Y = snap
            if ok:
                fp_t0 = t
        if fp_t0 is not None and t >= fp_t0 + P.pool_fast_window_s:
            snap = (c.X, c.Y)
            with Wrap(c, cfg.exec_move):
                live = c.sqrt_k()
                ok = position_floor_ok(c.locked_depth(), m.ema)
            c.X, c.Y = snap
            if ok:
                if rule == "built":
                    m.adopt_pool(c, t, l_rec=l_rec, l_live=live)
                else:
                    m.adopt_pool(c, t)
                t_adopt = t
            else:
                fp_t0 = None                           # cancelled
                out["cancelled"] += 1
    if t_adopt is None:
        return finish(m, pos, out, t, None)
    out["adopt"] = t_adopt - TH
    out["l_adopt"] = m.liq / c.sqrt_k()
    held_units = 0
    for s in range(1, cfg.secs + 1):
        t = t_adopt + s
        mult = cfg.path(s) if cfg.path else None
        if mult is not None and c.has_route():
            c.move_to_price(int(p0 * mult))
        if cfg.new_halt_at is not None and s == cfg.new_halt_at:
            c.disabled = True
        if cfg.held and s == cfg.held[0]:
            held_units = lp_withdraw_all(c) if cfg.held[2] == "wd" else 0
        if cfg.held and s == cfg.held[1] and held_units:
            lp_deposit(c, held_units)
            held_units = 0
        r = None
        who = None
        hp = cfg.held_price and cfg.held_price[0] <= s < cfg.held_price[1] and c.has_route()
        if hp:                                         # the attacker holds the price for the whole second
            base = c.spot_wad()
            c.move_to_price(int(base * cfg.held_price[2]))
        observed = False
        if s >= cfg.observe_from:
            if cfg.keeper(s):
                m.step_to(t)
                r = m.try_exit(pos, t)
                who = "keeper"
                observed = True
            if r is None and cfg.att_crank and c.has_route():
                with Wrap(c, cfg.att_crank):
                    m.advance(t)
                    m.observe(t)
                observed = True
            if r is None and cfg.att_price and c.has_route():
                kind, k = cfg.att_price
                b0 = c.spot_wad()
                snap = (c.X, c.Y)
                c.move_to_price(int(b0 * (k if kind == "pump" else 1 / k)))
                m.advance(t)
                m.observe(t)
                c.X, c.Y = snap
                observed = True
            if r is None and cfg.att_liq and c.has_route():
                m2, pos2 = copy.deepcopy((m, pos))     # a reverted liquidate records nothing
                c2 = m2.venue
                wr = Wrap(c2, cfg.att_liq)
                wr.__enter__()
                r2 = m2.try_exit(pos2, t)
                wr.__exit__()
                if r2 is not None:
                    m, pos, c, r = m2, pos2, c2, r2
                    who = "attacker"
        if not observed:
            m.advance(t)
        if hp:
            c.move_to_price(base)
        if c.has_route() and c.has_price():
            wide = box_wide(m)
            if m.warm > 0:
                out["wide_warm"] += wide
            elif s <= 60 + (EMA_WARMUP_BUCKETS if rule != "built" else 0):
                out["wide_after"] += wide
        if rule != "built" and m.warm == 0 and out["l_warm"] is None and c.has_price():
            out["l_warm"] = m.liq / c.sqrt_k()
            out["p_warm"] = m.ema / c.spot_wad()
            out["t_warm"] = s
        if rule != "built" and out["t_clear"] is None and not m.guard:
            out["t_clear"] = s
        if r is not None:
            out["who"] = who
            return finish(m, pos, out, t, r, t_adopt, p0)
    return finish(m, pos, out, t, None, t_adopt, p0)


def finish(m, pos, out, t, r, t_adopt=None, p0=None):
    if r is None:
        out["exit"] = "open"
        v = m.venue
        if v.has_route():
            qs = v.quote_at(pos.Q, *v.eff())
            R = m.rent_due(pos)
            out["borrower_eq"] = (pos.V - qs[1] - R) / LAMPORTS_PER_SOL if qs else None
        else:
            out["borrower_eq"] = None
        out["lender_sol"] = 0.0
    else:
        out["exit"] = r["kind"]
        out["t_exit"] = t - (t_adopt or 0)
        out["mult"] = r["spot"] / p0 if p0 else None
        out["borrower_eq"] = r["borrower"] / LAMPORTS_PER_SOL
        out["lender_sol"] = r["lender_sol"] / LAMPORTS_PER_SOL
    out["m"] = m
    return out


def wcell(o, money=False):
    """Engine W cell: outcome, k = keeper / a = attacker, seconds after the adoption, (borrower SOL at the exit
    or equity if open[ / lender SOL])."""
    if o["adopt"] is None:
        return "no adoption" + (" (window floor)" if not o["cancelled"] else f" ({o['cancelled']} cancelled)")
    if o["exit"] == "open":
        be = o.get("borrower_eq")
        return f"open ({be:.3f})" if be is not None else "open"
    if money:
        return (f"{o['exit']} {o['who'][0]} +{o['t_exit']}s @{o['mult']:.2f}x ({o['borrower_eq']:.3f} / "
                f"{o['lender_sol']:.3f})")
    return f"{o['exit']} {o['who'][0]} +{o['t_exit']}s ({o['borrower_eq']:.3f})"


def ramp(s0, k, dur, base=1.0):
    g = math.log(k) / dur
    return lambda s: base * math.exp(g * min(max(s - s0, 0), dur)) if s >= s0 else base


def step(s0, k, base=1.0):
    return lambda s: base * k if s >= s0 else base


# ======================================================================================================
# Engine P (invariant-seed's switch_run)
# ======================================================================================================
FAST_MIN_BPS = 2500


def unlocked_bps(v):
    return (v.lp_units - v.lp_locked) * BPS // v.lp_units


def wrapped(v, move):
    """The pool one instruction sees inside an LP round trip: 'wd' = every unlocked LP withdrawn
    (sqrt(X*Y) -> D), 'dep' = as much again as the LP counter deposited (sqrt(X*Y) x2, lambda /2).  Price
    unchanged."""
    if move is None:
        return v
    w = copy.deepcopy(v)
    if move == "wd":
        u = unlocked_bps(w)
        if u > 0:
            w.remove_liquidity(u)
    elif move.startswith("dep"):
        k = int(move[3:] or "1")
        w.add_liquidity(BPS * k)
    else:
        raise ValueError(move)
    return w


def restore(v, w):
    """After an instruction ran on the wrapped copy w of v (possibly trading), the round trip closes: the LP
    moved out (in) goes back (out) at the new reserve ratio."""
    if w is v:
        return
    v.X = w.X * v.lp_units // w.lp_units
    v.Y = w.Y * v.lp_units // w.lp_units


class _PWindow:
    """The collection window's hooks of engine P."""

    def floor_at_proposal(self, cand_sample):
        return window_floor_ok(cand_sample.locked_depth(), self.ema, cand_sample.spot_wad())

    def floor_at_execution(self, cand_sample):
        return position_floor_ok(cand_sample.locked_depth(), self.ema)

    def win_record(self, t, cand_sample):
        pass

    def win_observe(self, t, cand_sample):
        pass                                     # a re-proposal of the window's own candidate records nothing

    def win_adopt(self, pool, t, exec_sample):
        self.adopt_pool(pool, t)


class PBuilt(_PWindow, BuiltM4):
    def floor_at_proposal(self, cand_sample):
        return position_floor_ok(cand_sample.locked_depth(), self.ema)

    def win_record(self, t, cand_sample):
        self.fp_lrec = cand_sample.sqrt_k()

    def win_adopt(self, pool, t, exec_sample):
        self.adopt_pool(pool, t, l_rec=self.fp_lrec, l_live=exec_sample.sqrt_k())


class PM4R(_PWindow, Market):
    pass


class PNoGuard(_PWindow, NoGuard):
    pass


class PNoSlow(_PWindow, NoSlowGuard):
    pass


P_RULES = {"built": PBuilt, "m4r": PM4R, "noguard": PNoGuard, "noslow": PNoSlow}


def ray_pool(sol_real, lock_bps, fee_ppm=2500, out_share_ppm=120_000, name="AMMv4"):
    """A Raydium pool with sol_real SOL at the RC token price (PUMP_POOL_TOKENS per 84.99 SOL)."""
    xps = PUMP_POOL_TOKENS / 84.99
    return RaydiumPool(int(xps * sol_real), int(sol_real * LAMPORTS_PER_SOL), fee_ppm=fee_ppm,
                       out_share_ppm=out_share_ppm, name=name, lock_bps=lock_bps)


INCUMBENTS = {
    "R100": (lambda: ray_pool(100, 10_000), "C"),          # the M4R-1 reproducer's incumbent: 100 SOL, fully locked
    "RA": (lambda: MARKETS["RA"](), "A"),
    "RC": (lambda: MARKETS["RC"](), "C"),
}


class Spec:
    def __init__(self, **kw):
        self.inc = "R100"
        self.size = 1.0                  # position = size x max_open_q
        self.cand_sol = 52               # candidate's real SOL at the pre-halt price
        self.cand_lock = 5000            # candidate lambda (bps)
        self.m = 1.0                     # candidate price at the halt, x pre-halt spot (a move during the halt)
        self.post = None                 # t_rel (s after adoption) -> multiple of the adoption price, or None
        self.rec_move = None             # attacker makes the recording proposal inside an LP move
        self.exec_move = None            # attacker executes inside an LP move
        self.rec_pump = None             # attacker makes the recording proposal inside a price push (x spot)
        self.win_move = None             # (f, s): the candidate genuinely moves to f x at window second s (M4X-3)
        self.win_lp = None               # attacker re-proposes an LP-wrapped candidate every window second
        self.freeze = None               # attacker's per-second wrapped crank after adoption: 'wd' / 'dep'
        self.freeze_from = 1             # ... from this second after adoption
        self.att_liq = None              # attacker's per-second wrapped liquidation after adoption: 'wd'
        self.att_first = False           # the attacker's transactions land before the keeper's in each second
        self.no_exit_until = 0           # the keeper does not try exits before this second after adoption
        self.keeper_off = ()             # absolute-second ranges (a, b) without the keeper
        self.horizon = 120               # seconds after adoption
        self.pre_warm = TE               # honest period before the halt
        for k, v in kw.items():
            setattr(self, k, v)


def as_tuple(x):
    if x is None:
        return ()
    return x if isinstance(x, tuple) else (x,)


def keeper_on(spec, t):
    return not any(a <= t < b for a, b in spec.keeper_off)


def try_liq(m, pos, t, move=None):
    v = m.venue
    w = wrapped(v, move)
    m.venue = w
    r = m.try_exit(pos, t)
    m.venue = v
    restore(v, w)
    return r


def obs_wrapped(m, t, move):
    v = m.venue
    w = wrapped(v, move)
    m.advance(t)
    m.venue = w
    m.observe(t)
    m.venue = v
    restore(v, w)


def pump_cost(v, k):
    """Round-trip cost (lamports) of pushing v's spot to k x and back inside one transaction (no arbitrage)."""
    w = copy.deepcopy(v)
    d = move_size(w, int(w.spot_wad() * k))
    if d > 0:
        c = w.buy(d)
        return c - w.sell(d)
    if d < 0:
        got = w.sell(-d)
        return w.buy(-d) - got if -d < w.X else 0
    return 0


def switch_run(spec, rule):
    """Returns a dict: adopted, t_adopt, l_ratio (L_ema/sqrt(X*Y) of the adopted pool at the switch), exit (result
    dict or None), t_exit_rel, who, box_wide (seconds of the first 60 after adoption with the box > fv_dev wide in
    L), t_clear (seconds after the adoption at which the guard cleared), cost (attacker round-trip fees)."""
    f, tier = INCUMBENTS[spec.inc]
    m = P_RULES[rule](f(), TIERS[tier], DEFAULT)
    Q = max(1, int(m.max_open_q() * spec.size))
    pos = m.open(Q, 0)
    t = 0
    while t < spec.pre_warm - 60:
        t += 10
        m.step_to(t)
    while t < spec.pre_warm:
        t += 1
        m.step_to(t)
    inc = m.venue
    p_pre = inc.spot_wad()
    cand = RaydiumPool(inc.X * spec.cand_sol * LAMPORTS_PER_SOL // inc.Y,
                       spec.cand_sol * LAMPORTS_PER_SOL, fee_ppm=inc.fee_ppm, out_share_ppm=inc.out_share_ppm,
                       name="cand", lock_bps=spec.cand_lock)
    cand.move_to_price(int(p_pre * spec.m))
    pool_d = inc.locked_depth()
    inc.disabled = True
    out = dict(pool_d=pool_d, adopted=False, t_adopt=None, l_ratio=None, exit=None, t_exit_rel=None, who=None,
               box_wide=None, cost=0, pos=pos, p_pre=p_pre, cancelled=0, floor_refused=0, t_clear=None)
    fp_t0 = None
    t_adopt = None
    wide = 0
    t_end = spec.pre_warm + 400 + spec.horizon
    while t < t_end:
        t += 1
        kon = keeper_on(spec, t)
        if t_adopt is not None and spec.post is not None:
            g = spec.post(t - t_adopt)
            if g is not None:
                m.venue.move_to_price(int(out["p_adopt"] * g))
        if t_adopt is None:
            # --- halted incumbent; the window ------------------------------------------------------
            armed = m.pool_fail_since is not None and t >= m.pool_fail_since + m.P.pool_fast_route_s
            if fp_t0 is None:
                if kon:
                    m.step_to(t)                       # keeper crank: the pool_fail chain
                if armed and (kon or spec.rec_move or spec.rec_pump):
                    # the recording proposal: the attacker's (wrapped) if it plays, else the keeper's
                    m.advance(t)
                    m.observe(t)                       # its prologue on the halted incumbent
                    s = wrapped(cand, spec.rec_move)
                    if spec.rec_pump:
                        s = copy.deepcopy(cand)
                        s.move_to_price(int(s.spot_wad() * spec.rec_pump))
                    fast_ok = s.locked_depth() * BPS >= FAST_MIN_BPS * out["pool_d"]
                    if fast_ok and m.floor_at_proposal(s):
                        m.win_record(t, s)
                        fp_t0 = t
                        if spec.rec_pump:
                            out["cost"] += pump_cost(cand, spec.rec_pump)
                    else:
                        out["floor_refused"] += 1
                continue
            if t < fp_t0 + m.P.pool_fast_window_s:
                if spec.win_move and t == fp_t0 + spec.win_move[1]:
                    cand.move_to_price(int(cand.spot_wad() * spec.win_move[0]))
                if kon:
                    m.advance(t)
                    m.observe(t)                       # prologue on the incumbent
                    m.win_observe(t, cand)
                for mv in as_tuple(spec.win_lp):
                    m.advance(t)
                    m.observe(t)
                    m.win_observe(t, wrapped(cand, mv))
                continue
            # --- the execution --------------------------------------------------------------------
            if not (kon or spec.exec_move):
                continue
            m.advance(t)
            m.observe(t)
            s = wrapped(cand, spec.exec_move)
            chain_ok = (m.pool_fail_since is not None and m.pool_fail_since + m.P.pool_fast_route_s <= fp_t0
                        and m.pool_fail())
            if chain_ok and m.floor_at_execution(s):
                m.win_adopt(cand, t, s)
                t_adopt = t
                out.update(adopted=True, t_adopt=t, l_ratio=m.liq / cand.sqrt_k(), p_adopt=cand.spot_wad(),
                           ema_adopt=m.ema, p_ratio=m.ema / cand.spot_wad(), t_clear=None if m.guard else 0)
            else:
                out["cancelled"] += 1
                fp_t0 = None
            continue
        # --- after the adoption: keeper crank + liquidation, attacker's freeze and wrapped liquidation
        tr = t - t_adopt
        done = False
        for actor in (("attacker", "keeper") if spec.att_first else ("keeper", "attacker")):
            if actor == "keeper" and kon:
                m.step_to(t)
                if tr < spec.no_exit_until:
                    continue
                r = m.try_exit(pos, t)
                if r is not None:
                    out.update(exit=r, t_exit_rel=tr, who="keeper")
                    done = True
                    break
            if actor == "attacker":
                if tr >= spec.freeze_from:
                    for mv in as_tuple(spec.freeze):
                        obs_wrapped(m, t, mv)
                if spec.att_liq:
                    r = try_liq(m, pos, t, spec.att_liq)
                    if r is not None:
                        out.update(exit=r, t_exit_rel=tr, who="attacker")
                        done = True
                        break
        if done:
            break
        if out["t_clear"] is None and not m.guard:
            out["t_clear"] = tr
        if "p_gate" not in out and m.warm == 0 and m.venue.has_price():
            out["p_gate"] = m.ema / m.venue.spot_wad()
        if tr <= 60:
            P_lo, P_hi, L_lo, L_hi = m.settle_box(t)
            if (L_hi - L_lo) * BPS > DEFAULT.fv_dev_bps * L_lo:
                wide += 1
        if tr >= spec.horizon:
            break
    out["box_wide"] = wide
    out["m"] = m
    out["t_end"] = t
    return out


def lender_pnl(o):
    r = o["exit"]
    if r is None:
        return 0
    pos = o["pos"]
    return r["lender_sol"] - (pos.Q - r["tokens"]) * r["spot"] // WAD


def pcell(o, with_money=True):
    if not o["adopted"]:
        return "no adoption" + (f" (refused {o['floor_refused']}, cancelled {o['cancelled']})"
                                if (o["floor_refused"] or o["cancelled"]) else "")
    r = o["exit"]
    if r is None:
        return f"open ({o['t_end'] - o['t_adopt']} s)"
    who = "" if o["who"] == "keeper" else " by attacker"
    s = f"{r['kind']} +{o['t_exit_rel']} s{who} @{r['spot'] / o['p_pre']:.3f}x"
    if with_money:
        s += f", lender {sol(lender_pnl(o)):+.3f}, borrower {sol(r['borrower']):.3f}"
    return s


def lratio(o):
    return "-" if not o["adopted"] else f"{o['l_ratio']:.3f}"


# ======================================================================================================
# Engine M (multi-sample's run_plan)
# ======================================================================================================
P0X = 150_000_000_000_000
TH_M = 60


def ms_withdraw(v, lp):
    dx, dy = v.X * lp // v.lp_units, v.Y * lp // v.lp_units
    v.X -= dx
    v.Y -= dy
    v.lp_units -= lp


def ms_deposit(v, lp):
    dx, dy = cdiv(v.X * lp, v.lp_units), cdiv(v.Y * lp, v.lp_units)
    v.X += dx
    v.Y += dy
    v.lp_units += lp


def with_move(v, move, fn):
    """Runs fn() with pool v wrapped in `move`; the round trip restores the pool (bar rounding).
    move: None | 'wd' (every unlocked LP withdrawn) | 'dep' (as much again as the LP counter deposited) |
    ('pump', f) / ('dump', f) (spot moved by a factor f with a swap, reversed after, reserves restored)."""
    if move is None:
        return fn()
    if move == "wd":
        lp = v.lp_units - v.lp_locked
        if lp <= 0:
            return fn()
        ms_withdraw(v, lp)
        try:
            return fn()
        finally:
            ms_deposit(v, lp)
    if move == "dep":
        lp = v.lp_units
        ms_deposit(v, lp)
        try:
            return fn()
        finally:
            ms_withdraw(v, lp)
    kind, f = move
    X = v.X
    snap = (v.X, v.Y, v.lp_units)
    if kind == "pump":
        d = X - math.isqrt(X * X * BPS // int(f * BPS))       # X' = X/sqrt(f)
        paid = v.buy(d)
        try:
            return fn()
        finally:
            got = v.sell(d)
            v.wrap_cost = getattr(v, "wrap_cost", 0) + paid - got
            v.X, v.Y, v.lp_units = snap
    if kind == "dump":
        d = math.isqrt(X * X * int(f * BPS) // BPS) - X       # X' = X*sqrt(f)
        got = v.sell(d)
        try:
            return fn()
        finally:
            paid = v.buy(d)
            v.wrap_cost = getattr(v, "wrap_cost", 0) + paid - got
            v.X, v.Y, v.lp_units = snap
    raise ValueError(move)


def sample_of(v, move):
    return with_move(v, move, lambda: (v.spot_wad(), v.sqrt_k()))


def ms_market(rule):
    v = RaydiumPool(P0X, 100 * LAMPORTS_PER_SOL, fee_ppm=2500, out_share_ppm=120_000, name="AMMv4", lock_bps=9978)
    return RULE_CLS[rule](v, TIER_C, DEFAULT)


def ms_cand(sol_at_p0=52.0, lam=5000):
    y = int(sol_at_p0 * LAMPORTS_PER_SOL)
    x = P0X * y // (100 * LAMPORTS_PER_SOL)
    return RaydiumPool(x, y, fee_ppm=2500, out_share_ppm=160_000, name="CPMM", lock_bps=lam)


def run_plan(rule, mult=1.0, q_bps=40, rec_move=None, exec_move=None, post_crank="wd", post_liq="wd",
             secs=90, pre_mult=1.0):
    m = ms_market(rule)
    P = m.P
    Q = m.venue.X * q_bps // BPS
    pos = m.open(Q, 0)
    P0 = pos.spot_post
    t = 0
    for t in range(1, TH_M + 1):
        if t == 1 and pre_mult != 1.0:
            m.exo_move(int(P0 * pre_mult))
        m.step_to(t)
        if pre_mult != 1.0 and m.try_exit(pos, t) is not None:
            return dict(adopted=False, result="liquidated before the halt", pos=pos, m=m), m
    m.venue.disabled = True
    c = ms_cand()
    c.move_to_price(int(P0 * mult))
    out = dict(P0=P0, Q=Q, pos=pos, exit=None, exit_by=None, exit_s=None, adopted=False, c=c, m=m)
    while True:
        t += 1
        m.step_to(t)
        if m.pool_fail_since is not None and t >= m.pool_fail_since + P.pool_fast_route_s:
            break
    D = c.locked_depth()
    p_rec, l_rec = sample_of(c, rec_move)
    ok = position_floor_ok(D, m.ema) if rule == "built" else window_floor_ok(D, m.ema, p_rec)
    if not ok:
        out.update(result="no window (window floor)")
        return out, m
    due = t + P.pool_fast_window_s
    while t < due - 1:
        t += 1
        m.step_to(t)
    t = due
    m.step_to(t)
    if not position_floor_ok(D, m.ema):
        out.update(result="cancelled at execution (window floor)")
        return out, m
    if rule == "built":
        _, l_live = sample_of(c, exec_move)
        m.adopt_pool(c, t, l_rec=l_rec, l_live=l_live)
    else:
        m.adopt_pool(c, t)
    out.update(adopted=True, t_adopt=t, l_ratio=m.liq / c.sqrt_k(), p_ratio=m.ema / c.spot_wad())
    for s in range(1, secs + 1):
        t += 1
        r = None
        m.step_to(t)
        r = m.try_exit(pos, t)
        if r is not None:
            out.update(exit=r, exit_by="keeper", exit_s=s)
        if r is None and post_crank is not None:
            with_move(c, post_crank, lambda: m.observe(t))
        if r is None and post_liq is not None:
            r = with_move(c, post_liq, lambda: m.try_exit(pos, t))
            if r is not None:
                out.update(exit=r, exit_by="attacker", exit_s=s)
        if r is not None:
            break
    out["m"] = m
    out["t_end"] = t
    return out, m


def honest_health(mult, q_bps=40):
    """CR of the position on the candidate's honest state (live quote at the true depth / at D)."""
    m = ms_market("m4r")
    Q = m.venue.X * q_bps // BPS
    pos = m.open(Q, 0)
    c = ms_cand()
    c.move_to_price(int(pos.spot_post * mult))
    R = m.rent_due(pos)
    q = c.quote_at(Q, *c.eff(), cap_ppm=m.cap_ppm())
    qd = with_move(c, "wd", lambda: c.quote_at(Q, *c.eff(), cap_ppm=m.cap_ppm()))
    return (pos.V - R) * BPS // q[1], (pos.V - R) * BPS // qd[1]


def mcell(o):
    if not o["adopted"]:
        return o["result"]
    r = o["exit"]
    if r is None:
        return "open"
    return f"{r['kind']} by {o['exit_by']} +{o['exit_s']} s"


def mpnl(o):
    """borrower P&L / lender P&L vs holding the tokens / attacker's keeper reward (SOL) at the candidate's final
    price; an open position is marked at its live buyback cost + rent."""
    pos, m, c = o["pos"], o["m"], o["c"]
    p_term = c.spot_wad()
    r = o["exit"]
    if r is None:
        st = m.status(pos, o["t_end"])
        mtm = st["qs"][1] + st["R"]
        return f"{(pos.V - mtm - pos.fee_open - pos.M) / 1e9:+.4f} / +0.0000 / 0.0000"
    b = r["borrower"] - pos.M - pos.fee_open
    lnd = r["lender_sol"] - (pos.Q - r["tokens"]) * p_term // WAD
    att = r["keeper"] if o["exit_by"] == "attacker" else 0
    return f"{b / 1e9:+.4f} / {lnd / 1e9:+.4f} / {att / 1e9:.4f}"


# ======================================================================================================
# Parts
# ======================================================================================================
WRAPS = (("none", None, None), ("withdraw around execute", None, "wd"),
         ("withdraw around proposal", "wd", None), ("withdraw around both", "wd", "wd"))


def part1():
    print("## 1. M4R-1: a lambda 50 % candidate (52 SOL at the pre-halt price) whose every unlocked LP unit the")
    print("attacker holds; the position carried from a 100-SOL fully locked AMM v4 (Tier C, cr_liq 1.5).  The")
    print("candidate trades at m x the pre-halt price, flat after the adoption.  Every second after the adoption:")
    print("the keeper's honest crank and plain liquidation attempt; the attacker's [withdraw all, crank, deposit]")
    print("and [withdraw all, liquidate, deposit].  Cells: outcome, k = keeper / a = attacker, seconds after the")
    print("adoption, (borrower SOL at the exit, or equity if open).  120 s.  The honest threshold is m 2.01 (the")
    print("'none' rows): every earlier exit in a wrapped row is forced.")
    print()
    for q in (40, 74):
        print(f"### q = {q / 100:.2f} % of the incumbent's X")
        print()
        hdr(["m", "LP wraps of the two samples"] + [RULE_NAME[r] for r in RULES2])
        for mult in (1.96, 1.97, 1.98, 1.99, 2.00, 2.01, 2.02, 2.05):
            for name, rm, em in WRAPS:
                cells = [wcell(run_fast_path(r, Cfg(cand_mult=mult, q_bps=q, rec_move=rm, exec_move=em,
                                                   att_crank="wd", att_liq="wd", secs=120))) for r in RULES2]
                row([f"{mult:.2f}", name] + cells)
        print()
    print("Control (q 0.40 %, withdraw around execute, NO per-second wrapped crank; the attacker still tries a")
    print("withdraw-wrapped liquidation every second):")
    print()
    hdr(["m"] + [RULE_NAME[r] for r in RULES2])
    for mult in (1.98, 1.99, 2.00):
        row([f"{mult:.2f}"] + [wcell(run_fast_path(r, Cfg(cand_mult=mult, exec_move="wd", att_liq="wd", secs=120)))
                               for r in RULES2])
    print()


def part2():
    print("## 2. lambda < lambda_min candidates under the M4R-1 attacker")
    print()
    print("### 2a. 240 SOL at 25 % / 400 SOL at 10 % (engine W; withdraw-wrapped execute, per-second withdraw-")
    print("wrapped crank and liquidation), q 0.74 %, flat at m.  Honest: liquidatable at 2.05, not at 2.03.")
    print()
    hdr(["candidate", "m", "honest (M4R)"] + [RULE_NAME[r] for r in RULES2])
    for sol_, lock in ((240, 2500), (400, 1000)):
        for mult in (1.99, 2.00, 2.01, 2.02, 2.03, 2.05):
            h = wcell(run_fast_path("m4r", Cfg(cand_sol=sol_, cand_lock=lock, cand_mult=mult, secs=120, q_bps=74)))
            cells = [wcell(run_fast_path(r, Cfg(cand_sol=sol_, cand_lock=lock, cand_mult=mult, exec_move="wd",
                                                att_crank="wd", att_liq="wd", secs=120, q_bps=74)))
                     for r in RULES2]
            row([f"{sol_} SOL, {lock / 100:.0f} %", f"{mult:.2f}", h] + cells)
    print()
    print("### 2b. The fine grid of a 104-SOL lambda 25 % candidate (engine P, R100, position at the size cap):")
    print("honest vs attacker a2 (every sample withdraw-wrapped, its wrapped liquidation first in each second).")
    print()
    hdr(["rule", "m", "honest only", "a2 attacked", "L_ema / √(X·Y) at the switch (a2)"])
    for rule in RULES2:
        for mm in (2.00, 2.01, 2.02, 2.03, 2.04):
            h = switch_run(Spec(cand_sol=104, cand_lock=2500, m=mm, horizon=120), rule)
            a = switch_run(Spec(cand_sol=104, cand_lock=2500, m=mm, rec_move="wd", exec_move="wd", win_lp="wd",
                                freeze="wd", att_liq="wd", att_first=True, horizon=120), rule)
            row([RULE_NAME[rule], f"{mm:.2f}", pcell(h, False), pcell(a, False), lratio(a)])
    print()


def diag(o):
    la = "-" if o["l_adopt"] is None else f"{o['l_adopt']:.3f}"
    lw = "-" if o["l_warm"] is None else f"{o['l_warm']:.3f}"
    return la, lw, o["wide_warm"], o["wide_after"]


def part3():
    print("## 3. Honest fast path onto thinner / deeper (M4-SEC-6) / lambda < lambda_min (M4V-3) candidates;")
    print("incumbent 100 SOL fully locked; honest keeper; then a 2.3x/120 s pump from +30 s (q 0.40 %).  Columns:")
    print("L_ema / √(X·Y) right after the adoption and at the gate (M4R), seconds the box is wider than 1 % in L")
    print("during the warm-up / in the 60 s after it (M4 as built: the first 60 s), the exit in the pump, and the")
    print("second the guard cleared (M4R).")
    print()
    hdr(["candidate (SOL at the pre-halt price, lambda)", "rule", "L at adoption", "L at the gate",
         "box wide: warm-up / after", "exit in the pump", "guard cleared"])
    for sol_, lock in ((30, 9978), (52, 5000), (150, 9000), (150, 9978), (120, 2500), (240, 2500), (400, 1000)):
        for rule in RULES2:
            o = run_fast_path(rule, Cfg(cand_sol=sol_, cand_lock=lock, cand_mult=1.0, path=ramp(30, 2.3, 120),
                                        secs=400))
            la, lw, ww, wa = diag(o)
            tc = "-" if rule == "built" else f"+{o['t_clear']} s"
            row([f"{sol_} SOL, {lock / 100:.2f} %", RULE_NAME[rule], la, lw, f"{ww} / {wa}", wcell(o), tc])
    print()


UP = [("none", {}),
      ("deposit around the recording proposal (M4V-2)", dict(rec_move="dep")),
      ("deposit around the execution", dict(exec_move="dep")),
      ("deposit around both", dict(rec_move="dep", exec_move="dep")),
      ("deposit freeze after the adoption only", dict(freeze="dep")),
      ("both + deposit freeze after", dict(rec_move="dep", exec_move="dep", freeze="dep"))]


def part4():
    print("## 4. Capital deposit freeze: the attacker deposits as much again as the pool's LP counter around")
    print("sampling instructions (capital only, no swap fee) and around one crank every second from the adoption")
    print("(up-freeze).  'M4R without the guard' is an analysis row: the exit reference would read the frozen L_ema.")
    for inc, cs, title in (("RA", 630, "RA -> 630 SOL, lambda 99.78 % (D = 50 % of pool_d; q = 3.0 % of the "
                                       "candidate's X)"),
                           ("RA", 314, "RA -> 314 SOL, lambda 99.78 % (D = 25 % of pool_d, the fast-path minimum; "
                                       "q = 6.0 %)")):
        print()
        print(f"### 4a. {title}; genuine pump 3x/120 s from the adoption (engine P).  Cells: L_ema / √(X·Y) at the")
        print("switch; seconds with the box > 1 % wide in L (first 60 s); outcome (time after the adoption, price vs")
        print("pre-halt, lender P&L at the exit price, borrower remainder).")
        print()
        rules = ("built", "m4r", "noguard")
        hdr(["LP move"] + [RULE_NAME[r] for r in rules])
        for name, kw in UP:
            cells = [name]
            for rule in rules:
                o = switch_run(Spec(inc=inc, cand_sol=cs, cand_lock=9978, post=ramp_path(1, 3.0, 120), horizon=400,
                                    **kw), rule)
                cells.append(f"{lratio(o)}; {o['box_wide']} s; {pcell(o)}")
            row(cells)
    print()
    print("### 4b. Engine W (honest keeper every second): near-threshold rows and the big carried position (1 %-fee")
    print("incumbent of 200 SOL at lambda 100 %, q at the size cap, onto a 50-SOL lambda 99.78 % 1 %-fee candidate,")
    print("D = 25 % of pool_d).  Cells: outcome, seconds after the adoption (borrower / lender SOL, or equity if")
    print("open).  Honest threshold of the q 0.40 % position: m 2.01.")
    print()
    hdr(["attack", "m"] + [RULE_NAME[r] for r in RULES2])
    big = dict(inc_sol=200, inc_fee=10000, cand_fee=10000, cand_sol=50, cand_lock=9978, q_bps=250)
    rows = [
        ("deposit-wrapped crank every second (up-freeze)", 2.01, dict(att_crank="dep")),
        ("deposit-wrapped crank every second (up-freeze)", 2.02, dict(att_crank="dep")),
        ("deposit-wrapped crank every second (up-freeze)", 2.03, dict(att_crank="dep")),
        ("deposit-wrapped crank every second (up-freeze)", 2.05, dict(att_crank="dep")),
        ("deposit-wrapped crank every second + deposit-wrapped execute", 2.05, dict(att_crank="dep", exec_move="dep")),
        ("deposit-wrapped crank + withdraw-wrapped liquidation every second", 1.99,
         dict(att_crank="dep", att_liq="wd")),
        ("big carried position, no attacker", 2.0, dict(big)),
        ("big carried position, deposit-wrapped crank every second (capital = the pool)", 2.0,
         dict(big, att_crank="dep")),
        ("big carried position, deposit-wrapped crank every second (capital = the pool)", 2.5,
         dict(big, att_crank="dep")),
        ("big carried position, deposit-wrapped crank every second (capital = the pool)", 3.0,
         dict(big, att_crank="dep")),
    ]
    for name, mult, kw in rows:
        row([name, f"{mult:.2f}"] + [wcell(run_fast_path(r, Cfg(cand_mult=mult, secs=300, **kw)), True)
                                     for r in RULES2])
    print()
    part4c()


def flat_then_ramp(w):
    """Flat at the adoption price for w seconds, then the part-4 genuine pump 3x over 120 s."""
    return lambda s: 1.0 if s <= w else 1.0 + 2.0 * min(s - w, 120) / 120


def part4c():
    print("### 4c. The freeze held (M4X-2): the deposit freeze runs from the adoption for W seconds at a flat price,")
    print("then the part-4 genuine pump 3x/120 s (engine P; the freeze goes on).  dep = capital of the pool's size,")
    print("dep3 = three times.  Before M4X-2, L_slow integrated the frozen L_ema (lobs) while the guard was set --")
    print("+1/tau_slow per second -- and L_b = max(L_live, min(L_ema, L_mlo), L_slow) took it unguarded: the")
    print("reference blocked again after 10-30 min and the pump ended in S1.  Cells: outcome; L_slow / sqrt(X*Y) at")
    print("the end.")
    print()
    rules = ("m4r", "noslow")
    hdr(["candidate", "freeze", "W (s)"] + [RULE_NAME[r] for r in rules])
    for cs, fr, w in ((314, "dep", 600), (314, "dep", 1800), (314, "dep3", 600), (630, "dep3", 600),
                      (630, "dep3", 1800)):
        cells = [f"RA -> {cs} SOL, lambda 99.78 %", fr, w]
        for rule in rules:
            o = switch_run(Spec(inc="RA", cand_sol=cs, cand_lock=9978, post=flat_then_ramp(w), horizon=w + 400,
                                freeze=fr), rule)
            m = o["m"]
            cells.append(f"{pcell(o)}; {m.liq_slow / m.venue.sqrt_k():.3f}")
        row(cells)
    print()


def part5():
    print("## 5. Withdraw contest while the guard is set: RA -> 625 SOL, lambda 50 % (D = 25 % of pool_d; q = 3 % of")
    print("the candidate's X); candidate at 1.60x the pre-halt price (the position is liquidatable on it).  The")
    print("attacker holds the candidate's unlocked LP and capital (engine P).  Cells: outcome and the buyback B paid;")
    print("the overpay is the borrower's, against the honest liquidation of the same rule.")
    print()
    hdr(["attacker", "rule", "outcome", "B (SOL)", "borrower overpay vs honest"])
    atts = [("none (honest)", {}),
            ("withdraw-wrapped liquidation only, lands first", dict(att_liq="wd", att_first=True)),
            ("deposit freeze + both samples deposit-wrapped (blocker)",
             dict(rec_move="dep", exec_move="dep", win_lp="dep", freeze="dep")),
            ("combined: every sample both ways, withdraw-wrapped liquidation first",
             dict(rec_move="wd", exec_move="dep", win_lp=("wd", "dep"), freeze=("wd", "dep"), att_liq="wd",
                  att_first=True))]
    for rule in RULES2:
        base = switch_run(Spec(inc="RA", cand_sol=625, cand_lock=5000, m=1.60, horizon=120), rule)
        for name, kw in atts:
            o = switch_run(Spec(inc="RA", cand_sol=625, cand_lock=5000, m=1.60, horizon=120, **kw), rule)
            B = o["exit"]["B"] if o["exit"] else None
            Bh = base["exit"]["B"] if base["exit"] else None
            over = f"{(B - Bh) * 100 / Bh:+.2f} %" if (B and Bh) else "-"
            row([name, RULE_NAME[rule], pcell(o), f"{sol(B):.4f}" if B else "-", over])
    print()


def part6():
    print("## 6. M4R-2: the token dumps f x during the halt; the only candidate has 30 SOL of locked SOL at the")
    print("pre-halt price (lambda 99.78 %, D = 30 % of pool_d), i.e. 30/sqrt(f) SOL at the live price (window floor")
    print("25 SOL).  Attacker: none / the thin pool's backer pump-wraps the recording proposal back to the frozen")
    print("price; or no dump during the halt but a genuine one 5 s into the collection window.  Cells: adopted?; P_ema")
    print("vs the candidate's price at the switch (M4R: -> at the gate); real locked SOL at the live price; the")
    print("attacker's round-trip cost (engine P).")
    print()
    hdr(["f", "attacker"] + [RULE_NAME[r] for r in RULES2])
    cases = []
    for f in (4, 16):
        cases += [(f, "none", dict(m=1.0 / f)),
                  (f, "pump-wrapped recording proposal", dict(m=1.0 / f, rec_pump=f * 1.0)),
                  (f, "none; the dump comes 5 s INTO the window (M4X-3)", dict(win_move=(1.0 / f, 5)))]
    for f, name, kw in cases:
        cells = [f"{f}x", name]
        for rule in RULES2:
            o = switch_run(Spec(cand_sol=30, cand_lock=9978, horizon=30, **kw), rule)
            if o["adopted"]:
                c = o["m"].venue
                ls = locked_sol(c.locked_depth(), c.spot_wad())
                pg = f" -> {o['p_gate']:.2f}" if (rule != "built" and "p_gate" in o) else ""
                cells.append(f"adopted; P_ema/P {o['p_ratio']:.2f}{pg}; {sol(ls):.1f} SOL locked; cost "
                             f"{sol(o['cost']):.2f} SOL")
            else:
                cells.append(f"not adopted (refused {o['floor_refused']}, cancelled {o['cancelled']}); cost "
                             f"{sol(o['cost']):.2f} SOL")
        row(cells)
    print()
    print("A dump inside the 60-s window (after the proposal that opened it) is judged only by the execution, at")
    print("P_ema (M4X-3): adopted under both rules; the warm-up then re-seeds P_ema at the live price (opens and")
    print("tiers see the pool as it is).  A re-check at the execution against P_cand would either revert (and hold")
    print("every other proposal, M4V-4) or cancel (and let a dump-wrapped execution cancel windows until S3).")
    print()


def part7():
    print("## 7. Genuine moves (no attacker; honest keeper every second): exits must still happen.")
    print()
    print("### 7a. Engine W, q 0.40 %.  Cells: outcome, seconds after the adoption, price vs pre-halt at the exit,")
    print("(borrower / lender SOL).  The dump row: the M4R proposal judges the 52-SOL lambda 50 % candidate at its")
    print("live price (13 SOL locked < 25 SOL, M4R-2) and opens no window.")
    print()
    hdr(["scenario"] + [RULE_NAME[r] for r in RULES2])
    cases = [("pump during the halt: candidate at 2.2x, flat", dict(cand_mult=2.2)),
             ("pump during the halt: 3x, flat", dict(cand_mult=3.0)),
             ("pump during the halt: 5x, flat", dict(cand_mult=5.0)),
             ("pump during the halt: 16x, flat", dict(cand_mult=16.0)),
             ("step 2.2x at +1 s", dict(cand_mult=1.0, path=step(1, 2.2))),
             ("step 5x at +1 s", dict(cand_mult=1.0, path=step(1, 5.0))),
             ("ramp 2.2x/60 s from +1 s", dict(cand_mult=1.0, path=ramp(1, 2.2, 60))),
             ("ramp 5x/60 s from +1 s", dict(cand_mult=1.0, path=ramp(1, 5.0, 60))),
             ("ramp 2.2x/120 s from +300 s", dict(cand_mult=1.0, path=ramp(300, 2.2, 120), secs=700)),
             ("dump 4x during the halt, then 10x/60 s from +1 s", dict(cand_mult=0.25, path=ramp(1, 10.0, 60, 0.25))),
             ("lambda 99.78 % 30-SOL candidate: pump during the halt 2.0x, flat",
              dict(cand_sol=30, cand_lock=9978, cand_mult=2.0)),
             ("lambda 99.78 % 30-SOL candidate: pump during the halt 3x, flat",
              dict(cand_sol=30, cand_lock=9978, cand_mult=3.0)),
             ("lambda 99.78 % 30-SOL candidate: step 2x at +1 s", dict(cand_sol=30, cand_lock=9978, path=step(1, 2.0))),
             ]
    for name, kw in cases:
        kw.setdefault("secs", 300)
        row([name] + [wcell(run_fast_path(r, Cfg(**kw)), True) for r in RULES2])
    print()
    print("### 7b. The big carried position (part 4b's markets), pump during the halt to m, flat (engine W).")
    print()
    hdr(["m", "q / x_cand"] + [RULE_NAME[r] for r in RULES2])
    for mult in (1.6, 1.8, 2.0, 2.5):
        os_ = [run_fast_path(r, Cfg(inc_sol=200, inc_fee=10000, cand_fee=10000, cand_sol=50, cand_lock=9978,
                                    q_bps=250, cand_mult=mult, secs=300)) for r in RULES2]
        row([f"{mult:.1f}", f"{os_[0]['q_x'] * 100:.2f} %"] + [wcell(o, True) for o in os_])
    print()
    print("### 7c. Engine P: candidate at 50 % of the incumbent's reserves (R100: 50 SOL), lambda 90 %; 'during the")
    print("halt' = the candidate trades at m x the pre-halt price when the window opens; 'after' = the adopted pool")
    print("moves from the adoption on.  Cells: outcome (time after the adoption, price vs pre-halt, lender P&L at")
    print("the exit price, borrower remainder).")
    print()
    hdr(["market", "move"] + [RULE_NAME[r] for r in RULES2])
    moves = [("during the halt: x2", dict(m=2.0)), ("during the halt: x5", dict(m=5.0)),
             ("during the halt: x0.5, then flat", dict(m=0.5)),
             ("after: step x2 at +1 s", dict(post=step_path(1, 2.0))),
             ("after: step x5 at +1 s", dict(post=step_path(1, 5.0))),
             ("after: ramp x2 over 30 s", dict(post=ramp_path(1, 2.0, 30))),
             ("after: ramp x5 over 60 s", dict(post=ramp_path(1, 5.0, 60)))]
    for inc, cs in (("R100", 50), ("RA", 1250), ("RC", 43)):
        for name, kw in moves:
            row([inc, name] + [pcell(switch_run(Spec(inc=inc, cand_sol=cs, cand_lock=9000, horizon=300, **kw), r))
                               for r in RULES2])
    print()
    print("### 7d. Engine M: a genuine pump during the halt, flat after (honest keeper only).")
    print()
    hdr(["candidate price"] + [RULE_NAME[r] for r in RULES2])
    for mult in (2.1, 2.5, 3.0, 5.0):
        row([f"{mult}x"] + [mcell(run_plan(r, mult=mult, post_crank=None, post_liq=None, secs=300)[0])
                            for r in RULES2])
    print()


def part8():
    print("## 8. Degraded mode: no honest observer during the warm-up.")
    print()
    print("### 8a. Engine W, m 1.99 (healthy on the pool as it trades; honest threshold 2.01).  (a) The keeper is")
    print("absent for the first k seconds after the adoption; the M4R-1 attacker is present every second")
    print("(withdraw-wrapped execute, cranks and liquidation attempts).  (b) Nobody observes for 600 s after the")
    print("adoption, then the keeper; genuinely liquidatable at 2.2x.  (c) The adopted pool halts at +3 s (warm-up")
    print("incomplete), 2.2x; keeper cranks every second.  (d) A genuine held withdrawal of every unlocked LP unit")
    print("for s in [1, k), re-deposited at k, then the per-second withdraw-wrapped freeze and liquidation.  (e), (f)")
    print("A held price push through the warm-up (the §12.6 held model).")
    print()
    hdr(["case"] + [RULE_NAME[r] for r in RULES2])
    for k in (2, 4, 8, 9, 13, 31):
        row([f"(a) keeper absent s < {k}"] + [wcell(run_fast_path(r, Cfg(
            cand_mult=1.99, exec_move="wd", att_crank="wd", att_liq="wd", keeper=lambda s, k=k: s >= k, secs=150)))
            for r in RULES2])
    row(["(b) no observation for 600 s, then the keeper (2.2x)"] +
        [wcell(run_fast_path(r, Cfg(cand_mult=2.2, observe_from=600, secs=900))) for r in RULES2])
    row(["(c) the new pool halts at +3 s (2.2x), 1,500 s"] +
        [wcell(run_fast_path(r, Cfg(cand_mult=2.2, new_halt_at=3, secs=1500))) for r in RULES2])
    for k in (4, 9, 21, 41):
        row([f"(d) held withdrawal s in [1, {k}), then withdraw-wrapped freeze"] + [wcell(run_fast_path(r, Cfg(
            cand_mult=1.99, held=(1, k, "wd"), att_crank="wd", att_liq="wd", secs=200))) for r in RULES2])
    row(["(e) the attacker holds the pool at 1.2x for s in [1, 9) (every observation sees it), withdraw-wrapped "
         "liquidation every second"] + [wcell(run_fast_path(r, Cfg(cand_mult=1.99, held_price=(1, 9, 1.2),
                                                                   att_liq="wd", secs=300)), True) for r in RULES2])
    row(["(f) as (e), the keeper absent for s in [1, 9)"] + [wcell(run_fast_path(r, Cfg(
        cand_mult=1.99, held_price=(1, 9, 1.2), att_liq="wd", keeper=lambda s: s >= 9, secs=300)), True)
        for r in RULES2])
    print()
    print("### 8b. Engine M: no keeper in the window nor after; the recording proposal is pump-wrapped (3x) and the")
    print("attacker holds the adopted pool at mult x the true price every second (held model: every observation")
    print("of the second is pushed) until the position would be liquidatable, then sandwiches the liquidation.")
    print("Cells: P_ema / P at the switch; the best mult, the seconds held, the attacker's net (SOL).")
    print()
    hdr(["rule", "result"])
    for rule in RULES2:
        o, m = run_plan(rule, mult=1.0, rec_move=("pump", 3.0), post_crank=None, post_liq=None, secs=0)
        pos, t0 = o["pos"], o["t_adopt"]
        p_true = m.venue.spot_wad()
        best = None
        for mu in (1.1, 1.2, 1.3, 1.5, 2.0, 3.0):
            mm, pp = copy.deepcopy(m), copy.deepcopy(pos)
            got = push_until(mm, [pp], t0, p_true, mu, "held", lambda a, b: would_liq(a, pp, b), 60)
            if got is None:
                continue
            t, cost, n = got
            sw = sandwich_exit(mm, pp, t + 1)
            if sw is None:
                continue
            net = sw[0] - cost
            if best is None or net > best[0]:
                best = (net, mu, n)
        res = ("not liquidatable in 60 s" if best is None else
               f"x{best[1]} for {best[2]} s, net {best[0] / 1e9:+.4f} SOL")
        row([RULE_NAME[rule], f"P/P {o['p_ratio']:.3f}; " + res])
    print()
    print("### 8c. Steady state for comparison (no switch; the same under both rules; pre-existing).  The position is")
    print("opened on a lambda 50 % 52-SOL CPMM pool; the price walks to m x over 60 s (EMA converged).  At T0 the")
    print("attacker withdraws every unlocked LP unit for k seconds (a GENUINE withdrawal: the keeper's cranks see D),")
    print("re-deposits, then holds L_ema with a withdraw-wrapped crank every second and tries a withdraw-wrapped")
    print("liquidation every second; or k attacker-only seconds (keeper absent, the attacker's cranks withdraw-")
    print("wrapped).  Cells: outcome, seconds after T0, L_ema / √(X·Y) at the end.")
    print()
    ks = (0, 3, 5, 8, 12, 20, 40)
    hdr(["m", "variant"] + [f"k = {k}" for k in ks])
    for mult in (1.99, 2.00, 2.01):
        for label, kw in (("held withdrawal k s, then freeze", lambda k: dict(k_held=k)),
                          ("k attacker-only seconds, then freeze", lambda k: dict(k_held=0, keeper_gap=(0, k)))):
            cells = []
            for k in ks:
                who, s_, r, l = steady_run(mult, **kw(k))
                cells.append(f"{who} +{s_}s, {l:.2f}" if r else f"open, {l:.2f}")
            row([f"{mult:.2f}", label] + cells)
    ref = []
    for mult in (2.00, 2.01, 2.02):
        who, s_, r, l = steady_run(mult, 0, freeze=False, secs=60)
        ref.append(f"m {mult:.2f} {'liquidated' if r else 'healthy'}")
    print()
    print("Reference (no attack, 60 s): " + "; ".join(ref) + ".")
    print()
    part8d()


def bundle_market(rule, extra_bps):
    """Engine W's reproducer market with the position's vault raised by extra_bps of V (add_margin), adopted onto
    the 52-SOL lambda 50 % candidate trading flat at the pre-halt price: (market, position, candidate, t_adopt)."""
    inc = pool_at(100.0, 10_000)
    m = RULE_CLS[rule](inc, TIER_C, DEFAULT)
    pos = m.open(inc.X * 40 // BPS, 0)
    pos.V += pos.V * extra_bps // BPS
    p0 = inc.spot_wad()
    for t in list(range(1, 61)) + list(range(61, TH - 10, 30)) + list(range(TH - 10, TH)):
        m.step_to(t)
    inc.disabled = True
    c = pool_at(52.0, 5000, out_share_ppm=160_000, name="CPMM")
    c.move_to_price(p0)
    t, fp_t0 = TH, None
    while True:
        t += 1
        m.step_to(t)
        if m.pool_fail_since is None:
            continue
        if fp_t0 is None and t >= m.pool_fail_since + m.P.pool_fast_route_s and proposal_floor(rule, m, c):
            fp_t0 = t
            l_rec = c.sqrt_k()
        if fp_t0 is not None and t >= fp_t0 + m.P.pool_fast_window_s:
            if rule == "built":
                m.adopt_pool(c, t, l_rec=l_rec, l_live=c.sqrt_k())
            else:
                m.adopt_pool(c, t)
            return m, pos, c, t


def bundle_run(base, kc, kl, absent, secs=30):
    """The keeper is absent for s <= absent after the adoption, then cranks and tries to liquidate every second;
    every second the attacker sends [buy to kc x, crank_price, sell] and [buy to kl x, liquidate, sell] (atomic
    bundles; a failed liquidation records nothing).  Returns the second of a forced liquidation (the attacker's)
    or None."""
    m, pos, c, t_adopt = copy.deepcopy(base)
    for s in range(1, secs + 1):
        t = t_adopt + s
        if s > absent:
            m.step_to(t)
            if m.try_exit(pos, t) is not None:
                return None
        snap = (c.X, c.Y)
        c.move_to_price(int(c.spot_wad() * kc))
        m.advance(t)
        m.observe(t)
        c.X, c.Y = snap
        m2, pos2 = copy.deepcopy((m, pos))
        m2.venue.move_to_price(int(m2.venue.spot_wad() * kl))
        if m2.try_exit(pos2, t) is not None:
            return s
    return None


def honest_threshold(extra_bps):
    """The candidate's price (x pre-halt, step 0.05) from which the honest keeper liquidates within 30 s (M4R)."""
    for i in range(150, 900, 5):
        mult = i / 100
        m, pos, c, t_adopt = bundle_market("m4r", extra_bps)
        c.move_to_price(int(c.spot_wad() * mult))
        for s in range(1, 31):
            m.step_to(t_adopt + s)
            if m.try_exit(pos, t_adopt + s) is not None:
                return mult
    return None


def part8d():
    print("### 8d. Atomic price bundles in attacker-only seconds (M4X-1).  Engine W's market; the position's vault")
    print("raised by add_margin (+0 / +25 / +50 / +100 % of V) so its honest threshold is about 2-4x the pre-halt")
    print("price; the candidate trades FLAT at 1.00x after the adoption.  The keeper is absent for s <= k after the")
    print("adoption (degraded mode); every second the attacker sends [buy kc, crank_price, sell] and [buy kl, liquidate,")
    print("sell], with its best kc in {1.5 ... 12} and kl in {1.0, 1.1, 1.25, 1.5} x the threshold (the pumped buyback")
    print("must still fit V - K_s).  Cells: the earliest forced liquidation, seconds after the adoption (kc, kl), or")
    print("open (30 s).  Before M4X-1 the warm-up seed read the pumped low directly: 8 attacker-only seconds forced ANY")
    print("threshold at +9 s.  With the step bound the reach is exactly as built's (whose P_ema moves at most")
    print("x(tau + dt*(1 + C))/(tau + dt) per integration, the same bound), and never earlier (the gate).")
    print()
    rules = ("built", "nobound", "m4r")
    ks = (7, 8, 9, 10, 12)
    hdr(["margin", "threshold", "rule"] + [f"keeper absent s <= {k}" for k in ks])
    for extra in (0, 2500, 5000, 10_000):
        th = honest_threshold(extra)
        for rule in rules:
            base = bundle_market(rule, extra)
            cells = []
            for absent in ks:
                best = None
                for kc in (1.5, 2.0, 2.5, 3.0, 4.0, 6.0, 8.0, 12.0):
                    for f in (1.0, 1.1, 1.25, 1.5):
                        s_ = bundle_run(base, kc, th * f, absent)
                        if s_ is not None and (best is None or s_ < best[0]):
                            best = (s_, kc, th * f)
                cells.append("open" if best is None else f"+{best[0]} s ({best[1]:g}, {best[2]:.2f})")
            row([f"+{extra // 100} %", f"{th:.2f}x", RULE_NAME[rule]] + cells)
    print()


def part9():
    print("## 9. Stale-high P_ema after a mild dip (engine M): the incumbent's price rises to pre x the open spot")
    print("(EMA converged there), the incumbent halts, the candidate trades at m x (a mild dip); after the adoption")
    print("the LP holder sends a withdraw-wrapped crank and a withdraw-wrapped liquidation every second after the")
    print("keeper.  CR = the position's CR on the candidate at its true depth / at D (cr_liq 15,000).  Cells: P_ema /")
    print("P at the switch; outcome in 90 s [borrower / lender / attacker's keeper reward, SOL].")
    print()
    hdr(["pre -> m", "CR", "M4 as built", "M4R"])
    for pre, mult in ((2.0, 1.98), (2.005, 1.98), (2.01, 1.98), (2.01, 1.985), (2.01, 1.99), (2.01, 1.95)):
        h, d = honest_health(mult)
        cells = []
        for rule in RULES2:
            o, m = run_plan(rule, mult=mult, pre_mult=pre)
            cells.append(f"P/P {o.get('p_ratio', 0):.3f} {mcell(o)} [{mpnl(o)}]")
        row([f"{pre}->{mult}", f"{h}/{d}"] + cells)
    print()


def steady_run(mult, k_held, q_bps=40, lam=5000, freeze=True, secs=200, keeper_gap=None):
    """No switch (steady state, the same under both rules): the position is opened on a lambda `lam` 52-SOL CPMM
    pool; the price walks to mult x over 60 s (the EMA follows honestly); at T0 the attacker withdraws every
    unlocked LP unit for k_held seconds (genuine: every observation sees D), re-deposits, and from then on sends a
    withdraw-wrapped crank and a withdraw-wrapped liquidation every second.  keeper_gap: (s0, s1) keeper absent
    (attacker-only seconds, his cranks withdraw-wrapped)."""
    c = pool_at(52, lam, 2500, 160_000, "CPMM")
    m = Market(c, TIER_C, DEFAULT)
    pos = m.open(c.X * q_bps // BPS, 0)
    p0 = c.spot_wad()
    for t in range(1, 61):
        m.step_to(t)
    for t in range(61, 121):
        c.move_to_price(int(p0 * (1 + (mult - 1) * (t - 60) / 60)))
        m.step_to(t)
    for t in range(121, 200):
        m.step_to(t)
    T0 = 200
    units = lp_withdraw_all(c) if k_held else 0
    for s in range(0, secs):
        t = T0 + s
        if k_held and s == k_held:
            lp_deposit(c, units)
        keeper = keeper_gap is None or not (keeper_gap[0] <= s < keeper_gap[1])
        if keeper:
            m.step_to(t)
            r = m.try_exit(pos, t)
            if r:
                return ("keeper", s, r, m.liq / c.sqrt_k())
        if freeze and (s >= k_held or not keeper):
            with Wrap(c, "wd"):
                m.advance(t)
                m.observe(t)
            m2, pos2 = copy.deepcopy((m, pos))
            c2 = m2.venue
            wr = Wrap(c2, "wd")
            wr.__enter__()
            r2 = m2.try_exit(pos2, t)
            wr.__exit__()
            if r2:
                return ("attacker", s, r2, m.liq / c.sqrt_k())
        if not keeper and not freeze:
            m.advance(t)
    return ("open", secs, None, m.liq / c.sqrt_k())


FUZZ_ATT = [dict(rec_move="wd", exec_move="wd", att_crank="wd", att_liq="wd"),
            dict(exec_move="wd", att_crank="wd", att_liq="wd", att_price=("pump", 1.5)),
            dict(rec_move="dep", exec_move="wd", att_crank="wd", att_liq="wd"),
            dict(rec_move="wd", exec_move="dep", att_crank="dep", att_liq="wd"),
            dict(att_crank="wd", att_liq="wd")]


def fuzz_forced(n=1200, seed=23):
    """Random candidate (lambda 10-99.78 %, 52-300 SOL), price multiple drawn around the position's own threshold,
    random size; attacker = one of the strongest per-second LP schedules.  FORCED = the attacked run exits while
    the honest-only run of the same rule stays open for 90 s."""
    rng = random.Random(seed)
    cases = []
    for _ in range(n):
        lam = rng.choice((1000, 2500, 5000, 7500, 9000, 9978))
        sol_ = rng.choice((52, 80, 150, 300))
        mult = round(rng.uniform(1.93, 2.06), 3)
        q = rng.choice((20, 40, 74))
        cases.append((lam, sol_, mult, q, rng.choice(FUZZ_ATT)))
    res = {}
    for rule in RULES2:
        forced = honest_open = n_ad = 0
        ex = []
        for lam, sol_, mult, q, att in cases:
            base = dict(cand_sol=sol_, cand_lock=lam, cand_mult=mult, q_bps=q, secs=90)
            h = run_fast_path(rule, Cfg(**base))
            if h["adopt"] is None:
                continue
            n_ad += 1
            if h["exit"] != "open":
                continue
            honest_open += 1
            a = run_fast_path(rule, Cfg(**base, **att))
            if a["exit"] != "open":
                forced += 1
                ex.append((lam, sol_, mult, q, att, a["exit"], a["t_exit"], a["who"]))
        res[rule] = (n_ad, honest_open, forced, ex)
    return cases, res


def part10():
    print("## 10. Targeted random search (engine W): 1,200 runs, seed 23.  Candidate lambda 10-99.78 %, 52-300 SOL,")
    print("m drawn in [1.93, 2.06] around the position's threshold, q 0.20 / 0.40 / 0.74 %; attacker = one of five")
    print("per-second LP schedules (withdraw or deposit around the samples, withdraw-wrapped cranks and liquidations,")
    print("atomic 1.5x price bundles).  FORCED = the attacked run exits while the honest-only run of the same rule")
    print("stays open for 90 s.")
    print()
    cases, res = fuzz_forced()
    hdr(["rule", "adopted", "honest-open", "FORCED",
         "first forced cases (lambda, SOL, m, q bps, attacker, exit, s, by)"])
    for rule in RULES2:
        n_ad, ho, fo, ex = res[rule]
        row([RULE_NAME[rule], f"{n_ad}/{len(cases)}", ho, fo, "; ".join(str(e) for e in ex[:3]) or "-"])
    print()


def selftest():
    """Honest switch (52-SOL lambda 50 % and 150-SOL lambda 99.78 % candidates, flat): L_ema / sqrt(X*Y) at the gate,
    box width after it, the second the guard cleared; and the gate itself."""
    out = []
    for sol_, lock in ((52, 5000), (150, 9978), (240, 2500)):
        o = run_fast_path("m4r", Cfg(cand_sol=sol_, cand_lock=lock, cand_mult=1.0, secs=60))
        out.append(f"honest switch onto {sol_} SOL at lambda {lock / 100:.2f} %: L_ema/sqrt(XY) {o['l_adopt']:.3f} at "
                   f"the adoption, {o['l_warm']:.3f} at the gate (+{o['t_warm']} s); P_ema/P {o['p_warm']:.3f}; box "
                   f"wide {o['wide_warm']} s in the warm-up, {o['wide_after']} s after; guard cleared at "
                   f"+{o['t_clear']} s")
    o = run_fast_path("m4r", Cfg(cand_mult=2.2, secs=30))
    out.append(f"gate: a genuinely liquidatable position (2.2x) is liquidated at +{o['t_exit']} s "
               f"(the 8th warm-up integration lands at +9 s)")
    return "\n".join(out)


PARTS = {"1": part1, "2": part2, "3": part3, "4": part4, "5": part5, "6": part6, "7": part7, "8": part8,
         "9": part9, "10": part10}

if __name__ == "__main__":
    want = sys.argv[1:] or ["selftest"] + list(PARTS)
    for k in want:
        if k == "selftest":
            print("## selftest")
            print()
            print(selftest())
            print()
        else:
            PARTS[k]()
