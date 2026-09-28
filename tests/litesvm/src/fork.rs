//! A mainnet fork of the Raydium venues (M3; ARCH §8.2 "dumped mainnet ELFs", "account
//! fixtures", "scenario builders"): the real AMM v4 and CPMM programs and pools, dumped by
//! `scripts/dump-fixtures.py` into `tests/fixtures/` with their provenance in
//! `tests/fixtures/manifest.json` (groups `raydium_programs` and `raydium_state`: every venue
//! state account at **one** mainnet slot).
//!
//! - **Loading.** [`Env::fork_raydium_v4`] / [`Env::fork_cpmm`] give an initialized short.win
//!   environment (release build, [`Env::initialized`]) with the Raydium programs deployed at
//!   their mainnet addresses from their dumped ProgramData (header ‖ ELF ‖ zeros, rebuilt and
//!   sha256-checked against the mainnet account, so the venue-upgrade latch reads the real
//!   deploy slot: T-LATCH on the dumped ProgramData), one pool with every account its swaps
//!   need, and the clock at the snapshot's slot and block time. On any other environment (e.g.
//!   the test-venue build) use [`Env::load_raydium_programs`], [`Env::load_ray_v4`],
//!   [`Env::load_cpmm`] and [`Env::set_fork_clock`]. Every fixture file is checked against its
//!   manifest sha256 when it is first read.
//! - **Pools.** [`V4Pool`] and [`CpmmPool`] hold the keys; they read the live state with the
//!   `shortwin-venue-state` readers ([`V4Pool::info`], [`CpmmPool::state`], reserves), build the
//!   venues' own swap instructions (AMM v4 tags 16/17, CPMM `swap_base_input` /
//!   `swap_base_output`, exact account orders of R-RAY §3.1–§3.2) and quote them with
//!   `shortwin-math` (`quote::ray_v4`, `quote::cpmm`) on the reader's reserves.
//! - **Users.** [`fund_wsol`] (a real WSOL account: create, airdrop, `SyncNative`),
//!   [`fund_token`] (a real token account; the balance is planted, since these mints have no
//!   mint authority), [`trader`], and [`swap`]: a direct swap signed by a normal user, through
//!   [`Env::send`] (the invariant hooks run).
//! - **Admin actions.** Byte-level scenario builders on the dumped accounts (AMM v4 `status`,
//!   `pool_open_time`, swap fee, `need_take_pnl`; CPMM `status`, `open_time`, fee counters,
//!   creator-fee fields, the pool's config, `AmmConfig` rates; vault balances; the ProgramData
//!   deploy slot), and the **real** admin instructions run by the dumped programs with the pool
//!   admin's signature forged ([`admin_send`]: LiteSVM's signature check is switched off for
//!   that one transaction; the hooks still run): AMM v4 `SetParams` (tag 6) and CPMM
//!   `update_pool_status` / `update_amm_config`, so a test can show the byte-level builder
//!   leaves exactly what the admin's own instruction leaves.
//! - **Logs.** [`ray_logs`] decodes AMM v4's `ray_log` (the reserves the venue priced on) and
//!   [`cpmm_swap_events`] CPMM's `SwapEvent` (`emit!`, "Program data:").

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::OnceLock;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use litesvm::LiteSVM;
use litesvm::types::TransactionMetadata;
use litesvm_token::{CreateAccount, SyncNative};
use shortwin::constants::{
    RAYDIUM_CPMM_AUTHORITY, RAYDIUM_CPMM_PROGRAM_ID, RAYDIUM_V4_AUTHORITY, RAYDIUM_V4_PROGRAM_ID,
    TOKEN_PROGRAM_ID, WSOL_MINT,
};
use shortwin::state::Market;
use shortwin_math::fees::{self as mfees, CpmmConfig, CpmmFees, CpmmPoolFees, Direction};
use shortwin_math::quote::{BuyQuote, SellQuote, cpmm as qcpmm, ray_v4 as qv4};
use shortwin_venue_state::{raydium_cpmm, raydium_v4, token_account};
use solana_account::Account;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;

use crate::tokens::TokenProgram;
use crate::{Env, TxResult, invariants, read};

// ---- fixtures ------------------------------------------------------------------------------

/// `tests/fixtures/` (`$SHORTWIN_FIXTURES` overrides).
pub fn fixtures_dir() -> PathBuf {
    std::env::var_os("SHORTWIN_FIXTURES").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures"),
        PathBuf::from,
    )
}

/// One manifest entry of the Raydium groups, with its account rebuilt and checked.
#[derive(Clone, Debug)]
pub struct Fixture {
    /// The manifest name (e.g. `ray_v4_fartcoin_pool`).
    pub name: String,
    /// `raydium_programs` or `raydium_state`.
    pub group: String,
    pub address: Pubkey,
    /// The mainnet slot the account was read at.
    pub slot: u64,
    /// The account exactly as mainnet held it (ProgramData: header ‖ ELF ‖ zero padding).
    pub account: Account,
    /// ProgramData only: the length of the ELF after the 45-byte header (the rest is zeros).
    pub elf_len: Option<usize>,
}

/// The snapshot a group was read at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub slot: u64,
    /// `getBlockTime(slot)`.
    pub block_time: i64,
}

struct Fixtures {
    by_name: HashMap<String, Fixture>,
    by_address: HashMap<Pubkey, String>,
    state: Snapshot,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_fixture_file(rel: &str) -> Vec<u8> {
    let path = fixtures_dir().join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn load_fixtures() -> Fixtures {
    let path = fixtures_dir().join("manifest.json");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let m: serde_json::Value = serde_json::from_str(&text).expect("manifest.json");
    let str_of = |e: &serde_json::Value, k: &str| -> String {
        e[k].as_str()
            .unwrap_or_else(|| panic!("manifest entry {}: no `{k}`", e["name"]))
            .to_owned()
    };
    let u64_of = |e: &serde_json::Value, k: &str| -> u64 {
        e[k].as_u64()
            .unwrap_or_else(|| panic!("manifest entry {}: no `{k}`", e["name"]))
    };
    let mut by_name = HashMap::new();
    let mut by_address = HashMap::new();
    for e in m["accounts"].as_array().expect("accounts") {
        let group = e["group"].as_str().unwrap_or_default();
        if group != "raydium_programs" && group != "raydium_state" {
            continue;
        }
        let name = str_of(e, "name");
        let space = usize::try_from(u64_of(e, "space")).expect("space");
        let mut elf_len = None;
        let data = if let Some(file) = e["file"].as_str() {
            read_fixture_file(file)
        } else {
            // ProgramData: header ‖ ELF ‖ zeros up to `space`.
            let mut d = read_fixture_file(e["files"]["header"].as_str().expect("header"));
            let elf = read_fixture_file(e["files"]["elf"].as_str().expect("elf"));
            assert_eq!(hex(&sha256(&elf)), str_of(e, "elf_sha256"), "{name}: ELF");
            elf_len = Some(elf.len());
            d.extend_from_slice(&elf);
            assert!(d.len() <= space, "{name}: ELF longer than the account");
            d.resize(space, 0);
            d
        };
        assert_eq!(data.len(), space, "{name}: length");
        assert_eq!(
            hex(&sha256(&data)),
            str_of(e, "sha256"),
            "{name}: the fixture bytes differ from the manifest (re-run scripts/dump-fixtures.py)"
        );
        let address = Pubkey::from_str(&str_of(e, "address")).expect("address");
        let owner = Pubkey::from_str(&str_of(e, "owner")).expect("owner");
        let fixture = Fixture {
            name: name.clone(),
            group: group.to_owned(),
            address,
            slot: u64_of(e, "slot"),
            account: Account {
                lamports: u64_of(e, "lamports"),
                data,
                owner,
                executable: e["executable"].as_bool().expect("executable"),
                rent_epoch: u64_of(e, "rent_epoch"),
            },
            elf_len,
        };
        by_address.insert(address, name.clone());
        by_name.insert(name, fixture);
    }
    let snap = m["snapshots"]
        .as_array()
        .expect("snapshots")
        .iter()
        .find(|s| s["group"] == "raydium_state")
        .expect("the raydium_state snapshot");
    Fixtures {
        by_name,
        by_address,
        state: Snapshot {
            slot: snap["slot"].as_u64().expect("slot"),
            block_time: snap["block_time"].as_i64().expect("block_time"),
        },
    }
}

fn fixtures() -> &'static Fixtures {
    static F: OnceLock<Fixtures> = OnceLock::new();
    F.get_or_init(load_fixtures)
}

/// The fixture named `name` (panics if the manifest has no such Raydium entry).
pub fn fixture(name: &str) -> &'static Fixture {
    fixtures()
        .by_name
        .get(name)
        .unwrap_or_else(|| panic!("no Raydium fixture `{name}` in manifest.json"))
}

/// The fixture at `address`, if the Raydium groups hold one.
pub fn fixture_at(address: &Pubkey) -> Option<&'static Fixture> {
    let f = fixtures();
    f.by_address.get(address).map(|n| &f.by_name[n])
}

/// Every Raydium fixture (both groups).
pub fn all_fixtures() -> impl Iterator<Item = &'static Fixture> {
    fixtures().by_name.values()
}

/// The slot and block time every venue-state fixture was read at.
pub fn state_snapshot() -> Snapshot {
    fixtures().state
}

/// Writes fixture `name` into the VM (as mainnet held it). Returns its address.
pub fn load(svm: &mut LiteSVM, name: &str) -> Pubkey {
    let f = fixture(name);
    svm.set_account(f.address, f.account.clone())
        .unwrap_or_else(|e| panic!("set_account({name}): {e:?}"));
    f.address
}

/// Writes the fixture at `address` into the VM; panics if there is none.
pub fn load_at(svm: &mut LiteSVM, address: &Pubkey) {
    let f = fixture_at(address).unwrap_or_else(|| panic!("no fixture at {address}"));
    load(svm, &f.name);
}

// ---- programs ------------------------------------------------------------------------------

/// The two Raydium programs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaydiumProgram {
    AmmV4,
    Cpmm,
}

impl RaydiumProgram {
    pub const ALL: [Self; 2] = [Self::AmmV4, Self::Cpmm];

    fn prefix(self) -> &'static str {
        match self {
            Self::AmmV4 => "raydium_amm_v4",
            Self::Cpmm => "raydium_cpmm",
        }
    }

    /// The program id.
    pub fn id(self) -> Pubkey {
        match self {
            Self::AmmV4 => RAYDIUM_V4_PROGRAM_ID,
            Self::Cpmm => RAYDIUM_CPMM_PROGRAM_ID,
        }
    }

    /// The fixture of its ProgramData account.
    pub fn program_data(self) -> &'static Fixture {
        fixture(&format!("{}_program_data", self.prefix()))
    }

    /// The fixture of its program account.
    pub fn program(self) -> &'static Fixture {
        fixture(&format!("{}_program", self.prefix()))
    }

    /// The dumped ELF (the ProgramData after its 45-byte header, up to the ELF's own end).
    pub fn elf(self) -> &'static [u8] {
        let pd = self.program_data();
        let start = shortwin_venue_state::program_data::HEADER_LEN;
        let len = pd.elf_len.expect("a ProgramData fixture");
        &pd.account.data[start..start + len]
    }
}

/// Deploys `program` at its mainnet address from the dumped accounts: the ProgramData first
/// (the real header: deploy slot and upgrade authority), then the executable program account,
/// which makes LiteSVM compile the ELF.
pub fn load_program(svm: &mut LiteSVM, program: RaydiumProgram) {
    let pd = program.program_data();
    load(svm, &pd.name);
    load(svm, &program.program().name);
}

/// Rewrites the deploy slot in `program`'s ProgramData header (an "upgrade" for the latch
/// tests, T-LATCH), keeping the ELF and the authority. The program keeps running the dumped
/// ELF.
pub fn set_deploy_slot(svm: &mut LiteSVM, program: RaydiumProgram, slot: u64) {
    patch(svm, &program.program_data().address, |d| {
        d[4..12].copy_from_slice(&slot.to_le_bytes());
    });
}

impl Env {
    /// Deploys both Raydium programs from the dumped ProgramData ([`load_program`]).
    pub fn load_raydium_programs(&mut self) {
        for p in RaydiumProgram::ALL {
            load_program(&mut self.svm, p);
        }
    }

    /// Sets the clock to the snapshot's slot and block time ([`state_snapshot`]); time only
    /// moves forward from there ([`Env::warp`]).
    pub fn set_fork_clock(&mut self) {
        let s = state_snapshot();
        self.set_clock(s.block_time, s.slot);
    }

    /// Loads AMM v4 pool `which` and every account its swaps need (pool, vaults, both mints,
    /// LP mint, the authority, the AMM v4 `AmmConfig`), and the AMM v4 program if missing.
    pub fn load_ray_v4(&mut self, which: V4Fixture) -> V4Pool {
        if self.svm.get_account(&RAYDIUM_V4_PROGRAM_ID).is_none() {
            load_program(&mut self.svm, RaydiumProgram::AmmV4);
        }
        let pool = load(&mut self.svm, &format!("{}_pool", which.prefix()));
        let info = raydium_v4::read(
            &raydium_v4::PROGRAM,
            &fixture(&format!("{}_pool", which.prefix())).account.data,
        )
        .expect("fixture pool");
        let keys = V4Pool::from_info(which, pool, &info);
        for k in [
            keys.coin_vault,
            keys.pc_vault,
            keys.coin_mint,
            keys.pc_mint,
            keys.lp_mint,
            RAYDIUM_V4_AUTHORITY,
        ] {
            load_at(&mut self.svm, &k);
        }
        load(&mut self.svm, "ray_v4_amm_config");
        keys
    }

    /// Loads CPMM pool `which` and every account its swaps need (pool, its `AmmConfig`, vaults,
    /// mints, LP mint, observation, the authority), all 21 live `AmmConfig`s (fee variants),
    /// and the CPMM program if missing.
    pub fn load_cpmm(&mut self, which: CpmmFixture) -> CpmmPool {
        if self.svm.get_account(&RAYDIUM_CPMM_PROGRAM_ID).is_none() {
            load_program(&mut self.svm, RaydiumProgram::Cpmm);
        }
        let name = format!("{}_pool", which.prefix());
        let pool = load(&mut self.svm, &name);
        let state = raydium_cpmm::read_pool(&raydium_cpmm::PROGRAM, &fixture(&name).account.data)
            .expect("fixture pool");
        let keys = CpmmPool::from_state(which, pool, &state);
        for k in [
            keys.amm_config,
            keys.token_0_vault,
            keys.token_1_vault,
            keys.token_0_mint,
            keys.token_1_mint,
            keys.lp_mint,
            keys.observation,
            RAYDIUM_CPMM_AUTHORITY,
        ] {
            load_at(&mut self.svm, &k);
        }
        for index in 0..CPMM_CONFIGS {
            load(&mut self.svm, &cpmm_config_name(index));
        }
        keys
    }

    /// [`Env::initialized`] (release short.win, config at the harness defaults, whose venue
    /// deploy slots are the dumped ones), both Raydium programs, the Fartcoin AMM v4 pool
    /// `Bzc9NZfM…`, the clock at the snapshot.
    pub fn fork_raydium_v4() -> (Self, V4Pool) {
        let mut env = Self::initialized();
        env.load_raydium_programs();
        let pool = env.load_ray_v4(V4Fixture::Fartcoin);
        env.set_fork_clock();
        (env, pool)
    }

    /// [`Env::initialized`], both Raydium programs, the USELESS CPMM pool `Q2sPHPdU…` (with all
    /// 21 configs), the clock at the snapshot.
    pub fn fork_cpmm() -> (Self, CpmmPool) {
        let mut env = Self::initialized();
        env.load_raydium_programs();
        let pool = env.load_cpmm(CpmmFixture::Useless);
        env.set_fork_clock();
        (env, pool)
    }
}

// ---- AMM v4 ---------------------------------------------------------------------------------

/// The dumped AMM v4 pools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V4Fixture {
    /// `Bzc9NZfM…`: Fartcoin's main pool, WSOL is the coin (R-RAY §6.4).
    Fartcoin,
    /// `9GWMrDgH…`: the tiny reversed-order Fartcoin pool, WSOL is the pc, `need_take_pnl` ≠ 0.
    FartcoinReversed,
}

impl V4Fixture {
    pub const ALL: [Self; 2] = [Self::Fartcoin, Self::FartcoinReversed];

    /// The manifest name prefix.
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Fartcoin => "ray_v4_fartcoin",
            Self::FartcoinReversed => "ray_v4_fartcoin_rev",
        }
    }
}

/// An AMM v4 pool's keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V4Pool {
    pub which: V4Fixture,
    pub pool: Pubkey,
    pub coin_vault: Pubkey,
    pub pc_vault: Pubkey,
    pub coin_mint: Pubkey,
    pub pc_mint: Pubkey,
    pub lp_mint: Pubkey,
    /// WSOL is the coin (else the pc).
    pub sol_is_coin: bool,
}

/// The AMM v4 `ExceededSlippage` error (`AmmError` 0x1e).
pub const V4_ERR_EXCEEDED_SLIPPAGE: u32 = 0x1e;
/// The AMM v4 `InvalidStatus` error (`AmmError` 0x16).
pub const V4_ERR_INVALID_STATUS: u32 = 0x16;
/// The AMM v4 `InvalidInput` error (`AmmError` 0x1d).
pub const V4_ERR_INVALID_INPUT: u32 = 0x1d;
/// The AMM v4 `CheckedSubOverflow` error (`AmmError` 0x20).
pub const V4_ERR_CHECKED_SUB_OVERFLOW: u32 = 0x20;
/// The AMM v4 `InsufficientFunds` error (`AmmError` 0x28).
pub const V4_ERR_INSUFFICIENT_FUNDS: u32 = 0x28;
/// The AMM v4 `InvalidSignAccount` error (`AmmError` 0x15).
pub const V4_ERR_INVALID_SIGN_ACCOUNT: u32 = 0x15;
/// The AMM v4 `InvalidFee` error (`AmmError` 0x30; a fee fraction with `num ≥ den`).
pub const V4_ERR_INVALID_FEE: u32 = 0x30;

/// AMM v4 `SwapBaseInV2` tag.
pub const V4_TAG_SWAP_BASE_IN_V2: u8 = 16;
/// AMM v4 `SwapBaseOutV2` tag.
pub const V4_TAG_SWAP_BASE_OUT_V2: u8 = 17;
/// AMM v4 `SetParams` tag (admin).
pub const V4_TAG_SET_PARAMS: u8 = 6;

fn key(k: &shortwin_venue_state::Key) -> Pubkey {
    Pubkey::new_from_array(*k)
}

impl V4Pool {
    fn from_info(which: V4Fixture, pool: Pubkey, info: &raydium_v4::AmmInfo) -> Self {
        Self {
            which,
            pool,
            coin_vault: key(&info.coin_vault),
            pc_vault: key(&info.pc_vault),
            coin_mint: key(&info.coin_vault_mint),
            pc_mint: key(&info.pc_vault_mint),
            lp_mint: key(&info.lp_mint),
            sol_is_coin: info.sol_is_coin().expect("a SOL pool"),
        }
    }

    /// The token (non-WSOL) mint.
    pub fn token_mint(&self) -> Pubkey {
        if self.sol_is_coin {
            self.pc_mint
        } else {
            self.coin_mint
        }
    }

    /// `(input vault, output vault)` of a direction.
    pub fn io_vaults(&self, dir: Direction) -> (Pubkey, Pubkey) {
        let (sol, tok) = if self.sol_is_coin {
            (self.coin_vault, self.pc_vault)
        } else {
            (self.pc_vault, self.coin_vault)
        };
        match dir {
            Direction::Buy => (sol, tok),
            Direction::Sell => (tok, sol),
        }
    }

    /// The pool as the reader sees it now.
    pub fn info(&self, svm: &LiteSVM) -> raydium_v4::AmmInfo {
        let acc = svm.get_account(&self.pool).expect("pool");
        raydium_v4::read(&acc.owner.to_bytes(), &acc.data).expect("AmmInfo")
    }

    /// `(coin_vault.amount, pc_vault.amount)`.
    pub fn vault_amounts(&self, svm: &LiteSVM) -> (u64, u64) {
        (
            vault_amount(svm, &self.coin_vault),
            vault_amount(svm, &self.pc_vault),
        )
    }

    /// The reader's pricing reserves `(coin, pc)`; `None` if the venue cannot price.
    pub fn reserves(&self, svm: &LiteSVM) -> Option<(u64, u64)> {
        let (c, p) = self.vault_amounts(svm);
        self.info(svm).reserves(c, p)
    }

    /// `(input reserve, output reserve)` of a direction.
    pub fn io_reserves(&self, svm: &LiteSVM, dir: Direction) -> Option<(u64, u64)> {
        let (coin, pc) = self.reserves(svm)?;
        let (sol, tok) = if self.sol_is_coin {
            (coin, pc)
        } else {
            (pc, coin)
        };
        Some(match dir {
            Direction::Buy => (sol, tok),
            Direction::Sell => (tok, sol),
        })
    }

    /// `ARCH §5.3`'s predicate on the live state.
    pub fn venue_executable(&self, svm: &LiteSVM, now: i64) -> bool {
        let (c, p) = self.vault_amounts(svm);
        self.info(svm).venue_executable(now, c, p)
    }

    /// `quote::ray_v4::sell_exact_in` of `amount_in` in direction `dir` on the live state
    /// (the formula is direction-agnostic: input reserve, output reserve).
    pub fn quote_exact_in(
        &self,
        svm: &LiteSVM,
        dir: Direction,
        amount_in: u64,
    ) -> Option<SellQuote> {
        let (x, y) = self.io_reserves(svm, dir)?;
        let i = self.info(svm);
        qv4::sell_exact_in(
            amount_in,
            u128::from(x),
            u128::from(y),
            i.swap_fee_numerator,
            i.swap_fee_denominator,
        )
    }

    /// `quote::ray_v4::buy_exact_out` of `amount_out` in direction `dir` on the live state.
    pub fn quote_exact_out(
        &self,
        svm: &LiteSVM,
        dir: Direction,
        amount_out: u64,
    ) -> Option<BuyQuote> {
        let (x, y) = self.io_reserves(svm, dir)?;
        let i = self.info(svm);
        qv4::buy_exact_out(
            amount_out,
            u128::from(x),
            u128::from(y),
            i.swap_fee_numerator,
            i.swap_fee_denominator,
        )
    }

    /// The v2 swap accounts (R-RAY §3.1): `[Tokenkeg, pool W, authority, coin_vault W,
    /// pc_vault W, source W, destination W, owner S]`.
    pub fn swap_accounts(
        &self,
        owner: &Pubkey,
        source: &Pubkey,
        destination: &Pubkey,
    ) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new(self.pool, false),
            AccountMeta::new_readonly(RAYDIUM_V4_AUTHORITY, false),
            AccountMeta::new(self.coin_vault, false),
            AccountMeta::new(self.pc_vault, false),
            AccountMeta::new(*source, false),
            AccountMeta::new(*destination, false),
            AccountMeta::new_readonly(*owner, true),
        ]
    }

    /// `swap_base_in_v2` (tag 16): `[16 | amount_in | minimum_amount_out]`.
    pub fn swap_base_in_v2_ix(
        &self,
        owner: &Pubkey,
        source: &Pubkey,
        destination: &Pubkey,
        amount_in: u64,
        minimum_amount_out: u64,
    ) -> Instruction {
        Instruction {
            program_id: RAYDIUM_V4_PROGRAM_ID,
            accounts: self.swap_accounts(owner, source, destination),
            data: tagged(V4_TAG_SWAP_BASE_IN_V2, &[amount_in, minimum_amount_out]),
        }
    }

    /// `swap_base_out_v2` (tag 17): `[17 | max_amount_in | amount_out]`.
    pub fn swap_base_out_v2_ix(
        &self,
        owner: &Pubkey,
        source: &Pubkey,
        destination: &Pubkey,
        max_amount_in: u64,
        amount_out: u64,
    ) -> Instruction {
        Instruction {
            program_id: RAYDIUM_V4_PROGRAM_ID,
            accounts: self.swap_accounts(owner, source, destination),
            data: tagged(V4_TAG_SWAP_BASE_OUT_V2, &[max_amount_in, amount_out]),
        }
    }
}

fn tagged(tag: u8, args: &[u64]) -> Vec<u8> {
    let mut d = vec![tag];
    for a in args {
        d.extend_from_slice(&a.to_le_bytes());
    }
    d
}

// ---- CPMM -----------------------------------------------------------------------------------

/// The dumped CPMM pools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpmmFixture {
    /// `Q2sPHPdU…` USELESS: WSOL token 0, config 0, creator fee off.
    Useless,
    /// `FQd8z3…`: WSOL token 0, config 0, creator fee on token 0 (= WSOL).
    CreatorFee,
    /// `7ZRtFcgf…`: token 1 under Token-2022, config 0, creator fee off.
    Token2022,
    /// `3Cv9uPpa…`: WSOL token 1, config 10, creator fee on token 1 (= WSOL).
    WsolToken1,
    /// `FrZnJAFt…`: WSOL token 0, config 0, creator fee on both tokens (always the input).
    CreatorBoth,
}

impl CpmmFixture {
    pub const ALL: [Self; 5] = [
        Self::Useless,
        Self::CreatorFee,
        Self::Token2022,
        Self::WsolToken1,
        Self::CreatorBoth,
    ];

    /// The manifest name prefix.
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Useless => "cpmm_useless",
            Self::CreatorFee => "cpmm_creator_fee",
            Self::Token2022 => "cpmm_token2022",
            Self::WsolToken1 => "cpmm_wsol_token1",
            Self::CreatorBoth => "cpmm_creator_both",
        }
    }
}

/// How many live CPMM `AmmConfig`s the fixtures hold (indices 0–20, R-RAY §2.5).
pub const CPMM_CONFIGS: u16 = 21;

/// The manifest name of CPMM config `index`.
pub fn cpmm_config_name(index: u16) -> String {
    format!("cpmm_config_{index:02}")
}

/// The address of CPMM config `index` (the fixture's; equals PDA `["amm_config", index BE]`).
pub fn cpmm_config(index: u16) -> Pubkey {
    fixture(&cpmm_config_name(index)).address
}

/// `sha256("global:swap_base_input")[..8]`.
pub const CPMM_SWAP_BASE_INPUT: [u8; 8] = [143, 190, 90, 218, 196, 30, 51, 222];
/// `sha256("global:swap_base_output")[..8]`.
pub const CPMM_SWAP_BASE_OUTPUT: [u8; 8] = [55, 217, 98, 86, 163, 74, 180, 173];
/// `sha256("global:update_pool_status")[..8]` (admin).
pub const CPMM_UPDATE_POOL_STATUS: [u8; 8] = [130, 87, 108, 6, 46, 224, 117, 123];
/// `sha256("global:update_amm_config")[..8]` (admin).
pub const CPMM_UPDATE_AMM_CONFIG: [u8; 8] = [49, 60, 174, 136, 154, 28, 116, 200];
/// `sha256("event:SwapEvent")[..8]`.
pub const CPMM_SWAP_EVENT: [u8; 8] = [64, 198, 205, 232, 38, 8, 113, 226];

/// CPMM `NotApproved` (swap bit or before `open_time`).
pub const CPMM_ERR_NOT_APPROVED: u32 = 6000;
/// CPMM `ExceededSlippage`.
pub const CPMM_ERR_EXCEEDED_SLIPPAGE: u32 = 6005;
/// CPMM `ZeroTradingTokens` (the curve has no answer).
pub const CPMM_ERR_ZERO_TRADING_TOKENS: u32 = 6006;
/// CPMM `MathOverflow` (e.g. the fee counters' sum).
pub const CPMM_ERR_MATH_OVERFLOW: u32 = 6011;
/// CPMM `InsufficientVault` (a vault below its counters).
pub const CPMM_ERR_INSUFFICIENT_VAULT: u32 = 6012;
/// CPMM `InvalidFeeModel` (`creator_fee_on > 2`).
pub const CPMM_ERR_INVALID_FEE_MODEL: u32 = 6013;

/// A CPMM pool's keys (the config key as loaded; [`CpmmPool::config_key`] reads the live one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpmmPool {
    pub which: CpmmFixture,
    pub pool: Pubkey,
    pub amm_config: Pubkey,
    pub observation: Pubkey,
    pub token_0_vault: Pubkey,
    pub token_1_vault: Pubkey,
    pub token_0_mint: Pubkey,
    pub token_1_mint: Pubkey,
    pub token_0_program: Pubkey,
    pub token_1_program: Pubkey,
    pub lp_mint: Pubkey,
    /// WSOL is token 0 (else token 1).
    pub sol_is_token0: bool,
}

impl CpmmPool {
    fn from_state(which: CpmmFixture, pool: Pubkey, s: &raydium_cpmm::PoolState) -> Self {
        Self {
            which,
            pool,
            amm_config: key(&s.amm_config),
            observation: key(&s.observation_key),
            token_0_vault: key(&s.token_0_vault),
            token_1_vault: key(&s.token_1_vault),
            token_0_mint: key(&s.token_0_mint),
            token_1_mint: key(&s.token_1_mint),
            token_0_program: key(&s.token_0_program),
            token_1_program: key(&s.token_1_program),
            lp_mint: key(&s.lp_mint),
            sol_is_token0: s.sol_is_token0().expect("a SOL pool"),
        }
    }

    /// The token (non-WSOL) mint.
    pub fn token_mint(&self) -> Pubkey {
        if self.sol_is_token0 {
            self.token_1_mint
        } else {
            self.token_0_mint
        }
    }

    /// The token (non-WSOL) side's program.
    pub fn token_program(&self) -> TokenProgram {
        let p = if self.sol_is_token0 {
            self.token_1_program
        } else {
            self.token_0_program
        };
        if p == TokenProgram::Token2022.id() {
            TokenProgram::Token2022
        } else {
            TokenProgram::Spl
        }
    }

    /// The input token of direction `dir` is token 0.
    pub fn input_is_token0(&self, dir: Direction) -> bool {
        (dir == Direction::Buy) == self.sol_is_token0
    }

    /// `(input vault, output vault, input mint, output mint, input program, output program)`.
    pub fn io(&self, dir: Direction) -> (Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey) {
        let zero = (self.token_0_vault, self.token_0_mint, self.token_0_program);
        let one = (self.token_1_vault, self.token_1_mint, self.token_1_program);
        let (i, o) = if self.input_is_token0(dir) {
            (zero, one)
        } else {
            (one, zero)
        };
        (i.0, o.0, i.1, o.1, i.2, o.2)
    }

    /// The pool as the reader sees it now.
    pub fn state(&self, svm: &LiteSVM) -> raydium_cpmm::PoolState {
        let acc = svm.get_account(&self.pool).expect("pool");
        raydium_cpmm::read_pool(&acc.owner.to_bytes(), &acc.data).expect("PoolState")
    }

    /// The config key the pool names now.
    pub fn config_key(&self, svm: &LiteSVM) -> Pubkey {
        key(&self.state(svm).amm_config)
    }

    /// The pool's `AmmConfig` as the reader sees it now.
    pub fn config(&self, svm: &LiteSVM) -> raydium_cpmm::AmmConfig {
        let acc = svm.get_account(&self.config_key(svm)).expect("amm_config");
        raydium_cpmm::read_config(&acc.owner.to_bytes(), &acc.data).expect("AmmConfig")
    }

    /// `(token_0_vault.amount, token_1_vault.amount)`.
    pub fn vault_amounts(&self, svm: &LiteSVM) -> (u64, u64) {
        (
            vault_amount(svm, &self.token_0_vault),
            vault_amount(svm, &self.token_1_vault),
        )
    }

    /// The reader's pricing reserves `(r0, r1)`; `None` if the venue cannot price.
    pub fn reserves(&self, svm: &LiteSVM) -> Option<(u64, u64)> {
        let (v0, v1) = self.vault_amounts(svm);
        self.state(svm).reserves(v0, v1)
    }

    /// `(input reserve, output reserve)` of a direction.
    pub fn io_reserves(&self, svm: &LiteSVM, dir: Direction) -> Option<(u64, u64)> {
        let (r0, r1) = self.reserves(svm)?;
        Some(if self.input_is_token0(dir) {
            (r0, r1)
        } else {
            (r1, r0)
        })
    }

    /// `ARCH §5.3`'s predicate on the live state and config (M3-SEC-1's fee split included).
    pub fn venue_executable(&self, svm: &LiteSVM, now: i64) -> bool {
        let (v0, v1) = self.vault_amounts(svm);
        self.state(svm)
            .venue_executable(&self.config(svm), now, v0, v1)
    }

    /// short.win's resolved fees for `dir` (`fees::cpmm` on the reader's config and pool
    /// fields); `None` where the venue cannot swap (`fees::cpmm`'s `InvalidInput`).
    pub fn fees(&self, svm: &LiteSVM, dir: Direction) -> Option<CpmmFees> {
        let s = self.state(svm);
        let c = self.config(svm);
        mfees::cpmm(
            &CpmmConfig {
                trade_fee_rate: c.trade_fee_rate,
                protocol_fee_rate: c.protocol_fee_rate,
                fund_fee_rate: c.fund_fee_rate,
                creator_fee_rate: c.creator_fee_rate,
            },
            &CpmmPoolFees {
                enable_creator_fee: s.enable_creator_fee,
                creator_fee_on: s.creator_fee_on,
                sol_is_token0: self.sol_is_token0,
            },
            dir,
        )
        .ok()
    }

    /// `quote::cpmm::sell_exact_in` of `amount_in` in direction `dir` on the live state (the
    /// formula is direction-agnostic: input reserve, output reserve, the direction's fees).
    pub fn quote_exact_in(
        &self,
        svm: &LiteSVM,
        dir: Direction,
        amount_in: u64,
    ) -> Option<SellQuote> {
        let (x, y) = self.io_reserves(svm, dir)?;
        qcpmm::sell_exact_in(
            amount_in,
            u128::from(x),
            u128::from(y),
            &self.fees(svm, dir)?,
        )
    }

    /// `quote::cpmm::buy_exact_out` of `amount_out` in direction `dir` on the live state.
    pub fn quote_exact_out(
        &self,
        svm: &LiteSVM,
        dir: Direction,
        amount_out: u64,
    ) -> Option<BuyQuote> {
        let (x, y) = self.io_reserves(svm, dir)?;
        qcpmm::buy_exact_out(
            amount_out,
            u128::from(x),
            u128::from(y),
            &self.fees(svm, dir)?,
        )
    }

    /// The 13 `Swap` accounts (R-RAY §3.2): `[payer S, authority, amm_config, pool W, input W,
    /// output W, input_vault W, output_vault W, input_program, output_program, input_mint,
    /// output_mint, observation W]`, with the config key the pool names now.
    pub fn swap_accounts(
        &self,
        svm: &LiteSVM,
        payer: &Pubkey,
        input: &Pubkey,
        output: &Pubkey,
        dir: Direction,
    ) -> Vec<AccountMeta> {
        let (iv, ov, im, om, ip, op) = self.io(dir);
        vec![
            AccountMeta::new_readonly(*payer, true),
            AccountMeta::new_readonly(RAYDIUM_CPMM_AUTHORITY, false),
            AccountMeta::new_readonly(self.config_key(svm), false),
            AccountMeta::new(self.pool, false),
            AccountMeta::new(*input, false),
            AccountMeta::new(*output, false),
            AccountMeta::new(iv, false),
            AccountMeta::new(ov, false),
            AccountMeta::new_readonly(ip, false),
            AccountMeta::new_readonly(op, false),
            AccountMeta::new_readonly(im, false),
            AccountMeta::new_readonly(om, false),
            AccountMeta::new(self.observation, false),
        ]
    }

    /// `swap_base_input(amount_in, minimum_amount_out)`.
    #[allow(clippy::too_many_arguments)]
    pub fn swap_base_input_ix(
        &self,
        svm: &LiteSVM,
        payer: &Pubkey,
        input: &Pubkey,
        output: &Pubkey,
        dir: Direction,
        amount_in: u64,
        minimum_amount_out: u64,
    ) -> Instruction {
        let mut data = CPMM_SWAP_BASE_INPUT.to_vec();
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());
        Instruction {
            program_id: RAYDIUM_CPMM_PROGRAM_ID,
            accounts: self.swap_accounts(svm, payer, input, output, dir),
            data,
        }
    }

    /// `swap_base_output(max_amount_in, amount_out)`.
    #[allow(clippy::too_many_arguments)]
    pub fn swap_base_output_ix(
        &self,
        svm: &LiteSVM,
        payer: &Pubkey,
        input: &Pubkey,
        output: &Pubkey,
        dir: Direction,
        max_amount_in: u64,
        amount_out: u64,
    ) -> Instruction {
        let mut data = CPMM_SWAP_BASE_OUTPUT.to_vec();
        data.extend_from_slice(&max_amount_in.to_le_bytes());
        data.extend_from_slice(&amount_out.to_le_bytes());
        Instruction {
            program_id: RAYDIUM_CPMM_PROGRAM_ID,
            accounts: self.swap_accounts(svm, payer, input, output, dir),
            data,
        }
    }
}

// ---- users and swaps --------------------------------------------------------------------------

/// The amount of a token account (panics if it is not one).
pub fn vault_amount(svm: &LiteSVM, key: &Pubkey) -> u64 {
    read::token_amount(svm, key).unwrap_or_else(|| panic!("{key} is not a token account"))
}

/// Refills LiteSVM's airdrop account when a large wrap would drain it (setup only).
fn ensure_airdrop(svm: &mut LiteSVM, lamports: u64) {
    let key = svm.airdrop_pubkey();
    let have = svm.get_account(&key).map_or(0, |a| a.lamports);
    if have < lamports.saturating_add(1_000_000_000_000) {
        svm.set_account(
            key,
            Account {
                lamports: u64::MAX / 4,
                data: vec![],
                owner: anchor_lang::system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap_or_else(|e| panic!("refill airdrop: {e:?}"));
    }
}

/// A new WSOL token account (SPL Token, not an ATA) owned by `owner` holding `lamports`
/// wrapped: created by the Token program on the dumped native mint, credited, `SyncNative`.
pub fn fund_wsol(env: &mut Env, owner: &Pubkey, lamports: u64) -> Pubkey {
    let payer = env.payer.insecure_clone();
    let spl = TOKEN_PROGRAM_ID;
    let acc = CreateAccount::new(&mut env.svm, &payer, &WSOL_MINT)
        .owner(owner)
        .token_program_id(&spl)
        .send()
        .unwrap_or_else(|e| panic!("create WSOL account: {:?}", e.err));
    if lamports > 0 {
        wrap(env, &acc, lamports);
    }
    acc
}

/// Adds `lamports` wrapped lamports to the WSOL account `acc` (credit + `SyncNative`). Each
/// call runs on a fresh blockhash, so repeating the same top-up is not a duplicate transaction.
pub fn wrap(env: &mut Env, acc: &Pubkey, lamports: u64) {
    env.svm.expire_blockhash();
    ensure_airdrop(&mut env.svm, lamports);
    env.svm
        .airdrop(acc, lamports)
        .unwrap_or_else(|e| panic!("airdrop to {acc}: {:?}", e.err));
    let payer = env.payer.insecure_clone();
    let spl = TOKEN_PROGRAM_ID;
    SyncNative::new(&mut env.svm, &payer, acc)
        .token_program_id(&spl)
        .send()
        .unwrap_or_else(|e| panic!("sync_native: {:?}", e.err));
}

/// A new token account of `mint` (under `program`) owned by `owner`, created by the token
/// program, holding `amount`. The mainnet mints have no mint authority, so the balance is
/// planted in the account's `amount` field (the mint's supply is not adjusted; no venue reads
/// it).
pub fn fund_token(
    env: &mut Env,
    owner: &Pubkey,
    mint: &Pubkey,
    program: TokenProgram,
    amount: u64,
) -> Pubkey {
    let payer = env.payer.insecure_clone();
    let id = program.id();
    let acc = CreateAccount::new(&mut env.svm, &payer, mint)
        .owner(owner)
        .token_program_id(&id)
        .send()
        .unwrap_or_else(|e| panic!("create token account: {:?}", e.err));
    if amount > 0 {
        set_token_amount(&mut env.svm, &acc, amount);
    }
    acc
}

/// A user with a WSOL account and a token account of the pool's token.
pub struct Trader {
    pub owner: Keypair,
    pub wsol: Pubkey,
    pub token: Pubkey,
}

impl Trader {
    /// `(source, destination)` of a direction.
    pub fn accounts(&self, dir: Direction) -> (Pubkey, Pubkey) {
        match dir {
            Direction::Buy => (self.wsol, self.token),
            Direction::Sell => (self.token, self.wsol),
        }
    }
}

/// A funded user of a pool whose token is `mint` under `program`: `wsol` lamports wrapped and
/// `tokens` raw units.
pub fn trader(
    env: &mut Env,
    mint: &Pubkey,
    program: TokenProgram,
    wsol: u64,
    tokens: u64,
) -> Trader {
    let owner = env.funded_keypair();
    let w = fund_wsol(env, &owner.pubkey(), wsol);
    let t = fund_token(env, &owner.pubkey(), mint, program, tokens);
    Trader {
        owner,
        wsol: w,
        token: t,
    }
}

/// A swap's mode and bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swap {
    /// `(amount_in, minimum_amount_out)`.
    ExactIn(u64, u64),
    /// `(max_amount_in, amount_out)`.
    ExactOut(u64, u64),
}

/// Any dumped pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnyPool {
    V4(V4Pool),
    Cpmm(CpmmPool),
}

impl AnyPool {
    /// The pool's token mint and its program.
    pub fn token(&self) -> (Pubkey, TokenProgram) {
        match self {
            Self::V4(p) => (p.token_mint(), TokenProgram::Spl),
            Self::Cpmm(p) => (p.token_mint(), p.token_program()),
        }
    }

    /// The venue's swap instruction for `trader` (who signs as source owner / payer).
    pub fn swap_ix(
        &self,
        svm: &LiteSVM,
        owner: &Pubkey,
        trader: &Trader,
        dir: Direction,
        s: Swap,
    ) -> Instruction {
        let (src, dst) = trader.accounts(dir);
        match (self, s) {
            (Self::V4(p), Swap::ExactIn(a, m)) => p.swap_base_in_v2_ix(owner, &src, &dst, a, m),
            (Self::V4(p), Swap::ExactOut(m, q)) => p.swap_base_out_v2_ix(owner, &src, &dst, m, q),
            (Self::Cpmm(p), Swap::ExactIn(a, m)) => {
                p.swap_base_input_ix(svm, owner, &src, &dst, dir, a, m)
            }
            (Self::Cpmm(p), Swap::ExactOut(m, q)) => {
                p.swap_base_output_ix(svm, owner, &src, &dst, dir, m, q)
            }
        }
    }

    /// `(input reserve, output reserve)` of a direction on the live state.
    pub fn io_reserves(&self, svm: &LiteSVM, dir: Direction) -> Option<(u64, u64)> {
        match self {
            Self::V4(p) => p.io_reserves(svm, dir),
            Self::Cpmm(p) => p.io_reserves(svm, dir),
        }
    }

    /// The exact-in quote on the live state.
    pub fn quote_exact_in(
        &self,
        svm: &LiteSVM,
        dir: Direction,
        amount_in: u64,
    ) -> Option<SellQuote> {
        match self {
            Self::V4(p) => p.quote_exact_in(svm, dir, amount_in),
            Self::Cpmm(p) => p.quote_exact_in(svm, dir, amount_in),
        }
    }

    /// The exact-out quote on the live state.
    pub fn quote_exact_out(
        &self,
        svm: &LiteSVM,
        dir: Direction,
        amount_out: u64,
    ) -> Option<BuyQuote> {
        match self {
            Self::V4(p) => p.quote_exact_out(svm, dir, amount_out),
            Self::Cpmm(p) => p.quote_exact_out(svm, dir, amount_out),
        }
    }
}

/// A direct swap on `pool` by `trader` as a normal user (the trader signs; the harness payer
/// pays the fee), through [`Env::send`].
pub fn swap(env: &mut Env, pool: &AnyPool, trader: &Trader, dir: Direction, s: Swap) -> TxResult {
    let owner = trader.owner.pubkey();
    let ix = pool.swap_ix(&env.svm, &owner, trader, dir, s);
    let signer = trader.owner.insecure_clone();
    env.send(&[ix], &[&signer])
}

// ---- scenario builders (byte-level admin actions) --------------------------------------------

/// Rewrites account `key`'s data with `f` (lamports, owner and flags unchanged).
pub fn patch(svm: &mut LiteSVM, key: &Pubkey, f: impl FnOnce(&mut Vec<u8>)) {
    let mut acc = svm
        .get_account(key)
        .unwrap_or_else(|| panic!("no account {key}"));
    f(&mut acc.data);
    svm.set_account(*key, acc)
        .unwrap_or_else(|e| panic!("set_account({key}): {e:?}"));
}

fn put_u64(d: &mut [u8], off: usize, v: u64) {
    d[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

/// Sets a token account's `amount`. A native (WSOL) account also gets `lamports = rent
/// reserve + amount`, the state `SyncNative` would leave.
pub fn set_token_amount(svm: &mut LiteSVM, key: &Pubkey, amount: u64) {
    let mut acc = svm
        .get_account(key)
        .unwrap_or_else(|| panic!("no account {key}"));
    let t = token_account::read(&acc.owner.to_bytes(), &acc.data)
        .unwrap_or_else(|e| panic!("{key} is not a token account: {e:?}"));
    put_u64(&mut acc.data, token_account::AMOUNT_OFFSET, amount);
    if let Some(rent) = t.is_native {
        acc.lamports = rent.checked_add(amount).expect("lamports");
    }
    svm.set_account(*key, acc)
        .unwrap_or_else(|e| panic!("set_account({key}): {e:?}"));
}

/// AMM v4 `status` (the admin's `SetParams` param 0 accepts 1..=7; any `u64` here).
pub fn set_v4_status(svm: &mut LiteSVM, pool: &V4Pool, status: u64) {
    patch(svm, &pool.pool, |d| {
        put_u64(d, raydium_v4::OFF_STATUS, status)
    });
}

/// AMM v4 `pool_open_time` (`SetParams` param 3).
pub fn set_v4_open_time(svm: &mut LiteSVM, pool: &V4Pool, open_time: u64) {
    patch(svm, &pool.pool, |d| {
        put_u64(d, raydium_v4::OFF_POOL_OPEN_TIME, open_time)
    });
}

/// AMM v4 swap fee `num/den` (the swap-fee half of `SetParams` param 2).
pub fn set_v4_swap_fee(svm: &mut LiteSVM, pool: &V4Pool, num: u64, den: u64) {
    patch(svm, &pool.pool, |d| {
        put_u64(d, raydium_v4::OFF_SWAP_FEE_NUMERATOR, num);
        put_u64(d, raydium_v4::OFF_SWAP_FEE_DENOMINATOR, den);
    });
}

/// AMM v4 `need_take_pnl_{coin,pc}` (what `deposit` / `withdraw` accrue for `withdrawpnl`).
pub fn set_v4_need_take_pnl(svm: &mut LiteSVM, pool: &V4Pool, coin: u64, pc: u64) {
    patch(svm, &pool.pool, |d| {
        put_u64(d, raydium_v4::OFF_NEED_TAKE_PNL_COIN, coin);
        put_u64(d, raydium_v4::OFF_NEED_TAKE_PNL_PC, pc);
    });
}

/// CPMM pool `status` (the admin's `update_pool_status`).
pub fn set_cpmm_status(svm: &mut LiteSVM, pool: &CpmmPool, status: u8) {
    patch(svm, &pool.pool, |d| d[raydium_cpmm::OFF_STATUS] = status);
}

/// CPMM pool `open_time` (no instruction can change it on mainnet; a scenario builder for
/// pools created with a future open time).
pub fn set_cpmm_open_time(svm: &mut LiteSVM, pool: &CpmmPool, open_time: u64) {
    patch(svm, &pool.pool, |d| {
        put_u64(d, raydium_cpmm::OFF_OPEN_TIME, open_time)
    });
}

/// CPMM fee counters `(protocol, fund, creator)` of token 0 and token 1.
pub fn set_cpmm_counters(
    svm: &mut LiteSVM,
    pool: &CpmmPool,
    token_0: (u64, u64, u64),
    token_1: (u64, u64, u64),
) {
    patch(svm, &pool.pool, |d| {
        put_u64(d, raydium_cpmm::OFF_PROTOCOL_FEES_TOKEN_0, token_0.0);
        put_u64(d, raydium_cpmm::OFF_FUND_FEES_TOKEN_0, token_0.1);
        put_u64(d, raydium_cpmm::OFF_CREATOR_FEES_TOKEN_0, token_0.2);
        put_u64(d, raydium_cpmm::OFF_PROTOCOL_FEES_TOKEN_1, token_1.0);
        put_u64(d, raydium_cpmm::OFF_FUND_FEES_TOKEN_1, token_1.1);
        put_u64(d, raydium_cpmm::OFF_CREATOR_FEES_TOKEN_1, token_1.2);
    });
}

/// CPMM `enable_creator_fee` and `creator_fee_on` (fixed at pool creation on mainnet; a
/// scenario builder for the fee variants no dumped pool has, e.g. the fee on the token side).
pub fn set_cpmm_creator_fee(svm: &mut LiteSVM, pool: &CpmmPool, enable: bool, creator_fee_on: u8) {
    patch(svm, &pool.pool, |d| {
        d[raydium_cpmm::OFF_ENABLE_CREATOR_FEE] = u8::from(enable);
        d[raydium_cpmm::OFF_CREATOR_FEE_ON] = creator_fee_on;
    });
}

/// Points the pool at another `AmmConfig` (a fee-variant builder: the pool then prices and
/// swaps with that config's rates; the swap must pass the new key).
pub fn set_cpmm_pool_config(svm: &mut LiteSVM, pool: &CpmmPool, config: &Pubkey) {
    patch(svm, &pool.pool, |d| {
        d[raydium_cpmm::OFF_AMM_CONFIG..raydium_cpmm::OFF_AMM_CONFIG + 32]
            .copy_from_slice(config.as_ref());
    });
}

/// `AmmConfig` rates `(trade, protocol, fund, creator)` in ppm (the admin's
/// `update_amm_config` params 0, 1, 2, 7; every pool on the config at once).
pub fn set_cpmm_config_rates(
    svm: &mut LiteSVM,
    config: &Pubkey,
    trade: u64,
    protocol: u64,
    fund: u64,
    creator: u64,
) {
    patch(svm, config, |d| {
        put_u64(d, raydium_cpmm::OFF_CONFIG_TRADE_FEE_RATE, trade);
        put_u64(d, raydium_cpmm::OFF_CONFIG_PROTOCOL_FEE_RATE, protocol);
        put_u64(d, raydium_cpmm::OFF_CONFIG_FUND_FEE_RATE, fund);
        put_u64(d, raydium_cpmm::OFF_CONFIG_CREATOR_FEE_RATE, creator);
    });
}

// ---- real admin instructions (forged admin signature) -----------------------------------------

/// Sends `ix` with its signer metas `forged` signed by nobody: LiteSVM's signature check is off
/// for this one transaction (the Raydium pool admin `GThUX1…` is a Squads vault whose key no
/// test holds), the harness payer pays. The invariant hooks run as in [`Env::send`] for a
/// one-instruction transaction: [`invariants::check_ix`] against the state before, then
/// [`invariants::check_all`] on every tracked market (the instruction's own markets too).
pub fn admin_send(env: &mut Env, ix: Instruction, forged: &[Pubkey]) -> TxResult {
    assert!(
        forged
            .iter()
            .all(|k| ix.accounts.iter().any(|m| m.is_signer && m.pubkey == *k)),
        "admin_send: every forged key must be a signer meta of the instruction"
    );
    for m in &ix.accounts {
        if read::zc::<Market>(&env.svm, &m.pubkey).is_some() {
            env.track_market(m.pubkey);
        }
    }
    let admins = env.admin_keys();
    let pre = invariants::snapshot(&env.svm, env.tracked_markets());
    let payer = env.payer.insecure_clone();
    let blockhash = env.svm.latest_blockhash();
    let mut tx = Transaction::new_unsigned(Message::new(
        std::slice::from_ref(&ix),
        Some(&payer.pubkey()),
    ));
    tx.try_partial_sign(&[&payer], blockhash)
        .unwrap_or_else(|e| panic!("sign: {e:?}"));
    let sigverify = env.svm.get_sigverify();
    env.svm = std::mem::take(&mut env.svm).with_sigverify(false);
    let res = env.svm.send_transaction(tx);
    env.svm = std::mem::take(&mut env.svm).with_sigverify(sigverify);
    env.svm.expire_blockhash();
    if env.check_invariants {
        if let Ok(meta) = &res {
            let ev = invariants::IxEvents::of_instruction(meta, 0, &ix);
            let admin = invariants::is_admin_signed(&ix, &admins);
            if let Err(v) = invariants::check_ix(&env.svm, &pre, admin, &ev) {
                panic!("{v}\nlogs:\n{}", meta.pretty_logs());
            }
        }
        env.assert_invariants();
    }
    res
}

/// The pool admin of both Raydium programs (`GThUX1…`, R-RAY §1.1).
pub fn raydium_admin() -> Pubkey {
    Pubkey::new_from_array(raydium_v4::POOL_ADMIN)
}

/// AMM v4 `SetParams` (tag 6) with a `u64` value: param 0 = status, 1 = state, 3 = open time.
pub fn v4_set_params_ix(pool: &V4Pool, param: u8, value: u64) -> Instruction {
    let mut data = vec![V4_TAG_SET_PARAMS, param];
    data.extend_from_slice(&value.to_le_bytes());
    Instruction {
        program_id: RAYDIUM_V4_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(pool.pool, false),
            AccountMeta::new_readonly(raydium_admin(), true),
        ],
        data,
    }
}

/// AMM v4 `Fees` (8 × u64 at `AmmInfo` offset 128, R-RAY §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V4Fees {
    pub min_separate: (u64, u64),
    pub trade_fee: (u64, u64),
    pub pnl: (u64, u64),
    pub swap_fee: (u64, u64),
}

impl V4Fees {
    /// The fees the pool holds now.
    pub fn of(svm: &LiteSVM, pool: &V4Pool) -> Self {
        let d = svm.get_account(&pool.pool).expect("pool").data;
        let at = |o: usize| u64::from_le_bytes(d[o..o + 8].try_into().expect("u64"));
        Self {
            min_separate: (at(128), at(136)),
            trade_fee: (at(144), at(152)),
            pnl: (at(160), at(168)),
            swap_fee: (at(176), at(184)),
        }
    }

    fn pack(&self) -> Vec<u8> {
        [self.min_separate, self.trade_fee, self.pnl, self.swap_fee]
            .iter()
            .flat_map(|(n, d)| n.to_le_bytes().into_iter().chain(d.to_le_bytes()))
            .collect()
    }
}

/// AMM v4 `SetParams` param 2 (fees: every fraction must satisfy `num < den`).
pub fn v4_set_fees_ix(pool: &V4Pool, fees: &V4Fees) -> Instruction {
    let mut data = vec![V4_TAG_SET_PARAMS, 2];
    data.extend_from_slice(&fees.pack());
    Instruction {
        program_id: RAYDIUM_V4_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(pool.pool, false),
            AccountMeta::new_readonly(raydium_admin(), true),
        ],
        data,
    }
}

/// CPMM `update_pool_status(status)` (admin).
pub fn cpmm_update_pool_status_ix(pool: &CpmmPool, status: u8) -> Instruction {
    let mut data = CPMM_UPDATE_POOL_STATUS.to_vec();
    data.push(status);
    Instruction {
        program_id: RAYDIUM_CPMM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(raydium_admin(), true),
            AccountMeta::new(pool.pool, false),
        ],
        data,
    }
}

/// CPMM `update_amm_config(param, value)` (admin): 0 trade fee, 1 protocol, 2 fund, 7 creator
/// fee rate (ppm).
pub fn cpmm_update_amm_config_ix(config: &Pubkey, param: u8, value: u64) -> Instruction {
    let mut data = CPMM_UPDATE_AMM_CONFIG.to_vec();
    data.push(param);
    data.extend_from_slice(&value.to_le_bytes());
    Instruction {
        program_id: RAYDIUM_CPMM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(raydium_admin(), true),
            AccountMeta::new(*config, false),
        ],
        data,
    }
}

/// Runs a Raydium admin instruction as the pool admin ([`admin_send`] with `GThUX1…` forged).
pub fn raydium_admin_send(env: &mut Env, ix: Instruction) -> TxResult {
    admin_send(env, ix, &[raydium_admin()])
}

// ---- logs ------------------------------------------------------------------------------------

/// An AMM v4 `ray_log` swap record (bincode, R-RAY §3.1): `log_type` 3 = `SwapBaseIn`
/// (`amount` = amount in, `bound` = minimum out, `result` = amount out) or 4 = `SwapBaseOut`
/// (`amount` = max in, `bound` = amount out, `result` = the input deducted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RayLog {
    pub log_type: u8,
    pub amount: u64,
    pub bound: u64,
    /// 1 = PC2Coin (pc in), 2 = Coin2PC (coin in).
    pub direction: u64,
    pub user_source: u64,
    /// The coin reserve the swap priced on (`vault − need_take_pnl`).
    pub pool_coin: u64,
    /// The pc reserve the swap priced on.
    pub pool_pc: u64,
    pub result: u64,
}

/// Every `ray_log` swap record in `logs`.
pub fn ray_logs(logs: &[String]) -> Vec<RayLog> {
    logs.iter()
        .filter_map(|l| l.strip_prefix("Program log: ray_log: "))
        .filter_map(|b| base64_decode(b.trim()))
        .filter(|b| b.len() == 57 && matches!(b[0], 3 | 4))
        .map(|b| {
            let u = |i: usize| u64::from_le_bytes(b[1 + 8 * i..9 + 8 * i].try_into().expect("u64"));
            RayLog {
                log_type: b[0],
                amount: u(0),
                bound: u(1),
                direction: u(2),
                user_source: u(3),
                pool_coin: u(4),
                pool_pc: u(5),
                result: u(6),
            }
        })
        .collect()
}

/// CPMM's `SwapEvent` (`states/events.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpmmSwapEvent {
    pub pool_id: Pubkey,
    /// The input reserve the swap priced on (vault − counters).
    pub input_vault_before: u64,
    /// The output reserve the swap priced on.
    pub output_vault_before: u64,
    pub input_amount: u64,
    pub output_amount: u64,
    pub input_transfer_fee: u64,
    pub output_transfer_fee: u64,
    pub base_input: bool,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub trade_fee: u64,
    pub creator_fee: u64,
    pub creator_fee_on_input: bool,
}

/// Every CPMM `SwapEvent` in `logs` (`emit!`: "Program data: " + base64).
pub fn cpmm_swap_events(logs: &[String]) -> Vec<CpmmSwapEvent> {
    logs.iter()
        .filter_map(|l| l.strip_prefix("Program data: "))
        .filter_map(|b| base64_decode(b.trim()))
        .filter(|b| b.len() == 8 + 32 + 6 * 8 + 1 + 64 + 16 + 1 && b[..8] == CPMM_SWAP_EVENT)
        .map(|b| {
            let u = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().expect("u64"));
            let k = |o: usize| Pubkey::new_from_array(b[o..o + 32].try_into().expect("key"));
            CpmmSwapEvent {
                pool_id: k(8),
                input_vault_before: u(40),
                output_vault_before: u(48),
                input_amount: u(56),
                output_amount: u(64),
                input_transfer_fee: u(72),
                output_transfer_fee: u(80),
                base_input: b[88] != 0,
                input_mint: k(89),
                output_mint: k(121),
                trade_fee: u(153),
                creator_fee: u(161),
                creator_fee_on_input: b[169] != 0,
            }
        })
        .collect()
}

/// The logs of a successful transaction.
pub fn logs(meta: &TransactionMetadata) -> &[String] {
    &meta.logs
}

/// Standard base64 (with padding) decoding; `None` on a malformed string.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(u32::from(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        }))
    }
    let bytes = s.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 {
            return None;
        }
        let mut n = 0u32;
        for &c in &chunk[..4 - pad] {
            n = (n << 6) | val(c)?;
        }
        n <<= 6 * pad as u32;
        let b = n.to_be_bytes();
        out.extend_from_slice(&b[1..4 - pad]);
    }
    Some(out)
}

// ---- sha256 ----------------------------------------------------------------------------------

/// SHA-256 (FIPS 180-4), for checking the fixture bytes against the manifest without another
/// dependency.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    let mut w = [0u32; 64];
    for block in msg.as_chunks::<64>().0 {
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (a, b) in h.iter_mut().zip(v) {
            *a = a.wrapping_add(b);
        }
    }
    let mut out = [0u8; 32];
    for (o, x) in out.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        *o = x.to_be_bytes();
    }
    out
}

/// Lowercase hex of `sha256(data)`.
pub fn sha256_hex(data: &[u8]) -> String {
    hex(&sha256(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn base64() {
        assert_eq!(base64_decode("").unwrap(), b"");
        assert_eq!(base64_decode("Zg==").unwrap(), b"f");
        assert_eq!(base64_decode("Zm8=").unwrap(), b"fo");
        assert_eq!(base64_decode("Zm9v").unwrap(), b"foo");
        assert_eq!(base64_decode("Zm9vYmFy").unwrap(), b"foobar");
        assert_eq!(base64_decode("Zm9"), None);
        assert_eq!(base64_decode("Zm9!"), None);
    }
}
