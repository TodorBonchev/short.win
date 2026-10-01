//! Duty 11 (ARCH §7.1, §4.7, §3.9; C1-12; M7 K3): `crank_sol_usd` every ≤ 30 s — the USD tiers read
//! the cached Pyth SOL/USD price, and a cache older than `oracle_stale_s` (3,600 s) puts every market
//! in Tier C. The recorded mainnet `PriceUpdateV2` (`tests/fixtures`) is planted at the configured
//! address with fresh publish times:
//!
//! - the keeper cranks the first update at its first tick, then only newer ones — each within
//!   `oracle.interval_s` (30 s) of its publication — and reads nothing but the feed in between;
//! - an update the program's own rules refuse (a step above `oracle_max_step_bps` against a fresh
//!   cache) is not sent: `Alert::OracleRefused` once, the cache kept; an acceptable one after it
//!   lands and clears the alert;
//! - a feed that stops updating: `Alert::OracleAging` once the cache is older than
//!   `aging_alert_s` (hourly), and the market's tier falls to C at `oracle_stale_s` (the
//!   program's rule, unchanged);
//! - the keeper's crank is a global cost line (`oracle_lamports`), never a market's budget.

use shortwin::constants::PYTH_SOL_USD_ACCOUNT;
use shortwin_venue_state::pyth::{self, PriceUpdate};
use solana_account::Account;

use crate::common::*;

/// The mainnet dump (`tests/fixtures/accounts/pyth_sol_usd_shard0.bin`).
const DUMP: &[u8] = include_bytes!("../../../../tests/fixtures/accounts/pyth_sol_usd_shard0.bin");

pub(crate) fn dump() -> PriceUpdate {
    pyth::read(&pyth::RECEIVER, DUMP).expect("the dump decodes")
}

/// Writes `u` at the configured feed address (the receiver's layout, owner the receiver).
pub(crate) fn plant(env: &mut Env, u: &PriceUpdate) {
    let data = pyth::encode(u).to_vec();
    let lamports = env.svm.minimum_balance_for_rent_exemption(data.len());
    env.svm
        .set_account(
            PYTH_SOL_USD_ACCOUNT,
            Account {
                lamports,
                data,
                owner: shortwin::constants::PYTH_RECEIVER_ID,
                executable: false,
                rent_epoch: 0,
            },
        )
        .expect("feed");
}

/// The dump republished at `publish_time` with `price`.
pub(crate) fn update(publish_time: i64, price: i64) -> PriceUpdate {
    let u = dump();
    PriceUpdate {
        price,
        publish_time,
        prev_publish_time: publish_time - 1,
        ..u
    }
}

fn cranked(r: &TickReport) -> Option<i64> {
    r.oracle
        .as_ref()
        .filter(|(_, x)| x.landed().is_some())
        .map(|(t, _)| *t)
}

/// A mock market deep enough for Tier A (10,000 SOL a side: `Λ` ≈ 20,000 SOL, far above θ_A =
/// $500 k at the recorded price) and listed 8 days ago (age ≥ 7 d), with one position.
fn aged_deep_market() -> (Harness, MockMarket) {
    let mut env = Env::test_venue();
    let mm = env.mock_market(MarketSpec {
        base: 10_000_000_000_000_000,
        quote: 10_000_000_000_000,
        lend: 1_000_000_000_000_000,
        warmup: 8 * 86_400,
        ..MarketSpec::default()
    });
    let h = Harness::new(env, KeeperConfig::default());
    let _ = open(&h, &mm, Q);
    (h, mm)
}

#[test]
fn the_keeper_keeps_the_sol_usd_cache_fresh() {
    let (mut h, mm) = aged_deep_market();
    let key = mm.market();
    let p0 = dump().price;
    let now = h.now();
    h.with_env(|env| plant(env, &update(now - 5, p0)));
    // The harness turns duty 11 off where no feed is planted (`Harness::new`); on here.
    h.keeper.config_mut().oracle.enabled = true;
    let r = h.second().clone();
    assert_eq!(cranked(&r), Some(now - 5), "{:?}", r.oracle);
    let cache = h.with_env(|env| env.oracle_cache());
    assert_eq!((cache.price, cache.publish_time), (p0, now - 5));
    // Nothing newer: reads only.
    let mut sent = 0;
    for _ in 0..90 {
        let r = h.second().clone();
        sent += usize::from(r.oracle.is_some());
    }
    assert_eq!(sent, 0);
    assert!(
        (3..=4).contains(&h.keeper.metrics.oracle_reads),
        "{}",
        h.keeper.metrics.oracle_reads
    );
    // The feed publishes every 55 s (the sponsored heartbeat): each update is cached within 30 s.
    let mut lags = Vec::new();
    let mut next_pub = h.now() + 1;
    let mut published: Vec<i64> = Vec::new();
    for _ in 0..330 {
        if h.now() + 1 >= next_pub {
            let t = h.now() + 1;
            let price = p0 + (published.len() as i64 + 1) * p0 / 1_000;
            h.with_env(|env| plant(env, &update(t, price)));
            published.push(t);
            next_pub = t + 55;
        }
        let r = h.second().clone();
        if let Some(t) = cranked(&r) {
            lags.push(h.now() - t);
        }
    }
    assert_eq!(lags.len(), published.len(), "{lags:?} {published:?}");
    assert!(lags.iter().all(|l| (0..=30).contains(l)), "{lags:?}");
    println!("duty 11: {} updates cached, lags {lags:?} s", lags.len());
    let last = *published.last().unwrap();
    assert_eq!(h.with_env(|env| env.oracle_cache()).publish_time, last);

    // A 50 % step against the fresh cache: refused by the program's rules — not sent, one alert.
    let t = h.now() + 1;
    h.with_env(|env| plant(env, &update(t, p0 * 3 / 2)));
    let mut refused = Vec::new();
    for _ in 0..60 {
        let r = h.second().clone();
        assert!(cranked(&r).is_none());
        refused.extend(r.alerts.iter().filter_map(|a| match a {
            Alert::OracleRefused { reason, .. } => Some(reason.clone()),
            _ => None,
        }));
    }
    assert_eq!(refused, vec!["Step".to_string()]);
    assert_eq!(h.with_env(|env| env.oracle_cache()).publish_time, last);
    // An acceptable update lands.
    let t = h.now() + 1;
    let fine = h.with_env(|env| env.oracle_cache()).price * 1_002 / 1_000;
    h.with_env(|env| plant(env, &update(t, fine)));
    let landed = (0..31).any(|_| cranked(&h.second().clone()).is_some());
    assert!(landed);

    // The feed stops: aging alerted once past 1,800 s; the tier falls to C at 3,600 s.
    let tier = |h: &Harness| h.with_env(|env| env.market(&key).tier);
    let tier_fresh = tier(&h);
    let mut aging = Vec::new();
    for _ in 0..3_700 {
        let r = h.second().clone();
        aging.extend(
            r.alerts
                .iter()
                .filter(|a| matches!(a, Alert::OracleAging { .. }))
                .cloned(),
        );
    }
    assert_eq!(aging.len(), 1, "{aging:?}");
    let tier_stale = tier(&h);
    println!(
        "duty 11: tier {tier_fresh} with a fresh cache, {tier_stale} 3,700 s after the last update"
    );
    // With the keeper's cache fresh the aged, deep market is Tier A; once the feed stops, the
    // keeper's next crank_price after `oracle_stale_s` re-tiers it C (the program's rule).
    assert_eq!(tier_fresh, shortwin_math::tier::Tier::A as u8);
    assert_eq!(tier_stale, shortwin_math::tier::Tier::C as u8);
    // The oracle's cost is a global line; the market's budget paid only its observations.
    let m = &h.keeper.metrics;
    assert!(m.oracle_cranks >= 8 && m.oracle_lamports >= 5_000 * m.oracle_cranks);
    let spent = h.keeper.track(&key).unwrap().spent_today;
    let cranks_fee = m.markets[&key].cranks * 5_000;
    assert!(spent <= cranks_fee, "{spent} vs {cranks_fee}");
}

/// Duty 11 with the configured feed account missing (a misconfigured `pyth_sol_usd`, or the
/// account closed): `Alert::OracleUnreadable` once, nothing sent, the markets still covered; the
/// feed appearing is cranked at the next read.
#[test]
fn a_missing_feed_alerts_once_and_is_cranked_once_it_appears() {
    let (mut h, mms) = setup(1, KeeperConfig::default());
    let key = mms[0].market();
    let _ = open(&h, &mms[0], Q);
    h.keeper.config_mut().oracle.enabled = true;
    let mut unreadable = 0;
    for _ in 0..95 {
        let r = h.second().clone();
        assert!(r.oracle.is_none() && r.errors.is_empty(), "{r:?}");
        unreadable += r
            .alerts
            .iter()
            .filter(|a| matches!(a, Alert::OracleUnreadable { detail, .. } if detail == "missing"))
            .count();
    }
    assert_eq!(unreadable, 1);
    assert_eq!(h.keeper.metrics.oracle_reads, 4, "one read every 30 s");
    assert_eq!(h.keeper.metrics.markets[&key].misses, 0);
    let now = h.now();
    h.with_env(|env| plant(env, &update(now, dump().price)));
    let landed = (0..31).find(|_| cranked(&h.second().clone()).is_some());
    assert!(landed.is_some());
    assert_eq!(h.with_env(|env| env.oracle_cache()).publish_time, now);
}
