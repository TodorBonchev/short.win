//! Actual-SBF lender and generation handlers with real SPL/T22 transfers.
//! SOL-income and wiped-generation starting states are explicitly constructed
//! post-operation fixtures below. They qualify claim/roll behavior, not a prior
//! Jupiter loan sale or default handler (covered by separate execution tests).
// Preserve LiteSVM failed-transaction logs and metadata intact in this test harness.
#![allow(clippy::result_large_err)]
use anchor_lang::{prelude::Pubkey, AccountSerialize, InstructionData, ToAccountMetas};
use shortwin_simple::{
    accounts, instruction,
    state::{LenderReceipt, Market, PoolGeneration},
};
use shortwin_simple_runtime_tests::{assert_ok, deterministic_key, Env};
use shortwin_simple_runtime_tests::{
    env as pda,
    tokens::{self, TokenProgram},
};
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_system_interface::instruction as system;

fn ix(accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
    Instruction {
        program_id: shortwin_simple::ID,
        accounts: accounts.to_account_metas(None),
        data: data.data(),
    }
}
struct Lp {
    owner: Keypair,
    tokens: Pubkey,
}
struct Fixture {
    env: Env,
    mint: Pubkey,
    market: Pubkey,
    pool: Pubkey,
    program: TokenProgram,
    a: Lp,
    b: Lp,
}
fn fixture(program: TokenProgram) -> Fixture {
    let mut env = Env::initialized();
    let authority = deterministic_key(10);
    let mint = deterministic_key(11);
    let mint = tokens::create_mint(&mut env, &mint, &authority.pubkey(), None, 6, program);
    let creator = deterministic_key(12);
    env.fund(creator.pubkey(), 1_000_000_000);
    assert_ok(env.register_market(&creator, mint, program.id(), 10_000, 100_000_000));
    let mut lp = |tag| {
        let owner = deterministic_key(tag);
        env.fund(owner.pubkey(), 1_000_000_000);
        let account = deterministic_key(tag + 1);
        let account = tokens::create_account(&mut env, &account, &mint, &owner.pubkey(), program);
        tokens::mint_to(&mut env, &mint, &account, &authority, 1_000_000, program);
        Lp {
            owner,
            tokens: account,
        }
    };
    let a = lp(20);
    let b = lp(30);
    let market = pda::market(&mint);
    let pool = pda::pool(&market, 0);
    Fixture {
        env,
        mint,
        market,
        pool,
        program,
        a,
        b,
    }
}
fn deposit(
    f: &mut Fixture,
    who: bool,
    amount: u64,
    min_shares: u128,
) -> shortwin_simple_runtime_tests::TxResult {
    let lp = if who { &f.b } else { &f.a };
    let accounts = accounts::DepositTokens {
        owner: lp.owner.pubkey(),
        market: f.market,
        pool: f.pool,
        receipt: pda::lender(&f.pool, &lp.owner.pubkey()),
        mint: f.mint,
        owner_tokens: lp.tokens,
        token_vault: pda::token_vault(&f.pool),
        token_program: f.program.id(),
        system_program: anchor_lang::system_program::ID,
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    f.env.send(
        &[ix(
            accounts,
            instruction::Deposit {
                amount,
                min_shares,
                max_receipt_rent: 100_000_000,
            },
        )],
        &[&lp.owner],
    )
}
fn withdraw(
    f: &mut Fixture,
    who: bool,
    pool: Pubkey,
    shares: u128,
    min_tokens: u64,
    min_sol: u64,
) -> shortwin_simple_runtime_tests::TxResult {
    let lp = if who { &f.b } else { &f.a };
    let accounts = accounts::WithdrawTokens {
        owner: lp.owner.pubkey(),
        market: f.market,
        pool,
        receipt: pda::lender(&pool, &lp.owner.pubkey()),
        mint: f.mint,
        owner_tokens: lp.tokens,
        token_vault: pda::token_vault(&pool),
        token_program: f.program.id(),
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    f.env.send(
        &[ix(
            accounts,
            instruction::Withdraw {
                shares,
                min_tokens,
                min_sol,
            },
        )],
        &[&lp.owner],
    )
}
fn claim(
    f: &mut Fixture,
    who: bool,
    pool: Pubkey,
    min_sol: u64,
) -> shortwin_simple_runtime_tests::TxResult {
    let lp = if who { &f.b } else { &f.a };
    let accounts = accounts::ClaimSol {
        owner: lp.owner.pubkey(),
        pool,
        receipt: pda::lender(&pool, &lp.owner.pubkey()),
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    f.env.send(
        &[ix(accounts, instruction::ClaimSol { min_sol })],
        &[&lp.owner],
    )
}
fn store_pool(env: &mut Env, key: Pubkey, pool: &PoolGeneration) {
    let mut a = env.svm.get_account(&key).unwrap();
    let mut bytes = vec![];
    pool.try_serialize(&mut bytes).unwrap();
    assert_eq!(a.data.len(), bytes.len());
    a.data = bytes;
    env.svm.set_account(key, a).unwrap();
}
fn fixture_paid_income(f: &mut Fixture, amount: u64) {
    // Real lamports transfer plus explicit matching test-only accounting fixture.
    let transfer = system::transfer(&f.env.payer.pubkey(), &f.pool, amount);
    assert_ok(f.env.send(&[transfer], &[]));
    let mut pool: PoolGeneration = f.env.read(&f.pool);
    let mut core = pool.core();
    shortwin_pooled_core::credit_income(&mut core, amount).unwrap();
    pool.store(&core).unwrap();
    store_pool(&mut f.env, f.pool, &pool);
}
fn snapshot(env: &Env, keys: &[Pubkey]) -> Vec<Option<solana_account::Account>> {
    keys.iter().map(|k| env.svm.get_account(k)).collect()
}

#[test]
fn spl_and_t22_deposit_withdraw_move_exact_principal_and_obey_signed_minima() {
    for program in TokenProgram::BOTH {
        let mut f = fixture(program);
        let pool = f.pool;
        let receipt = pda::lender(&pool, &f.a.owner.pubkey());
        let keys = [
            pool,
            pda::token_vault(&pool),
            f.a.tokens,
            receipt,
            f.a.owner.pubkey(),
        ];
        let before = snapshot(&f.env, &keys);
        assert!(deposit(&mut f, false, 700, 701).is_err());
        assert_eq!(snapshot(&f.env, &keys), before);
        assert_ok(deposit(&mut f, false, 700, 700));
        let p: PoolGeneration = f.env.read(&pool);
        let r: LenderReceipt = f.env.read(&receipt);
        assert_eq!(
            (p.cash, p.principal_out, p.shares, r.shares),
            (700, 0, 700, 700)
        );
        assert_eq!(f.env.token_amount(&pda::token_vault(&pool)), 700);
        assert_eq!(f.env.token_amount(&f.a.tokens), 999_300);
        let before = snapshot(&f.env, &keys);
        assert!(withdraw(&mut f, false, pool, 200, 201, 0).is_err());
        assert_eq!(snapshot(&f.env, &keys), before);
        assert_ok(withdraw(&mut f, false, pool, 200, 200, 0));
        let p: PoolGeneration = f.env.read(&pool);
        assert_eq!((p.cash, p.shares), (500, 500));
        let rent = f.env.lamports(&receipt);
        let owner_before = f.env.lamports(&f.a.owner.pubkey());
        assert_ok(withdraw(&mut f, false, pool, 500, 500, 0));
        assert!(f.env.svm.get_account(&receipt).is_none());
        assert_eq!(f.env.lamports(&f.a.owner.pubkey()), owner_before + rent);
        let p: PoolGeneration = f.env.read(&pool);
        assert_eq!(
            (p.cash, p.principal_out, p.shares, p.sol_reserve),
            (0, 0, 0, 0)
        );
        assert_eq!(f.env.token_amount(&f.a.tokens), 1_000_000);
    }
}

#[test]
fn registration_fee_rent_and_plain_mint_rules_are_actual_handler_checks() {
    for program in TokenProgram::BOTH {
        let mut env = Env::initialized();
        let authority = deterministic_key(70);
        let mint = deterministic_key(71);
        let mint = tokens::create_mint(&mut env, &mint, &authority.pubkey(), None, 9, program);
        let creator = deterministic_key(72);
        env.fund(creator.pubkey(), 1_000_000_000);
        let fee_recipient = env.fee_recipient.pubkey();
        let before = env.lamports(&fee_recipient);
        assert!(env
            .register_market(&creator, mint, program.id(), 9_999, 100_000_000)
            .is_err());
        assert!(env.svm.get_account(&pda::market(&mint)).is_none());
        assert!(env
            .register_market(&creator, mint, program.id(), 10_000, 0)
            .is_err());
        assert!(env.svm.get_account(&pda::market(&mint)).is_none());
        assert_eq!(env.lamports(&fee_recipient), before);
        assert_ok(env.register_market(&creator, mint, program.id(), 10_000, 100_000_000));
        assert_eq!(env.lamports(&fee_recipient), before + 10_000);
        let market: Market = env.read(&pda::market(&mint));
        assert_eq!(
            (market.mint, market.decimals, market.active_generation),
            (mint, 9, 0)
        );
        let frozen = deterministic_key(73);
        let frozen = tokens::create_mint(
            &mut env,
            &frozen,
            &authority.pubkey(),
            Some(&authority.pubkey()),
            6,
            program,
        );
        assert!(env
            .register_market(&creator, frozen, program.id(), 10_000, 100_000_000)
            .is_err());
        assert!(env.svm.get_account(&pda::market(&frozen)).is_none());
    }
}

#[test]
fn actual_claims_preserve_fraction_and_last_exit_pays_tracked_dust() {
    let mut f = fixture(TokenProgram::Spl);
    let pool = f.pool;
    assert_ok(deposit(&mut f, false, 1, 1));
    assert_ok(deposit(&mut f, true, 2, 2));
    fixture_paid_income(&mut f, 1);
    let a = f.a.owner.pubkey();
    let b = f.b.owner.pubkey();
    let before_a = f.env.lamports(&a);
    let before_b = f.env.lamports(&b);
    assert_ok(claim(&mut f, false, pool, 0));
    assert_ok(claim(&mut f, true, pool, 0));
    assert_eq!(f.env.lamports(&a), before_a);
    assert_eq!(f.env.lamports(&b), before_b);
    let ar: LenderReceipt = f.env.read(&pda::lender(&pool, &a));
    let br: LenderReceipt = f.env.read(&pda::lender(&pool, &b));
    assert!(ar.fraction > 0 && br.fraction > 0);
    let before = snapshot(&f.env, &[pool, pda::lender(&pool, &a), a]);
    assert!(claim(&mut f, false, pool, 1).is_err());
    assert_eq!(snapshot(&f.env, &[pool, pda::lender(&pool, &a), a]), before);
    assert_ok(withdraw(&mut f, false, pool, 1, 1, 0));
    assert!(f.env.svm.get_account(&pda::lender(&pool, &a)).is_none());
    let rent = f.env.lamports(&pda::lender(&pool, &b));
    let before = f.env.lamports(&b);
    assert_ok(withdraw(&mut f, true, pool, 2, 2, 1));
    assert_eq!(f.env.lamports(&b), before + rent + 1);
    let p: PoolGeneration = f.env.read(&pool);
    assert_eq!((p.shares, p.sol_reserve), (0, 0));
    // Reusing this empty non-retired pool starts its new cohort at the current index.
    assert_ok(deposit(&mut f, false, 3, 3));
    assert_ok(claim(&mut f, false, pool, 0));
    let r: LenderReceipt = f.env.read(&pda::lender(&pool, &a));
    assert_eq!((r.fraction, r.pending_sol), (0, 0));
}

#[test]
fn actual_roll_and_old_claims_separate_wiped_generation_from_new_deposits() {
    let mut f = fixture(TokenProgram::Spl);
    let old = f.pool;
    assert_ok(deposit(&mut f, false, 3, 3));
    // Explicit synthetic post-default ledger, not an invocation of the loan handlers.
    let transfer = system::transfer(&f.env.payer.pubkey(), &old, 5);
    assert_ok(f.env.send(&[transfer], &[]));
    let mut p: PoolGeneration = f.env.read(&old);
    let mut core = p.core();
    shortwin_pooled_core::borrow(&mut core, 3).unwrap();
    shortwin_pooled_core::default(&mut core, 3, 5).unwrap();
    p.store(&core).unwrap();
    store_pool(&mut f.env, old, &p);
    let vault = pda::token_vault(&old);
    let mut a = f.env.svm.get_account(&vault).unwrap();
    a.data[64..72].copy_from_slice(&0u64.to_le_bytes());
    f.env.svm.set_account(vault, a).unwrap();
    let new = pda::pool(&f.market, 1);
    let accounts = accounts::RollPool {
        payer: f.env.payer.pubkey(),
        market: f.market,
        old_pool: old,
        new_pool: new,
        mint: f.mint,
        token_vault: pda::token_vault(&new),
        token_program: f.program.id(),
        system_program: anchor_lang::system_program::ID,
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    assert_ok(f.env.send(
        &[ix(
            accounts,
            instruction::RollPool {
                max_setup_rent: 100_000_000,
            },
        )],
        &[],
    ));
    let old_state: PoolGeneration = f.env.read(&old);
    let new_state: PoolGeneration = f.env.read(&new);
    let market: Market = f.env.read(&f.market);
    assert!(old_state.retired);
    assert_eq!(old_state.sol_reserve, 5);
    assert_eq!(
        (
            new_state.generation,
            new_state.cash,
            new_state.shares,
            new_state.sol_reserve,
            new_state.sol_index
        ),
        (1, 0, 0, 0, [0; 4])
    );
    assert_eq!(market.active_generation, 1);
    f.pool = new;
    assert_ok(deposit(&mut f, true, 5, 5));
    assert_ok(claim(&mut f, true, new, 0));
    let owner = f.a.owner.pubkey();
    let before = f.env.lamports(&owner);
    assert_ok(claim(&mut f, false, old, 4));
    assert_eq!(f.env.lamports(&owner), before + 4);
    let p: PoolGeneration = f.env.read(&old);
    assert_eq!(p.sol_reserve, 1);
    let rent = f.env.lamports(&pda::lender(&old, &owner));
    let before = f.env.lamports(&owner);
    assert_ok(withdraw(&mut f, false, old, 3, 0, 1));
    assert_eq!(f.env.lamports(&owner), before + rent + 1);
    assert_ok(withdraw(&mut f, true, new, 5, 5, 0));
    let p: PoolGeneration = f.env.read(&new);
    assert_eq!((p.cash, p.shares, p.sol_reserve), (0, 0, 0));
}

#[test]
fn unsolicited_tokens_remain_untracked_and_in_their_original_generation() {
    for program in TokenProgram::BOTH {
        let mut f = fixture(program);
        let old = f.pool;
        assert_ok(deposit(&mut f, false, 10, 10));
        let donate = litesvm_token::spl_token::instruction::transfer_checked(
            &program.id(),
            &f.a.tokens,
            &f.mint,
            &pda::token_vault(&old),
            &f.a.owner.pubkey(),
            &[],
            7,
            6,
        )
        .unwrap();
        assert_ok(f.env.send(&[donate], &[&f.a.owner]));
        let state: PoolGeneration = f.env.read(&old);
        assert_eq!((state.cash, state.shares), (10, 10));
        assert_eq!(f.env.token_amount(&pda::token_vault(&old)), 17);
        assert_ok(withdraw(&mut f, false, old, 10, 10, 0));
        assert_eq!(f.env.token_amount(&pda::token_vault(&old)), 7);
        let state: PoolGeneration = f.env.read(&old);
        assert_eq!((state.cash, state.shares), (0, 0));
        assert_ok(deposit(&mut f, false, 3, 3));
        let mut p: PoolGeneration = f.env.read(&old);
        let mut core = p.core();
        shortwin_pooled_core::borrow(&mut core, 3).unwrap();
        shortwin_pooled_core::default(&mut core, 3, 0).unwrap();
        p.store(&core).unwrap();
        store_pool(&mut f.env, old, &p);
        let vault = pda::token_vault(&old);
        let mut account = f.env.svm.get_account(&vault).unwrap();
        account.data[64..72].copy_from_slice(&7u64.to_le_bytes());
        f.env.svm.set_account(vault, account).unwrap();
        let new = pda::pool(&f.market, 1);
        let accounts = accounts::RollPool {
            payer: f.env.payer.pubkey(),
            market: f.market,
            old_pool: old,
            new_pool: new,
            mint: f.mint,
            token_vault: pda::token_vault(&new),
            token_program: program.id(),
            system_program: anchor_lang::system_program::ID,
            event_authority: pda::event_authority(),
            program: shortwin_simple::ID,
        };
        assert_ok(f.env.send(
            &[ix(
                accounts,
                instruction::RollPool {
                    max_setup_rent: 100_000_000,
                },
            )],
            &[],
        ));
        assert_eq!(f.env.token_amount(&vault), 7);
        assert_eq!(f.env.token_amount(&pda::token_vault(&new)), 0);
        let state: PoolGeneration = f.env.read(&new);
        assert_eq!((state.cash, state.shares), (0, 0));
    }
}
