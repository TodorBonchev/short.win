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

/// **PROPOSED** (M4R, post-change confirmation; owner item O18): integrations of the warm-up
/// that every **restart** of the observation state runs — a Raydium adoption (a listing
/// window's, a zero-position, a fast-path or a list-only market's window adoption), a Raydium
/// listing, a list-only market's first adoption (`observe::mark_restart`). A restart reads no
/// sample of the new pool: it writes the anchor `⌊D·10⁴/λ_min⌋` into `L_ema` and the ring (or,
/// for a price start, `min(√(X·Y), anchor)` at the pool's spot) and sets `Ema.warm` to this
/// value; each of the next `EMA_WARMUP_BUCKETS` integrations re-seeds the EMA over the
/// post-change ring entries only (`P_ema = min lo`, rising by at most the largest v0.4.1 step,
/// `L_ema = min(max lhi, ⌊lomed(lhi)·(10⁴ + C)/10⁴⌋)`, `observe::integrate`) and counts `warm`
/// down. While `warm > 0` on an executable
/// venue, opens, `liquidate` and `expire` refuse `EmaWarmingUp` and `sol_settle` evaluates
/// nothing (ECON §5.2, §5.5; ARCH §2.6 rule 4, §3.4, §3.6–§3.8, §4.4). `= RING_LEN`: when the
/// gate opens every ring entry is post-change; `≥` the largest `liq_persist_n`, so the EMA leg
/// reads post-change entries only.
// `RING_LEN ≤ u8::MAX` is asserted below, so the cast is exact.
#[allow(clippy::cast_possible_truncation)]
pub const EMA_WARMUP_BUCKETS: u8 = RING_LEN as u8;

/// **PROPOSED** (M4R; owner item O18): the tolerance of the post-change reference guard,
/// bps (`= fv_dev_bps`'s default). The guard `Ema.guard`, set by every restart, makes the
/// keeper exits' reference liquidity read `min(L_ema, L_mlo)` (`bounds::ref_liquidity`); it
/// clears at the first integration after the warm-up with `L_ema·10⁴ ≤ L_mlo·(10⁴ +
/// REF_GUARD_TOL_BPS)`, i.e. once `L_ema` is within 1 % of the ring's lower median of the
/// bucket liquidity lows (an honest switch: at the gate).
pub const REF_GUARD_TOL_BPS: u32 = 100;

/// **PROPOSED** (FITS-1; owner decision of 2026-09-29, ECON §6.1, O18): the ceiling, ppm, on the
/// one-way fee φ the carried-position fit's fee-scaled caps take (`caps::carried_phi`: `φ =
/// min(φ_inc, φ_cand, CARRIED_FEE_CAP_PPM)`). 2,500 ppm = 25 bps puts every position a pool
/// switch carries in the ≤ 0.75 %-of-`X₀` size class (`fee_size_cap_x100` 300 at 25 bps) that
/// the sims and the fork validated under the switch's pushes, whatever fee it was margined at
/// and whatever the candidate charges: at `min(φ_inc, φ_cand)` alone a 1 % → 1 % switch carried
/// positions up to the candidate's impact cap (2.47 % of `X₀`), whose margin — posted on the
/// incumbent's deeper cost — the pushes turned into S1 at 60–100 % of that limit. A position
/// larger than the carried cap is not adopted: it settles in S3 at fair value on the failed pool
/// (the accepted fallback).
pub const CARRIED_FEE_CAP_PPM: u32 = 2_500;

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
// M4R: the warm-up refills the whole ring, and the EMA leg's `liq_persist_n` newest entries are
// post-change entries once it ends.
const _: () = assert!(
    Rule::LiqPersistN.bounds().1 <= EMA_WARMUP_BUCKETS as u64
        && EMA_WARMUP_BUCKETS as usize == RING_LEN
);

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
        assert_eq!((EMA_WARMUP_BUCKETS, REF_GUARD_TOL_BPS), (8, 100));
        assert_eq!(CARRIED_FEE_CAP_PPM, 2_500);
        assert_eq!(
            (S3_OBS_GAP_S, LOCK_BUCKET_S, LOCK_BUCKETS),
            (60, 86_400, 128)
        );
        assert_eq!(MAX_RELEASE_SPAN_S, 93 * LOCK_BUCKET_S);
    }
}
