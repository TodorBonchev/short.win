//! Per-market coverage state (ARCHITECTURE §7.1 duties 1, 2, 5, 8, 9; ECON §5.8): when the
//! market needs the keeper's next observation, deviation bursts, the per-second SLO with its
//! degraded mode, and the daily budget.
//!
//! - **Cadence** ([`MarketTrack::observation_due`]): every slot during a deviation burst
//!   (duty 2; never while an S3 on an executable venue is due, duty 5); once per second when
//!   the market needs it (duty 1: a near position or `|P_spot/P_ema − 1| > 2 %`; duty 5: S3 due
//!   on an executable venue); at once when the route is gone and no S3 chain runs yet, then
//!   every `no_route_interval_s` (the chain restarts after `S3_OBS_GAP_S`); every
//!   `idle_interval_s` with open positions.
//! - **Bursts** ([`MarketTrack::check_foreign`]): the keeper remembers the spot prices it read
//!   in each second; an observation in a bucket (the open one, or the last completed one) that
//!   deviates from that second's view by more than `burst_dev_bps` was not the keeper's — an
//!   atomic `[pump, crank, dump]` bundle leaves exactly that — and starts 60 s of per-slot
//!   observations.
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
    /// Own spot views: (second, lowest, highest) read in that second.
    views: VecDeque<(i64, u128, u128)>,
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
    /// Records the spot price the keeper read at `now`.
    pub fn record_view(&mut self, now: i64, price: Option<u128>) {
        let Some(p) = price else { return };
        match self.views.back_mut() {
            Some((s, lo, hi)) if *s == now => {
                *lo = (*lo).min(p);
                *hi = (*hi).max(p);
            }
            _ => self.views.push_back((now, p, p)),
        }
        while self
            .views
            .front()
            .is_some_and(|(s, _, _)| now.saturating_sub(*s) > VIEW_HISTORY_S)
        {
            self.views.pop_front();
        }
    }

    fn view_at(&self, sec: i64) -> Option<(u128, u128)> {
        self.views
            .iter()
            .find(|(s, _, _)| *s == sec)
            .map(|(_, lo, hi)| (*lo, *hi))
    }

    /// Whether bucket extremes `(lo, hi)` of second `sec` deviate from the keeper's own view
    /// of that second by more than `dev_bps` (`None`: no view of that second).
    pub fn deviates(&self, sec: i64, lo: u128, hi: u128, dev_bps: u32) -> Option<bool> {
        let (vlo, vhi) = self.view_at(sec)?;
        let d = u128::from(dev_bps);
        let below = lo.saturating_mul(10_000) < vlo.saturating_mul(10_000u128.saturating_sub(d));
        let above = hi.saturating_mul(10_000) > vhi.saturating_mul(10_000u128.saturating_add(d));
        Some(below || above)
    }

    /// Duty 2: checks the market's open bucket and last completed bucket for an observation
    /// the keeper did not make; a deviation starts (or extends) a burst of `burst_s` from
    /// that second. Returns the second of a newly detected deviation.
    pub fn check_foreign(&mut self, m: &Market, cov: &CoverageConfig) -> Option<i64> {
        let mut found = None;
        let buckets = [
            (m.b_sec, m.b_lo.get(), m.b_hi.get()),
            (m.lb_sec, m.lb_lo.get(), m.lb_hi.get()),
        ];
        for (sec, lo, hi) in buckets {
            if sec == 0 || lo == 0 || self.burst_seen.contains(&sec) {
                continue;
            }
            if self.deviates(sec, lo, hi, cov.burst_dev_bps) == Some(true) {
                self.burst_seen.push(sec);
                self.burst_until = self.burst_until.max(sec.saturating_add(cov.burst_s));
                found = Some(sec);
            }
        }
        if self.burst_seen.len() > 64 {
            self.burst_seen.drain(..32);
        }
        found
    }

    /// Whether a burst runs at `now`.
    pub fn in_burst(&self, now: i64) -> bool {
        now < self.burst_until
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
    }

    #[test]
    fn a_foreign_deviation_starts_a_per_slot_burst() {
        let c = cov();
        let mut t = MarketTrack::default();
        let p = 1_000_000u128;
        t.record_view(10, Some(p));
        t.record_view(10, Some(p + 5_000)); // +0.5 %: the keeper's own range
        let mut m = Market::zeroed();
        m.b_sec = 10;
        m.b_lo.set(p);
        m.b_hi.set(p + 5_000);
        assert_eq!(t.check_foreign(&m, &c), None);
        // Someone observed +3 % in second 10.
        m.b_hi.set(p * 103 / 100);
        assert_eq!(t.check_foreign(&m, &c), Some(10));
        assert_eq!(t.burst_until, 70);
        // Seen once: not re-detected when the bucket completes.
        m.lb_sec = 10;
        m.lb_lo.set(p);
        m.lb_hi.set(p * 103 / 100);
        m.b_sec = 11;
        m.b_lo.set(p);
        m.b_hi.set(p);
        t.record_view(11, Some(p));
        assert_eq!(t.check_foreign(&m, &c), None);
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
