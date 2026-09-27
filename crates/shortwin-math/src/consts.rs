//! Owner: oracle track (integration: lock calendar, S3 gap). Compile-time protocol
//! constants shared by the program, the keeper and the WASM facade (ARCHITECTURE §2.1
//! `constants.rs`, §2.2, §2.3.3 scope C; ECONOMICS §2.2, §5.7, §15). Values equal
//! `sim/shortwin_sim.py` v0.4.1 (`P_ACC_MAX`, `P_OPEN_MAX`, `Params.ring_len`,
//! `Params.s3_obs_gap_s`, `Tier.ema_tau_s`) and `sim/pool_model.py` v0.4.1 (`DAY`, the
//! calendar). The compiled parameter bounds are [`crate::params::Rule::bounds`].

use crate::params::Rule;

/// τ of the EMA in seconds (ECON §5.2; ARCH §2.3.3). A compile-time constant since v0.4
/// (R3B-6): τ = 6 loses 83 of the 1,655 liveness-battery runs (ECON §5.7).
pub const EMA_TAU_S: u32 = 5;

/// Entries of the EMA history ring (ECON §5.4 `ring_len`; ARCH §2.3.3). The settlement
/// box collapses to the EMA point after `RING_LEN` seconds without an integration (ECON
/// §5.6; ARCH §3.8).
pub const RING_LEN: usize = 8;

/// Rent accrual clamps the price at `P̃ = min(P_ema, P_ACC_MAX)` (WAD, a price of 100 SOL
/// per raw unit), which keeps the rent basis indices below 2¹²⁸ for 100 years (ECON §2.2).
pub const P_ACC_MAX: u128 = 100_000_000_000_000_000_000_000_000_000;

/// Opens are refused while `P_ema > P_OPEN_MAX` (WAD, 1 SOL per raw unit; ECON §2.2).
pub const P_OPEN_MAX: u128 = 1_000_000_000_000_000_000_000_000_000;

/// No-route observations further apart than this restart the S3 timer, seconds (ECON §10.3,
/// §15; ARCH §2.3.3, §5.3; C0-16; sim `Params.s3_obs_gap_s`).
pub const S3_OBS_GAP_S: i64 = 60;

/// Length of one lock-calendar bucket: one day (ECON §9.3, §15; ARCH §2.3.3, §2.3.6, A75).
pub const LOCK_BUCKET_S: i64 = 86_400;

/// Day buckets in the lock calendar ring (ECON §9.3, §15; ARCH §2.3.3, §2.3.6, A75).
pub const LOCK_BUCKETS: usize = 128;

/// `CoverageGap` threshold in seconds: an integration that finds the EMA older than this
/// while the market has open positions emits the event (ARCH §2.3.3, §3.10, §4.4, A69; no
/// economic effect).
pub const COVERAGE_GAP_S: i64 = 30;

// ARCH §2.3.3: `RING_LEN ≥ max(liq_persist_n, kappa_growth_n + 1)` for every compiled
// value, and ring counters are `u8`.
const _: () = assert!(
    RING_LEN as u64 >= Rule::LiqPersistN.bounds().1
        && RING_LEN as u64 > Rule::KappaGrowthN.bounds().1
        && RING_LEN <= u8::MAX as usize
);
const _: () = assert!(P_OPEN_MAX < P_ACC_MAX);

/// Longest time from open to a position's last possible resolution that a lock must cover:
/// the compiled maxima of `max_duration_s` and of the S3 tail `max_migration_pause_s +
/// grad_grace_s + s3_wait_s` (ECON §15: 90 d + 3 d; `pool::borrow` refuses a later
/// `release_at`).
pub const MAX_RELEASE_SPAN_S: i64 = Rule::TierMaxDurationS.bounds().1 as i64
    + Rule::MaxMigrationPauseS.bounds().1 as i64
    + Rule::GradGraceS.bounds().1 as i64
    + Rule::S3WaitS.bounds().1 as i64;

// ECON §15: `LOCK_BUCKETS·LOCK_BUCKET_S ≥` max duration + S3 tail + `lock_grace_s` + 2 d, so
// a new lock's day is always inside the calendar ring (`lock_until − now` ≤ 101 d).
const _: () = assert!(
    (LOCK_BUCKETS as i64) * LOCK_BUCKET_S
        >= MAX_RELEASE_SPAN_S + Rule::LockGraceS.bounds().1 as i64 + 2 * LOCK_BUCKET_S
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_match_the_sim() {
        assert_eq!(P_ACC_MAX, 10u128.pow(29));
        assert_eq!(P_OPEN_MAX, 10u128.pow(27));
        assert_eq!((EMA_TAU_S, RING_LEN, COVERAGE_GAP_S), (5, 8, 30));
        assert_eq!(
            (S3_OBS_GAP_S, LOCK_BUCKET_S, LOCK_BUCKETS),
            (60, 86_400, 128)
        );
        assert_eq!(MAX_RELEASE_SPAN_S, 93 * LOCK_BUCKET_S);
    }
}
