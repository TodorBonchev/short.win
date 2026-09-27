//! Owner: oracle track. Observation buckets, EMA integration and the history ring (ECON
//! §5.1–§5.4; ARCH §3.0 prologue steps 5 and 7, §4.4). Normative reference:
//! `Market.observe`, `_integrate`, `ring_medians` and `_rewrite_ring_liq` of
//! `sim/shortwin_sim.py` v0.4.1, which this module reproduces to the unit.
//!
//! Prices `P` are WAD-scaled (lamports per raw token unit × 10¹⁸) and liquidities
//! `L = √(X·Y)`; both are `u128`. An observation must be non-zero: a zero price has no
//! reprojection (`reprice` is undefined at `P = 0`) and would pin the EMA at 0 forever,
//! so the caller treats a zero spot as "price undefined" and records nothing.
//!
//! Every state struct is `#[repr(C)]`, fixed-size, all-integer and free of implicit
//! padding whether `u128` is 8- or 16-byte aligned, so the layout is identical on SBF,
//! x86-64 and wasm. These are math types, not account layouts: the program converts them
//! to and from its `PodU128` zero-copy fields (ARCH §2.2, §2.3). Emptiness is encoded by a
//! zero low (`lo == 0`), never by a timestamp, so second 0 is a valid second.

use crate::consts::{COVERAGE_GAP_S, EMA_TAU_S};
use crate::error::MathError;
use crate::num::{BPS, Rounding, U256, mul_div, overflow_to_max, saturating_u64, to_u128};
use crate::quote::reprice;

pub use crate::consts::RING_LEN;

/// The observation bucket of the current second (ECON §5.1; ARCH §2.3.4 `b_*`).
///
/// Open iff `lo != 0`. `lo`/`hi` are the lowest and highest spot price recorded in second
/// `sec`, `llo`/`lhi` the lowest and highest liquidity.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Bucket {
    pub lo: u128,
    pub hi: u128,
    pub llo: u128,
    pub lhi: u128,
    pub sec: i64,
    pub _pad: [u8; 8],
}

impl Bucket {
    /// The empty bucket.
    pub const EMPTY: Self = Self {
        lo: 0,
        hi: 0,
        llo: 0,
        lhi: 0,
        sec: 0,
        _pad: [0; 8],
    };

    /// Whether the bucket holds at least one observation.
    pub const fn is_open(&self) -> bool {
        self.lo != 0
    }

    /// Empties the bucket.
    pub fn clear(&mut self) {
        *self = Self::EMPTY;
    }
}

/// The last completed bucket (ECON §5.4; ARCH §2.3.4 `lb_*`): the evidence of the robust
/// spot and of the exit references. Set iff `lo != 0`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LastBucket {
    pub lo: u128,
    pub hi: u128,
    pub sec: i64,
    pub _pad: [u8; 8],
}

impl LastBucket {
    /// No completed bucket yet.
    pub const NONE: Self = Self {
        lo: 0,
        hi: 0,
        sec: 0,
        _pad: [0; 8],
    };

    /// Whether a completed bucket exists.
    pub const fn is_set(&self) -> bool {
        self.lo != 0
    }

    /// A completed bucket exists and `now − sec ≤ max_age_s` (ECON §5.4, §8.2: the fresh
    /// reference; ARCH §3.6 precondition 2, `ref_max_age_s`).
    pub fn is_fresh(&self, now: i64, max_age_s: u8) -> bool {
        self.is_set() && now.saturating_sub(self.sec) <= i64::from(max_age_s)
    }
}

/// EMA state (ECON §5.2; ARCH §2.3.4 `ema_price`, `ema_liq`, `liq_slow`, `ema_ts`,
/// `slow_ts`, `ema_count`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Ema {
    /// `P_ema` (WAD).
    pub price: u128,
    /// `L_ema`, the fast liquidity EMA.
    pub liq: u128,
    /// `L_slow`, the slow liquidity EMA (`liq_slow_tau_s`).
    pub liq_slow: u128,
    /// Second of the last integration of `price` and `liq`.
    pub ts: i64,
    /// Second of the last integration of `liq_slow`.
    pub slow_ts: i64,
    /// Number of integrations so far (`ema_count`; the settlement persistence counts it).
    pub count: u64,
    pub _pad: [u8; 8],
}

impl Ema {
    /// State at market creation: `P_ema = P_spot`, `L_ema = L_slow = √(X·Y)` (ECON §5.2;
    /// ARCH §4.4 "Initialisation"). `InvalidInput` if `p` or `l` is 0.
    pub fn new(now: i64, p: u128, l: u128) -> Result<Self, MathError> {
        nonzero(p, l)?;
        Ok(Self {
            price: p,
            liq: l,
            liq_slow: l,
            ts: now,
            slow_ts: now,
            count: 0,
            _pad: [0; 8],
        })
    }
}

/// One ring entry (ECON §2.1 `RingEntry`, §5.4; ARCH §2.3.4, §4.4): the EMA state after an
/// integration, its cached reprojection and the extremes of the bucket it integrated.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RingEntry {
    /// `P_ema` after the step (growth `g`, the expiry reference).
    pub p: u128,
    /// `L_ema` after the step.
    pub l: u128,
    /// Cached `(x, y) = reprice(l, p)` (decision 3): the position-independent state that the
    /// liquidation EMA leg and the D4 margin quote on. `(0, 0)` when `reprice` is undefined
    /// (the sim's `reprice(l, p) or (0, 0)`); a quote on `x = 0` is undefined.
    pub x: u128,
    pub y: u128,
    /// Lowest and highest spot price of the integrated bucket (v0.4.1, R4B-2: the
    /// settlement box's medians). The creation entry holds `P` in both.
    pub lo: u128,
    pub hi: u128,
    /// Lowest and highest liquidity of the integrated bucket (the creation entry: `L`).
    pub llo: u128,
    pub lhi: u128,
    /// Second of the integration (the roll time, not the bucket's second).
    pub ts: i64,
    pub _pad: [u8; 8],
}

impl RingEntry {
    /// An entry whose `(x, y)` is `reprice(l, p)`, or `(0, 0)` where that is undefined
    /// (sim `Market._entry`).
    pub fn reprojected(
        ts: i64,
        p: u128,
        l: u128,
        lo: u128,
        hi: u128,
        llo: u128,
        lhi: u128,
    ) -> Self {
        let (x, y) = reprice(l, p).unwrap_or((0, 0));
        Self {
            p,
            l,
            x,
            y,
            lo,
            hi,
            llo,
            lhi,
            ts,
            _pad: [0; 8],
        }
    }
}

/// The last [`RING_LEN`] integrations (ECON §5.4). `entries[head]` is the newest; `len`
/// entries are valid. A ring built by [`Ring::new`] is never empty.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Ring {
    pub entries: [RingEntry; RING_LEN],
    pub head: u8,
    pub len: u8,
    pub _pad: [u8; 14],
}

impl Ring {
    /// The ring at market creation: one entry `(now, P, L, reprice(L, P), P, P, L, L)`
    /// (sim `Market.__init__`; ARCH §4.4 "Initialisation"). `InvalidInput` if `p` or `l`
    /// is 0.
    pub fn new(now: i64, p: u128, l: u128) -> Result<Self, MathError> {
        nonzero(p, l)?;
        let mut ring = Self::default();
        ring.push(RingEntry::reprojected(now, p, l, p, p, l, l))?;
        Ok(ring)
    }

    /// `InvalidInput` unless `head < RING_LEN` and `len ≤ RING_LEN` (a corrupted account).
    pub fn check(&self) -> Result<(), MathError> {
        if usize::from(self.head) < RING_LEN && usize::from(self.len) <= RING_LEN {
            Ok(())
        } else {
            Err(MathError::InvalidInput)
        }
    }

    /// Number of valid entries (at most [`RING_LEN`]).
    pub fn len(&self) -> usize {
        usize::from(self.len).min(RING_LEN)
    }

    /// Whether the ring has no entry.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Slot of the `k`-th newest entry (`k = 0` is the newest).
    fn slot(&self, k: usize) -> Option<usize> {
        if k >= self.len() {
            return None;
        }
        usize::from(self.head)
            .checked_add(RING_LEN)?
            .checked_sub(k)?
            .checked_rem(RING_LEN)
    }

    /// The `k`-th newest entry (`k = 0` is the newest).
    pub fn get(&self, k: usize) -> Option<&RingEntry> {
        self.entries.get(self.slot(k)?)
    }

    /// The newest entry.
    pub fn newest(&self) -> Option<&RingEntry> {
        self.get(0)
    }

    /// Entries from the newest to the oldest.
    pub fn iter(&self) -> impl Iterator<Item = &RingEntry> + '_ {
        (0..self.len()).filter_map(move |k| self.get(k))
    }

    /// Appends `e` as the newest entry, dropping the oldest when full.
    pub fn push(&mut self, e: RingEntry) -> Result<(), MathError> {
        self.check()?;
        let head = if self.len == 0 {
            0
        } else {
            usize::from(self.head)
                .checked_add(1)
                .and_then(|h| h.checked_rem(RING_LEN))
                .ok_or(MathError::Overflow)?
        };
        let slot = self.entries.get_mut(head).ok_or(MathError::InvalidInput)?;
        *slot = e;
        self.head = u8::try_from(head).map_err(|_| MathError::Overflow)?;
        self.len = u8::try_from(
            self.len()
                .checked_add(1)
                .ok_or(MathError::Overflow)?
                .min(RING_LEN),
        )
        .map_err(|_| MathError::Overflow)?;
        Ok(())
    }
}

/// Market-level EMA parameters (ARCH §2.3.3 scope M, §2.3.4 `ema_params`; ECON §5.2,
/// §15). τ is the compile-time constant [`EMA_TAU_S`] (R3B-6).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmaParams {
    /// Δt_max (`ema_dtmax_s`).
    pub dtmax_s: u16,
    pub _pad: [u8; 2],
    /// C (`ema_clamp_bps`): an observation is clamped to `[P/(1 + C), P·(1 + C)]`.
    pub clamp_bps: u32,
    /// `liq_slow_tau_s`.
    pub slow_tau_s: u32,
    /// `liq_slow_dtmax_s`.
    pub slow_dtmax_s: u32,
}

impl EmaParams {
    /// Defaults (ECON §5.7, §15): Δt_max = 2 s, C = 100 %, slow τ = 3,600 s, slow
    /// Δt_max = 60 s.
    pub const DEFAULT: Self = Self::new(2, 10_000, 3_600, 60);

    /// The parameters with the given values.
    pub const fn new(dtmax_s: u16, clamp_bps: u32, slow_tau_s: u32, slow_dtmax_s: u32) -> Self {
        Self {
            dtmax_s,
            _pad: [0; 2],
            clamp_bps,
            slow_tau_s,
            slow_dtmax_s,
        }
    }

    /// `InvalidInput` outside the compiled hard bounds (ARCH §2.3.3; ECON §15; sim
    /// `TIER_BOUNDS` and `COMPILED_BOUNDS`; [`crate::params::Rule::bounds`]): Δt_max ∈
    /// [1, 2], C ∈ [5,000, 10,000], slow τ ∈ [600, 86,400], slow Δt_max ∈ [1, 600].
    pub fn validate(&self) -> Result<(), MathError> {
        use crate::params::Rule;
        let within = |rule: Rule, v: u32| {
            let (lo, hi) = rule.bounds();
            (lo..=hi).contains(&u64::from(v))
        };
        let ok = within(Rule::EmaDtmaxS, self.dtmax_s.into())
            && within(Rule::EmaClampBps, self.clamp_bps)
            && within(Rule::LiqSlowTauS, self.slow_tau_s)
            && within(Rule::LiqSlowDtmaxS, self.slow_dtmax_s);
        if ok {
            Ok(())
        } else {
            Err(MathError::InvalidInput)
        }
    }
}

impl Default for EmaParams {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The whole observation state of a market, in the order the prologue uses it
/// (ARCH §3.0 steps 5 and 7). A convenience wrapper over the free functions.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Observation {
    pub ema: Ema,
    pub ring: Ring,
    pub bucket: Bucket,
    pub last: LastBucket,
}

impl Observation {
    /// State at market creation (ARCH §4.4 "Initialisation").
    pub fn new(now: i64, p: u128, l: u128) -> Result<Self, MathError> {
        Ok(Self {
            ema: Ema::new(now, p, l)?,
            ring: Ring::new(now, p, l)?,
            bucket: Bucket::EMPTY,
            last: LastBucket::NONE,
        })
    }

    /// Prologue step 5: [`integrate`] a completed bucket, if any; `Some(gap)` if it rolled.
    pub fn roll(&mut self, now: i64, params: &EmaParams) -> Result<Option<i64>, MathError> {
        integrate(
            &mut self.ema,
            &mut self.ring,
            &mut self.bucket,
            &mut self.last,
            now,
            params,
        )
    }

    /// Prologue step 7: [`record`] the pre-trade `(P, L)` of an executable venue.
    pub fn record(&mut self, now: i64, p: u128, l: u128) -> Result<(), MathError> {
        record(&mut self.bucket, now, p, l)
    }

    /// [`rewrite_liquidity`] at a venue change.
    pub fn rewrite_liquidity(&mut self, l_new: u128, kind: VenueChange) -> Result<(), MathError> {
        rewrite_liquidity(&mut self.ema, &mut self.ring, &mut self.bucket, l_new, kind)
    }
}

fn nonzero(p: u128, l: u128) -> Result<(), MathError> {
    if p == 0 || l == 0 {
        Err(MathError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Adds the pre-trade spot `(P, L)` to the bucket of second `now` (ECON §5.1; ARCH §4.4
/// `record`, prologue step 7). Every instruction of the second records; there is no
/// per-slot limit.
///
/// The bucket must be empty or already be second `now`: a completed bucket is rolled by
/// [`integrate`] first (prologue step 5), so an open bucket of another second means the
/// caller skipped the roll or the clock went backwards. Errors: `InvalidInput` in that
/// case, or if `p` or `l` is 0.
pub fn record(bucket: &mut Bucket, now: i64, p: u128, l: u128) -> Result<(), MathError> {
    nonzero(p, l)?;
    if !bucket.is_open() {
        *bucket = Bucket {
            lo: p,
            hi: p,
            llo: l,
            lhi: l,
            sec: now,
            _pad: [0; 8],
        };
        return Ok(());
    }
    if bucket.sec != now {
        return Err(MathError::InvalidInput);
    }
    bucket.lo = bucket.lo.min(p);
    bucket.hi = bucket.hi.max(p);
    bucket.llo = bucket.llo.min(l);
    bucket.lhi = bucket.lhi.max(l);
    Ok(())
}

/// Rolls a completed bucket into the EMA, the slow liquidity EMA and the ring (ECON §5.2;
/// ARCH §4.4 `integrate`, prologue step 5). Nothing happens unless the bucket is open and
/// `now > bucket.sec`, so the EMA an instruction reads never contains that instruction's
/// own observation (the one-second lag) and it moves at most once per second.
///
/// ```text
/// obs   = clamp(P_ema, b_lo, b_hi)                        directional aggregation
/// obs   = clamp(obs, ⌊P_ema·10⁴/(10⁴+C)⌋, ⌊P_ema·(10⁴+C)/10⁴⌋)
/// lobs  = the same for L_ema with b_llo, b_lhi
/// dt    = min(now − ema_ts, Δt_max)
/// P_ema = ⌊(P_ema·τ + obs·dt)/(τ + dt)⌋ ;  L_ema likewise with lobs      τ = EMA_TAU_S
/// dts   = min(now − slow_ts, liq_slow_dtmax_s) ; L_slow = ⌊(L_slow·τ_slow + lobs·dts)/(τ_slow + dts)⌋
/// ring.push(now, P_ema, L_ema, reprice(L_ema, P_ema), b_lo, b_hi, b_llo, b_lhi)
/// ```
///
/// then `ema_ts = slow_ts = now`, `count += 1`, the bucket becomes the last completed
/// bucket and is cleared. The clamped `obs`/`lobs` drive the step but are not stored
/// (v0.4.1: the ring keeps the bucket extremes, R4B-2). Every division floors, exactly as
/// the sim. If `dt ≤ 0` (only reachable when the EMA was re-initialised after the bucket
/// opened) the EMA and ring are left alone and only the bucket moves, again as the sim.
///
/// Returns `None` if nothing was rolled, else `Some(now − ema_ts)` with the EMA timestamp
/// **before** the roll: the pre-roll gap that the caller reports as `CoverageGap` when the
/// market has open positions ([`coverage_gap`]; ARCH §4.4, A69).
///
/// All arithmetic is computed before anything is written, so an error leaves the state
/// untouched. Errors: `InvalidInput` for a corrupted ring; `DivByZero` if `τ_slow + dts` is
/// 0 (only with parameters outside [`EmaParams::validate`]).
pub fn integrate(
    ema: &mut Ema,
    ring: &mut Ring,
    bucket: &mut Bucket,
    last: &mut LastBucket,
    now: i64,
    params: &EmaParams,
) -> Result<Option<i64>, MathError> {
    if !bucket.is_open() || now <= bucket.sec {
        return Ok(None);
    }
    ring.check()?;
    let gap = now.saturating_sub(ema.ts);
    let dt = elapsed(now, ema.ts, params.dtmax_s.into());
    if dt > 0 {
        let c = u128::from(params.clamp_bps);
        let obs = aggregate(ema.price, bucket.lo, bucket.hi, c)?;
        let lobs = aggregate(ema.liq, bucket.llo, bucket.lhi, c)?;
        let tau = u128::from(EMA_TAU_S);
        let price = weighted(ema.price, tau, obs, dt)?;
        let liq = weighted(ema.liq, tau, lobs, dt)?;
        let dts = elapsed(now, ema.slow_ts, params.slow_dtmax_s.into());
        let liq_slow = weighted(ema.liq_slow, params.slow_tau_s.into(), lobs, dts)?;
        let count = ema.count.checked_add(1).ok_or(MathError::Overflow)?;
        ring.push(RingEntry::reprojected(
            now, price, liq, bucket.lo, bucket.hi, bucket.llo, bucket.lhi,
        ))?;
        *ema = Ema {
            price,
            liq,
            liq_slow,
            ts: now,
            slow_ts: now,
            count,
            _pad: ema._pad,
        };
    }
    *last = LastBucket {
        lo: bucket.lo,
        hi: bucket.hi,
        sec: bucket.sec,
        _pad: [0; 8],
    };
    bucket.clear();
    Ok(Some(gap))
}

/// Whether an integration with pre-roll gap `gap_s` ([`integrate`]) emits `CoverageGap`:
/// the market has open positions and `gap_s > COVERAGE_GAP_S` (ARCH §4.4, A69).
pub const fn coverage_gap(gap_s: i64, open_positions: u64) -> bool {
    open_positions > 0 && gap_s > COVERAGE_GAP_S
}

/// `min(max(now − since, 0), cap)` seconds.
fn elapsed(now: i64, since: i64, cap: u128) -> u128 {
    let d = i128::from(now).saturating_sub(i128::from(since));
    u128::try_from(d).map_or(0, |d| d.min(cap))
}

/// Directional aggregation of one bucket side, then the ±C clamp (ECON §5.2).
fn aggregate(e: u128, lo: u128, hi: u128, c_bps: u128) -> Result<u128, MathError> {
    let bps = u128::from(BPS);
    let x = e.max(lo).min(hi);
    let den = bps.checked_add(c_bps).ok_or(MathError::Overflow)?;
    let lower = mul_div(e, bps, den, Rounding::Floor)?;
    // The upper clamp only ever bounds a u128 from above: past u128 it cannot bind.
    let upper = overflow_to_max(mul_div(e, den, bps, Rounding::Floor))?;
    Ok(x.max(lower).min(upper))
}

/// `⌊(a·wa + b·wb)/(wa + wb)⌋`. ECON §2.2 bounds `P_ema·(τ + dt)` at 4.2·10³⁶, so the u128
/// path is the one taken; the 256-bit fallback keeps any larger state exact (the result
/// lies between `a` and `b`, so it fits).
fn weighted(a: u128, wa: u128, b: u128, wb: u128) -> Result<u128, MathError> {
    let den = wa.checked_add(wb).ok_or(MathError::Overflow)?;
    let fast = a
        .checked_mul(wa)
        .zip(b.checked_mul(wb))
        .and_then(|(x, y)| x.checked_add(y));
    if let Some(num) = fast {
        return num.checked_div(den).ok_or(MathError::DivByZero);
    }
    let num = U256::from_product(a, wa)
        .checked_add(U256::from_product(b, wb))
        .ok_or(MathError::Overflow)?;
    let (q, _) = num.div_rem_u128(den).ok_or(MathError::DivByZero)?;
    to_u128(q)
}

/// Robust spot `P_rs = min(P_live, lb_lo)`, defined only while the last completed bucket
/// is fresh (`now − lb_sec ≤ max_age_s`) (ECON §5.4; ARCH §4.4). `None` means the spot leg
/// cannot be confirmed this second: the keeper cranks one second and acts the next.
pub fn robust_spot(p_live: u128, last: &LastBucket, now: i64, max_age_s: u8) -> Option<u128> {
    last.is_fresh(now, max_age_s).then(|| p_live.min(last.lo))
}

/// Sustained EMA growth `g` in bps per second (ECON §2.1, §5.4; ARCH §4.4):
/// `max(0, min over the newest n ring steps of ⌊(P_i − P_{i−1})·10⁴/(P_{i−1}·Δt_i)⌋)`.
///
/// 0 if the ring has fewer than `n + 1` entries, if `n` is 0, or if any of those steps is
/// flat or falling or has `Δt ≤ 0` (sim `ema_growth_bps`). A single held-pump second
/// produces one step; only a genuine trend produces `n`. Saturates at `u64::MAX` bps/s
/// (the sim's integer is unbounded); an integrated step grows at most ×(1 + C) ≤ ×2, so
/// rings built by [`integrate`] stay orders of magnitude below it. Errors: `InvalidInput`
/// for a corrupted ring.
pub fn sustained_growth(ring: &Ring, n: u8) -> Result<u64, MathError> {
    ring.check()?;
    let n = usize::from(n);
    if n == 0 || ring.len() <= n {
        return Ok(0);
    }
    let step = |k: usize| -> Option<(&RingEntry, &RingEntry)> {
        Some((ring.get(k.checked_add(1)?)?, ring.get(k)?))
    };
    // Any non-positive step makes the minimum ≤ 0, whatever the other steps are.
    for k in 0..n {
        let (old, new) = step(k).ok_or(MathError::InvalidInput)?;
        if new.ts <= old.ts || old.p == 0 || new.p <= old.p {
            return Ok(0);
        }
    }
    let mut g = u128::MAX;
    for k in 0..n {
        let (old, new) = step(k).ok_or(MathError::InvalidInput)?;
        let rise = new.p.checked_sub(old.p).ok_or(MathError::Underflow)?;
        let dt = u128::try_from(i128::from(new.ts).saturating_sub(i128::from(old.ts)))
            .map_err(|_| MathError::Underflow)?;
        // ⌊⌊a/b⌋/c⌋ = ⌊a/(b·c)⌋, and a rate past u128 cannot be the minimum of a finite one.
        let rate = overflow_to_max(mul_div(rise, u128::from(BPS), old.p, Rounding::Floor))?
            .checked_div(dt)
            .ok_or(MathError::DivByZero)?;
        g = g.min(rate);
    }
    Ok(saturating_u64(g))
}

/// The ring medians of the settlement box `(P_mlo, P_mhi, L_mlo, L_mhi)` (ECON §2.1, §5.6;
/// ARCH §2.2, §4.5; sim `Market.ring_medians`): the **lower** median of the entries' bucket
/// lows `lo` and `llo` (index `⌊(n − 1)/2⌋` of the sorted values, the 4th smallest of 8)
/// and the **upper** median of their highs `hi` and `lhi` (index `⌊n/2⌋`). Moving `P_mlo`
/// up or `P_mhi` down needs 5 of 8 entries, and only an attacker-only second can raise a
/// bucket low or lower a bucket high (R4B-2, R4A-9).
///
/// Errors: `InvalidInput` for a corrupted or empty ring (a market's ring always holds its
/// creation entry).
pub fn ring_medians(ring: &Ring) -> Result<(u128, u128, u128, u128), MathError> {
    ring.check()?;
    let n = ring.len();
    let lower = n.checked_sub(1).ok_or(MathError::InvalidInput)? / 2;
    let upper = n / 2;
    let mut cols = [[0u128; RING_LEN]; 4];
    for (k, e) in ring.iter().enumerate() {
        for (col, v) in cols.iter_mut().zip([e.lo, e.hi, e.llo, e.lhi]) {
            *col.get_mut(k).ok_or(MathError::InvalidInput)? = v;
        }
    }
    let mut pick = |col: usize, idx: usize| -> Result<u128, MathError> {
        let vals = cols
            .get_mut(col)
            .and_then(|c| c.get_mut(..n))
            .ok_or(MathError::InvalidInput)?;
        vals.sort_unstable();
        vals.get(idx).copied().ok_or(MathError::InvalidInput)
    };
    Ok((
        pick(0, lower)?,
        pick(1, upper)?,
        pick(2, lower)?,
        pick(3, upper)?,
    ))
}

/// The kind of venue change that rewrites the ring's liquidity (ARCH §2.5, §2.6, §4.4,
/// A72; ECON §5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VenueChange {
    /// Graduation to the canonical pool: `L_ema = L_slow = l_new` (sim `_switch_to_pool`).
    Graduation,
    /// Raydium adoption or fast path: `L_ema = min(l_new, L_ema)`, `L_slow = min(l_new,
    /// L_slow)`, so a lagging `L_slow` is never raised (sim `fast_switch`).
    Raydium,
}

/// Rewrites the liquidity side of the observation state at a venue change (ECON §5.2
/// "Initialisation and venue changes"; ARCH §4.4 `rewrite_liquidity`, A72; sim
/// `_switch_to_pool` / `fast_switch` and `_rewrite_ring_liq`): `L_ema` and `L_slow` as
/// [`VenueChange`] says; then **every** ring entry gets `l = llo = lhi = L_ema` (the reset
/// value) and `(x, y) = reprice(L_ema, p)` recomputed, while its `ts`, `p`, `lo` and `hi`
/// carry over; the open bucket is cleared. `P_ema`, the EMA timestamps, the count and the
/// last completed bucket are unchanged. `l_new` is the new venue's `√(X·Y)`.
///
/// Without the rewrite the ring medians would carry the old venue's liquidity and the
/// settlement box would stay more than 1 % wide in `L` for 4 s after each change.
///
/// Errors: `InvalidInput` if `l_new` is 0 or the ring is corrupted; nothing is written
/// then.
pub fn rewrite_liquidity(
    ema: &mut Ema,
    ring: &mut Ring,
    bucket: &mut Bucket,
    l_new: u128,
    kind: VenueChange,
) -> Result<(), MathError> {
    if l_new == 0 {
        return Err(MathError::InvalidInput);
    }
    ring.check()?;
    let (liq, liq_slow) = match kind {
        VenueChange::Graduation => (l_new, l_new),
        VenueChange::Raydium => (l_new.min(ema.liq), l_new.min(ema.liq_slow)),
    };
    for k in 0..ring.len() {
        let slot = ring.slot(k).ok_or(MathError::InvalidInput)?;
        let e = ring.entries.get_mut(slot).ok_or(MathError::InvalidInput)?;
        *e = RingEntry::reprojected(e.ts, e.p, liq, e.lo, e.hi, liq, liq);
    }
    ema.liq = liq;
    ema.liq_slow = liq_slow;
    bucket.clear();
    Ok(())
}

const _: () = {
    assert!(core::mem::size_of::<Bucket>() == 80);
    assert!(core::mem::size_of::<LastBucket>() == 48);
    assert!(core::mem::size_of::<Ema>() == 80);
    assert!(core::mem::size_of::<RingEntry>() == 144);
    assert!(core::mem::size_of::<Ring>() == 1_168);
    assert!(core::mem::size_of::<EmaParams>() == 16);
    assert!(core::mem::size_of::<Observation>() == 1_376);
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use core::mem::offset_of;

    const WAD: u128 = crate::num::WAD;

    fn entry(ts: i64, p: u128, lo: u128, hi: u128, llo: u128, lhi: u128) -> RingEntry {
        RingEntry::reprojected(ts, p, 1_000, lo, hi, llo, lhi)
    }

    #[test]
    fn layouts_have_no_implicit_padding() {
        assert_eq!(offset_of!(Bucket, sec), 64);
        assert_eq!(offset_of!(LastBucket, sec), 32);
        assert_eq!(offset_of!(Ema, ts), 48);
        assert_eq!(offset_of!(Ema, count), 64);
        assert_eq!(offset_of!(RingEntry, x), 32);
        assert_eq!(offset_of!(RingEntry, lo), 64);
        assert_eq!(offset_of!(RingEntry, lhi), 112);
        assert_eq!(offset_of!(RingEntry, ts), 128);
        assert_eq!(offset_of!(Ring, head), 1_152);
        assert_eq!(offset_of!(Ring, len), 1_153);
        assert_eq!(offset_of!(EmaParams, clamp_bps), 4);
        assert_eq!(offset_of!(Observation, ring), 80);
        assert_eq!(offset_of!(Observation, bucket), 1_248);
        assert_eq!(offset_of!(Observation, last), 1_328);
    }

    #[test]
    fn creation_entry() {
        let o = Observation::new(7, 4 * WAD, 1_000).unwrap();
        let e = *o.ring.newest().unwrap();
        assert_eq!((e.ts, e.p, e.l), (7, 4 * WAD, 1_000));
        assert_eq!((e.lo, e.hi, e.llo, e.lhi), (4 * WAD, 4 * WAD, 1_000, 1_000));
        // sp = 2·10⁹: X = 1,000·10⁹/(2·10⁹) = 500, Y = 2,000.
        assert_eq!((e.x, e.y), (500, 2_000));
        assert_eq!(ring_medians(&o.ring), Ok((4 * WAD, 4 * WAD, 1_000, 1_000)));
        assert_eq!(Ring::new(0, 0, 1), Err(MathError::InvalidInput));
    }

    #[test]
    fn record_and_lag() {
        let mut o = Observation::new(0, WAD, 1_000).unwrap();
        let prm = EmaParams::DEFAULT;
        o.record(1, 2 * WAD, 1_000).unwrap();
        // Same second: nothing to roll, the EMA does not see its own observation.
        assert_eq!(o.roll(1, &prm), Ok(None));
        assert_eq!(o.ema.price, WAD);
        o.record(1, 3 * WAD, 900).unwrap();
        assert_eq!((o.bucket.lo, o.bucket.hi), (2 * WAD, 3 * WAD));
        assert_eq!((o.bucket.llo, o.bucket.lhi), (900, 1_000));
        assert_eq!(o.roll(2, &prm), Ok(Some(2)));
        assert_eq!(o.roll(2, &prm), Ok(None));
        // obs = clamp(WAD, 2 WAD, 3 WAD) = 2 WAD; dt = 2 (capped); (5·1 + 2·2)/7 WAD.
        assert_eq!(o.ema.price, (5 * WAD + 4 * WAD) / 7);
        assert_eq!(o.ema.count, 1);
        assert_eq!(o.ring.len(), 2);
        let e = o.ring.newest().unwrap();
        assert_eq!((e.lo, e.hi, e.llo, e.lhi), (2 * WAD, 3 * WAD, 900, 1_000));
        assert_eq!((e.x, e.y), reprice(e.l, e.p).unwrap());
        assert_eq!(o.last.sec, 1);
        assert!(!o.bucket.is_open());
    }

    #[test]
    fn record_rejects_other_second_and_zero() {
        let mut b = Bucket::EMPTY;
        record(&mut b, 5, 1, 1).unwrap();
        assert_eq!(record(&mut b, 6, 1, 1), Err(MathError::InvalidInput));
        assert_eq!(record(&mut b, 4, 1, 1), Err(MathError::InvalidInput));
        assert_eq!(record(&mut b, 5, 0, 1), Err(MathError::InvalidInput));
        assert_eq!(record(&mut b, 5, 1, 0), Err(MathError::InvalidInput));
    }

    #[test]
    fn mixed_bucket_freezes_ema() {
        let mut o = Observation::new(0, 10 * WAD, 7).unwrap();
        o.record(1, 5 * WAD, 7).unwrap();
        o.record(1, 50 * WAD, 7).unwrap();
        o.roll(2, &EmaParams::DEFAULT).unwrap();
        assert_eq!(o.ema.price, 10 * WAD);
        // The frozen second keeps its honest extremes (v0.4.1).
        let e = o.ring.newest().unwrap();
        assert_eq!((e.lo, e.hi), (5 * WAD, 50 * WAD));
    }

    #[test]
    fn step_bounds_at_default_clamp() {
        // +16.7 % at dt = 1 and +28.6 % at dt = 2 (ECON §5.2).
        for (dt, num, den) in [(1i64, 7u128, 6u128), (2, 9, 7)] {
            let mut o = Observation::new(0, 6 * WAD, 7).unwrap();
            o.record(10, 1_000 * WAD, 7).unwrap();
            o.ema.ts = 11 - dt;
            assert_eq!(o.roll(11, &EmaParams::DEFAULT), Ok(Some(dt)));
            assert_eq!(o.ema.price, 6 * WAD * num / den);
        }
    }

    #[test]
    fn stale_ema_rolls_without_a_step() {
        let mut o = Observation::new(10, WAD, 7).unwrap();
        o.record(5, 2 * WAD, 7).unwrap();
        // ema_ts = 10 > now = 6: dt ≤ 0, only the bucket moves; the gap is negative.
        assert_eq!(o.roll(6, &EmaParams::DEFAULT), Ok(Some(-4)));
        assert_eq!((o.ema.price, o.ema.count, o.ring.len()), (WAD, 0, 1));
        assert_eq!((o.last.lo, o.last.sec), (2 * WAD, 5));
        assert!(coverage_gap(31, 1) && !coverage_gap(30, 1) && !coverage_gap(100, 0));
    }

    #[test]
    fn ring_wraps_newest_first() {
        let mut r = Ring::new(0, 1, 1).unwrap();
        for i in 1..=20u128 {
            r.push(RingEntry {
                p: i + 1,
                ts: i64::try_from(i).unwrap(),
                ..RingEntry::default()
            })
            .unwrap();
        }
        assert_eq!(r.len(), RING_LEN);
        let ts: std::vec::Vec<i64> = r.iter().map(|e| e.ts).collect();
        assert_eq!(ts, [20, 19, 18, 17, 16, 15, 14, 13]);
        let mut bad = r;
        bad.head = 8;
        assert_eq!(bad.check(), Err(MathError::InvalidInput));
        assert_eq!(sustained_growth(&bad, 3), Err(MathError::InvalidInput));
        assert_eq!(ring_medians(&bad), Err(MathError::InvalidInput));
        assert_eq!(ring_medians(&Ring::default()), Err(MathError::InvalidInput));
    }

    #[test]
    fn growth_and_medians() {
        let mut r = Ring::new(0, 100, 1).unwrap();
        for (ts, p) in [(1i64, 110u128), (2, 121), (4, 145)] {
            r.push(entry(ts, p, p, p, 1, 1)).unwrap();
        }
        // Steps: +1,000 bps/1 s, +1,000 bps/1 s, ⌊2,396/2⌋ = 991 bps/s.
        assert_eq!(sustained_growth(&r, 3).unwrap(), 991);
        assert_eq!(sustained_growth(&r, 4).unwrap(), 0);
        assert_eq!(sustained_growth(&r, 0).unwrap(), 0);
        // Lows 100, 110, 121, 145: lower median 110; highs alike: upper median 121.
        assert_eq!(ring_medians(&r).unwrap(), (110, 121, 1, 1));
        // Eight entries: 4th smallest low, 5th smallest high.
        let mut r = Ring::default();
        for i in 0..8u128 {
            let ts = i64::try_from(i).unwrap();
            r.push(entry(ts, 50, 10 + i, 20 + i, 30 - i, 40 - i))
                .unwrap();
        }
        assert_eq!(ring_medians(&r).unwrap(), (13, 24, 26, 37));
    }

    #[test]
    fn rewrite_liquidity_resets_every_entry() {
        let setup = || {
            let mut o = Observation::new(0, WAD, 120).unwrap();
            o.ema.liq = 100;
            o.ema.liq_slow = 50;
            o.ring
                .push(entry(1, 2 * WAD, WAD, 3 * WAD, 90, 130))
                .unwrap();
            o.ring
                .push(entry(2, 3 * WAD, 2 * WAD, 4 * WAD, 40, 45))
                .unwrap();
            o.record(5, WAD, 90).unwrap();
            o
        };
        let mut o = setup();
        o.rewrite_liquidity(200, VenueChange::Raydium).unwrap();
        assert_eq!((o.ema.price, o.ema.liq, o.ema.liq_slow), (WAD, 100, 50));
        assert!(
            o.ring
                .iter()
                .all(|e| (e.l, e.llo, e.lhi) == (100, 100, 100))
        );
        assert!(
            o.ring
                .iter()
                .all(|e| (e.x, e.y) == reprice(100, e.p).unwrap())
        );
        let ps: std::vec::Vec<_> = o.ring.iter().map(|e| (e.ts, e.p, e.lo, e.hi)).collect();
        assert_eq!(
            ps,
            [
                (2, 3 * WAD, 2 * WAD, 4 * WAD),
                (1, 2 * WAD, WAD, 3 * WAD),
                (0, WAD, WAD, WAD)
            ]
        );
        assert_eq!(o.bucket, Bucket::EMPTY);

        let mut o = setup();
        o.rewrite_liquidity(70, VenueChange::Raydium).unwrap();
        assert_eq!((o.ema.liq, o.ema.liq_slow), (70, 50));
        assert!(
            o.ring
                .iter()
                .all(|e| e.l == 70 && e.llo == 70 && e.lhi == 70)
        );

        let mut o = setup();
        o.rewrite_liquidity(200, VenueChange::Graduation).unwrap();
        assert_eq!((o.ema.liq, o.ema.liq_slow), (200, 200));
        assert!(
            o.ring
                .iter()
                .all(|e| e.l == 200 && e.llo == 200 && e.lhi == 200)
        );
        let before = o;
        assert_eq!(
            o.rewrite_liquidity(0, VenueChange::Graduation),
            Err(MathError::InvalidInput)
        );
        assert_eq!(o, before);
    }

    #[test]
    fn robust_spot_freshness() {
        let last = LastBucket {
            lo: 5,
            hi: 9,
            sec: 10,
            _pad: [0; 8],
        };
        assert_eq!(robust_spot(7, &last, 12, 2), Some(5));
        assert_eq!(robust_spot(3, &last, 12, 2), Some(3));
        assert_eq!(robust_spot(7, &last, 13, 2), None);
        assert_eq!(robust_spot(7, &LastBucket::NONE, 0, 2), None);
    }

    #[test]
    fn params_bounds() {
        assert_eq!(EmaParams::DEFAULT.validate(), Ok(()));
        for bad in [
            EmaParams::new(0, 10_000, 3_600, 60),
            EmaParams::new(3, 10_000, 3_600, 60),
            EmaParams::new(2, 4_999, 3_600, 60),
            EmaParams::new(2, 10_001, 3_600, 60),
            EmaParams::new(2, 10_000, 599, 60),
            EmaParams::new(2, 10_000, 86_401, 60),
            EmaParams::new(2, 10_000, 3_600, 0),
            EmaParams::new(2, 10_000, 3_600, 601),
        ] {
            assert_eq!(bad.validate(), Err(MathError::InvalidInput));
        }
        assert_eq!(EmaParams::new(1, 5_000, 600, 1).validate(), Ok(()));
    }

    #[test]
    fn huge_prices_do_not_overflow() {
        let p = u128::MAX / 3;
        let mut o = Observation::new(0, p, u128::MAX).unwrap();
        o.record(1, u128::MAX, u128::MAX).unwrap();
        o.roll(2, &EmaParams::DEFAULT).unwrap();
        assert!(o.ema.price > p);
        assert_eq!(o.ema.liq, u128::MAX);
        // No reprojection past u128: the entry caches (0, 0).
        let e = o.ring.newest().unwrap();
        assert_eq!((e.x, e.y), (0, 0));
    }
}
