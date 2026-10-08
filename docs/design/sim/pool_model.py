#!/usr/bin/env python3
"""
short.win -- lender-pool accounting reference model + property tests (v0.4.2).

Companion to docs/design/ECONOMICS.md §9 (normative for the math crate's `pool`
module, ARCH §3.3).  Standard library only; deterministic.

    python3 pool_model.py            # invariant fuzzing, liveness, attack / edge-case demos
    python3 pool_model.py --ops 200000 --seed 7

Model (all integers; rounding in favour of the pool):
  * Token claims: shares are pro-rata claims on T + B (unborrowed + borrowed tokens,
    INTERNAL counters, donations ignored); mint = floor(a*(S+VS)/(T+B+VA)).
  * Lender SOL, option (a) of the round-3 brief (SPEC §2.4 "rent accrues to the pool
    continuously"):
      - entitlement `acc` (WAD lamports/share, signed): rent credited continuously,
        trued up per position at repay/settle (a shortfall at once; M10-8C-1: the rent paid
        above what was credited -- R_min within min_rent_s, a settlement's rent -- vests like
        a lump); lumps and settlement principal;
      - realised allocation `acc_r`: SOL allocated to the shares holding when it
        reaches the vault; a claim pays from the lender's own realised allocation;
      - v0.4 (R3A-2): a lender's realised allocation beyond its entitlement (surplus)
        goes, at its next touch, to a MarketVault-level pot `unmatched`; a claim may
        draw from the pot up to the lender's own entitlement deficit.  O(1), no
        division, no keeper recycling duty (sync_lender stays permissionless for plain
        catch-up; since KI65-REV-SEC1 only the owner converts its position through a rebase
        record -- the model does not check signatures -- and anyone else's sync of a lender
        behind one is the pot-only touch _touch_pot, KI-66, which still moves the surplus
        accrued up to that record to the pot);
        liveness invariant: after all positions resolve, the vest is released and
        every lender is synced, sequential claims pay out lender_sol - dust;
      - v0.4 (R3B-2): LUMP credits (lender half of the penalty, expiry fee, penalty
        part of an S2/S3 take, trader sweeps, venue cashback) enter a vesting
        balance released linearly over lump_vest_s inside accrue, into acc and acc_r
        together; settlement principal stays immediate;
      - v0.4 (R3A-10): SOL credited while S == 0 goes to the live full-wipe record
        (V2-12) or, when there is none, to the protocol (no `unassigned`); the last
        lender to leave is paid the unreleased vest first;
      - v0.4 (R3A-5 / R3B-9): rent_protocol_bps is snapshot per position and folded
        into the aggregate A = sum q*(1e4 - p_pos), so receivable returns to 0
        whenever no position is open, whatever governance changes.
    Option (b) (single realised accumulator, ALL lender SOL released linearly) is the
    Pool(mode="release") comparison model (owner option, ECON §9.4).
  * Withdrawals (SPEC §2.4, C0-9): pro-rata part of unborrowed inventory; the borrowed
    part of what a lender withdrew is locked (survival index PI decaying only below
    the running minimum b_min of B, V2-3) and, v0.4 (R3B-1), every lock expires at
    lock_until.  v0.4.1: lock_until = ceil((max(now, max_release) + lock_grace_s) / 1 d) * 1 d,
    max_release = latest (deadline + that position's own S3 tail) of any position opened so
    far (R4A-8 / R4B-8); the aggregate lock term lives in a day-bucket expiry calendar, so an
    expired lock leaves the aggregate at its lock_until without its holder being touched
    (R4A-2 / R4B-3); lock terms are floored so the aggregate never exceeds the lenders' own
    locks; check() asserts I-P4 on the state the program sees (only the withdrawer touched).
  * v0.4.1 lumps: value-weighted vest end (a lump no longer restarts the window, R4B-5); a
    deposit while a full-wipe record is live waits for the post-wipe sweep (R4A-7).
  * Share epochs (C0-6), 0-share jump and post-wipe record (V2-12) unchanged.
  * v0.4.2 (KI-65): no rebase orphans.  A rebase record (1/den) carries a conversion remainder
    conv_rem in [0, den); a lender's conversion through it mints floor((conv_rem + shares)/den)
    and leaves the remainder there, so the conversions of every old-epoch holder sum to
    floor(s_old/den) = the new S exactly, in any order (each lender gets floor(s/den) or one
    more).  Once every lender converted, sum(shares) == S; R3B-2's "the last lender is paid the
    vest" (a withdrawal of all of S) again reaches every exit, and no SOL is credited to shares
    nobody holds.  v0.4.1 floored each lender separately (up to n - 1 orphan shares per rebase).
  * KI-66: the pot-only touch (_touch_pot), a third party's sync_lender of a lender behind a
    rebase record: it accrues the lender to the record's acc_end/acc_r_end and moves its surplus
    to the pot without converting (no shares, conv_rem or epoch move), so a surplus holder that
    never returns no longer keeps the deficit lenders waiting for the surplus it held at the roll.
"""
from __future__ import annotations

import argparse
import copy
import importlib.util
import os
import random

WAD = 10**18
BPS = 10_000
VS = 1_000        # virtual shares (decimals offset 3)
VA = 1            # virtual token unit
R_MAX = 1_000_000   # max shares per raw token unit before a rebase
REBASE = 1_000
U128 = 2**128 - 1
LUMP_VEST_S = 6 * 3600       # PROPOSED lump_vest_s (ECON §15)
LOCK_GRACE_S = 86_400        # PROPOSED lock_grace_s (ECON §15)
RENT_PROTOCOL_BPS = 1_500    # SPEC default (per position snapshot)
LOCK_BUCKET_S = 86_400       # v0.4.1 (R4A-2 / R4B-3): lock expiry calendar, one bucket per day ...
LOCK_BUCKETS = 128           # ... 128 buckets >= max_duration_s (90 d) + S3 tail (3 d) + lock_grace_s (7 d) + 2 d
S3_TAIL_S = 3_600 + 600 + 600   # default max_migration_pause_s + grad_grace_s + s3_wait_s (R4A-8 / R4B-8)


def cdiv(a, b):
    return -((-a) // b)


class Lender:
    __slots__ = ("shares", "snap", "pending", "epoch", "lock", "lock_pi", "lock_epoch", "lock_term",
                 "lock_until", "snap_r", "real", "claimed", "withdrawn")

    def __init__(self, acc, acc_r, epoch):
        self.shares, self.snap, self.pending, self.epoch = 0, acc, 0, epoch
        self.lock, self.lock_pi, self.lock_epoch, self.lock_term, self.lock_until = 0, WAD, -1, 0, 0
        self.snap_r, self.real = acc_r, 0          # realised SOL allocated to this lender (cumulative)
        self.claimed = 0                           # SOL paid to this lender (cumulative)
        self.withdrawn = 0                         # ghost (tests only)


class Pool:
    def __init__(self, mode: str = "continuous", util_cap_bps: int = 9_000, prorata: bool = True,
                 lump_vest_s: int = LUMP_VEST_S, lock_grace_s: int = LOCK_GRACE_S, pi_decay: bool = True,
                 lock_expiry: bool = True, lock_calendar: bool = True, vest_weighted: bool = True,
                 wipe_sweep_gate: bool = True, exit_flush_bps: int = 0, lock_floor: bool = True,
                 cum_convert: bool = True):
        assert mode in ("continuous", "release")
        self.mode = mode
        self.util_cap = util_cap_bps
        self.prorata = prorata
        self.lump_vest_s = lump_vest_s
        self.lock_grace_s = lock_grace_s
        self.pi_decay = pi_decay                # False: locks clear only at lock_until or B == 0
        self.lock_expiry = lock_expiry          # False: v0.3 behaviour (no lock_until)
        self.lock_calendar = lock_calendar      # False: v0.4 behaviour (expiry leaves lock_norm at the holder's touch)
        self.vest_weighted = vest_weighted      # False: v0.4 behaviour (every lump restarts the whole window)
        self.wipe_sweep_gate = wipe_sweep_gate  # False: v0.4 behaviour (a deposit finalises the wipe record at once)
        self.lock_floor = lock_floor            # False: v0.4 ceil'd lock terms and aggregate (comparison rows)
        self.exit_flush_bps = exit_flush_bps    # analysis only (R4B-4's suggestion, rejected): flush the vest before
                                                #   a withdrawal that leaves S - shares < exit_flush_bps of S
        self.cum_convert = cum_convert          # False: v0.4.1 behaviour (each lender floored alone at a rebase:
                                                #   up to n - 1 orphan shares nobody holds; KI-65 comparison rows)
        self.now = 0
        self.T = 0
        self.B = 0
        self.S = 0
        self.acc = 0             # entitlement, WAD lamports per share (signed)
        self.carry = 0
        self.acc_r = 0           # realised allocation, WAD lamports per share
        self.carry_r = 0
        self.dust = 0            # lamports*WAD dropped at epoch rolls
        self.sol_vault = 0       # MarketVault.lender_sol
        self.unmatched = 0       # MarketVault pot: realised SOL beyond its receivers' entitlement (R3A-2)
        self.protocol = 0        # lender credits that found no lender (S == 0, no live wipe record)
        self.receivable = 0      # rent credited to lenders, not yet paid in
        self.index = 0           # market rent index (lamports per raw token, WAD)
        self.A = 0               # sum over open positions of q*(1e4 - p_pos)
        self.rent_protocol_bps = RENT_PROTOCOL_BPS   # current governance value (new positions only)
        self.vest_bal = 0        # R3B-2: unreleased lump SOL (already in sol_vault)
        self.vest_last = 0
        self.vest_end = 0
        self.epoch = 0
        self.epochs = {}         # e -> [acc_end, num, den, acc_r_end, s_old, carry, carry_r, conv_rem]
        self.wipe_live = False
        self.pi = WAD
        self.pi_epoch = 0
        self.lock_norm = 0       # sum of the live calendar buckets (v0.4.1) = aggregate lock term
        self.lock_cal = {}       # bucket index (lock_until // LOCK_BUCKET_S) -> sum of lock terms expiring then
        self.b_min = 0
        self.max_release = 0     # latest (deadline + S3 tail) of any position opened so far (R3B-1, R4A-8)
        self.wipe_swept = True   # v0.4.1 (R4A-7): trader / venue rewards swept into the live wipe record
        self.lenders = {}
        self.positions = {}      # pid -> [q, index_open, p_rent, deadline]
        self.vault_token_balance = 0
        self.inflow = 0          # ghost: lamports that reached sol_vault for lenders
        self.n_touch = 0         # ghost: rounding-event counter for the dust bound

    # ---- time, rent, vesting --------------------------------------------------
    def advance(self, dt: int, d_index: int = 0):
        """accrue(): time passes (vest release), market rent index grows by d_index."""
        self.now += dt
        self._release()
        if d_index:
            self.index += d_index
            if self.mode == "continuous":
                credited = self.A * d_index // (WAD * BPS)          # floor; A folds (1e4 - p_pos)
                self.receivable += credited
                self._credit(credited)

    def accrue(self, d_index: int):                  # backwards-compatible helper
        self.advance(0, d_index)

    def _release(self):
        if self.vest_bal == 0:
            self.vest_last = self.now
            return
        if self.now >= self.vest_end:
            rel = self.vest_bal
        else:
            rel = self.vest_bal * (self.now - self.vest_last) // (self.vest_end - self.vest_last)
        self.vest_last = self.now
        if rel:
            self.vest_bal -= rel
            self._credit(rel, realised=True)

    def _vest(self, lamports: int):
        """R3B-2: a lump already in sol_vault, released linearly over lump_vest_s."""
        if lamports <= 0:
            return
        self._release()
        if self.S == 0:
            self._credit(lamports, realised=True)   # nobody to vest for: wipe record or protocol
            return
        if self.vest_weighted and self.vest_bal and self.vest_end > self.now:
            # v0.4.1 (R4B-5): value-weighted end time.  While lump_vest_s >= vest_end - now the end never
            # moves earlier and a dust lump moves it at most 1 s later (ceil); after a governance decrease of
            # lump_vest_s below the remaining span a lump pulls the end earlier, towards now + lump_vest_s
            span = cdiv(self.vest_bal * (self.vest_end - self.now) + lamports * self.lump_vest_s,
                        self.vest_bal + lamports)
        else:
            span = self.lump_vest_s
        self.vest_bal += lamports
        self.vest_last = self.now
        self.vest_end = self.now + span

    # ---- lock expiry calendar (v0.4.1, R4A-2 / R4B-3) ----------------------------------
    def _cal_advance(self):
        """Drop every bucket whose end has passed (lock_until = bucket end): the aggregate sees
        an expiry at the same second as the lender's own view, without touching the lender.
        O(buckets elapsed) <= LOCK_BUCKETS on-chain (a ring indexed by day)."""
        if not (self.lock_calendar and self.lock_expiry):
            return
        for b in [b for b in self.lock_cal if b * LOCK_BUCKET_S <= self.now]:
            self.lock_norm -= min(self.lock_cal.pop(b), self.lock_norm)

    def _cal_add(self, until: int, term: int):
        if self.lock_calendar and term:
            b = until // LOCK_BUCKET_S
            self.lock_cal[b] = self.lock_cal.get(b, 0) + term
        self.lock_norm += term

    def _cal_sub(self, until: int, term: int):
        """Remove a LIVE lock's term (callers check now < until and the Pi epoch)."""
        if self.lock_calendar:
            b = until // LOCK_BUCKET_S
            x = min(term, self.lock_cal.get(b, 0))
            if x:
                self.lock_cal[b] -= x
                if self.lock_cal[b] == 0:
                    del self.lock_cal[b]
            self.lock_norm -= min(x, self.lock_norm)
        else:
            self.lock_norm -= min(term, self.lock_norm)

    def lock_until_for(self) -> int:
        """lock_until of a new or extended lock: the latest release of any position opened so far
        (deadline + that position's own S3 tail, R4A-8 / R4B-8) plus lock_grace_s, rounded UP to
        the calendar's bucket end so that lender view and aggregate expire together."""
        x = max(self.now, self.max_release) + self.lock_grace_s
        if self.lock_calendar:
            x = cdiv(x, LOCK_BUCKET_S) * LOCK_BUCKET_S
        return x

    # ---- SOL accumulators ---------------------------------------------------
    def _credit(self, lamports: int, realised: bool = False):
        """Entitlement credit (acc); realised=True also allocates the SOL to shares (acc_r)."""
        if lamports == 0:
            return
        if self.S == 0:
            if self.wipe_live:                 # V2-12: top up the full-wipe record for the old lenders
                rec = self.epochs[self.epoch - 1]
                num = lamports * WAD + rec[5]
                d = num // rec[4]
                rec[5] = num - d * rec[4]
                rec[0] += d
                if realised or self.mode == "release":
                    num = lamports * WAD + rec[6]
                    d = num // rec[4]
                    rec[6] = num - d * rec[4]
                    rec[3] += d
                return
            # R3A-10: no lender to credit -> protocol (only non-position credits can arrive here)
            self.protocol += lamports
            self.sol_vault -= lamports
            return
        num = lamports * WAD + self.carry
        d = num // self.S
        self.carry = num - d * self.S
        self.acc += d
        if realised and self.mode == "continuous":
            self._credit_r(lamports)

    def _credit_r(self, lamports: int):
        if lamports <= 0 or self.S == 0:
            return
        num = lamports * WAD + self.carry_r
        d = num // self.S
        self.carry_r = num - d * self.S
        self.acc_r += d

    def _touch(self, who, pot_only: bool = False) -> Lender:
        self._cal_advance()
        L = self.lenders.get(who)
        if L is None:
            L = self.lenders[who] = Lender(self.acc, self.acc_r, self.epoch)
        while L.epoch < self.epoch:            # lazy epoch conversion (C0-6)
            acc_end, num, den, acc_r_end = self.epochs[L.epoch][:4]
            L.pending += L.shares * (acc_end - L.snap) // WAD
            L.real += L.shares * (acc_r_end - L.snap_r) // WAD
            held = pot_only and num and (L.shares or L.lock or L.lock_term)
            if self.wipe_live and L.epoch == self.epoch - 1 or held:
                # V2-12: parked on the live wipe record; KI-66: the pot-only touch stays on a
                # rebase record, accrued to its ends, unconverted
                L.snap, L.snap_r = acc_end, acc_r_end
                self._to_pot(L)
                return L
            if num and self.cum_convert:       # v0.4.2 (KI-65): cumulative rounding through the record
                rec = self.epochs[L.epoch]
                L.shares, rec[7] = divmod(rec[7] + L.shares, den)
            else:
                L.shares = L.shares * num // den   # a full wipe (conv_rem is not touched); v0.4.1 rebases
            L.lock = L.lock * num // den
            L.lock_term = L.lock_term * num // den     # v0.4.1: keeps the term consistent with the scaled bucket
            L.snap = L.snap_r = 0
            L.epoch += 1
            if L.shares == 0 and L.lock == 0:  # V2-12: nothing left to convert -> jump
                L.epoch = self.epoch
                L.snap, L.snap_r = self.acc, self.acc_r
        if L.shares:
            self.n_touch += 1
        L.pending += L.shares * (self.acc - L.snap) // WAD
        L.snap = self.acc
        L.real += L.shares * (self.acc_r - L.snap_r) // WAD
        L.snap_r = self.acc_r
        if self.mode == "release":
            L.real = L.claimed + max(L.pending, 0)   # (b): everything credited is realised
        self._to_pot(L)
        if L.lock and self.lock_expiry and self.now >= L.lock_until and L.lock_epoch == self.pi_epoch:
            if not self.lock_calendar:
                self.lock_norm -= min(L.lock_term, self.lock_norm)     # v0.4: expired lock leaves lock_norm here
            L.lock, L.lock_term, L.lock_epoch = 0, 0, -1               # v0.4.1: the calendar already dropped it
        return L

    def _touch_pot(self, who) -> Lender:
        """KI-66: the pot-only touch, anyone's sync_lender of a lender it does not own.
        Since KI65-REV-SEC1 only the owner converts its position through a rebase record (the
        conversion order decides who receives the record's rounding units), so this touch stops at
        one: the lender accrues pending/real to the record's acc_end/acc_r_end, keeps them as its
        snapshots (its later conversion accrues 0 there) and moves its surplus to the pot (R3A-2);
        its shares, lock, epoch and the record's conv_rem stay.  Idempotent.  Everywhere else it is
        _touch (wipe catch-up, the V2-12 jump and parking, a current lender).  What the unconverted
        shares earn after the record waits for the owner's conversion."""
        return self._touch(who, pot_only=True)

    def _to_pot(self, L: Lender):
        """R3A-2: realised allocation beyond the lender's entitlement -> MarketVault pot."""
        surplus = (L.real - L.claimed) - max(L.pending, 0)
        if surplus > 0:
            L.real -= surplus
            self.unmatched += surplus

    # ---- C0-6 share epochs ----------------------------------------------------------
    def _maybe_roll(self):
        if self.S == 0:
            return
        if self.T + self.B == 0:
            self._release()
            vb, self.vest_bal = self.vest_bal, 0          # the wiped epoch's lenders get the unreleased vest
            if vb:
                self._credit(vb, realised=True)
            self._roll(0, 1)
        elif self.S + VS > R_MAX * (self.T + self.B + VA):
            den = REBASE
            while self.S // den + VS > R_MAX * (self.T + self.B + VA):
                den *= REBASE
            self._roll(1, den)

    def _roll(self, num, den):
        # v0.4.2 (KI-65): conv_rem starts at 0; the lenders' conversions through this record sum
        # to S * num // den exactly (no orphan shares)
        self.epochs[self.epoch] = [self.acc, num, den, self.acc_r, self.S, 0, 0, 0]
        self.epoch += 1
        self.S = self.S * num // den
        if self.lock_calendar:
            self.lock_cal = {b: v * num // den for b, v in self.lock_cal.items() if v * num // den}
            self.lock_norm = sum(self.lock_cal.values())
        else:
            self.lock_norm = self.lock_norm * num // den
        self.dust += self.carry + self.carry_r
        self.carry = self.carry_r = 0
        self.acc = self.acc_r = 0
        self.wipe_live = self.S == 0
        if self.wipe_live and self.wipe_sweep_gate:
            self.wipe_swept = False             # R4A-7: a deposit must wait for the post-wipe sweep

    # ---- C0-9 pro-rata withdrawal locks ----------------------------------------------
    def _resolve(self, q: int):
        """Locks decay only when B falls below its running minimum since the first lock of
        the PI epoch (V2-3); all clear when B reaches 0."""
        self._cal_advance()
        if q >= self.B:
            self.pi_epoch += 1
            self.pi = WAD
            self.lock_norm = 0
            self.lock_cal = {}
            self.b_min = 0
            return
        bn = self.B - q
        if self.pi_decay and self.lock_norm > 0 and bn < self.b_min:
            self.pi = cdiv(self.pi * bn, self.b_min)
            self.b_min = bn

    def _lock_of(self, L: Lender) -> int:
        if L.lock_epoch != self.pi_epoch or L.lock == 0:
            return 0
        if self.lock_expiry and self.now >= L.lock_until:
            return 0
        return min(cdiv(L.lock * self.pi, L.lock_pi), L.shares)

    def locked_total(self) -> int:
        self._cal_advance()
        x = self.lock_norm * self.pi // WAD if self.lock_floor else cdiv(self.lock_norm * self.pi, WAD)
        return min(x, self.S)                                      # floor (v0.4.1)

    def allowance_view(self, who) -> int:
        """free_allowance(who) exactly as the program computes it when ONLY `who` is touched
        (every other lender left as it is): the state I-P4 must hold on (R4A-2 / R4B-3)."""
        if who not in self.lenders:
            return 0
        saved = (self.lenders[who], self.unmatched, self.n_touch, self.lock_norm, dict(self.lock_cal))
        rems = {e: self.epochs[e][7] for e in range(saved[0].epoch, self.epoch)}   # its touch may convert (KI-65)
        self.lenders[who] = copy.copy(saved[0])
        try:
            return self.free_allowance(who)
        finally:
            self.lenders[who], self.unmatched, self.n_touch, self.lock_norm, self.lock_cal = saved
            for e, r in rems.items():
                self.epochs[e][7] = r

    def free_allowance(self, who) -> int:
        L = self._touch(who)
        if L.epoch != self.epoch:
            return 0
        if not self.prorata:
            return min(L.shares * (self.T + self.B + VA) // (self.S + VS), self.T)
        u = L.shares - self._lock_of(L)
        su = max(self.S - self.locked_total(), u)
        if su == 0:
            return 0
        return min(u * self.T // su, L.shares * (self.T + self.B + VA) // (self.S + VS), self.T)

    # ---- lender actions -------------------------------------------------------
    def deposit(self, who, amount: int) -> int:
        self._release()
        L = self._touch(who)
        minted = amount * (self.S + VS) // (self.T + self.B + VA)
        if minted == 0:
            raise ValueError("deposit too small")
        if self.S + minted > U128:
            raise ValueError("share overflow")
        if self.S == 0 and self.wipe_live:
            if not self.wipe_swept:
                raise ValueError("WipeSweepPending: sweep_trader / claim_venue_rewards first")   # R4A-7
            self.wipe_live = False             # V2-12: the full-wipe record is final from now on
            L = self._touch(who)
        L.shares += minted
        self.S += minted
        self.T += amount
        self.vault_token_balance += amount
        return minted

    def cover(self, who, lamports: int):
        """Lender pays SOL to bring a negative pending balance back to >= 0 (C0-13)."""
        L = self._touch(who)
        self.sol_vault += lamports
        self.inflow += lamports
        L.pending += lamports
        L.real += lamports

    def withdraw(self, who, shares: int) -> int:
        self._release()
        L = self._touch(who)
        if L.pending < 0:
            raise ValueError("negative SOL balance: cover it first")      # C0-13
        if L.epoch != self.epoch or shares <= 0 or shares > L.shares:
            raise ValueError("not enough shares")
        amount = shares * (self.T + self.B + VA) // (self.S + VS)
        if amount > self.T:
            raise ValueError("insufficient unborrowed inventory")
        if self.prorata:
            allow = self.free_allowance(who)
            if amount > allow:
                raise ValueError("above pro-rata share of unborrowed inventory")
            cur = self._lock_of(L)
            u = L.shares - cur
            burn_max = cdiv(allow * (self.S + VS), self.T + self.B + VA) if allow else 0
            add = 0
            if self.B > 0 and burn_max > 0 and u > burn_max:
                add = cdiv(min(shares, burn_max) * (u - burn_max), burn_max)
            new_lock = min(cur + add, L.shares - shares)
            if new_lock and self.lock_norm == 0:
                self.b_min = self.B
            if L.lock_epoch == self.pi_epoch and (cur or not self.lock_expiry or not self.lock_calendar):
                self._cal_sub(L.lock_until, L.lock_term)      # a live lock leaves its bucket
            # v0.4.1: floor, so the aggregate locked <= sum of the lenders' own (ceil) locks and the
            # allowances of the unlocked shares never sum above T (the v0.4 ceil broke I-P4 when every
            # lender was locked down to rounding dust: two 1-share lenders could each draw T)
            term = (new_lock * WAD // self.pi if self.lock_floor else cdiv(new_lock * WAD, self.pi)) if new_lock else 0
            until = max(L.lock_until if cur else 0, self.lock_until_for()) if add else L.lock_until
            self._cal_add(until, term)
            L.lock, L.lock_pi, L.lock_epoch, L.lock_term, L.lock_until = new_lock, self.pi, self.pi_epoch, term, until
        flush = shares == self.S or (self.exit_flush_bps and (self.S - shares) * BPS < self.exit_flush_bps * self.S)
        if flush and self.vest_bal:     # the last lender is paid the unreleased vest first
            vb, self.vest_bal = self.vest_bal, 0
            self._credit(vb, realised=True)
            L = self._touch(who)
        L.shares -= shares
        L.withdrawn += amount
        self.S -= shares
        self.T -= amount
        self.vault_token_balance -= amount
        return amount

    def claim_sol(self, who) -> int:
        """Pays min(pending, own realised allocation + pot draw up to the own deficit,
        sol_vault).  Memoryful: a second call in the same state pays 0 (V2-2)."""
        L = self._touch(who)
        own = L.real - L.claimed
        need = max(0, L.pending - own)
        draw = min(self.unmatched, need)
        if draw:
            self.unmatched -= draw
            L.real += draw
        pay = min(L.pending, L.real - L.claimed, self.sol_vault)
        if pay <= 0:
            return 0
        L.pending -= pay
        L.claimed += pay
        self.sol_vault -= pay
        return pay

    def sweep_sol(self, lamports: int):
        """Trader sweep / venue cashback: a lump (vests).  While a wipe record is live (S == 0) it
        tops up the record and marks the post-wipe sweep done (R4A-7: deposits wait for it)."""
        self.sol_vault += lamports
        self.inflow += lamports
        if self.S == 0 and self.wipe_live:
            self.wipe_swept = True
        self._vest(lamports)

    def donate_tokens(self, amount: int):
        self.vault_token_balance += amount

    # ---- borrower side ----------------------------------------------------------
    def borrow(self, pid, q: int, duration: int = 30 * 86400, tail: int = S3_TAIL_S):
        self._release()
        if q <= 0 or q > self.T or (self.B + q) * BPS > self.util_cap * (self.T + self.B):
            raise ValueError("utilization cap")
        self.T -= q
        self.B += q
        self.vault_token_balance -= q
        p = self.rent_protocol_bps
        self.positions[pid] = [q, self.index, p, self.now + duration]
        self.A += q * (BPS - p)
        self.max_release = max(self.max_release, self.now + duration + tail)   # R4A-8: deadline + own S3 tail

    def _credited_for(self, q, i_open, p):
        # ceil: the per-position true-up never gives lenders more than was paid
        return cdiv(q * (self.index - i_open) * (BPS - p), WAD * BPS) if self.mode == "continuous" else 0

    def _close_pos(self, pid):
        q, i_open, p, _dl = self.positions.pop(pid)
        self.A -= q * (BPS - p)
        self._resolve(q)
        credited = self._credited_for(q, i_open, p)
        self.receivable -= min(credited, self.receivable)
        return q, i_open, p, credited

    @staticmethod
    def _rent_top_up(principal: int, rent: int, credited: int) -> int:
        """M10-8C-1: exit rent never credited while the position was open (R_due = R_min within
        min_rent_s of the open; a settlement's rent, which has no protocol share).  Credited at
        once it would go to the shares holding at the exit (a JIT deposit takes its part); it
        vests like a lump instead.  `principal` pays the rent first."""
        return max(min(principal, rent) - credited, 0)

    def _true_up(self, principal: int, rent: int, credited: int) -> int:
        """Continuous mode, repay/settle: `principal` reached sol_vault and `credited` of it was
        credited while the position was open.  The immediate part is trued up against `credited`
        (a shortfall is a debit) and realised; returns the top-up, which the caller vests."""
        top = self._rent_top_up(principal, rent, credited)
        self._credit(principal - top - credited)
        self._credit_r(principal - top)
        return top

    def repay(self, pid, rent_paid: int, lender_lump: int):
        """Close / liquidation / expiry: Q returns; rent_paid (total, before the split with
        the position's own protocol share) and lender_lump (penalty share / expiry fee)."""
        self._release()
        q, i_open, p, credited = self._close_pos(pid)
        self.T += q
        self.B -= q
        self.vault_token_balance += q
        lender_rent = rent_paid - rent_paid * p // BPS
        self.sol_vault += lender_rent + lender_lump
        self.inflow += lender_rent + lender_lump
        if self.mode == "continuous":
            top = self._true_up(lender_rent, lender_rent, credited)
            self._vest(lender_lump + top)               # M10-8C-1: the rent top-up vests with the lump
        else:
            self._vest(lender_rent + lender_lump)       # (b): all lender SOL released linearly

    def settle(self, pid, sol_in: int, lump: int = 0, rent: int | None = None):
        """SOL settlement: token debt extinguished; sol_in lamports to lenders, of which
        `lump` (the penalty part of an S2/S3 take) vests and the rest is immediate, but for
        the rent top-up (M10-8C-1): the principal pays `rent` first (R_due, all of it the
        lenders' in a settlement; default the position's accrued rent ceil(q*dI/WAD), R_due
        past min_rent_s), and its part above `credited` vests with the lump."""
        self._release()
        if rent is None:
            q, i_open, _p, _dl = self.positions[pid]
            rent = cdiv(q * (self.index - i_open), WAD)
        q, i_open, p, credited = self._close_pos(pid)
        self.B -= q
        lump = min(lump, sol_in)
        principal = sol_in - lump
        self.sol_vault += sol_in
        self.inflow += sol_in
        if self.mode == "continuous":
            lump += self._true_up(principal, rent, credited)
        else:
            self._credit(principal, realised=True)
        self._vest(lump)
        self._maybe_roll()

    # ---- invariants -----------------------------------------------------------------
    def check(self):
        if self.prorata:
            # I-P4 on the state the program sees: each lender's allowance with only that lender touched
            self._cal_advance()
            seen = sum(self.allowance_view(w) for w in list(self.lenders))
            assert seen <= self.T + len(self.lenders), ("I-P4 (untouched state): allowances exceed T", seen, self.T)
        for who in list(self.lenders):
            self._touch(who)
        Ls = self.lenders.values()
        cur = [L for L in Ls if L.epoch == self.epoch]
        tok_claims = sum(L.shares * (self.T + self.B + VA) // (self.S + VS) for L in cur)
        assert tok_claims <= self.T + self.B, ("token claims exceed assets", tok_claims, self.T + self.B)
        # I-P1, v0.4.2 (KI-65): every lender is touched above, so every share of S has a holder
        held = sum(L.shares for L in cur)
        assert held == self.S if self.cum_convert else held <= self.S, ("shares", held, self.S)
        # I-P2 (claim side, strict per lender): unclaimed realised allocations + pot + unreleased
        # vest are backed by the vault; nobody was paid more than its own allocation.
        unclaimed_real = sum(L.real - L.claimed for L in Ls)
        assert unclaimed_real + self.unmatched + self.vest_bal <= self.sol_vault, (
            "realised allocations exceed the vault", unclaimed_real, self.unmatched, self.vest_bal, self.sol_vault)
        assert all(L.claimed <= L.real for L in Ls), "claimed above realised allocation"
        assert all(L.real - L.claimed <= max(L.pending, 0) for L in Ls), "own allocation above entitlement"
        assert sum(L.claimed for L in Ls) + self.protocol <= self.inflow
        # entitlement side, signed aggregate identity
        pos = sum(max(L.pending, 0) for L in Ls)
        neg = sum(max(-L.pending, 0) for L in Ls)
        assert pos - neg <= self.sol_vault - self.vest_bal + self.receivable, (
            "SOL claims exceed assets (aggregate)", pos, neg, self.sol_vault, self.receivable)
        self.max_neg = max(getattr(self, "max_neg", 0), neg)
        # R3B-9: no phantom receivable once nothing is borrowed
        if self.B == 0:
            assert self.receivable == 0, ("receivable without borrowed inventory", self.receivable)
            assert self.A == 0
        assert self.S + VS <= R_MAX * (self.T + self.B + VA) or self.S == 0 or self.T + self.B == 0, (
            "share ratio", self.S, self.T, self.B)
        assert self.S <= U128
        assert self.vault_token_balance >= self.T
        assert self.B == sum(p[0] for p in self.positions.values())
        if self.prorata:
            assert sum(self.free_allowance(w) for w in self.lenders) <= self.T + len(self.lenders)
            # R3B-1: no lock outlives lock_until
            if self.lock_expiry:
                for L in Ls:
                    if self._lock_of(L):
                        assert self.now < L.lock_until

    def wind_down_liveness(self) -> tuple:
        """R3A-2 liveness invariant on a copy: resolve every position (full rent), release the
        vest, sync every lender, claim in sequence.  Returns (paid, lender_sol_before, stranded,
        dust_bound); asserts stranded <= dust_bound."""
        p = copy.deepcopy(self)
        for k in list(p.positions):
            q, io, pr, dl = p.positions[k]
            owed = cdiv(q * (p.index - io), WAD)
            p.repay(k, owed, 0)
        p.advance(p.lump_vest_s + 1)
        for w in list(p.lenders):
            p._touch(w)
        before = p.sol_vault
        paid = 0
        for _ in range(2):
            for w in list(p.lenders):
                paid += p.claim_sol(w)
        stranded = p.sol_vault
        bound = p.n_touch + 2 * len(p.lenders) + p.epoch + 2
        # a lender with an uncovered negative balance forfeits nothing to others; the vault is
        # either drained to dust or every positive entitlement is fully paid.
        unpaid = sum(max(L.pending, 0) for L in p.lenders.values())
        assert stranded <= bound or unpaid == 0, ("stranded lender SOL", stranded, bound)
        return paid, before, stranded, bound


# ------------------------------------------------------------------------------------
def token_value(p: "Pool", L: "Lender") -> int:
    return L.shares * (p.T + p.B + VA) // (p.S + VS) if L.epoch == p.epoch else 0


def fuzz(ops: int, seed: int, mode: str = "continuous", util_cap_bps: int = 9_000, liveness_every: int = 2000):
    rng = random.Random(seed)
    p = Pool(mode, util_cap_bps=util_cap_bps)
    users = [f"u{i}" for i in range(6)]
    pid = 0
    counts = {}
    worst_strand = 0
    for n in range(ops):
        op = rng.choice(["dep", "dep", "wd", "wd", "borrow", "accrue", "repay", "settle", "claim", "donate",
                         "cover", "wipe", "claimN", "sweep", "churn", "adv", "gov", "wdall"])
        try:
            if op == "dep":
                if p.wipe_live and not p.wipe_swept and rng.random() < 0.7:
                    p.sweep_sol(rng.choice([0, rng.randrange(1, 10**9)]))   # the depositor bundles the post-wipe sweep
                p.deposit(rng.choice(users), rng.choice([1, 7, 10**6, 10**9, 10**12, rng.randrange(1, 10**13)]))
            elif op == "wd":
                u = rng.choice(users)
                L = p.lenders.get(u)
                if L and L.shares:
                    before = (L.shares, L.pending)
                    allow = p.free_allowance(u)
                    s = rng.randrange(1, L.shares + 1) if rng.random() < 0.5 else \
                        max(1, allow * (p.S + VS) // (p.T + p.B + VA))
                    got = p.withdraw(u, min(s, L.shares))
                    assert before[1] >= 0, "withdrew with negative pending"
                    assert got <= allow, ("withdrew above pro-rata allowance", got, allow)
            elif op == "wdall":                  # a lender takes everything it may (R3A-2/R3A-10 paths)
                u = rng.choice(users)
                L = p.lenders.get(u)
                if L and L.shares and p.B == 0:
                    p.withdraw(u, L.shares)
            elif op == "borrow" and p.T > 0:
                pid += 1
                p.borrow(pid, rng.randrange(1, p.T + 1), rng.choice([3600, 86400, 30 * 86400]))
            elif op == "accrue":
                p.advance(rng.randrange(0, 600), rng.randrange(0, 10**15))
            elif op == "adv":
                p.advance(rng.choice([60, 3600, 6 * 3600, 2 * 86400, 31 * 86400]))
            elif op == "gov":                    # governance changes rent_protocol_bps (new positions only)
                p.rent_protocol_bps = rng.choice([0, 1000, 1500, 3000])
            elif op == "repay" and p.positions:
                k = rng.choice(list(p.positions))
                q, io, pr, dl = p.positions[k]
                owed = cdiv(q * (p.index - io), WAD)
                paid = owed if rng.random() < 0.8 else rng.randrange(0, owed + 1)
                p.repay(k, paid, rng.randrange(0, 10**9))
            elif op == "settle" and p.positions:
                k = rng.choice(list(p.positions))
                sol_in = rng.randrange(0, 10**12)
                p.settle(k, sol_in, rng.randrange(0, sol_in + 1) if rng.random() < 0.3 else 0)
            elif op == "wipe" and p.T > 0 and p.S > 0:
                pid += 1
                q = p.T * p.util_cap // BPS
                if q > 0 and (p.B + q) * BPS <= p.util_cap * (p.T + p.B):
                    p.borrow(pid, q)
                for k in list(p.positions):
                    p.settle(k, rng.randrange(0, 10**10))
            elif op == "claim":
                p.claim_sol(rng.choice(users))
            elif op == "claimN":
                u = rng.choice(users)
                first = p.claim_sol(u)
                again = sum(p.claim_sol(u) for _ in range(rng.randrange(1, 20)))
                assert again == 0, ("repeated claim paid more", first, again)
            elif op == "sweep":
                p.sweep_sol(rng.randrange(1, 10**9))
            elif op == "churn" and p.T > 0 and p.B > 0:
                locks = {w: p._lock_of(p._touch(w)) for w in p.lenders}
                pid += 1
                c = rng.randrange(1, p.T + 1)
                p.borrow(pid, c, 3600)
                p.repay(pid, 0, 0)
                for w, lk in locks.items():
                    assert p._lock_of(p._touch(w)) >= lk, ("churn released a lock", w, lk)
            elif op == "cover":
                u = rng.choice(users)
                L = p.lenders.get(u)
                if L:
                    p._touch(u)
                    if L.pending < 0:
                        p.cover(u, -L.pending)
            elif op == "donate":
                p.donate_tokens(rng.randrange(1, 10**12))
            counts[op] = counts.get(op, 0) + 1
        except ValueError:
            counts[op + "!"] = counts.get(op + "!", 0) + 1
        p.check()
        if liveness_every and n % liveness_every == liveness_every - 1:
            paid, before, stranded, bound = p.wind_down_liveness()
            worst_strand = max(worst_strand, stranded)
    paid, before, stranded, bound = p.wind_down_liveness()
    worst_strand = max(worst_strand, stranded)
    return counts, p.epoch, getattr(p, "max_neg", 0), worst_strand


def legacy_pool_module():
    """The frozen v0.3 pool model (comparison rows only)."""
    here = os.path.dirname(os.path.abspath(__file__))
    spec = importlib.util.spec_from_file_location("pool_model_v03", os.path.join(here, "legacy", "v0_3", "pool_model.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def stranded_table(seeds: int = 40, ops: int = 3000):
    """R3A-2 (reviewer's r3_stranded.py): random histories without shortfalls or settlements,
    then an honest wind-down (full rent, every lender synced, repeated claims)."""
    v03 = legacy_pool_module()
    out = {}
    for label, build in (("v0.3 (surplus recycled into acc_r)", lambda: v03.Pool("continuous", util_cap_bps=9000)),
                         ("v0.4 (unmatched pot)", lambda: Pool("continuous", util_cap_bps=9000))):
        tot_in = tot_str = worst = 0
        above = 0
        for seed in range(seeds):
            rng = random.Random(seed)
            p = build()
            users = [f"u{i}" for i in range(6)]
            pid = 0
            for _ in range(ops):
                op = rng.choice(["dep", "dep", "wd", "wd", "borrow", "accrue", "repay", "claim", "sweep"])
                try:
                    if op == "dep":
                        p.deposit(rng.choice(users), rng.choice([10**6, 10**9, 10**12, rng.randrange(1, 10**13)]))
                    elif op == "wd":
                        u = rng.choice(users)
                        L = p.lenders.get(u)
                        if L and L.shares:
                            allow = p.free_allowance(u)
                            s = max(1, allow * (p.S + VS) // (p.T + p.B + VA)) if rng.random() < 0.7 else L.shares
                            p.withdraw(u, min(s, L.shares))
                    elif op == "borrow" and p.T > 0:
                        pid += 1
                        p.borrow(pid, rng.randrange(1, p.T + 1))
                    elif op == "accrue":
                        p.accrue(rng.randrange(0, 10**15))
                    elif op == "repay" and p.positions:
                        k = rng.choice(list(p.positions))
                        q, io = p.positions[k][:2]
                        p.repay(k, cdiv(q * (p.index - io), WAD), 0)
                    elif op == "claim":
                        p.claim_sol(rng.choice(users))
                    elif op == "sweep":
                        p.sweep_sol(rng.randrange(1, 10**9))
                except ValueError:
                    pass
            inflow = p.inflow
            for k in list(p.positions):
                q, io = p.positions[k][:2]
                p.repay(k, cdiv(q * (p.index - io), WAD), 0)
            if hasattr(p, "advance"):
                p.advance(p.lump_vest_s + 1)
            for _ in range(6):
                for w in list(p.lenders):
                    p._touch(w)
                for w in list(p.lenders):
                    p.claim_sol(w)
            stranded = p.sol_vault
            tot_in += inflow
            tot_str += stranded
            worst = max(worst, stranded)
            above += stranded > 10**7
        out[label] = (tot_in, tot_str, worst, above)
    return out


def jit_demo():
    """Attacker deposits 9x the pool right before a 30-day position closes (rent only)."""
    out = []
    for mode in ("continuous", "release"):
        p = Pool(mode)
        p.deposit("honest", 1_000 * 10**6)
        p.borrow(1, 900 * 10**6)
        d_index = WAD * 5_000 * 30 * 86400 // (BPS * 31_536_000)
        for _ in range(30):
            p.advance(86400, d_index // 30)
        p.deposit("attacker", 9_000 * 10**6)
        q, io = p.positions[1][:2]
        rent = cdiv(q * (p.index - io), WAD)
        p.repay(1, rent, 0)
        got_tokens = p.withdraw("attacker", p.lenders["attacker"].shares)
        got_sol = p.claim_sol("attacker")
        p.advance(p.lump_vest_s + 1)
        honest_sol = p.claim_sol("honest")
        out.append((mode, rent, got_sol, honest_sol, got_tokens))
    return out


def jit_hold_table():
    """R3B-2 and the (a)/(b) comparison: a whale deposits 9x the pool just before the last
    open position expires (rent 2.4165 SOL, expiry fee 1.2159 SOL: ECON §12.2 Tier A) and
    withdraws after h hours.  Share of the lender income it captures."""
    rows = []
    rent, fee = 2_416_500_000, 1_215_900_000
    lender_rent = rent - rent * RENT_PROTOCOL_BPS // BPS
    for mode, label in (("continuous", "(a) v0.4: continuous rent, lumps vest 6 h"),
                        ("release", "(b) owner option: all lender SOL released over 6 h")):
        for h in (0, 1, 3, 6, 24):
            p = Pool(mode)
            p.deposit("H", 10**12)
            p.borrow("pos", 5 * 10**11)
            p.advance(30 * 86400, rent * WAD // (5 * 10**11))
            p.deposit("W", 9 * 10**12)
            p.repay("pos", rent, fee)
            p.advance(h * 3600)
            p.withdraw("W", p.lenders["W"].shares)
            w = p.claim_sol("W")
            p.advance(p.lump_vest_s + 1)
            w += p.claim_sol("W")
            hs = p.claim_sol("H")
            rows.append((label, h, w, hs, lender_rent + fee))
    return rows


def lump_legacy_row():
    v03 = legacy_pool_module()
    rent, fee = 2_416_500_000, 1_215_900_000
    p = v03.Pool("continuous")
    p.deposit("H", 10**12)
    p.borrow("pos", 5 * 10**11)
    p.accrue(rent * WAD // (5 * 10**11))
    p.deposit("W", 9 * 10**12)
    p.repay("pos", rent, fee)
    p.withdraw("W", p.lenders["W"].shares)
    w = p.claim_sol("W")
    hs = p.claim_sol("H")
    return w, hs, rent - rent * RENT_PROTOCOL_BPS // BPS + fee


def stuck_lock_table():
    """R3B-1 (reviewer's stuck_lock.py) and the PI decision.  A and B deposit 500 each; 900 is
    borrowed (90 %); A withdraws its free part once.  Then (i) the market grows and stays busy
    (C deposits 2,000, a further 1,000 is borrowed, the original position is replaced every day),
    or (ii) interest fades (positions of 300, 300 and 200 close at 6 h, 12 h and 24 h; 100 stays
    out).  A's allowance as a share of its SPEC §2.4 pro-rata part of unborrowed inventory
    (shares x T / S, what B -- who never withdrew -- gets) at 24 h, 7 d, 30 d and 32 d."""
    v03 = legacy_pool_module()
    U = 1_000_000_000
    checkpoints = (24, 7 * 24, 30 * 24, 32 * 24)
    rows = []

    def frac(p):
        L = p._touch("A")
        fair = L.shares * p.T // (p.S + VS)
        return p.free_allowance("A") / fair if fair else 1.0

    for scen in ("growing, always busy", "interest fades (100 of 900 stays out)"):
        for label, build in (("v0.3 (PI / running minimum only)", lambda: v03.Pool("continuous", util_cap_bps=9000)),
                             ("v0.4 (PI + lock_until)", lambda: Pool("continuous", util_cap_bps=9000)),
                             ("v0.4 without PI (lock_until only)", lambda: Pool("continuous", util_cap_bps=9000,
                                                                               pi_decay=False))):
            p = build()
            adv = hasattr(p, "advance")
            dur = [30 * 86400] if adv else []
            p.deposit("A", 500 * U)
            p.deposit("B", 500 * U)
            if scen.startswith("growing"):
                p.borrow("p0", 900 * U, *dur)
            else:
                for pid, q in (("p0a", 300), ("p0b", 300), ("p0c", 200), ("p0d", 100)):
                    p.borrow(pid, q * U, *dur)
            s = p._touch("A").shares * p.free_allowance("A") // (500 * U) - 1
            p.withdraw("A", s)
            if scen.startswith("growing"):
                p.deposit("C", 2000 * U)
                p.borrow("p1", 1000 * U, *dur)
            cells = []
            k = 0
            for step in range(1, checkpoints[-1] + 1):
                if adv:
                    p.advance(3600)
                if scen.startswith("growing") and step % 24 == 0:
                    old = "p0" if k == 0 else f"r{k - 1}"
                    p.repay(old, 0, 0)
                    p.borrow(f"r{k}", 900 * U, *dur)
                    k += 1
                if not scen.startswith("growing"):
                    for hh, pid in ((6, "p0a"), (12, "p0b"), (24, "p0c")):
                        if step == hh:
                            p.repay(pid, 0, 0)
                if step in checkpoints:
                    cells.append(f"{100 * frac(p):.0f}%")
            rows.append((scen, label, cells))
    return rows


def inflation_demo():
    p = Pool()
    p.deposit("attacker", 1)
    p.donate_tokens(10**12)
    minted = p.deposit("victim", 10**9)
    back = p.withdraw("victim", minted)
    return minted, back


def run_demo():
    out = []
    for prorata in (False, True):
        p = Pool(prorata=prorata)
        p.deposit("A", 10**12)
        p.deposit("B", 10**12)
        p.borrow(1, 18 * 10**11)
        took = 0
        for _ in range(50):
            try:
                s = max(1, p.free_allowance("A") * (p.S + VS) // (p.T + p.B + VA))
                if s > p.lenders["A"].shares:
                    s = p.lenders["A"].shares
                took += p.withdraw("A", s)
            except ValueError:
                break
        p.settle(1, 9 * 10**11)
        vals = {}
        for who in ("A", "B"):
            L = p._touch(who)
            vals[who] = (took if who == "A" else 0) + token_value(p, L) + L.pending
        out.append((prorata, took, vals["A"], vals["B"]))
    return out


def claim_loop_demo():
    out = []
    for label, v2rule in (("v0.2 (memoryless)", True), ("v0.4 (own allocation + pot)", False)):
        for n in (1, 2, 5, 20, 100):
            p = Pool("continuous")
            p.deposit("A", 10**12)
            p.deposit("B", 10**12)
            p.borrow("pos1", 9 * 10**11)
            p.borrow("pos2", 9 * 10**11)
            p.accrue(200 * WAD // 1000)
            q, io = p.positions["pos1"][:2]
            p.repay("pos1", cdiv(q * (p.index - io), WAD), 0)
            fair = p._touch("A").pending * p.sol_vault // (p.sol_vault + p.receivable)
            got = 0
            for _ in range(n):
                if v2rule:
                    L = p._touch("A")
                    den = p.sol_vault + p.receivable
                    pay = min(L.pending * p.sol_vault // den, p.sol_vault, L.pending) if L.pending > 0 else 0
                    L.pending -= pay
                    p.sol_vault -= pay
                    got += pay
                else:
                    got += p.claim_sol("A")
            out.append((label, n, fair, got, p.sol_vault))
    return out


def run_util_table():
    rows = []
    for util in (90, 80, 60, 50, 30):
        cells = [f"{util}%"]
        for churn in (0, 30):
            p = Pool(prorata=True)
            p.deposit("A", 10**12)
            p.deposit("B", 10**12)
            big = util * 2 * 10**12 // 100
            p.borrow("big", big)
            took = 0

            def drain():
                nonlocal took
                for _ in range(50):
                    try:
                        a = p.free_allowance("A")
                        if a == 0:
                            break
                        s = min(max(1, a * (p.S + VS) // (p.T + p.B + VA)), p.lenders["A"].shares)
                        took += p.withdraw("A", s)
                    except ValueError:
                        break
            drain()
            for r in range(churn):
                c = min(p.T, 9 * (p.T + p.B) // 10 - p.B)
                if c <= 0:
                    break
                p.borrow(f"c{r}", c, 3600)
                p.repay(f"c{r}", 0, 0)
                drain()
            p.settle("big", big // 2)
            vals = {}
            for w in ("A", "B"):
                L = p._touch(w)
                vals[w] = (took if w == "A" else 0) + token_value(p, L) + L.pending
            fair = (vals["A"] + vals["B"]) / 2
            cells.append(f"{100 * (vals['A'] - fair) / fair:+.1f}%")
        rows.append(cells)
    return rows


def wipe_demo():
    lines = []
    for label, cls_roll in (("v0.1 (no epochs)", False), ("v0.4 (share epochs)", True)):
        p = Pool(util_cap_bps=10_000)
        if not cls_roll:
            p._maybe_roll = lambda: None
        p.deposit("old", 10**14)
        p.borrow(1, 10**14)
        p.settle(1, 50 * 10**9)
        p.sweep_sol(0)                             # post-wipe sweep (nothing to sweep), R4A-7
        minted = p.deposit("new", 10**14)
        p.borrow(2, 5 * 10**13)
        p.accrue(WAD // 10**6)
        p.repay(2, cdiv(5 * 10**13 * (WAD // 10**6), WAD), 10**12)
        p.advance(p.lump_vest_s + 1)
        new_pending = p._touch("new").pending
        p.borrow(3, p.T)
        p.settle(3, 10**9)
        m2 = 10**14 * (p.S + VS) // (p.T + p.B + VA)
        lines.append((label, minted, new_pending, m2, m2 > U128, p.epoch))
    return lines


def wipe_tail_demo():
    """V2-12 / R3B-2: SOL credited while S == 0 after a full wipe goes to the wiped epoch's
    lenders; unreleased vest at the wipe too; a dust depositor who arrives first and triggers a
    sweep gets it only over lump_vest_s (released pro-rata to whoever holds shares)."""
    p = Pool(util_cap_bps=10_000)
    p.deposit("old", 10**14)
    p.borrow(1, 10**14)
    p.repay(1, 0, 3 * 10**9)                  # 3 SOL lump vesting
    p.borrow(2, 10**14)
    p.settle(2, 50 * 10**9)                    # full wipe: unreleased vest -> the old record
    p.sweep_sol(10**9)                         # 1 SOL post-wipe credit -> old record (also the R4A-7 sweep)
    old = p._touch("old")
    got_old = old.pending - 50 * 10**9
    p.deposit("dust", 1)                       # dust depositor finalises the record ...
    p.sweep_sol(10**9)                         # ... then triggers another 1 SOL credit (vests)
    now_dust = p.claim_sol("dust")
    p.advance(60)
    p.deposit("late", 10**14)                  # an ordinary lender joins 1 minute later
    p.advance(p.lump_vest_s)
    dust_total = now_dust + p.claim_sol("dust")
    late = p.claim_sol("late")
    for i in range(5):
        p.deposit(f"x{i}", 10**14)
        p.borrow(10 + i, p.T)
        p.settle(10 + i, 10**9)
        p.sweep_sol(0)                         # post-wipe sweep before the next deposit (R4A-7)
        p.deposit(f"y{i}", 10**3)
    steps = p.epoch - old.epoch
    before = old.epoch
    p._touch("old")
    return got_old, now_dust, dust_total, late, steps, p.epoch - before, old.epoch == p.epoch


def zero_share_demo():
    """R3A-10: all lenders withdraw (no wipe), then a trader sweep arrives, then a dust deposit."""
    rows = []
    v03 = legacy_pool_module()
    for label, build in (("v0.3", lambda: v03.Pool()), ("v0.4", lambda: Pool())):
        p = build()
        p.deposit("A", 10**12)
        p.withdraw("A", p.lenders["A"].shares)
        p.sweep_sol(10**9)
        p.deposit("dust", 10**6)
        if hasattr(p, "advance"):
            p.advance(p.lump_vest_s + 1)
        d = p.claim_sol("dust")
        a = p.claim_sol("A")
        rows.append((label, d, a, getattr(p, "protocol", 0)))
    return rows


def protocol_change_demo():
    """R3B-9 / R3A-5: rent_protocol_bps rises 15 % -> 30 % halfway through a position's life."""
    rows = []
    v03 = legacy_pool_module()
    for label in ("v0.3 (global value at every accrual and at the true-up)", "v0.4 (snapshot per position)"):
        if label.startswith("v0.3"):
            p = v03.Pool()
            p.deposit("L", 10**12)
            p.borrow("pos", 5 * 10**11)
            v03.LENDER_RENT_BPS = 8_500
            p.accrue(10**15)
            v03.LENDER_RENT_BPS = 7_000                 # governance change (global)
            p.accrue(10**15)
            q, io = p.positions["pos"]
            rent = cdiv(q * (p.index - io), WAD)
            p.repay("pos", rent * 7_000 // 8_500 * 8_500 // 7_000, 0)   # the program pays at the new split
            v03.LENDER_RENT_BPS = 8_500
            rows.append((label, p.receivable, p._touch("L").pending, p.sol_vault))
        else:
            p = Pool()
            p.deposit("L", 10**12)
            p.borrow("pos", 5 * 10**11)
            p.accrue(10**15)
            p.rent_protocol_bps = 3_000                 # governance change: new positions only
            p.accrue(10**15)
            q, io, pr, dl = p.positions["pos"]
            rent = cdiv(q * (p.index - io), WAD)
            p.repay("pos", rent, 0)
            rows.append((label, p.receivable, p._touch("L").pending, p.sol_vault))
    return rows


def escape_demo():
    out = []
    for label, v2 in (("v0.1", False), ("v0.4", True)):
        p = Pool()
        if not v2:
            def claim_v1(who, p=p):
                L = p._touch(who)
                pay = min(max(L.pending, 0), p.sol_vault)
                L.pending -= pay
                p.sol_vault -= pay
                return pay
            p.claim_sol = claim_v1
            p.prorata = False
        p.deposit("A", 10**12)
        p.deposit("B", 10**12)
        p.borrow(1, 10**12)
        d = WAD * 5_000 * 29 * 86400 // (BPS * 31_536_000)
        p.accrue(d)
        p.borrow(2, 5 * 10**11)
        p.settle(2, 10**12)
        a = p.claim_sol("A")
        q, io = p.positions[1][:2]
        owed = cdiv(q * (p.index - io), WAD)
        p.repay(1, owed // 10, 0)
        LA = p._touch("A")
        try:
            if not v2:
                amount = LA.shares * (p.T + p.B + VA) // (p.S + VS)
                p.S -= LA.shares
                p.T -= amount
                LA.shares = 0
                escaped = True
            else:
                p.withdraw("A", LA.shares)
                escaped = True
        except ValueError:
            escaped = False
        b = p.claim_sol("B")
        LB = p._touch("B")
        out.append((label, a, LA.pending, escaped, b, LB.pending, p.sol_vault, p.receivable))
    return out


def lock_fuzz(ops: int, seed: int, sample_every: int = 25, **kw):
    """R4A-2 / R4B-3: the reviewers' lock-heavy op mix (max withdrawals, multi-day advances, rolling
    positions).  Counts sampled states in which the allowances AS THE PROGRAM SEES THEM (each lender
    touched alone) sum above T + dust; with the default model check() also asserts it every op."""
    rng = random.Random(seed)
    p = Pool("continuous", util_cap_bps=9000, **kw)
    users = [f"u{i}" for i in range(6)]
    pid = bad = checks = 0
    worst = 0.0
    for n in range(ops):
        op = rng.choice(["dep", "dep", "wd", "wd", "borrow", "adv", "repay", "claim"])
        try:
            if op == "dep":
                p.deposit(rng.choice(users), rng.choice([10**9, 10**12, rng.randrange(10**6, 10**13)]))
            elif op == "wd":
                u = rng.choice(users)
                L = p.lenders.get(u)
                if L and L.shares:
                    allow = p.free_allowance(u)
                    p.withdraw(u, min(max(1, allow * (p.S + VS) // (p.T + p.B + VA)), L.shares))
            elif op == "borrow" and p.T > 0:
                pid += 1
                p.borrow(pid, rng.randrange(1, p.T + 1), rng.choice([3600, 86400, 7 * 86400]))
            elif op == "adv":
                p.advance(rng.choice([600, 3600, 86400, 3 * 86400]), rng.randrange(0, 10**14))
            elif op == "repay" and p.positions:
                k = rng.choice(list(p.positions))
                q, io, pr, dl = p.positions[k]
                p.repay(k, cdiv(q * (p.index - io), WAD), 0)
            elif op == "claim":
                p.claim_sol(rng.choice(users))
        except ValueError:
            pass
        if n % sample_every == 0 and p.T > 0:
            checks += 1
            p._cal_advance()
            tot = sum(p.allowance_view(w) for w in list(p.lenders))
            if tot > p.T + len(p.lenders):
                bad += 1
                worst = max(worst, tot / p.T)
        if not kw:
            p.check()
    return checks, bad, worst


def stale_lock_demo():
    """R4A-2 / R4B-3 (reviewers' stale_lock.py): an idle lender's lock expired at lock_until but its
    holder is never touched; the others withdraw first."""
    U = 10**9
    D = 86_400
    rows = []
    for label, kw in (("v0.4 (expiry leaves the aggregate at the holder's touch)", dict(lock_calendar=False)),
                      ("v0.4.1 (day-bucket expiry calendar)", {})):
        p = Pool("continuous", util_cap_bps=9000, **kw)
        for w in "EFG":
            p.deposit(w, 10**12)
        p.borrow("pA", 75 * 10**10, 30 * D)
        p.borrow("pB", 75 * 10**10, 30 * D)
        a = p.free_allowance("E")
        L = p._touch("E")
        p.withdraw("E", min(L.shares, a * (p.S + VS) // (p.T + p.B + VA)))
        until = p._touch("E").lock_until
        p.advance(20 * D)
        p.borrow("pC", 75 * 10**10, 30 * D)
        p.repay("pA", 0, 0)
        p.advance(9 * D)
        p.borrow("pD", 75 * 10**10, 30 * D)
        p.repay("pB", 0, 0)
        p.advance(4 * D)                               # past E's lock_until; nobody touches E
        T0 = p.T
        got = {}
        for w in "FGE":
            a = p.free_allowance(w)
            Lw = p._touch(w)
            sh = min(Lw.shares, a * (p.S + VS) // (p.T + p.B + VA))
            got[w] = p.withdraw(w, sh) if sh > 0 else 0
        rows.append((label, until / D, T0, got["F"], got["G"], got["E"]))
    return rows


def lock_tail_demo():
    """R4A-8 / R4B-8: a position opened at t = 0 (deadline 30 d) is stuck in PumpMigrating and reaches
    S3 only at deadline + max_migration_pause_s + grad_grace_s + s3_wait_s; lock_grace_s at its
    compiled minimum (1 h) and the three timers at their compiled maxima (1 d each)."""
    D = 86_400
    tail = 3 * D
    out = []
    for label, tail_used, cal in (("v0.4 (lock_until from the deadline alone)", 0, False),
                                  ("v0.4.1 (deadline + own S3 tail, day buckets)", tail, True)):
        p = Pool("continuous", util_cap_bps=9000, lock_grace_s=3600, lock_calendar=cal)
        p.deposit("A", 10**12)
        p.deposit("B", 10**12)
        p.borrow("pos", 18 * 10**11, 30 * D, tail=tail_used)
        a = p.free_allowance("A")
        p.withdraw("A", min(p._touch("A").shares, a * (p.S + VS) // (p.T + p.B + VA)))
        out.append((label, p._touch("A").lock_until / D, (30 * D + tail) / D))
    return out


def wipe_gate_demo():
    """R4A-7 (reviewer's wipe_dust.py): the keeper that executes the wiping sol_settle lands
    [sol_settle(wipe) -> deposit(2 raw units) -> claim_venue_rewards (1 SOL of pre-wipe cashback)]."""
    rows = []
    for label, gate in (("v0.4 (deposit finalises the record at once)", False), ("v0.4.1 (deposit waits for the post-wipe sweep)", True)):
        p = Pool(util_cap_bps=10_000, wipe_sweep_gate=gate)
        p.deposit("old", 10**14)
        p.borrow(1, 10**14)
        p.settle(1, 50 * 10**9)                     # full wipe
        try:
            p.deposit("ATT", 2)
            first = "ATT deposits first"
        except ValueError:
            first = "deposit refused (WipeSweepPending)"
        p.sweep_sol(10**9)                          # claim_venue_rewards: 1 SOL
        if first.startswith("deposit refused"):
            p.deposit("ATT", 2)                     # after the sweep the dust deposit goes through
        p.advance(p.lump_vest_s + 1)
        att = p.claim_sol("ATT")
        old = p._touch("old").pending - 50 * 10**9
        rows.append((label, first, att, old))
    return rows


def vest_exit_demo():
    """R4B-4: (i) an honest lender H (all shares but dust) leaves 10 min after 3 SOL of lumps; (ii)
    the reviewer's suggested fix (flush the vest before a withdrawal that leaves < 1 % of S) lets a
    whale that deposits 199x just before a lump take 99.5 % of it atomically (REJECTED)."""
    rows = []
    for dust in (0, 1_000):
        p = Pool("continuous", util_cap_bps=9000)
        p.deposit("H", 10**12)
        if dust:
            p.deposit("D", dust)
        p.borrow("pos", 5 * 10**11)
        p.advance(3600)
        p.repay("pos", 0, 3 * 10**9)
        p.advance(600)
        p.withdraw("H", p._touch("H").shares)
        h = p.claim_sol("H")
        p.advance(p.lump_vest_s + 1)
        h += p.claim_sol("H")
        d = p.claim_sol("D") if dust else 0
        rows.append(("honest exit 10 min after the lumps", f"dust {dust}", h, d))
    for flush in (0, 100):
        p = Pool("continuous", util_cap_bps=9000, exit_flush_bps=flush)
        p.deposit("H", 10**12)
        p.borrow("pos", 5 * 10**11)
        p.advance(30 * 86400)
        p.deposit("W", 199 * 10**12)                # whale, just in time
        p.repay("pos", 0, 3 * 10**9)                # 3 SOL of lumps (expiry fee / penalty share)
        p.withdraw("W", p._touch("W").shares)
        w = p.claim_sol("W")
        p.advance(p.lump_vest_s + 1)
        w += p.claim_sol("W")
        h = p.claim_sol("H")
        rows.append(("JIT whale x199 around a 3 SOL lump", "flush below 1 % of S (rejected)" if flush else "v0.4.1 (no flush)", w, h))
    return rows


def restart_grief_table():
    """R4B-5 (reviewer's lender_r4.py part 3): a 10 SOL lump, then a 1-lamport sweep every k seconds."""
    rows = []
    for label, weighted in (("v0.4 (every lump restarts the window)", False), ("v0.4.1 (value-weighted end)", True)):
        for per in (3600, 600, 60):
            p = Pool("continuous", util_cap_bps=9000, vest_weighted=weighted)
            p.deposit("H", 10**12)
            p.sweep_sol(10 * 10**9)
            t = 0
            rel = {}
            while t < 24 * 3600:
                p.advance(per)
                t += per
                p.sweep_sol(1)
                if t % 3600 == 0:
                    rel[t // 3600] = 1 - p.vest_bal / (10 * 10**9)
            rows.append((label, per, rel[6], rel[24]))
    return rows


def premium_jit_table():
    """R4B-6: a whale bundles [deposit x9 -> sol_settle(S3) -> withdraw -> claim] around the last open
    position.  Market value of Q = 500 SOL at P_ema; B_FV = Q*(1 + premium) (impact + fees on EMA
    reserves); take x = B_FV*(1 + 2 %) (expiry-fee reading) + rent 0.  Immediate part: v0.4 = B_FV + R;
    v0.4.1 = min(x, Q*P_ema + R)."""
    rows = []
    Q = 5 * 10**11
    for prem in (350, 700, 2000):
        b_fv = Q * (BPS + prem) // BPS
        x = b_fv + cdiv(200 * b_fv, BPS)
        for label, immediate in (("v0.4", b_fv), ("v0.4.1", Q)):
            p = Pool("continuous", util_cap_bps=9000)
            p.deposit("H", 10**12)
            p.borrow("pos", Q)
            p.deposit("W", 9 * 10**12)
            p.settle("pos", x, x - immediate)
            a = p.free_allowance("W")
            Lw = p._touch("W")
            tok = p.withdraw("W", min(Lw.shares, a * (p.S + VS) // (p.T + p.B + VA)))
            sol_w = p.claim_sol("W")
            left = token_value(p, p._touch("W"))
            gain = tok + sol_w + left - 9 * 10**12
            rows.append((prem, label, x - Q, gain))
    return rows


# ---- v0.4.2 (KI-65): no rebase orphans -------------------------------------------------------
def exit_all(p, who, max_burns: int = 64) -> int:
    """`who` withdraws every share it holds in the largest burns it may: its allowance, or, while
    that is 0, the largest burn that pays 0 tokens (always allowed); at most `max_burns` burns (a
    pool whose share price is above a token per share while nothing is free cannot be left by
    0-token burns).  Returns the tokens."""
    got = 0
    for _ in range(max_burns):
        L = p._touch(who)
        if not L.shares or L.epoch != p.epoch:
            return got
        allow = p.free_allowance(who)
        zero = cdiv(p.S + VS, p.T + p.B + VA) - 1            # the largest burn that pays 0 tokens
        fit = allow * (p.S + VS) // (p.T + p.B + VA)
        got += p.withdraw(who, min(L.shares, max(fit, zero, 1)))
    return got


def dust_shares(p, who, amount: int, rem: int):
    """`who` deposits `amount` tokens, then burns (for 0 tokens) what puts its shares at `rem`
    modulo 1,000, below a multiple of the next rebase's denominator (a 0-token burn is allowed)."""
    p.deposit(who, amount)
    extra = (p._touch(who).shares - rem) % REBASE
    if extra:
        try:
            p.withdraw(who, extra)
        except ValueError:                               # the burn would pay a token above the allowance
            pass


def orphan_vest_demo(seeds: int = 200):
    """KI-65 (FUZZ-M10-24H-A): three lenders, a settlement that vests a 3 SOL lump and rebases the
    shares (1/1,000), then every lender exits during the vest -- each converts lazily at its own exit --
    and claims again after it.  v0.4.1 floors each lender alone: the orphan shares keep S above 0, the
    R3B-2 flush (a withdrawal of all of S) never fires and the vest is released to shares nobody holds.
    Returns rows (model, runs that rolled, runs with S > 0 after every exit, worst stranded, its seed)."""
    rows = []
    for label, cum in (("v0.4.1 (each lender floored alone)", False),
                       ("v0.4.2 (cumulative conversion)", True)):
        runs = left = 0
        worst = (0, None)
        for seed in range(seeds):
            rng = random.Random(seed)
            p = Pool(util_cap_bps=10_000, cum_convert=cum)
            p.deposit("u0", 1000)
            p.borrow(1, 500)
            p.settle(1, 10**6)                           # the share price is no longer 1,000 per token
            for i in (1, 2):
                p.deposit(f"u{i}", rng.randrange(100, 2000))
            p.borrow(2, p.T - 1)
            e0 = p.epoch
            p.settle(2, 10**9 + 3 * 10**9, 3 * 10**9)    # 3 SOL vest, then (mostly) a rebase
            rolled = p.epoch > e0
            for w in list(p.lenders):                     # everyone leaves while the vest runs
                exit_all(p, w)
                p.claim_sol(w)
            s_left = p.S
            p.advance(p.lump_vest_s + 1)
            for w in list(p.lenders):
                p.claim_sol(w)
            if not rolled:
                continue
            runs += 1
            left += s_left > 0
            if worst[1] is None or p.sol_vault > worst[0]:
                worst = (p.sol_vault, seed)
        rows.append((label, runs, left, worst[0], worst[1]))
    return rows


def _rebased_pair(cum: bool, h_rem: int, w_rem: int, w_amount: int = 9_000):
    """H (1 token) and W (`w_amount` tokens) hold `h_rem`, `w_rem` shares modulo 1,000; a settlement
    of everything but one raw token pays 1 SOL immediately and vests a 3 SOL lump, and the shares
    rebase by 1/1,000 (H's old shares are dust below one new share)."""
    p = Pool(util_cap_bps=10_000, cum_convert=cum)
    dust_shares(p, "H", 1, h_rem)
    dust_shares(p, "W", w_amount, w_rem)
    p.borrow(1, p.T - 1)
    p.settle(1, 10**9 + 3 * 10**9, 3 * 10**9)
    assert p.epoch == 1 and p.epochs[0][2] == REBASE, (p.epoch, p.epochs)
    return p


def whale_order_demo():
    """KI-65 against R4B-4: what a whale can do with the conversion order and the R3B-2 flush.
    W1: a 199x JIT deposit around a 3 SOL lump while H holds a share it has not converted yet
        (latent): S counts H's share, so the whale's exit never takes all of S and it realises 0.
    W2: the whale bundles [settlement (3 SOL vest + rebase) -> sync in either order -> exit] while H
        holds 600 old shares (dust below one new share).  The record's rounding unit goes to the
        second converter; nobody gets less than its floor (H's floor is 0 shares).  Since
        KI65-REV-SEC1 each lender converts only itself (the owner signs a conversion through a
        rebase record), so order H then W is the whale converting after H's own conversion (at
        any later time: no atomicity needed).
    W3: after the roll the other holder G leaves; H's unconverted share keeps S at 1; a whale
        deposits, a 3 SOL sweep vests, the whale leaves: no flush (S > the whale's shares), and H,
        converting later, is paid the vest.
    Returns rows (case, model, whale gets, H gets, stranded)."""
    rows = []
    lump = 3 * 10**9
    for label, cum in (("v0.4.1", False), ("v0.4.2", True)):
        # W1
        p = _rebased_pair(cum, 0, 0, w_amount=10**6)          # H holds 1,000 old shares: 1 new share
        exit_all(p, "W")
        p.claim_sol("W")
        p.deposit("J", 199 * 10**9)                          # the whale, just in time
        p.borrow(2, p.T // 2)
        p.repay(2, 0, lump)                                  # a 3 SOL lump vests
        exit_all(p, "J")
        j = p.claim_sol("J")
        p.advance(p.lump_vest_s + 1)
        h = p.claim_sol("H")
        rows.append(("W1 JIT x199 around a lump, H's share not converted", label, j, h, p.sol_vault))
        # W2, both orders
        for order in (("H", "W"), ("W", "H")):
            p = _rebased_pair(cum, 600, 600)
            for w in order:
                p._touch(w)                                  # sync_lender, by w's owner
            w_got = p.claim_sol("W")
            exit_all(p, "W")
            w_got += p.claim_sol("W")
            p.advance(p.lump_vest_s + 1)
            h = p.claim_sol("H")
            rows.append((f"W2 sync order {order[0]} then {order[1]}, then the whale exits", label, w_got, h,
                         p.sol_vault))
        # W3
        p = _rebased_pair(cum, 600, 600)
        exit_all(p, "W")                                     # G's part: the other holder leaves first
        p.claim_sol("W")
        s_latent = p.S
        p.deposit("J", 10**6)
        p.sweep_sol(lump)
        exit_all(p, "J")
        j = p.claim_sol("J")
        p.advance(p.lump_vest_s + 1)
        h = p.claim_sol("H")
        rows.append((f"W3 whale deposits while S = {s_latent} is H's unconverted share", label, j, h, p.sol_vault))
    return rows


def lazy_order_check(trials: int = 2_000, seed: int = 65):
    """KI-65 lemma, on the model: random lenders and holdings, one to three rebases (deep ones too),
    lazy conversions in random order with deposits, 0-token burns and partial exits in between, then
    every lender converts in a random order.  Asserts sum(shares) == S once all converted, and that
    every conversion through k records lands in [floor chain, floor chain + 1 per record].
    Returns (trials, rebases, conversions, worst orphans of the v0.4.1 floor on the same trials)."""
    rng = random.Random(seed)
    rebases = conversions = worst_v041 = 0
    for _t in range(trials):
        tseed = rng.randrange(2**32)
        held = {}
        for cum in (True, False):
            r = random.Random(tseed)
            p = Pool(util_cap_bps=10_000, cum_convert=cum)
            users = [f"u{i}" for i in range(r.randrange(2, 9))]
            for u in users:
                dust_shares(p, u, r.choice([1, r.randrange(1, 50), r.randrange(1, 10**6)]), r.randrange(REBASE))
            for _k in range(r.randrange(1, 4)):
                q = p.T - r.choice([1, 1, 2, 3])
                if q > 0:
                    p.borrow(f"b{_k}", q)                # settle all but a few raw units: a (deep) rebase
                    for k in list(p.positions):
                        p.settle(k, r.randrange(0, 10**9))
                for u in r.sample(users, r.randrange(0, len(users) + 1)):
                    L = p.lenders[u]
                    lo = hi = L.shares
                    for e in range(L.epoch, p.epoch):
                        num, den = p.epochs[e][1], p.epochs[e][2]
                        lo, hi = lo * num // den, (hi * num // den + 1) if num else 0
                    p._touch(u)
                    assert lo <= L.shares <= hi or L.epoch != p.epoch or not cum, (lo, L.shares, hi)
                    conversions += cum
                    if r.random() < 0.3:
                        dust_shares(p, u, r.randrange(1, 10**4), r.randrange(REBASE))
            for u in r.sample(users, len(users)):
                p._touch(u)
            held[cum] = (sum(L.shares for L in p.lenders.values() if L.epoch == p.epoch), p.S)
            p.check()
            rebases += cum * sum(1 for e in p.epochs.values() if e[1])
        assert held[True][0] == held[True][1], held
        worst_v041 = max(worst_v041, held[False][1] - held[False][0])
    return trials, rebases, conversions, worst_v041


# ---- KI-66: the pot-only touch -------------------------------------------------------------------
def _pot_pair():
    """lender_it's T-POT scenario (the_pot_connects_early_leavers_and_late_joiners) up to A's first
    claim: A and B lend 1,000 tokens each, 900 are borrowed for 10 days, A withdraws its allowance
    (early leaver), C deposits 2,000 (late joiner) before the rent is paid, so the paid rent is
    realised on C's shares beyond C's entitlement: C holds a surplus, A a deficit.  Then a
    settlement of everything but one raw token (1 SOL) rebases the shares (1/1,000) before anyone
    touched C."""
    p = Pool(util_cap_bps=10_000)
    p.deposit("A", 10**12)
    p.deposit("B", 10**12)
    p.borrow(1, 9 * 10**11)
    p.advance(10 * 86_400, 10**16)
    L = p._touch("A")
    p.withdraw("A", min(L.shares, p.free_allowance("A") * (p.S + VS) // (p.T + p.B + VA)))
    p.deposit("C", 2 * 10**12)
    q, io, _pr, _dl = p.positions[1]
    p.repay(1, cdiv(q * (p.index - io), WAD), 0)
    first = p.claim_sol("A")
    short = p.lenders["A"].pending
    p.borrow(2, p.T - 1)
    p.settle(2, 10**9)
    assert p.epoch == 1 and p.epochs[0][1] == 1, p.epochs     # a rebase, C not converted
    return p, first, short


def pot_touch_demo():
    """KI-66: a surplus holder behind a rebase record that never returns.  Since KI65-REV-SEC1 a third
    party's sync cannot convert C through the record, so before KI-66 C's surplus stayed on C and A's
    shortfall waited for C's owner; the pot-only touch moves it to the pot without converting C.
    The pot-only touch is idempotent, and C's own later conversion counts nothing twice: C, A and the
    vault end as when C's owner had converted first.
    Returns rows (case, pot after C's sync, A's next claim, A's pending left, C's shares after its own
    later sync, C's claim then, lender_sol left)."""
    rows = []
    for case in ("owner-signed conversions only (no sync of C)", "pot-only touch of C (anyone)",
                 "pot-only touch twice", "C's owner converts C"):
        p, first, short = _pot_pair()
        if case.startswith("pot-only"):
            p._touch_pot("C")
            if "twice" in case:
                once = (p.unmatched, p.lenders["C"].real, p.lenders["C"].pending, p.lenders["C"].snap)
                p._touch_pot("C")
                assert once == (p.unmatched, p.lenders["C"].real, p.lenders["C"].pending, p.lenders["C"].snap)
            assert p.lenders["C"].epoch == 0 and p.epochs[0][7] == 0       # C stays unconverted
        elif case.startswith("C's owner"):
            p._touch("C")
        pot = p.unmatched
        a = p.claim_sol("A")
        left = p.lenders["A"].pending
        L = p._touch("C")                                    # C's owner returns
        c = p.claim_sol("C")
        p.claim_sol("A")
        rows.append((case, pot, a, left, L.shares, c, p.sol_vault))
    base = rows[-1][4:]
    assert all(r[4:] == base for r in rows), rows              # same end, whoever synced C and when
    assert all(r[3] == 0 for r in rows[1:]), rows               # A paid in full without C returning
    return rows


def pot_liveness_table(seeds: int = 2_000):
    """KI-66, I-P6 with lenders that never return: random pools (deposits, late joiners, early
    leavers, positions across rebases, rent, lumps, claims, one or more rebases), then a random set of
    lenders is absent; the wind-down resolves every position, releases the vest, touches the present
    lenders and the absent ones the way a third party may, and the present lenders claim twice.
    Variants: KI65-REV-SEC1 alone (an absent lender behind a rebase record is left as it is), the
    pot-only touch (KI-66), and every owner returning (the absent lenders converted by their owners).
    Returns rows (variant, runs with an absent lender behind a rebase record, runs where a present
    lender is left unpaid above the dust bound, present entitlement left unpaid in total (lamports),
    worst run (lamports))."""
    variants = ("KI65-REV-SEC1 only (absent lenders behind a rebase untouched)",
                "pot-only touch of the absent lenders (KI-66)", "every absent owner returns and converts")
    out = {v: [0, 0, 0, 0] for v in variants}
    for seed in range(seeds):
        rng = random.Random(seed)
        p = Pool(util_cap_bps=10_000)
        users = [f"u{i}" for i in range(rng.randrange(3, 7))]
        for u in users[:2]:
            p.deposit(u, rng.randrange(10**9, 10**12))
        pid = 0
        for _ in range(rng.randrange(8, 30)):
            r = rng.random()
            try:
                if r < 0.2 and p.T > 1:
                    pid += 1
                    p.borrow(pid, rng.randrange(1, p.T * 9 // 10 + 1), rng.choice([86_400, 9 * 86_400, 30 * 86_400]))
                elif r < 0.4:
                    p.advance(rng.choice([3_600, 86_400, 9 * 86_400]), rng.randrange(0, 10**16))
                elif r < 0.5 and p.positions:
                    k = rng.choice(list(p.positions))
                    q, io, _pr, _dl = p.positions[k]
                    p.repay(k, cdiv(q * (p.index - io), WAD), rng.choice([0, rng.randrange(0, 10**9)]))
                elif r < 0.65:
                    p.deposit(rng.choice(users), rng.randrange(10**6, 10**12))   # late joiners
                elif r < 0.75:
                    u = rng.choice(users)                                       # early leavers
                    if u in p.lenders and p._touch(u).shares:
                        L = p.lenders[u]
                        p.withdraw(u, max(1, min(L.shares, p.free_allowance(u) * (p.S + VS) // (p.T + p.B + VA))))
                elif r < 0.85:
                    u = rng.choice(users)
                    if u in p.lenders:
                        p.claim_sol(u)
                elif p.T > 1 and p.S:
                    pid += 1                                                    # all but a raw unit: a rebase
                    p.borrow(pid, p.T - 1)
                    p.settle(pid, rng.randrange(0, 10**10), rng.choice([0, rng.randrange(0, 10**9)]))
            except ValueError:
                pass
        lenders = list(p.lenders)
        if len(lenders) < 2:
            continue
        absent = set(rng.sample(lenders, rng.randrange(1, len(lenders))))
        behind = any(p.lenders[w].epoch < p.epoch and p.epochs[p.lenders[w].epoch][1] for w in absent)
        for v in variants:
            q = copy.deepcopy(p)
            for k in list(q.positions):
                qq, io, _pr, _dl = q.positions[k]
                q.repay(k, cdiv(qq * (q.index - io), WAD), 0)
            q.advance(q.lump_vest_s + 1)
            for w in lenders:
                if w not in absent or v.startswith("every"):
                    q._touch(w)
                elif v.startswith("pot-only"):
                    q._touch_pot(w)
                else:                                   # a third party's sync: refused behind a rebase record
                    L = q.lenders[w]
                    if not (L.epoch < q.epoch and q.epochs[L.epoch][1] and (L.shares or L.lock or L.lock_term)):
                        q._touch(w)
            for _ in range(2):
                for w in lenders:
                    if w not in absent:
                        q.claim_sol(w)
            unpaid = sum(max(q.lenders[w].pending, 0) for w in lenders if w not in absent)
            bound = q.n_touch + 2 * len(lenders) + q.epoch + 2
            row = out[v]
            row[0] += behind
            row[1] += unpaid > bound
            row[2] += unpaid
            row[3] = max(row[3], unpaid)
    return [(v, *out[v]) for v in variants]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ops", type=int, default=60_000)
    ap.add_argument("--seed", type=int, default=20260927)
    a = ap.parse_args()
    for mode, cap in (("continuous", 9_000), ("continuous", 10_000), ("release", 9_000)):
        c, ep, mneg, strand = fuzz(a.ops, a.seed, mode, cap)
        print(f"fuzz[{mode}, util cap {cap/100:.0f}%]: {a.ops} ops, invariants held (liveness checked every "
              f"2,000 ops); share epochs rolled {ep}; largest sum of negative pendings {mneg:,} lamports; "
              f"worst stranded SOL after wind-down {strand:,} lamports (all within the rounding bound; no "
              f"rebase-orphan shares, KI-65); op counts {dict(sorted(c.items()))}")
    print("\nR3A-2 stranded lender SOL after an honest wind-down (40 seeds x 3,000 ops, reviewer's r3_stranded.py):")
    print("| model | realised inflow (SOL) | stranded (SOL) | worst seed (SOL) | seeds above 0.01 SOL |")
    print("|---|---|---|---|---|")
    for label, (inflow, stranded, worst, above) in stranded_table().items():
        print(f"| {label} | {inflow/1e9:,.1f} | {stranded/1e9:.6f} | {worst/1e9:.6f} | {above} |")
    print("\nJIT rent capture (attacker deposits 9x pool just before a 30-day position closes, withdraws at once):")
    print("| accounting | rent paid (lamports) | attacker SOL | honest lender SOL | attacker tokens back |")
    print("|---|---|---|---|---|")
    for mode, rent, att, hon, tok in jit_demo():
        lab = "(a) continuous" if mode == "continuous" else "(b) release"
        print(f"| {lab} | {rent:,} | {att:,} ({100*att/rent:.1f}%) | {hon:,} ({100*hon/rent:.1f}%) | {tok:,} |")
    w, hs, tot = lump_legacy_row()
    print("\nR3B-2 whale deposits 9x just before the last position expires, withdraws after h hours "
          "(lender income 3.2699 SOL = 85 % of 2.4165 SOL rent + 1.2159 SOL expiry fee):")
    print("| model | h | whale SOL (share of lender income) | honest lender SOL |")
    print("|---|---|---|---|")
    print(f"| v0.3 (lumps immediate) | 0 | {w/1e9:.4f} ({100*w/tot:.1f}%) | {hs/1e9:.4f} |")
    for label, h, w, hs, tot in jit_hold_table():
        print(f"| {label} | {h} | {w/1e9:.4f} ({100*w/tot:.1f}%) | {hs/1e9:.4f} |")
    print("\nR3B-1 locks: A withdrew its free part once at 90 % utilisation; A's allowance as a share of its "
          "pro-rata part of unborrowed inventory (B, who never withdrew, is always at 100 %):")
    print("| market path | lock rule | after 24 h | after 7 d | after 30 d | after 32 d |")
    print("|---|---|---|---|---|---|")
    for scen, label, cells in stuck_lock_table():
        print(f"| {scen} | {label} | " + " | ".join(cells) + " |")
    print("\nR4A-2 / R4B-3 stale aggregate lock (reviewers' stale_lock.py): E withdrew its free part at 50 % "
          "utilisation; positions roll, Pi stays; at day 33 E's lock has expired but nobody touched E; F, G, then E "
          "withdraw the most they may (tokens, 1e12 = 1,000 units of free inventory T):")
    print("| model | E's lock_until (day) | free T | F gets | G gets | E gets |")
    print("|---|---|---|---|---|---|")
    for label, until, T0, f, g, e in stale_lock_demo():
        print(f"| {label} | {until:.2f} | {T0:,} | {f:,} | {g:,} | {e:,} |")
    print("\nR4A-2 / R4B-3 I-P4 on the state the program sees (each lender touched alone), reviewers' lock-heavy op mix, "
          "6 seeds x 4,000 ops sampled every 25 ops:")
    print("| model | sampled states | states with sum(allowances) > T + dust | worst sum / T |")
    print("|---|---|---|---|")
    for label, kw in (("v0.4 (touch-time expiry, ceil'd terms)", dict(lock_calendar=False, lock_floor=False)),
                      ("expiry calendar, ceil'd terms", dict(lock_floor=False)),
                      ("v0.4.1 (calendar, floored terms)", {})):
        tot = [0, 0, 0.0]
        for sd in range(6):
            c, b, w = lock_fuzz(4000, sd, **kw)
            tot[0] += c
            tot[1] += b
            tot[2] = max(tot[2], w)
        print(f"| {label} | {tot[0]} | {tot[1]} | {tot[2]:.2f} |")
    print("\nR4A-8 / R4B-8 a position stuck in PumpMigrating (deadline day 30) reaches S3 at day 33 with the timers at their "
          "compiled maxima; lock_grace_s at its compiled minimum (1 h):")
    print("| model | lock_until (day) | position resolves (day) |")
    print("|---|---|---|")
    for label, lu, rel in lock_tail_demo():
        print(f"| {label} | {lu:.2f} | {rel:.2f} |")
    print("\nR4A-7 the keeper that executes the wiping sol_settle bundles [sol_settle -> deposit(2 raw units) -> "
          "claim_venue_rewards (1 SOL of pre-wipe cashback)]:")
    print("| model | first step after the wipe | dust depositor gets (lamports) | wiped lenders get (lamports) |")
    print("|---|---|---|---|")
    for label, first, att, old in wipe_gate_demo():
        print(f"| {label} | {first} | {att:,} | {old:,} |")
    print("\nR4B-4 exits and the unreleased vest (3 SOL of lumps):")
    print("| case | variant | leaving / JIT lender gets (lamports) | remaining holder gets (lamports) |")
    print("|---|---|---|---|")
    for case, var, a, b in vest_exit_demo():
        print(f"| {case} | {var} | {a:,} | {b:,} |")
    print("\nKI-65 rebase orphans: three lenders, a settlement vests a 3 SOL lump and rebases the shares (1/1,000), "
          "every lender exits during the vest (converting at its own exit) and claims again after it; 200 seeds:")
    print("| model | runs that rebased | runs with S > 0 after every exit | worst stranded (lamports) | its seed |")
    print("|---|---|---|---|---|")
    for label, runs, left, worst, sd in orphan_vest_demo():
        print(f"| {label} | {runs} | {left} | {worst:,} | {sd} |")
    print("\nKI-65 against R4B-4: a whale, the conversion order and the R3B-2 flush (H: an honest lender whose "
          "shares were not converted yet; 3 SOL lumps):")
    print("| case | model | whale gets (lamports) | H gets (lamports) | stranded (lamports) |")
    print("|---|---|---|---|---|")
    for case, label, w, h, st in whale_order_demo():
        print(f"| {case} | {label} | {w:,} | {h:,} | {st:,} |")
    print("\nKI-66 a surplus holder behind a rebase record that never returns (T-POT's early leaver A and late "
          "joiner C, a rebase rolled before anyone touched C; then C's owner returns):")
    print("| sync of C | pot after it (lamports) | A's next claim | A's pending left | C's shares, converted | "
          "C's claim | lender_sol left |")
    print("|---|---|---|---|---|---|---|")
    for case, pot, a, left, cs, c, sv in pot_touch_demo():
        print(f"| {case} | {pot:,} | {a:,} | {left:,} | {cs:,} | {c:,} | {sv:,} |")
    print("\nKI-66 I-P6 with absent lenders: random pools (late joiners, early leavers, positions across one or more "
          "rebases), a random set of lenders never returns, the present lenders claim after the wind-down; 2,000 seeds:")
    print("| variant | runs with an absent lender behind a rebase | runs with a present lender unpaid above dust | "
          "unpaid in total (lamports) | worst run (lamports) |")
    print("|---|---|---|---|---|")
    for v, behind, short, tot, worst in pot_liveness_table(2_000):
        print(f"| {v} | {behind} | {short} | {tot:,} | {worst:,} |")
    n, rb, conv, orph = lazy_order_check()
    print(f"\nKI-65 lemma, lazy conversion order: {n:,} random pools, {rb:,} rebase records, {conv:,} conversions in "
          f"random order between deposits and 0-token burns; sum(shares) == S once every lender converted, each "
          f"conversion within [floor, floor + 1 per record] (v0.4.1 on the same pools: up to {orph} orphan shares)")
    print("\nR4B-5 a 10 SOL lump, then a 1-lamport sweep every k seconds (share of the 10 SOL released):")
    print("| model | k (s) | released after 6 h | after 24 h |")
    print("|---|---|---|---|")
    for label, per, r6, r24 in restart_grief_table():
        print(f"| {label} | {per} | {100*r6:.1f}% | {100*r24:.1f}% |")
    print("\nR4B-6 JIT whale (x9) around an S2/S3 settlement of the last open position (Q worth 500 SOL at P_ema; "
          "take = B_FV x 1.02, B_FV = Q x (1 + premium)):")
    print("| premium of B_FV over Q.P_ema | model | take above Q.P_ema (lamports) | whale's atomic gain (lamports) |")
    print("|---|---|---|---|")
    for prem, label, extra, gain in premium_jit_table():
        print(f"| {prem/100:.1f}% | {label} | {extra:,} | {max(gain, 0):,} |")
    minted, back = inflation_demo()
    print(f"\nfirst-depositor attack: victim deposits 1e9, receives {minted:,} shares, redeems {back:,} tokens")
    print("\nC0-9 lender run before an S1 (two equal lenders, 90 % utilisation, S1 pays 50 % of token value):")
    print("| withdrawal rule | tokens A withdrew before S1 | A final value | B final value |")
    print("|---|---|---|---|")
    for prorata, took, va, vb in run_demo():
        print(f"| {'v0.4 pro-rata + lock' if prorata else 'v0.1 first come, first served'} | {took:,} | {va:,} | {vb:,} |")
    print("\nV2-2 lender A calls claim_sol N times in one transaction (A's fair realised share shown):")
    print("| claim rule | calls | A's fair realised share | A received | sol_vault left for B |")
    print("|---|---|---|---|---|")
    for label, n, fair, got, left in claim_loop_demo():
        print(f"| {label} | {n} | {fair:,} | {got:,} | {left:,} |")
    print("\nRunner advantage over fair value by utilisation (v0.4 lock rule; churn = 30 rounds of others' "
          "open + close between A's withdrawals):")
    print("| utilisation | no churn | churn |")
    print("|---|---|---|")
    for r in run_util_table():
        print("| " + " | ".join(r) + " |")
    print("\nC0-6 repeated full SOL settlements (critic pool_edge (a)):")
    print("| model | new deposit 1e14 mints | new lender pending after 1,000 SOL lump + rent (lamports) | next deposit after 2nd wipe would mint | > u128 | epochs |")
    print("|---|---|---|---|---|---|")
    for label, minted, npend, m2, over, ep in wipe_demo():
        print(f"| {label} | {minted:.3e} | {npend:,} | {m2:.3e} | {over} | {ep} |")
    got_old, now_dust, dust_total, late, behind, _, jumped = wipe_tail_demo()
    print(f"\nV2-12 / R3B-2 after a full wipe: old epoch's lenders +{got_old:,} lamports (3 SOL unreleased vest + "
          f"1 SOL post-wipe sweep); dust depositor that arrives first and triggers a 1 SOL sweep: {now_dust:,} "
          f"at once, {dust_total:,} in total after a lender joins a minute later (that lender: {late:,}); a lender "
          f"with 0 shares {behind} epochs behind caught up in one touch: {jumped}")
    print("\nR3A-10 all lenders withdrew (no wipe), then a 1 SOL sweep, then a dust deposit:")
    print("| model | dust depositor claims | former lender claims | to protocol |")
    print("|---|---|---|---|")
    for label, d, aa, pr in zero_share_demo():
        print(f"| {label} | {d:,} | {aa:,} | {pr:,} |")
    print("\nR3B-9 / R3A-5 rent_protocol_bps raised 15 % -> 30 % halfway through the only position's life:")
    print("| model | receivable after the position closed | lender pending | sol_vault |")
    print("|---|---|---|---|")
    for label, rc, pend, sv in protocol_change_demo():
        print(f"| {label} | {rc:,} | {pend:,} | {sv:,} |")
    print("\nC0-13 early claim then 90 % rent shortfall (critic pool_edge (b)):")
    print("| model | A claimed | A pending after shortfall | A could exit | B claims now | B pending left | sol_vault | receivable |")
    print("|---|---|---|---|---|---|---|---|")
    for label, a_, pa, esc, b_, pb, sv, rc in escape_demo():
        print(f"| {label} | {a_:,} | {pa:,} | {esc} | {b_:,} | {pb:,} | {sv:,} | {rc:,} |")


if __name__ == "__main__":
    main()
