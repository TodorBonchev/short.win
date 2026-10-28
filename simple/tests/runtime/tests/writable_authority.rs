//! Additional actual-SBF regression for the reviewed loan-local writable mode.
//! Historical DEX/mint bytes are untouched. Authority gifts/invalid authority
//! states are explicitly synthetic protocol inputs; no live transaction is sent.
use anchor_lang::prelude::Pubkey;
use shortwin_simple::{
    route,
    state::{Exposure, Loan, PoolGeneration},
};
use shortwin_simple_runtime_tests::{
    assert_ok, env as pda, exit_case,
    position_case::{loan, PositionCase, Q},
};
use solana_account::Account;
use solana_instruction::Instruction;
use solana_signer::Signer;

fn authority(c: &PositionCase) -> Pubkey {
    Pubkey::find_program_address(
        &[
            route::AUTHORITY_SEED,
            loan(c.pool, c.borrower.pubkey(), 0).as_ref(),
        ],
        &shortwin_simple::ID,
    )
    .0
}
fn writable(mut instruction: Instruction, authority: Pubkey) -> Instruction {
    for meta in &mut instruction.accounts {
        if meta.pubkey == authority {
            meta.is_writable = true;
        }
    }
    instruction
}
fn gift(c: &mut PositionCase, amount: u64) {
    let address = authority(c);
    c.env
        .svm
        .set_account(
            address,
            Account {
                lamports: amount,
                ..Account::default()
            },
        )
        .unwrap();
}

#[test]
fn writable_authority_gifts_stay_untracked_and_return_separately_on_buyback() {
    let mut c = PositionCase::new();
    let authority = authority(&c);
    gift(&mut c, 29);
    let borrower_before = c.env.lamports(&c.borrower.pubkey());
    let mut args = c.args();
    args.route.setup_lamports = 5_000_000;
    let instruction = writable(c.ix(args), authority);
    assert_ok(c.env.send_v0(&[instruction], &[&c.borrower]).0);
    let key = loan(c.pool, c.borrower.pubkey(), 0);
    let loan: Loan = c.env.read(&key);
    let pool: PoolGeneration = c.env.read(&c.pool);
    let loan_rent = c.env.svm.minimum_balance_for_rent_exemption(Loan::SPACE);
    let exposure_rent = c
        .env
        .svm
        .minimum_balance_for_rent_exemption(Exposure::SPACE);
    assert_eq!(c.env.lamports(&authority), 0);
    assert_eq!(c.env.lamports(&key), loan_rent + loan.collateral + 29);
    assert_eq!(
        borrower_before - c.env.lamports(&c.borrower.pubkey()),
        loan_rent + exposure_rent + loan.margin + loan.term_fee
    );
    assert_eq!(
        (pool.cash, pool.principal_out, pool.sol_reserve),
        (9 * Q, Q, loan.lender_fee)
    );

    // A second independent gift before close cannot reduce booked spend, raise
    // lender SOL reserves, or be included in the loan's economic collateral.
    gift(&mut c, 43);
    let borrower_before = c.env.lamports(&c.borrower.pubkey());
    let loan_balance = c.env.lamports(&key);
    let mut instruction = writable(exit_case::close(&c, 3_000_000, Q), authority);
    // CloseArgs discriminator+maxcost+temporary, then RouteArgs.input+setup.
    instruction.data[32..40].copy_from_slice(&5_000_000u64.to_le_bytes());
    assert_ok(c.env.send_v0(&[instruction], &[&c.borrower]).0);
    let after: PoolGeneration = c.env.read(&c.pool);
    assert_eq!(c.env.lamports(&authority), 0);
    assert_eq!(
        c.env.lamports(&c.borrower.pubkey()) - borrower_before,
        loan_balance + exposure_rent + 43 - 3_000_000
    );
    assert_eq!(after.sol_reserve, pool.sol_reserve);
    assert_eq!(
        (
            after.principal_out,
            after.loan_count,
            after.booked_notional_out
        ),
        (0, 0, 0)
    );
    assert!(after.cash >= pool.cash + Q);
    assert_eq!(after.cash, c.env.token_amount(&pda::token_vault(&c.pool)));
    assert!(c.env.svm.get_account(&key).is_none());
}

#[test]
fn readonly_authority_gift_stays_outside_the_loan_and_route() {
    let mut c = PositionCase::new();
    let authority = authority(&c);
    gift(&mut c, 17);
    let borrower_before = c.env.lamports(&c.borrower.pubkey());
    let instruction = c.ix(c.args());
    assert_ok(c.env.send_v0(&[instruction], &[&c.borrower]).0);
    let key = loan(c.pool, c.borrower.pubkey(), 0);
    let loan: Loan = c.env.read(&key);
    assert_eq!(c.env.lamports(&authority), 17);
    assert_eq!(
        borrower_before - c.env.lamports(&c.borrower.pubkey()),
        c.env.svm.minimum_balance_for_rent_exemption(Loan::SPACE)
            + c.env
                .svm
                .minimum_balance_for_rent_exemption(Exposure::SPACE)
            + loan.margin
            + loan.term_fee
    );
    let instruction = exit_case::close(&c, 3_000_000, Q);
    assert_ok(c.env.send_v0(&[instruction], &[&c.borrower]).0);
    assert_eq!(c.env.lamports(&authority), 17);
}

#[test]
fn writable_foreign_or_data_bearing_authority_is_refused_atomically() {
    for bad_owner in [false, true] {
        let mut c = PositionCase::new();
        let authority = authority(&c);
        c.env
            .svm
            .set_account(
                authority,
                Account {
                    lamports: 1_000_000,
                    owner: if bad_owner {
                        shortwin_simple::ID
                    } else {
                        anchor_lang::system_program::ID
                    },
                    data: if bad_owner { vec![] } else { vec![1] },
                    ..Account::default()
                },
            )
            .unwrap();
        let keys = c.protected(0);
        let before = c.snapshot(&keys);
        let instruction = writable(c.ix(c.args()), authority);
        let error = c
            .env
            .send_v0(&[instruction], &[&c.borrower])
            .0
            .expect_err("invalid authority must refuse");
        assert!(error.meta.logs.iter().any(|l| l.contains("RouteAccounts")));
        assert_eq!(
            c.snapshot(&keys),
            before,
            "all protected DEX and protocol state rolls back"
        );
    }
}

#[test]
fn signed_setup_maximum_and_total_advance_refuse_without_spending() {
    for mode in 0..3 {
        let mut c = PositionCase::new();
        let authority = authority(&c);
        let mut args = c.args();
        args.route.setup_lamports = if mode == 1 { 5_000_001 } else { 5_000_000 };
        if mode == 0 {
            args.temporary_funding = 0;
        }
        let instruction = if mode == 2 {
            c.ix(args)
        } else {
            writable(c.ix(args), authority)
        };
        let keys = c.protected(0);
        let before = c.snapshot(&keys);
        let failure = c
            .env
            .send_v0(&[instruction], &[&c.borrower])
            .0
            .expect_err("signed setup bound must refuse");
        assert!(!failure
            .meta
            .logs
            .iter()
            .any(|l| l == &format!("Program {} invoke [2]", route::JUPITER_ID)));
        assert_eq!(c.snapshot(&keys), before);
    }
}
