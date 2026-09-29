//! TEST-ONLY: the keeper's in-process chain backend on LiteSVM, and the scenario driver.
//!
//! [`LiteSvmChain`] implements [`Chain`] over the program-test harness's [`Env`]
//! (`tests/litesvm`): short.win's **test-venue** build and the mock venue (A49). Every
//! transaction the keeper sends runs the harness's **invariant hooks after every instruction**
//! (ARCH §8.3), exactly as `Env::send` does for the harness's own legacy transactions — but on
//! the keeper's versioned (v1 / v0) transaction as built, so formats, sizes and compute budgets
//! are the real ones:
//!
//! 1. every existing `Market` the transaction names is tracked;
//! 2. a transaction of several instructions is replayed prefix by prefix on a clone of the VM
//!    (signature checks off on the clone), and after prefix `k` instruction `k` is checked
//!    (`invariants::check_ix`) against the state after prefix `k − 1`, and every tracked market
//!    (`invariants::check_all`);
//! 3. the transaction itself runs; its last instruction is checked against the state after
//!    all the others; a failed transaction is checked for committing nothing.
//!
//! A violation panics with the transaction's logs. Knobs for the scenarios: refuse v1
//! transactions ([`LiteSvmChain::set_v1`], to exercise the v0 fallback) and the recent
//! priority fees the backend reports ([`LiteSvmChain::set_priority_fees`]); every backend call
//! is logged ([`LiteSvmChain::rpc_log`], M4-SEC-7: the keeper's RPC work per tick).
//!
//! **Bundles** (the straddle, ARCH §5.4; U22): [`LiteSvmChain`] executes a bundle as Jito does —
//! its transactions in order, in one slot (the blockhash is the one the bundle was built with,
//! checked once), all-or-nothing (the VM is restored when one fails) — each through
//! [`send_with_hooks`]; `simulate_bundle` runs them in order on a clone. A test can refuse bundles
//! ([`LiteSvmChain::set_bundles`]: a chain without a block engine). [`load_complete_curve`]
//! deploys the straddle helper (`target/deploy/complete_curve.so`, or `$COMPLETE_CURVE_SO`).
//!
//! [`Harness`] couples the environment, a funded keeper and the clock: one keeper tick per
//! slot or per second.

#![allow(clippy::result_large_err)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use litesvm::types::TransactionMetadata;
use shortwin::state::Market;
use shortwin_keeper::{Keeper, KeeperConfig, TickReport};
use shortwin_litesvm_tests::{Env, invariants, read};
use shortwin_services_common::tx::{self, TxFormat};
use shortwin_services_common::{
    AccountData, BundleOutcome, Chain, ChainClock, InnerIx, Memcmp, TxError, TxOutcome,
};
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_message::{AddressLookupTableAccount, Message, VersionedMessage};
use solana_signature::Signature;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

/// Lamports the keeper's fee payer starts with.
pub const KEEPER_FUNDING: u64 = 100_000_000_000;

/// The in-process chain (module docs).
pub struct LiteSvmChain {
    env: Rc<RefCell<Env>>,
    v1: Cell<bool>,
    fees: RefCell<Vec<u64>>,
    sent: RefCell<Vec<SentTx>>,
    rpc: RefCell<Vec<Rpc>>,
    bundles: Cell<bool>,
    bundles_sent: RefCell<Vec<SentBundle>>,
    before_bundle: RefCell<Option<BeforeBundle>>,
}

/// What runs when the next bundle reaches the chain, before it executes (a trade landing between
/// the keeper's read and its bundle).
type BeforeBundle = Box<dyn FnOnce(&mut Env)>;

/// A bundle the keeper sent (for assertions).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentBundle {
    /// Each transaction as sent.
    pub txs: Vec<SentTx>,
    /// Top-level instructions of each transaction (ComputeBudget included).
    pub top: Vec<usize>,
    /// Instruction-trace entries of each transaction that ran (top-level instructions, ComputeBudget
    /// included, plus every inner instruction; ARCH §6.2).
    pub trace: Vec<usize>,
    /// Compute units of each transaction that ran.
    pub units: Vec<u64>,
    pub landed: bool,
}

/// One backend call, as a JSON-RPC backend would make it (M4-SEC-7's measurements).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rpc {
    /// `getMultipleAccounts` of these keys (a JSON-RPC backend splits them into calls of 100).
    Accounts(Vec<Pubkey>),
    /// `getProgramAccounts`.
    ProgramAccounts,
    /// `sendTransaction` of a transaction naming these short.win instruction count.
    Send { instructions: usize },
    /// `sendBundle` of this many transactions (a block engine call).
    SendBundle { txs: usize },
}

impl Rpc {
    /// The JSON-RPC calls this stands for (`getMultipleAccounts` takes 100 keys a call).
    pub fn calls(&self) -> usize {
        match self {
            Self::Accounts(keys) => keys.len().div_ceil(100).max(1),
            Self::ProgramAccounts | Self::Send { .. } | Self::SendBundle { .. } => 1,
        }
    }
}

/// A transaction the keeper sent (for assertions).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentTx {
    pub format: Option<TxFormat>,
    pub bytes: usize,
    pub accounts: usize,
    /// short.win instructions in it (ComputeBudget excluded).
    pub instructions: usize,
    pub ok: bool,
}

impl LiteSvmChain {
    /// A backend on `env` (shared with the test driving the market).
    pub fn new(env: Rc<RefCell<Env>>) -> Self {
        Self {
            env,
            v1: Cell::new(true),
            fees: RefCell::new(Vec::new()),
            sent: RefCell::new(Vec::new()),
            rpc: RefCell::new(Vec::new()),
            bundles: Cell::new(true),
            bundles_sent: RefCell::new(Vec::new()),
            before_bundle: RefCell::new(None),
        }
    }

    /// Runs `f` on the environment when the next bundle reaches the chain — after the keeper's
    /// read and pre-flight, before the bundle executes: T-STRADDLE's "a curve trade lands between
    /// the keeper's read and the bundle". The bundle's blockhash stays the one it was built with.
    pub fn before_next_bundle(&self, f: impl FnOnce(&mut Env) + 'static) {
        *self.before_bundle.borrow_mut() = Some(Box::new(f));
    }

    /// Accept (or refuse, as a chain without a block engine) bundles.
    pub fn set_bundles(&self, accepted: bool) {
        self.bundles.set(accepted);
    }

    /// Every bundle sent so far.
    pub fn bundles_sent(&self) -> Vec<SentBundle> {
        self.bundles_sent.borrow().clone()
    }

    /// Every backend call so far, in order (M4-SEC-7).
    pub fn rpc_log(&self) -> Vec<Rpc> {
        self.rpc.borrow().clone()
    }

    /// Accept (or refuse, as a cluster without SIMD-0385) v1 transactions.
    pub fn set_v1(&self, accepted: bool) {
        self.v1.set(accepted);
    }

    /// The recent prioritization fees the backend reports.
    pub fn set_priority_fees(&self, fees: Vec<u64>) {
        *self.fees.borrow_mut() = fees;
    }

    /// Every transaction sent so far.
    pub fn sent(&self) -> Vec<SentTx> {
        self.sent.borrow().clone()
    }

    fn refused(&self, tx: &VersionedTransaction) -> Option<TxOutcome> {
        (matches!(tx.message, VersionedMessage::V1(_)) && !self.v1.get()).then(|| TxOutcome {
            err: Some(TxError::Rejected(
                "unsupported transaction version 1".into(),
            )),
            ..TxOutcome::default()
        })
    }
}

/// The lookup tables a v0 transaction names, read from the VM.
fn lookup_tables(env: &Env, tx: &VersionedTransaction) -> Vec<AddressLookupTableAccount> {
    tx.message
        .address_table_lookups()
        .unwrap_or_default()
        .iter()
        .filter_map(|l| {
            let a = env.svm.get_account(&l.account_key)?;
            tx::parse_lookup_table(l.account_key, &a.owner, &a.data).ok()
        })
        .collect()
}

fn outcome(
    tx: &VersionedTransaction,
    alts: &[AddressLookupTableAccount],
    meta: &TransactionMetadata,
    err: Option<TxError>,
) -> TxOutcome {
    let keys =
        tx::account_keys(tx, alts).unwrap_or_else(|_| tx.message.static_account_keys().to_vec());
    let n_top = tx.message.instructions().len();
    let mut inner = vec![Vec::new(); n_top];
    for (i, list) in meta.inner_instructions.iter().enumerate() {
        if let Some(slot) = inner.get_mut(i) {
            for ii in list {
                if let Some(program_id) = keys.get(usize::from(ii.instruction.program_id_index)) {
                    slot.push(InnerIx {
                        program_id: *program_id,
                        data: ii.instruction.data.clone(),
                    });
                }
            }
        }
    }
    TxOutcome {
        signature: meta.signature,
        err,
        logs: meta.logs.clone(),
        units_consumed: meta.compute_units_consumed,
        fee: meta.fee,
        inner,
    }
}

/// Tracks every existing `Market` an instruction of `ixs` names (as `Env::send`).
fn track_named(env: &mut Env, ixs: &[Instruction]) {
    let named: Vec<Pubkey> = ixs
        .iter()
        .flat_map(|ix| ix.accounts.iter().map(|m| m.pubkey))
        .filter(|k| read::zc::<Market>(&env.svm, k).is_some())
        .collect();
    for k in named {
        env.track_market(k);
    }
}

/// Sends `tx` on `env` with the invariant hooks after every instruction (module docs).
/// Panics on a violation.
pub fn send_with_hooks(
    env: &mut Env,
    tx: &VersionedTransaction,
) -> Result<TransactionMetadata, litesvm::types::FailedTransactionMetadata> {
    if !env.check_invariants {
        // A test that corrupts state on purpose turned the hooks off (as `Env::send`).
        let res = env.svm.send_transaction(tx.clone());
        env.svm.expire_blockhash();
        return res;
    }
    let alts = lookup_tables(env, tx);
    let ixs = tx::decompile(tx, &alts).expect("decompiling the keeper's transaction");
    let payer = *tx
        .message
        .static_account_keys()
        .first()
        .expect("a fee payer");
    track_named(env, &ixs);
    let admins = env.admin_keys();
    let mut pre = invariants::snapshot(&env.svm, env.tracked_markets());
    let mut prefix_ok = true;
    for k in 1..ixs.len() {
        let prefix = &ixs[..k];
        let mut svm = env.svm.clone().with_sigverify(false);
        let msg = Message::new_with_blockhash(prefix, Some(&payer), tx.message.recent_blockhash());
        let n_sig = usize::from(msg.header.num_required_signatures);
        let prefix_tx = VersionedTransaction {
            signatures: vec![Signature::default(); n_sig],
            message: VersionedMessage::Legacy(msg),
        };
        let Ok(meta) = svm.send_transaction(prefix_tx) else {
            prefix_ok = false;
            break;
        };
        let fail = |v: invariants::Violation| -> ! {
            panic!(
                "{v}\n(after instruction {k} of {} of a keeper transaction)\nlogs:\n{}",
                ixs.len(),
                meta.pretty_logs()
            )
        };
        let ev = invariants::IxEvents::of_instruction(&meta, k - 1, &ixs[k - 1]);
        let admin = invariants::is_admin_signed(&ixs[k - 1], &admins);
        if let Err(v) = invariants::check_ix(&svm, &pre, admin, &ev) {
            fail(v);
        }
        let mut markets: Vec<Pubkey> = env.tracked_markets().to_vec();
        for key in prefix
            .iter()
            .flat_map(|ix| ix.accounts.iter().map(|m| m.pubkey))
            .filter(|key| read::zc::<Market>(&svm, key).is_some())
        {
            if !markets.contains(&key) {
                markets.push(key);
            }
        }
        for m in &markets {
            if let Err(v) = invariants::check_all(&svm, m) {
                fail(v);
            }
        }
        env.hook_checkpoints += 1;
        pre = invariants::snapshot(&svm, &markets);
    }

    let res = env.svm.send_transaction(tx.clone());
    env.svm.expire_blockhash();
    track_named(env, &ixs);
    env.hook_checkpoints += 1;
    let check_tracked = |env: &Env, meta: &TransactionMetadata| {
        for m in env.tracked_markets() {
            if let Err(v) = invariants::check_all(&env.svm, m) {
                panic!("{v}\nlogs:\n{}", meta.pretty_logs());
            }
        }
    };
    match &res {
        Ok(meta) => {
            assert!(
                prefix_ok,
                "a prefix of the keeper's transaction failed on the clone but the transaction \
                 succeeded\nlogs:\n{}",
                meta.pretty_logs()
            );
            let last = ixs.len().saturating_sub(1);
            if let Some(ix) = ixs.get(last) {
                let ev = invariants::IxEvents::of_instruction(meta, last, ix);
                let admin = invariants::is_admin_signed(ix, &admins);
                if let Err(v) = invariants::check_ix(&env.svm, &pre, admin, &ev) {
                    panic!(
                        "{v}\n(instruction {} of {} of a keeper transaction)\nlogs:\n{}",
                        last + 1,
                        ixs.len(),
                        meta.pretty_logs()
                    );
                }
            }
            check_tracked(env, meta);
        }
        Err(failed) => check_tracked(env, &failed.meta),
    }
    res
}

impl Chain for LiteSvmChain {
    async fn clock(&self) -> anyhow::Result<ChainClock> {
        let c = self.env.borrow().clock();
        Ok(ChainClock {
            slot: c.slot,
            unix_timestamp: c.unix_timestamp,
        })
    }

    async fn accounts(&self, keys: &[Pubkey]) -> anyhow::Result<Vec<Option<AccountData>>> {
        self.rpc.borrow_mut().push(Rpc::Accounts(keys.to_vec()));
        let env = self.env.borrow();
        Ok(keys
            .iter()
            .map(|k| {
                env.svm.get_account(k).map(|a| AccountData {
                    lamports: a.lamports,
                    owner: a.owner,
                    executable: a.executable,
                    data: a.data,
                })
            })
            .collect())
    }

    async fn program_accounts(
        &self,
        program: &Pubkey,
        filters: &[Memcmp],
    ) -> anyhow::Result<Vec<(Pubkey, AccountData)>> {
        self.rpc.borrow_mut().push(Rpc::ProgramAccounts);
        let env = self.env.borrow();
        Ok(env
            .svm
            .get_program_accounts(program)
            .into_iter()
            .filter(|(_, a)| {
                filters.iter().all(|f| {
                    a.data
                        .get(f.offset..f.offset.saturating_add(f.bytes.len()))
                        .is_some_and(|b| b == f.bytes.as_slice())
                })
            })
            .map(|(k, a)| {
                (
                    k,
                    AccountData {
                        lamports: a.lamports,
                        owner: a.owner,
                        executable: a.executable,
                        data: a.data,
                    },
                )
            })
            .collect())
    }

    async fn latest_blockhash(&self) -> anyhow::Result<Hash> {
        Ok(self.env.borrow().svm.latest_blockhash())
    }

    async fn simulate(&self, tx: &VersionedTransaction) -> anyhow::Result<TxOutcome> {
        if let Some(r) = self.refused(tx) {
            return Ok(r);
        }
        let env = self.env.borrow();
        let alts = lookup_tables(&env, tx);
        Ok(match env.svm.simulate_transaction(tx.clone()) {
            Ok(info) => outcome(tx, &alts, &info.meta, None),
            Err(f) => outcome(
                tx,
                &alts,
                &f.meta,
                Some(TxError::from_transaction_error(f.err)),
            ),
        })
    }

    async fn send(&self, tx: &VersionedTransaction) -> anyhow::Result<TxOutcome> {
        if let Some(r) = self.refused(tx) {
            return Ok(r);
        }
        let (res, alts) = {
            let mut env = self.env.borrow_mut();
            let alts = lookup_tables(&env, tx);
            (send_with_hooks(&mut env, tx), alts)
        };
        let out = match &res {
            Ok(meta) => outcome(tx, &alts, meta, None),
            Err(f) => outcome(
                tx,
                &alts,
                &f.meta,
                Some(TxError::from_transaction_error(f.err.clone())),
            ),
        };
        let ixs = tx::decompile(tx, &alts).unwrap_or_default();
        self.rpc.borrow_mut().push(Rpc::Send {
            instructions: ixs.iter().filter(|i| i.program_id == shortwin::ID).count(),
        });
        self.sent.borrow_mut().push(SentTx {
            format: tx::format_of(tx),
            bytes: tx::size(tx),
            accounts: tx::account_count(tx),
            instructions: ixs.iter().filter(|i| i.program_id == shortwin::ID).count(),
            ok: out.ok(),
        });
        Ok(out)
    }

    async fn recent_priority_fees(&self, _writable: &[Pubkey]) -> anyhow::Result<Vec<u64>> {
        Ok(self.fees.borrow().clone())
    }

    async fn send_bundle(&self, txs: &[VersionedTransaction]) -> anyhow::Result<BundleOutcome> {
        if !self.bundles.get() {
            anyhow::bail!("no block engine configured: bundles cannot be sent");
        }
        if txs.is_empty() || txs.len() > 5 {
            anyhow::bail!("a bundle holds 1 to 5 transactions, not {}", txs.len());
        }
        self.rpc
            .borrow_mut()
            .push(Rpc::SendBundle { txs: txs.len() });
        let mut env = self.env.borrow_mut();
        // One slot: every transaction must carry the blockhash the chain had when the bundle
        // arrived; each then runs with the blockhash check off (the harness expires the
        // blockhash after every send).
        let latest = env.svm.latest_blockhash();
        if let Some(f) = self.before_bundle.borrow_mut().take() {
            f(&mut env);
        }
        if let Some(i) = txs
            .iter()
            .position(|t| *t.message.recent_blockhash() != latest)
        {
            return Ok(BundleOutcome {
                detail: format!("tx{}: BlockhashNotFound", i + 1),
                ..BundleOutcome::default()
            });
        }
        let before = env.svm.clone();
        env.svm = before.clone().with_blockhash_check(false);
        let mut outs = Vec::with_capacity(txs.len());
        let mut record = SentBundle {
            txs: Vec::new(),
            top: Vec::new(),
            trace: Vec::new(),
            units: Vec::new(),
            landed: false,
        };
        let mut failed: Option<usize> = None;
        for (i, tx) in txs.iter().enumerate() {
            let alts = lookup_tables(&env, tx);
            let res = send_with_hooks(&mut env, tx);
            let (out, trace) = match &res {
                Ok(meta) => (
                    outcome(tx, &alts, meta, None),
                    tx.message.instructions().len()
                        + meta.inner_instructions.iter().map(Vec::len).sum::<usize>(),
                ),
                Err(f) => (
                    outcome(
                        tx,
                        &alts,
                        &f.meta,
                        Some(TxError::from_transaction_error(f.err.clone())),
                    ),
                    0,
                ),
            };
            let ixs = tx::decompile(tx, &alts).unwrap_or_default();
            record.txs.push(SentTx {
                format: tx::format_of(tx),
                bytes: tx::size(tx),
                accounts: tx::account_count(tx),
                instructions: ixs.iter().filter(|i| i.program_id == shortwin::ID).count(),
                ok: out.ok(),
            });
            record.top.push(tx.message.instructions().len());
            record.trace.push(trace);
            record.units.push(out.units_consumed);
            let ok = out.ok();
            outs.push(out);
            if !ok {
                failed = Some(i);
                break;
            }
        }
        let landed = failed.is_none();
        if landed {
            // The harness's VM checks blockhashes (`LiteSVM::new`); restore that.
            env.svm = std::mem::replace(&mut env.svm, before).with_blockhash_check(true);
            env.svm.expire_blockhash();
        } else {
            // All-or-nothing: nothing of the bundle is committed.
            env.svm = before;
        }
        record.landed = landed;
        self.bundles_sent.borrow_mut().push(record);
        let detail = failed
            .and_then(|i| outs.get(i).map(|o| format!("tx{}: {:?}", i + 1, o.err)))
            .unwrap_or_default();
        Ok(BundleOutcome {
            id: format!("litesvm-bundle-{}", self.bundles_sent.borrow().len()),
            landed,
            txs: outs,
            detail,
        })
    }

    async fn simulate_bundle(
        &self,
        txs: &[VersionedTransaction],
    ) -> anyhow::Result<Vec<TxOutcome>> {
        if !self.bundles.get() {
            anyhow::bail!("no bundle simulation on this chain");
        }
        let env = self.env.borrow();
        let mut svm = env
            .svm
            .clone()
            .with_sigverify(false)
            .with_blockhash_check(false);
        let mut outs = Vec::with_capacity(txs.len());
        for tx in txs {
            if let Some(r) = self.refused(tx) {
                outs.push(r);
                break;
            }
            let alts = lookup_tables(&env, tx);
            let out = match svm.send_transaction(tx.clone()) {
                Ok(meta) => outcome(tx, &alts, &meta, None),
                Err(f) => outcome(
                    tx,
                    &alts,
                    &f.meta,
                    Some(TxError::from_transaction_error(f.err)),
                ),
            };
            let ok = out.ok();
            outs.push(out);
            if !ok {
                break;
            }
        }
        Ok(outs)
    }
}

/// Path of the straddle helper's SBF build: `$COMPLETE_CURVE_SO`, else
/// `target/deploy/complete_curve.so` (`anchor build --tools-version v1.57 --arch v3`).
pub fn complete_curve_so() -> std::path::PathBuf {
    std::env::var_os("COMPLETE_CURVE_SO").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/deploy/complete_curve.so")
        },
        std::path::PathBuf::from,
    )
}

/// Deploys the `complete-curve` helper (ARCH §5.4, A66) into `env` at its program id.
pub fn load_complete_curve(env: &mut Env) {
    let path = complete_curve_so();
    let so = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}. Build it first: `anchor build --tools-version v1.57 --arch v3` \
             (or set COMPLETE_CURVE_SO)",
            path.display()
        )
    });
    env.svm
        .add_program(complete_curve::ID, &so)
        .unwrap_or_else(|e| panic!("add_program(complete_curve): {e:?}"));
}

/// Writes an active address lookup table holding `addresses` at `key` (as the ALT program
/// leaves it after `extend_lookup_table` in an earlier slot): the protocol ALT of A28 for the
/// v0 fallback.
pub fn install_lookup_table(env: &mut Env, key: Pubkey, addresses: &[Pubkey]) {
    let mut data = vec![0u8; tx::ALT_META_SIZE];
    data[..4].copy_from_slice(&1u32.to_le_bytes()); // ProgramState::LookupTable
    data[4..12].copy_from_slice(&u64::MAX.to_le_bytes()); // deactivation_slot: active
    // last_extended_slot 0, start index 0, no authority (frozen), padding.
    for a in addresses {
        data.extend_from_slice(a.as_ref());
    }
    let lamports = env.svm.minimum_balance_for_rent_exemption(data.len());
    env.svm
        .set_account(
            key,
            solana_account::Account {
                lamports,
                data,
                owner: tx::ALT_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        )
        .expect("lookup table");
}

/// A test environment with a keeper (module docs).
pub struct Harness {
    pub env: Rc<RefCell<Env>>,
    pub keeper: Keeper<LiteSvmChain>,
    /// Every tick's report, in order.
    pub reports: Vec<TickReport>,
}

impl Harness {
    /// A keeper with `cfg` on `env`; its fee payer is funded with [`KEEPER_FUNDING`].
    pub fn new(env: Env, cfg: KeeperConfig) -> Self {
        let env = Rc::new(RefCell::new(env));
        let payer = Keypair::new();
        env.borrow_mut().fund(&payer.pubkey(), KEEPER_FUNDING);
        let chain = LiteSvmChain::new(env.clone());
        let keeper = Keeper::new(chain, payer, cfg).expect("keeper config");
        Self {
            env,
            keeper,
            reports: Vec::new(),
        }
    }

    /// The keeper's backend.
    pub fn chain(&self) -> &LiteSvmChain {
        self.keeper.chain()
    }

    /// Runs `f` on the environment.
    pub fn with_env<R>(&self, f: impl FnOnce(&mut Env) -> R) -> R {
        f(&mut self.env.borrow_mut())
    }

    /// One keeper tick at the current clock (panics on a keeper error).
    pub fn tick(&mut self) -> &TickReport {
        let report = block_on(self.keeper.tick()).expect("keeper tick");
        self.reports.push(report);
        self.reports.last().expect("just pushed")
    }

    /// Advances one slot (same second) and ticks.
    pub fn slot(&mut self) -> &TickReport {
        {
            let mut env = self.env.borrow_mut();
            let c = env.clock();
            env.set_clock(c.unix_timestamp, c.slot + 1);
        }
        self.tick()
    }

    /// Advances one second (and 2 slots) and ticks.
    pub fn second(&mut self) -> &TickReport {
        self.env.borrow_mut().warp(1);
        self.tick()
    }

    /// `n` seconds, one tick each.
    pub fn seconds(&mut self, n: u32) {
        for _ in 0..n {
            self.second();
        }
    }

    /// Advances `secs` without ticking (the keeper is down).
    pub fn idle(&mut self, secs: i64) {
        self.env.borrow_mut().warp(secs);
    }

    /// The current unix time.
    pub fn now(&self) -> i64 {
        self.env.borrow().now()
    }
}

/// Runs a future to completion on a current-thread runtime (the keeper's futures are not
/// `Send`; nothing in them awaits I/O on this backend).
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime")
        .block_on(f)
}
