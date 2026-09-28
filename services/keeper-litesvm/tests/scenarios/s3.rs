//! Duty 5 and S3 (ARCH §7.1, §3.8 step 4, §5.3): the mock pool is disabled by its admin (a
//! halted venue: not executable, no observations, the S3 chain runs). The keeper starts the chain
//! at once and keeps it continuous with a crank at least every 30 s; once the S3 timer has run, an expired position
//! is settled in SOL (`sol_settle`, S3), and a healthy one stays frozen.

use shortwin::events::SolSettle;
use shortwin_keeper::events;

use crate::common::*;

#[test]
fn an_expired_position_on_a_disabled_pool_settles_in_s3_and_a_healthy_one_stays() {
    let (mut h, mms) = setup(1, KeeperConfig::default());
    let mm = &mms[0];
    let key = mm.market();
    let (b1, n1) = open(&h, mm, Q);
    let pos1 = position_key(mm, &b1.key(), n1);
    let deadline = h.with_env(|env| env.position(mm, &b1.key(), n1).deadline);
    h.seconds(2);
    // A second position, opened later: still healthy and far from its deadline at the end.
    h.idle(3_600);
    h.seconds(2);
    let (b2, n2) = open(&h, mm, Q / 3);
    h.seconds(2);

    // The keeper is idle until 700 s before the first deadline; then the pool is halted.
    let wait = deadline - 700 - h.now();
    h.idle(wait);
    h.seconds(5);
    h.with_env(|env| mock::update_knobs(env, &mm.pool, |k| k.disabled = true));
    let halted_at = h.now();

    let mut since = None;
    let mut last_crank = halted_at;
    let mut settled = None;
    for s in 0..1_000 {
        let r = h.second().clone();
        let now = h.now();
        assert!(r.errors.is_empty(), "second {s}: {:?}", r.errors);
        if r.cranked.contains(&key) {
            assert!(now - last_crank <= 30, "crank gap {} s", now - last_crank);
            last_crank = now;
        }
        let m = h.with_env(|env| env.market(&key));
        if m.no_route_since != 0 {
            // One continuous chain: it never restarts.
            assert_eq!(*since.get_or_insert(m.no_route_since), m.no_route_since);
        }
        if let Some(out) = landed_for(&r, ExitKind::SolSettle, &pos1).next() {
            let ev: Vec<SolSettle> = events::of(out);
            if let Some(e) = ev.first() {
                assert_eq!(e.condition, 3, "S3");
                assert_eq!(e.keeper, h.keeper.payer());
                settled = Some(now);
                break;
            }
        }
    }
    let since = since.expect("the S3 chain never started");
    // The keeper sees the outage at its next tick and observes at once: the chain starts then.
    assert_eq!(since, halted_at + 1);
    let settled = settled.expect("no S3 settlement");
    println!("halted at {halted_at}, chain since {since}, deadline {deadline}, S3 at {settled}");
    // Due once the position is expired and the chain has run s3_wait_s (600 s).
    assert!(settled >= deadline && settled >= since + 600, "{settled}");
    assert!(settled <= deadline.max(since + 600) + 1, "late: {settled}");
    assert!(!position_open(&h, mm, &b1.key(), n1));
    // The healthy position is frozen, not settled.
    assert!(position_open(&h, mm, &b2.key(), n2));
    assert_eq!(h.keeper.metrics.markets[&key].settlements, 1);
}
