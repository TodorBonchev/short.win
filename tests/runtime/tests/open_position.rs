//! Actual new-program Jupiter opening and post-route economic-limit rollback.
use anchor_lang::prelude::Pubkey;
use shortwin_simple::{
    accounts, instruction, route,
    state::{Exposure, Loan, Market, PoolGeneration, PriceReference},
};
use shortwin_simple_runtime_tests::{
    assert_ok, env as pda, jupiter,
    position_case::{exposure, instruction, loan, PositionCase as Case, Q},
};
use solana_signer::Signer;
#[test]
fn real_jupiter_open_books_principal_and_pays_fixed_upfront_fee_exactly() {
    let mut c = Case::new();
    let ix = c.ix(c.args());
    let borrower = c.borrower.pubkey();
    let pool_before = c.env.lamports(&c.pool);
    let borrower_before = c.env.lamports(&borrower);
    let treasury = c.env.fee_recipient.pubkey();
    let treasury_before = c.env.lamports(&treasury);
    let base_before = c.env.token_amount(&c.route.base_vault);
    let wsol_before = c.env.token_amount(&c.route.wsol_vault);
    let (result, wire) = c.env.send_v0(&[ix], &[&c.borrower]);
    let metadata = assert_ok(result);
    println!(
        "actualJupiteropenwire={wire} compute={}",
        metadata.compute_units_consumed
    );
    assert!(
        metadata
            .logs
            .iter()
            .any(|l| l == &format!("Program {} success", jupiter::CPMM_ID)),
        "actual CPMM inner execution"
    );
    let key = loan(c.pool, borrower, 0);
    let l: Loan = c.env.read(&key);
    let p: PoolGeneration = c.env.read(&c.pool);
    let m: Market = c.env.read(&c.market);
    let e: Exposure = c.env.read(&exposure(c.pool, borrower));
    let r: PriceReference = c.env.read(&c.reference);
    assert_eq!(
        (
            l.q,
            p.cash,
            p.principal_out,
            p.loan_count,
            m.next_loan_nonce
        ),
        (Q, 9 * Q, Q, 1, 1)
    );
    assert_eq!((e.q, e.notional), (Q, l.opening_notional));
    assert_eq!(l.policy, pda::policy(1));
    assert_eq!(
        l.opening_notional,
        l.sale_proceeds
            .max(r.live_ask_upper)
            .max(r.history_ask_upper_max)
    );
    let required = (u128::from(l.opening_notional) * 20000).div_ceil(10000) as u64;
    assert_eq!(
        (l.margin, l.collateral),
        (required - l.sale_proceeds, required)
    );
    let fee = (u128::from(l.opening_notional) * 100).div_ceil(10000) as u64;
    assert_eq!(l.term_fee, fee);
    assert_eq!(l.protocol_fee, fee / 10);
    assert_eq!(l.lender_fee, fee - fee / 10);
    assert_eq!(p.sol_reserve, l.lender_fee);
    assert_eq!(c.env.lamports(&c.pool), pool_before + l.lender_fee);
    assert_eq!(c.env.lamports(&treasury), treasury_before + l.protocol_fee);
    let rent = c.env.svm.minimum_balance_for_rent_exemption(Loan::SPACE);
    let exposure_rent = c
        .env
        .svm
        .minimum_balance_for_rent_exemption(Exposure::SPACE);
    assert_eq!(c.env.lamports(&key), rent + l.collateral);
    assert_eq!(
        borrower_before - c.env.lamports(&borrower),
        rent + exposure_rent + l.margin + l.term_fee
    );
    assert_eq!(c.env.token_amount(&pda::token_vault(&c.pool)), 9 * Q);
    assert_eq!(c.env.token_amount(&c.route.base_vault), base_before + Q);
    assert_eq!(
        wsol_before - c.env.token_amount(&c.route.wsol_vault),
        l.sale_proceeds
    );
    assert_eq!(
        (l.deadline - l.opened_at, l.default_at - l.deadline),
        (3600, 86400)
    );
    for seed in [route::BASE_SEED, route::WSOL_SEED] {
        let scratch = Pubkey::find_program_address(&[seed, key.as_ref()], &shortwin_simple::ID).0;
        assert!(c.env.svm.get_account(&scratch).is_none());
    }
    // Lender fee is actually claimable immediately; time never books receivables.
    let accounts = accounts::ClaimSol {
        owner: c.lender.pubkey(),
        pool: c.pool,
        receipt: pda::lender(&c.pool, &c.lender.pubkey()),
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    let before = c.env.lamports(&c.lender.pubkey());
    assert_ok(c.env.send(
        &[instruction(
            accounts,
            instruction::ClaimSol {
                min_sol: l.lender_fee,
            },
        )],
        &[&c.lender],
    ));
    assert_eq!(c.env.lamports(&c.lender.pubkey()) - before, l.lender_fee);
}

#[test]
fn signed_max_fee_margin_and_market_nonce_refuse_atomically() {
    let mut c = Case::new();
    for mode in 0..3 {
        let mut args = c.args();
        if mode == 0 {
            args.max_term_fee = 0;
        } else if mode == 1 {
            args.max_margin = 0;
        } else {
            args.nonce = 7;
        }
        let keys = c.protected(args.nonce);
        let before = c.snapshot(&keys);
        let ix = c.ix(args);
        let (result, wire) = c.env.send_v0(&[ix], &[&c.borrower]);
        assert!(wire <= 1232);
        let e = result.expect_err("signed limit/nonce must refuse");
        assert_eq!(
            c.snapshot(&keys),
            before,
            "whole DEX and lender/borrower ledger rollback mode{mode}"
        );
        if mode < 2 {
            assert!(
                e.meta
                    .logs
                    .iter()
                    .any(|l| l == &format!("Program {} success", jupiter::CPMM_ID)),
                "maxbound refusal occurs after real inner swap then rolls back"
            );
        }
    }
}
