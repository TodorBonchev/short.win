//! Market prologue (ARCH §3.0; ECON §2.1 order of operations).
//!
//! Every instruction that loads a `Market` for writing runs, in order: (1) load `config` and
//! `market`, check versions (the Accounts constraints); (2) params-version sync
//! ([`sync_market_params`]: EMA fields when `open_positions == 0` or `now ≥
//! ema_params.force_at`; the kink only when `open_positions == 0`; A17, A59, R3A-9); (3) the
//! mint guard ([`crate::engine::guard::mint_guard`]); (4) accrue ([`accrue`]: vest release
//! (`pool::release`) then rent (`rent::accrue` on `P_ema` before the roll, `U` from the
//! pre-instruction counters; lender credit to `acc` and `receivable`)); (5) observation roll
//! (`observe::integrate`, `CoverageGap`); (6)–(8) venue sync, observation record and route
//! chains (M2+); then the body; (10) post-conditions; (11) one `emit_cpi!`.
//!
//! **M1 subset.** A16: `deposit` and `withdraw` run steps 1–5 and 10–11 (no venue accounts, no
//! observation); `claim_sol`, `cover_deficit`, `sync_lender` do not write `Market` at all. Step
//! 5 is M2's: no M1 instruction records an observation, so the bucket is never open
//! (`Market.b_lo == 0`) and the roll has nothing to integrate; M2 adds it between [`accrue`]
//! and the body. On a list-only (`None`) market there is no price, so rent is 0 and only the
//! vest is released.
//!
//! Every step runs at [`market_now`], never before the market's last accrual.

use anchor_lang::prelude::*;
use shortwin_math::pool::{self, Pool, Vault};
use shortwin_math::rent;

use crate::errors::MathResultExt;
use crate::state::{GlobalConfig, Market};

/// The time a `Market`-writing instruction runs at: the clock, never before the market's
/// last accrual. `Clock::unix_timestamp` is not guaranteed to be monotonic, and the vest
/// release refuses a time before `vest_last` (`pool::release`); every pool mutation runs after
/// the prologue's accrual in the same instruction, so `vest_last ≤ last_accrual_ts`, and this
/// keeps deposits and withdrawals live across a clock step back.
pub fn market_now(market: &Market, clock_now: i64) -> i64 {
    clock_now.max(market.last_accrual_ts)
}

/// ARCH §3.0 step 2, params-version sync (A17, A59; R3A-9): the EMA fields are adopted when
/// their version changed and the market has no open position or `now ≥ force_at`; the kink
/// only while the market has no open position (a forced EMA adoption never touches it).
pub fn sync_market_params(config: &GlobalConfig, market: &mut Market, now: i64) {
    let ema = &config.ema_params;
    if market.ema_params_version != ema.version
        && (market.open_positions == 0 || now >= ema.force_at)
    {
        market.ema_dtmax_s = ema.ema_dtmax_s;
        market.ema_clamp_bps = ema.ema_clamp_bps;
        market.liq_slow_tau_s = ema.liq_slow_tau_s;
        market.liq_slow_dtmax_s = ema.liq_slow_dtmax_s;
        market.ema_params_version = ema.version;
    }
    let kink = &config.kink;
    if market.kink_version != kink.version && market.open_positions == 0 {
        market.util_kink_bps = kink.util_kink_bps;
        market.kink_version = kink.version;
    }
}

/// ARCH §3.0 step 4, accrue (ECON §7, §9.4): the rent basis indices advance over
/// `now − last_accrual_ts` at `P̃ = min(P_ema, P_ACC_MAX)` (the EMA **before** this
/// instruction's roll) and the utilisation of the pre-instruction counters (`rent::accrue`);
/// the lenders' credit `⌊Σ_j A_j·δJ_j/(10⁴·10⁴·31,536,000·WAD)⌋` (`rent::lender_credit`) goes
/// to `acc` and `receivable` after the vest release (`pool::accrue` releases first). On a
/// list-only market (no price) and with no open position both are 0 and only the vest is
/// released. Returns the lender credit.
pub fn accrue(market: &mut Market, pool: &mut Pool, vault: &mut Vault, now: i64) -> Result<u64> {
    let dt = u64::try_from(now.saturating_sub(market.last_accrual_ts)).unwrap_or(0);
    let util = rent::utilization_bps(pool.tokens_free, pool.borrowed_q).math()?;
    let mut basis = market.basis();
    let delta = rent::accrue(
        &mut basis,
        market.ema_price.get(),
        util,
        market.util_kink_bps,
        dt,
    )
    .math()?;
    let credit = rent::lender_credit(&market.aggr(), &delta).math()?;
    pool::accrue(pool, vault, now, credit).math()?;
    market.set_basis(&basis);
    market.last_accrual_ts = market.last_accrual_ts.max(now);
    Ok(credit)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;
    use shortwin_math::num::WAD;

    #[test]
    fn params_sync_follows_versions_and_positions() {
        let mut config = GlobalConfig::zeroed();
        config.ema_params.version = 2;
        config.ema_params.ema_dtmax_s = 1;
        config.ema_params.ema_clamp_bps = 5_000;
        config.ema_params.force_at = 1_000;
        config.kink.version = 3;
        config.kink.util_kink_bps = 7_000;
        let mut m = Market::zeroed();
        m.ema_params_version = 1;
        m.kink_version = 1;
        m.util_kink_bps = 8_000;
        m.open_positions = 2;

        // Positions open, before force_at: nothing is adopted.
        sync_market_params(&config, &mut m, 999);
        assert_eq!((m.ema_params_version, m.kink_version), (1, 1));
        // After force_at the EMA fields are forced; the kink is not (R3A-9).
        sync_market_params(&config, &mut m, 1_000);
        assert_eq!(
            (m.ema_params_version, m.ema_dtmax_s, m.ema_clamp_bps),
            (2, 1, 5_000)
        );
        assert_eq!((m.kink_version, m.util_kink_bps), (1, 8_000));
        // With no open position the kink follows.
        m.open_positions = 0;
        sync_market_params(&config, &mut m, 1_000);
        assert_eq!((m.kink_version, m.util_kink_bps), (3, 7_000));
    }

    #[test]
    fn accrue_on_a_list_only_market_releases_the_vest_only() {
        let mut m = Market::zeroed();
        m.last_accrual_ts = 100;
        m.util_kink_bps = 8_000;
        let mut p = Pool::new();
        p.total_shares = 1_000;
        p.tokens_free = 1;
        p.vest_bal = 600;
        p.vest_last = 100;
        p.vest_end = 700;
        let mut v = Vault::new();
        v.lender_sol = 600;
        assert_eq!(accrue(&mut m, &mut p, &mut v, 400).unwrap(), 0);
        assert_eq!(
            (p.vest_bal, p.vest_last, m.last_accrual_ts),
            (300, 400, 400)
        );
        assert_eq!(p.acc_r, 300 * WAD / 1_000);
        assert_eq!(m.rent_basis.iter().map(|j| j.get()).sum::<u128>(), 0);
        // Never backwards.
        assert_eq!(market_now(&m, 390), 400);
    }

    #[test]
    fn accrue_credits_rent_from_the_aggregate() {
        // One position: q = 10⁶ at base 5,000 bps (slopes 0), protocol share 1,500 bps, at
        // P_ema = WAD (1 lamport per raw unit), for one day: rent ⌈10⁶·0.5/365⌉ lamports, of
        // which 85 % is credited to lenders (floor).
        let mut m = Market::zeroed();
        m.last_accrual_ts = 0;
        m.util_kink_bps = 8_000;
        m.ema_price.set(WAD);
        let terms = rent::RentTerms {
            base_bps: 5_000,
            slope1_bps: 0,
            slope2_bps: 0,
        };
        let mut aggr = rent::Aggr::default();
        aggr.add(1_000_000, &terms, 1_500).unwrap();
        m.set_aggr(&aggr);
        let mut p = Pool::new();
        p.total_shares = 2_000_000_000;
        p.tokens_free = 1_000_000;
        p.borrowed_q = 1_000_000;
        let mut v = Vault::new();
        let credit = accrue(&mut m, &mut p, &mut v, 86_400).unwrap();
        let expected = 1_000_000u128 * 5_000 * 86_400 * 8_500 / (10_000 * 10_000 * 31_536_000);
        assert_eq!(u128::from(credit), expected);
        assert_eq!(p.receivable, credit);
        assert_eq!(m.basis().j[0], WAD * 86_400);
    }
}
