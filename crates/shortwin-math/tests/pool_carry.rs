//! Last-share withdrawal closes both SOL distribution carries before another cohort enters.

#[path = "pool_harness.rs"]
mod pool_harness;

use pool_harness::{LUMP_VEST_S, Market};
use proptest::prelude::*;
use shortwin_math::MathError;
use shortwin_math::num::WAD;
use shortwin_math::pool;

fn legacy_empty_pool() -> Market {
    let mut m = Market::new();
    let shares = m.deposit(0, 2_000_000_000_000_000_000).unwrap();
    m.sweep(1_000).unwrap();
    m.now += i64::from(LUMP_VEST_S);
    m.release().unwrap();
    let (carry, carry_r) = (m.pool.carry, m.pool.carry_r);
    m.withdraw(0, shares).unwrap();
    // Reconstruct the old withdrawal's persisted state: it burned the same shares
    // but neither assigned the carry to the old lender nor cleared those fields.
    m.lenders[0].1.pending = 0;
    m.lenders[0].1.real = 0;
    m.pool.carry = carry;
    m.pool.carry_r = carry_r;
    // Captured by the unmodified b815269 public-API generator in the independent
    // accounting review. Compare the whole state, not just the two carry fields.
    assert_eq!(
        m.canonical(),
        include_str!("fixtures/legacy_empty_carry.canonical").trim_end()
    );
    m
}

#[test]
fn legacy_empty_pool_carry_refuses_a_new_cohort_after_completed_sweeps() {
    for rewards_required in [false, true] {
        let mut m = legacy_empty_pool();
        m.sweep(0).unwrap();
        m.pool.wipe_swept |= pool::WIPE_REWARDS_CLAIMED;
        let mut next = pool::Lender::new(&m.pool);
        let before = m.clone();
        let next_before = next;
        for _ in 0..2 {
            assert_eq!(
                pool::deposit(
                    &mut m.pool,
                    &mut m.vault,
                    &mut next,
                    &mut m.records,
                    m.now,
                    1,
                    rewards_required,
                ),
                Err(MathError::LegacyPoolCarry)
            );
            assert_eq!(
                m, before,
                "old backing, claims and carries must be preserved"
            );
            assert_eq!(
                next, next_before,
                "no new shares or snapshots may be committed"
            );
        }
        assert_eq!(m.claim(0), Ok(0));
        assert_eq!(m.vault.lender_sol, 1_000);
    }
}

#[test]
fn legacy_empty_pool_refuses_either_carry_including_sub_lamport_residues() {
    for (carry, carry_r) in [
        (1, 0),
        (0, 1),
        (-1, 0),
        (1_000 * WAD as i128, 0),
        (0, 1_000 * WAD),
    ] {
        for swept in [0, pool::WIPE_TRADER_SWEPT | pool::WIPE_REWARDS_CLAIMED] {
            for rewards_required in [false, true] {
                let (mut p, mut v) = (pool::Pool::new(), pool::Vault::new());
                p.carry = carry;
                p.carry_r = carry_r;
                p.wipe_swept = swept;
                let mut lender = pool::Lender::new(&p);
                let before = (p, v, lender);
                assert_eq!(
                    pool::deposit(&mut p, &mut v, &mut lender, &mut [], 0, 1, rewards_required),
                    Err(MathError::LegacyPoolCarry)
                );
                assert_eq!((p, v, lender), before);
            }
        }
    }
}

#[test]
fn last_exit_pays_whole_carry_before_a_new_cohort_enters() {
    for (tokens, income, release_first) in [
        (2_000_000_000_000_000_000, 1_000, false),
        (2_000_000_000_000_000_000, 1_000, true),
        (18_000_000_000_000_000_000, 17_999, false),
    ] {
        let mut m = Market::new();
        let shares = m.deposit(0, tokens).unwrap();
        m.sweep(income).unwrap();
        if release_first {
            m.now += i64::from(LUMP_VEST_S);
            m.release().unwrap();
            assert_eq!(m.pool.carry, i128::from(income) * WAD as i128);
        }
        assert_eq!(m.pool.borrowed_q, 0);
        assert_eq!(m.withdraw(0, shares), Ok(tokens));
        assert_eq!(m.claim(0), Ok(income));
        assert_eq!((m.pool.carry, m.pool.carry_r, m.pool.dust), (0, 0, 0));
        assert_eq!(m.vault.lender_sol, 0);
        m.sweep(0).unwrap();
        let new_shares = m.deposit(1, 1).unwrap();
        m.sweep(1).unwrap();
        assert_eq!(m.withdraw(1, new_shares), Ok(1));
        assert_eq!(m.claim(1), Ok(1));
        m.invariants().unwrap();
    }
}

#[test]
fn late_depositor_can_capture_preexisting_carry_unresolved_residual() {
    // Characterization only: this historical ownership question remains unresolved.
    // A's swept income has already vested before B buys one raw token of shares.
    let mut m = Market::new();
    let a_tokens = 2_000_000_000_000_000_000;
    let a = m.deposit(0, a_tokens).unwrap();
    m.sweep(1_001_000).unwrap();
    m.now += i64::from(LUMP_VEST_S);
    m.release().unwrap();
    assert_eq!((m.pool.acc, m.pool.carry), (500, 1_000 * WAD as i128));

    let b = m.deposit(1, 1).unwrap();
    assert_eq!(b, 1_000);
    assert_eq!(m.withdraw(0, a), Ok(a_tokens));
    assert_eq!(m.claim(0), Ok(1_000_000));
    assert_eq!(m.withdraw(1, b), Ok(1));
    assert_eq!(m.claim(1), Ok(1_000));
    assert_eq!(
        (m.pool.carry, m.pool.carry_r, m.vault.lender_sol),
        (0, 0, 0)
    );
    m.invariants().unwrap();
}

#[test]
fn carry_mismatch_preserves_a_previous_lenders_pending_claim_and_the_pot() {
    // A earns rent before B enters; repayment allocates realised SOL at the new supply.
    // B exits last. For a small B, entitlement carry exceeds realised carry and B draws
    // the pot. For a large B, the reverse holds and B must return A's backing to the pot.
    for (b_tokens, realised_carry, pot_before_claim) in [
        (1_000_000_000_000_000_000, 0, 1_000),
        (3_000_000_000_000_000_000, 3_000, 2_000),
    ] {
        let mut m = Market::new();
        let a = m.deposit(0, 2_000_000_000_000_000_000).unwrap();
        m.borrow(1, 1).unwrap();
        m.accrue(0, 3_000).unwrap();
        let b = m.deposit(1, b_tokens).unwrap();
        m.repay(1, 3_000, 0, 3_000, 0).unwrap();
        assert_eq!((m.pool.borrowed_q, m.pool.receivable), (0, 0));
        m.withdraw(0, a).unwrap();
        assert_eq!(m.lender(0).unwrap().pending, 2_000);
        assert_eq!(m.lender(0).unwrap().shares, 0);
        assert_eq!(m.pool.carry, 1_000 * WAD as i128);
        assert_eq!(m.pool.carry_r, realised_carry * WAD);
        m.withdraw(1, b).unwrap();
        assert_eq!(m.vault.unmatched, pot_before_claim);
        assert_eq!(m.lender(1).unwrap().pending, 1_000);
        assert!(m.lender(1).unwrap().real <= 1_000);
        m.invariants().unwrap();
        assert_eq!(m.claim(1), Ok(1_000));
        m.sweep(0).unwrap();
        let new_shares = m.deposit(2, 1).unwrap();
        m.sweep(1).unwrap();
        m.withdraw(2, new_shares).unwrap();
        assert_eq!(m.claim(2), Ok(1));
        assert_eq!(m.claim(0), Ok(2_000));
        assert_eq!(m.claim(0), Ok(0));
        assert_eq!((m.vault.lender_sol, m.vault.unmatched), (0, 0));
        m.invariants().unwrap();
    }
}

#[test]
fn partial_burn_keeps_carry_and_last_burn_drops_only_sub_lamport_residues() {
    let mut m = Market::new();
    let shares = m.deposit(0, 2_000_000_000_000_000_001).unwrap();
    m.sweep(2_001).unwrap();
    m.now += i64::from(LUMP_VEST_S);
    m.release().unwrap();
    let carry = WAD - 1_000;
    assert_eq!((m.pool.carry, m.pool.carry_r), (carry as i128, carry));
    m.withdraw(0, 1_000).unwrap();
    assert_eq!((m.pool.carry, m.pool.carry_r), (carry as i128, carry));
    m.withdraw(0, shares - 1_000).unwrap();
    assert_eq!((m.pool.carry, m.pool.carry_r), (0, 0));
    assert_eq!(m.pool.dust, 2 * carry as i128);
    assert_eq!(m.claim(0), Ok(2_000));
    assert_eq!(m.vault.lender_sol, 1);
    assert!(pool::closable(&m.pool, m.lender(0).unwrap()));
    // Legitimate dust/backing without either pool carry must not quarantine a
    // clean empty pool. The new cohort earns only its own one-lamport income.
    m.sweep(0).unwrap();
    let next = m.deposit(1, 1).unwrap();
    m.sweep(1).unwrap();
    m.withdraw(1, next).unwrap();
    assert_eq!(m.claim(1), Ok(1));
    assert_eq!(m.vault.lender_sol, 1);
    m.invariants().unwrap();
}

#[test]
fn overflowing_last_exit_credit_leaves_all_inputs_unchanged() {
    let mut m = Market::new();
    let shares = m.deposit(0, 2_000_000_000_000_000_000).unwrap();
    m.sweep(i64::MAX as u64 + 1).unwrap();
    let before = m.clone();
    let result = pool::withdraw(
        &mut m.pool,
        &mut m.vault,
        &mut m.lenders[0].1,
        &mut m.records,
        m.now,
        m.lock_grace_s,
        shares,
    );
    assert_eq!(result, Err(MathError::Overflow));
    assert_eq!(
        m, before,
        "vest flush and carry payout must roll back together"
    );
}

#[test]
fn signed_rent_true_up_carry_returns_only_the_unallocated_cover() {
    let mut m = Market::new();
    let a = m.deposit(0, 2_000_000_000_000_000_000).unwrap();
    m.borrow(1, 1).unwrap();
    m.accrue(0, 3_000).unwrap();
    let b = m.deposit(1, 1_000_000_000_000_000_000).unwrap();
    m.repay(1, 0, 0, 3_000, 0).unwrap();
    m.withdraw(0, a).unwrap();
    m.touch(1).unwrap();
    assert_eq!(m.lender(1).unwrap().pending, -1_000);
    assert_eq!(m.withdraw(1, b), Err(MathError::NegativePending));
    m.cover(1, 1_000).unwrap();
    m.withdraw(1, b).unwrap();
    assert_eq!(m.claim(1), Ok(1_000));
    assert_eq!(m.claim(0), Ok(0));
    assert_eq!((m.vault.lender_sol, m.vault.unmatched), (0, 0));
    m.invariants().unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn changing_share_supply_preserves_backing_and_cannot_feed_the_next_cohort(
        a_tokens in 1_000_000_000_000_000u64..4_000_000_000_000_000_000,
        b_tokens in 1_000_000_000_000_000u64..4_000_000_000_000_000_000,
        rent in 1u64..100_000,
        old_claims_first in any::<bool>(),
    ) {
        let mut m = Market::new();
        let a = m.deposit(0, a_tokens).unwrap();
        m.borrow(1, 1).unwrap();
        m.accrue(0, rent).unwrap();
        let b = m.deposit(1, b_tokens).unwrap();
        m.repay(1, rent, 0, rent, 0).unwrap();
        m.withdraw(0, a).unwrap();
        let first = if old_claims_first { m.claim(0).unwrap() } else { 0 };
        m.withdraw(1, b).unwrap();
        prop_assert_eq!((m.pool.carry, m.pool.carry_r), (0, 0));
        prop_assert!(m.pool.dust < 2 * WAD as i128);
        m.invariants().unwrap();
        let last = m.claim(1).unwrap();
        m.sweep(0).unwrap();
        let new = m.deposit(2, 1).unwrap();
        m.sweep(1).unwrap();
        m.withdraw(2, new).unwrap();
        prop_assert_eq!(m.claim(2).unwrap(), 1);
        let old = first + last + m.claim(0).unwrap();
        prop_assert_eq!(old + m.vault.lender_sol, rent);
        // Only the documented floors of the two indices / touches remain unallocated.
        prop_assert!(m.vault.lender_sol <= 4);
        m.invariants().unwrap();
    }
}
