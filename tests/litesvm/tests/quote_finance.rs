//! Actual SBF quote-native repayment from explicit controlled debt fixtures.
//!
//! Public quote admission/adoption is deliberately absent. This seeds its distinct
//! accounts to exercise the actual repayment handler with all Env invariant hooks
//! enabled; it does not claim a qualified quote opening or a complete recovery path.
#![allow(clippy::result_large_err)]
use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::{AccountSerialize, InstructionData, ToAccountMetas};
use shortwin::engine::quote_finance as engine;
use shortwin::errors::ShortwinError as E;
use shortwin::events::Repay;
use shortwin::state::{
    MARKET_FLAG_QUOTE_NATIVE, Market, QUOTE_FINANCE_VERSION, QuoteFinanceExposure,
    QuoteFinanceMarket, QuoteFinancePosition,
};
use shortwin_litesvm_tests::fixtures::MarketKeys;
use shortwin_litesvm_tests::tokens::TokenProgram;
use shortwin_litesvm_tests::{
    Env, TxResult,
    errors::{assert_err, ok},
    events, fixtures, invariants, ix, pda, read, tokens,
};
use shortwin_math::{
    pool,
    quote_rent::{self, LockedRates, NotionalPosition, OpeningNotional, persistence},
};
use solana_keypair::Keypair;
use solana_signer::Signer;

const Q: u64 = 1_000_000;
const LEND: u64 = 10_000_000;
struct Case {
    env: Env,
    keys: MarketKeys,
    borrower: Keypair,
    authority: Keypair,
    lender: Keypair,
    lender_source: Pubkey,
    program: TokenProgram,
    finance_key: Pubkey,
    position_key: Pubkey,
    exposure_key: Pubkey,
    source: Pubkey,
}
fn setup(program: TokenProgram, source_amount: u64) -> Case {
    setup_with_q(program, source_amount, Q)
}
fn setup_with_q(program: TokenProgram, source_amount: u64, q: u64) -> Case {
    let mut env = Env::initialized();
    let (keys, authority) = env.install_market(program, 6);
    let lender = env.funded_keypair();
    let borrower = env.funded_keypair();
    let payer = env.payer.insecure_clone();
    let lender_source =
        tokens::create_token_account(&mut env.svm, &payer, &keys.mint, &lender.pubkey(), program);
    tokens::mint_to(
        &mut env.svm,
        &payer,
        &keys.mint,
        &lender_source,
        &authority,
        LEND,
        program,
    );
    ok(env.send(
        &[ix::deposit(
            &ix::LenderKeys {
                owner: lender.pubkey(),
                mint: keys.mint,
                token_program: keys.token_program,
                owner_token_account: lender_source,
            },
            LEND,
            0,
        )],
        &[&lender],
    ));
    let source = tokens::create_token_account(
        &mut env.svm,
        &payer,
        &keys.mint,
        &borrower.pubkey(),
        program,
    );
    if source_amount > 0 {
        tokens::mint_to(
            &mut env.svm,
            &payer,
            &keys.mint,
            &source,
            &authority,
            source_amount,
            program,
        );
    }
    let (finance_key, finance_bump) = Pubkey::find_program_address(
        &[QuoteFinanceMarket::SEED, keys.market.as_ref()],
        &shortwin::ID,
    );
    let nonce = 3u64;
    let (position_key, position_bump) = Pubkey::find_program_address(
        &[
            QuoteFinancePosition::SEED,
            keys.market.as_ref(),
            borrower.pubkey().as_ref(),
            &nonce.to_le_bytes(),
        ],
        &shortwin::ID,
    );
    let (exposure_key, exposure_bump) = Pubkey::find_program_address(
        &[
            QuoteFinanceExposure::SEED,
            keys.market.as_ref(),
            borrower.pubkey().as_ref(),
        ],
        &shortwin::ID,
    );
    let now = env.now();
    let mut market = env.market(&keys.market);
    let mut finance = QuoteFinanceMarket {
        version: QUOTE_FINANCE_VERSION,
        bump: finance_bump,
        market: keys.market,
        config: pda::config(),
        model_version: 2,
        model_epoch: 1,
        policy_hash: [7; 32],
        rent_state: [0; persistence::MARKET_ENCODED_LEN],
    };
    finance
        .store(&persistence::MarketCheckpoint::empty(now as u64))
        .unwrap();
    let rent = NotionalPosition::lock(
        OpeningNotional::new(1_000_000_007).unwrap(),
        &quote_rent::Basis::zero(),
        LockedRates::new(10000, 37500, 550000),
        1500,
        1000,
        market.util_kink_bps,
        3600,
    )
    .unwrap();
    let mut position = QuoteFinancePosition {
        version: QUOTE_FINANCE_VERSION,
        borrower: borrower.pubkey(),
        market: keys.market,
        finance_market: finance_key,
        nonce,
        bump: position_bump,
        tier: 2,
        q,
        s: 1_000_000_000,
        m: 3_000_000_000,
        v: 4_000_000_000,
        n_open: 1_000_000_007,
        fee_open: 0,
        t_open: now,
        deadline: now + 2000,
        default_at: now + 2000 + shortwin::state::QUOTE_DEFAULT_GRACE_SECONDS,
        settle_keeper_fixed: 10_000_000,
        settle_keeper_bps: 100,
        settle_keeper_cap: 250_000_000,
        lump_vest_seconds: 86_400,
        model_version: finance.model_version,
        model_epoch: finance.model_epoch,
        policy_hash: finance.policy_hash,
        rent_state: [0; persistence::POSITION_ENCODED_LEN],
    };
    position.store_rent(&rent).unwrap();
    market.set_flag(MARKET_FLAG_QUOTE_NATIVE, true);
    let mut borrowed = market.pool();
    pool::borrow(&mut borrowed, now, q, position.default_at).unwrap();
    engine::register_position(
        &pda::config(),
        &keys.market,
        &finance_key,
        &mut market,
        &mut finance,
        &position,
        &borrowed,
    )
    .unwrap();
    fixtures::set_zc(&mut env.svm, &keys.market, &market, 0);
    fixtures::set_token_account(
        &mut env.svm,
        &keys.pool_vault,
        &keys.mint,
        &keys.market,
        LEND - q,
        program,
    );
    fixtures::set_borsh(
        &mut env.svm,
        &finance_key,
        &finance,
        QuoteFinanceMarket::SPACE,
        0,
    );
    fixtures::set_borsh(
        &mut env.svm,
        &position_key,
        &position,
        QuoteFinancePosition::SPACE,
        position.v,
    );
    let mut exposure = QuoteFinanceExposure {
        market: keys.market,
        wallet: borrower.pubkey(),
        q_open: 0,
        notional_open: 0,
        bump: exposure_bump,
    };
    engine::add_exposure(&mut exposure, position.q, position.n_open).unwrap();
    fixtures::set_borsh(
        &mut env.svm,
        &exposure_key,
        &exposure,
        QuoteFinanceExposure::SPACE,
        0,
    );
    invariants::check_all(&env.svm, &keys.market).unwrap();
    env.warp(1);
    Case {
        env,
        keys,
        borrower,
        authority,
        lender,
        lender_source,
        program,
        finance_key,
        position_key,
        exposure_key,
        source,
    }
}
impl Case {
    fn instruction(&self, max_rent: u64) -> Instruction {
        Instruction {
            program_id: shortwin::ID,
            accounts: shortwin::accounts::RepayQuoteShort {
                borrower: self.borrower.pubkey(),
                config: pda::config(),
                market: self.keys.market,
                finance_market: self.finance_key,
                position: self.position_key,
                exposure: self.exposure_key,
                mint: self.keys.mint,
                borrower_token_account: self.source,
                pool_vault: self.keys.pool_vault,
                market_vault: self.keys.market_vault,
                token_program: self.keys.token_program,
                event_authority: pda::event_authority(),
                program: shortwin::ID,
            }
            .to_account_metas(None),
            data: shortwin::instruction::RepayQuoteShort { max_rent }.data(),
        }
    }
    fn attempt(&mut self, max_rent: u64) -> TxResult {
        let ix = self.instruction(max_rent);
        self.env.send(&[ix], &[&self.borrower])
    }
    fn position(&self) -> QuoteFinancePosition {
        read::borsh(&self.env.svm, &self.position_key).unwrap()
    }
    fn default_instruction(&self, keeper: Pubkey) -> Instruction {
        let market: Market = read::zc(&self.env.svm, &self.keys.market).unwrap();
        Instruction {
            program_id: shortwin::ID,
            accounts: shortwin::accounts::OverdueQuoteDefault {
                keeper,
                config: pda::config(),
                market: self.keys.market,
                finance_market: self.finance_key,
                position: self.position_key,
                borrower: self.borrower.pubkey(),
                exposure: self.exposure_key,
                market_vault: self.keys.market_vault,
                share_epoch_rec: pda::share_epoch(&self.keys.market, market.share_epoch).0,
                system_program: anchor_lang::system_program::ID,
                event_authority: pda::event_authority(),
                program: shortwin::ID,
            }
            .to_account_metas(None),
            data: shortwin::instruction::OverdueQuoteDefault {
                max_epoch_record_rent: u64::MAX,
            }
            .data(),
        }
    }
    fn default(&mut self, keeper: &Keypair) -> TxResult {
        let instruction = self.default_instruction(keeper.pubkey());
        self.env.send(&[instruction], &[keeper])
    }
    fn warp_to(&mut self, time: i64) {
        let elapsed = time - self.env.now();
        self.env.warp(elapsed);
    }
    fn tracked(&self) -> Vec<Option<solana_account::Account>> {
        [
            self.keys.market,
            self.keys.market_vault,
            self.keys.pool_vault,
            self.finance_key,
            self.position_key,
            self.exposure_key,
            self.source,
        ]
        .iter()
        .map(|key| self.env.svm.get_account(key))
        .collect()
    }
}
#[test]
fn actual_sbf_repayment_is_price_independent_and_preserves_shared_lender_accounting() {
    for program in TokenProgram::BOTH {
        let mut case = setup(program, Q);
        let position: QuoteFinancePosition =
            read::borsh(&case.env.svm, &case.position_key).unwrap();
        let borrower_before = read::lamports(&case.env.svm, &case.borrower.pubkey());
        let account_rent = case
            .env
            .svm
            .minimum_balance_for_rent_exemption(QuoteFinancePosition::SPACE);
        let exposure_rent = case
            .env
            .svm
            .minimum_balance_for_rent_exemption(QuoteFinanceExposure::SPACE);
        let instruction = case.instruction(u64::MAX);
        for forbidden in [pda::oracle_cache(), pda::venue_guard()] {
            assert!(
                !instruction
                    .accounts
                    .iter()
                    .any(|meta| meta.pubkey == forbidden)
            );
        }
        assert_eq!(instruction.accounts.len(), 13);
        let metadata = ok(case.attempt(u64::MAX));
        eprintln!(
            "quote repay {:?}: {} CU",
            program, metadata.compute_units_consumed
        );
        assert!(metadata.compute_units_consumed < 200_000);
        let event: Repay = events::one(&metadata);
        assert_eq!(event.q, Q);
        assert_eq!(event.position, case.position_key);
        assert_eq!(event.borrower, case.borrower.pubkey());
        assert_eq!(event.borrower_payout + event.rent, position.v);
        assert_eq!(read::token_amount(&case.env.svm, &case.source), Some(0));
        assert_eq!(
            read::token_amount(&case.env.svm, &case.keys.pool_vault),
            Some(LEND)
        );
        assert_eq!(
            read::lamports(&case.env.svm, &case.borrower.pubkey()),
            borrower_before + event.borrower_payout + account_rent + exposure_rent
        );
        assert!(read::borsh::<QuoteFinancePosition>(&case.env.svm, &case.position_key).is_none());
        assert!(read::borsh::<QuoteFinanceExposure>(&case.env.svm, &case.exposure_key).is_none());
        let market: Market = read::zc(&case.env.svm, &case.keys.market).unwrap();
        assert_eq!(
            (
                market.open_positions,
                market.total_notional_open,
                market.borrowed_q
            ),
            (0, 0, 0)
        );
        assert_eq!(market.receivable, 0);
        let finance: QuoteFinanceMarket = read::borsh(&case.env.svm, &case.finance_key).unwrap();
        let checkpoint = finance
            .restore(&pda::config(), &case.keys.market, &market)
            .unwrap();
        assert_eq!(checkpoint.aggregate().positions(), 0);
        assert_ne!(checkpoint.carry().numerator(), 0);
        invariants::check_all(&case.env.svm, &case.keys.market).unwrap();
    }
}
#[test]
fn explicit_rent_bound_and_insufficient_token_balance_revert_every_financial_account() {
    for program in TokenProgram::BOTH {
        let mut case = setup(program, Q);
        let before = case.tracked();
        assert_err(&case.attempt(0), E::RepaymentRentAboveMax);
        assert_eq!(case.tracked(), before);
        let mut case = setup(program, Q - 1);
        let before = case.tracked();
        assert!(case.attempt(u64::MAX).is_err());
        assert_eq!(case.tracked(), before);
    }
}
#[test]
fn named_account_signer_tail_and_extra_account_refusals_do_not_release_quote_debt() {
    let mut case = setup(TokenProgram::Spl, Q);
    let before = case.tracked();
    let mut ix = case.instruction(u64::MAX);
    ix.accounts
        .push(AccountMeta::new_readonly(pda::oracle_cache(), false));
    assert_err(&case.env.send(&[ix], &[&case.borrower]), E::AccountMismatch);
    assert_eq!(case.tracked(), before);
    let mut ix = case.instruction(u64::MAX);
    ix.accounts[0].is_signer = false;
    assert!(case.env.send(&[ix], &[]).is_err());
    assert_eq!(case.tracked(), before);
    let mut malformed = case.env.svm.get_account(&case.finance_key).unwrap();
    malformed.data.push(0);
    case.env
        .svm
        .set_account(case.finance_key, malformed.clone())
        .unwrap();
    // A malformed sidecar is itself outside the market invariant; restore its exact
    // bytes before any subsequent transaction. The instruction must reject it.
    let transaction = case
        .env
        .transaction(&[case.instruction(u64::MAX)], &[&case.borrower]);
    let failure = shortwin_litesvm_tests::env::send_direct(&mut case.env.svm, |svm| {
        svm.send_transaction(transaction)
    });
    assert_err(&failure, E::AccountMismatch);
    assert_eq!(
        case.env.svm.get_account(&case.finance_key).unwrap(),
        malformed
    );
}
#[test]
fn old_pool_donations_do_not_substitute_for_new_net_q() {
    let mut case = setup(TokenProgram::Spl, 0);
    let payer = case.env.payer.insecure_clone();
    tokens::mint_to(
        &mut case.env.svm,
        &payer,
        &case.keys.mint,
        &case.keys.pool_vault,
        &case.authority,
        Q,
        case.program,
    );
    let before = case.tracked();
    assert!(case.attempt(u64::MAX).is_err());
    assert_eq!(case.tracked(), before);
}
#[test]
fn real_token2022_fee_transfer_succeeds_but_net_q_shortfall_rolls_back() {
    use shortwin_litesvm_tests::fixtures::mints;
    use spl_token_2022_interface::extension::{
        BaseStateWithExtensionsMut, ExtensionType, StateWithExtensionsMut,
        transfer_fee::TransferFeeConfig,
    };
    use spl_token_2022_interface::state::Mint;
    let mut case = setup(TokenProgram::Token2022, Q);
    let fee_mint = mints::t22_mint(&mut case.env, &[ExtensionType::TransferFeeConfig], false);
    let original = case.env.svm.get_account(&case.keys.mint).unwrap();
    let mut changed = case.env.svm.get_account(&fee_mint.mint).unwrap();
    changed.data[..82].copy_from_slice(&original.data[..82]);
    {
        let mut mint = StateWithExtensionsMut::<Mint>::unpack(&mut changed.data).unwrap();
        let fees = mint.get_extension_mut::<TransferFeeConfig>().unwrap();
        fees.older_transfer_fee.epoch = 0u64.into();
        fees.older_transfer_fee.maximum_fee = u64::MAX.into();
        fees.older_transfer_fee.transfer_fee_basis_points = 100u16.into();
        fees.newer_transfer_fee = fees.older_transfer_fee;
    }
    case.env.svm.set_account(case.keys.mint, changed).unwrap();
    for key in [case.source, case.keys.pool_vault] {
        let mut account = case.env.svm.get_account(&key).unwrap();
        account.data.resize(178, 0);
        account.data[165] = 2;
        account.data[166..168]
            .copy_from_slice(&(ExtensionType::TransferFeeAmount as u16).to_le_bytes());
        account.data[168..170].copy_from_slice(&8u16.to_le_bytes());
        account.lamports = account.lamports.max(10_000_000);
        case.env.svm.set_account(key, account).unwrap();
    }
    let before = case.tracked();
    assert_err(&case.attempt(u64::MAX), E::VenueResultMismatch);
    assert_eq!(case.tracked(), before);
}
#[test]
fn quote_invariant_hook_detects_unregistered_debt_bad_exposure_and_escrow_shortfall() {
    let mut case = setup(TokenProgram::Spl, Q);
    let original = case.env.svm.get_account(&case.position_key).unwrap();
    let mut missing = original.clone();
    missing.lamports -= 1;
    case.env
        .svm
        .set_account(case.position_key, missing)
        .unwrap();
    assert_eq!(
        invariants::check_all(&case.env.svm, &case.keys.market)
            .unwrap_err()
            .invariant,
        "I2"
    );
    case.env
        .svm
        .set_account(case.position_key, original)
        .unwrap();
    let original = case.env.svm.get_account(&case.exposure_key).unwrap();
    let mut exposure: QuoteFinanceExposure =
        read::borsh(&case.env.svm, &case.exposure_key).unwrap();
    exposure.notional_open += 1;
    let mut account = original.clone();
    let mut bytes = Vec::new();
    exposure.try_serialize(&mut bytes).unwrap();
    account.data = bytes;
    case.env
        .svm
        .set_account(case.exposure_key, account)
        .unwrap();
    assert_eq!(
        invariants::check_all(&case.env.svm, &case.keys.market)
            .unwrap_err()
            .invariant,
        "I6"
    );
    case.env
        .svm
        .set_account(case.exposure_key, original)
        .unwrap();
    let original = case.env.svm.get_account(&case.keys.market).unwrap();
    let mut market: Market = read::zc(&case.env.svm, &case.keys.market).unwrap();
    market.total_notional_open += 1;
    fixtures::set_zc(&mut case.env.svm, &case.keys.market, &market, 0);
    assert_eq!(
        invariants::check_all(&case.env.svm, &case.keys.market)
            .unwrap_err()
            .invariant,
        "I6"
    );
    case.env
        .svm
        .set_account(case.keys.market, original)
        .unwrap();
    invariants::check_all(&case.env.svm, &case.keys.market).unwrap();
}

fn healthy_receiver(case: &mut Case) -> Pubkey {
    use shortwin::state::{
        InitializePriceSnapshotArgs, PriceReportModelArgs, PriceReportPolicyArgs, PriceSnapshot,
    };
    let gov = case.env.governance.insecure_clone();
    let pos = case.position();
    let key = Pubkey::find_program_address(
        &[
            PriceSnapshot::SEED,
            case.keys.market.as_ref(),
            &pos.q.to_le_bytes(),
            &pos.model_epoch.to_le_bytes(),
        ],
        &shortwin::ID,
    )
    .0;
    let mut publishers = [Pubkey::default(); 8];
    publishers[0] = gov.pubkey();
    let args = InitializePriceSnapshotArgs {
        genesis_hash: [7; 32],
        producer_commitment: [8; 32],
        quantity: pos.q,
        model: PriceReportModelArgs {
            version: pos.model_version,
            epoch: pos.model_epoch,
            policy_hash: pos.policy_hash,
        },
        policy: PriceReportPolicyArgs {
            max_age_slots: 100,
            max_age_seconds: 4,
            max_window_slots: 100,
            max_window_seconds: 1,
            max_update_gap_slots: 10,
            max_update_gap_seconds: 3,
            epoch_seconds: 10,
            minimum_completed_epochs: 1,
        },
        publisher_count: 1,
        publishers,
    };
    let instruction = Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::InitializePriceSnapshot {
            authority: gov.pubkey(),
            config: pda::config(),
            market: case.keys.market,
            price_snapshot: key,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: shortwin::instruction::InitializePriceSnapshot { args }.data(),
    };
    ok(case.env.send(&[instruction], &[&gov]));
    key
}
fn publish_healthy_reference(case: &mut Case, receiver: Pubkey) {
    use shortwin::state::{
        AttestedPriceBucketArgs, PriceReportStatusArgs, PriceSnapshot, PublishPriceSnapshotArgs,
    };
    let gov = case.env.governance.insecure_clone();
    let clock = case.env.clock();
    let epoch = clock.unix_timestamp as u64 / 10;
    let first_ts = ((epoch - 1) * 10) as i64;
    let last_ts = first_ts + 9;
    let mut history = [AttestedPriceBucketArgs::default(); 8];
    history[0] = AttestedPriceBucketArgs {
        epoch: epoch - 1,
        first_slot: clock.slot - (clock.unix_timestamp - first_ts) as u64 * 2,
        first_ts,
        last_slot: clock.slot - (clock.unix_timestamp - last_ts) as u64 * 2,
        last_ts,
        lower_min: 200_000_000,
        central_last: 201_000_000,
        upper_max: 202_000_000,
        observations: 10,
        largest_gap_slots: 2,
        largest_gap_seconds: 1,
    };
    let args = PublishPriceSnapshotArgs {
        version: 2,
        sequence: 1,
        start_slot: clock.slot,
        start_ts: clock.unix_timestamp,
        end_slot: clock.slot,
        end_ts: clock.unix_timestamp,
        observation_digest: [9; 32],
        status: PriceReportStatusArgs::Finite {
            lower: 200_000_000,
            central: 201_000_000,
            upper: 202_000_000,
        },
        history_count: 1,
        history,
    };
    let instruction = Instruction {
        program_id: shortwin::ID,
        accounts: shortwin::accounts::PublishPriceSnapshot {
            publisher: gov.pubkey(),
            config: pda::config(),
            market: case.keys.market,
            price_snapshot: receiver,
        }
        .to_account_metas(None),
        data: shortwin::instruction::PublishPriceSnapshot { args }.data(),
    };
    ok(case.env.send(&[instruction], &[&gov]));
    let wrapper: PriceSnapshot = read::borsh(&case.env.svm, &receiver).unwrap();
    let market = case.env.market(&case.keys.market);
    let state = wrapper
        .restore(&pda::config(), &case.keys.market, &market)
        .unwrap();
    let now = state.latest_metadata().unwrap().window.end;
    let marks = state.live_marks_at(now, 1).unwrap();
    assert_eq!(marks.upper(), 202_000_000);
    assert!(state.evidence_at(now, 1).is_ok());
    // Explicit healthy reference scenario: about20x replacement-cost coverage.
    assert!(shortwin_math::risk::cr_bps(case.position().v, Some(marks.upper())).unwrap() > 100_000);
}
#[test]
fn overdue_default_refuses_deadline_and_last_grace_second_then_allows_exact_boundary() {
    use shortwin::events::OverdueCollateralDefault;
    let mut case = setup(TokenProgram::Spl, Q);
    let keeper = case.env.funded_keypair();
    let pos = case.position();
    for time in [pos.deadline, pos.default_at - 1] {
        case.warp_to(time);
        let before = case.tracked();
        assert_err(&case.default(&keeper), E::OverdueDefaultNotReady);
        assert_eq!(case.tracked(), before);
    }
    case.warp_to(pos.default_at);
    let meta = ok(case.default(&keeper));
    let event: OverdueCollateralDefault = events::one(&meta);
    assert_eq!(event.ts, pos.default_at);
    assert_eq!(event.q_lost, pos.q);
    assert_eq!(event.borrower_amount, 0);
    assert_eq!(event.keeper_payment + event.lender_amount, pos.v);
    assert!(events::of_ix::<Repay>(&meta, 0).is_empty());
    assert!(events::of_ix::<shortwin::events::SolSettle>(&meta, 0).is_empty());
}
#[test]
fn healthy_cached_prices_do_not_veto_authorized_foreclosure_and_no_collateral_returns_to_borrower()
{
    use shortwin::events::OverdueCollateralDefault;
    for program in TokenProgram::BOTH {
        let mut case = setup(program, Q);
        let receiver = healthy_receiver(&mut case);
        let pos = case.position();
        let keeper = case.env.funded_keypair();
        case.warp_to(pos.default_at);
        publish_healthy_reference(&mut case, receiver);
        let instruction = case.default_instruction(keeper.pubkey());
        for forbidden in [
            receiver,
            pda::oracle_cache(),
            pda::venue_guard(),
            case.keys.mint,
            case.keys.pool_vault,
        ] {
            assert!(
                !instruction
                    .accounts
                    .iter()
                    .any(|meta| meta.pubkey == forbidden)
            );
        }
        let borrower_before = read::lamports(&case.env.svm, &case.borrower.pubkey());
        let keeper_before = read::lamports(&case.env.svm, &keeper.pubkey());
        let vault_before = read::lamports(&case.env.svm, &case.keys.market_vault);
        let untracked_refund = read::lamports(&case.env.svm, &case.position_key) - pos.v;
        let exposure_refund = read::lamports(&case.env.svm, &case.exposure_key);
        let tokens_before = read::token_amount(&case.env.svm, &case.keys.pool_vault);
        let metadata = ok(case.default(&keeper));
        eprintln!(
            "overdue default {:?}: {} CU",
            program, metadata.compute_units_consumed
        );
        assert!(metadata.compute_units_consumed < 200_000);
        let event: OverdueCollateralDefault = events::one(&metadata);
        assert_eq!(event.collateral, pos.v);
        assert_eq!(event.default_at, pos.default_at);
        assert_eq!(event.keeper_payment, 50_000_000);
        assert_eq!(event.borrower_amount, 0);
        assert_eq!(event.position_account_refund, untracked_refund);
        assert_eq!(
            read::lamports(&case.env.svm, &case.borrower.pubkey()),
            borrower_before + untracked_refund + exposure_refund
        );
        assert_eq!(
            read::lamports(&case.env.svm, &keeper.pubkey()),
            keeper_before + event.keeper_payment
        );
        assert_eq!(
            read::lamports(&case.env.svm, &case.keys.market_vault),
            vault_before + event.lender_amount
        );
        assert_eq!(
            read::token_amount(&case.env.svm, &case.keys.pool_vault),
            tokens_before
        );
        assert_eq!(read::token_amount(&case.env.svm, &case.source), Some(Q));
        let market = case.env.market(&case.keys.market);
        assert_eq!(
            (
                market.tokens_free,
                market.borrowed_q,
                market.total_notional_open,
                market.receivable
            ),
            (LEND - Q, 0, 0, 0)
        );
        assert!(
            case.env
                .svm
                .get_account(&case.position_key)
                .is_none_or(|account| account.lamports == 0)
        );
        assert!(
            case.env
                .svm
                .get_account(&case.exposure_key)
                .is_none_or(|account| account.lamports == 0)
        );
    }
}
#[test]
fn repayment_and_overdue_default_race_has_exactly_one_terminal_winner() {
    for repay_first in [false, true] {
        let mut case = setup(TokenProgram::Spl, Q);
        let keeper = case.env.funded_keypair();
        case.warp_to(case.position().default_at);
        if repay_first {
            ok(case.attempt(u64::MAX));
            let before = case.tracked();
            assert!(case.default(&keeper).is_err());
            assert_eq!(case.tracked(), before);
            assert_eq!(case.env.market(&case.keys.market).tokens_free, LEND);
        } else {
            ok(case.default(&keeper));
            let before = case.tracked();
            assert!(case.attempt(u64::MAX).is_err());
            assert_eq!(case.tracked(), before);
            assert_eq!(case.env.market(&case.keys.market).tokens_free, LEND - Q);
        }
    }
}
#[test]
fn full_token_claim_wipe_persists_old_lender_epoch_and_handles_prefunded_record() {
    use shortwin::events::{ClaimSol, EpochRolled, OverdueCollateralDefault};
    use shortwin::state::ShareEpoch;
    let mut case = setup_with_q(TokenProgram::Spl, 0, LEND);
    let keeper = case.env.funded_keypair();
    let pos = case.position();
    let record_key = pda::share_epoch(&case.keys.market, 0).0;
    let donation = 7_000_003;
    case.env.fund(&record_key, donation);
    case.warp_to(pos.default_at);
    let keeper_before = read::lamports(&case.env.svm, &keeper.pubkey());
    let metadata = ok(case.default(&keeper));
    let event: OverdueCollateralDefault = events::one(&metadata);
    let rolled: EpochRolled = events::one(&metadata);
    assert_eq!(rolled.epoch, 0);
    assert_eq!(event.share_epoch_after, 1);
    let record: ShareEpoch = read::borsh(&case.env.svm, &record_key).unwrap();
    assert!(record.record().is_full_wipe());
    let market = case.env.market(&case.keys.market);
    assert_eq!(
        (
            market.tokens_free,
            market.borrowed_q,
            market.open_positions,
            market.receivable
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(market.total_shares.get(), 0);
    assert_eq!(
        read::token_amount(&case.env.svm, &case.keys.pool_vault),
        Some(0)
    );
    let vault = case.env.market_vault(&case.keys.market);
    assert_eq!(vault.lender_sol, event.lender_amount + donation);
    assert_eq!(vault.protocol_fees, 0);
    assert_eq!(
        read::lamports(&case.env.svm, &keeper.pubkey()),
        keeper_before + event.keeper_payment
            - case
                .env
                .svm
                .minimum_balance_for_rent_exemption(ShareEpoch::SPACE)
    );
    let finance: QuoteFinanceMarket = read::borsh(&case.env.svm, &case.finance_key).unwrap();
    assert_ne!(
        finance
            .restore(&pda::config(), &case.keys.market, &market)
            .unwrap()
            .carry()
            .numerator(),
        0
    );
    let mut claim = ix::claim_sol(&case.lender.pubkey(), &case.keys.market);
    claim.accounts.push(AccountMeta::new(record_key, false));
    let lender_before = read::lamports(&case.env.svm, &case.lender.pubkey());
    let meta = ok(case.env.send(&[claim], &[&case.lender]));
    let paid: ClaimSol = events::one(&meta);
    assert!(paid.sol_paid > 0);
    assert_eq!(
        read::lamports(&case.env.svm, &case.lender.pubkey()),
        lender_before + paid.sol_paid
    );
    assert_eq!(
        read::token_amount(&case.env.svm, &case.lender_source),
        Some(0)
    );
}

#[test]
fn overdue_keeper_is_bounded_by_locked_cap_and_remaining_collateral() {
    use shortwin::events::OverdueCollateralDefault;
    for (fixed, bps, cap, expected) in [
        (u64::MAX, 10000, 12_345, 12_345),
        (u64::MAX, 10000, u64::MAX, 4_000_000_000),
        (0, 0, 0, 0),
    ] {
        let mut case = setup(TokenProgram::Spl, 0);
        // Controlled opening terms: no instruction can rewrite these on live debt.
        let mut position = case.position();
        position
            .lock_recovery(shortwin_math::risk::SettleKeeperTerms { fixed, bps, cap })
            .unwrap();
        fixtures::set_borsh(
            &mut case.env.svm,
            &case.position_key,
            &position,
            QuoteFinancePosition::SPACE,
            position.v,
        );
        let keeper = case.env.funded_keypair();
        case.warp_to(position.default_at);
        let meta = ok(case.default(&keeper));
        let event: OverdueCollateralDefault = events::one(&meta);
        assert_eq!(event.keeper_payment, expected);
        assert_eq!(event.lender_amount, position.v - expected);
        assert_eq!(event.borrower_amount, 0);
        assert_eq!(event.keeper_payment + event.lender_amount, position.v);
    }
}
#[test]
fn overdue_clock_cannot_borrow_authority_from_clamped_accrual_and_named_accounts_are_strict() {
    let mut case = setup(TokenProgram::Spl, Q);
    let keeper = case.env.funded_keypair();
    let position = case.position();
    // A legitimate past accrual followed by a chain-clock step back. Financial
    // accounting stays monotonic, but eligibility must use actual Clock again.
    let mut market = case.env.market(&case.keys.market);
    let mut finance: QuoteFinanceMarket = read::borsh(&case.env.svm, &case.finance_key).unwrap();
    let mut pool = market.pool();
    let mut vault = case.env.market_vault(&case.keys.market).vault();
    engine::accrue(
        &pda::config(),
        &case.keys.market,
        &mut market,
        &mut finance,
        &mut pool,
        &mut vault,
        position.default_at,
    )
    .unwrap();
    fixtures::set_zc(&mut case.env.svm, &case.keys.market, &market, 0);
    fixtures::set_borsh(
        &mut case.env.svm,
        &case.finance_key,
        &finance,
        QuoteFinanceMarket::SPACE,
        0,
    );
    case.warp_to(position.default_at - 1);
    let before = case.tracked();
    assert_err(&case.default(&keeper), E::OverdueDefaultNotReady);
    assert_eq!(case.tracked(), before);
    case.warp_to(position.default_at);
    for index in [3usize, 5, 6, 8] {
        let mut instruction = case.default_instruction(keeper.pubkey());
        instruction.accounts[index].pubkey = Pubkey::new_unique();
        assert!(case.env.send(&[instruction], &[&keeper]).is_err());
        assert_eq!(case.tracked(), before);
    }
    let mut extra = case.default_instruction(keeper.pubkey());
    extra
        .accounts
        .push(AccountMeta::new_readonly(pda::oracle_cache(), false));
    assert_err(&case.env.send(&[extra], &[&keeper]), E::AccountMismatch);
    assert_eq!(case.tracked(), before);
    ok(case.default(&keeper));
}

#[test]
fn overdue_epoch_record_rent_is_signed_bounded_and_unused_zero_cap_is_allowed() {
    use shortwin::state::ShareEpoch;
    let mut case = setup_with_q(TokenProgram::Spl, 0, LEND);
    let keeper = case.env.funded_keypair();
    case.warp_to(case.position().default_at);
    let record_key = pda::share_epoch(&case.keys.market, 0).0;
    case.env.fund(&record_key, 7_000_003);
    let floor = case
        .env
        .svm
        .minimum_balance_for_rent_exemption(ShareEpoch::SPACE);
    assert!(floor > 0);
    let before = case.tracked();
    let record_before = case.env.svm.get_account(&record_key);
    let keeper_before = read::lamports(&case.env.svm, &keeper.pubkey());
    let mut instruction = case.default_instruction(keeper.pubkey());
    instruction.data = shortwin::instruction::OverdueQuoteDefault {
        max_epoch_record_rent: floor - 1,
    }
    .data();
    assert_err(
        &case.env.send(&[instruction], &[&keeper]),
        E::EpochRecordRentAboveMax,
    );
    assert_eq!(case.tracked(), before);
    assert_eq!(case.env.svm.get_account(&record_key), record_before);
    assert_eq!(
        read::lamports(&case.env.svm, &keeper.pubkey()),
        keeper_before
    );
    let mut instruction = case.default_instruction(keeper.pubkey());
    instruction.data = shortwin::instruction::OverdueQuoteDefault {
        max_epoch_record_rent: floor,
    }
    .data();
    ok(case.env.send(&[instruction], &[&keeper]));
    assert!(read::borsh::<ShareEpoch>(&case.env.svm, &record_key).is_some());
    let mut case = setup(TokenProgram::Spl, 0);
    let keeper = case.env.funded_keypair();
    case.warp_to(case.position().default_at);
    let record_key = pda::share_epoch(&case.keys.market, 0).0;
    let mut instruction = case.default_instruction(keeper.pubkey());
    instruction.data = shortwin::instruction::OverdueQuoteDefault {
        max_epoch_record_rent: 0,
    }
    .data();
    ok(case.env.send(&[instruction], &[&keeper]));
    assert!(read::borsh::<ShareEpoch>(&case.env.svm, &record_key).is_none());
}

#[test]
fn locked_position_vesting_survives_global_config_changes_on_repay_and_default() {
    for repay in [false, true] {
        let mut case = setup(TokenProgram::Spl, Q);
        let pos = case.position();
        assert_eq!(pos.lump_vest_seconds, 86_400);
        // Controlled governance outcome: mutable global settings may change for
        // other contracts, but cannot rewrite this quote position's opening policy.
        let mut global = case.env.config();
        global.params.lump_vest_s = 3600;
        fixtures::set_zc(&mut case.env.svm, &pda::config(), &global, 0);
        if repay {
            ok(case.attempt(u64::MAX));
        } else {
            let keeper = case.env.funded_keypair();
            case.warp_to(pos.default_at);
            ok(case.default(&keeper));
        }
        let market = case.env.market(&case.keys.market);
        assert!(market.vest_bal > 0);
        assert_eq!(
            market.vest_end - market.vest_last,
            i64::from(pos.lump_vest_seconds)
        );
        assert_ne!(
            market.vest_end - market.vest_last,
            i64::from(global.params.lump_vest_s)
        );
    }
}
#[test]
fn quote_price_pause_cannot_block_full_token_repayment_or_locked_clock_default() {
    for repay in [false, true] {
        let mut case = setup(TokenProgram::Spl, Q);
        let emergency = case.env.pause_authority.insecure_clone();
        let pause = Instruction {
            program_id: shortwin::ID,
            accounts: shortwin::accounts::SetQuotePricePause {
                authority: emergency.pubkey(),
                config: pda::config(),
                market: case.keys.market,
            }
            .to_account_metas(None),
            data: shortwin::instruction::SetQuotePricePause { paused: true }.data(),
        };
        ok(case.env.send(&[pause], &[&emergency]));
        assert!(
            case.env
                .market(&case.keys.market)
                .has_flag(shortwin::state::MARKET_FLAG_QUOTE_PRICE_PAUSED)
        );
        if repay {
            ok(case.attempt(u64::MAX));
        } else {
            let keeper = case.env.funded_keypair();
            case.warp_to(case.position().default_at);
            ok(case.default(&keeper));
        }
    }
}
