//! [`Env`]: one LiteSVM instance with short.win loaded, the admin keys, a clock under test
//! control, and the invariant hooks wired into every transaction.

use std::path::PathBuf;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use litesvm::LiteSVM;
use litesvm::types::{FailedTransactionMetadata, TransactionMetadata};
use shortwin::constants::{
    PYTH_SOL_USD_ACCOUNT, PYTH_SOL_USD_FEED_ID, VENUE_BIT_PUMP_CURVE, VENUE_BIT_PUMP_SWAP,
    VENUE_BIT_RAYDIUM_CPMM, VENUE_BIT_RAYDIUM_V4, VENUE_GUARD_SLOTS_2026_09_27, VENUE_PROGRAM_DATA,
};
use shortwin::state::{
    EmaParamsArgs, GlobalConfig, InitConfigArgs, LenderPosition, Market, MarketVault, OracleCache,
    Params, TierParams, VenueGuard,
};
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;

use crate::fixtures::{self, MarketKeys};
use crate::tokens::TokenProgram;
use crate::{invariants, pda, read};

/// Result of a transaction.
pub type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

/// Unix time the tests start at (2026-09-21T15:06:40Z).
pub const START_TS: i64 = 1_790_000_000;
/// Slot the tests start at.
pub const START_SLOT: u64 = 450_000_000;
/// Lamports every harness key starts with.
pub const FUNDING: u64 = 1_000_000_000_000;

/// Path of the program under test: `$SHORTWIN_SO`, else `target/deploy/shortwin.so` (the
/// output of `anchor build --tools-version v1.57 --arch v3`).
pub fn program_so_path() -> PathBuf {
    std::env::var_os("SHORTWIN_SO").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/deploy/shortwin.so"),
        PathBuf::from,
    )
}

/// Path of the TEST-ONLY `test-venue` build (`VenueKind::Mock`; A49):
/// `$SHORTWIN_TEST_VENUE_SO`, else `target/test-venue/deploy/shortwin.so` (the output of
/// `scripts/build-test-venue.sh`). The release build never carries the mock adapter.
pub fn test_venue_so_path() -> PathBuf {
    std::env::var_os("SHORTWIN_TEST_VENUE_SO").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/test-venue/deploy/shortwin.so")
        },
        PathBuf::from,
    )
}

/// A LiteSVM environment with short.win deployed.
pub struct Env {
    /// The VM. Tests may drive it directly; transactions that should run the invariant hooks
    /// go through [`Env::send`].
    pub svm: LiteSVM,
    /// Fee payer of every harness transaction (never an admin key).
    pub payer: Keypair,
    /// short.win's upgrade authority (recorded in its ProgramData).
    pub upgrade_authority: Keypair,
    /// `GlobalConfig.governance`.
    pub governance: Keypair,
    /// `GlobalConfig.pause_authority` (the Emergency vault).
    pub pause_authority: Keypair,
    /// `GlobalConfig.fee_recipient` (a funded system account).
    pub fee_recipient: Keypair,
    /// Run the invariant hooks in every [`Env::send`] (default `true`). Only a test that
    /// corrupts the state on purpose turns them off.
    pub check_invariants: bool,
    /// How many post-instruction states the hooks checked so far: one per transaction, plus
    /// one per proper prefix of a multi-instruction transaction (the harness's own tests read
    /// it).
    pub hook_checkpoints: u64,
    tracked: Vec<Pubkey>,
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

impl Env {
    /// Loads `target/deploy/shortwin.so` (upgradeable loader), makes
    /// [`Env::upgrade_authority`] its upgrade authority, funds the harness keys and sets the
    /// clock to [`START_TS`] / [`START_SLOT`]. The config is **not** initialized.
    pub fn new() -> Self {
        Self::with_program(
            &program_so_path(),
            "`anchor build --tools-version v1.57 --arch v3` (or set SHORTWIN_SO)",
        )
    }

    /// [`Env::new`] with short.win's **test-venue** build ([`test_venue_so_path`]): the release
    /// program plus `VenueKind::Mock`. The config is not initialized; see
    /// [`Env::test_venue`] for the ready-to-trade environment.
    pub fn new_test_venue_program() -> Self {
        Self::with_program(
            &test_venue_so_path(),
            "`scripts/build-test-venue.sh` (or set SHORTWIN_TEST_VENUE_SO)",
        )
    }

    fn with_program(path: &std::path::Path, how: &str) -> Self {
        let so = std::fs::read(path).unwrap_or_else(|e| {
            panic!(
                "cannot read {}: {e}. Build the program first: {how}",
                path.display()
            )
        });
        let mut svm = LiteSVM::new();
        svm.add_program(shortwin::ID, &so)
            .unwrap_or_else(|e| panic!("add_program: {e:?}"));
        let env = Self {
            svm,
            payer: Keypair::new(),
            upgrade_authority: Keypair::new(),
            governance: Keypair::new(),
            pause_authority: Keypair::new(),
            fee_recipient: Keypair::new(),
            check_invariants: true,
            hook_checkpoints: 0,
            tracked: Vec::new(),
        };
        let mut env = env;
        env.set_clock(START_TS, START_SLOT);
        let authority = env.upgrade_authority.pubkey();
        env.set_own_upgrade_authority(Some(&authority));
        for key in [
            env.payer.pubkey(),
            env.upgrade_authority.pubkey(),
            env.governance.pubkey(),
            env.pause_authority.pubkey(),
            env.fee_recipient.pubkey(),
        ] {
            env.fund(&key, FUNDING);
        }
        env
    }

    /// [`Env::new`] plus `initialize_config` with [`Env::default_init_args`].
    pub fn initialized() -> Self {
        let mut env = Self::new();
        let args = env.default_init_args();
        crate::errors::ok(env.initialize_config(args));
        env
    }

    /// Rewrites short.win's ProgramData header so `authority` is its upgrade authority
    /// (LiteSVM deploys with none).
    pub fn set_own_upgrade_authority(&mut self, authority: Option<&Pubkey>) {
        let key = pda::program_data();
        let mut acc = self
            .svm
            .get_account(&key)
            .unwrap_or_else(|| panic!("no ProgramData at {key}"));
        let slot = shortwin_venue_state::bytes::u64_at(&acc.data, 4);
        let auth = authority.map(Pubkey::to_bytes);
        let header = shortwin_venue_state::program_data::encode_header(slot, auth.as_ref());
        acc.data[..header.len()].copy_from_slice(&header);
        self.svm
            .set_account(key, acc)
            .unwrap_or_else(|e| panic!("set ProgramData: {e:?}"));
    }

    /// ECON §15 defaults, the harness authorities, the sponsored Pyth feed, every MVP venue
    /// enabled, the 48 h timelock and the v0.4.1 venue deploy slots.
    pub fn default_init_args(&self) -> InitConfigArgs {
        InitConfigArgs {
            governance: self.governance.pubkey(),
            pause_authority: self.pause_authority.pubkey(),
            coverage_authority: Pubkey::default(),
            fee_recipient: self.fee_recipient.pubkey(),
            pyth_sol_usd: PYTH_SOL_USD_ACCOUNT,
            pyth_feed_id: PYTH_SOL_USD_FEED_ID,
            timelock_s: shortwin::constants::DEFAULT_TIMELOCK_S,
            venue_enabled: VENUE_BIT_PUMP_CURVE
                | VENUE_BIT_PUMP_SWAP
                | VENUE_BIT_RAYDIUM_V4
                | VENUE_BIT_RAYDIUM_CPMM,
            params: Params::defaults(),
            tiers: TierParams::defaults(),
            ema: EmaParamsArgs::defaults(),
            util_kink_bps: shortwin_math::params::Params::DEFAULT.util_kink_bps,
            guard_slot: VENUE_GUARD_SLOTS_2026_09_27,
        }
    }

    /// `apply_config()` with the optional accounts the pending change needs
    /// ([`crate::ix::apply_config_for`]); the plain instruction if nothing is pending.
    pub fn apply_config_ix(&self) -> Instruction {
        self.config().pending.change().map_or_else(
            |_| crate::ix::apply_config(),
            |c| crate::ix::apply_config_for(&c),
        )
    }

    /// `initialize_config(args)` signed by the upgrade authority.
    pub fn initialize_config(&mut self, args: InitConfigArgs) -> TxResult {
        let authority = self.upgrade_authority.insecure_clone();
        let ix = crate::ix::initialize_config(&authority.pubkey(), args);
        self.send(&[ix], &[&authority])
    }

    /// Sends `ixs` with [`Env::payer`] as fee payer and `signers`, with the invariant hooks
    /// (ARCH §8.3, "after every instruction"; M1: I1, I2, I4, I8, I17):
    ///
    /// 1. every existing `Market` any instruction of the transaction names is tracked from now
    ///    on (so a test cannot forget one), and after the transaction every market it created
    ///    (`create_market`);
    /// 2. a transaction of several instructions is replayed prefix by prefix on a clone of the
    ///    VM: after prefix `k` the hooks check instruction `k` — [`invariants::check_ix`]
    ///    against the state **after prefix `k − 1`** (the transaction's start for `k = 1`),
    ///    with instruction `k`'s own events and signers (M1-FID-3, M1-FID-5), then
    ///    [`invariants::check_all`] on every tracked market — so the state after every
    ///    instruction is checked, not only the transaction's end (C0-9, V2-2 bundle cases);
    /// 3. after the transaction: on success `check_ix` for its last instruction (against the
    ///    state after all the others) and `check_all` for every tracked market; on failure
    ///    (nothing is committed) `check_all` again.
    ///
    /// "Admin-signed" (I4) is decided per instruction, from that instruction's signer metas
    /// ([`invariants::is_admin_signed`]) against the harness's admin keys and the config's
    /// current authorities.
    ///
    /// Panics on a violation, with the transaction's logs. Expires the blockhash afterwards, so
    /// an identical transaction can be sent again. [`Env::check_invariants`] `= false` skips
    /// every hook.
    pub fn send(&mut self, ixs: &[Instruction], signers: &[&Keypair]) -> TxResult {
        if !self.check_invariants {
            let tx = self.transaction(ixs, signers);
            let res = self.svm.send_transaction(tx);
            self.svm.expire_blockhash();
            return res;
        }
        self.track_markets_named_by(ixs);
        let admins = self.admin_keys();
        let pre = invariants::snapshot(&self.svm, &self.tracked);
        // The state before the last instruction (after every other one), unless a prefix
        // failed (the whole transaction then fails at the same instruction).
        let before_last = if ixs.len() > 1 {
            self.check_every_prefix(ixs, signers, &admins, pre)
        } else {
            Some(pre)
        };

        let tx = self.transaction(ixs, signers);
        let res = self.svm.send_transaction(tx);
        self.svm.expire_blockhash();

        self.track_markets_named_by(ixs);
        self.hook_checkpoints += 1;
        match &res {
            Ok(meta) => {
                let pre = before_last.unwrap_or_else(|| {
                    panic!(
                        "a prefix of the transaction failed on the clone but the transaction \
                         succeeded\nlogs:\n{}",
                        meta.pretty_logs()
                    )
                });
                let last = ixs.len().saturating_sub(1);
                if let Some(ix) = ixs.get(last) {
                    let ev = invariants::IxEvents::of_instruction(meta, last, ix);
                    let admin = invariants::is_admin_signed(ix, &admins);
                    if let Err(v) = invariants::check_ix(&self.svm, &pre, admin, &ev) {
                        panic!(
                            "{v}\n(instruction {} of {})\nlogs:\n{}",
                            last + 1,
                            ixs.len(),
                            meta.pretty_logs()
                        );
                    }
                }
                self.check_tracked_or_panic(Some(meta));
            }
            Err(failed) => self.check_tracked_or_panic(Some(&failed.meta)),
        }
        res
    }

    /// `ixs` signed by [`Env::payer`] and those of `signers` the message requires (a prefix of
    /// a transaction may need fewer signers than the whole).
    pub fn transaction(&self, ixs: &[Instruction], signers: &[&Keypair]) -> Transaction {
        let needed = |k: &Pubkey| {
            ixs.iter()
                .flat_map(|ix| ix.accounts.iter())
                .any(|m| m.is_signer && m.pubkey == *k)
        };
        let mut all: Vec<&Keypair> = vec![&self.payer];
        for s in signers {
            let k = s.pubkey();
            if needed(&k) && !all.iter().any(|a| a.pubkey() == k) {
                all.push(s);
            }
        }
        Transaction::new_signed_with_payer(
            ixs,
            Some(&self.payer.pubkey()),
            &all,
            self.svm.latest_blockhash(),
        )
    }

    /// Tracks every existing short.win `Market` an instruction of `ixs` names.
    fn track_markets_named_by(&mut self, ixs: &[Instruction]) {
        let named: Vec<Pubkey> = ixs
            .iter()
            .flat_map(|ix| ix.accounts.iter().map(|m| m.pubkey))
            .filter(|k| read::zc::<Market>(&self.svm, k).is_some())
            .collect();
        for market in named {
            self.track_market(market);
        }
    }

    /// Runs `ixs[..k]` for every `k < ixs.len()` on a clone of the VM and, after each prefix
    /// that succeeds, checks instruction `k` against the state after prefix `k − 1` (`pre` for
    /// `k = 1`) and every tracked market's state invariants. Returns the state after prefix
    /// `ixs.len() − 1` (what the last instruction starts from), or `None` if a prefix failed
    /// (the whole transaction then fails at the same instruction and commits nothing).
    fn check_every_prefix(
        &mut self,
        ixs: &[Instruction],
        signers: &[&Keypair],
        admins: &[Pubkey],
        mut pre: invariants::TxSnapshot,
    ) -> Option<invariants::TxSnapshot> {
        for k in 1..ixs.len() {
            let prefix = &ixs[..k];
            let mut svm = self.svm.clone();
            let tx = self.transaction(prefix, signers);
            let Ok(meta) = svm.send_transaction(tx) else {
                return None;
            };
            let fail = |v: invariants::Violation| -> ! {
                panic!(
                    "{v}\n(after instruction {} of {} in one transaction)\nlogs:\n{}",
                    k,
                    ixs.len(),
                    meta.pretty_logs()
                )
            };
            let ev = invariants::IxEvents::of_instruction(&meta, k - 1, &ixs[k - 1]);
            let admin = invariants::is_admin_signed(&ixs[k - 1], admins);
            if let Err(v) = invariants::check_ix(&svm, &pre, admin, &ev) {
                fail(v);
            }
            let created = prefix
                .iter()
                .flat_map(|ix| ix.accounts.iter().map(|m| m.pubkey))
                .filter(|key| read::zc::<Market>(&svm, key).is_some());
            let mut markets: Vec<Pubkey> = self.tracked.clone();
            for key in created {
                if !markets.contains(&key) {
                    markets.push(key);
                }
            }
            for market in &markets {
                if let Err(v) = invariants::check_all(&svm, market) {
                    fail(v);
                }
            }
            self.hook_checkpoints += 1;
            pre = invariants::snapshot(&svm, &markets);
        }
        Some(pre)
    }

    /// The admin keys (I4): the harness's upgrade authority, governance and Emergency keys,
    /// and the config's current governance, Emergency and (if set) coverage authority.
    pub fn admin_keys(&self) -> Vec<Pubkey> {
        let mut keys = vec![
            self.upgrade_authority.pubkey(),
            self.governance.pubkey(),
            self.pause_authority.pubkey(),
        ];
        if let Some(c) = read::zc::<GlobalConfig>(&self.svm, &pda::config()) {
            keys.extend([c.governance, c.pause_authority]);
            if c.coverage_authority != Pubkey::default() {
                keys.push(c.coverage_authority);
            }
        }
        keys
    }

    fn check_tracked_or_panic(&self, meta: Option<&TransactionMetadata>) {
        for market in &self.tracked {
            if let Err(v) = invariants::check_all(&self.svm, market) {
                let logs = meta
                    .map(TransactionMetadata::pretty_logs)
                    .unwrap_or_default();
                panic!("{v}\nlogs:\n{logs}");
            }
        }
    }

    /// Adds `market` to the markets [`invariants::check_all`] runs on after every transaction
    /// ([`Env::send`] also tracks every market a transaction names or creates).
    pub fn track_market(&mut self, market: Pubkey) {
        if !self.tracked.contains(&market) {
            self.tracked.push(market);
        }
    }

    /// The tracked markets.
    pub fn tracked_markets(&self) -> &[Pubkey] {
        &self.tracked
    }

    /// Runs [`invariants::check_all`] on every tracked market now (panics on a violation).
    pub fn assert_invariants(&self) {
        self.check_tracked_or_panic(None);
    }

    // ---- clock ------------------------------------------------------------------------

    /// The current clock.
    pub fn clock(&self) -> Clock {
        self.svm.get_sysvar::<Clock>()
    }

    /// `Clock::unix_timestamp`.
    pub fn now(&self) -> i64 {
        self.clock().unix_timestamp
    }

    /// Sets the clock's unix time and slot.
    pub fn set_clock(&mut self, unix_timestamp: i64, slot: u64) {
        let mut c = self.clock();
        c.unix_timestamp = unix_timestamp;
        c.slot = slot;
        self.svm.set_sysvar(&c);
    }

    /// Advances time by `seconds` (and the slot by 2.5 per second, 400 ms slots).
    pub fn warp(&mut self, seconds: i64) {
        let c = self.clock();
        let slots = u64::try_from(seconds.max(0)).unwrap_or(0) * 5 / 2;
        self.set_clock(c.unix_timestamp + seconds, c.slot + slots);
    }

    // ---- accounts -----------------------------------------------------------------------

    /// Airdrops `lamports` to `key`.
    pub fn fund(&mut self, key: &Pubkey, lamports: u64) {
        self.svm
            .airdrop(key, lamports)
            .unwrap_or_else(|e| panic!("airdrop: {:?}", e.err));
    }

    /// A new funded keypair.
    pub fn funded_keypair(&mut self) -> Keypair {
        let kp = Keypair::new();
        self.fund(&kp.pubkey(), FUNDING);
        kp
    }

    /// `GlobalConfig` (panics if not initialized).
    pub fn config(&self) -> GlobalConfig {
        read::zc(&self.svm, &pda::config()).unwrap_or_else(|| panic!("config not initialized"))
    }

    /// `VenueGuard` (panics if not initialized).
    pub fn venue_guard(&self) -> VenueGuard {
        read::zc(&self.svm, &pda::venue_guard())
            .unwrap_or_else(|| panic!("venue guard not initialized"))
    }

    /// `OracleCache` (panics if not initialized).
    pub fn oracle_cache(&self) -> OracleCache {
        read::borsh(&self.svm, &pda::oracle_cache())
            .unwrap_or_else(|| panic!("oracle cache not initialized"))
    }

    /// A market (panics if missing).
    pub fn market(&self, market: &Pubkey) -> Market {
        read::zc(&self.svm, market).unwrap_or_else(|| panic!("no market {market}"))
    }

    /// A market's vault (panics if missing).
    pub fn market_vault(&self, market: &Pubkey) -> MarketVault {
        read::zc(&self.svm, &pda::market_vault(market).0)
            .unwrap_or_else(|| panic!("no market vault for {market}"))
    }

    /// A lender position, if it exists.
    pub fn lender(&self, market: &Pubkey, owner: &Pubkey) -> Option<LenderPosition> {
        read::borsh(&self.svm, &pda::lender(market, owner).0)
    }

    /// Writes a mock ProgramData for venue program `venue` (a `VenueGuard` index) with deploy
    /// `slot`.
    pub fn set_venue_program_data(&mut self, venue: u8, slot: u64) {
        let key = VENUE_PROGRAM_DATA[usize::from(venue)];
        fixtures::set_program_data(&mut self.svm, &key, slot, None, 64);
    }

    /// Writes mock ProgramData accounts for all six venue programs at the slots
    /// `initialize_config` recorded (no venue is "upgraded").
    pub fn set_all_venue_program_data(&mut self) {
        for (venue, slot) in VENUE_GUARD_SLOTS_2026_09_27.iter().enumerate() {
            self.set_venue_program_data(venue as u8, *slot);
        }
    }

    /// Installs a list-only market for a new mint of `program` (see
    /// [`fixtures::install_market`]) and tracks it. Returns the keys and the mint authority.
    pub fn install_market(&mut self, program: TokenProgram, decimals: u8) -> (MarketKeys, Keypair) {
        let authority = Keypair::new();
        let payer = self.payer.insecure_clone();
        let mint = crate::tokens::create_mint(
            &mut self.svm,
            &payer,
            &authority.pubkey(),
            decimals,
            program,
        );
        let config = self.config();
        let now = self.now();
        let keys = fixtures::install_market(
            &mut self.svm,
            &mint,
            decimals,
            program,
            &payer.pubkey(),
            now,
            &config,
        );
        self.track_market(keys.market);
        (keys, authority)
    }
}
