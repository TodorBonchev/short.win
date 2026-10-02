//! The fuzzed world: one `Env` (the invariant hooks on every transaction), its markets
//! ([`Slot`]: a venue, borrowers, lenders, open positions), a keeper, and the interpretation of
//! every [`Kind`] of action. Oracles beyond the hooks, each a panic that the runner turns into a
//! crash file:
//!
//! - **gates (I10, I17)**: an `open_short` that lands while the market's or the global opens
//!   pause, the market's or the venue's route-broken flag, a disabled venue bit, the venue's
//!   upgrade latch or a non-executable venue (the harness's own view) holds just before it; a
//!   close, liquidation, expiry, settlement or top-up refused with an open-only gate code
//!   (paused, route broken, latched, disabled, caps, oracle, warm-up);
//! - **exits on the mock (I9, I11, I12, I13, I-E11)**: every `liquidate` / `expire` the keeper
//!   sends on an undisturbed mock market goes through `liquidate_support`'s differential oracle
//!   (the decision, including `ExitBoundNotMet` against `⌊B̂_ref·(10⁴ + δ)/10⁴⌋`, and every
//!   lamport and token of the waterfall recomputed from `shortwin-math` on the pre-state);
//! - **I14**: an instruction with a remaining account omitted or replaced by a fresh key must
//!   fail (the hooks check that nothing changed; an accepted replacement is an anomaly for
//!   review, an accepted omission a violation);
//! - **I4's coverage clause (O17)**: a landed `set_coverage_pause` changes nothing on the whole VM
//!   but bit 6 of one market's `flags` (and, for a clear of a set flag, its `coverage_cleared_at`);
//!   every refusal is the predicted code (the role inert, the signer, the reason) — since the
//!   owner's decision of 2026-10-01 (★OWNER-M7-3 (b)) the Emergency's clear lands with the role
//!   installed, unset or rotated, its set never does, and no open lands in the clear delay after
//!   it (`emergency_clear`);
//! - **the mint filter, governance, the oracle, client bounds**: exits and `withdraw` never
//!   return a mint-filter code; a change never applies before its timelock, after its window or
//!   after a cancel, and the Emergency cannot cancel a `Timelock` change; `crank_sol_usd` never
//!   caches a future-dated, too uncertain or foreign-owned update; unmeetable slippage bounds and
//!   zero deposits never land;
//! - **liveness (I8 / I-P6, I-E11)**: the run's wind-down — every gate and venue disturbance
//!   cleared, the price restored, every position closed or exited by the keeper (with deadline
//!   and S3 warps when needed), every lender out — must end with no position, no receivable, no
//!   rent aggregate and no borrowed tokens; the hooks' own liveness check (dust ≤ 0.01 SOL) runs
//!   after each of those transactions.

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::{InstructionData, ToAccountMetas};
use shortwin::PauseScope;
use shortwin::constants::{
    CONFIG_APPLY_WINDOW_S, COVERAGE_CLEAR_DELAY_S, MOCK_CPAMM_PROGRAM_ID, PUMP_AMM_PROGRAM_ID,
    PUMP_FEES_PROGRAM_ID, PUMP_PROGRAM_ID, PYTH_RECEIVER_ID, PYTH_SOL_USD_ACCOUNT,
    RAYDIUM_CPMM_PROGRAM_ID, RAYDIUM_V4_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
use shortwin::errors::ShortwinError as E;
use shortwin::events::COVERAGE_REASON_KEEPER;
use shortwin::state::{
    CONFIG_FLAG_PAUSED_OPENS, ConfigChange, EmaParamsArgs, MARKET_FLAG_MINT_UNSAFE,
    MARKET_FLAG_PAUSED_COVERAGE, MARKET_FLAG_PAUSED_OPENS, MARKET_FLAG_ROUTE_BROKEN, Market,
    Params, TierParams, VenueKind,
};
use shortwin::venues::ExtrasSel;
use shortwin_litesvm_tests::engine::{
    Borrower, add_margin_ix, close_short_ix, crank_price_ix, expire_ix, liquidate_ix,
    open_short_ix, set_route_broken_ix, sol_settle_ix, trader_wsol_ata, with_cu_limit,
};
use shortwin_litesvm_tests::invariants::positions::venue_executable;
use shortwin_litesvm_tests::ix::{self, LenderKeys};
use shortwin_litesvm_tests::{Env, TxResult, pda, read};
use shortwin_math::params::{self as mparams, Rule};
use shortwin_math::tier::Tier;
use solana_keypair::Keypair;
use solana_signer::Signer;

use crate::action::{Action, Kind, Profile};
use crate::liq;
use crate::stats::{Ctx, Stats, failure, shortwin_code};
use crate::venue::{Move, Venue};

/// The mainnet `PriceUpdateV2` dump (`tests/fixtures`), re-planted with moved prices.
const PYTH_DUMP: &[u8] = include_bytes!("../../fixtures/accounts/pyth_sol_usd_shard0.bin");

/// Codes an exit (close, liquidate, expire, sol_settle, add_margin) must never return: the
/// open-only gates of I10 / I17, the open checks, and the mint filter (exit paths never revert
/// on it: ARCH §3.0 step 3, SPEC §11).
const OPEN_ONLY: [E; 24] = [
    E::OpensPaused,
    E::CoveragePaused,
    E::VenueDisabled,
    E::VenueUpgraded,
    E::VenueFlagged,
    E::RouteBroken,
    E::MarketWarmingUp,
    E::PriceDeviatesFromEma,
    E::PriceAboveOpenMax,
    E::CapPositionImpact,
    E::CapMarketOi,
    E::CapUtilization,
    E::CapCurveGamma,
    E::CapWallet,
    E::BelowMinNotional,
    E::MarketNotionalCap,
    E::GraduationHalted,
    E::PoolListWindow,
    E::OracleStale,
    E::OracleFeed,
    E::MintFreezeAuthority,
    E::MintDisallowedExtension,
    E::MintMalformed,
    E::MintUnsafeFlagged,
];

/// The mint filter's codes: a lender's `withdraw` (an exit path) must never return them.
const MINT_FILTER: [E; 4] = [
    E::MintFreezeAuthority,
    E::MintDisallowedExtension,
    E::MintMalformed,
    E::MintUnsafeFlagged,
];

/// `COption<Pubkey>` `freeze_authority` in an SPL / Token-2022 mint's base layout: the tag at
/// 46, the key at 50..82.
const MINT_FREEZE_AT: usize = 46;
const MINT_FREEZE_END: usize = 82;

/// The owners whose accounts [`World::snapshot`] takes: the protocol, the token programs, every
/// venue program and the System program.
const SNAPSHOT_OWNERS: [Pubkey; 10] = [
    shortwin::ID,
    TOKEN_PROGRAM_ID,
    TOKEN_2022_PROGRAM_ID,
    MOCK_CPAMM_PROGRAM_ID,
    PUMP_PROGRAM_ID,
    PUMP_AMM_PROGRAM_ID,
    PUMP_FEES_PROGRAM_ID,
    RAYDIUM_V4_PROGRAM_ID,
    RAYDIUM_CPMM_PROGRAM_ID,
    anchor_lang::system_program::ID,
];

/// `Market.flags` (u16) in the account data (discriminator included); bit 6 is in its low byte.
const MARKET_FLAGS_OFFSET: usize = 10;

/// `Market.coverage_cleared_at`'s offset (discriminator included; M7-SEC-3).
const MARKET_COVERAGE_CLEARED_AT_OFFSET: usize = 2_928;

/// The whole VM state [`SNAPSHOT_OWNERS`] hold: key → (lamports, owner, data).
pub type Snapshot = std::collections::BTreeMap<Pubkey, (u64, Pubkey, Vec<u8>)>;

fn code(e: E) -> u32 {
    u32::from(e)
}

/// The name of a short.win error code (for messages).
pub fn code_name(c: u32) -> String {
    code_name_any(c)
}

/// A fee-shock level every venue maps to its highest fee (above the hard cap): `41 ≡ 6 (mod 7)`
/// (the mock's 15 %) and `≡ 5 (mod 6)` (AMM v4's 10 %, CPMM's 10 %).
pub const FEE_ABOVE_CAP: u32 = 41;

/// A fuzzer panic message prefix (the runner's crash files group by it).
pub const ORACLE: &str = "FUZZ-ORACLE";

thread_local! {
    /// A deliberate failure for the harness's own tests (`tests/smoke.rs`): the first action of
    /// this kind applied while a position is open panics with [`INJECTED`].
    pub static INJECT: std::cell::Cell<Option<Kind>> = const { std::cell::Cell::new(None) };
}

/// The injected failure's message.
pub const INJECTED: &str = "FUZZ-INJECTED failure (harness self-test)";

/// An open position: its borrower (index into the slot's borrowers) and nonce.
#[derive(Clone, Copy, Debug)]
pub struct Pos {
    pub b: usize,
    pub nonce: u64,
}

/// A lender of a market.
pub struct Lender {
    pub kp: Keypair,
    pub account: Pubkey,
}

/// A market under test.
pub struct Slot {
    pub venue: Box<dyn Venue>,
    pub borrowers: Vec<Borrower>,
    pub lenders: Vec<Lender>,
    pub positions: Vec<Pos>,
    /// The spot at setup (the wind-down restores it).
    pub spot0: Option<u128>,
}

/// The fuzzed world (module docs).
pub struct World {
    pub env: Env,
    pub profile: Profile,
    pub slots: Vec<Slot>,
    pub keeper: Keypair,
    pub donor: Keypair,
    pub stats: Stats,
    /// Keeper exits on an undisturbed mock market go through the differential oracle.
    pub differential: bool,
    /// Skip the actions that would only re-trigger a known finding (README "Known findings").
    pub avoid_known: bool,
    /// A venue bit disabled by `disable_venue` (re-enabled by the wind-down).
    pub disabled_bits: u32,
    /// Actions applied so far.
    pub step: u64,
    /// Disturbances to clear at a later step (gates, halts, fee shocks, latches): each one set
    /// by an action lasts `1..=8` further actions, so it interleaves with any other action
    /// without starving the run of opens.
    pub undo: Vec<(u64, Undo)>,
    /// The key installed as `coverage_authority` (O17), `None` while the role is inert (the
    /// default key). Setup plants one in most runs; [`Kind::CoveragePause`] rotates it through
    /// Governance's timelocked `Authorities` change.
    pub coverage: Option<Keypair>,
    /// Mints whose `freeze_authority` a mint shock replaced: the original bytes `46..82`.
    pub mint_saved: Vec<(Pubkey, Vec<u8>)>,
}

/// A disturbance's clearing action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Undo {
    MarketGates(usize),
    GlobalPause,
    VenueRoute(usize),
    VenueHalt(usize),
    VenueFee(usize),
    VenueKnobs(usize),
    EnableVenues,
    Revalidate(usize),
    /// The keeper clears the market's `paused_coverage` (coverage restored).
    CoveragePause(usize),
    /// The mint's original `freeze_authority` is written back (the market's `mint_unsafe` flag,
    /// if one was set, stays: it is one-way).
    Mint(usize),
}

/// An open's client bounds from its number `pick`: `(min_proceeds, max_margin, max_open_fee)`;
/// three opens in thirty-two carry one bound they cannot meet (`ProceedsBelowMin`,
/// `MarginAboveMax`, `OpenFeeAboveMax`).
fn open_bounds(pick: u32) -> (u64, u64, u64) {
    match (pick >> 8) % 32 {
        29 => (u64::MAX, u64::MAX, u64::MAX),
        30 => (0, 0, u64::MAX),
        31 => (0, u64::MAX, 0),
        _ => (0, u64::MAX, u64::MAX),
    }
}

/// `√` of a WAD price ratio for spot comparisons.
fn spot_of(r: Option<(u64, u64)>) -> Option<u128> {
    let (x, y) = r?;
    (x > 0).then(|| u128::from(y) * shortwin_math::num::WAD / u128::from(x))
}

/// A System transfer.
pub fn transfer_ix(from: &Pubkey, to: &Pubkey, lamports: u64) -> Instruction {
    let mut data = vec![2, 0, 0, 0];
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: anchor_lang::system_program::ID,
        accounts: vec![AccountMeta::new(*from, true), AccountMeta::new(*to, false)],
        data,
    }
}

/// The factor `k` in ppm of a price move from `b`: log-uniform in `[1.05, 10]`.
pub fn k_ppm(b: u32) -> u64 {
    let t = f64::from(b % 10_000) / 10_000.0;
    let k = 1.05_f64 * (10.0_f64 / 1.05).powf(t);
    (k * 1e6) as u64
}

impl World {
    /// A [`Ctx`] over the world's VM and counters.
    pub fn cx(&mut self) -> Ctx<'_> {
        Ctx {
            env: &mut self.env,
            stats: &mut self.stats,
        }
    }

    fn slot_index(&self, m: u8) -> usize {
        usize::from(m) % self.slots.len()
    }

    /// The market of slot `si`.
    pub fn market_key(&self, si: usize) -> Pubkey {
        self.slots[si].venue.vm().market()
    }

    /// The market of slot `si` as stored.
    pub fn market(&self, si: usize) -> Market {
        self.env.market(&self.market_key(si))
    }

    /// Sends through the hooks and the counters.
    pub fn send(&mut self, ixs: &[Instruction], signers: &[&Keypair]) -> TxResult {
        self.cx().send(ixs, signers)
    }

    /// Sends `ixs` behind the venue's compute-unit limit instruction (when it has one).
    /// Returns the result and the instructions **as sent**: an instruction index in the
    /// result refers to that list (the oracles read failures against it).
    fn send_cu(
        &mut self,
        si: usize,
        ixs: &[Instruction],
        signers: &[&Keypair],
    ) -> (TxResult, Vec<Instruction>) {
        let cu = self.slots[si].venue.exit_cu();
        Self::with_cu(cu, ixs, |all| self.cx().send(all, signers))
    }

    /// `ixs` behind a compute-unit limit instruction when `cu` is set, sent by `f`.
    fn with_cu(
        cu: Option<u32>,
        ixs: &[Instruction],
        f: impl FnOnce(&[Instruction]) -> TxResult,
    ) -> (TxResult, Vec<Instruction>) {
        let mut all = Vec::with_capacity(ixs.len() + 1);
        if let Some(units) = cu {
            all.push(shortwin_litesvm_tests::pump::cu_limit_ix(units));
        }
        all.extend_from_slice(ixs);
        (f(&all), all)
    }

    /// A bundle's compute budget: the venue's, or 1.4 M on a pump venue.
    /// (The instructions as sent come back with the result, as for [`World::send_cu`].)
    fn send_bundle(
        &mut self,
        si: usize,
        ixs: &[Instruction],
        signers: &[&Keypair],
    ) -> (TxResult, Vec<Instruction>) {
        let cu = self.slots[si].venue.exit_cu().map(|_| 1_400_000);
        Self::with_cu(cu, ixs, |all| self.cx().send(all, signers))
    }

    // ---- gates ---------------------------------------------------------------------------------

    /// Why an open on slot `si` must be refused now, if it must (I10 / I17 gates read from the
    /// state the program sees).
    pub fn open_gate(&self, si: usize) -> Option<String> {
        self.open_gate_with(si, true)
    }

    /// [`Self::open_gate`], with the coverage pause and its clear delay left out when `coverage`
    /// is false (the gates a coverage bracket's open still meets, M7-SEC-3).
    fn open_gate_with(&self, si: usize, coverage: bool) -> Option<String> {
        let m = self.market(si);
        let c = self.env.config();
        if m.has_flag(MARKET_FLAG_PAUSED_OPENS) {
            return Some("the market's opens pause".into());
        }
        if coverage && m.has_flag(MARKET_FLAG_PAUSED_COVERAGE) {
            return Some("the market's coverage pause".into());
        }
        if coverage && self.env.now() < m.coverage_cleared_at.saturating_add(COVERAGE_CLEAR_DELAY_S)
        {
            return Some("the coverage pause's clear delay (M7-SEC-3)".into());
        }
        if m.has_flag(MARKET_FLAG_MINT_UNSAFE) {
            return Some("the market's mint_unsafe flag".into());
        }
        let mint = self.slots[si].venue.vm().mint();
        if self.mint_saved.iter().any(|(k, _)| *k == mint) {
            return Some("a freeze authority on the mint (the mint filter)".into());
        }
        if m.has_flag(MARKET_FLAG_ROUTE_BROKEN) {
            return Some("the market's route-broken flag".into());
        }
        if c.flags & CONFIG_FLAG_PAUSED_OPENS != 0 {
            return Some("the global opens pause".into());
        }
        if let Some(kind) = m.venue_kind() {
            let bit = kind.venue_bit();
            if bit != 0 && c.venue_enabled & bit == 0 {
                return Some(format!("{kind:?} disabled in venue_enabled"));
            }
            if bit != 0 && c.venue_route_broken & bit != 0 {
                return Some(format!("{kind:?}'s global route-broken flag"));
            }
            let g = self.env.venue_guard();
            if g.upgraded & kind.guard_mask() != 0 {
                return Some(format!("{kind:?}'s upgrade latch ({:#b})", g.upgraded));
            }
            if kind == VenueKind::None || kind == VenueKind::PumpMigrating {
                return Some(format!("venue {kind:?}"));
            }
        }
        if venue_executable(&self.env.svm, &m) == Some(false) {
            return Some("a venue that is not executable (harness view)".into());
        }
        None
    }

    /// Fails the run if an exit was refused with an open-only gate code (I10).
    fn check_exit_refusal(&mut self, what: &str, ixs: &[Instruction], res: &TxResult) {
        let Some(c) = shortwin_code(ixs, res) else {
            return;
        };
        if OPEN_ONLY.iter().any(|e| code(*e) == c) {
            let logs = res
                .as_ref()
                .err()
                .map(|f| f.meta.pretty_logs())
                .unwrap_or_default();
            panic!(
                "{ORACLE} I10: {what} refused with the open-only gate {} ({c})\nlogs:\n{logs}",
                code_name(c)
            );
        }
    }

    // ---- bookkeeping -----------------------------------------------------------------------------

    /// Drops positions whose account is gone (closed, liquidated, expired or settled).
    pub fn prune(&mut self) {
        for si in 0..self.slots.len() {
            let market = self.market_key(si);
            let slot = &mut self.slots[si];
            let borrowers = &slot.borrowers;
            let svm = &self.env.svm;
            slot.positions.retain(|p| {
                let key = pda::position(&market, &borrowers[p.b].key(), p.nonce).0;
                read::account(svm, &key).is_some_and(|a| a.lamports > 0)
            });
        }
    }

    fn lender_keys(&self, si: usize, li: usize) -> LenderKeys {
        let vm = &self.slots[si].venue.vm();
        let l = &self.slots[si].lenders[li];
        LenderKeys {
            owner: l.kp.pubkey(),
            mint: vm.mint(),
            token_program: vm.keys().token_program,
            owner_token_account: l.account,
        }
    }

    /// The `ShareEpoch` records a lender instruction needs (the lender's epoch up to the
    /// market's).
    fn epoch_metas(&self, si: usize, owner: &Pubkey) -> Vec<AccountMeta> {
        let market = self.market_key(si);
        let m = self.env.market(&market);
        let from = self
            .env
            .lender(&market, owner)
            .map_or(m.share_epoch, |l| l.epoch);
        let to = m.share_epoch.min(from + 16);
        ix::epoch_records(&market, from, to)
    }

    // ---- keeper ----------------------------------------------------------------------------------

    /// One `crank_price` of slot `si` (a refusal is recorded, never fatal).
    pub fn crank(&mut self, si: usize) -> TxResult {
        let ix = crank_price_ix(&self.slots[si].venue.vm());
        let res = self.send(&[ix], &[]);
        let World {
            env, stats, slots, ..
        } = self;
        slots[si].venue.after_crank(&mut Ctx { env, stats });
        res
    }

    /// A keeper exit on position `k` of slot `si`: `which` 0 liquidate, 1 expire, 2 sol_settle.
    /// Returns whether the position is gone.
    pub fn keeper_exit(&mut self, si: usize, k: usize, which: u32) -> bool {
        let Some(p) = self.slots[si].positions.get(k).copied() else {
            return false;
        };
        let borrower = self.slots[si].borrowers[p.b].key();
        let keeper = self.keeper.insecure_clone();
        let differential =
            self.differential && which < 2 && self.slots[si].as_mock_undisturbed(&self.env);
        if differential {
            let mm = self.slots[si].venue.as_mock().expect("mock");
            let w = if which == 0 {
                liq::Which::Liquidate
            } else {
                liq::Which::Expire
            };
            let exit = liq::exit_checked(
                &mut self.env,
                mm,
                w,
                &keeper,
                &borrower,
                p.nonce,
                &[],
                &[],
                &[],
            );
            let name = if which == 0 { "liquidate" } else { "expire" };
            let tag = if exit.executed() {
                format!("{name}:differential-landed")
            } else {
                format!("{name}:differential-refused:{:?}", exit.refused())
            };
            *self.stats.events.entry(tag).or_default() += 1;
            if exit.executed() {
                self.stats.record(&[], &Ok(exit.meta().clone()));
                self.slots[si].positions.remove(k);
                return true;
            }
            return false;
        }
        let vm = &self.slots[si].venue.vm();
        let ix = match which {
            0 => liquidate_ix(vm, &keeper.pubkey(), &borrower, p.nonce),
            1 => expire_ix(vm, &keeper.pubkey(), &borrower, p.nonce),
            _ => sol_settle_ix(&self.env, vm, &keeper.pubkey(), &borrower, p.nonce),
        };
        let (res, sent) = self.send_cu(si, std::slice::from_ref(&ix), &[&keeper]);
        let what = ["liquidate", "expire", "sol_settle"][which as usize % 3];
        self.check_exit_refusal(what, &sent, &res);
        let market = self.market_key(si);
        let gone = read::account(&self.env.svm, &pda::position(&market, &borrower, p.nonce).0)
            .is_none_or(|a| a.lamports == 0);
        if gone {
            self.slots[si].positions.remove(k);
        }
        gone
    }

    /// Every keeper exit on every open position of slot `si` (liquidate, expire, then
    /// `sol_settle`, as the in-house keeper does).
    pub fn keeper_sweep(&mut self, si: usize) {
        let mut k = 0;
        while k < self.slots[si].positions.len() {
            let gone = self.keeper_exit(si, k, 0)
                || self.keeper_exit(si, k, 1)
                || self.keeper_exit(si, k, 2);
            if !gone {
                k += 1;
            }
        }
    }

    /// One keeper second: the clock advances by 1 s, every market is cranked, then every exit.
    pub fn keeper_second(&mut self) {
        self.env.warp(1);
        for si in 0..self.slots.len() {
            self.crank(si);
        }
        for si in 0..self.slots.len() {
            self.keeper_sweep(si);
        }
    }

    /// `n` keeper seconds.
    pub fn keeper_seconds(&mut self, n: u32) {
        for _ in 0..n {
            self.keeper_second();
        }
    }

    // ---- actions ---------------------------------------------------------------------------------

    /// Interprets `act` (module docs). Returns `false` when it did nothing (skipped).
    pub fn apply(&mut self, act: &Action) -> bool {
        if INJECT.with(std::cell::Cell::get) == Some(act.kind)
            && self.slots.iter().any(|s| !s.positions.is_empty())
        {
            panic!("{INJECTED}: `{act}`");
        }
        self.step += 1;
        self.run_undos(false);
        let si = self.slot_index(act.m);
        let (a, b) = (act.a, act.b);
        let done = match act.kind {
            Kind::Open => self.open(si, a, b, false),
            Kind::OpenNearCap => self.open(si, a, b, true),
            Kind::Close => self.close(si, a),
            Kind::AddMargin => self.add_margin(si, a, b),
            Kind::Deposit => self.deposit(si, a, b),
            Kind::Withdraw => self.withdraw(si, a, b),
            Kind::ClaimSol => self.claim(si, a, b),
            Kind::CoverDeficit => self.cover_deficit(si, a, b),
            Kind::SyncLender => self.sync_lender(si, a),
            Kind::SweepTrader => {
                let ix = self.slots[si].venue.sweep_ix(&self.env);
                self.send(&[ix], &[]);
                true
            }
            Kind::DustSweeps => self.dust_sweeps(si, b),
            Kind::Outage => self.outage(si, a, b),
            Kind::FastPath => self.fast_path(si, a, b),
            Kind::LendersOut => self.lenders_out(si, b % 2 == 1, false),
            Kind::KeeperSeconds => {
                self.keeper_seconds(1 + a % 40);
                true
            }
            Kind::Crank => {
                self.crank(si);
                true
            }
            Kind::KeeperExit => {
                let n = self.slots[si].positions.len();
                if n == 0 {
                    return self.skip(act);
                }
                self.keeper_exit(si, a as usize % n, b % 3);
                true
            }
            Kind::Silence => {
                // Past `COVERAGE_GAP_S` (40 s since OWNER-M7-12): the next crank reports the gap.
                self.env
                    .warp(shortwin_math::consts::COVERAGE_GAP_S + 1 + i64::from(a % 900));
                for si in 0..self.slots.len() {
                    self.crank(si);
                }
                // Degraded mode (O17): the keeper that lost coverage pauses opens on every
                // market it missed, and clears the pause `1..=8` actions later.
                if !b.is_multiple_of(3) && self.coverage.is_some() {
                    for si in 0..self.slots.len() {
                        if self.coverage_set(si, true) {
                            self.later(b / 3, Undo::CoveragePause(si));
                        }
                    }
                }
                true
            }
            Kind::Warp => {
                let t = f64::from(a % 10_000) / 10_000.0;
                let secs = (259_200_f64).powf(t).max(1.0) as i64;
                self.env.warp(secs);
                true
            }
            Kind::WarpToDeadline => self.warp_to_deadline(si, a, b),
            Kind::Trade => self.trade(si, a, b),
            Kind::Shock => self.shock(si, a, b),
            Kind::Ramp => self.ramp(si, a, b),
            Kind::Liquidity => self.liquidity(si, a, b),
            Kind::LpWrappedCrank => self.lp_wrapped_crank(si, a, b),
            Kind::DepositWrappedOpen => self.deposit_wrapped_open(si, a, b),
            Kind::WithdrawWrappedExit => self.withdraw_wrapped_exit(si, a, b),
            Kind::PumpCrankDump => self.pump_crank_dump(si, a, b),
            Kind::ContraryLow => self.contrary_low(si, a, b),
            Kind::AttackerSeconds => self.attacker_seconds(si, a, b),
            Kind::PushThenFreeze => self.push_then_freeze(si, a, b),
            Kind::SandwichExit => self.sandwich_exit(si, a, b, false),
            Kind::HeldSandwich => self.sandwich_exit(si, a, b, true),
            Kind::DumpAroundSettle => self.dump_around_settle(si, a, b),
            Kind::JitDeposit => self.jit_deposit(si, a, b),
            Kind::Gates => self.gates(si, a),
            Kind::GlobalPause => {
                let e = self.env.pause_authority.insecure_clone();
                let on = a % 4 == 0;
                let ix = ix::set_pause(&e.pubkey(), PauseScope::Global, on, None);
                self.send(&[ix], &[&e]);
                if on {
                    self.later(a / 4, Undo::GlobalPause);
                }
                true
            }
            Kind::DisableVenue => self.disable_venue(si),
            Kind::VenueHalt => {
                let level = if a % 4 == 0 { 1 + a / 4 } else { 0 };
                let World {
                    env, stats, slots, ..
                } = self;
                let done = slots[si].venue.halt(&mut Ctx { env, stats }, level);
                if done && level != 0 {
                    self.later(b, Undo::VenueHalt(si));
                }
                done
            }
            Kind::FeeShock => {
                let level = if a % 3 == 0 { 1 + a / 3 } else { 0 };
                let World {
                    env, stats, slots, ..
                } = self;
                let done = slots[si].venue.fee_shock(&mut Ctx { env, stats }, level);
                if done && level != 0 {
                    self.later(b, Undo::VenueFee(si));
                }
                done
            }
            Kind::UpgradeShock => self.upgrade_shock(si, a, b),
            Kind::Revalidate => self.revalidate(si),
            Kind::RouteBrokenVenue => {
                let bit = self.venue_bit_index(si);
                let e = self.env.pause_authority.insecure_clone();
                let on = a % 4 == 0;
                let ix = set_route_broken_ix(&e.pubkey(), PauseScope::Global, bit, on, None);
                self.send(&[ix], &[&e]);
                if on {
                    self.later(b, Undo::VenueRoute(si));
                }
                true
            }
            Kind::Donate => self.donate(si, a, b),
            Kind::ConfigChange => self.config_change(a, b),
            Kind::OracleCrank => self.oracle_crank(a, b),
            Kind::OmitAccount => self.i14(si, a, b, false),
            Kind::ReplaceAccount => self.i14(si, a, b, true),
            Kind::CoveragePause => self.coverage_pause(si, a, b),
            Kind::CoverageBracket => self.coverage_bracket(si, a, b),
            Kind::EmergencyClear => self.emergency_clear(si, a, b),
            Kind::WrappedOpen => self.wrapped_open(si, a, b),
            Kind::SweepFees => {
                let market = self.market_key(si);
                let to = self.env.config().fee_recipient;
                self.send(&[ix::sweep_protocol_fees(&market, &to)], &[]);
                true
            }
            Kind::MintShock => self.mint_shock(si, a, b),
            Kind::ConfigCancel => self.config_cancel(a, b),
            Kind::ExtrasSet
            | Kind::LayoutShock
            | Kind::OverDelivery
            | Kind::ExtrasShock
            | Kind::PlantCandidate
            | Kind::ProposePool
            | Kind::ExecuteSwitch
            | Kind::CompleteCurve
            | Kind::Migrate
            | Kind::GraduationHalt => {
                let World {
                    env, stats, slots, ..
                } = self;
                let done = slots[si]
                    .venue
                    .special(&mut Ctx { env, stats }, act.kind, a, b)
                    .is_some();
                if done
                    && matches!(
                        act.kind,
                        Kind::LayoutShock | Kind::OverDelivery | Kind::ExtrasShock
                    )
                {
                    self.later(b, Undo::VenueKnobs(si));
                }
                done
            }
        };
        self.prune();
        if !done {
            return self.skip(act);
        }
        *self
            .stats
            .actions
            .entry(act.kind.name().to_string())
            .or_default() += 1;
        true
    }

    /// Schedules `u` to run `1 + (a mod 8)` actions from now.
    fn later(&mut self, a: u32, u: Undo) {
        let at = self.step + 1 + u64::from(a % 8);
        self.undo.push((at, u));
    }

    /// Runs the undos that are due (all of them with `all`).
    pub fn run_undos(&mut self, all: bool) {
        let step = self.step;
        let (due, keep): (Vec<_>, Vec<_>) =
            self.undo.drain(..).partition(|(at, _)| all || *at <= step);
        self.undo = keep;
        for (_, u) in due {
            let e = self.env.pause_authority.insecure_clone();
            match u {
                Undo::MarketGates(si) => {
                    let market = Some(self.market_key(si));
                    self.send(
                        &[
                            ix::set_pause(&e.pubkey(), PauseScope::Market, false, market),
                            set_route_broken_ix(&e.pubkey(), PauseScope::Market, 0, false, market),
                        ],
                        &[&e],
                    );
                    let World {
                        env, stats, slots, ..
                    } = self;
                    slots[si].venue.halt(&mut Ctx { env, stats }, 0);
                }
                Undo::GlobalPause => {
                    self.send(
                        &[ix::set_pause(&e.pubkey(), PauseScope::Global, false, None)],
                        &[&e],
                    );
                }
                Undo::VenueRoute(si) => {
                    let bit = self.venue_bit_index(si);
                    self.send(
                        &[set_route_broken_ix(
                            &e.pubkey(),
                            PauseScope::Global,
                            bit,
                            false,
                            None,
                        )],
                        &[&e],
                    );
                }
                Undo::VenueHalt(si) => {
                    let World {
                        env, stats, slots, ..
                    } = self;
                    slots[si].venue.halt(&mut Ctx { env, stats }, 0);
                }
                Undo::VenueFee(si) => {
                    let World {
                        env, stats, slots, ..
                    } = self;
                    slots[si].venue.fee_shock(&mut Ctx { env, stats }, 0);
                }
                Undo::VenueKnobs(si) => {
                    let World {
                        env, stats, slots, ..
                    } = self;
                    slots[si].venue.restore(&mut Ctx { env, stats });
                }
                Undo::EnableVenues => self.reenable_venues(),
                Undo::Revalidate(si) => {
                    self.revalidate(si);
                }
                Undo::CoveragePause(si) => {
                    if self.market(si).has_flag(MARKET_FLAG_PAUSED_COVERAGE) {
                        self.coverage_set(si, false);
                    }
                }
                Undo::Mint(si) => self.mint_restore(si),
            }
        }
    }

    fn skip(&mut self, act: &Action) -> bool {
        *self
            .stats
            .skipped
            .entry(act.kind.name().to_string())
            .or_default() += 1;
        false
    }

    fn venue_bit_index(&self, si: usize) -> u8 {
        let m = self.market(si);
        let bit = m.venue_kind().map_or(0, VenueKind::venue_bit);
        if bit == 0 {
            0
        } else {
            bit.trailing_zeros() as u8
        }
    }

    /// The open's size from `b`: log-uniform from just below the minimum notional (0.1 SOL at
    /// the spot) to just above `min(X/20, tokens_free)` — both sides of every size bound.
    fn size(&self, si: usize, b: u32) -> u64 {
        let Some((x, y)) = self.slots[si].venue.reserves(&self.env) else {
            return 1_000_000_000_000;
        };
        let m = self.market(si);
        let min_notional = self.env.config().params.min_notional as f64;
        let min_q = (min_notional * x as f64 / y.max(1) as f64).max(1.0);
        let cap = ((x / 20).min(m.tokens_free.max(1)) as f64).max(min_q * 1.5);
        let t = -0.1 + 1.15 * f64::from(b % 10_000) / 10_000.0;
        (min_q * (cap / min_q).powf(t)).max(1.0) as u64
    }

    fn open_ix(&self, si: usize, bi: usize, q: u64) -> (Vec<Instruction>, Keypair) {
        self.open_ix_sel(si, bi, q, 0)
    }

    /// `open_short` of `q` by borrower `bi` with the extras selection `pick` names
    /// ([`Venue::extras`]).
    fn open_ix_sel(&self, si: usize, bi: usize, q: u64, pick: u32) -> (Vec<Instruction>, Keypair) {
        let slot = &self.slots[si];
        let b = &slot.borrowers[bi];
        let vm = &slot.venue.vm();
        let (sel, extras) = slot.venue.extras(&self.env, pick);
        let (min_proceeds, max_margin, max_open_fee) = open_bounds(pick);
        let ix = open_short_ix(
            vm,
            &b.key(),
            b.next_nonce,
            q,
            min_proceeds,
            max_margin,
            max_open_fee,
            sel,
            &extras,
        );
        (with_cu_limit(vm, ix), b.kp.insecure_clone())
    }

    /// Opens `q` for borrower `bi` with the gate oracle; returns the result.
    fn open_q(&mut self, si: usize, bi: usize, q: u64, pick: u32) -> TxResult {
        let gate = self.open_gate(si);
        let (ixs, kp) = self.open_ix_sel(si, bi, q, pick);
        let res = self.send(&ixs, &[&kp]);
        if res.is_ok() {
            if let Some(g) = gate {
                panic!(
                    "{ORACLE} I10/I17: open_short of {q} landed through {g}\nlogs:\n{}",
                    res.as_ref().map(|m| m.pretty_logs()).unwrap_or_default()
                );
            }
            if open_bounds(pick).0 == u64::MAX {
                panic!("{ORACLE} open_short landed with min_proceeds = u64::MAX (slippage bound)");
            }
            let slot = &mut self.slots[si];
            let nonce = slot.borrowers[bi].next_nonce;
            slot.borrowers[bi].next_nonce += 1;
            slot.positions.push(Pos { b: bi, nonce });
        } else if shortwin_code(&ixs, &res).is_none()
            && !self.slots[si].venue.disturbed(&self.env)
            && let Some((i, e)) = failure(&res)
        {
            let owner = ixs.get(i).map(|ix| ix.program_id);
            if owner == Some(shortwin::ID) {
                // Recorded by `Ctx::send` (framework error / compute / panic).
            } else {
                self.stats.anomaly(
                    "open-failed-outside-shortwin",
                    format!("instruction {i} ({owner:?}): {e:?}"),
                );
            }
        }
        res
    }

    fn open(&mut self, si: usize, a: u32, b: u32, near_cap: bool) -> bool {
        let n = self.slots[si].borrowers.len();
        let bi = a as usize % n;
        if !near_cap {
            let q = self.size(si, b);
            self.open_q(si, bi, q, b >> 16);
            return true;
        }
        // The largest q an open accepts now: bisection on clones of the VM.
        let x = self.slots[si].venue.reserves(&self.env).map_or(0, |r| r.0);
        if x == 0 {
            return false;
        }
        let saved = self.env.svm.clone();
        let accepts = |w: &mut World, q: u64| -> bool {
            w.env.svm = saved.clone();
            let (ixs, kp) = w.open_ix(si, bi, q);
            w.env.send(&ixs, &[&kp]).is_ok()
        };
        let hi0 = (x / 10).max(2);
        let lo0 = (x / 100_000).max(1);
        let q = if accepts(self, hi0) {
            hi0
        } else if !accepts(self, lo0) {
            self.env.svm = saved;
            return false;
        } else {
            let (mut lo, mut hi) = (lo0, hi0);
            while hi - lo > 1 {
                let mid = lo + (hi - lo) / 2;
                if accepts(self, mid) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            // The refusal one unit above the boundary (coverage of the cap branches).
            self.env.svm = saved.clone();
            let (ixs, kp) = self.open_ix(si, bi, hi);
            let r = self.env.send(&ixs, &[&kp]);
            let tag = shortwin_code(&ixs, &r).map_or_else(|| "other".into(), code_name_any);
            *self
                .stats
                .events
                .entry(format!("near-cap-refusal:{tag}"))
                .or_default() += 1;
            lo
        };
        self.env.svm = saved;
        self.open_q(si, bi, q, 0);
        true
    }

    fn position_key(&self, si: usize, k: usize) -> Option<(Pubkey, u64, Keypair)> {
        let p = self.slots[si].positions.get(k)?;
        let b = &self.slots[si].borrowers[p.b];
        Some((b.key(), p.nonce, b.kp.insecure_clone()))
    }

    fn close(&mut self, si: usize, a: u32) -> bool {
        let n = self.slots[si].positions.len();
        if n == 0 {
            return false;
        }
        let (borrower, nonce, kp) = self.position_key(si, a as usize % n).expect("pos");
        let vm = &self.slots[si].venue.vm();
        let (sel, extras) = self.slots[si].venue.extras(&self.env, a >> 16);
        // One close in thirty-two bounds the buyback at one lamport (`BuybackAboveMax`).
        let tight = (a >> 8) % 32 == 31;
        let max_cost = if tight { 1 } else { u64::MAX };
        let ixs = with_cu_limit(
            vm,
            close_short_ix(vm, &borrower, nonce, max_cost, sel, &extras),
        );
        let res = self.send(&ixs, &[&kp]);
        self.check_exit_refusal("close_short", &ixs, &res);
        if tight && res.is_ok() {
            panic!("{ORACLE} close_short landed with max_buyback_cost = 1 (slippage bound)");
        }
        true
    }

    fn add_margin(&mut self, si: usize, a: u32, b: u32) -> bool {
        let n = self.slots[si].positions.len();
        if n == 0 {
            return false;
        }
        let (borrower, nonce, kp) = self.position_key(si, a as usize % n).expect("pos");
        let amount = 1 + u64::from(b % 1_000_000_000);
        let ix = add_margin_ix(&self.slots[si].venue.vm(), &borrower, nonce, amount);
        let res = self.send(std::slice::from_ref(&ix), &[&kp]);
        self.check_exit_refusal("add_margin", std::slice::from_ref(&ix), &res);
        if let Err(f) = &res {
            // ARCH §3.5: no gate refuses a top-up.
            panic!(
                "{ORACLE} add_margin of {amount} refused: {:?}\nlogs:\n{}",
                f.err,
                f.meta.pretty_logs()
            );
        }
        true
    }

    // ---- lenders -----------------------------------------------------------------------------------

    fn deposit(&mut self, si: usize, a: u32, b: u32) -> bool {
        let li = a as usize % self.slots[si].lenders.len();
        let k = self.lender_keys(si, li);
        let have = read::token_amount(&self.env.svm, &k.owner_token_account).unwrap_or(0);
        let x = self.slots[si]
            .venue
            .reserves(&self.env)
            .map_or(10u64.pow(12), |r| r.0);
        let want = (u128::from(x) * u128::from(1 + b % 1_000) / 20_000) as u64;
        let amount = if b.is_multiple_of(101) {
            0
        } else if b.is_multiple_of(7) {
            1 + u64::from(b % 1_000)
        } else {
            want.max(1)
        };
        if have < amount {
            let World { env, slots, .. } = self;
            slots[si]
                .venue
                .top_up(env, &k.owner_token_account, amount - have);
            self.env.assert_invariants();
        }
        let ix = ix::with_remaining(ix::deposit(&k, amount, 0), &self.epoch_metas(si, &k.owner));
        let kp = self.slots[si].lenders[li].kp.insecure_clone();
        let res = self.send(std::slice::from_ref(&ix), &[&kp]);
        if amount == 0 && res.is_ok() {
            panic!("{ORACLE} a deposit of 0 tokens landed (ZeroAmount)");
        }
        true
    }

    /// The shares whose tokens are exactly the lender's pro-rata allowance now
    /// (`pool::allowance` on the state the program sees, then `shares_for_tokens`), or `None`
    /// for a lender behind the current share epoch.
    fn allowance_shares(&self, si: usize, owner: &Pubkey) -> Option<u128> {
        let market = self.market_key(si);
        let m = self.env.market(&market);
        let lp = self.env.lender(&market, owner)?;
        let pool = m.pool();
        let vault = *self.env.market_vault(&market).vault();
        let lender = lp.lender();
        if lender.epoch != pool.share_epoch {
            return None;
        }
        let allow = shortwin_math::pool::allowance(&pool, &vault, &lender, self.env.now()).ok()?;
        let shares = shortwin_math::pool::shares_for_tokens(&pool, allow).ok()?;
        Some(shares.min(lp.shares))
    }

    /// The lender's SOL balance after a touch now (`pool::touch` on the state the program sees),
    /// or `None` for a lender behind the share epoch.
    fn touched_pending(&self, si: usize, owner: &Pubkey) -> Option<i64> {
        let market = self.market_key(si);
        let m = self.env.market(&market);
        let lp = self.env.lender(&market, owner)?;
        let pool = m.pool();
        let mut vault = *self.env.market_vault(&market).vault();
        let mut lender = lp.lender();
        if lender.epoch != pool.share_epoch {
            return None;
        }
        shortwin_math::pool::touch(&pool, &mut vault, &mut lender, &[], self.env.now()).ok()?;
        Some(lender.pending)
    }

    /// A lender whose SOL balance is negative after a shortfall debit (ECON §9.4) covers
    /// exactly the deficit (`cover_deficit`), as it must before it can withdraw.
    ///
    /// The deficit is the lender's pro-rata share of the shortfall at its current shares (a
    /// lender that deposited after the rent was credited bears part of it, ECON §9.4), so it is
    /// bounded by neither the lender's deposit nor its income: one that deposited ≈ 59 % of the
    /// shares while a near-cap position carried ≈ 3,640 SOL of receivable owed ≈ 1,044 SOL once
    /// that position exited short. Every lender starts with `FUNDING` (1,000 SOL), so the System
    /// transfer inside `cover_deficit` failed (insufficient funds), the withdraw then refused
    /// `NegativePending` and the wind-down reported `wind-down-withdraw-refused` (FUZZ-M7-24H-3:
    /// the 24 h run at eb82a2a, amm-v4 seed 20333958). The lender is first topped up to the
    /// deficit plus 1 SOL for fees, as a real lender brings the SOL (`wind-down-deficit-funded`).
    fn cover_negative(&mut self, si: usize, li: usize) {
        let owner = self.slots[si].lenders[li].kp.insecure_clone();
        let Some(p) = self.touched_pending(si, &owner.pubkey()).filter(|p| *p < 0) else {
            return;
        };
        let need = p.unsigned_abs().saturating_add(1_000_000_000);
        let have = self.env.svm.get_balance(&owner.pubkey()).unwrap_or(0);
        if have < need {
            crate::setup::refill_faucet(&mut self.env);
            self.env.fund(&owner.pubkey(), need - have);
            *self
                .stats
                .events
                .entry("wind-down-deficit-funded".into())
                .or_default() += 1;
        }
        let market = self.market_key(si);
        let ix = ix::with_remaining(
            ix::cover_deficit(&owner.pubkey(), &market, p.unsigned_abs()),
            &self.epoch_metas(si, &owner.pubkey()),
        );
        // Counted only when it lands: a cover that fails leaves the withdraw's NegativePending to
        // the wind-down's `wind-down-withdraw-refused` report.
        let landed = self.send(&[ix], &[&owner]).is_ok();
        let event = if landed {
            "wind-down-deficit-covered"
        } else {
            "wind-down-deficit-cover-failed"
        };
        *self.stats.events.entry(event.into()).or_default() += 1;
    }

    fn withdraw(&mut self, si: usize, a: u32, b: u32) -> bool {
        let li = a as usize % self.slots[si].lenders.len();
        let k = self.lender_keys(si, li);
        let market = self.market_key(si);
        let Some(l) = self.env.lender(&market, &k.owner) else {
            return false;
        };
        let per_mille = u128::from(b % 1_100);
        let shares = if per_mille >= 1_050 {
            // Exactly the pro-rata allowance (the boundary of `AboveProRataAllowance`).
            match self.allowance_shares(si, &k.owner) {
                Some(s) if s > 0 => s,
                _ => return false,
            }
        } else if per_mille >= 1_000 {
            l.shares
        } else {
            (l.shares * per_mille / 1_000).max(1)
        };
        let ix = ix::with_remaining(ix::withdraw(&k, shares, 0), &self.epoch_metas(si, &k.owner));
        let kp = self.slots[si].lenders[li].kp.insecure_clone();
        let res = self.send(std::slice::from_ref(&ix), &[&kp]);
        if let Some(c) = shortwin_code(std::slice::from_ref(&ix), &res)
            && MINT_FILTER.iter().any(|e| code(*e) == c)
        {
            panic!(
                "{ORACLE} I10: withdraw refused by the mint filter ({}): an exit path never \
                 reverts on it (ARCH §3.0 step 3)",
                code_name(c)
            );
        }
        true
    }

    fn claim(&mut self, si: usize, a: u32, b: u32) -> bool {
        let li = a as usize % self.slots[si].lenders.len();
        let owner = self.slots[si].lenders[li].kp.insecure_clone();
        let market = self.market_key(si);
        if self.env.lender(&market, &owner.pubkey()).is_none() {
            return false;
        }
        for _ in 0..=(b % 4) {
            let ix = ix::with_remaining(
                ix::claim_sol(&owner.pubkey(), &market),
                &self.epoch_metas(si, &owner.pubkey()),
            );
            self.send(&[ix], &[&owner]);
        }
        true
    }

    fn cover_deficit(&mut self, si: usize, a: u32, b: u32) -> bool {
        let li = a as usize % self.slots[si].lenders.len();
        let owner = self.slots[si].lenders[li].kp.insecure_clone();
        let market = self.market_key(si);
        if self.env.lender(&market, &owner.pubkey()).is_none() {
            return false;
        }
        let ix = ix::with_remaining(
            ix::cover_deficit(&owner.pubkey(), &market, 1 + u64::from(b % 100_000_000)),
            &self.epoch_metas(si, &owner.pubkey()),
        );
        self.send(&[ix], &[&owner]);
        true
    }

    fn sync_lender(&mut self, si: usize, a: u32) -> bool {
        let li = a as usize % self.slots[si].lenders.len();
        let owner = self.slots[si].lenders[li].kp.pubkey();
        let market = self.market_key(si);
        if self.env.lender(&market, &owner).is_none() {
            return false;
        }
        let metas = self.epoch_metas(si, &owner);
        let ix = ix::with_remaining(ix::sync_lender(&owner, &market, metas.len() as u8), &metas);
        self.send(&[ix], &[]);
        true
    }

    fn dust_sweeps(&mut self, si: usize, b: u32) -> bool {
        let trader = pda::trader(&self.market_key(si)).0;
        let donor = self.donor.insecure_clone();
        for _ in 0..=(b % 5) {
            self.send(&[transfer_ix(&donor.pubkey(), &trader, 1)], &[&donor]);
            let ix = self.slots[si].venue.sweep_ix(&self.env);
            self.send(&[ix], &[]);
            self.env.warp(60);
            self.crank(si);
        }
        true
    }

    /// Every lender withdraws all its shares and claims (`keep_dust`: the first keeps one
    /// share), then a `sweep_trader`. `strict` (the wind-down): refusals are reported.
    pub fn lenders_out(&mut self, si: usize, keep_dust: bool, strict: bool) -> bool {
        let market = self.market_key(si);
        for li in 0..self.slots[si].lenders.len() {
            let k = self.lender_keys(si, li);
            let kp = self.slots[si].lenders[li].kp.insecure_clone();
            // A lender behind the market's share epoch (a full wipe) is synced first: its
            // shares are then the current epoch's (none, if the wipe took them all).
            for _ in 0..8 {
                match self.env.lender(&market, &k.owner) {
                    Some(l) if l.epoch < self.env.market(&market).share_epoch => {
                        self.sync_lender(si, li as u32);
                    }
                    _ => break,
                }
            }
            self.cover_negative(si, li);
            if let Some(l) = self.env.lender(&market, &k.owner) {
                let shares = if keep_dust && li == 0 {
                    l.shares.saturating_sub(1)
                } else {
                    l.shares
                };
                if shares > 0 {
                    let ix = ix::with_remaining(
                        ix::withdraw(&k, shares, 0),
                        &self.epoch_metas(si, &k.owner),
                    );
                    let mut res = self.send(&[ix], &[&kp]);
                    // All shares may be worth more than the allowance or `T` once losses were
                    // paid in SOL (the virtual shares' part, ECON §9.2 / §9.5): the lender then
                    // takes exactly its allowance, and the remainder is the rounding and
                    // rebase-orphan dust the hooks' liveness check bounds.
                    if res.is_err()
                        && let Some(s) = self.allowance_shares(si, &k.owner).filter(|s| *s > 0)
                    {
                        let ix = ix::with_remaining(
                            ix::withdraw(&k, s, 0),
                            &self.epoch_metas(si, &k.owner),
                        );
                        res = self.send(&[ix], &[&kp]);
                    }
                    // Shares worth at most one raw token at the vault's rate are rounding
                    // dust (a post-wipe epoch with `T` ≈ 1): counted, not reported.
                    let m = self.env.market(&market);
                    let worth = shares * u128::from(m.tokens_free) / m.total_shares.get().max(1);
                    if strict && res.is_err() && worth <= 1 {
                        *self
                            .stats
                            .events
                            .entry("wind-down-dust-lender".into())
                            .or_default() += 1;
                    } else if strict && let Err(f) = &res {
                        let lp = self.env.lender(&market, &k.owner);
                        let detail = format!(
                            "{:?}\nlender {li}: {lp:?}\nmarket: total_shares {} tokens_free {} \
                                 borrowed_q {} open {} share_epoch {} venue {:?} now {}\n{}",
                            f.err,
                            m.total_shares.get(),
                            m.tokens_free,
                            m.borrowed_q,
                            m.open_positions,
                            m.share_epoch,
                            m.venue_kind(),
                            self.env.now(),
                            f.meta.pretty_logs()
                        );
                        self.stats.anomaly("wind-down-withdraw-refused", detail);
                    }
                }
                if self.env.lender(&market, &k.owner).is_some() {
                    let ix = ix::with_remaining(
                        ix::claim_sol(&k.owner, &market),
                        &self.epoch_metas(si, &k.owner),
                    );
                    self.send(&[ix], &[&kp]);
                }
            }
        }
        let ix = self.slots[si].venue.sweep_ix(&self.env);
        self.send(&[ix], &[]);
        true
    }

    // ---- time ------------------------------------------------------------------------------------

    fn warp_to_deadline(&mut self, si: usize, a: u32, b: u32) -> bool {
        let n = self.slots[si].positions.len();
        if n == 0 {
            return false;
        }
        let (borrower, nonce, _) = self.position_key(si, a as usize % n).expect("pos");
        let pos = self
            .env
            .position(&self.slots[si].venue.vm(), &borrower, nonce);
        let s3 = i64::from(self.env.config().params.s3_wait_s);
        let target = match b % 4 {
            0 => pos.deadline - 3,
            1 => pos.deadline,
            2 => pos.deadline + i64::from(b % 8),
            _ => pos.deadline + s3 + 2,
        };
        let now = self.env.now();
        if target > now {
            self.env.warp(target - now);
        }
        self.keeper_seconds(1 + b % 6);
        true
    }

    // ---- prices and liquidity ------------------------------------------------------------------

    fn send_move(&mut self, si: usize, mv: &Move, back: bool) -> TxResult {
        let ix = if back {
            mv.back.clone().expect("back")
        } else {
            mv.front.clone()
        };
        let signer = mv.signer.insecure_clone();
        self.send_bundle(si, &[ix], &[&signer]).0
    }

    fn trade(&mut self, si: usize, a: u32, b: u32) -> bool {
        let Some((x, y)) = self.slots[si].venue.reserves(&self.env) else {
            return false;
        };
        let buy = a.is_multiple_of(2);
        let frac = 1 + u64::from(b % 500);
        let size = if buy {
            (u128::from(y) * u128::from(frac) / 10_000) as u64
        } else {
            (u128::from(x) * u128::from(frac) / 10_000) as u64
        };
        let World { env, slots, .. } = self;
        let Some(mv) = slots[si].venue.trade(env, buy, size.max(1)) else {
            return false;
        };
        self.send_move(si, &mv, false);
        true
    }

    fn push(&mut self, si: usize, up: bool, k: u64) -> Option<Move> {
        let World { env, slots, .. } = self;
        slots[si].venue.push(env, up, k)
    }

    fn shock(&mut self, si: usize, a: u32, b: u32) -> bool {
        let up = a.is_multiple_of(2);
        let Some(mv) = self.push(si, up, k_ppm(b)) else {
            return false;
        };
        self.env.warp(1);
        self.send_move(si, &mv, false);
        self.keeper_seconds(a % 30);
        true
    }

    fn ramp(&mut self, si: usize, a: u32, b: u32) -> bool {
        let secs = 2 + a % 60;
        let up = b.is_multiple_of(2);
        let k = (k_ppm(b) as f64 / 1e6).powf(1.0 / f64::from(secs));
        for _ in 0..secs {
            if let Some(mv) = self.push(si, up, (k * 1e6) as u64) {
                self.env.warp(1);
                self.send_move(si, &mv, false);
            } else {
                self.env.warp(1);
            }
            for s in 0..self.slots.len() {
                self.crank(s);
                self.keeper_sweep(s);
            }
        }
        true
    }

    fn lp_pair(&mut self, si: usize, b: u32) -> Option<(Instruction, Instruction, Keypair)> {
        let World { env, slots, .. } = self;
        let r = slots[si].venue.lp_pair(env, 1 + b % 5_000);
        self.env.assert_invariants();
        r
    }

    fn liquidity(&mut self, si: usize, a: u32, b: u32) -> bool {
        let Some((remove, add, kp)) = self.lp_pair(si, b) else {
            return false;
        };
        let ix = if a.is_multiple_of(2) { add } else { remove };
        self.send_bundle(si, &[ix], &[&kp]);
        true
    }

    fn lp_wrapped_crank(&mut self, si: usize, a: u32, b: u32) -> bool {
        let Some((remove, add, kp)) = self.lp_pair(si, b) else {
            return false;
        };
        let crank = crank_price_ix(&self.slots[si].venue.vm());
        let ixs = if a.is_multiple_of(2) {
            vec![remove, crank, add]
        } else {
            vec![add, crank, remove]
        };
        self.env.warp(1);
        self.send_bundle(si, &ixs, &[&kp]);
        true
    }

    fn deposit_wrapped_open(&mut self, si: usize, a: u32, b: u32) -> bool {
        let Some((remove, add, kp)) = self.lp_pair(si, b) else {
            return false;
        };
        let bi = a as usize % self.slots[si].borrowers.len();
        let q = self.size(si, b / 3);
        let gate = self.open_gate(si);
        let (open, bk) = self.open_ix(si, bi, q);
        let mut ixs = vec![add];
        ixs.extend(open.into_iter().filter(|ix| ix.program_id == shortwin::ID));
        ixs.push(remove);
        let (res, _) = self.send_bundle(si, &ixs, &[&kp, &bk]);
        if res.is_ok() {
            if let Some(g) = gate {
                panic!("{ORACLE} I10/I17: a deposit-wrapped open landed through {g}");
            }
            let slot = &mut self.slots[si];
            let nonce = slot.borrowers[bi].next_nonce;
            slot.borrowers[bi].next_nonce += 1;
            slot.positions.push(Pos { b: bi, nonce });
        }
        true
    }

    /// M7-SEC-6: an open wrapped in a price move of the same transaction — a sale (`a` odd) or a
    /// buy of the market's venue to `1 + (b mod open_ema_dev)` bps from the spot, within the
    /// open's spot–EMA guard at most — then `open_short`, then (`a / 2` odd) the move unwound
    /// (M5-SEC-3, M7-SEC-4, M7-SEC-5: every term the open computes must hold against the move).
    /// The gate oracle as for any open; the open's terms are the I19 hook's (every open).
    fn wrapped_open(&mut self, si: usize, a: u32, b: u32) -> bool {
        let dev = self.env.config().params.open_ema_dev_bps.max(1);
        let k = 1_000_000 + u64::from(b % dev) * 100;
        let Some(mv) = self.push(si, a.is_multiple_of(2), k) else {
            return false;
        };
        let bi = (a as usize / 4) % self.slots[si].borrowers.len();
        let q = self.size(si, b / 7);
        let (open, bk) = self.open_ix(si, bi, q);
        let mut ixs = vec![mv.front.clone()];
        let before = self.env.svm.clone();
        // The gate as the open will see it: after the move (a move changes no gate but the
        // venue's executability, which the harness view reads).
        let gate = {
            let r = self
                .env
                .send(std::slice::from_ref(&mv.front), &[&mv.signer]);
            let g = r.is_ok().then(|| self.open_gate(si));
            self.env.svm = before;
            match g {
                Some(g) => g,
                None => return false,
            }
        };
        ixs.extend(open.into_iter().filter(|ix| ix.program_id == shortwin::ID));
        let unwind = (a / 2) % 2 == 1;
        if unwind && let Some(back) = mv.back.clone() {
            ixs.push(back);
        }
        let cu = self.slots[si].venue.exit_cu().map(|_| 1_400_000);
        let (res, _) = Self::with_cu(cu, &ixs, |all| self.cx().send(all, &[&mv.signer, &bk]));
        if res.is_ok() {
            if let Some(g) = gate {
                panic!("{ORACLE} I10/I17: a price-wrapped open landed through {g}");
            }
            let slot = &mut self.slots[si];
            let nonce = slot.borrowers[bi].next_nonce;
            slot.borrowers[bi].next_nonce += 1;
            slot.positions.push(Pos { b: bi, nonce });
            *self
                .stats
                .events
                .entry("wrapped-open-landed".into())
                .or_default() += 1;
        }
        true
    }

    /// M7-SEC-3: the `coverage_authority` key brackets an open in one transaction —
    /// `[set_coverage_pause(false), open_short, set_coverage_pause(true)]`. On a market whose flag
    /// is set the clear starts the clear delay, so the open must be refused `CoveragePaused` (the
    /// whole transaction fails); on one whose flag is clear the clear is a no-op and the open
    /// meets its ordinary gates. `a` odd first sets the flag (the keeper's degraded mode).
    fn coverage_bracket(&mut self, si: usize, a: u32, b: u32) -> bool {
        let Some(kp) = self.coverage.as_ref().map(Keypair::insecure_clone) else {
            return false;
        };
        if self.env.config().coverage_authority != kp.pubkey() {
            return false;
        }
        if a % 2 == 1 {
            self.coverage_set(si, true);
            self.later(b % 8, Undo::CoveragePause(si));
        }
        let market = self.market_key(si);
        let was_set = self.market(si).has_flag(MARKET_FLAG_PAUSED_COVERAGE);
        // A gate the open meets before the coverage pause (opens paused, flags, the latch, …): the
        // bracket is refused for it then.
        let other_gate = self.open_gate_with(si, false);
        let gate = if was_set {
            Some("the coverage clear delay of the bracket's own clear (M7-SEC-3)".to_string())
        } else {
            self.open_gate(si)
        };
        let bi = (a as usize / 2) % self.slots[si].borrowers.len();
        let q = self.size(si, b / 5);
        let (open, bk) = self.open_ix(si, bi, q);
        let mut ixs = vec![Self::coverage_ix(
            &kp.pubkey(),
            &market,
            false,
            COVERAGE_REASON_KEEPER,
        )];
        ixs.extend(open.into_iter().filter(|ix| ix.program_id == shortwin::ID));
        ixs.push(Self::coverage_ix(
            &kp.pubkey(),
            &market,
            true,
            COVERAGE_REASON_KEEPER,
        ));
        let cu = self.slots[si].venue.exit_cu().map(|_| 1_400_000);
        let (res, all) = Self::with_cu(cu, &ixs, |all| self.cx().send(all, &[&kp, &bk]));
        match (&res, gate) {
            (Ok(_), Some(g)) => panic!("{ORACLE} M7-SEC-3: a bracketed open landed through {g}"),
            (Ok(_), None) => {
                let slot = &mut self.slots[si];
                let nonce = slot.borrowers[bi].next_nonce;
                slot.borrowers[bi].next_nonce += 1;
                slot.positions.push(Pos { b: bi, nonce });
                // The bracket's own `set` left the flag set: the keeper clears it later.
                self.later(b % 8, Undo::CoveragePause(si));
            }
            (Err(_), _) if was_set => {
                let got = shortwin_code(&all, &res);
                if got != Some(code(E::CoveragePaused)) && other_gate.is_none() {
                    panic!(
                        "{ORACLE} M7-SEC-3: a bracketed open on a coverage-paused market was \
                         refused with {got:?}, not CoveragePaused"
                    );
                }
                *self
                    .stats
                    .events
                    .entry("coverage-bracket-refused".into())
                    .or_default() += 1;
            }
            (Err(_), _) => {}
        }
        true
    }

    /// ★OWNER-M7-3 (b) (owner decision of 2026-10-01): the Emergency vault clears a market's
    /// coverage pause — never sets one ([`Kind::EmergencyClear`]).
    fn emergency_clear(&mut self, si: usize, a: u32, b: u32) -> bool {
        let installed = self
            .coverage
            .as_ref()
            .is_some_and(|k| self.env.config().coverage_authority == k.pubkey());
        if a % 2 == 1 && installed && self.coverage_set(si, true) {
            // A (hijacked) key's pause, which the keeper would otherwise clear later.
            self.later(b % 8, Undo::CoveragePause(si));
        }
        let ea = self.env.pause_authority.insecure_clone();
        let role_unset = self.env.config().coverage_authority == Pubkey::default();
        match b % 4 {
            1 => {
                let expect = if role_unset {
                    E::CoverageAuthorityDisabled
                } else {
                    E::Unauthorized
                };
                self.coverage_send(si, &ea, true, COVERAGE_REASON_KEEPER, Err(expect));
            }
            2 => self.emergency_open(si, a, b, true),
            _ => {
                let reason = if (b / 4) % 8 == 7 {
                    [0u8, 2, 255][(b / 32) as usize % 3]
                } else {
                    COVERAGE_REASON_KEEPER
                };
                let was_set = self.market(si).has_flag(MARKET_FLAG_PAUSED_COVERAGE);
                let expect = if reason == COVERAGE_REASON_KEEPER {
                    Ok(())
                } else {
                    Err(E::InvalidParams)
                };
                let landed = expect.is_ok();
                self.coverage_send(si, &ea, false, reason, expect);
                if landed {
                    *self
                        .stats
                        .events
                        .entry(if was_set {
                            "emergency-clear".into()
                        } else {
                            "emergency-clear-noop".into()
                        })
                        .or_default() += 1;
                }
                if landed && was_set {
                    // The clear delay: no open lands in this second.
                    self.emergency_open(si, a, b, false);
                }
            }
        }
        true
    }

    /// An open on slot `si` — with `[Emergency clear, open_short]` in one transaction when
    /// `bundled` (★OWNER-M7-3 (b), M7-SEC-3's bracket with the Emergency's clear), or alone right
    /// after an Emergency clear of a set flag. Either way a clear of a set flag starts the clear
    /// delay, so the open must be refused `CoveragePaused` (unless a gate the open meets first
    /// refuses it); on a flag that was clear the bundled clear is a no-op and the open meets its
    /// ordinary gates.
    fn emergency_open(&mut self, si: usize, a: u32, b: u32, bundled: bool) {
        let ea = self.env.pause_authority.insecure_clone();
        let market = self.market_key(si);
        let m = self.market(si);
        let was_set = m.has_flag(MARKET_FLAG_PAUSED_COVERAGE);
        let in_delay = !was_set
            && self.env.now() < m.coverage_cleared_at.saturating_add(COVERAGE_CLEAR_DELAY_S);
        let expect_delay = (bundled && was_set) || (!bundled && in_delay);
        let other_gate = self.open_gate_with(si, false);
        let gate = if bundled && was_set {
            Some(
                "the clear delay of the bundled Emergency clear (OWNER-M7-3, M7-SEC-3)".to_string(),
            )
        } else {
            self.open_gate(si)
        };
        let bi = (a as usize / 2) % self.slots[si].borrowers.len();
        let q = self.size(si, b / 5);
        let (open, bk) = self.open_ix(si, bi, q);
        let mut ixs = Vec::new();
        if bundled {
            ixs.push(Self::coverage_ix(
                &ea.pubkey(),
                &market,
                false,
                COVERAGE_REASON_KEEPER,
            ));
        }
        ixs.extend(open.into_iter().filter(|ix| ix.program_id == shortwin::ID));
        let cu = self.slots[si].venue.exit_cu().map(|_| 1_400_000);
        let (res, all) = Self::with_cu(cu, &ixs, |all| self.cx().send(all, &[&ea, &bk]));
        match (&res, gate) {
            (Ok(_), Some(g)) => {
                panic!(
                    "{ORACLE} OWNER-M7-3: an open after the Emergency's clear landed through {g}"
                )
            }
            (Ok(_), None) => {
                let slot = &mut self.slots[si];
                let nonce = slot.borrowers[bi].next_nonce;
                slot.borrowers[bi].next_nonce += 1;
                slot.positions.push(Pos { b: bi, nonce });
            }
            (Err(_), _) if expect_delay => {
                let got = shortwin_code(&all, &res);
                if got != Some(code(E::CoveragePaused)) && other_gate.is_none() {
                    panic!(
                        "{ORACLE} OWNER-M7-3: an open in the clear delay of the Emergency's clear \
                         was refused with {got:?}, not CoveragePaused"
                    );
                }
                *self
                    .stats
                    .events
                    .entry("emergency-clear-open-refused".into())
                    .or_default() += 1;
            }
            (Err(_), _) => {}
        }
    }

    fn exit_ix(&self, si: usize, k: usize, which: u32) -> Option<(Instruction, Option<Keypair>)> {
        let (borrower, nonce, bkp) = self.position_key(si, k)?;
        let vm = &self.slots[si].venue.vm();
        let keeper = self.keeper.pubkey();
        Some(match which % 4 {
            0 => (
                close_short_ix(vm, &borrower, nonce, u64::MAX, ExtrasSel::NONE, &[]),
                Some(bkp),
            ),
            1 => (liquidate_ix(vm, &keeper, &borrower, nonce), None),
            2 => (expire_ix(vm, &keeper, &borrower, nonce), None),
            _ => (
                sol_settle_ix(&self.env, vm, &keeper, &borrower, nonce),
                None,
            ),
        })
    }

    fn withdraw_wrapped_exit(&mut self, si: usize, a: u32, b: u32) -> bool {
        let n = self.slots[si].positions.len();
        if n == 0 {
            return false;
        }
        let Some((remove, add, kp)) = self.lp_pair(si, b) else {
            return false;
        };
        let (exit, _) = self.exit_ix(si, a as usize % n, 1).expect("pos");
        let keeper = self.keeper.insecure_clone();
        let ixs = vec![remove, exit, add];
        let (res, sent) = self.send_bundle(si, &ixs, &[&kp, &keeper]);
        self.check_exit_refusal("withdraw-wrapped liquidate", &sent, &res);
        true
    }

    // ---- bundles ---------------------------------------------------------------------------------

    fn pump_crank_dump(&mut self, si: usize, a: u32, b: u32) -> bool {
        let mut any = false;
        for _ in 0..=(a % 20) {
            self.env.warp(1);
            let Some(mv) = self.push(si, true, k_ppm(b)) else {
                break;
            };
            let Some(back) = mv.back.clone() else {
                break;
            };
            let crank = crank_price_ix(&self.slots[si].venue.vm());
            let signer = mv.signer.insecure_clone();
            self.send_bundle(si, &[mv.front.clone(), crank, back], &[&signer]);
            any = true;
        }
        any
    }

    fn settle_all(&mut self, si: usize) {
        let mut k = 0;
        while k < self.slots[si].positions.len() {
            if !self.keeper_exit(si, k, 2) {
                k += 1;
            }
        }
    }

    fn contrary_low(&mut self, si: usize, a: u32, b: u32) -> bool {
        let mut any = false;
        for _ in 0..=(a % 20) {
            self.env.warp(1);
            if let Some(mv) = self.push(si, false, k_ppm(b))
                && let Some(back) = mv.back.clone()
            {
                let crank = crank_price_ix(&self.slots[si].venue.vm());
                let signer = mv.signer.insecure_clone();
                self.send_bundle(si, &[mv.front.clone(), crank, back], &[&signer]);
                any = true;
            }
            if b.is_multiple_of(2) {
                // The keeper's own observation in the same second (the contrary one widens).
                self.crank(si);
            }
            self.settle_all(si);
        }
        any
    }

    fn attacker_seconds(&mut self, si: usize, a: u32, b: u32) -> bool {
        let n = 1 + a % 6;
        let mode = (a / 6) % 3;
        for i in 0..n {
            self.env.warp(1);
            let up = match mode {
                0 => true,
                1 => false,
                _ => i % 2 == 0,
            };
            if let Some(mv) = self.push(si, up, k_ppm(b))
                && let Some(back) = mv.back.clone()
            {
                let crank = crank_price_ix(&self.slots[si].venue.vm());
                let signer = mv.signer.insecure_clone();
                self.send_bundle(si, &[mv.front.clone(), crank, back], &[&signer]);
            }
        }
        // The keeper arrives: a settlement and a liquidation attempt on every position.
        self.env.warp(1);
        self.crank(si);
        self.settle_all(si);
        self.keeper_sweep(si);
        true
    }

    fn push_then_freeze(&mut self, si: usize, a: u32, b: u32) -> bool {
        let up = b.is_multiple_of(2);
        let Some(mv) = self.push(si, up, k_ppm(b)) else {
            return false;
        };
        self.env.warp(1);
        self.send_move(si, &mv, false);
        for _ in 0..=(a % 30) {
            self.env.warp(1);
            if let Some(opp) = self.push(si, !up, k_ppm(b / 7))
                && let Some(back) = opp.back.clone()
            {
                let crank = crank_price_ix(&self.slots[si].venue.vm());
                let signer = opp.signer.insecure_clone();
                self.send_bundle(si, &[opp.front.clone(), crank, back], &[&signer]);
            }
            self.crank(si);
            self.keeper_sweep(si);
        }
        true
    }

    fn sandwich_exit(&mut self, si: usize, a: u32, b: u32, held: bool) -> bool {
        let n = self.slots[si].positions.len();
        if n == 0 {
            return false;
        }
        let k = a as usize % n;
        let which = b % 4;
        let up = (b / 4).is_multiple_of(2);
        let Some(mv) = self.push(si, up, k_ppm(b / 8)) else {
            return false;
        };
        let (exit, bkp) = self.exit_ix(si, k, which).expect("pos");
        let keeper = self.keeper.insecure_clone();
        let signer = mv.signer.insecure_clone();
        let mut signers: Vec<&Keypair> = vec![&signer, &keeper];
        if let Some(bk) = bkp.as_ref() {
            signers.push(bk);
        }
        let what = ["close_short", "liquidate", "expire", "sol_settle"][which as usize];
        if held {
            self.send_bundle(si, std::slice::from_ref(&mv.front), &signers);
            let (res, sent) = self.send_bundle(si, std::slice::from_ref(&exit), &signers);
            self.check_exit_refusal(what, &sent, &res);
            if let Some(back) = mv.back.clone() {
                self.send_bundle(si, &[back], &signers);
            }
        } else {
            let mut ixs = vec![mv.front.clone(), exit];
            if let Some(back) = mv.back.clone() {
                ixs.push(back);
            }
            let (res, sent) = self.send_bundle(si, &ixs, &signers);
            self.check_exit_refusal(what, &sent, &res);
        }
        true
    }

    fn dump_around_settle(&mut self, si: usize, a: u32, b: u32) -> bool {
        let n = self.slots[si].positions.len();
        if n == 0 {
            return false;
        }
        let (settle, _) = self.exit_ix(si, a as usize % n, 3).expect("pos");
        let keeper = self.keeper.insecure_clone();
        let (front, back, signer) = if b.is_multiple_of(2) {
            let Some(mv) = self.push(si, false, k_ppm(b)) else {
                return false;
            };
            (mv.front, mv.back, mv.signer)
        } else {
            let Some((remove, add, kp)) = self.lp_pair(si, b) else {
                return false;
            };
            (add, Some(remove), kp)
        };
        let mut ixs = vec![front, settle];
        ixs.extend(back);
        let (res, sent) = self.send_bundle(si, &ixs, &[&signer, &keeper]);
        self.check_exit_refusal("sol_settle", &sent, &res);
        true
    }

    fn jit_deposit(&mut self, si: usize, a: u32, b: u32) -> bool {
        let n = self.slots[si].positions.len();
        if n == 0 {
            return false;
        }
        let li = b as usize % self.slots[si].lenders.len();
        let k = self.lender_keys(si, li);
        let x = self.slots[si]
            .venue
            .reserves(&self.env)
            .map_or(10u64.pow(12), |r| r.0);
        let amount = (x / 50).max(1);
        let have = read::token_amount(&self.env.svm, &k.owner_token_account).unwrap_or(0);
        if have < amount {
            let World { env, slots, .. } = self;
            slots[si]
                .venue
                .top_up(env, &k.owner_token_account, amount - have);
            self.env.assert_invariants();
        }
        let (exit, bkp) = self
            .exit_ix(si, a as usize % n, 1 + (b / 3) % 3)
            .expect("pos");
        let epochs = self.epoch_metas(si, &k.owner);
        let dep = ix::with_remaining(ix::deposit(&k, amount, 0), &epochs);
        // The withdrawal of what the deposit mints (all shares; the lock and the pro-rata
        // allowance decide what leaves).
        let market = self.market_key(si);
        let shares =
            self.env.lender(&market, &k.owner).map_or(0, |l| l.shares) + u128::from(amount);
        let wd = ix::with_remaining(ix::withdraw(&k, shares, 0), &epochs);
        let lender = self.slots[si].lenders[li].kp.insecure_clone();
        let keeper = self.keeper.insecure_clone();
        let mut signers: Vec<&Keypair> = vec![&lender, &keeper];
        if let Some(bk) = bkp.as_ref() {
            signers.push(bk);
        }
        self.send_bundle(si, &[dep, exit, wd], &signers);
        true
    }

    // ---- outages and the fast path ------------------------------------------------------------

    fn venue_ctx<R>(&mut self, si: usize, f: impl FnOnce(&mut dyn Venue, &mut Ctx) -> R) -> R {
        let World {
            env, stats, slots, ..
        } = self;
        f(slots[si].venue.as_mut(), &mut Ctx { env, stats })
    }

    /// Keeper cadence for `secs` seconds, one visit every `step` seconds: cranks, settlements
    /// of every position, and every fifth visit the other exits.
    fn keeper_cadence(&mut self, secs: u32, step: u32) {
        let mut t = 0;
        let mut visit = 0;
        while t < secs {
            self.env.warp(i64::from(step));
            t += step;
            visit += 1;
            for s in 0..self.slots.len() {
                self.crank(s);
                self.settle_all(s);
                if visit % 5 == 0 {
                    self.keeper_sweep(s);
                }
            }
        }
    }

    fn outage(&mut self, si: usize, a: u32, b: u32) -> bool {
        let secs = [130, 400, 700, 3_700, 7_300][a as usize % 5];
        // At most ~240 keeper visits (a long outage is observed every 15–30 s).
        let step = [1, 5, 15, 30][(a / 5) as usize % 4].max(secs / 240);
        let how = b % 3;
        let e = self.env.pause_authority.insecure_clone();
        let market = Some(self.market_key(si));
        let halted = how != 1 && self.venue_ctx(si, |v, cx| v.halt(cx, 1 + b / 3));
        let feed = how == 1 && self.venue_ctx(si, |v, cx| v.fee_shock(cx, FEE_ABOVE_CAP));
        if how == 2 {
            self.send(
                &[set_route_broken_ix(
                    &e.pubkey(),
                    PauseScope::Market,
                    0,
                    true,
                    market,
                )],
                &[&e],
            );
        }
        if !(halted || feed || how == 2) {
            return false;
        }
        self.keeper_cadence(secs, step);
        if halted {
            self.venue_ctx(si, |v, cx| v.halt(cx, 0));
        }
        if feed {
            self.venue_ctx(si, |v, cx| v.fee_shock(cx, 0));
        }
        if how == 2 {
            self.send(
                &[set_route_broken_ix(
                    &e.pubkey(),
                    PauseScope::Market,
                    0,
                    false,
                    market,
                )],
                &[&e],
            );
        }
        self.keeper_seconds(2);
        true
    }

    fn fast_path(&mut self, si: usize, a: u32, b: u32) -> bool {
        // A deeper candidate (×3 or ×2, any locked share).
        let plant_a = if a.is_multiple_of(2) { 0 } else { 3 } + 5 * ((a / 2) % 4);
        if self
            .venue_ctx(si, |v, cx| v.special(cx, Kind::PlantCandidate, plant_a, b))
            .is_none()
        {
            return false;
        }
        let fee = a.is_multiple_of(3);
        let disturbed = if fee {
            self.venue_ctx(si, |v, cx| v.fee_shock(cx, FEE_ABOVE_CAP))
        } else {
            self.venue_ctx(si, |v, cx| v.halt(cx, 1 + b / 7))
        };
        if !disturbed {
            return false;
        }
        let route = i64::from(self.env.config().params.pool_fast_route_s);
        let window = self.env.config().params.pool_fast_window_s;
        self.keeper_cadence(route as u32 + 5, 5);
        self.venue_ctx(si, |v, cx| v.special(cx, Kind::ProposePool, u32::MAX, b));
        self.keeper_cadence(window + 2, 1 + b % 5);
        self.venue_ctx(si, |v, cx| v.special(cx, Kind::ExecuteSwitch, u32::MAX, b));
        if fee {
            self.venue_ctx(si, |v, cx| v.fee_shock(cx, 0));
        } else {
            self.venue_ctx(si, |v, cx| v.halt(cx, 0));
        }
        self.keeper_seconds(3);
        true
    }

    // ---- gates, venue admin, upgrades --------------------------------------------------------

    fn gates(&mut self, si: usize, a: u32) -> bool {
        // Set (a quarter of the time) or cleared: gates stay until the next `gates` action.
        let a = if a.is_multiple_of(4) {
            1 + (a / 4) % 7
        } else {
            0
        };
        let e = self.env.pause_authority.insecure_clone();
        let market = Some(self.market_key(si));
        self.send(
            &[
                ix::set_pause(&e.pubkey(), PauseScope::Market, a & 1 != 0, market),
                set_route_broken_ix(&e.pubkey(), PauseScope::Market, 0, a & 2 != 0, market),
            ],
            &[&e],
        );
        let World {
            env, stats, slots, ..
        } = self;
        slots[si].venue.halt(
            &mut Ctx { env, stats },
            if a & 4 != 0 { 1 + a / 8 } else { 0 },
        );
        if a != 0 {
            self.later(a / 8, Undo::MarketGates(si));
        }
        true
    }

    fn disable_venue(&mut self, si: usize) -> bool {
        let bit = self.venue_bit_index(si);
        if self.disabled_bits & (1 << bit) != 0 {
            // Disabled: Governance re-enables it (timelocked).
            self.reenable_venues();
            return true;
        }
        let e = self.env.pause_authority.insecure_clone();
        if self
            .send(&[ix::disable_venue(&e.pubkey(), bit)], &[&e])
            .is_ok()
        {
            self.disabled_bits |= 1 << bit;
            self.later(u32::from(bit), Undo::EnableVenues);
        }
        true
    }

    /// Re-enables every venue bit `disable_venue` cleared (Governance, timelocked).
    fn reenable_venues(&mut self) {
        if self.disabled_bits == 0 {
            return;
        }
        let bits = self.disabled_bits;
        self.governance_change(ConfigChange::EnableVenues { bits });
        self.disabled_bits = 0;
    }

    fn upgrade_shock(&mut self, si: usize, a: u32, b: u32) -> bool {
        let progs = self.slots[si].venue.guard_programs();
        if progs.is_empty() {
            return false;
        }
        let index = progs[b as usize % progs.len()];
        let slot = self.env.clock().slot;
        {
            let World { env, slots, .. } = self;
            slots[si]
                .venue
                .set_deploy_slot(env, index, slot.saturating_sub(1));
        }
        self.env.assert_invariants();
        if a % 2 == 1 {
            self.send(&[ix::flag_venue_upgraded(index)], &[]);
        }
        self.later(a / 2, Undo::Revalidate(si));
        true
    }

    fn revalidate(&mut self, si: usize) -> bool {
        let g = self.env.venue_guard();
        let e = self.env.pause_authority.insecure_clone();
        let mut any = false;
        for index in self.slots[si].venue.guard_programs() {
            if g.upgraded & (1 << index) == 0 {
                continue;
            }
            if let Some(slot) = self.slots[si].venue.deploy_slot(&self.env, index) {
                self.send(&[ix::revalidate_venue(&e.pubkey(), index, slot)], &[&e]);
                any = true;
            }
        }
        any
    }

    fn donate(&mut self, si: usize, a: u32, b: u32) -> bool {
        let market = self.market_key(si);
        let m = self.env.market(&market);
        let lamports = 1 + u64::from(b % 50) * 10u64.pow(b % 10);
        // `trader_wsol` on a curve market (FUZZ-M7-1, resolved): a curve leg never names the
        // address, so the donation stays until `sweep_trader` or the first WSOL leg after the
        // graduation, and I7's curve clause checks the curve legs leave it unchanged.
        let target = match a % 7 {
            0 => Some(m.trader),
            1 => Some(m.trader_wsol),
            2 => Some(trader_wsol_ata(&market)),
            3 => self.slots[si].venue.donation_targets().first().copied(),
            4 => {
                let vault = m.pool_vault;
                let World { env, slots, .. } = self;
                slots[si].venue.top_up(env, &vault, lamports);
                self.env.assert_invariants();
                return true;
            }
            5 => Some(m.market_vault),
            _ => {
                let n = self.slots[si].positions.len();
                if n == 0 {
                    return false;
                }
                let (borrower, nonce, _) = self.position_key(si, b as usize % n).expect("pos");
                Some(pda::position(&market, &borrower, nonce).0)
            }
        };
        let Some(to) = target else { return false };
        let donor = self.donor.insecure_clone();
        self.send(&[transfer_ix(&donor.pubkey(), &to, lamports)], &[&donor]);
        true
    }

    // ---- governance and oracle -------------------------------------------------------------------

    /// Proposes `change`, waits out the timelock with the keeper silent (a governance change is
    /// rare; the markets' clocks jump), applies it.
    pub fn governance_change(&mut self, change: ConfigChange) -> bool {
        let g = self.env.governance.insecure_clone();
        if self
            .send(&[ix::propose_config(&g.pubkey(), change)], &[&g])
            .is_err()
        {
            return false;
        }
        let t = i64::from(self.env.config().timelock_s);
        self.env.warp(t + 1);
        let ix = self.env.apply_config_ix();
        let ok = self.send(&[ix], &[]).is_ok();
        for si in 0..self.slots.len() {
            self.crank(si);
        }
        ok
    }

    fn config_change(&mut self, a: u32, b: u32) -> bool {
        if a % 29 == 28 {
            // An `OracleFeed` change (the cache empties: every market Tier C until the next
            // `crank_sol_usd`, which `oracle_crank` plants at the configured account): to a
            // second account and back.
            let c = self.env.config();
            let account = if c.pyth_sol_usd == PYTH_SOL_USD_ACCOUNT {
                Pubkey::new_from_array([0x7e; 32])
            } else {
                PYTH_SOL_USD_ACCOUNT
            };
            return self.governance_change(ConfigChange::OracleFeed {
                account,
                feed_id: c.pyth_feed_id,
            });
        }
        let Some(change) = self.param_change(a, b) else {
            return false;
        };
        self.governance_change(change)
    }

    /// A valid in-bound `Params` change: the parameter `a` names at its lower or upper bound
    /// or midway (`b`), for one tier when it is per tier; `None` if the result does not
    /// validate.
    fn param_change(&self, a: u32, b: u32) -> Option<ConfigChange> {
        let c = self.env.config();
        let Ok(mut ps) = c.param_set() else {
            return None;
        };
        let rule = Rule::ALL[a as usize % Rule::ALL.len()];
        let (lo, hi) = rule.bounds();
        let v = match b % 3 {
            0 => lo,
            1 => hi,
            _ => lo + (hi - lo) / 2,
        };
        let tier = rule.per_tier().then(|| Tier::ALL[(b / 3) as usize % 4]);
        if ps.set(rule.name(), tier, v).is_err() {
            return None;
        }
        if mparams::validate(&ps)
            .and_then(|()| mparams::validate_econ(&ps))
            .is_err()
        {
            return None;
        }
        // The market-level EMA parameters and the kink are their own changes (a new
        // `ema_params.version`, adopted at once by a market without positions and forced after
        // `ema_force_delay_s`; a new `kink.version`).
        Some(match rule {
            Rule::EmaDtmaxS | Rule::EmaClampBps | Rule::LiqSlowTauS | Rule::LiqSlowDtmaxS => {
                ConfigChange::EmaParams {
                    ema: EmaParamsArgs {
                        ema_dtmax_s: ps.ema.dtmax_s,
                        ema_clamp_bps: ps.ema.clamp_bps,
                        liq_slow_tau_s: ps.ema.slow_tau_s,
                        liq_slow_dtmax_s: ps.ema.slow_dtmax_s,
                    },
                }
            }
            Rule::UtilKinkBps => ConfigChange::Kink {
                util_kink_bps: ps.params.util_kink_bps,
            },
            _ => ConfigChange::Params {
                params: Params::from_math(&ps.params),
                tiers: ps.tiers.map(|t| TierParams::from_math(&t)),
            },
        })
    }

    /// A pending change cancelled before it applies (Governance, or the Emergency vault with
    /// `a` odd), or left past `CONFIG_APPLY_WINDOW_S` (`b mod 4 = 3`): `apply_config` must then
    /// refuse `PendingChangeExpired`, and Governance cancels it. A `Timelock` change (`a mod 5
    /// = 4`) only Governance may cancel (`EmergencyCannotCancel`). Before its timelock the
    /// change must not apply (`TimelockNotElapsed`), and in every case the config's parameters
    /// and timelock must be the ones before the proposal.
    fn config_cancel(&mut self, a: u32, b: u32) -> bool {
        let g = self.env.governance.insecure_clone();
        let e = self.env.pause_authority.insecure_clone();
        let before = self.env.config();
        let timelock_change = a % 5 == 4;
        let change = if timelock_change {
            // A Timelock change that keeps a governance action affordable for the fuzzer.
            ConfigChange::Timelock {
                timelock_s: before.timelock_s + 1 + a % 3_600,
            }
        } else {
            match self.param_change(a / 2, b / 4) {
                Some(c) => c,
                None => return false,
            }
        };
        if self
            .send(&[ix::propose_config(&g.pubkey(), change)], &[&g])
            .is_err()
        {
            return false;
        }
        let timelock = i64::from(before.timelock_s);
        let expect = |_: &mut World, ixs: &[Instruction], res: &TxResult, want: E, what: &str| {
            if shortwin_code(ixs, res) != Some(code(want)) {
                let got = match res {
                    Ok(_) => "success".to_string(),
                    Err(f) => crate::stats::error_name(&f.err, &f.meta.logs),
                };
                panic!("{ORACLE} {what}: expected {want:?}, got {got}");
            }
        };
        // Not before the timelock.
        let ixs = [self.env.apply_config_ix()];
        let res = self.send(&ixs, &[]);
        expect(
            self,
            &ixs,
            &res,
            E::TimelockNotElapsed,
            "apply_config before the timelock",
        );
        if b % 4 == 3 {
            self.env.warp(timelock + CONFIG_APPLY_WINDOW_S + 1);
            let ixs = [self.env.apply_config_ix()];
            let res = self.send(&ixs, &[]);
            expect(
                self,
                &ixs,
                &res,
                E::PendingChangeExpired,
                "apply_config past the window",
            );
            self.send(&[ix::cancel_config(&g.pubkey())], &[&g]);
        } else {
            self.env.warp(i64::from(a % 3_600).min(timelock - 1).max(0));
            let emergency = a % 2 == 1;
            if emergency && timelock_change {
                let ixs = [ix::cancel_config(&e.pubkey())];
                let res = self.send(&ixs, &[&e]);
                expect(
                    self,
                    &ixs,
                    &res,
                    E::EmergencyCannotCancel,
                    "the Emergency vault cancelling a Timelock change",
                );
            }
            let (who, kp) = if emergency && !timelock_change {
                ("Emergency", &e)
            } else {
                ("Governance", &g)
            };
            let ixs = [ix::cancel_config(&kp.pubkey())];
            let res = self.send(&ixs, &[kp]);
            if let Err(f) = &res {
                panic!(
                    "{ORACLE} {who} could not cancel a pending change: {:?}\nlogs:\n{}",
                    f.err,
                    f.meta.pretty_logs()
                );
            }
            self.env.warp(timelock + 1);
            let ixs = [self.env.apply_config_ix()];
            let res = self.send(&ixs, &[]);
            expect(
                self,
                &ixs,
                &res,
                E::NoPendingChange,
                "apply_config after a cancel",
            );
        }
        let after = self.env.config();
        if before.params != after.params || before.timelock_s != after.timelock_s {
            panic!("{ORACLE} a cancelled or lapsed change moved the config");
        }
        for si in 0..self.slots.len() {
            self.crank(si);
        }
        true
    }

    fn oracle_crank(&mut self, a: u32, b: u32) -> bool {
        let Ok(mut u) =
            shortwin_venue_state::pyth::read(&shortwin_venue_state::pyth::RECEIVER, PYTH_DUMP)
        else {
            return false;
        };
        let now = self.env.now();
        let age = i64::from(a % 240);
        let step = i64::from(b % 6_000) - 3_000; // ±30 % (the step bound is 20 %)
        let cached = self.env.oracle_cache();
        let base = if cached.price > 0 && cached.expo == u.exponent {
            cached.price
        } else {
            u.price
        };
        u.price = base + base * step / 10_000;
        u.publish_time = now - age;
        // Updates the program must never cache: future-dated past the age bound, a
        // confidence interval over `oracle_max_conf_bps`, an account the receiver does not
        // own.
        let max_age = i64::from(self.env.config().params.oracle_max_age_s);
        let conf_max = u64::from(self.env.config().params.oracle_max_conf_bps);
        let bad = match a % 16 {
            15 => {
                u.publish_time = now + max_age + 1 + i64::from(b % 600);
                Some("future-dated")
            }
            14 => {
                u.conf = u.price.unsigned_abs() * (conf_max + 1 + u64::from(b % 500)) / 10_000;
                Some("too wide a confidence interval")
            }
            13 => Some("an account the Pyth receiver does not own"),
            _ => None,
        };
        u.prev_publish_time = u.publish_time - 1;
        u.posted_slot = self.env.clock().slot;
        let data = shortwin_venue_state::pyth::encode(&u).to_vec();
        let lamports = self.env.svm.minimum_balance_for_rent_exemption(data.len());
        let feed = self.env.config().pyth_sol_usd;
        let owner = if a % 16 == 13 {
            anchor_lang::system_program::ID
        } else {
            PYTH_RECEIVER_ID
        };
        self.env
            .svm
            .set_account(
                feed,
                solana_account::Account {
                    lamports,
                    data,
                    owner,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .expect("plant PriceUpdateV2");
        let res = self.send(&[ix::crank_sol_usd(&feed)], &[]);
        if let Some(why) = bad
            && res.is_ok()
        {
            panic!("{ORACLE} crank_sol_usd cached a price update with {why}");
        }
        true
    }

    // ---- degraded mode (O17), the mint filter ---------------------------------------------------

    /// Every account [`SNAPSHOT_OWNERS`] hold (the fee payer excluded: it pays every fee).
    pub fn snapshot(&self) -> Snapshot {
        let payer = self.env.payer.pubkey();
        let mut w = Snapshot::new();
        for owner in SNAPSHOT_OWNERS {
            for (k, a) in self.env.svm.get_program_accounts(&owner) {
                if k != payer {
                    w.insert(k, (a.lamports, a.owner, a.data));
                }
            }
        }
        w
    }

    /// I4's coverage clause: `after` is `before` except bit 6 of `market`'s `flags`, which is
    /// `paused` after, and — only when that cleared a set flag, and then to `now` —
    /// `market`'s `coverage_cleared_at` (M7-SEC-3's clear delay).
    fn assert_only_coverage_bit(
        before: &Snapshot,
        after: &Snapshot,
        market: &Pubkey,
        paused: bool,
        now: i64,
    ) {
        let bit = u8::try_from(MARKET_FLAG_PAUSED_COVERAGE).expect("low byte");
        if before.len() != after.len() || before.keys().ne(after.keys()) {
            panic!("{ORACLE} I4 (coverage): set_coverage_pause created or closed an account");
        }
        for (k, (la, oa, da)) in before {
            let (lb, ob, db) = &after[k];
            if (la, oa) != (lb, ob) || da.len() != db.len() {
                panic!(
                    "{ORACLE} I4 (coverage): set_coverage_pause changed the lamports, owner or \
                     size of {k}: {la} -> {lb}"
                );
            }
            let at = MARKET_COVERAGE_CLEARED_AT_OFFSET..MARKET_COVERAGE_CLEARED_AT_OFFSET + 8;
            if k == market {
                let bit = u8::try_from(MARKET_FLAG_PAUSED_COVERAGE).expect("low byte");
                let was_set = da.get(MARKET_FLAGS_OFFSET).is_some_and(|f| f & bit != 0);
                let word = |d: &[u8]| {
                    d.get(at.clone())
                        .and_then(|s| <[u8; 8]>::try_from(s).ok())
                        .map(i64::from_le_bytes)
                };
                let (t0, t1) = (word(da), word(db));
                let ok = if was_set && !paused {
                    t1 == Some(now)
                } else {
                    t0 == t1
                };
                if !ok {
                    panic!(
                        "{ORACLE} I4 (coverage): coverage_cleared_at {t0:?} -> {t1:?} after \
                         set_coverage_pause({paused}) on a flag that was {was_set} at clock {now}"
                    );
                }
            }
            for (i, (x, y)) in da.iter().zip(db).enumerate() {
                if k == market && at.contains(&i) {
                    continue;
                }
                let (x, y) = if k == market && i == MARKET_FLAGS_OFFSET {
                    if (y & bit != 0) != paused {
                        panic!(
                            "{ORACLE} I4 (coverage): paused_coverage is {} after \
                             set_coverage_pause({paused})",
                            y & bit != 0
                        );
                    }
                    (x & !bit, y & !bit)
                } else {
                    (*x, *y)
                };
                if x != y {
                    panic!(
                        "{ORACLE} I4 (coverage): set_coverage_pause changed byte {i} of {k} \
                         ({x:#04x} -> {y:#04x})"
                    );
                }
            }
        }
    }

    /// `set_coverage_pause(paused, reason)` on slot `si`, signed by `signer`.
    fn coverage_ix(signer: &Pubkey, market: &Pubkey, paused: bool, reason: u8) -> Instruction {
        Instruction {
            program_id: shortwin::ID,
            accounts: shortwin::accounts::SetCoveragePause {
                coverage_authority: *signer,
                config: pda::config(),
                market: *market,
                event_authority: pda::event_authority(),
                program: shortwin::ID,
            }
            .to_account_metas(None),
            data: shortwin::instruction::SetCoveragePause { paused, reason }.data(),
        }
    }

    /// The keeper's `set_coverage_pause(paused)` with the installed key: must land, and must
    /// change nothing but the flag (I4's coverage clause). `false` without a key.
    fn coverage_set(&mut self, si: usize, paused: bool) -> bool {
        let Some(kp) = self.coverage.as_ref().map(Keypair::insecure_clone) else {
            return false;
        };
        self.coverage_send(si, &kp, paused, COVERAGE_REASON_KEEPER, Ok(()));
        true
    }

    /// Sends `set_coverage_pause` and checks the outcome against `expect` (`Err(code)`: the
    /// refusal), and a landed one against the whole VM.
    fn coverage_send(
        &mut self,
        si: usize,
        kp: &Keypair,
        paused: bool,
        reason: u8,
        expect: Result<(), E>,
    ) {
        let market = self.market_key(si);
        let ixs = [Self::coverage_ix(&kp.pubkey(), &market, paused, reason)];
        let before = expect.is_ok().then(|| self.snapshot());
        let res = self.send(&ixs, &[kp]);
        match (expect, &res) {
            (Ok(()), Ok(_)) => {
                let after = self.snapshot();
                let now = self.env.now();
                Self::assert_only_coverage_bit(
                    before.as_ref().expect("snapshot"),
                    &after,
                    &market,
                    paused,
                    now,
                );
            }
            (Err(e), Err(_)) if shortwin_code(&ixs, &res) == Some(code(e)) => {}
            (expect, res) => panic!(
                "{ORACLE} set_coverage_pause({paused}, reason {reason}) by {}: expected {expect:?}, \
                 got {}",
                kp.pubkey(),
                match res {
                    Ok(_) => "success".to_string(),
                    Err(f) => format!(
                        "{}\nlogs:\n{}",
                        crate::stats::error_name(&f.err, &f.meta.logs),
                        f.meta.pretty_logs()
                    ),
                }
            ),
        }
    }

    fn coverage_pause(&mut self, si: usize, a: u32, b: u32) -> bool {
        let mode = b % 16;
        if mode == 15 {
            return self.rotate_coverage(b / 16);
        }
        let paused = a % 2 == 1;
        let reason = if (b / 16) % 8 == 7 {
            [0u8, 2, 255][(b / 128) as usize % 3]
        } else {
            COVERAGE_REASON_KEEPER
        };
        let stranger = Keypair::new();
        let (kp, role) = match mode {
            12 => (self.env.governance.insecure_clone(), false),
            13 => (self.env.pause_authority.insecure_clone(), false),
            14 => (stranger, false),
            _ => match &self.coverage {
                Some(k) => (k.insecure_clone(), true),
                None => (stranger, false),
            },
        };
        // ★OWNER-M7-3 (b): the Emergency's clear passes the role checks whatever the role.
        let emergency_clear = mode == 13 && !paused;
        // The checks in order: the Emergency clear, else the role enabled and the signer; then the
        // reason (ARCH §3.1).
        let expect =
            if !emergency_clear && self.env.config().coverage_authority == Pubkey::default() {
                Err(E::CoverageAuthorityDisabled)
            } else if !role && !emergency_clear {
                Err(E::Unauthorized)
            } else if reason != COVERAGE_REASON_KEEPER {
                Err(E::InvalidParams)
            } else {
                Ok(())
            };
        let landed = expect.is_ok();
        self.coverage_send(si, &kp, paused, reason, expect);
        if landed && paused {
            self.later(a / 2, Undo::CoveragePause(si));
        }
        true
    }

    /// Governance's timelocked `Authorities` change of `coverage_authority`: a new key, or
    /// (`x mod 4 = 3`) the default key, which makes the role inert. The old key must be refused
    /// afterwards (checked by the next [`Kind::CoveragePause`] against [`World::coverage`]).
    fn rotate_coverage(&mut self, x: u32) -> bool {
        let new = (x % 4 != 3).then(Keypair::new);
        let key = new.as_ref().map_or(Pubkey::default(), Keypair::pubkey);
        let c = self.env.config();
        let change = ConfigChange::Authorities {
            governance: c.governance,
            pause_authority: c.pause_authority,
            coverage_authority: key,
            fee_recipient: c.fee_recipient,
        };
        if !self.governance_change(change) {
            return false;
        }
        if self.env.config().coverage_authority != key {
            panic!("{ORACLE} an applied Authorities change did not install coverage_authority");
        }
        self.coverage = new;
        true
    }

    /// Plants a freeze authority on slot `si`'s mint (the filter then fails), optionally flags
    /// the market (`flag_unsafe_mint`, anyone); or flags a safe mint (`MintSafe`).
    fn mint_shock(&mut self, si: usize, a: u32, b: u32) -> bool {
        let mint = self.slots[si].venue.vm().mint();
        let market = self.market_key(si);
        let flagged = self.market(si).has_flag(MARKET_FLAG_MINT_UNSAFE);
        let planted = self.mint_saved.iter().any(|(k, _)| *k == mint);
        if a % 4 == 3 && !planted {
            // A safe mint cannot be flagged; a flagged market cannot be flagged again.
            let ixs = [ix::flag_unsafe_mint(&mint)];
            let res = self.send(&ixs, &[]);
            let want = if flagged {
                E::MintUnsafeFlagged
            } else {
                E::MintSafe
            };
            if shortwin_code(&ixs, &res) != Some(code(want)) {
                panic!(
                    "{ORACLE} flag_unsafe_mint on a mint the filter passes: expected {want:?}, got \
                     {:?}",
                    res.as_ref().map(|_| ()).map_err(|f| f.err.clone())
                );
            }
            return true;
        }
        if !planted {
            let Some(mut acc) = self.env.svm.get_account(&mint) else {
                return false;
            };
            if acc.data.len() < MINT_FREEZE_END {
                return false;
            }
            self.mint_saved
                .push((mint, acc.data[MINT_FREEZE_AT..MINT_FREEZE_END].to_vec()));
            acc.data[MINT_FREEZE_AT..MINT_FREEZE_AT + 4].copy_from_slice(&1u32.to_le_bytes());
            acc.data[MINT_FREEZE_AT + 4..MINT_FREEZE_END]
                .copy_from_slice(Pubkey::new_unique().as_ref());
            self.env
                .svm
                .set_account(mint, acc)
                .expect("plant a freeze authority");
            self.env.assert_invariants();
            self.later(a / 4, Undo::Mint(si));
        }
        // The flag is one-way (opens and deposits refused for the rest of the run): rare.
        if b % 8 == 1 {
            let ixs = [ix::flag_unsafe_mint(&mint)];
            let res = self.send(&ixs, &[]);
            let want = if flagged {
                Err(code(E::MintUnsafeFlagged))
            } else {
                Ok(())
            };
            let got = match &res {
                Ok(_) => Ok(()),
                Err(_) => Err(shortwin_code(&ixs, &res).unwrap_or(0)),
            };
            if got != want {
                panic!(
                    "{ORACLE} flag_unsafe_mint on market {market} (mint with a freeze authority): \
                     expected {want:?}, got {got:?}"
                );
            }
        }
        true
    }

    /// Writes slot `si`'s mint's original `freeze_authority` back.
    fn mint_restore(&mut self, si: usize) {
        let mint = self.slots[si].venue.vm().mint();
        let Some(at) = self.mint_saved.iter().position(|(k, _)| *k == mint) else {
            return;
        };
        let (_, bytes) = self.mint_saved.remove(at);
        if let Some(mut acc) = self.env.svm.get_account(&mint) {
            acc.data[MINT_FREEZE_AT..MINT_FREEZE_END].copy_from_slice(&bytes);
            self.env
                .svm
                .set_account(mint, acc)
                .expect("restore the mint");
            self.env.assert_invariants();
        }
    }

    // ---- I14 -------------------------------------------------------------------------------------

    fn i14(&mut self, si: usize, a: u32, b: u32, replace: bool) -> bool {
        let n = self.slots[si].positions.len();
        let which = a % 5;
        let (ix, signers, remaining): (Instruction, Vec<Keypair>, usize) = match which {
            0 => {
                let bi = b as usize % self.slots[si].borrowers.len();
                let q = self.size(si, b);
                let (ixs, kp) = self.open_ix(si, bi, q);
                let ix = ixs
                    .into_iter()
                    .find(|i| i.program_id == shortwin::ID)
                    .expect("open");
                let r = self.slots[si].venue.vm().trade_remaining(&[]).len();
                (ix, vec![kp], r)
            }
            1..=3 if n > 0 => {
                let (ix, bkp) = self.exit_ix(si, b as usize % n, which).expect("pos");
                let vm = &self.slots[si].venue.vm();
                let r = if which == 3 {
                    vm.read_set(false).len()
                } else {
                    vm.trade_remaining(&[]).len()
                };
                let mut s = vec![self.keeper.insecure_clone()];
                s.extend(bkp);
                (ix, s, r)
            }
            _ => {
                let vm = &self.slots[si].venue.vm();
                (crank_price_ix(vm), Vec::new(), vm.read_set(false).len())
            }
        };
        // Only the remaining accounts (after the Anchor accounts): what feeds pricing, route,
        // tier or guard.
        if remaining == 0 || ix.accounts.len() < remaining {
            return false;
        }
        let fixed = ix.accounts.len() - remaining;
        let at = fixed + (b as usize / 7) % remaining;
        let mut bad = ix.clone();
        let victim = bad.accounts[at].pubkey;
        if replace {
            bad.accounts[at].pubkey = Pubkey::new_unique();
        } else {
            bad.accounts.remove(at);
        }
        let refs: Vec<&Keypair> = signers.iter().collect();
        let (res, _) = self.send_cu(si, std::slice::from_ref(&bad), &refs);
        if res.is_ok() {
            let name = crate::stats::ix_name(&ix.data);
            let what = if replace {
                "replaced by a fresh key"
            } else {
                "omitted"
            };
            let detail = format!(
                "{name}: remaining account #{} ({victim}) {what}; the instruction landed",
                at - fixed
            );
            if replace {
                self.stats
                    .anomaly(&format!("i14-replace-accepted:{name}"), detail);
            } else {
                panic!("{ORACLE} I14: {detail}");
            }
        }
        true
    }

    // ---- wind-down -------------------------------------------------------------------------------

    /// Clears every gate and venue disturbance, restores prices, exits every position and
    /// takes every lender out; fails the run if the market does not return to rest (module
    /// docs, liveness).
    pub fn wind_down(&mut self) {
        self.stats.wind_downs += 1;
        self.run_undos(true);
        // Gates off.
        let e = self.env.pause_authority.insecure_clone();
        self.send(
            &[ix::set_pause(&e.pubkey(), PauseScope::Global, false, None)],
            &[&e],
        );
        for si in 0..self.slots.len() {
            let market = Some(self.market_key(si));
            let bit = self.venue_bit_index(si);
            self.send(
                &[
                    ix::set_pause(&e.pubkey(), PauseScope::Market, false, market),
                    set_route_broken_ix(&e.pubkey(), PauseScope::Market, 0, false, market),
                    set_route_broken_ix(&e.pubkey(), PauseScope::Global, bit, false, None),
                ],
                &[&e],
            );
            let World {
                env, stats, slots, ..
            } = self;
            slots[si].venue.restore(&mut Ctx { env, stats });
            self.revalidate(si);
        }
        self.reenable_venues();
        // Prices back near where they started.
        for si in 0..self.slots.len() {
            self.restore_price(si);
        }
        self.keeper_seconds(12);
        // Everyone out.
        let mut phase = 0;
        loop {
            let open: usize = self.slots.iter().map(|s| s.positions.len()).sum();
            if open == 0 {
                break;
            }
            phase += 1;
            let mut straddled: Vec<usize> = Vec::new();
            for si in 0..self.slots.len() {
                let mut k = 0;
                while k < self.slots[si].positions.len() {
                    let (borrower, nonce, kp) = self.position_key(si, k).expect("pos");
                    let vm = &self.slots[si].venue.vm();
                    let ixs = with_cu_limit(
                        vm,
                        close_short_ix(vm, &borrower, nonce, u64::MAX, ExtrasSel::NONE, &[]),
                    );
                    let res = self.send(&ixs, &[&kp]);
                    self.check_exit_refusal("close_short (wind-down)", &ixs, &res);
                    if shortwin_code(&ixs, &res) == Some(code(E::CurveStraddle))
                        && !straddled.contains(&si)
                    {
                        straddled.push(si);
                    }
                    self.prune();
                    let still = self.slots[si].positions.iter().any(|p| {
                        p.nonce == nonce && self.slots[si].borrowers[p.b].key() == borrower
                    });
                    if !still {
                        continue;
                    }
                    let gone = self.keeper_exit(si, k, 0)
                        || self.keeper_exit(si, k, 1)
                        || self.keeper_exit(si, k, 2);
                    if !gone {
                        k += 1;
                    }
                }
            }
            // A position the live curve's real token reserves cannot buy back (`Q > rT`: every
            // exit refuses `CurveStraddle`, ARCH §5.1) leaves once the curve completes — the
            // straddle's first step (§5.4: the in-house keeper's bundle, or anyone's buy of `rT`).
            // The wind-down completes it and, as for any completed curve, does not migrate it: the
            // market is left `PumpMigrating` and the S3 path below must settle it. The price
            // restore above cannot always bring `rT` back above `Q` (one push moves the price at
            // most 50×); as built the wind-down never completed such a curve, and the position
            // stayed open (FUZZ-M7D-1).
            for si in straddled {
                let World {
                    env, stats, slots, ..
                } = self;
                if slots[si]
                    .venue
                    .special(&mut Ctx { env, stats }, Kind::CompleteCurve, 0, 0)
                    .is_some()
                {
                    *self
                        .stats
                        .events
                        .entry("fuzz:wind-down-curve-completed".into())
                        .or_default() += 1;
                }
            }
            match phase {
                // The keeper's per-second coverage, long enough for an armed settlement's
                // persistence (`settle_persist_n` ≤ 600 observations inside
                // `settle_arm_gap_s`) at phases 31 and 92.
                1..=30 | 32..=90 | 93..=150 => self.keeper_second(),
                31 | 92 => self.persist_seconds(),
                // Past every deadline, then the keeper's cadence (every 30 s, inside
                // `S3_OBS_GAP_S`, so the S3 chain never restarts) for the whole S3 wait after
                // the migration pause and the graduation grace.
                91 => {
                    self.warp_past_deadlines();
                    self.s3_cadence();
                }
                151 | 152 => {
                    self.env.warp(86_400);
                    self.s3_cadence();
                    self.persist_seconds();
                }
                _ => {
                    let diag = self.stuck_diagnosis();
                    panic!(
                        "{ORACLE} liveness (I8/I-P6, I-E11): position(s) still open after the \
                         wind-down's closes, keeper exits, deadlines, S3 and two days\n{diag}"
                    );
                }
            }
        }
        // Past every lender's lock (a recent deposit's lock is legitimately effective).
        let now = self.env.now();
        let mut until = now;
        for si in 0..self.slots.len() {
            let market = self.market_key(si);
            for l in &self.slots[si].lenders {
                if let Some(lp) = self.env.lender(&market, &l.kp.pubkey()) {
                    until = until.max(lp.lock_until);
                }
            }
        }
        if until > now {
            self.env.warp((until - now + 1).min(30 * 86_400));
            self.keeper_seconds(2);
        }
        for si in 0..self.slots.len() {
            self.lenders_out(si, false, true);
            let m = self.market(si);
            let rest = (
                m.open_positions,
                m.borrowed_q,
                m.receivable,
                m.rent_aggr.iter().all(|x| x.get() == 0),
            );
            if rest != (0, 0, 0, true) {
                panic!(
                    "{ORACLE} liveness: market {} not at rest after the wind-down: open {} \
                     borrowed {} receivable {} rent aggregates zero {}",
                    self.market_key(si),
                    rest.0,
                    rest.1,
                    rest.2,
                    rest.3
                );
            }
        }
        self.env.assert_invariants();
    }

    /// Why the remaining positions are stuck: each exit's outcome now, the market's venue and
    /// chains, each position's clock (the liveness failure's message).
    fn stuck_diagnosis(&mut self) -> String {
        let mut out = String::new();
        for si in 0..self.slots.len() {
            let m = self.market(si);
            out += &format!(
                "market {} ({}): venue {:?} flags {:#x} ema_warm {} ema_guard {} no_route_since {} \
                 pool_fail_since {} executable {:?} open {} now {}\n",
                self.market_key(si),
                self.slots[si].venue.label(),
                m.venue_kind(),
                m.flags,
                m.ema_warm,
                m.ema_guard,
                m.no_route_since,
                m.pool_fail_since,
                venue_executable(&self.env.svm, &m),
                m.open_positions,
                self.env.now()
            );
            for k in 0..self.slots[si].positions.len() {
                let (borrower, nonce, _) = self.position_key(si, k).expect("pos");
                let pos = self
                    .env
                    .position(&self.slots[si].venue.vm(), &borrower, nonce);
                out += &format!(
                    "  position {borrower}#{nonce}: q {} v {} deadline {} s_armed {} s_count {} \
                     s_since {} venue_at_open {}\n",
                    pos.q,
                    pos.v,
                    pos.deadline,
                    pos.s_armed,
                    pos.s_count,
                    pos.s_since,
                    pos.venue_at_open
                );
                for which in 0..4u32 {
                    let (ix, bkp) = self.exit_ix(si, k, which).expect("pos");
                    let keeper = self.keeper.insecure_clone();
                    let mut signers = vec![&keeper];
                    if let Some(bk) = bkp.as_ref() {
                        signers.push(bk);
                    }
                    let saved = self.env.svm.clone();
                    let cu = self.slots[si].venue.exit_cu();
                    let mut ixs = Vec::new();
                    if let Some(units) = cu {
                        ixs.push(shortwin_litesvm_tests::pump::cu_limit_ix(units));
                    }
                    ixs.push(ix);
                    let r = self.env.send(&ixs, &signers);
                    self.env.svm = saved;
                    let what = ["close", "liquidate", "expire", "sol_settle"][which as usize];
                    let res = match &r {
                        Ok(_) => "lands (on a clone)".to_string(),
                        Err(f) => crate::stats::error_name(&f.err, &f.meta.logs),
                    };
                    out += &format!("    {what}: {res}\n");
                }
            }
        }
        out
    }

    /// Keeper seconds for `settle_persist_n` and half a minute more (while positions remain).
    fn persist_seconds(&mut self) {
        let n = self.env.config().params.settle_persist_n + 30;
        for _ in 0..n {
            if self.slots.iter().all(|s| s.positions.is_empty()) {
                break;
            }
            self.keeper_second();
        }
    }

    /// The keeper's cadence (every 30 s) for `s3_wait_s + max(grad_grace_s,
    /// max_migration_pause_s)` and three more minutes: long enough for S3 from any chain start.
    fn s3_cadence(&mut self) {
        let p = self.env.config().params;
        let secs = p.s3_wait_s + p.grad_grace_s.max(p.max_migration_pause_s) + 180;
        let mut t = 0;
        while t < secs && self.slots.iter().any(|s| !s.positions.is_empty()) {
            self.keeper_cadence(300.min(secs - t), 30);
            t += 300;
        }
    }

    fn warp_past_deadlines(&mut self) {
        let mut latest = self.env.now();
        for si in 0..self.slots.len() {
            let vm_market = self.market_key(si);
            for p in &self.slots[si].positions {
                let key =
                    pda::position(&vm_market, &self.slots[si].borrowers[p.b].key(), p.nonce).0;
                if let Some(pos) =
                    read::borsh::<shortwin::state::ShortPosition>(&self.env.svm, &key)
                {
                    latest = latest.max(pos.deadline + 5);
                }
            }
        }
        let now = self.env.now();
        if latest > now {
            self.env.warp(latest - now);
        }
    }

    fn restore_price(&mut self, si: usize) {
        let Some(p0) = self.slots[si].spot0 else {
            return;
        };
        let Some(p) = spot_of(self.slots[si].venue.reserves(&self.env)) else {
            return;
        };
        if p == 0 {
            return;
        }
        let (up, ratio) = if p0 > p {
            (true, p0 as f64 / p as f64)
        } else {
            (false, p as f64 / p0 as f64)
        };
        if ratio < 1.02 {
            return;
        }
        if let Some(mv) = self.push(si, up, (ratio.min(50.0) * 1e6) as u64) {
            self.env.warp(1);
            self.send_move(si, &mv, false);
        }
    }
}

impl Slot {
    /// The mock market when no knob disturbs its trades (the differential exit oracle's
    /// domain: a required extra makes the venue CPI fail, which the oracle does not model).
    fn as_mock_undisturbed(&self, env: &Env) -> bool {
        self.venue.as_mock().is_some_and(|mm| {
            shortwin_litesvm_tests::mock::knobs(&env.svm, &mm.mock().pool)
                .required_extras
                .is_empty()
        })
    }
}

/// A short.win code's name (any code).
pub fn code_name_any(c: u32) -> String {
    c.checked_sub(6000)
        .and_then(|i| ERR_NAMES.get(i as usize))
        .map_or_else(|| format!("{c}"), |n| (*n).to_string())
}

/// `programs/shortwin/src/errors.rs`, whose `ShortwinError` variants, in order, are the codes
/// from 6000 (Anchor's `#[error_code]`).
const ERRORS_RS: &str = include_str!("../../../programs/shortwin/src/errors.rs");

/// short.win's error names by `code − 6000`, read from `errors.rs` (so an appended code needs
/// no edit here; the `pins` test there fixes the numbering).
pub static ERR_NAMES: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
    let body = ERRORS_RS
        .split("pub enum ShortwinError {")
        .nth(1)
        .and_then(|b| b.split("\n}").next())
        .expect("errors.rs: `pub enum ShortwinError {`");
    body.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && !l.starts_with('#'))
        .filter_map(|l| l.strip_suffix(','))
        .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric()))
        .collect()
});

#[cfg(test)]
mod tests {
    use super::*;

    /// The parsed names agree with the enum's own codes at both ends and in the middle.
    #[test]
    fn error_names_match_the_codes() {
        for e in [
            E::MathOverflow,
            E::CoverageAuthorityDisabled,
            E::CoveragePaused,
            E::ExitBoundNotMet,
            E::PendingChangeExpired,
            E::GraduationHalted,
        ] {
            assert_eq!(code_name_any(code(e)), format!("{e:?}"));
        }
        assert!(ERR_NAMES.len() >= 105);
        assert_eq!(code_name_any(42), "42");
    }
}
