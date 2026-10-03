//! M1 CU baseline (ARCHITECTURE §6.1, §8.2; §9 M1 exit criterion "CU baseline bench"; T-CU
//! starts here and becomes a gate in M3–M7).
//!
//! Every [`Case`] builds its pre-state with the LiteSVM harness (`shortwin_litesvm_tests::Env`:
//! the SBF program from `target/deploy/shortwin.so`, the SPL Token (p-token under the mainnet
//! feature set) and ATA ELFs LiteSVM bundles, mainnet's Token-2022 and spl-memo v3 (since M10,
//! `fork::load_mainnet_spl`; `SHORTWIN_BUNDLED_SPL=1` keeps LiteSVM's), the clock under test
//! control), then
//!
//! 1. copies every account the instruction names into a fresh Mollusk 0.15.1 and loads the
//!    same ELFs into its program cache, with LiteSVM's clock and rent;
//! 2. runs the instruction on Mollusk: the figure the budget applies to;
//! 3. runs the same instruction on LiteSVM through `Env::send` (the harness's invariant hooks
//!    run too): a cross-check of the two harnesses.
//!
//! Every M1 handler is implemented (the M1 merge deleted `ShortwinError::NotImplemented`), so
//! every case is measured.

use std::path::PathBuf;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::bpf_loader_upgradeable;
use anchor_lang::solana_program::instruction::Instruction;
use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use shortwin::PauseScope;
use shortwin::constants::{GUARD_PUMP, PYTH_RECEIVER_ID, PYTH_SOL_USD_ACCOUNT, VENUE_PROGRAM_DATA};
use shortwin::state::ConfigChange;
use shortwin_litesvm_tests::fixtures::{self, MarketKeys};
use shortwin_litesvm_tests::fork;
use shortwin_litesvm_tests::ix::{self, LenderKeys};
use shortwin_litesvm_tests::tokens::{self, Holder, TokenProgram};
use shortwin_litesvm_tests::{Env, pda};
use shortwin_venue_state::pyth::{self, PriceUpdate};
use solana_account::Account;
use solana_keypair::Keypair;
use solana_rent::Rent;
use solana_signer::Signer;

/// A budget and where it comes from.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    /// Compute units.
    pub cu: u64,
    /// Source of the figure.
    pub source: &'static str,
}

/// Admin instructions with a parameter set to validate (no ARCH §6.1 row; the class limit of
/// `tests/litesvm/tests/it/cu.rs`).
pub const ADMIN: Budget = Budget {
    cu: 200_000,
    source: "no §6.1 row; admin class (tests/litesvm cu.rs)",
};
/// Admin flag writes (no ARCH §6.1 row).
pub const ADMIN_FLAG: Budget = Budget {
    cu: 60_000,
    source: "no §6.1 row; admin flag class (tests/litesvm cu.rs)",
};
/// ARCH §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5: the 10 MiB ProgramData copy is not
/// metered — VERIFIED for CU in M3, 6,032 / 8,440 CU; latency and fee cost unmeasured).
pub const LATCH: Budget = Budget {
    cu: 400_000,
    source: "§6.1 `flag_venue_upgraded` / `revalidate_venue`",
};
/// ARCH §6.1 `create_market`.
pub const CREATE_MARKET: Budget = Budget {
    cu: 250_000,
    source: "§6.1 `create_market`",
};
/// ARCH §6.1 `deposit` / `withdraw`.
pub const LENDER_TOKENS: Budget = Budget {
    cu: 100_000,
    source: "§6.1 `deposit` / `withdraw`",
};
/// ARCH §6.1 `claim_sol` / `cover_deficit` / `sync_lender`.
pub const LENDER_SOL: Budget = Budget {
    cu: 60_000,
    source: "§6.1 `claim_sol` / `cover_deficit` / `sync_lender`",
};
/// Permissionless cranks without a §6.1 row (the `crank_price` limit).
pub const CRANK: Budget = Budget {
    cu: 60_000,
    source: "no §6.1 row; crank class (§6.1 `crank_price`)",
};

/// An instruction ready to run, and the environment that holds its pre-state.
pub struct Prepared {
    /// The LiteSVM environment with the pre-state.
    pub env: Env,
    /// The instruction under measurement.
    pub ix: Instruction,
    /// Its signers (besides the harness fee payer).
    pub signers: Vec<Keypair>,
}

/// One row of the report.
pub struct Case {
    /// The instruction (its handler file under `programs/shortwin/src/instructions/`).
    pub handler: &'static str,
    /// What the case exercises.
    pub case: &'static str,
    /// The limit the Mollusk figure must stay within.
    pub budget: Budget,
    /// Builds the pre-state and the instruction.
    pub setup: fn() -> Prepared,
}

/// What [`measure`] found: compute units on Mollusk and on LiteSVM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Measured {
    /// Mollusk 0.15.1.
    pub mollusk: u64,
    /// LiteSVM 0.16.0.
    pub litesvm: u64,
}

/// A path under the repository root.
pub fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(rel)
}

/// The accounts `ix` names, as LiteSVM holds them (missing ones as empty system accounts;
/// the System program as Mollusk's builtin account).
fn snapshot(env: &Env, ix: &Instruction) -> Vec<(Pubkey, Account)> {
    let mut out: Vec<(Pubkey, Account)> = Vec::new();
    for meta in &ix.accounts {
        if out.iter().any(|(k, _)| *k == meta.pubkey) {
            continue;
        }
        let account = if meta.pubkey == anchor_lang::system_program::ID {
            keyed_account_for_system_program().1
        } else {
            env.svm.get_account(&meta.pubkey).unwrap_or_default()
        };
        out.push((meta.pubkey, account));
    }
    out
}

/// `BPFLoader2111111111111111111111111111111111` (loader v2).
const BPF_LOADER_2: Pubkey = Pubkey::from_str_const("BPFLoader2111111111111111111111111111111111");
/// `BPFLoader1111111111111111111111111111111111` (loader v1, deprecated).
const BPF_LOADER_1: Pubkey = Pubkey::from_str_const("BPFLoader1111111111111111111111111111111111");

/// The ELF of an executable account LiteSVM holds (loader v2 / v1: the data; loader v3: the
/// ProgramData after its 45-byte header). `None` for builtins.
fn elf_of(env: &Env, key: &Pubkey, account: &Account) -> Option<Vec<u8>> {
    if account.owner == bpf_loader_upgradeable::ID {
        let data = env
            .svm
            .get_account(&bpf_loader_upgradeable::get_program_data_address(key))?
            .data;
        Some(
            data.get(shortwin_venue_state::program_data::HEADER_LEN..)?
                .to_vec(),
        )
    } else if account.owner == BPF_LOADER_2 || account.owner == BPF_LOADER_1 {
        Some(account.data.clone())
    } else {
        None
    }
}

/// A Mollusk with short.win, every other program `accounts` names, and LiteSVM's clock and
/// rent.
fn mollusk_for(env: &Env, accounts: &[(Pubkey, Account)]) -> Mollusk {
    let mut mollusk = Mollusk::default();
    let so = shortwin_litesvm_tests::env::program_so_path();
    let elf = std::fs::read(&so).unwrap_or_else(|e| panic!("read {}: {e}", so.display()));
    mollusk.add_program_with_loader_and_elf(&shortwin::ID, &bpf_loader_upgradeable::ID, &elf);
    for (key, account) in accounts {
        if !account.executable || *key == shortwin::ID {
            continue;
        }
        if let Some(elf) = elf_of(env, key, account) {
            mollusk.add_program_with_loader_and_elf(key, &account.owner, &elf);
        }
    }
    mollusk.sysvars.clock = env.clock();
    mollusk.sysvars.rent = env.svm.get_sysvar::<Rent>();
    mollusk
}

/// Runs one case (see the crate docs). `Err` describes a failed run.
pub fn measure(case: &Case) -> Result<Measured, String> {
    let Prepared {
        mut env,
        ix,
        signers,
    } = (case.setup)();

    let accounts = snapshot(&env, &ix);
    let mollusk = mollusk_for(&env, &accounts);
    let result = mollusk.process_instruction(&ix, &accounts);

    let refs: Vec<&Keypair> = signers.iter().collect();
    let lite = env.send(std::slice::from_ref(&ix), &refs);

    match (&result.raw_result, lite) {
        (Ok(()), Ok(meta)) => Ok(Measured {
            mollusk: result.compute_units_consumed,
            litesvm: meta.compute_units_consumed,
        }),
        (m, l) => Err(format!(
            "{} ({}): Mollusk {:?} after {} CU; LiteSVM {}",
            case.handler,
            case.case,
            m,
            result.compute_units_consumed,
            match l {
                Ok(meta) => format!("succeeded ({} CU)", meta.compute_units_consumed),
                Err(f) => format!("failed with {:?}; logs:\n{}", f.err, f.meta.pretty_logs()),
            }
        )),
    }
}

// ---- setups ---------------------------------------------------------------------------------

fn prepared(env: Env, ix: Instruction, signers: &[&Keypair]) -> Prepared {
    Prepared {
        env,
        ix,
        signers: signers.iter().map(|k| k.insecure_clone()).collect(),
    }
}

#[track_caller]
fn ok(res: shortwin_litesvm_tests::TxResult) {
    shortwin_litesvm_tests::errors::ok(res);
}

fn initialize_config() -> Prepared {
    let env = Env::new();
    let mut args = env.default_init_args();
    // A config this build accepts (M5-SEC-4: the pump curve cannot be enabled before M6).
    args.venue_enabled &= !shortwin::constants::VENUE_BITS_UNFINISHED;
    let authority = env.upgrade_authority.insecure_clone();
    let ix = ix::initialize_config(&authority.pubkey(), args);
    prepared(env, ix, &[&authority])
}

fn params_change(env: &Env) -> ConfigChange {
    let c = env.config();
    ConfigChange::Params {
        params: c.params,
        tiers: c.tiers,
    }
}

fn propose_config() -> Prepared {
    let env = Env::initialized();
    let gov = env.governance.insecure_clone();
    let ix = ix::propose_config(&gov.pubkey(), params_change(&env));
    prepared(env, ix, &[&gov])
}

fn proposed() -> (Env, Keypair) {
    let mut env = Env::initialized();
    let gov = env.governance.insecure_clone();
    let change = params_change(&env);
    ok(env.send(&[ix::propose_config(&gov.pubkey(), change)], &[&gov]));
    (env, gov)
}

fn apply_config() -> Prepared {
    let (mut env, _) = proposed();
    env.warp(i64::from(env.config().timelock_s));
    prepared(env, ix::apply_config(), &[])
}

fn cancel_config() -> Prepared {
    let (env, gov) = proposed();
    let ix = ix::cancel_config(&gov.pubkey());
    prepared(env, ix, &[&gov])
}

fn set_pause_global() -> Prepared {
    let env = Env::initialized();
    let pause = env.pause_authority.insecure_clone();
    let ix = ix::set_pause(&pause.pubkey(), PauseScope::Global, true, None);
    prepared(env, ix, &[&pause])
}

fn set_pause_market() -> Prepared {
    let mut env = Env::initialized();
    let (k, _) = env.install_market(TokenProgram::Spl, 6);
    let pause = env.pause_authority.insecure_clone();
    let ix = ix::set_pause(&pause.pubkey(), PauseScope::Market, true, Some(k.market));
    prepared(env, ix, &[&pause])
}

/// `set_coverage_pause` (M7, O17; ARCH §3.1): the keeper's pause-opens-only key pauses one
/// market, the role installed the way production installs it (Governance's timelocked
/// `Authorities` change, `Env::enable_coverage_authority`).
fn set_coverage_pause_market() -> Prepared {
    let mut env = Env::initialized();
    let (k, _) = env.install_market(TokenProgram::Spl, 6);
    env.enable_coverage_authority();
    let cov = env.coverage_authority.insecure_clone();
    let ix = ix::set_coverage_pause(
        &cov.pubkey(),
        &k.market,
        true,
        shortwin::events::COVERAGE_REASON_KEEPER,
    );
    prepared(env, ix, &[&cov])
}

fn disable_venue() -> Prepared {
    let env = Env::initialized();
    let pause = env.pause_authority.insecure_clone();
    let ix = ix::disable_venue(&pause.pubkey(), 0);
    prepared(env, ix, &[&pause])
}

/// 10 MiB, the size of the pump and PumpSwap ProgramData accounts (ARCH §6.1, U5).
const TEN_MIB: usize = 10 * 1024 * 1024;

/// An initialized environment whose pump ProgramData shows a new deploy slot, `len` bytes.
fn pump_upgraded(len: usize) -> (Env, u64) {
    let mut env = Env::initialized();
    env.set_all_venue_program_data();
    let slot = fork::FIXTURE_GUARD_SLOTS[usize::from(GUARD_PUMP)] + 1;
    let extra = len - shortwin_venue_state::program_data::HEADER_LEN;
    fixtures::set_program_data(
        &mut env.svm,
        &VENUE_PROGRAM_DATA[usize::from(GUARD_PUMP)],
        slot,
        None,
        extra,
    );
    (env, slot)
}

fn flag_venue_upgraded_small() -> Prepared {
    let (env, _) = pump_upgraded(64 + shortwin_venue_state::program_data::HEADER_LEN);
    prepared(env, ix::flag_venue_upgraded(GUARD_PUMP), &[])
}

fn flag_venue_upgraded_10mib() -> Prepared {
    let (env, _) = pump_upgraded(TEN_MIB);
    prepared(env, ix::flag_venue_upgraded(GUARD_PUMP), &[])
}

fn revalidate(len: usize) -> Prepared {
    let (mut env, slot) = pump_upgraded(len);
    ok(env.send(&[ix::flag_venue_upgraded(GUARD_PUMP)], &[]));
    let pause = env.pause_authority.insecure_clone();
    let ix = ix::revalidate_venue(&pause.pubkey(), GUARD_PUMP, slot);
    prepared(env, ix, &[&pause])
}

fn revalidate_venue_small() -> Prepared {
    revalidate(64 + shortwin_venue_state::program_data::HEADER_LEN)
}

fn revalidate_venue_10mib() -> Prepared {
    revalidate(TEN_MIB)
}

/// A new mint of `program` and the `create_market` that lists it.
fn create_market_for(program: TokenProgram) -> (Prepared, Pubkey) {
    let mut env = Env::initialized();
    let payer = env.payer.insecure_clone();
    let authority = Keypair::new();
    let mint = tokens::create_mint(&mut env.svm, &payer, &authority.pubkey(), 6, program);
    let creator = env.funded_keypair();
    let ix = ix::create_market(
        &creator.pubkey(),
        &mint,
        &program.id(),
        0,
        &pda::canonical_pool(&mint),
    );
    (prepared(env, ix, &[&creator]), mint)
}

fn create_market_spl() -> Prepared {
    create_market_for(TokenProgram::Spl).0
}

fn create_market_t22() -> Prepared {
    create_market_for(TokenProgram::Token2022).0
}

fn flag_unsafe_mint() -> Prepared {
    let mut env = Env::initialized();
    let payer = env.payer.insecure_clone();
    let authority = Keypair::new();
    let mint = tokens::create_mint_with_freeze(
        &mut env.svm,
        &payer,
        &authority.pubkey(),
        6,
        TokenProgram::Spl,
    );
    let config = env.config();
    let now = env.now();
    let k = fixtures::install_market(
        &mut env.svm,
        &mint,
        6,
        TokenProgram::Spl,
        &payer.pubkey(),
        now,
        &config,
    );
    env.track_market(k.market);
    prepared(env, ix::flag_unsafe_mint(&mint), &[])
}

/// Raw token units a lender holds and deposits.
const HOLDING: u64 = 1_000_000_000;
const DEPOSIT: u64 = 400_000_000;
/// Lamports moved by `cover_deficit` (and then claimed).
const COVER: u64 = 1_000_000;

struct LenderEnv {
    env: Env,
    k: MarketKeys,
    h: Holder,
}

impl LenderEnv {
    fn new(program: TokenProgram) -> Self {
        let mut env = Env::initialized();
        let (k, authority) = env.install_market(program, 6);
        let payer = env.payer.insecure_clone();
        let h = tokens::holder(&mut env.svm, &payer, &k.mint, &authority, HOLDING, program);
        Self { env, k, h }
    }

    fn keys(&self) -> LenderKeys {
        LenderKeys {
            owner: self.h.owner.pubkey(),
            mint: self.k.mint,
            token_program: self.k.token_program,
            owner_token_account: self.h.token_account,
        }
    }

    fn deposited(program: TokenProgram) -> Self {
        let mut s = Self::new(program);
        let owner = s.h.owner.insecure_clone();
        let deposit = ix::deposit(&s.keys(), DEPOSIT, 0);
        ok(s.env.send(&[deposit], &[&owner]));
        s.env.warp(60);
        s
    }

    fn prepared(self, ix: Instruction) -> Prepared {
        let owner = self.h.owner.insecure_clone();
        prepared(self.env, ix, &[&owner])
    }
}

fn deposit_first_spl() -> Prepared {
    let s = LenderEnv::new(TokenProgram::Spl);
    let ix = ix::deposit(&s.keys(), DEPOSIT, 0);
    s.prepared(ix)
}

fn deposit_first_t22() -> Prepared {
    let s = LenderEnv::new(TokenProgram::Token2022);
    let ix = ix::deposit(&s.keys(), DEPOSIT, 0);
    s.prepared(ix)
}

fn deposit_again_spl() -> Prepared {
    let s = LenderEnv::deposited(TokenProgram::Spl);
    let ix = ix::deposit(&s.keys(), DEPOSIT, 0);
    s.prepared(ix)
}

fn withdraw_half_spl() -> Prepared {
    let s = LenderEnv::deposited(TokenProgram::Spl);
    let shares = s
        .env
        .lender(&s.k.market, &s.h.owner.pubkey())
        .map_or(0, |l| l.shares / 2);
    let ix = ix::withdraw(&s.keys(), shares, 0);
    s.prepared(ix)
}

fn cover_deficit() -> Prepared {
    let s = LenderEnv::deposited(TokenProgram::Spl);
    let ix = ix::cover_deficit(&s.h.owner.pubkey(), &s.k.market, COVER);
    s.prepared(ix)
}

fn claim_sol() -> Prepared {
    let mut s = LenderEnv::deposited(TokenProgram::Spl);
    let owner = s.h.owner.insecure_clone();
    let cover = ix::cover_deficit(&owner.pubkey(), &s.k.market, COVER);
    ok(s.env.send(&[cover], &[&owner]));
    let ix = ix::claim_sol(&owner.pubkey(), &s.k.market);
    s.prepared(ix)
}

fn sync_lender() -> Prepared {
    let s = LenderEnv::deposited(TokenProgram::Spl);
    let ix = ix::sync_lender(&s.h.owner.pubkey(), &s.k.market, 8);
    prepared(s.env, ix, &[])
}

fn sweep_protocol_fees() -> Prepared {
    let (
        Prepared {
            mut env,
            ix,
            signers,
        },
        mint,
    ) = create_market_for(TokenProgram::Spl);
    let refs: Vec<&Keypair> = signers.iter().collect();
    ok(env.send(&[ix], &refs));
    let market = pda::market(&mint).0;
    env.track_market(market);
    let recipient = env.config().fee_recipient;
    prepared(env, ix::sweep_protocol_fees(&market, &recipient), &[])
}

/// The mainnet SOL/USD `PriceUpdateV2` dump (`tests/fixtures/manifest.json`).
const PYTH_DUMP: &[u8] = include_bytes!("../../../fixtures/accounts/pyth_sol_usd_shard0.bin");
/// Wall clock and slot of the dump.
const PYTH_DUMP_TS: i64 = 1_790_523_064;
const PYTH_DUMP_SLOT: u64 = 451_037_803;

fn put_price_update(env: &mut Env, data: Vec<u8>) {
    let lamports = env.svm.minimum_balance_for_rent_exemption(data.len());
    env.svm
        .set_account(
            PYTH_SOL_USD_ACCOUNT,
            Account {
                lamports,
                data,
                owner: PYTH_RECEIVER_ID,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap_or_else(|e| panic!("set PriceUpdateV2: {e:?}"));
}

fn crank_first() -> Prepared {
    let mut env = Env::initialized();
    env.set_clock(PYTH_DUMP_TS, PYTH_DUMP_SLOT);
    put_price_update(&mut env, PYTH_DUMP.to_vec());
    prepared(env, ix::crank_sol_usd(&PYTH_SOL_USD_ACCOUNT), &[])
}

fn crank_against_fresh_cache() -> Prepared {
    let Prepared { mut env, ix, .. } = crank_first();
    ok(env.send(std::slice::from_ref(&ix), &[]));
    let u = pyth::read(&pyth::RECEIVER, PYTH_DUMP).unwrap_or_else(|e| panic!("{e:?}"));
    let next = PriceUpdate {
        price: u.price + 60_000_000,
        publish_time: u.publish_time + 55,
        prev_publish_time: u.publish_time + 54,
        ..u
    };
    put_price_update(&mut env, pyth::encode(&next).to_vec());
    env.warp(55);
    prepared(env, ix, &[])
}

/// Every M1 instruction (ARCH §9 M1), in the order of ARCH §3.1–§3.3 and §3.9; since M7 also
/// `set_coverage_pause` (O17), an admin flag of §3.1.
pub fn cases() -> Vec<Case> {
    let case = |handler, case, budget, setup| Case {
        handler,
        case,
        budget,
        setup,
    };
    vec![
        case(
            "initialize_config",
            "defaults, 7 accounts created",
            ADMIN,
            initialize_config as fn() -> Prepared,
        ),
        case("propose_config", "`Params`", ADMIN, propose_config),
        case("apply_config", "`Params`", ADMIN, apply_config),
        case("cancel_config", "pending `Params`", ADMIN, cancel_config),
        case("set_pause", "global", ADMIN_FLAG, set_pause_global),
        case("set_pause", "one market", ADMIN_FLAG, set_pause_market),
        case(
            "set_coverage_pause",
            "one market, role installed by the timelock (M7)",
            ADMIN_FLAG,
            set_coverage_pause_market,
        ),
        case("disable_venue", "pump curve bit", ADMIN_FLAG, disable_venue),
        case(
            "flag_venue_upgraded",
            "pump, 109 B mock ProgramData",
            LATCH,
            flag_venue_upgraded_small,
        ),
        case(
            "flag_venue_upgraded",
            "pump, 10 MiB ProgramData (U5)",
            LATCH,
            flag_venue_upgraded_10mib,
        ),
        case(
            "revalidate_venue",
            "pump, 109 B mock ProgramData",
            LATCH,
            revalidate_venue_small,
        ),
        case(
            "revalidate_venue",
            "pump, 10 MiB ProgramData (U5)",
            LATCH,
            revalidate_venue_10mib,
        ),
        case(
            "create_market",
            "SPL Token mint, `None` venue",
            CREATE_MARKET,
            create_market_spl,
        ),
        case(
            "create_market",
            "Token-2022 mint, `None` venue",
            CREATE_MARKET,
            create_market_t22,
        ),
        case(
            "flag_unsafe_mint",
            "freeze authority set",
            CRANK,
            flag_unsafe_mint,
        ),
        case(
            "deposit",
            "first deposit, SPL Token",
            LENDER_TOKENS,
            deposit_first_spl,
        ),
        case(
            "deposit",
            "first deposit, Token-2022",
            LENDER_TOKENS,
            deposit_first_t22,
        ),
        case(
            "deposit",
            "second deposit, SPL Token",
            LENDER_TOKENS,
            deposit_again_spl,
        ),
        case(
            "withdraw",
            "half the shares, SPL Token",
            LENDER_TOKENS,
            withdraw_half_spl,
        ),
        case("claim_sol", "pays a covered balance", LENDER_SOL, claim_sol),
        case(
            "cover_deficit",
            "1,000,000 lamports",
            LENDER_SOL,
            cover_deficit,
        ),
        case(
            "sync_lender",
            "lender on the current epoch",
            LENDER_SOL,
            sync_lender,
        ),
        case(
            "sweep_protocol_fees",
            "the market creation fee",
            CRANK,
            sweep_protocol_fees,
        ),
        case(
            "crank_sol_usd",
            "mainnet dump, empty cache",
            CRANK,
            crank_first,
        ),
        case(
            "crank_sol_usd",
            "55 s later, step checked",
            CRANK,
            crank_against_fresh_cache,
        ),
    ]
}
