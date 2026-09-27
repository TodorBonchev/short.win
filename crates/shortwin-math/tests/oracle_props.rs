//! Properties of `observe`, `persist` and `bounds` (ARCHITECTURE §8.1: EMA step bounds and
//! "own observation never read" (I-E7), directional aggregation, the ring's bucket
//! extremes and medians, the venue-change rewrite, the liquidation EMA leg (I-E8), exit
//! references and `max_in` incl. the rent-and-fee seniority of an expiry (I-E2) and the
//! liquidation liveness invariant I-E11).
#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]

mod common;

use num_bigint::BigUint;
use proptest::prelude::*;
use shortwin_math::bounds::{
    ExitParams, expiry_reference, kappa_eff, liq_reference, max_in_exp, max_in_liq, pi_ref,
};
use shortwin_math::consts::EMA_TAU_S;
use shortwin_math::observe::{
    Bucket, EmaParams, LastBucket, Observation, RING_LEN, Ring, RingEntry, VenueChange,
    ring_medians, robust_spot, sustained_growth,
};
use shortwin_math::persist::liq_ema_leg;
use shortwin_math::quote::reprice;

fn big(x: u128) -> BigUint {
    BigUint::from(x)
}

/// Any in-bound EMA parameters (ARCH §2.3.3), and some beyond them (Δt_max to 10, C from
/// 10 %): `integrate` does not validate.
fn ema_params() -> impl Strategy<Value = EmaParams> {
    (1u16..=10, 1_000u32..=10_000, 600u32..=86_400, 1u32..=600)
        .prop_map(|(d, c, st, sd)| EmaParams::new(d, c, st, sd))
}

/// Prices across the whole range, weighted towards realistic WAD prices.
fn price() -> impl Strategy<Value = u128> {
    prop_oneof![
        4 => 1_000u128..10u128.pow(30),
        1 => 1u128..=u128::MAX,
        1 => Just(u128::MAX),
        1 => 1u128..1_000,
    ]
}

/// One integration of a single-second bucket `[lo, hi]` after `gap` seconds.
fn step(p: u128, lo: u128, hi: u128, gap: i64, prm: &EmaParams) -> Observation {
    let mut o = Observation::new(0, p, 1).unwrap();
    o.record(gap - 1, lo, 1).unwrap();
    o.record(gap - 1, hi, 1).unwrap();
    assert_eq!(o.roll(gap, prm).unwrap(), Some(gap));
    o
}

fn entry(ts: i64, p: u128, lo: u128, hi: u128, llo: u128, lhi: u128) -> RingEntry {
    RingEntry::reprojected(ts, p, 1_000_000, lo, hi, llo, lhi)
}

proptest! {
    #![proptest_config(common::proptest_config())]

    /// ECON §5.2 step bounds: one integration moves P_ema at most to
    /// (τ + dt·(1 + C))/(τ + dt) and at least to (τ + dt/(1 + C))/(τ + dt), up to the floors;
    /// the ring entry keeps the bucket's extremes and caches reprice(L_ema, P_ema).
    #[test]
    fn ema_step_is_clamped(p in price(), a in price(), b in price(), gap in 1i64..30, prm in ema_params()) {
        let (lo, hi) = (a.min(b), a.max(b));
        let o = step(p, lo, hi, gap, &prm);
        let new = o.ema.price;
        let (tau, c) = (u128::from(EMA_TAU_S), u128::from(prm.clamp_bps));
        let dt = u128::try_from(gap).unwrap().min(u128::from(prm.dtmax_s));
        let w = big(tau + dt);
        // Upper: new·(τ+dt)·10⁴ ≤ P·(τ·10⁴ + dt·(10⁴ + C)).
        prop_assert!(big(new) * &w * big(10_000) <= big(p) * big(tau * 10_000 + dt * (10_000 + c)));
        // Lower: (new + 1)·(τ+dt)·(10⁴+C) > P·(τ·(10⁴+C) + dt·10⁴) − dt·(10⁴+C).
        let lhs = (big(new) + 1u8) * &w * big(10_000 + c) + big(dt * (10_000 + c));
        prop_assert!(lhs > big(p) * big(tau * (10_000 + c) + dt * 10_000));
        // The new EMA lies between the old one and the bucket.
        prop_assert!(new >= p.min(lo) && new <= p.max(hi));
        let e = o.ring.newest().unwrap();
        prop_assert_eq!((e.lo, e.hi, e.llo, e.lhi), (lo, hi, 1, 1));
        prop_assert_eq!((e.x, e.y), reprice(e.l, e.p).unwrap_or((0, 0)));
    }

    /// Directional aggregation (ECON §5.2): a bucket with observations on both sides of the
    /// EMA leaves it exactly where it was (+0.00 % with one honest observation), while the
    /// ring still records the second's honest extremes (the "frozen" second of R4B-2).
    #[test]
    fn mixed_bucket_freezes_ema(p in price(), below in 0u128..=10_000, above in 0u128..=10_000,
                                gap in 1i64..10, prm in ema_params()) {
        let lo = (p - p / 10_001 * below).max(1);
        let hi = p.saturating_add(p / 10_001 * above);
        let o = step(p, lo, hi, gap, &prm);
        prop_assert_eq!(o.ema.price, p);
        let e = o.ring.newest().unwrap();
        prop_assert_eq!((e.p, e.lo, e.hi), (p, lo, hi));
    }

    /// The new EMA is monotone in every observation and in the old EMA.
    #[test]
    fn ema_is_monotone(p in price(), a in price(), b in price(), up in price(), gap in 1i64..10,
                       prm in ema_params()) {
        let (lo, hi) = (a.min(b), a.max(b));
        let base = step(p, lo, hi, gap, &prm).ema.price;
        let hi2 = hi.saturating_add(up);
        prop_assert!(step(p, lo, hi2, gap, &prm).ema.price >= base);
        prop_assert!(step(p, lo.saturating_add(up).min(hi), hi, gap, &prm).ema.price >= base);
        prop_assert!(step(p.saturating_add(up), lo, hi, gap, &prm).ema.price >= base);
    }

    /// At most one integration per second, only of completed seconds, and an instruction
    /// never reads its own observation (ECON §5.2 one-second lag; I-E7). The pre-roll gap is
    /// `now − ema_ts`. Two markets that differ only in the observations of the current second
    /// have identical EMA state.
    #[test]
    fn one_update_per_second(p in price(),
                             seconds in prop::collection::vec((0i64..3, prop::collection::vec(price(), 1..4)), 1..40),
                             extra in prop::collection::vec(price(), 1..4), prm in ema_params()) {
        let mut o = Observation::new(0, p, 7).unwrap();
        let mut now = 0i64;
        let mut distinct = 0u64;
        for (dt, obs) in &seconds {
            let before = o.ema;
            now += dt;
            let rolled = o.roll(now, &prm).unwrap();
            prop_assert!(o.roll(now, &prm).unwrap().is_none(), "a second roll in the same second");
            if let Some(gap) = rolled {
                prop_assert_eq!(gap, now - before.ts);
                prop_assert_eq!(o.ema.count, before.count + 1);
                prop_assert_eq!(o.ring.newest().unwrap().ts, now);
                prop_assert_eq!(o.last.sec, now - dt);
            }
            let open_second = o.bucket.is_open().then_some(o.bucket.sec);
            prop_assert!(open_second.is_none_or(|s| s == now));
            if !(o.bucket.is_open() && o.bucket.sec == now) {
                distinct += 1;
            }
            let frozen = o.ema;
            for &x in obs {
                o.record(now, x, 7).unwrap();
            }
            prop_assert_eq!(o.ema, frozen, "an observation is read in its own second");
        }
        // Rolled exactly once per distinct observed second except the still-open one.
        prop_assert_eq!(o.ema.count + 1, distinct);
        let mut twin = o;
        for &x in &extra {
            twin.record(now, x, 7).unwrap();
        }
        prop_assert_eq!(twin.ema, o.ema);
        prop_assert_eq!(twin.ring, o.ring);
        // Ring: bounded, strictly increasing timestamps, newest = EMA state.
        prop_assert!(o.ring.len() <= RING_LEN);
        let ts: Vec<i64> = o.ring.iter().map(|e| e.ts).collect();
        prop_assert!(ts.windows(2).all(|w| w[0] > w[1]));
        let newest = o.ring.newest().unwrap();
        prop_assert_eq!((newest.p, newest.l), (o.ema.price, o.ema.liq));
    }

    /// Robust spot (ECON §5.4): never above the live spot or the last bucket low, and
    /// defined exactly while the completed bucket is fresh.
    #[test]
    fn robust_spot_is_conservative(live in price(), lo in price(), sec in -100i64..100, now in -100i64..200,
                                   age in 0u8..6) {
        let last = LastBucket { lo, hi: lo, sec, _pad: [0; 8] };
        match robust_spot(live, &last, now, age) {
            Some(p) => {
                prop_assert!(p <= live && p <= lo && (p == live || p == lo));
                prop_assert!(now - sec <= i64::from(age));
            }
            None => prop_assert!(now - sec > i64::from(age)),
        }
    }

    /// Sustained growth (ECON §5.4): 0 unless every one of the last n steps rose; never above
    /// the slowest step.
    #[test]
    fn sustained_growth_is_the_slowest_step(p0 in 1_000u128..10u128.pow(30),
                                            steps in prop::collection::vec((0u128..3_000, 0i64..4), 0..8),
                                            n in 0u8..9) {
        let mut ring = Ring::new(0, p0, 1).unwrap();
        let (mut p, mut ts) = (p0, 0i64);
        for &(bps, dt) in &steps {
            p += p * bps / 10_000;
            ts += dt;
            ring.push(entry(ts, p, p, p, 1, 1)).unwrap();
        }
        let g = sustained_growth(&ring, n).unwrap();
        let ents: Vec<RingEntry> = ring.iter().copied().collect();
        let n = usize::from(n);
        if n == 0 || ents.len() <= n {
            prop_assert_eq!(g, 0);
        } else {
            for k in 0..n {
                let (new, old) = (ents[k], ents[k + 1]);
                if new.ts <= old.ts || new.p <= old.p {
                    prop_assert_eq!(g, 0);
                } else {
                    let rate = (new.p - old.p) * 10_000 / old.p / u128::try_from(new.ts - old.ts).unwrap();
                    prop_assert!(u128::from(g) <= rate);
                }
            }
        }
    }

    /// Ring medians (ECON §2.1, §5.6): the lower median of the lows and the upper median of
    /// the highs are elements of their columns with the right ranks, and no set of at most
    /// ⌊(n − 1)/2⌋ entries (4 of 8) can raise P_mlo above the other entries' lows or lower
    /// P_mhi below their highs, whatever it records (R4B-2, R4A-9: "5 of 8").
    #[test]
    fn medians_need_five_of_eight(cols in prop::collection::vec((1u128..1_000, 1u128..1_000, 1u128..1_000, 1u128..1_000), 1..=8),
                                  attacker in prop::collection::vec((any::<bool>(), 1u128..10_000), 8)) {
        let mut ring = Ring::default();
        for (i, &(a, b, c, d)) in cols.iter().enumerate() {
            ring.push(entry(i as i64, 1, a, b, c, d)).unwrap();
        }
        let n = ring.len();
        let (pl, ph, ll, lh) = ring_medians(&ring).unwrap();
        let col = |f: fn(&RingEntry) -> u128| { let mut v: Vec<u128> = ring.iter().map(f).collect(); v.sort(); v };
        prop_assert_eq!(pl, col(|e| e.lo)[(n - 1) / 2]);
        prop_assert_eq!(ph, col(|e| e.hi)[n / 2]);
        prop_assert_eq!(ll, col(|e| e.llo)[(n - 1) / 2]);
        prop_assert_eq!(lh, col(|e| e.lhi)[n / 2]);
        // An attacker controls at most ⌊(n − 1)/2⌋ entries.
        let limit = (n - 1) / 2;
        let chosen: Vec<usize> = (0..n).filter(|&k| attacker[k].0).take(limit).collect();
        let mut pushed = ring;
        for (k, e) in pushed.entries.iter_mut().enumerate() {
            if chosen.contains(&k) {
                e.lo += attacker[k].1;
                e.hi = e.hi.saturating_sub(attacker[k].1).max(1);
            }
        }
        let (pl2, ph2, _, _) = ring_medians(&pushed).unwrap();
        let honest: Vec<&RingEntry> = pushed.entries.iter().take(n).enumerate()
            .filter(|(k, _)| !chosen.contains(k)).map(|(_, e)| e).collect();
        prop_assert!(pl2 <= honest.iter().map(|e| e.lo).max().unwrap());
        prop_assert!(ph2 >= honest.iter().map(|e| e.hi).min().unwrap());
    }

    /// Venue changes (ECON §5.2; ARCH §4.4, A72): every entry's l, llo, lhi become the reset
    /// L_ema with (x, y) re-cached, prices and price extremes carry over, the bucket clears;
    /// a Raydium change never raises L_ema or L_slow, a graduation sets both.
    #[test]
    fn rewrite_liquidity_resets_the_ring(p in 1_000u128..10u128.pow(24), l in 1u128..10u128.pow(18),
                                         slow in 1u128..10u128.pow(18), l_new in 1u128..10u128.pow(18),
                                         prices in prop::collection::vec((1_000u128..10u128.pow(24), 1u128..10u128.pow(18)), 0..8),
                                         raydium in any::<bool>()) {
        let mut o = Observation::new(0, p, l).unwrap();
        o.ema.liq_slow = slow;
        for (i, &(q, lq)) in prices.iter().enumerate() {
            o.ring.push(RingEntry::reprojected(i as i64 + 1, q, lq, q / 2, q * 2, lq / 2, lq * 2)).unwrap();
        }
        o.record(9, p, l).unwrap();
        let before = o;
        let kind = if raydium { VenueChange::Raydium } else { VenueChange::Graduation };
        o.rewrite_liquidity(l_new, kind).unwrap();
        if raydium {
            prop_assert_eq!((o.ema.liq, o.ema.liq_slow), (l_new.min(l), l_new.min(slow)));
        } else {
            prop_assert_eq!((o.ema.liq, o.ema.liq_slow), (l_new, l_new));
        }
        prop_assert_eq!((o.ema.price, o.ema.ts, o.ema.count), (before.ema.price, before.ema.ts, before.ema.count));
        prop_assert_eq!(o.bucket, Bucket::EMPTY);
        prop_assert_eq!(o.last, before.last);
        for (e, b) in o.ring.iter().zip(before.ring.iter()) {
            prop_assert_eq!((e.ts, e.p, e.lo, e.hi), (b.ts, b.p, b.lo, b.hi));
            prop_assert_eq!((e.l, e.llo, e.lhi), (o.ema.liq, o.ema.liq, o.ema.liq));
            prop_assert_eq!((e.x, e.y), reprice(o.ema.liq, e.p).unwrap_or((0, 0)));
        }
    }

    /// Liquidation EMA leg (ECON §5.5; I-E8): holds iff the newest n entries all have their
    /// cached state below L, and is monotone in n.
    #[test]
    fn liq_ema_leg_is_the_newest_n(flags in prop::collection::vec(any::<bool>(), 1..=8), n in 0u8..10) {
        let mut ring = Ring::default();
        for (i, _) in flags.iter().enumerate() {
            ring.push(RingEntry { x: i as u128 + 1, y: 1, ts: i as i64, ..RingEntry::default() }).unwrap();
        }
        let below = |x: u128, _y: u128| Ok(flags[usize::try_from(x).unwrap() - 1]);
        let leg = liq_ema_leg(&ring, below, n).unwrap();
        let k = usize::from(n);
        let want = k <= flags.len() && flags.iter().rev().take(k).all(|&f| f);
        prop_assert_eq!(leg, want);
        if leg && n > 0 {
            prop_assert!(liq_ema_leg(&ring, below, n - 1).unwrap());
        }
    }

    /// Exit references (ECON §8.2): the liquidation reference lies in [min(P_ema, lo_x),
    /// ⌊P_ema·(1 + κ_max)⌋] and never above ⌊lo_x·(1 + κ_eff)⌋ — without sustained growth a
    /// pushed-and-frozen P_ema cannot lift it above the bucket low (R4B-2); the expiry
    /// reference is at most P_ema, every ring price and the bucket low; both need a fresh
    /// completed bucket.
    #[test]
    fn references_are_bounded(p in 1_000u128..10u128.pow(33), steps in prop::collection::vec(0u128..2_000, 1..8),
                              lo_bps in 5_000u128..20_000, ema_bps in 5_000u128..20_000, age in 0i64..5,
                              kmax in 0u16..=5_000, kbase in 0u16..=1_000, max_age in 1u8..=5, gn in 2u8..=7) {
        let mut ring = Ring::new(0, p, 1).unwrap();
        let (mut q, mut ts) = (p, 0);
        for &bps in &steps {
            q += q * bps / 10_000;
            ts += 1;
            ring.push(entry(ts, q, q, q, 1, 1)).unwrap();
        }
        let mut o = Observation::new(0, p, 1).unwrap();
        o.ring = ring;
        o.ema.price = q * ema_bps / 10_000;
        let lo = q * lo_bps / 10_000;
        o.last = LastBucket { lo, hi: q, sec: ts, _pad: [0; 8] };
        let now = ts + age;
        let prm = ExitParams { kappa_liq_bps: kmax, kappa_base_bps: kbase, ref_max_age_s: max_age,
                               kappa_growth_n: gn, ..ExitParams::DEFAULT };
        let fresh = age <= i64::from(max_age);
        let g = sustained_growth(&o.ring, gn).unwrap();
        let kappa = u128::from(kappa_eff(g, &prm));
        prop_assert!(kappa <= u128::from(kmax));
        let r = liq_reference(&o.ema, &o.ring, &o.last, now, &prm).unwrap();
        prop_assert_eq!(r.is_some(), fresh);
        if let Some(r) = r {
            let lo_x = lo * (10_000 + u128::from(g) * u128::try_from(age).unwrap()) / 10_000;
            prop_assert!(r >= o.ema.price.min(lo_x));
            prop_assert!(r <= o.ema.price * (10_000 + u128::from(kmax)) / 10_000);
            prop_assert!(r <= lo_x * (10_000 + kappa) / 10_000);
            if g == 0 {
                prop_assert!(r <= lo * (10_000 + u128::from(kbase.min(kmax))) / 10_000);
            }
        }
        let e = expiry_reference(&o.ema, &o.ring, &o.last, now, &prm).unwrap();
        prop_assert_eq!(e.is_some(), fresh);
        if let Some(e) = e {
            prop_assert!(e <= o.ema.price && e <= lo && o.ring.iter().all(|x| e <= x.p));
        }
    }

    /// `max_in` (ECON §8.2 v0.4.1): a liquidation's bound is exactly
    /// min(⌊B̂_ref·(1 + δ)⌋, V − K_s − ert), so it never waits below it (I-E11); an expiry's
    /// is below its bound and the S1 budget, and keeps rent and the expiry fee senior whenever
    /// the fair cost leaves room for them (I-E2). Both are monotone in each input.
    #[test]
    fn max_in_properties(b in 0u64..10u64.pow(13), d in 0u16..=2_000, e in 0u16..=100, fee in 0u16..=500,
                         v in 0u64..10u64.pow(13), k in 0u64..10u64.pow(9), r in 0u64..10u64.pow(12),
                         ert in 0u64..10u64.pow(7), live in 0u64..10u64.pow(13), bump in 1u64..10u64.pow(10)) {
        let bound = i128::from(b) * (10_000 + i128::from(d)) / 10_000;
        let budget = i128::from(v) - i128::from(k) - i128::from(ert);
        let ml = max_in_liq(b, d, v, k, ert);
        prop_assert_eq!(ml.map(i128::from), (budget >= 0).then(|| bound.min(budget)));
        // I-E11: a live cost within the bound and the S1 budget always executes.
        if i128::from(live) <= bound && i128::from(live) <= budget {
            prop_assert!(ml.is_some_and(|m| live <= m));
        }
        let pi = pi_ref(b, fee).unwrap();
        prop_assert_eq!(u128::from(pi), (u128::from(b) * u128::from(fee)).div_ceil(10_000));
        let me = max_in_exp(b, d, e, v, k, r, pi, ert);
        let eps = i128::from(b) * (10_000 + i128::from(e)) / 10_000;
        let room = i128::from(v) - i128::from(r) - i128::from(pi) - i128::from(ert);
        match me {
            None => prop_assert!(budget < 0),
            Some(m) => {
                let m = i128::from(m);
                prop_assert!(m <= bound && m <= budget);
                if room >= eps {
                    prop_assert!(m <= room, "rent and fee must stay senior");
                }
            }
        }
        let key = |x: Option<u64>| x.map_or(-1i128, i128::from);
        prop_assert!(key(max_in_liq(b.saturating_add(bump), d, v, k, ert)) >= key(ml));
        prop_assert!(key(max_in_liq(b, d, v.saturating_add(bump), k, ert)) >= key(ml));
        prop_assert!(key(max_in_liq(b, d, v, k.saturating_add(bump), ert)) <= key(ml));
        prop_assert!(key(max_in_exp(b, d.saturating_add(100), e, v, k, r, pi, ert)) >= key(me));
        prop_assert!(key(max_in_exp(b, d, e, v.saturating_add(bump), k, r, pi, ert)) >= key(me));
        prop_assert!(key(max_in_exp(b, d, e, v, k, r.saturating_add(bump), pi, ert)) <= key(me));
        prop_assert!(key(max_in_exp(b, d, e, v, k, r, pi, ert.saturating_add(bump))) <= key(me));
    }
}

/// A bucket is open iff its low is non-zero; `record` refuses zero observations, so the
/// sentinel can never be forged.
#[test]
fn bucket_sentinel() {
    let mut b = Bucket::EMPTY;
    assert!(!b.is_open());
    assert!(shortwin_math::observe::record(&mut b, 0, 0, 1).is_err());
    shortwin_math::observe::record(&mut b, 0, 1, 1).unwrap();
    assert!(b.is_open());
}
