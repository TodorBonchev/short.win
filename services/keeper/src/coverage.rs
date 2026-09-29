//! Per-market coverage state (ARCHITECTURE §7.1 duties 1, 2, 5, 8, 9; ECON §5.8): when the
//! market needs the keeper's next observation, deviation bursts, the per-second SLO with its
//! degraded mode, and the daily budget.
//!
//! - **Cadence** ([`MarketTrack::observation_due`]): every slot during a deviation burst
//!   (duty 2; never while an S3 on an executable venue is due, duty 5); once per second when
//!   the market needs it (duty 1: a near position or `|P_spot/P_ema − 1| > 2 %`; duty 5: S3 due
//!   on an executable venue, or the post-change warm-up of a market with open positions after a
//!   `PoolSwitched`, until `ema_warm == 0`, M4R, and after it for as long as the post-change
//!   reference guard is set, `ema_guard == 1`, M4F-1: every second then carries an honest bucket
//!   low, so attacker-only seconds cannot lift `L_mlo` and clear the guard; duty 2's liquidity
//!   trigger on a market with open positions, `liq_cover_s` from a foreign bucket liquidity,
//!   FITS-2 (b); `decide::MarketNeeds::liq_covered`); at once when the
//!   route is gone and no S3 chain runs yet, then every `no_route_interval_s` (the chain
//!   restarts after `S3_OBS_GAP_S`); every
//!   `idle_interval_s` with open positions; a Raydium market without positions every
//!   `listing_interval_s` inside its listing window and every second at its end until its
//!   post-change warm-up is over (M4V-5, M4R: the first open after the window reads an honest
//!   `P_ema`).
//! - **Bursts** ([`MarketTrack::check_foreign`]): the keeper remembers the spot prices it read
//!   in each second; an observation in a bucket (the open one, or the last completed one) that
//!   deviates from that second's view by more than `burst_dev_bps` was not the keeper's — an
//!   atomic `[pump, crank, dump]` bundle leaves exactly that — and starts 60 s of per-slot
//!   observations. Only **prices** start a burst: the per-slot liquidity bursts M4F-1 added stay
//!   reverted (M4G-2: an atomic `[deposit, crank_price, withdraw]` bought a 60-s per-slot burst,
//!   25–150× the attacker's cost in the keeper's).
//! - **Liquidity coverage** ([`MarketTrack::check_foreign`], FITS-2 (b), owner decision of
//!   2026-09-29): the keeper also remembers the venue depths `√(X·Y)` it read in each second. On
//!   a market **with open positions**, a bucket whose liquidity extremes `llo` / `lhi` (the open
//!   bucket's, or those of the ring entry that integrated the last completed one) deviate from
//!   that second's depth view by more than `burst_dev_bps` — an LP-wrapped
//!   `[withdraw, crank, deposit]` (the withdraw drag of `L_ema` / `L_slow`, FITV-2) or
//!   `[deposit, crank, withdraw]` (the freeze, M4F-1) — starts **per-second** (not per-slot)
//!   observations for `liq_cover_s` (10 s) from that second, extended by every recurrence. Every
//!   second of the window then carries an honest bucket extreme, so the attacker has no
//!   attacker-only second to drag or push the EMA with while it keeps wrapping, and pays one
//!   transaction per second of the keeper's coverage (1:1); an isolated wrap buys at most
//!   `liq_cover_s` observations. The coverage holds the EMA; it cannot lift a low the attacker
//!   reached genuinely — its LP withdrawn for real for a few seconds before it starts wrapping,
//!   nothing foreign then — which stays frozen at 1:1 (FITX-1, ECON §12.12 residual 10, owner
//!   item O18). A market without open positions never gets liquidity-triggered
//!   coverage (nothing there rests on the liquidity EMA). A venue change (graduation, a Raydium
//!   adoption) rewrites the ring's liquidity to the new pool's, so the depths viewed before it are
//!   dropped.
//! - **Degraded mode** ([`MarketTrack::close_second`], [`MarketTrack::on_coverage_gap`]): a
//!   second in which an observation was required and none landed is a miss;
//!   `degraded_after_misses` consecutive misses, or a `CoverageGap` event from the program (an
//!   integration after more than 30 s without one on a market with open positions), make the
//!   market degraded: the keeper raises [`crate::alerts::Alert::Degraded`] (page the Emergency
//!   multisig to pause opens with reason `KeeperCoverage`; `set_coverage_pause` itself waits
//!   for O17). The state lifts after `recovery_s` (10 min) of met coverage
//!   ([`crate::alerts::Alert::CoverageRestored`]).
//! - **Budget** ([`MarketTrack::can_spend`]): lamports spent on observations per UTC day
//!   against the O17 budget; an exhausted budget skips observations (visible as misses, hence
//!   degraded mode), never exits.

use std::collections::{HashMap, VecDeque};

use anchor_lang::prelude::Pubkey;
use shortwin::state::Market;

use crate::alerts::{Alert, DegradedReason};
use crate::config::CoverageConfig;
use crate::decide::MarketNeeds;

/// Seconds of own spot views kept for the burst check.
const VIEW_HISTORY_S: i64 = 120;

/// The keeper's own view of one second: the lowest and highest spot price and venue depth
/// `√(X·Y)` it read (the depth `None` when the venue gave none, or when it was read before a
/// venue change).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct View {
    sec: i64,
    lo: u128,
    hi: u128,
    liq: Option<(u128, u128)>,
}

/// A market's venue identity (`Market.venue` and the first pool-cache key): a change rewrites
/// the ring's liquidity (graduation, a Raydium adoption).
pub type VenueId = (u8, Pubkey);

/// What [`MarketTrack::check_foreign`] found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Foreign {
    /// The second of a newly detected foreign **price** (a burst started or extended; duty 2).
    pub price: Option<i64>,
    /// The second of a newly detected foreign **liquidity** on a market with open positions
    /// (liquidity coverage started or extended; FITS-2 (b)).
    pub liquidity: Option<i64>,
    /// The liquidity deviation started a new window (none ran at that second).
    pub liq_started: bool,
}

/// Why an observation is due now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Due {
    /// Deviation burst: every slot (duty 2).
    Slot,
    /// Once this second (duties 1, 5).
    Second,
    /// No route: the S3 chain cadence (duty 5).
    NoRoute,
    /// Open positions: the idle cadence (duty 1).
    Idle,
    /// A Raydium market without positions inside its listing window: the listing cadence
    /// (M4V-5).
    Listing,
}

/// The degraded-mode state machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CoverageState {
    #[default]
    Healthy,
    /// Coverage cannot be met (opens should pause, reason `KeeperCoverage`).
    Degraded { since: i64 },
    /// Coverage met again since `since`; lifts after `recovery_s`.
    Recovering { degraded_since: i64, since: i64 },
}

impl CoverageState {
    /// Degraded or recovering (the pause is still on).
    pub fn is_degraded(&self) -> bool {
        !matches!(self, Self::Healthy)
    }
}

/// One market's coverage bookkeeping (module docs).
#[derive(Clone, Debug, Default)]
pub struct MarketTrack {
    /// The keeper's last landed observation (second, slot).
    pub last_obs: Option<(i64, u64)>,
    /// A deviation burst runs while `now < burst_until`.
    pub burst_until: i64,
    /// Seconds whose foreign deviation already started a burst.
    burst_seen: Vec<i64>,
    /// Liquidity-triggered per-second coverage runs while `now < liq_until` (FITS-2 (b)).
    pub liq_until: i64,
    /// Seconds whose foreign liquidity already started or extended the coverage.
    liq_seen: Vec<i64>,
    /// Own views per second (price and depth ranges read in that second).
    views: VecDeque<View>,
    /// The venue the depth views were read on.
    venue: Option<VenueId>,
    pub state: CoverageState,
    /// Consecutive seconds with a required observation and none landed.
    pub miss_streak: u32,
    /// The second being accumulated: (second, required, landed).
    current: Option<(i64, bool, bool)>,
    /// Last second a `sol_settle` was sent for a position (at most one per second).
    pub settle_sent: HashMap<Pubkey, i64>,
    /// Last S3 probe (simulated `sol_settle`) of a position.
    pub s3_probe: HashMap<Pubkey, i64>,
    /// UTC day and lamports spent on observations that day.
    pub spend_day: i64,
    pub spent_today: u64,
    budget_alerted_day: Option<i64>,
}

impl MarketTrack {
    /// Records the spot price and the venue depth `√(X·Y)` the keeper read at `now` on the venue
    /// `venue` ([`VenueId`]); a new venue drops the depths viewed on the previous one.
    pub fn record_view(
        &mut self,
        now: i64,
        price: Option<u128>,
        liquidity: Option<u128>,
        venue: VenueId,
    ) {
        if self.venue != Some(venue) {
            if self.venue.is_some() {
                for v in &mut self.views {
                    v.liq = None;
                }
            }
            self.venue = Some(venue);
        }
        let Some(p) = price else { return };
        match self.views.back_mut() {
            Some(v) if v.sec == now => {
                v.lo = v.lo.min(p);
                v.hi = v.hi.max(p);
                v.liq = match (v.liq, liquidity) {
                    (Some((lo, hi)), Some(l)) => Some((lo.min(l), hi.max(l))),
                    (Some(r), None) => Some(r),
                    (None, l) => l.map(|l| (l, l)),
                };
            }
            _ => self.views.push_back(View {
                sec: now,
                lo: p,
                hi: p,
                liq: liquidity.map(|l| (l, l)),
            }),
        }
        while self
            .views
            .front()
            .is_some_and(|v| now.saturating_sub(v.sec) > VIEW_HISTORY_S)
        {
            self.views.pop_front();
        }
    }

    fn view_at(&self, sec: i64) -> Option<&View> {
        self.views.iter().find(|v| v.sec == sec)
    }

    /// Whether bucket extremes `(lo, hi)` of second `sec` deviate from the keeper's own view
    /// of that second by more than `dev_bps` (`None`: no view of that second).
    pub fn deviates(&self, sec: i64, lo: u128, hi: u128, dev_bps: u32) -> Option<bool> {
        let v = self.view_at(sec)?;
        Some(outside(lo, hi, v.lo, v.hi, dev_bps))
    }

    /// Whether bucket liquidity extremes `(llo, lhi)` of second `sec` deviate from the depth
    /// `√(X·Y)` the keeper read in that second by more than `dev_bps` (`None`: no depth view of
    /// that second on the current venue).
    pub fn liq_deviates(&self, sec: i64, llo: u128, lhi: u128, dev_bps: u32) -> Option<bool> {
        let (vlo, vhi) = self.view_at(sec)?.liq?;
        Some(outside(llo, lhi, vlo, vhi, dev_bps))
    }

    /// Duty 2: checks the market's open bucket and last completed bucket for an observation
    /// the keeper did not make — a price, or (FITS-2 (b)) a liquidity, off the keeper's own view
    /// of that second by more than `burst_dev_bps`. A foreign price starts (or extends) a
    /// per-slot burst of `burst_s` from that second. A foreign liquidity starts (or extends)
    /// per-second coverage of `liq_cover_s` from that second, only when `open` (the market has
    /// open positions); without positions it is not even recorded. The last completed bucket's
    /// liquidity extremes are those of the ring entry that integrated it (the newest entry,
    /// pushed after that second with the same price extremes).
    pub fn check_foreign(&mut self, m: &Market, cov: &CoverageConfig, open: bool) -> Foreign {
        let mut found = Foreign::default();
        let (lb_lo, lb_hi) = (m.lb_lo.get(), m.lb_hi.get());
        let lb_liq = m
            .observation()
            .ring
            .newest()
            .filter(|e| e.ts > m.lb_sec && e.lo == lb_lo && e.hi == lb_hi)
            .map(|e| (e.llo, e.lhi));
        let buckets = [
            (
                m.b_sec,
                m.b_lo.get(),
                m.b_hi.get(),
                Some((m.b_llo.get(), m.b_lhi.get())),
            ),
            (m.lb_sec, lb_lo, lb_hi, lb_liq),
        ];
        let dev = cov.burst_dev_bps;
        for (sec, lo, hi, liq) in buckets {
            if sec == 0 || lo == 0 {
                continue;
            }
            if !self.burst_seen.contains(&sec) && self.deviates(sec, lo, hi, dev) == Some(true) {
                self.burst_seen.push(sec);
                self.burst_until = self.burst_until.max(sec.saturating_add(cov.burst_s));
                found.price = Some(sec);
            }
            if !open || cov.liq_cover_s <= 0 || self.liq_seen.contains(&sec) {
                continue;
            }
            let depth = liq
                .filter(|(llo, _)| *llo != 0)
                .and_then(|(llo, lhi)| self.liq_deviates(sec, llo, lhi, dev));
            if depth == Some(true) {
                self.liq_seen.push(sec);
                found.liq_started |= self.liq_until <= sec;
                self.liq_until = self.liq_until.max(sec.saturating_add(cov.liq_cover_s));
                found.liquidity = Some(sec);
            }
        }
        for seen in [&mut self.burst_seen, &mut self.liq_seen] {
            if seen.len() > 64 {
                seen.drain(..32);
            }
        }
        found
    }

    /// Whether a burst runs at `now`.
    pub fn in_burst(&self, now: i64) -> bool {
        now < self.burst_until
    }

    /// Whether liquidity-triggered per-second coverage runs at `now` (FITS-2 (b)).
    pub fn liq_covered(&self, now: i64) -> bool {
        now < self.liq_until
    }

    /// Whether an observation is due at (`now`, `slot`) given the market's needs, and why.
    pub fn observation_due(
        &self,
        needs: &MarketNeeds,
        now: i64,
        slot: u64,
        cov: &CoverageConfig,
    ) -> Option<Due> {
        let since = |interval: i64| {
            self.last_obs
                .is_none_or(|(t, _)| now.saturating_sub(t) >= interval)
        };
        let this_second = self.last_obs.is_none_or(|(t, _)| t < now);
        if self.in_burst(now) && !needs.s3_due_executable {
            // Every slot, with or without positions: the burst's own point (duty 2).
            return self
                .last_obs
                .is_none_or(|(_, s)| s < slot)
                .then_some(Due::Slot);
        }
        if needs.per_second && this_second {
            return Some(Due::Second);
        }
        if needs.start_chain && this_second {
            return Some(Due::NoRoute);
        }
        if needs.no_route && since(cov.no_route_interval_s) {
            return Some(Due::NoRoute);
        }
        if needs.open_positions && since(cov.idle_interval_s) {
            return Some(Due::Idle);
        }
        if needs.warm_up && this_second {
            return Some(Due::Second);
        }
        if needs.listing && since(cov.listing_interval_s) {
            return Some(Due::Listing);
        }
        None
    }

    /// Records a landed observation (a crank, or an exit that carries the ReadSet).
    pub fn landed(&mut self, now: i64, slot: u64) {
        self.last_obs = Some((now, slot));
        self.note(now, false, true);
    }

    /// Records that an observation was required at `now`.
    pub fn required(&mut self, now: i64) {
        self.note(now, true, false);
    }

    fn note(&mut self, now: i64, required: bool, landed: bool) {
        match &mut self.current {
            Some((s, r, l)) if *s == now => {
                *r |= required;
                *l |= landed;
            }
            _ => self.current = Some((now, required, landed)),
        }
    }

    /// Closes every second before `now`: a second that required an observation and got none
    /// is a miss; misses make the market degraded, met seconds recover it (module docs).
    /// Alerts go to `alerts`.
    pub fn close_second(
        &mut self,
        market: Pubkey,
        now: i64,
        cov: &CoverageConfig,
        alerts: &mut Vec<Alert>,
    ) -> Option<bool> {
        let (sec, required, landed) = self.current?;
        if sec >= now {
            return None;
        }
        self.current = None;
        let missed = required && !landed;
        if missed {
            self.miss_streak = self.miss_streak.saturating_add(1);
            if self.miss_streak >= cov.degraded_after_misses.max(1) {
                self.degrade(
                    market,
                    sec,
                    DegradedReason::Misses(self.miss_streak),
                    alerts,
                );
            }
        } else {
            self.miss_streak = 0;
            self.recover(market, sec, cov, alerts);
        }
        Some(missed)
    }

    /// The program reported a `CoverageGap` on this market (A69): degraded.
    pub fn on_coverage_gap(
        &mut self,
        market: Pubkey,
        now: i64,
        gap_s: i64,
        open_positions: u32,
        alerts: &mut Vec<Alert>,
    ) {
        let before = self.state;
        self.degrade(
            market,
            now,
            DegradedReason::CoverageGap {
                gap_s,
                open_positions,
            },
            alerts,
        );
        // The gap is in the past and this integration ended it: unless misses keep the
        // market degraded, coverage is met again from this second (the recovery clock
        // restarts).
        self.state = match before {
            CoverageState::Degraded { since } => CoverageState::Degraded { since },
            CoverageState::Healthy => CoverageState::Recovering {
                degraded_since: now,
                since: now,
            },
            CoverageState::Recovering { degraded_since, .. } => CoverageState::Recovering {
                degraded_since,
                since: now,
            },
        };
    }

    fn degrade(
        &mut self,
        market: Pubkey,
        now: i64,
        reason: DegradedReason,
        alerts: &mut Vec<Alert>,
    ) {
        match self.state {
            CoverageState::Degraded { .. } => {}
            CoverageState::Healthy => {
                self.state = CoverageState::Degraded { since: now };
                alerts.push(Alert::Degraded { market, reason });
            }
            CoverageState::Recovering { degraded_since, .. } => {
                // Still paused: back to degraded without a new page.
                self.state = CoverageState::Degraded {
                    since: degraded_since,
                };
            }
        }
    }

    fn recover(&mut self, market: Pubkey, now: i64, cov: &CoverageConfig, alerts: &mut Vec<Alert>) {
        match self.state {
            CoverageState::Healthy => {}
            CoverageState::Degraded { since } => {
                self.state = CoverageState::Recovering {
                    degraded_since: since,
                    since: now,
                };
            }
            CoverageState::Recovering {
                degraded_since,
                since,
            } => {
                if now.saturating_sub(since) >= cov.recovery_s {
                    self.state = CoverageState::Healthy;
                    alerts.push(Alert::CoverageRestored {
                        market,
                        degraded_s: now.saturating_sub(degraded_since),
                    });
                }
            }
        }
    }

    /// Whether the O17 budget allows another observation today (raises
    /// [`Alert::BudgetExhausted`] once per day when it does not).
    pub fn can_spend(
        &mut self,
        market: Pubkey,
        now: i64,
        budget: Option<u64>,
        alerts: &mut Vec<Alert>,
    ) -> bool {
        self.roll_day(now);
        let Some(b) = budget else { return true };
        if self.spent_today < b {
            return true;
        }
        if self.budget_alerted_day != Some(self.spend_day) {
            self.budget_alerted_day = Some(self.spend_day);
            alerts.push(Alert::BudgetExhausted {
                market,
                spent: self.spent_today,
                budget: b,
            });
        }
        false
    }

    /// Adds `lamports` to today's observation spend.
    pub fn spend(&mut self, now: i64, lamports: u64) {
        self.roll_day(now);
        self.spent_today = self.spent_today.saturating_add(lamports);
    }

    fn roll_day(&mut self, now: i64) {
        let day = now.div_euclid(86_400);
        if day != self.spend_day {
            self.spend_day = day;
            self.spent_today = 0;
        }
    }
}

/// Whether `[lo, hi]` reaches more than `dev_bps` below `vlo` or above `vhi`.
fn outside(lo: u128, hi: u128, vlo: u128, vhi: u128, dev_bps: u32) -> bool {
    let d = u128::from(dev_bps);
    let below = lo.saturating_mul(10_000) < vlo.saturating_mul(10_000u128.saturating_sub(d));
    let above = hi.saturating_mul(10_000) > vhi.saturating_mul(10_000u128.saturating_add(d));
    below || above
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;

    fn cov() -> CoverageConfig {
        CoverageConfig::default()
    }

    #[test]
    fn cadence_follows_the_needs() {
        let c = cov();
        let mut t = MarketTrack::default();
        let idle = MarketNeeds {
            open_positions: true,
            ..MarketNeeds::default()
        };
        // Nothing landed yet: due at once.
        assert_eq!(t.observation_due(&idle, 100, 250, &c), Some(Due::Idle));
        t.landed(100, 250);
        assert_eq!(t.observation_due(&idle, 129, 322, &c), None);
        assert_eq!(t.observation_due(&idle, 130, 325, &c), Some(Due::Idle));
        let near = MarketNeeds {
            per_second: true,
            open_positions: true,
            ..MarketNeeds::default()
        };
        assert_eq!(t.observation_due(&near, 100, 251, &c), None);
        assert_eq!(t.observation_due(&near, 101, 252, &c), Some(Due::Second));
        let no_route = MarketNeeds {
            no_route: true,
            open_positions: true,
            ..MarketNeeds::default()
        };
        assert_eq!(
            t.observation_due(&no_route, 130, 325, &c),
            Some(Due::NoRoute)
        );
        // The route just went away and no chain runs on chain yet: observe at once.
        let start = MarketNeeds {
            no_route: true,
            start_chain: true,
            open_positions: true,
            ..MarketNeeds::default()
        };
        assert_eq!(t.observation_due(&start, 101, 252, &c), Some(Due::NoRoute));
        assert_eq!(t.observation_due(&start, 100, 252, &c), None);
        // No positions, no route, no burst: nothing.
        assert_eq!(
            t.observation_due(&MarketNeeds::default(), 1_000, 9_000, &c),
            None
        );
        // M4V-5: a Raydium market without positions in its listing window, on the listing
        // cadence; at the window's end (and until its price state warmed up) every second.
        let listing = MarketNeeds {
            listing: true,
            ..MarketNeeds::default()
        };
        let mut t = MarketTrack::default();
        assert_eq!(
            t.observation_due(&listing, 1_000, 9_000, &c),
            Some(Due::Listing)
        );
        t.landed(1_000, 9_000);
        assert_eq!(t.observation_due(&listing, 1_059, 9_100, &c), None);
        assert_eq!(
            t.observation_due(&listing, 1_060, 9_200, &c),
            Some(Due::Listing)
        );
        let warm_up = MarketNeeds {
            listing: true,
            warm_up: true,
            ..MarketNeeds::default()
        };
        assert_eq!(t.observation_due(&warm_up, 1_000, 9_001, &c), None);
        assert_eq!(
            t.observation_due(&warm_up, 1_001, 9_002, &c),
            Some(Due::Second)
        );
    }

    const VENUE: VenueId = (1, Pubkey::new_from_array([7; 32]));

    /// The price found by [`MarketTrack::check_foreign`] (a market with open positions).
    fn price_of(t: &mut MarketTrack, m: &Market, c: &CoverageConfig) -> Option<i64> {
        t.check_foreign(m, c, true).price
    }

    #[test]
    fn a_foreign_deviation_starts_a_per_slot_burst() {
        let c = cov();
        let mut t = MarketTrack::default();
        let p = 1_000_000u128;
        t.record_view(10, Some(p), None, VENUE);
        t.record_view(10, Some(p + 5_000), None, VENUE); // +0.5 %: the keeper's own range
        let mut m = Market::zeroed();
        m.b_sec = 10;
        m.b_lo.set(p);
        m.b_hi.set(p + 5_000);
        assert_eq!(price_of(&mut t, &m, &c), None);
        // Someone observed +3 % in second 10.
        m.b_hi.set(p * 103 / 100);
        assert_eq!(price_of(&mut t, &m, &c), Some(10));
        assert_eq!(t.burst_until, 70);
        // Seen once: not re-detected when the bucket completes.
        m.lb_sec = 10;
        m.lb_lo.set(p);
        m.lb_hi.set(p * 103 / 100);
        m.b_sec = 11;
        m.b_lo.set(p);
        m.b_hi.set(p);
        t.record_view(11, Some(p), None, VENUE);
        assert_eq!(price_of(&mut t, &m, &c), None);
        let needs = MarketNeeds::default();
        t.landed(11, 30);
        assert_eq!(t.observation_due(&needs, 11, 30, &c), None);
        assert_eq!(t.observation_due(&needs, 11, 31, &c), Some(Due::Slot));
        assert_eq!(t.observation_due(&needs, 70, 200, &c), None);
        // An S3 on an executable venue suppresses the per-slot burst (duty 5).
        let s3 = MarketNeeds {
            per_second: true,
            s3_due_executable: true,
            ..MarketNeeds::default()
        };
        assert_eq!(t.observation_due(&s3, 11, 31, &c), None);
        // Without an own view of a second, nothing is concluded.
        assert_eq!(t.deviates(500, 1, 1, 200), None);
    }

    /// M4G-2 and FITS-2 (b): an atomic `[deposit, crank_price, withdraw]` or `[withdraw,
    /// crank_price, deposit]` leaves the bucket's price at the keeper's own view and its liquidity
    /// extremes off the pool's `√(X·Y)` — in the open bucket, or in the ring entry that integrated
    /// the last completed one. It starts **no** per-slot burst (only a foreign price does); on a
    /// market with open positions it starts `liq_cover_s` (10 s) of **per-second** coverage from
    /// its second, extended by each recurrence; on a market without positions nothing. Honest
    /// depths start nothing, and a venue change drops the depths viewed before it.
    #[test]
    fn a_foreign_liquidity_starts_per_second_coverage_only_with_positions() {
        use shortwin::state::RingEntry as AccRingEntry;
        let c = cov();
        let (p, h) = (1_000_000u128, 5_000_000u128);
        let open_bucket = |sec: i64, llo: u128, lhi: u128| {
            let mut m = Market::zeroed();
            m.b_sec = sec;
            m.b_lo.set(p);
            m.b_hi.set(p);
            m.b_llo.set(llo);
            m.b_lhi.set(lhi);
            m
        };
        // Without open positions: nothing, not even recorded.
        let mut t = MarketTrack::default();
        t.record_view(20, Some(p), Some(h), VENUE);
        for (llo, lhi) in [(8 * h, 8 * h), (h / 2, h)] {
            let f = t.check_foreign(&open_bucket(20, llo, lhi), &c, false);
            assert_eq!(f, Foreign::default());
        }
        assert!(!t.liq_covered(20) && t.burst_until == 0);
        // With positions: honest depths (within 2 %) start nothing.
        let f = t.check_foreign(&open_bucket(20, h, h + h / 100), &c, true);
        assert_eq!(f, Foreign::default());
        // A withdrawal-wrapped crank (the lows pulled 50 % under the depth): 10 s per second.
        let f = t.check_foreign(&open_bucket(20, h / 2, h), &c, true);
        assert_eq!(
            f,
            Foreign {
                price: None,
                liquidity: Some(20),
                liq_started: true
            }
        );
        assert_eq!((t.liq_until, t.burst_until), (30, 0), "no per-slot burst");
        // Seen once: the same second does not extend it again.
        assert_eq!(
            t.check_foreign(&open_bucket(20, h / 2, h), &c, true),
            Foreign::default()
        );
        let needs = |liq: bool| MarketNeeds {
            open_positions: true,
            per_second: liq,
            liq_covered: liq,
            ..MarketNeeds::default()
        };
        t.landed(20, 50);
        // Per second, not per slot: nothing more in second 20, one observation in 21.
        assert_eq!(
            t.observation_due(&needs(t.liq_covered(20)), 20, 51, &c),
            None
        );
        assert_eq!(
            t.observation_due(&needs(t.liq_covered(21)), 21, 52, &c),
            Some(Due::Second)
        );
        // A recurrence in second 25 (a deposit-wrapped crank, 7× the pool) extends it to 35.
        t.record_view(25, Some(p), Some(h), VENUE);
        let f = t.check_foreign(&open_bucket(25, h, 8 * h), &c, true);
        assert_eq!((f.liquidity, f.liq_started), (Some(25), false));
        assert_eq!(t.liq_until, 35);
        assert!(t.liq_covered(34) && !t.liq_covered(35));
        // The last completed bucket: its integrating ring entry carries the pushed lows.
        t.record_view(40, Some(p), Some(h), VENUE);
        t.record_view(41, Some(p), Some(h), VENUE);
        let mut m = Market::zeroed();
        m.lb_sec = 40;
        m.lb_lo.set(p);
        m.lb_hi.set(p);
        m.b_sec = 41;
        m.ring_count = 1;
        let mut e = AccRingEntry {
            ts: 41,
            ..AccRingEntry::default()
        };
        e.lo.set(p);
        e.hi.set(p);
        e.llo.set(h / 3);
        e.lhi.set(h);
        m.ring[0] = e;
        let f = t.check_foreign(&m, &c, true);
        assert_eq!(
            (f.price, f.liquidity, f.liq_started),
            (None, Some(40), true)
        );
        assert_eq!(t.liq_until, 50);
        // A ring entry that did not integrate that bucket (other price extremes) says nothing.
        t.record_view(43, Some(p), Some(h), VENUE);
        m.lb_sec = 43;
        m.ring[0].ts = 44;
        m.ring[0].lo.set(p + 1);
        assert_eq!(t.check_foreign(&m, &c, true), Foreign::default());
        // A price off the view in the same bucket still starts a per-slot burst.
        let mut m = open_bucket(41, h, h);
        m.b_hi.set(p * 103 / 100);
        assert_eq!(price_of(&mut t, &m, &c), Some(41));
        // A venue change (an adoption rewrote the ring's liquidity): depths read on the old
        // venue are not compared any more; the new venue's are.
        let mut t = MarketTrack::default();
        t.record_view(60, Some(p), Some(h), VENUE);
        t.record_view(
            61,
            Some(p),
            Some(3 * h),
            (2, Pubkey::new_from_array([9; 32])),
        );
        let f = t.check_foreign(&open_bucket(60, 3 * h, 3 * h), &c, true);
        assert_eq!(f, Foreign::default());
        let f = t.check_foreign(&open_bucket(61, h, 3 * h), &c, true);
        assert_eq!(f.liquidity, Some(61));
        // `liq_cover_s = 0` disables the trigger.
        let off = CoverageConfig {
            liq_cover_s: 0,
            ..cov()
        };
        let mut t = MarketTrack::default();
        t.record_view(70, Some(p), Some(h), VENUE);
        let f = t.check_foreign(&open_bucket(70, h / 2, h), &off, true);
        assert_eq!(f, Foreign::default());
    }

    #[test]
    fn misses_degrade_and_ten_minutes_of_coverage_restore() {
        let c = cov();
        let mk = Pubkey::new_unique();
        let mut t = MarketTrack::default();
        let mut alerts = Vec::new();
        for s in 0..3 {
            t.required(s);
            t.close_second(mk, s + 1, &c, &mut alerts);
        }
        assert!(matches!(t.state, CoverageState::Degraded { since: 2 }));
        assert!(matches!(
            alerts.as_slice(),
            [Alert::Degraded {
                reason: DegradedReason::Misses(3),
                ..
            }]
        ));
        alerts.clear();
        for s in 3..(3 + 601) {
            t.required(s);
            t.landed(s, 0);
            t.close_second(mk, s + 1, &c, &mut alerts);
        }
        assert_eq!(t.state, CoverageState::Healthy);
        assert!(matches!(
            alerts.as_slice(),
            [Alert::CoverageRestored { .. }]
        ));
    }

    #[test]
    fn the_budget_is_per_utc_day() {
        let mk = Pubkey::new_unique();
        let mut t = MarketTrack::default();
        let mut alerts = Vec::new();
        assert!(t.can_spend(mk, 10, Some(10_000), &mut alerts));
        t.spend(10, 10_000);
        assert!(!t.can_spend(mk, 11, Some(10_000), &mut alerts));
        assert!(!t.can_spend(mk, 12, Some(10_000), &mut alerts));
        assert_eq!(alerts.len(), 1);
        assert!(t.can_spend(mk, 86_400, Some(10_000), &mut alerts));
        assert!(t.can_spend(mk, 86_401, None, &mut alerts));
    }
}
