//! Fresh deterministic runtime and canonical upgradeable-loader account helpers.
// Preserve LiteSVM failed-transaction logs and metadata intact in this test harness.
#![allow(clippy::result_large_err)]
use anchor_lang::{prelude::Pubkey, solana_program::clock::Clock, AccountDeserialize};
use litesvm::{
    types::{FailedTransactionMetadata, TransactionMetadata},
    LiteSVM,
};
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::path::PathBuf;

pub type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;
pub const FUNDING: u64 = 1_000_000_000_000;
pub const START_TS: i64 = 1_790_000_000;
pub const LOADER: Pubkey =
    anchor_lang::prelude::pubkey!("BPFLoaderUpgradeab1e11111111111111111111111");

pub fn deterministic_key(tag: u8) -> Keypair {
    Keypair::new_from_array([tag; 32])
}
pub fn program_so_path() -> PathBuf {
    std::env::var_os("SHORTWIN_SIMPLE_SO").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/deploy/shortwin_simple.so"),
        PathBuf::from,
    )
}
pub fn program_data() -> Pubkey {
    Pubkey::find_program_address(&[shortwin_simple::ID.as_ref()], &LOADER).0
}
pub fn config() -> Pubkey {
    Pubkey::find_program_address(&[b"config"], &shortwin_simple::ID).0
}
pub fn policy(version: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[b"policy", config().as_ref(), &version.to_le_bytes()],
        &shortwin_simple::ID,
    )
    .0
}
pub fn market(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"market", mint.as_ref()], &shortwin_simple::ID).0
}
pub fn pool(market: &Pubkey, generation: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[b"pool", market.as_ref(), &generation.to_le_bytes()],
        &shortwin_simple::ID,
    )
    .0
}
pub fn token_vault(pool: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"token_vault", pool.as_ref()], &shortwin_simple::ID).0
}
pub fn lender(pool: &Pubkey, owner: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"lender", pool.as_ref(), owner.as_ref()],
        &shortwin_simple::ID,
    )
    .0
}
pub fn event_authority() -> Pubkey {
    Pubkey::find_program_address(&[b"__event_authority"], &shortwin_simple::ID).0
}
pub fn assert_ok(result: TxResult) -> TransactionMetadata {
    result.unwrap_or_else(|e| panic!("transaction {:?}\n{}", e.err, e.meta.logs.join("\n")))
}

pub struct Env {
    pub svm: LiteSVM,
    pub payer: Keypair,
    pub upgrade_authority: Keypair,
    pub governance: Keypair,
    pub emergency: Keypair,
    pub reporter: Keypair,
    pub fee_recipient: Keypair,
}
impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}
impl Env {
    pub fn new() -> Self {
        let mut svm = LiteSVM::new();
        let mut clock = svm.get_sysvar::<Clock>();
        clock.slot = crate::jupiter::EXECUTION_SLOT;
        clock.unix_timestamp = START_TS;
        svm.set_sysvar(&clock);
        let file = program_so_path();
        let elf = std::fs::read(&file).unwrap_or_else(|e| {
            panic!(
                "{}: {e}; build the NEW program before runtime tests",
                file.display()
            )
        });
        svm.add_program(shortwin_simple::ID, &elf)
            .expect("load independent new SBF");
        let mut env = Self {
            svm,
            payer: deterministic_key(1),
            upgrade_authority: deterministic_key(2),
            governance: deterministic_key(3),
            emergency: deterministic_key(4),
            reporter: deterministic_key(5),
            fee_recipient: deterministic_key(6),
        };
        let authority = env.upgrade_authority.pubkey();
        env.set_upgrade_authority(authority);
        for key in [
            env.payer.pubkey(),
            authority,
            env.governance.pubkey(),
            env.emergency.pubkey(),
            env.reporter.pubkey(),
            env.fee_recipient.pubkey(),
        ] {
            env.fund(key, FUNDING);
        }
        env
    }
    pub fn set_upgrade_authority(&mut self, authority: Pubkey) {
        // Canonical bincode UpgradeableLoaderState::ProgramData: tag3, slotu64,
        // Option<Pubkey> tag1 and32bytes. Selective reuse of loader fixture encoding
        // from the repository's BUSL-1.1 low-level harness; no old program imported.
        let key = program_data();
        let mut account = self
            .svm
            .get_account(&key)
            .expect("canonical ProgramData installed by LiteSVM");
        assert_eq!(account.owner, LOADER);
        assert!(account.data.len() >= 45);
        assert_eq!(&account.data[..4], &3u32.to_le_bytes());
        account.data[12] = 1;
        account.data[13..45].copy_from_slice(authority.as_ref());
        self.svm
            .set_account(key, account)
            .expect("set test upgrade authority");
    }
    pub fn fund(&mut self, key: Pubkey, amount: u64) {
        let r = self.svm.airdrop(&key, amount);
        self.svm.expire_blockhash();
        r.expect("fund test key");
    }
    pub fn send(&mut self, instructions: &[Instruction], additional: &[&Keypair]) -> TxResult {
        let mut signers = vec![&self.payer];
        for signer in additional {
            if !signers.iter().any(|s| s.pubkey() == signer.pubkey()) {
                signers.push(*signer);
            }
        }
        let tx = Transaction::new_signed_with_payer(
            instructions,
            Some(&self.payer.pubkey()),
            &signers,
            self.svm.latest_blockhash(),
        );
        let result = self.svm.send_transaction(tx);
        self.svm.expire_blockhash();
        result
    }
    pub fn read<T: AccountDeserialize>(&self, key: &Pubkey) -> T {
        let a = self
            .svm
            .get_account(key)
            .unwrap_or_else(|| panic!("missing account {key}"));
        T::try_deserialize(&mut a.data.as_slice()).unwrap_or_else(|e| panic!("decode {key}: {e}"))
    }
    pub fn lamports(&self, key: &Pubkey) -> u64 {
        self.svm.get_account(key).map_or(0, |a| a.lamports)
    }
    pub fn token_amount(&self, key: &Pubkey) -> u64 {
        let a = self.svm.get_account(key).expect("token account");
        u64::from_le_bytes(a.data[64..72].try_into().unwrap())
    }
    pub fn set_clock(&mut self, timestamp: i64, slot: u64) {
        let mut c = self.svm.get_sysvar::<Clock>();
        c.unix_timestamp = timestamp;
        c.slot = slot;
        self.svm.set_sysvar(&c);
    }
}

/// Functional-test policy. These values are fixtures, not release recommendations.
pub fn test_policy() -> shortwin_simple::state::FinancialPolicy {
    shortwin_simple::state::FinancialPolicy {
        term_seconds: 3600,
        default_grace_seconds: 86400,
        term_fee_bps: 100,
        protocol_share_bps: 1000,
        cr_open_bps: 20000,
        cr_liq_bps: 15000,
        min_notional_lamports: 1000,
        position_notional_cap_lamports: 1_000_000_000,
        market_notional_cap_lamports: 25_000_000_000,
        utilization_cap_bps: 8000,
        wallet_notional_cap_bps: 10000,
        keeper_fixed_lamports: 1000,
        keeper_bps: 100,
        keeper_cap_lamports: 1_000_000,
        buyback_premium_bps: 500,
        reference_max_age_seconds: 30,
        reference_max_age_slots: 100,
        history_epoch_seconds: 60,
        minimum_history_epochs: 2,
        max_spread_bps: 1000,
        cr_confirmation_delay_seconds: 5,
        cr_confirmation_delay_slots: 5,
        price_policy_hash: [42; 32],
    }
}

impl Env {
    pub fn initialize_args(&self) -> shortwin_simple::initialization::InitializeArgs {
        let review = crate::jupiter::HistoricalRoute::reviewed();
        shortwin_simple::initialization::InitializeArgs {
            governance: self.governance.pubkey(),
            emergency: self.emergency.pubkey(),
            reporter: self.reporter.pubkey(),
            fee_recipient: self.fee_recipient.pubkey(),
            genesis_hash: [7; 32],
            policy_version: 1,
            terms: test_policy(),
            opens_paused: false,
            market_creation_fee: 10_000,
            jupiter_deploy_slot: review.deploy_slot,
            jupiter_program_data_len: review.programdata_len,
            jupiter_elf_hash: review.elf_sha256,
            max_setup_rent: 100_000_000,
        }
    }
    pub fn initialize(
        &mut self,
        args: shortwin_simple::initialization::InitializeArgs,
    ) -> TxResult {
        use anchor_lang::{InstructionData, ToAccountMetas};
        let authority = self.upgrade_authority.insecure_clone();
        let accounts = shortwin_simple::accounts::Initialize {
            authority: authority.pubkey(),
            config: config(),
            policy: policy(args.policy_version),
            program_account: shortwin_simple::ID,
            program_data: program_data(),
            jupiter: shortwin_simple::route::JUPITER_ID,
            jupiter_program_data: shortwin_simple::route::JUPITER_PROGRAM_DATA,
            system_program: anchor_lang::system_program::ID,
            event_authority: event_authority(),
            program: shortwin_simple::ID,
        };
        let ix = Instruction {
            program_id: shortwin_simple::ID,
            accounts: accounts.to_account_metas(None),
            data: shortwin_simple::instruction::Initialize { args }.data(),
        };
        self.send(&[ix], &[&authority])
    }
    pub fn initialized() -> Self {
        let mut env = Self::new();
        crate::jupiter::install(&mut env.svm);
        let args = env.initialize_args();
        assert_ok(env.initialize(args));
        env
    }
    pub fn register_market(
        &mut self,
        creator: &Keypair,
        mint: Pubkey,
        program: Pubkey,
        max_fee: u64,
        max_rent: u64,
    ) -> TxResult {
        use anchor_lang::{InstructionData, ToAccountMetas};
        let market = market(&mint);
        let pool = pool(&market, 0);
        let accounts = shortwin_simple::accounts::RegisterMarket {
            creator: creator.pubkey(),
            config: config(),
            policy: policy(1),
            market,
            pool,
            mint,
            token_vault: token_vault(&pool),
            fee_recipient: self.fee_recipient.pubkey(),
            token_program: program,
            system_program: anchor_lang::system_program::ID,
            event_authority: event_authority(),
            program: shortwin_simple::ID,
        };
        let ix = Instruction {
            program_id: shortwin_simple::ID,
            accounts: accounts.to_account_metas(None),
            data: shortwin_simple::instruction::RegisterMarket {
                max_creation_fee: max_fee,
                max_setup_rent: max_rent,
            }
            .data(),
        };
        self.send(&[ix], &[creator])
    }
}

/// Test-only synthetic active ALT. A successful bounded v0 runtime transaction
/// is not evidence that this lookup table exists on mainnet or that it landed.
impl Env {
    pub fn send_v0(
        &mut self,
        instructions: &[Instruction],
        additional: &[&Keypair],
    ) -> (TxResult, usize) {
        use solana_message::{AddressLookupTableAccount, VersionedMessage};
        use solana_transaction::versioned::VersionedTransaction;
        let limit = Instruction {
            program_id: anchor_lang::prelude::pubkey!(
                "ComputeBudget111111111111111111111111111111"
            ),
            accounts: vec![],
            data: [vec![2], 1_400_000u32.to_le_bytes().to_vec()].concat(),
        };
        let mut instructions = instructions.to_vec();
        instructions.insert(0, limit);
        let mut addresses = vec![];
        for ix in &instructions {
            for a in &ix.accounts {
                if !a.is_signer && !addresses.contains(&a.pubkey) {
                    addresses.push(a.pubkey);
                }
            }
        }
        let key = deterministic_key(250).pubkey();
        let mut data = vec![0u8; 56];
        data[..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..12].copy_from_slice(&u64::MAX.to_le_bytes());
        for a in &addresses {
            data.extend_from_slice(a.as_ref());
        }
        self.svm
            .set_account(
                key,
                solana_account::Account {
                    lamports: self.svm.minimum_balance_for_rent_exemption(data.len()),
                    data,
                    owner: anchor_lang::prelude::pubkey!(
                        "AddressLookupTab1e1111111111111111111111111"
                    ),
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .expect("test-only ALT");
        let table = AddressLookupTableAccount { key, addresses };
        let message = VersionedMessage::V0(
            solana_message::v0::Message::try_compile(
                &self.payer.pubkey(),
                &instructions,
                &[table],
                self.svm.latest_blockhash(),
            )
            .expect("compile v0"),
        );
        let sigs = usize::from(message.header().num_required_signatures);
        assert!(sigs < 128);
        let size = 1 + 64 * sigs + message.serialize().len();
        assert!(size <= 1232, "test v0 packet too large: {size}");
        let mut signers = vec![&self.payer];
        for signer in additional {
            if !signers.iter().any(|s| s.pubkey() == signer.pubkey()) {
                signers.push(*signer);
            }
        }
        let tx = VersionedTransaction::try_new(message, &signers).expect("sign v0");
        let result = self.svm.send_transaction(tx);
        self.svm.expire_blockhash();
        (result, size)
    }
}
