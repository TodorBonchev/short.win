//! Real program whole-token repayment without route/oracle accounts.
use crate::common::*;
use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::AccountMeta;
use shortwin::events::Repay;
use shortwin::state::{Market, ShortPosition, WalletExposure};
use shortwin::{PauseScope, errors::ShortwinError as E};
use shortwin_litesvm_tests::{
    engine::repay_short_ix,
    ix,
    mock::{Knobs, update_knobs},
    tokens,
};
use solana_signer::Signer;

fn source(env: &mut Env, mm: &MockMarket, b: &Borrower, amount: u64) -> Pubkey {
    let payer = env.payer.insecure_clone();
    let account =
        tokens::create_token_account(&mut env.svm, &payer, &mm.mint(), &b.key(), mm.spec.program);
    tokens::mint_to(
        &mut env.svm,
        &payer,
        &mm.mint(),
        &account,
        &mm.pool.mint_authority,
        amount,
        mm.spec.program,
    );
    account
}
fn opened(program: TokenProgram, amount: u64) -> (Env, MockMarket, Borrower, u64, Pubkey) {
    let (mut env, mm) = setup_with(spec_on(program));
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &mm, &mut b, Q);
    env.tick(&mm, 2);
    let src = source(&mut env, &mm, &b, amount);
    (env, mm, b, nonce, src)
}
fn market(env: &Env, mm: &MockMarket) -> Market {
    read::zc(&env.svm, &mm.market()).unwrap()
}
fn attempt(
    env: &mut Env,
    mm: &MockMarket,
    b: &Borrower,
    nonce: u64,
    src: Pubkey,
    max_rent: u64,
) -> TxResult {
    env.send(
        &[repay_short_ix(mm, &b.key(), &src, nonce, max_rent)],
        &[&b.kp],
    )
}
#[test]
fn repays_all_tokens_under_both_token_programs_and_releases_exact_debt() {
    for program in TokenProgram::BOTH {
        let (mut env, mm, b, nonce, src) = opened(program, Q);
        let pool_before = market(&env, &mm).pool();
        let vault_before = read::token_amount(&env.svm, &mm.keys.pool_vault).unwrap();
        let venue_before = env.svm.get_account(&mm.pool.keys.pool).unwrap();
        let pos = env.position(&mm, &b.key(), nonce);
        let meta = ok(attempt(&mut env, &mm, &b, nonce, src, u64::MAX));
        let ev: Repay = events::one(&meta);
        assert_eq!(ev.q, Q);
        assert_eq!(ev.borrower, b.key());
        assert_eq!(ev.borrower_payout, pos.v - ev.rent);
        assert!(ev.rent_protocol <= ev.rent);
        assert_eq!(read::token_amount(&env.svm, &src), Some(0));
        assert_eq!(
            read::token_amount(&env.svm, &mm.keys.pool_vault),
            Some(vault_before + Q)
        );
        let after = market(&env, &mm);
        assert_eq!(after.pool().borrowed_q, pool_before.borrowed_q - Q);
        assert_eq!(after.pool().tokens_free, pool_before.tokens_free + Q);
        assert_eq!(after.open_positions, 0);
        assert!(read::borsh::<ShortPosition>(&env.svm, &ev.position).is_none());
        assert!(
            read::borsh::<WalletExposure>(&env.svm, &pda::exposure(&mm.market(), &b.key()).0)
                .is_none()
        );
        assert_eq!(
            env.svm.get_account(&mm.pool.keys.pool).unwrap(),
            venue_before
        );
    }
}
#[test]
fn works_while_opens_are_paused_and_the_route_is_unusable() {
    let (mut env, mm, b, nonce, src) = opened(TokenProgram::Spl, Q);
    let ea = env.pause_authority.insecure_clone();
    ok(env.send(
        &[ix::set_pause(&ea.pubkey(), PauseScope::Global, true, None)],
        &[&ea],
    ));
    update_knobs(&mut env, &mm.pool, |k: &mut Knobs| {
        k.disabled = true;
        k.layout_version = 1;
    });
    // Neither the venue nor oracle/guard accounts is supplied to repayment.
    let instruction = repay_short_ix(&mm, &b.key(), &src, nonce, u64::MAX);
    assert!(
        !instruction
            .accounts
            .iter()
            .any(|m| m.pubkey == mm.pool.keys.pool)
    );
    assert!(
        !instruction
            .accounts
            .iter()
            .any(|m| m.pubkey == pda::oracle_cache())
    );
    assert!(
        !instruction
            .accounts
            .iter()
            .any(|m| m.pubkey == pda::venue_guard())
    );
    ok(env.send(&[instruction], &[&b.kp]));
}
#[test]
fn too_few_tokens_does_not_release_debt_or_commit_accrual() {
    for program in TokenProgram::BOTH {
        let (mut env, mm, b, nonce, src) = opened(program, Q - 1);
        let keys = [
            mm.market(),
            pda::position(&mm.market(), &b.key(), nonce).0,
            mm.keys.market_vault,
            mm.keys.pool_vault,
            src,
        ];
        let before: Vec<_> = keys.iter().map(|k| env.svm.get_account(k)).collect();
        assert!(attempt(&mut env, &mm, &b, nonce, src, u64::MAX).is_err());
        let after: Vec<_> = keys.iter().map(|k| env.svm.get_account(k)).collect();
        assert_eq!(before, after);
    }
}
#[test]
fn enforces_borrower_rent_bound_before_any_token_debit() {
    let (mut env, mm, b, nonce, src) = opened(TokenProgram::Spl, Q);
    env.tick(&mm, 3);
    let before = env.svm.get_account(&src);
    assert_err(
        &attempt(&mut env, &mm, &b, nonce, src, 0),
        E::RepaymentRentAboveMax,
    );
    assert_eq!(env.svm.get_account(&src), before);
    ok(attempt(&mut env, &mm, &b, nonce, src, u64::MAX));
}
#[test]
fn refuses_foreign_owner_and_caller_supplied_remaining_accounts() {
    let (mut env, mm, b, nonce, src) = opened(TokenProgram::Spl, Q);
    let other = env.borrower();
    let mut wrong = repay_short_ix(&mm, &other.key(), &src, nonce, u64::MAX);
    wrong.accounts[3].pubkey = pda::position(&mm.market(), &b.key(), nonce).0;
    assert!(env.send(&[wrong], &[&other.kp]).is_err());
    let mut extras = repay_short_ix(&mm, &b.key(), &src, nonce, u64::MAX);
    extras
        .accounts
        .push(AccountMeta::new_readonly(mm.pool.keys.pool, false));
    assert_err(&env.send(&[extras], &[&b.kp]), E::AccountMismatch);
    ok(attempt(&mut env, &mm, &b, nonce, src, u64::MAX));
}
#[test]
fn preexisting_pool_donations_cannot_substitute_for_actual_repayment() {
    let (mut env, mm, b, nonce, src) = opened(TokenProgram::Spl, 0);
    let payer = env.payer.insecure_clone();
    tokens::mint_to(
        &mut env.svm,
        &payer,
        &mm.mint(),
        &mm.keys.pool_vault,
        &mm.pool.mint_authority,
        Q,
        TokenProgram::Spl,
    );
    let before = market(&env, &mm).pool();
    assert!(attempt(&mut env, &mm, &b, nonce, src, u64::MAX).is_err());
    assert_eq!(market(&env, &mm).pool(), before);
    assert!(
        read::borsh::<ShortPosition>(&env.svm, &pda::position(&mm.market(), &b.key(), nonce).0)
            .is_some()
    );
}

#[test]
fn sibling_position_and_wallet_exposure_survive_a_single_repayment() {
    let (mut env, mm) = setup();
    let mut b = env.borrower();
    let q = Q / 3; // Two positions together remain within the actual wallet cap.
    let (_, first, _) = open_ok(&mut env, &mm, &mut b, q);
    env.tick(&mm, 2);
    let src = source(&mut env, &mm, &b, q);
    let (_, second, _) = open_ok(&mut env, &mm, &mut b, q);
    let sibling_key = pda::position(&mm.market(), &b.key(), second).0;
    let sibling_before = env.svm.get_account(&sibling_key).unwrap();
    let exposure_key = pda::exposure(&mm.market(), &b.key()).0;
    assert_eq!(
        read::borsh::<WalletExposure>(&env.svm, &exposure_key)
            .unwrap()
            .q_open,
        2 * q
    );
    ok(attempt(&mut env, &mm, &b, first, src, u64::MAX));
    assert_eq!(
        read::borsh::<WalletExposure>(&env.svm, &exposure_key)
            .unwrap()
            .q_open,
        q
    );
    assert_eq!(env.svm.get_account(&sibling_key).unwrap(), sibling_before);
    assert_eq!(market(&env, &mm).open_positions, 1);
    assert_eq!(market(&env, &mm).pool().borrowed_q, q);
    let next = source(&mut env, &mm, &b, q);
    ok(attempt(&mut env, &mm, &b, second, next, u64::MAX));
    assert!(read::borsh::<WalletExposure>(&env.svm, &exposure_key).is_none());
    assert_eq!(market(&env, &mm).pool().borrowed_q, 0);
}

#[test]
fn collateral_below_accrued_rent_refuses_without_token_or_debt_changes() {
    use anchor_lang::AccountSerialize;
    let (mut env, mm, b, nonce, src) = opened(TokenProgram::Spl, Q);
    env.tick(&mm, 3);
    let pos_key = pda::position(&mm.market(), &b.key(), nonce).0;
    let mut account = env.svm.get_account(&pos_key).unwrap();
    let mut pos = env.position(&mm, &b.key(), nonce);
    // Controlled state: remove collateral, preserving position-account rent and
    // recorded accrued debt. This isolates the pre-token-CPI budget refusal.
    account.lamports -= pos.v;
    pos.v = 0;
    let mut encoded = Vec::new();
    pos.try_serialize(&mut encoded).unwrap();
    account.data[..encoded.len()].copy_from_slice(&encoded);
    env.svm.set_account(pos_key, account).unwrap();
    let keys = [
        pos_key,
        mm.market(),
        mm.keys.market_vault,
        mm.keys.pool_vault,
        src,
    ];
    let before: Vec<_> = keys.iter().map(|k| env.svm.get_account(k)).collect();
    assert_err(
        &attempt(&mut env, &mm, &b, nonce, src, u64::MAX),
        E::InsufficientVault,
    );
    assert_eq!(
        before,
        keys.iter()
            .map(|k| env.svm.get_account(k))
            .collect::<Vec<_>>()
    );
}

#[test]
fn token2022_fee_net_shortfall_reverts_even_with_existing_vault_donation() {
    use shortwin_litesvm_tests::fixtures::mints;
    use spl_token_2022_interface::extension::{
        BaseStateWithExtensionsMut, ExtensionType, StateWithExtensionsMut,
        transfer_fee::TransferFeeConfig,
    };
    use spl_token_2022_interface::state::Mint;
    let (mut env, mm, b, nonce, src) = opened(TokenProgram::Token2022, Q);
    let payer = env.payer.insecure_clone();
    tokens::mint_to(
        &mut env.svm,
        &payer,
        &mm.mint(),
        &mm.keys.pool_vault,
        &mm.pool.mint_authority,
        Q,
        TokenProgram::Token2022,
    );
    // Controlled post-open mint change: real initialized extension encoding, with
    // the original base mint fields preserved. This models a newly unsafe mint.
    let fee_mint = mints::t22_mint(&mut env, &[ExtensionType::TransferFeeConfig], false);
    let old_mint = env.svm.get_account(&mm.mint()).unwrap();
    let mut changed = env.svm.get_account(&fee_mint.mint).unwrap();
    changed.data[..82].copy_from_slice(&old_mint.data[..82]);
    {
        let mut mint = StateWithExtensionsMut::<Mint>::unpack(&mut changed.data).unwrap();
        let fees = mint.get_extension_mut::<TransferFeeConfig>().unwrap();
        fees.older_transfer_fee.epoch = 0u64.into();
        fees.older_transfer_fee.maximum_fee = u64::MAX.into();
        fees.older_transfer_fee.transfer_fee_basis_points = 100u16.into();
        fees.newer_transfer_fee = fees.older_transfer_fee;
    }
    env.svm.set_account(mm.mint(), changed).unwrap();
    // Required account extension: AccountType::Account + TransferFeeAmount (u64).
    // Preserve token balances and identity while making the real token CPI valid.
    for key in [src, mm.keys.pool_vault] {
        let mut account = env.svm.get_account(&key).unwrap();
        account.data.resize(178, 0);
        account.data[165] = 2;
        account.data[166..168]
            .copy_from_slice(&(ExtensionType::TransferFeeAmount as u16).to_le_bytes());
        account.data[168..170].copy_from_slice(&8u16.to_le_bytes());
        account.lamports = account.lamports.max(10_000_000);
        env.svm.set_account(key, account).unwrap();
    }
    let keys = [
        mm.market(),
        pda::position(&mm.market(), &b.key(), nonce).0,
        mm.keys.market_vault,
        mm.keys.pool_vault,
        src,
    ];
    let before: Vec<_> = keys.iter().map(|k| env.svm.get_account(k)).collect();
    // Token transfer succeeds but net lender credit is <q; the program's own
    // postcondition, not a token-program setup failure, must roll everything back.
    assert_err(
        &attempt(&mut env, &mm, &b, nonce, src, u64::MAX),
        E::VenueResultMismatch,
    );
    assert_eq!(
        before,
        keys.iter()
            .map(|k| env.svm.get_account(k))
            .collect::<Vec<_>>()
    );
}
