//! Degraded mode (ARCH §7.1 duty 9; ECON §5.8): a keeper that cannot meet its coverage on a
//! market says so — the program's `CoverageGap` after an outage, or consecutive missed
//! seconds (an exhausted O17 budget) — and the alert lifts after 10 minutes of restored
//! coverage.

use shortwin_keeper::DegradedReason;
use shortwin_keeper::coverage::CoverageState;

use crate::common::*;

#[test]
fn an_outage_is_reported_through_coverage_gap_and_recovers_after_ten_minutes() {
    let (mut h, mms) = setup(1, KeeperConfig::default());
    let key = mms[0].market();
    let _ = open(&h, &mms[0], Q);
    let r = h.second().clone();
    assert!(r.alerts.is_empty(), "no gap after the open: {:?}", r.alerts);
    // The keeper is down for 45 s: the next integration follows a 46 s gap.
    h.idle(45);
    let r = h.second().clone();
    let gap = r.alerts.iter().find_map(|a| match a {
        Alert::Degraded {
            market,
            reason:
                DegradedReason::CoverageGap {
                    gap_s,
                    open_positions,
                },
        } if *market == key => Some((*gap_s, *open_positions)),
        _ => None,
    });
    let (gap_s, open) = gap.unwrap_or_else(|| panic!("{:?}", r.alerts));
    assert_eq!(gap_s, 46);
    assert_eq!(open, 1);
    assert!(h.keeper.track(&key).unwrap().state.is_degraded());
    assert_eq!(h.keeper.metrics.markets[&key].coverage_gaps, 1);
    // Ten minutes of met coverage lift it.
    let mut restored = None;
    for i in 0..610 {
        let r = h.second();
        if r.alerts
            .iter()
            .any(|a| matches!(a, Alert::CoverageRestored { market, .. } if *market == key))
        {
            restored = Some(i);
            break;
        }
    }
    assert!(restored.is_some_and(|i| i >= 599), "{restored:?}");
    assert_eq!(h.keeper.track(&key).unwrap().state, CoverageState::Healthy);
    // A second outage pages again.
    h.idle(40);
    let r = h.second().clone();
    assert!(
        r.alerts
            .iter()
            .any(|a| matches!(a, Alert::Degraded { market, .. } if *market == key)),
        "{:?}",
        r.alerts
    );
}

#[test]
fn an_exhausted_budget_misses_observations_and_degrades_the_market() {
    let mut cfg = KeeperConfig::default();
    // Two cranks' worth of fees per day.
    cfg.budget.per_market_daily_lamports = Some(10_000);
    let (mut h, mms) = setup(1, cfg);
    let key = mms[0].market();
    let _ = open(&h, &mms[0], Q);
    let mut alerts = Vec::new();
    for _ in 0..70 {
        let r = h.second();
        alerts.extend(r.alerts.clone());
    }
    assert!(
        alerts
            .iter()
            .any(|a| matches!(a, Alert::BudgetExhausted { market, .. } if *market == key)),
        "{alerts:?}"
    );
    assert!(
        alerts.iter().any(|a| matches!(a, Alert::Degraded { market, reason: DegradedReason::Misses(3) } if *market == key)),
        "{alerts:?}"
    );
    let m = &h.keeper.metrics.markets[&key];
    assert_eq!(m.cranks, 2);
    assert!(m.misses >= 3 && m.degraded);
    // Raising the budget restores coverage at once.
    h.keeper.config_mut().budget.per_market_daily_lamports = None;
    let r = h.second();
    assert!(r.cranked.contains(&key));
    h.second();
    assert!(matches!(
        h.keeper.track(&key).unwrap().state,
        CoverageState::Recovering { .. }
    ));
}
