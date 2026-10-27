//! Actual SBF hostile-target isolation proof. NO Jupiter route or financial authorization.
//! The same test artifact runs at two distinct IDs; no native processor stands in for SBF.
#![allow(clippy::result_large_err)]

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    system_program,
};
use litesvm::LiteSVM;
use shortwin_litesvm_tests::tokens::{self, TokenProgram};
use solana_account::Account;
use solana_keypair::Keypair;
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_rent::Rent;
use solana_signer::Signer;
use solana_transaction::Transaction;
use spl_token_interface::state::{Account as TokenAccount, AccountState, Mint};
use std::{path::PathBuf, sync::OnceLock};
#[allow(dead_code)]
#[path = "../../jupiter_exec_probe/src/wire.rs"]
mod wire;
use wire::*;

fn program() -> &'static [u8] {
    static ELF: OnceLock<Vec<u8>> = OnceLock::new();
    ELF.get_or_init(|| {
        let path = std::env::var_os("SHORTWIN_JUPITER_EXEC_PROBE_SO")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../target/jupiter-exec-probe/deploy/shortwin_jupiter_exec_probe.so")
            });
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "SBF probe absent at {}: {e}; run scripts/build-jupiter-exec-probe.sh",
                path.display()
            )
        });
        assert_eq!(&bytes[..4], b"\x7fELF");
        assert_eq!(
            u32::from_le_bytes(bytes[48..52].try_into().unwrap()),
            3,
            "pinned SBPF v3"
        );
        bytes
    })
}
fn pda(seed: &[u8], escrow: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[seed, escrow.as_ref()],
        &Pubkey::new_from_array(EXECUTOR_ID),
    )
    .0
}
fn amount(svm: &LiteSVM, key: &Pubkey) -> u64 {
    TokenAccount::unpack(&svm.get_account(key).unwrap().data)
        .unwrap()
        .amount
}
struct Case {
    svm: LiteSVM,
    payer: Keypair,
    keys: Vec<Pubkey>,
}
impl Case {
    fn new() -> Self {
        let mut svm = LiteSVM::new();
        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
        let executor = Pubkey::new_from_array(EXECUTOR_ID);
        let target = Pubkey::new_from_array(HOSTILE_TARGET_ID);
        svm.add_program(executor, program()).unwrap();
        svm.add_program(target, program()).unwrap();
        let escrow =
            Pubkey::find_program_address(&[ESCROW_SEED, payer.pubkey().as_ref()], &executor).0;
        svm.set_account(
            escrow,
            Account {
                lamports: 500_000_000,
                data: vec![1],
                owner: executor,
                ..Account::default()
            },
        )
        .unwrap();
        let authority = pda(AUTHORITY_SEED, &escrow);
        let input = pda(INPUT_SEED, &escrow);
        let output = pda(OUTPUT_SEED, &escrow);
        let pool = pda(POOL_SEED, &escrow);
        let pool_owner = pda(POOL_OWNER_SEED, &escrow);
        let venue_authority = Pubkey::find_program_address(&[TARGET_AUTHORITY_SEED], &target).0;
        for key in [authority, venue_authority] {
            svm.set_account(
                key,
                Account {
                    lamports: 1,
                    owner: system_program::ID,
                    ..Account::default()
                },
            )
            .unwrap();
        }
        let token = spl_token_interface::ID;
        let wsol = spl_token_interface::native_mint::ID;
        let mut native_data = vec![0; Mint::LEN];
        Mint::pack(
            Mint {
                mint_authority: COption::None,
                supply: 0,
                decimals: 9,
                is_initialized: true,
                freeze_authority: COption::None,
            },
            &mut native_data,
        )
        .unwrap();
        svm.set_account(
            wsol,
            Account {
                lamports: Rent::default().minimum_balance(Mint::LEN),
                data: native_data,
                owner: token,
                ..Account::default()
            },
        )
        .unwrap();
        let mint = tokens::create_mint(&mut svm, &payer, &payer.pubkey(), 6, TokenProgram::Spl);
        let venue_in = tokens::create_token_account(
            &mut svm,
            &payer,
            &wsol,
            &venue_authority,
            TokenProgram::Spl,
        );
        let venue_out = tokens::create_token_account(
            &mut svm,
            &payer,
            &mint,
            &venue_authority,
            TokenProgram::Spl,
        );
        tokens::mint_to(
            &mut svm,
            &payer,
            &mint,
            &venue_out,
            &payer,
            10_000,
            TokenProgram::Spl,
        );
        let mut pool_data = vec![0; TokenAccount::LEN];
        TokenAccount::pack(
            TokenAccount {
                mint,
                owner: pool_owner,
                amount: 17,
                delegate: COption::None,
                state: AccountState::Initialized,
                is_native: COption::None,
                delegated_amount: 0,
                close_authority: COption::None,
            },
            &mut pool_data,
        )
        .unwrap();
        svm.set_account(
            pool,
            Account {
                lamports: Rent::default().minimum_balance(TokenAccount::LEN),
                data: pool_data,
                owner: token,
                ..Account::default()
            },
        )
        .unwrap();
        // Both fresh addresses can be grief-funded. Normalization must credit the escrow
        // before establishing its spend baseline, then refund all new-account rent.
        for (key, lamports) in [(input, 7), (output, 11)] {
            svm.set_account(
                key,
                Account {
                    lamports,
                    owner: system_program::ID,
                    ..Account::default()
                },
            )
            .unwrap();
        }
        let keys = vec![
            payer.pubkey(),
            escrow,
            authority,
            input,
            output,
            pool,
            mint,
            wsol,
            token,
            system_program::ID,
            target,
            venue_in,
            venue_out,
            venue_authority,
        ];
        Self { svm, payer, keys }
    }
    fn request() -> Request {
        Request {
            mode: MODE_GOOD,
            flags: 0,
            quantity: 50,
            input_amount: 80,
            maximum_input: 100,
            target_spend: 70,
            target_delivery: 50,
        }
    }
    fn ix(&self, r: Request) -> Instruction {
        let writable = [0, 1, 3, 4, 5, 11, 12];
        Instruction {
            program_id: Pubkey::new_from_array(EXECUTOR_ID),
            data: r.encode().to_vec(),
            accounts: self
                .keys
                .iter()
                .enumerate()
                .map(|(i, key)| AccountMeta {
                    pubkey: *key,
                    is_signer: i == 0,
                    is_writable: writable.contains(&i),
                })
                .collect(),
        }
    }
    fn send(
        &mut self,
        ix: Instruction,
        extra_signers: &[&Keypair],
    ) -> shortwin_litesvm_tests::TxResult {
        let mut signers = vec![&self.payer];
        signers.extend_from_slice(extra_signers);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &signers,
            self.svm.latest_blockhash(),
        );
        let out = self.svm.send_transaction(tx);
        self.svm.expire_blockhash();
        out
    }
    fn snapshot(&self) -> Vec<(Pubkey, Option<Account>)> {
        // A failed landed transaction may charge payer fees. Every other account is
        // compared byte-for-byte; that fee is deliberately excluded from rollback.
        self.keys
            .iter()
            .copied()
            .skip(1)
            .map(|k| (k, self.svm.get_account(&k)))
            .collect()
    }
    fn assert_rollback(&self, before: &[(Pubkey, Option<Account>)]) {
        for (key, old) in before {
            assert_eq!(&self.svm.get_account(key), old, "rollback account {key}");
        }
    }
    fn rejected(&mut self, ix: Instruction, signers: &[&Keypair], expected: Option<u32>) {
        let before = self.snapshot();
        let failure = self
            .send(ix, signers)
            .expect_err("hostile attempt must fail");
        eprintln!(
            "SBF refusal {:?}\n{}",
            failure.err,
            failure.meta.pretty_logs()
        );
        if let Some(code) = expected {
            assert_eq!(
                shortwin_litesvm_tests::errors::custom_code(&Err(failure)),
                Some(code)
            );
        }
        self.assert_rollback(&before);
    }
}
#[test]
fn actual_sbf_whole_debt_and_lower_consumption_refund_surplus() {
    for (spent, delivered) in [(80, 50), (70, 50), (1, 65)] {
        let mut c = Case::new();
        let mut r = Case::request();
        r.target_spend = spent;
        r.target_delivery = delivered;
        let before = c.svm.get_account(&c.keys[1]).unwrap().lamports;
        let pool = amount(&c.svm, &c.keys[5]);
        let result = c
            .send(c.ix(r), &[])
            .unwrap_or_else(|f| panic!("{:?}\n{}", f.err, f.meta.pretty_logs()));
        eprintln!(
            "SBF success spent={spent} output={delivered} CU={}\n{}",
            result.compute_units_consumed,
            result.pretty_logs()
        );
        assert!(
            result
                .logs
                .iter()
                .any(|x| x.contains("shortwin:test-only:jupiter-isolation-probe"))
        );
        assert_eq!(amount(&c.svm, &c.keys[5]), pool + delivered);
        assert_eq!(
            c.svm.get_account(&c.keys[1]).unwrap().lamports,
            before + 18 - spent,
            "prefunding normalized, input remainder and BOTH rents refunded"
        );
        for key in [&c.keys[3], &c.keys[4]] {
            let account = c.svm.get_account(key).unwrap_or_default();
            assert_eq!(account.lamports, 0);
            assert!(account.data.is_empty());
            assert_eq!(account.owner, system_program::ID);
        }
    }
}
#[test]
fn underdelivery_and_overspend_revert_actual_token_and_native_movements() {
    let mut c = Case::new();
    let mut r = Case::request();
    r.target_delivery = 49;
    c.rejected(c.ix(r), &[], Some(ERR_DELIVERY));
    let mut c = Case::new();
    let mut r = Case::request();
    r.target_spend = 81;
    c.rejected(c.ix(r), &[], None);
}
#[test]
fn delegate_owner_early_close_and_native_rent_theft_are_atomic_failures() {
    for mode in [
        MODE_DELEGATE,
        MODE_OWNER,
        MODE_EARLY_CLOSE,
        MODE_STEAL_NATIVE,
        MODE_REENTER,
    ] {
        let mut c = Case::new();
        let mut r = Case::request();
        r.mode = mode;
        if mode == MODE_EARLY_CLOSE {
            r.target_delivery = 0;
        }
        c.rejected(c.ix(r), &[], None);
    }
}
#[test]
fn postcondition_failure_after_transfers_and_refunds_rolls_everything_back() {
    let mut c = Case::new();
    let mut r = Case::request();
    r.flags = FORCE_POST_FAILURE;
    c.rejected(c.ix(r), &[], Some(ERR_FORCED_POST));
}
#[test]
fn critical_protocol_accounts_and_runtime_union_privileges_never_reach_target() {
    for slot in [1, 5, 2, 6] {
        let mut c = Case::new();
        let mut ix = c.ix(Case::request());
        // Escrow and pool are forbidden even readonly. A duplicate writable authority or
        // mint makes ALL matching runtime AccountInfos writable, despite the fixed role.
        ix.accounts.push(AccountMeta {
            pubkey: c.keys[slot],
            is_signer: false,
            is_writable: slot == 2 || slot == 6,
        });
        c.rejected(ix, &[], Some(ERR_SURFACE));
    }
    let mut c = Case::new();
    let foreign = Keypair::new();
    c.svm.airdrop(&foreign.pubkey(), 1_000_000).unwrap();
    let mut ix = c.ix(Case::request());
    ix.accounts
        .push(AccountMeta::new_readonly(foreign.pubkey(), true));
    c.rejected(ix, &[&foreign], Some(ERR_SURFACE));
    let mut c = Case::new();
    let other = Pubkey::new_unique();
    c.svm
        .set_account(
            other,
            Account {
                lamports: 1_000_000,
                data: vec![9],
                owner: Pubkey::new_from_array(EXECUTOR_ID),
                ..Account::default()
            },
        )
        .unwrap();
    let mut ix = c.ix(Case::request());
    ix.accounts.push(AccountMeta::new_readonly(other, false));
    c.rejected(ix, &[], Some(ERR_SURFACE));
}
#[test]
fn scratch_substitution_hidden_extensions_and_bounded_funding_fail_closed() {
    let mut c = Case::new();
    let mut ix = c.ix(Case::request());
    ix.accounts[3].pubkey = c.keys[4];
    c.rejected(ix, &[], Some(ERR_BINDINGS));
    let mut c = Case::new();
    let mut bad = c.svm.get_account(&c.keys[3]).unwrap();
    bad.owner = spl_token_interface::ID;
    bad.data = vec![0; 166];
    c.svm.set_account(c.keys[3], bad).unwrap();
    c.rejected(c.ix(Case::request()), &[], Some(ERR_BINDINGS));
    let mut c = Case::new();
    let mut r = Case::request();
    r.input_amount = 101;
    c.rejected(c.ix(r), &[], Some(ERR_FUNDING));
}

/// Independent second gate: actual pinned Jupiter -> frozen Raydium CPMM -> real Token CPI.
/// This is historical-state execution, not a current live quote/route or an oracle approval.
#[test]
fn actual_pinned_jupiter_cpmm_route_credits_full_debt_and_refunds_scratch() {
    use shortwin::constants::{
        JUPITER_PROGRAM_ID, RAYDIUM_CPMM_AUTHORITY, RAYDIUM_CPMM_PROGRAM_ID,
    };
    use shortwin_litesvm_tests::fork::{self, CpmmFixture, CpmmPool, RaydiumProgram};
    use shortwin_math::fees::Direction;
    let mut c = Case::new();
    let snapshot = fork::state_snapshot();
    let bank = solana_clock::Clock {
        slot: 461_997_165,
        unix_timestamp: snapshot.block_time,
        ..solana_clock::Clock::default()
    };
    c.svm.set_sysvar(&bank);
    for f in fork::all_fixtures().filter(|f| f.group == "raydium_state") {
        c.svm.set_account(f.address, f.account.clone()).unwrap();
    }
    fork::load_program(&mut c.svm, RaydiumProgram::Cpmm);
    let which = CpmmFixture::Useless;
    let f = fork::fixture("cpmm_useless_pool");
    let state =
        shortwin_venue_state::raydium_cpmm::read_pool(&f.account.owner.to_bytes(), &f.account.data)
            .unwrap();
    let key = |b: [u8; 32]| Pubkey::new_from_array(b);
    let p = CpmmPool {
        which,
        pool: f.address,
        amm_config: key(state.amm_config),
        observation: key(state.observation_key),
        token_0_vault: key(state.token_0_vault),
        token_1_vault: key(state.token_1_vault),
        token_0_mint: key(state.token_0_mint),
        token_1_mint: key(state.token_1_mint),
        token_0_program: key(state.token_0_program),
        token_1_program: key(state.token_1_program),
        lp_mint: key(state.lp_mint),
        sol_is_token0: state.sol_is_token0().unwrap(),
    };
    assert_eq!(
        p.token_program(),
        TokenProgram::Spl,
        "plain SPL-only first actual route"
    );
    let candidate = std::env::var_os("SHORTWIN_JUPITER_CANDIDATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/jupiter-exec-probe/jupiter-candidate")
        });
    let meta: serde_json::Value = serde_json::from_slice(
        &std::fs::read(candidate.join("accounts.json"))
            .expect("materialize fixed candidate snapshot first"),
    )
    .unwrap();
    assert_eq!(meta["context_slot"], 461_997_164u64);
    assert_eq!(meta["deploy_slot"], 459_479_624u64);
    let loader = anchor_lang::solana_program::bpf_loader_upgradeable::ID;
    let pd = std::fs::read(candidate.join("programdata.bin")).unwrap();
    let digest =
        shortwin::instructions::flag_venue_upgraded::trimmed_elf_hash_of(&loader.to_bytes(), &pd)
            .unwrap()
            .unwrap();
    assert_eq!(
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b"
    );
    assert_eq!(
        u64::from_le_bytes(pd[4..12].try_into().unwrap()),
        459_479_624
    );
    let program_data: Pubkey = "4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT"
        .parse()
        .unwrap();
    let prog = std::fs::read(candidate.join("program.bin")).unwrap();
    assert_eq!(&prog[4..36], program_data.as_ref());
    for (i, name, data) in [(1, "programdata", pd), (0, "program", prog)] {
        let m = &meta["accounts"][i];
        assert_eq!(m["name"], name);
        let address: Pubkey = m["key"].as_str().unwrap().parse().unwrap();
        assert_eq!(m["owner"].as_str().unwrap(), loader.to_string());
        c.svm
            .set_account(
                address,
                Account {
                    lamports: m["lamports"].as_u64().unwrap(),
                    data,
                    owner: loader,
                    executable: m["executable"].as_bool().unwrap(),
                    rent_epoch: m["rentEpoch"].as_u64().unwrap(),
                },
            )
            .unwrap();
    }
    let (iv, ov, im, om, ip, op) = p.io(Direction::Buy);
    assert_eq!(ip, spl_token_interface::ID);
    assert_eq!(op, spl_token_interface::ID);
    c.keys[6] = om;
    c.keys[7] = im;
    c.keys[10] = JUPITER_PROGRAM_ID;
    c.keys[11] = iv;
    c.keys[12] = ov;
    c.keys[13] = RAYDIUM_CPMM_AUTHORITY;
    let event = Pubkey::new_from_array(shortwin_jupiter_route_address_event());
    c.svm
        .set_account(
            event,
            Account {
                lamports: 1,
                owner: system_program::ID,
                ..Account::default()
            },
        )
        .unwrap();
    c.keys.extend([
        RAYDIUM_CPMM_PROGRAM_ID,
        p.amm_config,
        p.pool,
        p.observation,
        event,
    ]);
    let mut protected = c.svm.get_account(&c.keys[5]).unwrap();
    let mut token_state = TokenAccount::unpack(&protected.data).unwrap();
    token_state.mint = om;
    TokenAccount::pack(token_state, &mut protected.data).unwrap();
    c.svm.set_account(c.keys[5], protected).unwrap();
    let input = 1_000_000;
    let expected = p
        .quote_exact_in(&c.svm, Direction::Buy, input)
        .unwrap()
        .proceeds;
    assert!(expected > 0);
    let request = Request {
        mode: MODE_GOOD,
        flags: FLAG_PINNED_JUPITER,
        quantity: expected,
        input_amount: input,
        maximum_input: input,
        target_spend: 0,
        target_delivery: expected,
    };
    // A real routed underfill must fail after CPMM's successful inner swap and roll
    // back every protected/DEX account, rather than merely reject malformed bytes.
    let mut refused = Case {
        svm: c.svm.clone(),
        payer: c.payer.insecure_clone(),
        keys: c.keys.clone(),
    };
    let mut too_high = request;
    too_high.quantity = expected + 1;
    too_high.target_delivery = expected + 1;
    let mut bad_ix = refused.ix(too_high);
    for i in [16, 17] {
        bad_ix.accounts[i].is_writable = true;
    }
    let before_bad = refused.snapshot();
    let failed = refused
        .send(bad_ix, &[])
        .expect_err("actual routed slippage refusal");
    assert!(
        failed
            .meta
            .logs
            .iter()
            .any(|x| x == &format!("Program {RAYDIUM_CPMM_PROGRAM_ID} success")),
        "DEX executed before the real routed minimum-output refusal"
    );
    eprintln!(
        "PINNED JUPITER routed-underfill {:?}\n{}",
        failed.err,
        failed.meta.pretty_logs()
    );
    refused.assert_rollback(&before_bad);
    let mut ix = c.ix(request);
    for i in [16, 17] {
        ix.accounts[i].is_writable = true;
    }
    let preview = Transaction::new_signed_with_payer(
        &[ix.clone()],
        Some(&c.payer.pubkey()),
        &[&c.payer],
        c.svm.latest_blockhash(),
    );
    assert_eq!(preview.signatures.len(), 1);
    let packet_bytes = 1 + 64 + preview.message_data().len();
    assert!(packet_bytes <= 1232);
    eprintln!(
        "PINNED JUPITER legacy_packet_bytes={packet_bytes} outer_accounts={} inner_metas=24 instruction_bytes=39",
        ix.accounts.len()
    );
    let before = c.svm.get_account(&c.keys[1]).unwrap().lamports;
    let pool_before = amount(&c.svm, &c.keys[5]);
    let result = c.send(ix, &[]).unwrap_or_else(|f| {
        panic!(
            "actual pinned Jupiter route failed: {:?}\n{}",
            f.err,
            f.meta.pretty_logs()
        )
    });
    eprintln!(
        "PINNED JUPITER historical_state_slot={} execution_bank_slot={} input={} output={} CU={}\n{}",
        snapshot.slot,
        bank.slot,
        input,
        expected,
        result.compute_units_consumed,
        result.pretty_logs()
    );
    assert!(
        result
            .logs
            .iter()
            .any(|x| x.contains(&format!("Program {JUPITER_PROGRAM_ID} invoke")))
    );
    assert!(
        result
            .logs
            .iter()
            .any(|x| x.contains(&format!("Program {RAYDIUM_CPMM_PROGRAM_ID} invoke")))
    );
    assert_eq!(amount(&c.svm, &c.keys[5]), pool_before + expected);
    assert_eq!(
        c.svm.get_account(&c.keys[1]).unwrap().lamports,
        before + 18 - input
    );
    for key in [&c.keys[3], &c.keys[4]] {
        let x = c.svm.get_account(key).unwrap_or_default();
        assert_eq!(x.lamports, 0);
        assert!(x.data.is_empty());
    }
}

// Fixed primary IDL event PDA. The root harness intentionally gains no prototype dependency.
fn shortwin_jupiter_route_address_event() -> [u8; 32] {
    "D8cy77BBepLMngZx6ZukaTff5hCt1HrWyKk3Hnd9oitf"
        .parse::<Pubkey>()
        .unwrap()
        .to_bytes()
}
