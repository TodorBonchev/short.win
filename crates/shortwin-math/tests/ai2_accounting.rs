//! Focused evidence and regression tests for AI review 2's accounting findings.
#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]

#[path = "pool_harness.rs"]
mod pool_harness;

use pool_harness::{LUMP_VEST_S, Model};
use shortwin_math::MathError;
use shortwin_math::num::{Rounding, WAD, mul_div, sqrt_u128};
use shortwin_math::pool::{self, SweepSource};
use shortwin_math::pool_select::{lambda_bps, locked_depth};
use shortwin_math::{params, rent, risk, waterfall};

fn exit_allowance(s: &mut Model, who: usize) {
    for _ in 0..8 {
        let l = s.touch(who);
        let p = s.m.pool;
        let allow = s.free_allowance(who).unwrap();
        let claims = u128::from(p.tokens_free) + u128::from(p.borrowed_q) + pool::VIRTUAL_ASSETS;
        let zero = (p.total_shares + pool::VIRTUAL_SHARES).div_ceil(claims) - 1;
        let burn = l.shares.min(s.shares_at(allow).max(zero).max(1));
        if s.withdraw(who, burn).is_err() {
            break;
        }
    }
}

fn dust_flush() -> Model {
    let mut s = Model::new(9_000);
    s.deposit(0, 1_000_000_000_000).unwrap();
    s.deposit(1, 1_000_000_000_000).unwrap();
    let pos = s.borrow30(s.m.pool.tokens_free * 9 / 10);
    exit_allowance(&mut s, 0);
    exit_allowance(&mut s, 1);
    assert_eq!(s.m.pool.tokens_free, 1);
    s.advance(60, 0);
    s.settle(pos, 50_000_000_000, 4_000_000_000).unwrap();
    assert!(s.m.pool.total_shares > 0);
    assert_eq!(s.m.pool.vest_bal, 0);
    s
}

#[test]
fn air2_05_dust_flush_waits_for_old_cashback_then_credits_it_before_deposits() {
    let mut s = dust_flush();
    assert_eq!(
        s.m.deposit_with(2, 1_000_000, true),
        Err(MathError::WipeSweepPending)
    );
    s.m.sweep_from(0, SweepSource::Trader).unwrap();
    assert_eq!(
        s.m.deposit_with(2, 1_000_000, true),
        Err(MathError::WipeSweepPending)
    );
    s.m.sweep_from(1_000_000_000, SweepSource::VenueRewards)
        .unwrap();
    assert_eq!(
        s.m.pool.vest_bal, 0,
        "old cashback is credited to the flushed holders"
    );
    s.m.deposit_with(2, 1_000_000, true).unwrap();
    s.advance(i64::from(LUMP_VEST_S) + 1, 0);
    assert_eq!(s.claim(2), Ok(0));
    let paid = s.claim(0).unwrap() + s.claim(1).unwrap();
    assert!(
        (50_999_999_995..=51_000_000_000).contains(&paid),
        "paid {paid}"
    );
}

#[test]
fn air2_05_ordinary_income_still_vests_and_does_not_gate_deposits() {
    let mut s = Model::new(9_000);
    s.deposit(0, 1_000_000).unwrap();
    s.sweep(1_000_000_000);
    assert_eq!(s.m.pool.vest_bal, 1_000_000_000);
    s.m.deposit_with(1, 1_000_000, true).unwrap();
    s.advance(i64::from(LUMP_VEST_S), 0);
    assert_eq!(s.claim(1), Ok(500_000_000));
}

#[test]
fn air2_09_proportional_unlocked_deposits_cannot_erase_locked_depth() {
    assert_eq!(lambda_bps(10_000, 9_999), 1);
    assert_eq!(lambda_bps(10_001, 10_000), 0);
    assert_eq!(locked_depth(10_000, 9_999, 10_000, 10_000), Ok(1));
    assert_eq!(locked_depth(10_001, 10_000, 10_001, 10_001), Ok(1));
    for counter in [10_000u64, 10_001, 20_000, 1_000_000, 1_000_000_000] {
        // Scaled reserves make the same edge exceed the normal eligibility floor.
        assert_eq!(
            locked_depth(
                counter,
                counter - 1,
                counter * 10_000_000_000,
                counter * 10_000_000_000
            ),
            Ok(10_000_000_000)
        );
    }
}

#[test]
fn air2_09_raw_ratio_depth_is_conservative_and_bounds_the_old_bps_error() {
    for (counter, supply, x, y) in [
        (3, 1, 100, 225),
        (10_001, 10_000, 100_010, 100_010),
        (u64::MAX, u64::MAX / 2, u64::MAX, u64::MAX),
    ] {
        let l = sqrt_u128(u128::from(x) * u128::from(y), Rounding::Floor);
        let old = mul_div(
            u128::from(lambda_bps(counter, supply)),
            l,
            10_000,
            Rounding::Floor,
        )
        .unwrap();
        let exact = mul_div(
            u128::from(counter - supply),
            l,
            u128::from(counter),
            Rounding::Floor,
        )
        .unwrap();
        let new = locked_depth(counter, supply, x, y).unwrap();
        assert_eq!(new, exact);
        assert!(new <= l);
        assert!(new >= old && new - old <= l.div_ceil(10_000));
    }
    assert_eq!(locked_depth(0, 0, 10, 10), Ok(0));
    assert_eq!(locked_depth(10, 11, 10, 10), Ok(0));
}

#[test]
fn air2_01_a_fully_locked_lender_stays_locked_when_conversion_awards_a_unit() {
    let mut p = pool::Pool::new();
    p.share_epoch = 1;
    p.total_shares = 3_002;
    p.borrowed_q = 1;
    p.tokens_free = 1;
    let mut v = pool::Vault::new();
    v.lock_norm = 3_002;
    v.lock_cal[1] = 3_002;
    v.cal_day = 1;
    let mut l = pool::Lender {
        shares: 2_500_999,
        lock: 2_500_999,
        lock_term: 2_500_999,
        lock_pi: WAD,
        lock_epoch: v.pi_epoch,
        lock_until: 86_400,
        epoch: 0,
        ..pool::Lender::new(&p)
    };
    let mut records = [pool::ShareEpoch {
        acc_end: 0,
        acc_r_end: 0,
        s_old: 3_002_000,
        carry: 0,
        carry_r: 0,
        conv_rem: 1,
        epoch: 0,
        num: 1,
        den_log1000: 1,
        _pad: [0; 10],
    }];
    pool::convert(&p, &mut l, &mut records).unwrap();
    assert_eq!(l.shares, 2_501);
    assert_eq!(
        pool::allowance(&p, &v, &l, 1),
        Ok(0),
        "no unlocked share existed before the conversion"
    );
}

#[test]
fn l39_one_token_deposit_cannot_unlock_a_fully_locked_position() {
    let mut s = Model::new(9_000);
    for who in 0..3 {
        s.deposit(who, 1_000_000).unwrap();
    }
    s.borrow30(10_000);
    let shares = s.shares_at(1_000);
    assert_eq!(s.withdraw(2, shares), Ok(1_000));
    let repaid = s.borrow30(1_000_000);
    let lost = s.borrow30(1_000_000);
    for who in 0..3 {
        exit_allowance(&mut s, who);
    }
    s.repay(repaid, 0, 0).unwrap();
    for who in 0..3 {
        assert_eq!(s.free_allowance(who), Ok(0));
        let l = s.touch(who);
        assert_eq!(pool::lock_of(&s.m.vault, &l, s.m.now), Ok(l.shares));
    }
    let before = s.clone();
    let minted = s.deposit(0, 1).unwrap();
    assert_eq!(minted, 999);
    let unlocked_value = pool::tokens_for_shares(&s.m.pool, minted).unwrap();
    assert_eq!(unlocked_value, 0);
    let old_by_value = pool::tokens_for_shares(&s.m.pool, s.shares(0)).unwrap();
    assert_eq!(old_by_value, 670_224);
    assert_eq!(
        pool::locked_total(&s.m.pool, &s.m.vault, s.m.now).unwrap() + minted,
        s.m.pool.total_shares,
        "the old by_free cap permits all of T"
    );
    assert_eq!(s.free_allowance(0), Ok(0));
    let all = s.shares(0);
    assert_eq!(s.withdraw(0, all), Err(MathError::AboveAllowance));
    s.settle(lost, 0, 0).unwrap();
    let mut control = before;
    control.settle(lost, 0, 0).unwrap();
    let baseline = pool::tokens_for_shares(&control.m.pool, control.shares(0)).unwrap();
    assert_eq!(baseline, 336_779);
    let protected = pool::tokens_for_shares(&s.m.pool, s.shares(0)).unwrap();
    assert!(protected.abs_diff(baseline) <= 1);
    // The pre-fix run withdrew 670,224, net of its one-token deposit 670,223,
    // almost twice the control's residual. This exceeds KI-13's +11.1% bound.
    assert!((old_by_value - 1) * 100 > baseline * 190);
}

#[test]
fn l30_route_chain_keeps_the_sixty_second_boundary_but_restarts_after_it() {
    assert_eq!(
        shortwin_math::settle::chain_step(100, 700, 760, true),
        (100, 760)
    );
    assert_eq!(
        shortwin_math::settle::chain_step(100, 700, 761, true),
        (761, 761)
    );
}

#[test]
fn pass2_min_notional_room_guarantees_the_boundary_but_not_price_overshoot() {
    let v = params::MIN_NOTIONAL_FLOOR * 2;
    let k_s = risk::k_s(v, &risk::SettleKeeperTerms::DEFAULT).unwrap();
    let budget = risk::Budget {
        v,
        k_s,
        ert: 10_000_000,
    };
    let first_liquidatable = v * 10_000 / 12_500 + 1;
    assert_eq!(v, 106_000_000);
    assert_eq!(k_s, 11_060_000);
    assert!(risk::cr_below(v, Some(first_liquidatable), 12_500).unwrap());
    assert_eq!(budget.exceeded_by(Some(first_liquidatable)), Ok(false));
    assert_eq!(v - k_s - budget.ert - first_liquidatable, 139_999);
    assert_eq!(
        budget.exceeded_by(Some(first_liquidatable * 10_050 / 10_000)),
        Ok(true)
    );
}

#[test]
fn pass2_s3_currently_uses_the_markets_lower_cap_history() {
    use shortwin_math::settle::{S3Chain, S3Terms, s3_due, s3_no_route};
    assert!(s3_no_route(true, false, 40_000, 300));
    assert!(!s3_no_route(true, false, 40_000, 500));
    let chain = S3Chain {
        no_route_now: s3_no_route(true, false, 50_001, 500),
        no_route_since: Some(100),
        graduated_at: None,
        migrating: false,
        migration_seen_at: None,
    };
    // The higher-cap position first loses its route at 1,000, yet the market chain
    // started at 100 for a lower-cap position. This pins the open policy question.
    assert!(s3_due(&chain, &S3Terms::DEFAULT, 1_000, true));
    assert!(!s3_due(&chain, &S3Terms::DEFAULT, 1_000, false));
}

#[test]
fn l16_l79_calendar_expiry_survives_a_clock_step_back() {
    let mut s = Model::new(9_000);
    s.deposit(0, 1_000_000_000).unwrap();
    s.deposit(1, 1_000_000_000).unwrap();
    s.borrow30(1_800_000_000);
    exit_allowance(&mut s, 0);
    let until = s.touch(0).lock_until;
    assert!(pool::lock_of(&s.m.vault, s.m.lender(0).unwrap(), 0).unwrap() > 0);
    // A third party can touch the other lender without advancing the market's accrual.
    s.m.now = until;
    s.m.touch_pot(1).unwrap();
    s.m.now = until - 1;
    assert_eq!(
        pool::lock_of(&s.m.vault, s.m.lender(0).unwrap(), s.m.now),
        Ok(0)
    );
    assert!(
        s.withdraw(0, 1).is_ok(),
        "an expired calendar term must not be re-added to a dead day"
    );
}

#[test]
fn pass2_waterfalls_reject_rates_above_their_documented_domain() {
    let x = waterfall::ExitInputs {
        v: 1_000_000,
        b: 100_000,
        r_due: 1,
        b_live: 100_000,
        b_ref: 100_000,
        rent_protocol_bps: 1_500,
    };
    assert_eq!(
        waterfall::liquidate(&x, 10_001, &waterfall::PenaltySplit::DEFAULT, 0),
        Err(MathError::InvalidInput)
    );
    assert_eq!(
        waterfall::expire(&x, 10_001, &risk::ExpiryKeeperTerms::DEFAULT),
        Err(MathError::InvalidInput)
    );
}

#[test]
fn air2_13_rent_exhaustion_pays_no_keeper_even_with_a_floor() {
    let x = waterfall::ExitInputs {
        v: 20_000_000_000,
        b: 19_000_000_000,
        r_due: 1_000_000_000,
        b_live: 19_000_000_000,
        b_ref: 19_000_000_000,
        rent_protocol_bps: 1_500,
    };
    let k = risk::k_s(x.v, &risk::SettleKeeperTerms::DEFAULT).unwrap();
    let out = waterfall::liquidate(&x, 500, &waterfall::PenaltySplit::DEFAULT, k).unwrap();
    assert_eq!(out.keeper, 0);
    assert_eq!(out.rent, 1_000_000_000);
    assert_eq!(out.total(), Ok(x.v));
    let x = waterfall::ExitInputs {
        b: 13_000_000_000,
        b_live: 13_000_000_000,
        b_ref: 13_000_000_000,
        ..x
    };
    let expired = waterfall::expire(&x, 200, &risk::ExpiryKeeperTerms::DEFAULT).unwrap();
    let liquidated = waterfall::liquidate(&x, 500, &waterfall::PenaltySplit::DEFAULT, k).unwrap();
    assert_eq!(
        (expired.keeper, liquidated.keeper),
        (14_000_000, 210_000_000)
    );
}

#[test]
fn l40_l52_valid_extreme_minimum_rent_exceeds_the_open_liquidation_gap() {
    let mut p = params::ParamSet::DEFAULT;
    p.tiers[0].cr_liq_bps = p.tiers[0].cr_open_bps - 2_000;
    p.tiers[0].rate = rent::RentTerms {
        base_bps: 50_000,
        slope1_bps: 1_000_000,
        slope2_bps: 1_000_000,
    };
    p.tiers[0].min_rent_s = 86_400;
    assert_eq!(params::validate(&p), Ok(()));
    assert_eq!(params::validate_econ(&p), Ok(()));
    let t = p.tiers[0];
    let notional = 1_000_000_000;
    let r = rent::min_rent(
        notional,
        WAD,
        &t.rate,
        8_900,
        p.params.util_kink_bps,
        t.min_rent_s,
    )
    .unwrap();
    let at_zero = rent::min_rent(
        notional,
        WAD,
        &t.rate,
        0,
        p.params.util_kink_bps,
        t.min_rent_s,
    )
    .unwrap();
    assert_eq!((at_zero, r), (13_698_631, 257_534_247));
    let v = notional * u64::from(t.cr_open_bps) / 10_000;
    assert!(risk::cr_below(v - r, Some(notional), t.cr_liq_bps).unwrap());
}

#[test]
fn pass2_split_settlements_cannot_avoid_the_live_vest_hold() {
    let mut p = pool::Pool::new();
    let mut v = pool::Vault::new();
    let mut lender = pool::Lender::new(&p);
    pool::deposit(&mut p, &mut v, &mut lender, &mut [], 0, 1_000_000, false).unwrap();
    pool::borrow(&mut p, 0, 900_000, 86_400).unwrap();
    for q in [400_000, 240_000, 144_000, 86_400, 29_600] {
        let settled = pool::settle(
            &mut p,
            &mut v,
            0,
            LUMP_VEST_S,
            &pool::Resolution {
                q,
                credited: 0,
                lump: 1_000_000_000,
            },
            1_000_000_000,
            0,
        )
        .unwrap();
        assert!(!settled.flushed);
        assert!(
            settled.hold,
            "q={q}: settlement income still vests after token claims were removed"
        );
    }
    assert_eq!(p.borrowed_q, 0);
    assert_eq!(p.vest_bal, 5_000_000_000);
}

#[test]
fn pass2_pot_claims_can_use_receivables_and_the_proposed_current_share_subtraction_does_not_fix_it()
{
    let mut s = pool_harness::Market::new();
    s.deposit(0, 1_000_000_000).unwrap();
    s.deposit(1, 1_000_000_000).unwrap();
    s.borrow(1_000_000_000, 86_400).unwrap();
    s.accrue(0, 200_000_000_000).unwrap();
    s.deposit(2, 2_000_000_000).unwrap();
    s.repay(500_000_000, 100_000_000_000, 0, 100_000_000_000, 0)
        .unwrap();
    s.touch(2).unwrap();
    s.touch(0).unwrap();
    assert_eq!(
        (s.pool.receivable, s.vault.unmatched),
        (100_000_000_000, 50_000_000_000)
    );
    let a = s.lender(0).unwrap();
    let alleged_receivable = mul_div(
        u128::from(s.pool.receivable),
        a.shares,
        s.pool.total_shares,
        Rounding::Ceil,
    )
    .unwrap();
    assert_eq!(alleged_receivable, 25_000_000_000);
    assert_eq!((a.pending, a.real), (100_000_000_000, 25_000_000_000));
    let proposed_draw = (a.pending as u128 - u128::from(a.real) - alleged_receivable)
        .min(u128::from(s.vault.unmatched));
    assert_eq!(
        proposed_draw, 50_000_000_000,
        "the report patch would still give the first claimant the entire pot"
    );
    assert_eq!(
        (s.claim(0), s.claim(1)),
        (Ok(75_000_000_000), Ok(25_000_000_000))
    );
    let allowance = s.allowance(0).unwrap();
    let shares = pool::shares_for_tokens(&s.pool, allowance).unwrap();
    s.withdraw(0, shares).unwrap();
    s.repay(500_000_000, 0, 0, 100_000_000_000, 0).unwrap();
    for who in 0..3 {
        s.touch(who).unwrap();
    }
    assert_eq!(
        (
            s.lender(0).unwrap().pending,
            s.lender(1).unwrap().pending,
            s.lender(2).unwrap().pending
        ),
        (21_000_000_000, 43_000_000_000, -64_000_000_000)
    );
}
