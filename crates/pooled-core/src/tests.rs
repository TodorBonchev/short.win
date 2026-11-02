use super::*;

fn seeded(a: u64, b: u64) -> (Pool, Lender, Lender) {
    let mut p = Pool::new(0);
    let mut la = Lender::new(0);
    let mut lb = Lender::new(0);
    if a > 0 {
        deposit(&mut p, &mut la, a).unwrap();
    }
    if b > 0 {
        deposit(&mut p, &mut lb, b).unwrap();
    }
    (p, la, lb)
}

#[test]
fn paid_income_is_attributed_before_deposit_and_share_changes() {
    let (mut p, mut a, mut b) = seeded(100, 100);
    credit_income(&mut p, 1000).unwrap();
    assert_eq!(deposit(&mut p, &mut b, 100).unwrap(), 100);
    assert_eq!(b.pending_sol, 500);
    credit_income(&mut p, 900).unwrap();
    assert_eq!(claim(&mut p, &mut a).unwrap(), 800);
    assert_eq!(claim(&mut p, &mut b).unwrap(), 1100);
    assert_eq!(p.sol_reserve, 0);
    assert_eq!(
        withdraw(&mut p, &mut a, 100).unwrap(),
        Withdrawal {
            tokens: 100,
            sol: 0
        }
    );
    assert_eq!(
        withdraw(&mut p, &mut b, 200).unwrap(),
        Withdrawal {
            tokens: 200,
            sol: 0
        }
    );
    assert_eq!((p.cash, p.shares, p.sol_reserve), (0, 0, 0));
}

#[test]
fn newcomer_cannot_claim_previously_paid_fees() {
    let (mut p, mut a, mut b) = seeded(100, 0);
    credit_income(&mut p, 100).unwrap();
    deposit(&mut p, &mut b, 100).unwrap();
    credit_income(&mut p, 100).unwrap();
    assert_eq!(claim(&mut p, &mut a).unwrap(), 150);
    assert_eq!(claim(&mut p, &mut b).unwrap(), 50);
}

#[test]
fn repeated_claims_keep_receipt_fraction_until_whole_lamport_exists() {
    let (mut p, mut a, mut b) = seeded(1, 1);
    credit_income(&mut p, 1).unwrap();
    assert_eq!(claim(&mut p, &mut a).unwrap(), 0);
    assert_eq!(a.fraction, SOL_SCALE / 2);
    assert_eq!(claim(&mut p, &mut a).unwrap(), 0);
    credit_income(&mut p, 1).unwrap();
    assert_eq!(claim(&mut p, &mut a).unwrap(), 1);
    assert_eq!(a.fraction, 0);
    assert_eq!(claim(&mut p, &mut b).unwrap(), 1);
    assert_eq!(p.sol_reserve, 0);
}

#[test]
fn final_receipt_exit_clears_fraction_and_last_holder_receives_tracked_dust() {
    let (mut p, mut a, mut b) = seeded(1, 2);
    credit_income(&mut p, 4).unwrap();
    assert_eq!(claim(&mut p, &mut a).unwrap(), 1);
    assert!(a.fraction > 0);
    assert_eq!(claim(&mut p, &mut b).unwrap(), 2);
    assert_eq!(p.sol_reserve, 1);
    assert_eq!(
        withdraw(&mut p, &mut a, 1).unwrap(),
        Withdrawal { tokens: 1, sol: 0 }
    );
    assert_eq!(a.fraction, 0);
    assert_eq!(
        withdraw(&mut p, &mut b, 2).unwrap(),
        Withdrawal { tokens: 2, sol: 1 }
    );
    assert_eq!((p.shares, p.cash, p.sol_reserve), (0, 0, 0));
    // The same empty generation may start a new cohort, but no old balance/fraction is copied.
    let mut newcomer = Lender::new(0);
    deposit(&mut p, &mut newcomer, 10).unwrap();
    assert_eq!(claim(&mut p, &mut newcomer).unwrap(), 0);
    assert_eq!(newcomer.fraction, 0);
}

#[test]
fn outstanding_principal_limits_withdrawal_and_forbids_last_share_exit() {
    let (mut p, mut a, mut b) = seeded(100, 100);
    borrow(&mut p, 150).unwrap();
    assert_eq!(
        (p.cash, p.principal_out, p.principal_assets().unwrap()),
        (50, 150, 200)
    );
    let before = (p, a);
    assert_eq!(
        withdraw(&mut p, &mut a, 100),
        Err(Error::InsufficientLiquidity)
    );
    assert_eq!((p, a), before);
    assert_eq!(withdraw(&mut p, &mut a, 50).unwrap().tokens, 50);
    repay(&mut p, 150).unwrap();
    assert_eq!(withdraw(&mut p, &mut a, 50).unwrap().tokens, 50);
    assert_eq!(withdraw(&mut p, &mut b, 100).unwrap().tokens, 100);
    let (mut p, mut a, _) = seeded(1, 0);
    borrow(&mut p, 1).unwrap();
    let before = (p, a);
    assert_eq!(withdraw(&mut p, &mut a, 1), Err(Error::LastSharesBorrowed));
    assert_eq!((p, a), before);
}

#[test]
fn default_allocates_loss_and_real_sol_only_to_current_pool_holders() {
    let (mut p, mut a, mut b) = seeded(100, 100);
    borrow(&mut p, 100).unwrap();
    credit_income(&mut p, 20).unwrap();
    assert_eq!(
        withdraw(&mut p, &mut a, 50).unwrap(),
        Withdrawal {
            tokens: 50,
            sol: 10
        }
    );
    default(&mut p, 100, 90).unwrap();
    assert_eq!(p.principal_assets().unwrap(), 50);
    assert_eq!((p.principal_out, p.loan_count), (0, 0));
    let wa = withdraw(&mut p, &mut a, 50).unwrap();
    let wb = withdraw(&mut p, &mut b, 100).unwrap();
    assert_eq!(
        wa,
        Withdrawal {
            tokens: 16,
            sol: 30
        }
    );
    assert_eq!(
        wb,
        Withdrawal {
            tokens: 34,
            sol: 70
        }
    );
    assert_eq!(50 + wa.tokens + wb.tokens, 200 - 100);
    assert_eq!(10 + wa.sol + wb.sol, 20 + 90);
    assert_eq!((p.cash, p.sol_reserve, p.shares), (0, 0, 0));
}

#[test]
fn wiped_generation_keeps_claims_separate_from_new_lenders() {
    let (mut old, mut a, _) = seeded(100, 0);
    borrow(&mut old, 100).unwrap();
    default(&mut old, 100, 30).unwrap();
    let mut b = Lender::new(0);
    let before = (old, b);
    assert_eq!(deposit(&mut old, &mut b, 100), Err(Error::Wiped));
    assert_eq!((old, b), before);
    retire_wiped(&mut old).unwrap();
    let mut new = next_generation(&old).unwrap();
    assert_eq!(new, Pool::new(1));
    assert_eq!(claim(&mut new, &mut a), Err(Error::WrongGeneration));
    let mut b = Lender::new(1);
    deposit(&mut new, &mut b, 100).unwrap();
    assert_eq!(
        withdraw(&mut old, &mut a, 100).unwrap(),
        Withdrawal { tokens: 0, sol: 30 }
    );
    assert_eq!(claim(&mut new, &mut b).unwrap(), 0);
    assert_eq!(
        withdraw(&mut new, &mut b, 100).unwrap(),
        Withdrawal {
            tokens: 100,
            sol: 0
        }
    );
}

#[test]
fn no_generation_reset_or_income_without_real_owners() {
    let mut p = Pool::new(0);
    let before = p;
    assert_eq!(credit_income(&mut p, 1), Err(Error::NoShares));
    assert_eq!(credit_tokens(&mut p, 1), Err(Error::NoShares));
    assert_eq!(retire_wiped(&mut p), Err(Error::NotWiped));
    assert_eq!(p, before);
    assert_eq!(next_generation(&p), Err(Error::NotRetired));
    let (mut p, _, _) = seeded(10, 0);
    borrow(&mut p, 10).unwrap();
    let before = p;
    assert_eq!(retire_wiped(&mut p), Err(Error::NotWiped));
    assert_eq!(p, before);
}

#[test]
fn a_donated_token_surplus_increases_existing_share_value() {
    let (mut p, mut a, mut b) = seeded(50, 50);
    borrow(&mut p, 40).unwrap();
    repay(&mut p, 40).unwrap();
    credit_tokens(&mut p, 20).unwrap();
    assert_eq!(withdraw(&mut p, &mut a, 50).unwrap().tokens, 60);
    assert_eq!(withdraw(&mut p, &mut b, 50).unwrap().tokens, 60);
}

#[test]
fn all_failed_mutations_preserve_the_inputs() {
    let (mut p, mut a, _) = seeded(10, 0);
    for result in [
        borrow(&mut p, 0),
        borrow(&mut p, 11),
        repay(&mut p, 1),
        default(&mut p, 1, 99),
    ] {
        assert!(result.is_err());
    }
    assert_eq!((p.cash, p.principal_out, p.loan_count), (10, 0, 0));
    let before = (p, a);
    assert_eq!(deposit(&mut p, &mut a, 0), Err(Error::ZeroAmount));
    assert_eq!(withdraw(&mut p, &mut a, 11), Err(Error::InsufficientShares));
    assert_eq!((p, a), before);
    p.shares = u128::MAX;
    p.cash = 1;
    a.shares = u128::MAX;
    let before = (p, a);
    assert_eq!(deposit(&mut p, &mut a, 1), Err(Error::Overflow));
    assert_eq!((p, a), before);
    assert_eq!(withdraw(&mut p, &mut a, u128::MAX).unwrap().tokens, 1);
}

#[test]
fn tiny_floor_share_mint_is_refused_before_state_changes() {
    let mut p = Pool::new(0);
    let mut a = Lender::new(0);
    let mut b = Lender::new(0);
    deposit(&mut p, &mut a, 1).unwrap();
    credit_tokens(&mut p, u64::MAX - 1).unwrap();
    let before = (p, b);
    assert_eq!(deposit(&mut p, &mut b, 1), Err(Error::ZeroAmount));
    assert_eq!((p, b), before);
}

#[test]
fn wide_income_index_survives_multiple_u64_lifetimes_without_truncation() {
    let (mut p, mut a, _) = seeded(1, 0);
    for _ in 0..32 {
        credit_income(&mut p, u64::MAX).unwrap();
        assert_eq!(claim(&mut p, &mut a).unwrap(), u64::MAX);
    }
    assert!(U256::from_limbs(p.sol_index).to_u128().is_none());
    assert_eq!(p.sol_reserve, 0);
    assert_eq!(
        withdraw(&mut p, &mut a, 1).unwrap(),
        Withdrawal { tokens: 1, sol: 0 }
    );
}

#[test]
fn reward_and_generation_overflows_are_atomic() {
    let (mut p, _, _) = seeded(1, 0);
    p.sol_reserve = u64::MAX;
    let before = p;
    assert_eq!(credit_income(&mut p, 1), Err(Error::Overflow));
    assert_eq!(p, before);
    let mut p = Pool::new(u64::MAX);
    p.retired = true;
    assert_eq!(next_generation(&p), Err(Error::Overflow));
}

#[test]
fn persisted_state_validation_rejects_inconsistent_aggregates_and_receipts() {
    let (p, a, _) = seeded(10, 0);
    let mut bad = p;
    bad.loan_count = 1;
    assert_eq!(validate_pool(&bad), Err(Error::InvalidState));
    let mut bad = p;
    bad.retired = true;
    assert_eq!(validate_pool(&bad), Err(Error::InvalidState));
    let mut bad = a;
    bad.fraction = SOL_SCALE;
    assert_eq!(validate_lender(&p, &bad), Err(Error::InvalidState));
    let mut bad = a;
    bad.checkpoint = [1, 0, 0, 0];
    assert_eq!(validate_lender(&p, &bad), Err(Error::InvalidState));
    let mut bad = a;
    bad.generation = 1;
    assert_eq!(validate_lender(&p, &bad), Err(Error::WrongGeneration));
    let mut bad = a;
    bad.pending_sol = 1;
    assert_eq!(validate_lender(&p, &bad), Err(Error::InvalidState));
}

#[test]
fn deterministic_multi_lender_sequences_conserve_all_paid_income_and_principal() {
    for n in 1..=16u64 {
        let mut p = Pool::new(n);
        let mut lenders = [Lender::new(n); 8];
        for (i, l) in lenders.iter_mut().enumerate() {
            deposit(&mut p, l, 100 + (i as u64) * n).unwrap();
        }
        let initial = p.cash;
        let mut credit = 0;
        let mut paid = 0;
        for step in 0..100u64 {
            let q = 1 + step % 17;
            borrow(&mut p, q).unwrap();
            credit_income(&mut p, 1 + step % 23).unwrap();
            credit += 1 + step % 23;
            if step % 4 == 0 {
                paid += claim(&mut p, &mut lenders[(step as usize) % 8]).unwrap();
            }
            if step % 9 == 0 {
                default(&mut p, q, step % 7).unwrap();
                credit += step % 7;
            } else {
                repay(&mut p, q).unwrap();
            }
        }
        let remaining = p.cash;
        let mut tokens = 0;
        for l in &mut lenders {
            let shares = l.shares;
            let w = withdraw(&mut p, l, shares).unwrap();
            tokens += w.tokens;
            paid += w.sol;
        }
        assert_eq!(tokens, remaining);
        assert!(tokens < initial);
        assert_eq!(paid, credit);
        assert_eq!(
            (
                p.cash,
                p.principal_out,
                p.sol_reserve,
                p.shares,
                p.loan_count
            ),
            (0, 0, 0, 0, 0)
        );
    }
}

#[test]
fn wide_mul_div_vectors_cover_high_limbs_and_remainders() {
    let cases = [
        (u128::MAX, u128::MAX, u128::MAX),
        (u128::MAX, 2, 2),
        (1 << 100, 1 << 100, 1 << 100),
        (u128::MAX, 1, u128::MAX - 1),
    ];
    for (a, b, d) in cases {
        let product = U256::from_product(a, b);
        let (q, rem) = product.div_rem_u128(d).unwrap();
        assert_eq!(
            q.checked_mul(U256::from_u128(d))
                .unwrap()
                .checked_add(U256::from_u128(rem))
                .unwrap(),
            product
        );
        assert!(rem < d);
    }
    for a in 0..50u128 {
        for b in 0..50u128 {
            for d in 1..20u128 {
                let (q, rem) = U256::from_product(a, b).div_rem_u128(d).unwrap();
                assert_eq!(q.to_u128().unwrap(), a * b / d);
                assert_eq!(rem, a * b % d);
            }
        }
    }
}
