//! T-TRACE on the pump curve, with the stack depth (ARCH §6.2, §6.3, §8.2; C1-1, C1-8; §9 M5):
//! every curve-path instruction of the release short.win, on the dumped pump, pump-fees and
//! PumpSwap programs (SPL Token and Token-2022 curves), read off the transaction's own inner
//! instructions.
//!
//! - **Per instruction**: the exact instruction-trace entries (the instruction plus every CPI at
//!   any depth) and the exact call shape — which program at which stack height — of
//!   `create_market(Auto)` (a curve listing with the init set; a PumpSwap listing with the pAMM
//!   init set; a list-only listing),
//!   `crank_price` (roll and record; `PumpCurve → PumpMigrating`, `PumpMigrating → None`,
//!   `None → PumpCurve`), `open_short` (a wallet's first open, a repeat borrower, with
//!   normalisation, with the creator-vault top-up), `close_short` (plain, normalising, the top-up
//!   inside `B`, a completing buyback), `liquidate`, `expire`, `sol_settle` (no-op; S3 on
//!   `PumpMigrating` and on `None`), `sweep_trader`'s token leg, `flag_venue_upgraded` and
//!   `revalidate_venue`. Every curve leg is one pump CPI at height 2 whose pump-fees, token,
//!   System and event CPIs sit at height 3, the peak: short.win keeps two spare levels (ARCH
//!   §6.3; the bench `tests/bench/m5_cu` runs them through one and two wrapper programs).
//! - **Worst-case transactions** (ComputeBudget included): the largest open and close, an atomic
//!   sandwich of an open with the curve's own trades, the most opens of one wallet in one
//!   transaction, a batch of four cranks that all move their market to `PumpMigrating`, the
//!   keeper's `[crank, liquidate | expire | sol_settle S3]`, the three pump latch flags — each ≤ 64
//!   (`MAX_INSTRUCTION_TRACE_LENGTH`); one open more is refused by the runtime itself.
//!
//! Figures against ARCH §6.2's curve column (open 9 + 1 + 2 = 12, ≤ 15; exits 11 + 1 = 12,
//! ≤ 15): an open is 14 entries for a wallet's first position (the table omits the position
//! `init` and the exposure `init_if_needed`, as on Raydium), 13 for a repeat borrower, 17 with
//! normalisation and 18 with the creator-vault top-up; an exit is 12 on a curve without a
//! creator fee and 13 with one (pump's creator-vault transfer), 15 / 16 normalising, 17 with the
//! top-up; a crank that moves the venue is 3–4 (its `VenueStateChanged` and chain events). The
//! table's maxima of 15 are exceeded by the normalising and top-up paths.
//!
//! Every transaction goes through `Env::send`: the invariant hooks run after each instruction.
//! A keeper's decision to send is a `simulate_transaction` on the current state, never a
//! committed probe. The only planted state is the venue's (pump's curve account: a creator
//! change, reserves zeroed by a migration elsewhere, a curve removed before a listing; the
//! header of a ProgramData for the latch).

// `TxResult` is LiteSVM's own `TransactionResult` (as in the harness library).
#![allow(clippy::result_large_err)]

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use litesvm::types::TransactionMetadata;
use shortwin::constants::{
    ASSOCIATED_TOKEN_PROGRAM_ID, GUARD_PUMP, GUARD_PUMP_AMM, GUARD_PUMP_FEES, PUMP_FEES_PROGRAM_ID,
    PUMP_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
use shortwin::events::{
    Close, Expire, Liquidate, Open, PriceCranked, RouteChainChanged, SolSettle, TraderSwept,
    VenueStateChanged,
};
use shortwin::state::VenueKind;
use shortwin::venues::ExtrasSel;
use shortwin_litesvm_tests::engine::{
    Borrower, VenueMarket, close_short_ix, crank_price_ix, expire_ix, liquidate_ix, open_short_ix,
    sol_settle_ix,
};
use shortwin_litesvm_tests::errors::ok;
use shortwin_litesvm_tests::fixtures::MarketKeys;
use shortwin_litesvm_tests::fork::{self, PumpCoin, PumpCurve, PumpProgram, PumpV2};
use shortwin_litesvm_tests::pump::{self, CurveUser, PumpMarket, PumpSpec};
use shortwin_litesvm_tests::{Env, events, ix, pda};
use solana_account::Account;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;

/// A live legacy SPL-Token curve of 115 bytes without a creator.
const SPL: PumpCoin = PumpCoin::Legacy115;
/// A live Token-2022 curve of 115 bytes with a creator.
const T22: PumpCoin = PumpCoin::T22_115;
/// A live Token-2022 curve of 151 bytes near completion.
const NEAR: PumpCoin = PumpCoin::Near;
/// Both token programs.
const VARIANTS: [PumpCoin; 2] = [SPL, T22];

/// `MAX_INSTRUCTION_TRACE_LENGTH` (agave `transaction-context`; ARCH §6.2).
const MAX_TRACE: usize = 64;
/// `MAX_INSTRUCTION_STACK_DEPTH` on mainnet (SIMD-0268 inactive; ARCH §6.3).
const MAX_DEPTH: u8 = 5;

/// `ComputeBudget111111111111111111111111111111`.
const COMPUTE_BUDGET: Pubkey =
    Pubkey::from_str_const("ComputeBudget111111111111111111111111111111");

// ---- the shape of a transaction -------------------------------------------------------------

/// A program as the shapes name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum P {
    /// short.win itself (its `emit_cpi!` event self-CPI).
    Own,
    System,
    /// SPL Token or Token-2022.
    Token,
    /// pump (`6EF8rr…`), and its event self-CPI.
    Pump,
    /// pump-fees (`get_fees_with_quote_mint`).
    Pfee,
    /// The ATA program (the listing's `create_idempotent`).
    Ata,
    /// PumpSwap (`pAMMBay6…`), and its event self-CPI.
    Pamm,
    Other,
}

/// The inner instructions of top-level instruction `index` of the transaction `ixs` (paid by the
/// harness payer) as `(program, stack height)`.
fn shape(env: &Env, ixs: &[Instruction], meta: &TransactionMetadata, index: usize) -> Vec<(P, u8)> {
    let msg = Message::new(ixs, Some(&env.payer.pubkey()));
    let keys = msg.account_keys;
    meta.inner_instructions
        .get(index)
        .map(|list| {
            list.iter()
                .map(|i| {
                    let p = keys[usize::from(i.instruction.program_id_index)];
                    let kind = if p == shortwin::ID {
                        P::Own
                    } else if p == anchor_lang::system_program::ID {
                        P::System
                    } else if p == TOKEN_PROGRAM_ID || p == TOKEN_2022_PROGRAM_ID {
                        P::Token
                    } else if p == PUMP_PROGRAM_ID {
                        P::Pump
                    } else if p == PUMP_FEES_PROGRAM_ID {
                        P::Pfee
                    } else if p == ASSOCIATED_TOKEN_PROGRAM_ID {
                        P::Ata
                    } else if p == shortwin::constants::PUMP_AMM_PROGRAM_ID {
                        P::Pamm
                    } else {
                        P::Other
                    };
                    (kind, i.stack_height)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Entries of top-level instruction `index`: itself plus every CPI at any depth.
fn entries(meta: &TransactionMetadata, index: usize) -> usize {
    1 + meta.inner_instructions.get(index).map_or(0, Vec::len)
}

/// Entries of a whole transaction of `top_level` instructions (ARCH §6.2).
fn tx_entries(meta: &TransactionMetadata, top_level: usize) -> usize {
    top_level + meta.inner_instructions.iter().map(Vec::len).sum::<usize>()
}

/// The deepest stack height in the transaction (1 = top level).
fn peak(meta: &TransactionMetadata) -> u8 {
    meta.inner_instructions
        .iter()
        .flatten()
        .map(|i| i.stack_height)
        .max()
        .unwrap_or(1)
}

/// pump's `sell_v2` under short.win (ARCH §2.4.1 PumpCurve row): pump at height 2, its fee
/// lookup in pump-fees, the token transfer `trader_base_ata → curve_base_ata` and its event at
/// height 3 (the proceeds are credited by direct lamport moves: no inner instruction); an empty
/// creator vault adds pump's top-up System transfer from the trader after the token transfer and
/// before the proceeds are credited (T-RENT).
fn sell(top_up: bool) -> Vec<(P, u8)> {
    let mut v = vec![(P::Pump, 2), (P::Pfee, 3), (P::Token, 3)];
    if top_up {
        v.push((P::System, 3));
    }
    v.push((P::Pump, 3));
    v
}

/// The sell leg of an open: `q` from the lending vault into `trader_base_ata` (the market signs),
/// `ert` from the borrower to the trader, `sell_v2`, then `S` to the position and `ert` back to
/// the borrower (System transfers the trader signs).
fn sell_leg(top_up: bool) -> Vec<(P, u8)> {
    let mut v = vec![(P::Token, 2), (P::System, 2)];
    v.extend(sell(top_up));
    v.extend([(P::System, 2), (P::System, 2)]);
    v
}

/// pump's `buy_v2` under short.win with the trader PDA as `user` (T-PDA-USER): pump at height 2,
/// its fee lookup, the token transfer `curve_base_ata → trader_base_ata`, the System transfers
/// **from the trader** — the curve's net, the protocol fee, the buyback fee, the creator fee when
/// the curve has a creator, and the top-up of an empty creator vault — and its event at height 3;
/// a buy that completes the curve emits pump's second event (its completion event).
fn buy(creator_fee: bool, top_up: bool, completing: bool) -> Vec<(P, u8)> {
    let mut v = vec![(P::Pump, 2), (P::Pfee, 3), (P::Token, 3)];
    let transfers = 3 + usize::from(creator_fee) + usize::from(top_up);
    v.extend(std::iter::repeat_n((P::System, 3), transfers));
    v.extend(std::iter::repeat_n(
        (P::Pump, 3),
        1 + usize::from(completing),
    ));
    v
}

/// The buy leg of an exit: the position funds the trader by direct lamport moves, a 0-lamport
/// System transfer syncs both balances for the runtime's per-CPI check, `buy_v2`, the refund of
/// what is left to the position, and `q` from `trader_base_ata` to the lending vault.
fn buy_leg(creator_fee: bool, top_up: bool, completing: bool) -> Vec<(P, u8)> {
    let mut v = vec![(P::System, 2)];
    v.extend(buy(creator_fee, top_up, completing));
    v.extend([(P::System, 2), (P::Token, 2)]);
    v
}

/// Trader normalisation on the pump venues (ARCH §2.4 rule 5): the donated lamports of the
/// trader PDA to `market_vault`, the donated tokens of `trader_base_ata` to the lending vault,
/// and `TraderSwept`.
const NORMALISE: [(P, u8); 3] = [(P::System, 2), (P::Token, 2), (P::Own, 2)];

/// An open: `inits` System creations (the position; a first open also the exposure record), the
/// normalisation if any, the sell leg, the margin and fee transfers, the event.
fn open_shape(inits: usize, normalise: bool, top_up: bool) -> Vec<(P, u8)> {
    let mut v = vec![(P::System, 2); inits];
    if normalise {
        v.extend(NORMALISE);
    }
    v.extend(sell_leg(top_up));
    v.extend([(P::System, 2), (P::System, 2), (P::Own, 2)]);
    v
}

/// An exit: the normalisation if any, the buy leg, a completing buyback's `VenueStateChanged`
/// (`→ PumpMigrating` in the instruction), the exit's event.
fn exit_shape(normalise: bool, creator_fee: bool, top_up: bool, completing: bool) -> Vec<(P, u8)> {
    let mut v = if normalise {
        NORMALISE.to_vec()
    } else {
        Vec::new()
    };
    v.extend(buy_leg(creator_fee, top_up, completing));
    v.extend(std::iter::repeat_n(
        (P::Own, 2),
        1 + usize::from(completing),
    ));
    v
}

/// Prints and returns the shape of instruction `index`.
fn shown(
    env: &Env,
    what: &str,
    ixs: &[Instruction],
    meta: &TransactionMetadata,
    index: usize,
) -> Vec<(P, u8)> {
    let s = shape(env, ixs, meta, index);
    println!(
        "{what}: {} entries, peak {}, {} CU: {s:?}",
        entries(meta, index),
        peak(meta),
        meta.compute_units_consumed
    );
    s
}

/// Every shape is short.win's own CPIs at height 2 and the venue's at 2 (pump, PumpSwap, the ATA
/// program) with theirs at 3; returns the number of pump CPIs short.win made.
fn pump_calls(s: &[(P, u8)]) -> usize {
    assert!(
        s.iter().all(|&(p, h)| match p {
            P::Own => h == 2,
            P::System | P::Token => h == 2 || h == 3,
            P::Pump | P::Pamm | P::Ata => h == 2 || h == 3,
            P::Pfee => h == 3,
            P::Other => false,
        }),
        "{s:?}"
    );
    s.iter().filter(|&&(p, h)| p == P::Pump && h == 2).count()
}

// ---- scenarios ------------------------------------------------------------------------------

/// `vT/200`, the `pump_it` position size (0.5 % of the virtual token reserve).
fn q_of(env: &Env, c: &PumpCurve) -> u64 {
    c.state(&env.svm).virtual_token_reserves / 200
}

/// Lists `c`, lends `40·Q`, waits out the warm-up and cranks once.
fn add_market(env: &mut Env, c: PumpCurve) -> (PumpMarket, u64) {
    ok(env.list_pump(&c));
    let q = q_of(env, &c);
    let pm = env.pump_market_listed(PumpSpec::new(c.coin, q.saturating_mul(40)), c);
    env.crank(&pm);
    (pm, q)
}

fn setup(coin: PumpCoin) -> (Env, PumpMarket, u64) {
    let (mut env, c) = Env::fork_pump_coin(coin);
    let (pm, q) = add_market(&mut env, c);
    (env, pm, q)
}

fn open_ix(pm: &impl VenueMarket, b: &Pubkey, nonce: u64, q: u64) -> Instruction {
    open_short_ix(pm, b, nonce, q, 0, u64::MAX, u64::MAX, ExtrasSel::NONE, &[])
}

fn close_ix(pm: &impl VenueMarket, b: &Pubkey, nonce: u64) -> Instruction {
    close_short_ix(pm, b, nonce, u64::MAX, ExtrasSel::NONE, &[])
}

/// The client's transaction: ARCH §6.1's CU limit for a curve trade, then `ix`.
fn client(ix: Instruction) -> Vec<Instruction> {
    vec![pump::cu_limit_ix(pump::CURVE_TRADE_CU), ix]
}

/// The keeper's transaction: ARCH §6.1's CU limit for a curve exit, then `ix`.
fn keeper_tx(ix: Instruction) -> Vec<Instruction> {
    vec![pump::cu_limit_ix(pump::CURVE_KEEPER_EXIT_CU), ix]
}

/// A position of `q` two cranked seconds old.
fn with_position(coin: PumpCoin) -> (Env, PumpMarket, Borrower, u64, u64) {
    let (mut env, pm, q) = setup(coin);
    let mut b = env.borrower();
    let (res, nonce) = env.open(&pm, &mut b, q);
    ok(res);
    env.tick(&pm, 2);
    (env, pm, b, nonce, q)
}

/// A System transfer.
fn transfer(from: &Keypair, to: &Pubkey, lamports: u64) -> Instruction {
    let mut data = vec![2, 0, 0, 0];
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: anchor_lang::system_program::ID,
        accounts: vec![
            AccountMeta::new(from.pubkey(), true),
            AccountMeta::new(*to, false),
        ],
        data,
    }
}

/// Tokens from a curve user's (planted) balance to `to` (`TransferChecked`, the user signing).
fn donate_tokens(env: &mut Env, pm: &PumpMarket, to: &Pubkey, amount: u64) {
    let u = CurveUser::new(env, &pm.curve);
    fork::set_token_amount(&mut env.svm, &u.ata, amount);
    let mut data = vec![12u8];
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(6);
    let ix = Instruction {
        program_id: pm.keys.token_program,
        accounts: vec![
            AccountMeta::new(u.ata, false),
            AccountMeta::new_readonly(pm.keys.mint, false),
            AccountMeta::new(*to, false),
            AccountMeta::new_readonly(u.kp.pubkey(), true),
        ],
        data,
    };
    ok(env.send(&[ix], &[&u.kp]));
}

/// Lamports to the trader PDA and tokens to `trader_base_ata`, so the next leg normalises.
fn donate_to_trader(env: &mut Env, pm: &PumpMarket) {
    let griefer = env.funded_keypair();
    ok(env.send(&[transfer(&griefer, &pm.trader(), 1_000_000)], &[&griefer]));
    donate_tokens(env, pm, &pm.trader_base_ata, 777);
}

/// The curve's creator changed to a fresh key whose vault is empty (the adapter re-derives
/// T12 / T13; pump tops the new vault up from the trader, T-RENT).
fn change_creator(env: &mut Env, pm: &mut PumpMarket) {
    fork::set_curve_creator(&mut env.svm, &pm.curve.curve, &Pubkey::new_unique());
    pm.refresh_creator(&env.svm);
    assert_eq!(read_lamports(env, &pm.creator_vault), 0);
}

fn read_lamports(env: &Env, key: &Pubkey) -> u64 {
    env.svm.get_account(key).map_or(0, |a| a.lamports)
}

/// `simulate_transaction` of `ixs` (nothing is committed): its metadata if it would succeed.
fn simulate(env: &Env, ixs: &[Instruction], signers: &[&Keypair]) -> Option<TransactionMetadata> {
    let mut all: Vec<&Keypair> = vec![&env.payer];
    all.extend(signers.iter().copied());
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&env.payer.pubkey()),
        &all,
        env.svm.latest_blockhash(),
    );
    env.svm.simulate_transaction(tx).ok().map(|s| s.meta)
}

/// A third party buys `rT − rt_left` on the curve (a real `buy_v2`).
fn buy_down_to(env: &mut Env, c: &PumpCurve, rt_left: u64) {
    let u = CurveUser::new(env, c);
    let rt = c.state(&env.svm).real_token_reserves;
    ok(u.buy(env, c, rt - rt_left, u64::MAX));
}

/// A liquidatable position (40 % of `vT` bought, the keeper's simulated `[crank, liquidate]`
/// succeeds at the returned, uncranked second).
fn liquidatable(coin: PumpCoin) -> (Env, PumpMarket, Borrower, u64, Keypair) {
    let (mut env, pm, b, nonce, _) = with_position(coin);
    env.tick(&pm, 1);
    let u = CurveUser::new(&mut env, &pm.curve);
    let vt = pm.curve.state(&env.svm).virtual_token_reserves;
    ok(u.buy(&mut env, &pm.curve, vt / 10 * 4, u64::MAX));
    let keeper = env.funded_keypair();
    for _ in 0..900 {
        env.warp(1);
        let ixs = [
            pump::cu_limit_ix(pump::CURVE_KEEPER_EXIT_CU),
            crank_price_ix(&pm),
            liquidate_ix(&pm, &keeper.pubkey(), &b.key(), nonce),
        ];
        if simulate(&env, &ixs, &[&keeper]).is_some() {
            return (env, pm, b, nonce, keeper);
        }
        env.crank(&pm);
    }
    panic!("{coin:?}: never liquidatable");
}

/// An expired healthy position, three cranked seconds past its deadline.
fn expired(coin: PumpCoin) -> (Env, PumpMarket, Borrower, u64, Keypair) {
    let (mut env, pm, b, nonce, _) = with_position(coin);
    let deadline = env.position(&pm, &b.key(), nonce).deadline;
    let now = env.now();
    env.warp(deadline + 1 - now);
    env.tick(&pm, 3);
    let keeper = env.funded_keypair();
    (env, pm, b, nonce, keeper)
}

/// A list-only view of a pump market (the detection set as ReadSet).
struct AsNone<'a>(&'a PumpMarket);

impl VenueMarket for AsNone<'_> {
    fn keys(&self) -> &MarketKeys {
        &self.0.keys
    }
    fn read_set(&self, _writable: bool) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.0.curve.curve, false),
            AccountMeta::new_readonly(pda::canonical_pool(&self.0.keys.mint), false),
        ]
    }
    fn trade_extras(&self) -> Vec<AccountMeta> {
        Vec::new()
    }
}

/// The keeper's `[crank, sol_settle]` every 30 s until the simulation settles S3; the clock at
/// that second, uncranked.
fn until_s3(env: &mut Env, mm: &impl VenueMarket, keeper: &Keypair, b: &Pubkey, nonce: u64) {
    for _ in 0..200 {
        env.warp(30);
        let ixs = [
            crank_price_ix(mm),
            sol_settle_ix(env, mm, &keeper.pubkey(), b, nonce),
        ];
        if let Some(meta) = simulate(env, &ixs, &[keeper])
            && events::of::<SolSettle>(&meta)
                .first()
                .is_some_and(|e| e.condition == 3)
        {
            return;
        }
        ok(env.send(&[crank_price_ix(mm)], &[]));
    }
    panic!("S3 never due");
}

/// A stuck migration with a position: S3 due (70 min after `migration_seen_at`).
fn s3_migrating() -> (Env, PumpMarket, Borrower, u64, Keypair) {
    let (mut env, pm, b, nonce, _) = with_position(NEAR);
    let keeper = env.funded_keypair();
    let deadline = env.position(&pm, &b.key(), nonce).deadline;
    let now = env.now();
    env.warp(deadline - 1_800 - now);
    env.crank(&pm);
    env.warp(1);
    buy_down_to(&mut env, &pm.curve, 0);
    env.warp(1);
    env.crank(&pm);
    until_s3(&mut env, &pm, &keeper, &b.key(), nonce);
    (env, pm, b, nonce, keeper)
}

/// A curve that migrated elsewhere (`→ None`) with an expired position: S3 due on the detection
/// set.
fn s3_none() -> (Env, PumpMarket, Borrower, u64, Keypair) {
    let (mut env, pm, b, nonce, _) = with_position(NEAR);
    let keeper = env.funded_keypair();
    let deadline = env.position(&pm, &b.key(), nonce).deadline;
    let now = env.now();
    env.warp(deadline + 1 - now);
    env.crank(&pm);
    env.warp(1);
    buy_down_to(&mut env, &pm.curve, 0);
    env.warp(1);
    env.crank(&pm);
    fork::set_curve_reserves(&mut env.svm, &pm.curve.curve, 0, 0, 0, 0);
    env.warp(1);
    env.crank(&pm);
    assert_eq!(env.market(&pm.market()).venue_kind(), Some(VenueKind::None));
    until_s3(&mut env, &AsNone(&pm), &keeper, &b.key(), nonce);
    (env, pm, b, nonce, keeper)
}

fn send(env: &mut Env, ixs: &[Instruction], signers: &[&Keypair]) -> TransactionMetadata {
    ok(env.send(ixs, signers))
}

fn with_cb(ixs: Vec<Instruction>) -> Vec<Instruction> {
    let mut limit = vec![2u8];
    limit.extend_from_slice(&1_400_000u32.to_le_bytes());
    let mut price = vec![3u8];
    price.extend_from_slice(&1u64.to_le_bytes());
    let mut v = vec![
        Instruction {
            program_id: COMPUTE_BUDGET,
            accounts: vec![],
            data: limit,
        },
        Instruction {
            program_id: COMPUTE_BUDGET,
            accounts: vec![],
            data: price,
        },
    ];
    v.extend(ixs);
    v
}

/// Whether `coin`'s curve charges a creator fee (a creator is set): the buy's extra transfer.
fn has_creator(env: &Env, pm: &PumpMarket) -> bool {
    !pm.curve.state(&env.svm).creator_is_default()
}

// ---- per instruction ------------------------------------------------------------------------

/// `create_market(Auto)` of a live curve: `Market`, `MarketVault` and the lending vault
/// (creation and `InitializeAccount3`), the creation fee, the trader's base ATA through the ATA
/// program (its size query, creation, immutable owner and initialisation one level down), the
/// trader's pump UVA through pump's `init_user_volume_accumulator` (its creation and event one
/// level down), the event: 15 entries, peak 3. A coin on its canonical PumpSwap pool (M6) has the
/// same shape with pAMM's `init_user_volume_accumulator` in pump's place: 15, peak 3. A list-only
/// listing (a coin graduated elsewhere): 7, peak 2.
#[test]
fn create_market() {
    for coin in VARIANTS {
        let (mut env, c) = Env::fork_pump_coin(coin);
        let creator = env.funded_keypair();
        let ixs = [pump::create_pump_market_ix(&creator.pubkey(), &c)];
        let meta = send(&mut env, &ixs, &[&creator]);
        let s = shown(&env, &format!("{coin:?} create_market"), &ixs, &meta, 0);
        assert_eq!(
            s,
            [
                (P::System, 2),
                (P::System, 2),
                (P::System, 2),
                (P::Token, 2),
                (P::System, 2),
                (P::Ata, 2),
                (P::Token, 3),
                (P::System, 3),
                (P::Token, 3),
                (P::Token, 3),
                (P::Pump, 2),
                (P::System, 3),
                (P::Pump, 3),
                (P::Own, 2),
            ],
            "{coin:?}"
        );
        assert_eq!((entries(&meta, 0), peak(&meta)), (15, 3));
    }
    {
        let (mut env, c) = Env::fork_pump_coin(PumpCoin::Graduated);
        let creator = env.funded_keypair();
        let plain = ix::create_market(
            &creator.pubkey(),
            &c.mint,
            &c.token_program.id(),
            shortwin::instructions::create_market::VENUE_HINT_AUTO,
            &pda::canonical_pool(&c.mint),
        );
        let market = pda::market(&c.mint).0;
        let ixs = [ix::with_remaining(
            plain,
            &shortwin_litesvm_tests::pamm::pamm_init_set(&market),
        )];
        let meta = send(&mut env, &ixs, &[&creator]);
        let s = shown(&env, "Graduated create_market (PumpSwap)", &ixs, &meta, 0);
        assert_eq!(
            s,
            [
                (P::System, 2),
                (P::System, 2),
                (P::System, 2),
                (P::Token, 2),
                (P::System, 2),
                (P::Ata, 2),
                (P::Token, 3),
                (P::System, 3),
                (P::Token, 3),
                (P::Token, 3),
                (P::Pamm, 2),
                (P::System, 3),
                (P::Pamm, 3),
                (P::Own, 2),
            ]
        );
        assert_eq!((entries(&meta, 0), peak(&meta)), (15, 3));
    }
    {
        let coin = PumpCoin::Fartcoin;
        let (mut env, c) = Env::fork_pump_coin(coin);
        let creator = env.funded_keypair();
        let ixs = [ix::create_market(
            &creator.pubkey(),
            &c.mint,
            &c.token_program.id(),
            shortwin::instructions::create_market::VENUE_HINT_AUTO,
            &pda::canonical_pool(&c.mint),
        )];
        let meta = send(&mut env, &ixs, &[&creator]);
        let s = shown(
            &env,
            &format!("{coin:?} create_market (None)"),
            &ixs,
            &meta,
            0,
        );
        assert_eq!(
            s,
            [
                (P::System, 2),
                (P::System, 2),
                (P::System, 2),
                (P::Token, 2),
                (P::System, 2),
                (P::Own, 2)
            ],
            "{coin:?}: the list-only listing creates no venue account"
        );
        assert_eq!((entries(&meta, 0), peak(&meta)), (7, 2));
    }
}

/// `crank_price` on the curve: roll + record is its `PriceCranked` (2 entries); a transition adds
/// `VenueStateChanged` — `PumpCurve → PumpMigrating` also starts the chains (`RouteChainChanged`:
/// 4 entries), `PumpMigrating → None` keeps them running (3); the `None → PumpCurve`
/// re-detection 4; a `None` market's crank 2.
#[test]
fn crank_price() {
    for coin in VARIANTS {
        let (mut env, pm, _) = setup(coin);
        env.warp(1);
        let ixs = [crank_price_ix(&pm)];
        let meta = send(&mut env, &ixs, &[]);
        let s = shown(&env, &format!("{coin:?} crank_price"), &ixs, &meta, 0);
        assert_eq!(s, [(P::Own, 2)], "roll + record: the event only");
        let _: PriceCranked = events::one(&meta);
    }

    // PumpCurve → PumpMigrating (a third party completed the curve), a position open.
    let (mut env, pm, _, _, _) = with_position(NEAR);
    buy_down_to(&mut env, &pm.curve, 0);
    env.warp(1);
    let ixs = [crank_price_ix(&pm)];
    let meta = send(&mut env, &ixs, &[]);
    let s = shown(&env, "crank_price → PumpMigrating", &ixs, &meta, 0);
    let ev: VenueStateChanged = events::one(&meta);
    assert_eq!(ev.to, VenueKind::PumpMigrating as u8);
    assert_eq!(
        events::of::<RouteChainChanged>(&meta).len(),
        1,
        "the chains start"
    );
    let _: PriceCranked = events::one(&meta);
    assert_eq!(s, [(P::Own, 2); 3], "{s:?}");
    assert_eq!((entries(&meta, 0), peak(&meta)), (4, 2));

    // PumpMigrating → None (migrated elsewhere).
    fork::set_curve_reserves(&mut env.svm, &pm.curve.curve, 0, 0, 0, 0);
    env.warp(1);
    let meta = send(&mut env, &ixs, &[]);
    let s = shown(&env, "crank_price → None", &ixs, &meta, 0);
    let ev: VenueStateChanged = events::one(&meta);
    assert_eq!(ev.to, VenueKind::None as u8);
    assert!(
        events::of::<RouteChainChanged>(&meta).is_empty(),
        "the chains run on"
    );
    let _: PriceCranked = events::one(&meta);
    assert_eq!(s, [(P::Own, 2); 2], "{s:?}");

    // A None market's crank on its detection set.
    env.warp(1);
    let none = [crank_price_ix(&AsNone(&pm))];
    let meta = send(&mut env, &none, &[]);
    let s = shown(&env, "crank_price (None)", &none, &meta, 0);
    assert_eq!(s, [(P::Own, 2)], "{s:?}");

    // None → PumpCurve: a mint listed before its curve was readable, re-detected by a crank.
    let (mut env, c) = Env::fork_pump_coin(T22);
    let saved = env.svm.get_account(&c.curve).unwrap();
    env.svm.set_account(c.curve, Account::default()).unwrap();
    let creator = env.funded_keypair();
    let list = ix::create_market(
        &creator.pubkey(),
        &c.mint,
        &c.token_program.id(),
        shortwin::instructions::create_market::VENUE_HINT_AUTO,
        &pda::canonical_pool(&c.mint),
    );
    ok(env.send(&[list], &[&creator]));
    env.svm.set_account(c.curve, saved).unwrap();
    let vt = c.state(&env.svm).virtual_token_reserves;
    let pm = env.pump_market_listed(PumpSpec::new(T22, (vt / 200) * 40), c);
    let ixs = [crank_price_ix(&AsNone(&pm))];
    let meta = send(&mut env, &ixs, &[]);
    let s = shown(&env, "crank_price None → PumpCurve", &ixs, &meta, 0);
    let ev: VenueStateChanged = events::one(&meta);
    assert_eq!(ev.to, VenueKind::PumpCurve as u8);
    let _: PriceCranked = events::one(&meta);
    assert_eq!(
        events::of::<RouteChainChanged>(&meta).len(),
        1,
        "the chains cleared"
    );
    assert_eq!(s, [(P::Own, 2); 3], "{s:?}");
    assert_eq!((entries(&meta, 0), peak(&meta)), (4, 2));
}

/// `open_short` on the curve: 14 entries for a wallet's first position, 13 for a repeat, 17
/// normalising, 18 with pump's top-up of an empty creator vault; one pump CPI, peak 3.
#[test]
fn open_short() {
    for coin in VARIANTS {
        let (mut env, pm, q) = setup(coin);
        env.tick(&pm, 1);
        let b = env.borrower();
        let first = client(open_ix(&pm, &b.key(), 0, q));
        let meta = send(&mut env, &first, &[&b.kp]);
        let _: Open = events::one(&meta);
        let s = shown(&env, &format!("{coin:?} open (first)"), &first, &meta, 1);
        assert_eq!(s, open_shape(2, false, false), "{coin:?}");
        assert_eq!(pump_calls(&s), 1);
        assert_eq!((entries(&meta, 1), peak(&meta)), (14, 3));

        let repeat = client(open_ix(&pm, &b.key(), 1, q));
        let meta = send(&mut env, &repeat, &[&b.kp]);
        let s = shown(&env, &format!("{coin:?} open (repeat)"), &repeat, &meta, 1);
        assert_eq!(s, open_shape(1, false, false), "{coin:?}");
        assert_eq!(
            (entries(&meta, 1), peak(&meta)),
            (13, 3),
            "the exposure exists"
        );

        env.warp(1);
        donate_to_trader(&mut env, &pm);
        let c = env.borrower();
        let normalising = client(open_ix(&pm, &c.key(), 0, q));
        let meta = send(&mut env, &normalising, &[&c.kp]);
        let _: TraderSwept = events::one(&meta);
        let s = shown(
            &env,
            &format!("{coin:?} open (normalising)"),
            &normalising,
            &meta,
            1,
        );
        assert_eq!(s, open_shape(2, true, false), "{coin:?}");
        assert_eq!((entries(&meta, 1), peak(&meta)), (17, 3));
    }

    // The creator-vault top-up (T-RENT) inside the sale: a creator change into an empty vault
    // and a sale large enough to cover it within exec_tol_bps.
    let (mut env, mut pm, _) = setup(NEAR);
    let q = pm.curve.state(&env.svm).virtual_token_reserves / 100;
    change_creator(&mut env, &mut pm);
    env.tick(&pm, 1);
    donate_to_trader(&mut env, &pm);
    let b = env.borrower();
    let ixs = client(open_ix(&pm, &b.key(), 0, q));
    let meta = send(&mut env, &ixs, &[&b.kp]);
    let s = shown(
        &env,
        "open (normalising, creator-vault top-up)",
        &ixs,
        &meta,
        1,
    );
    assert_eq!(s, open_shape(2, true, true));
    assert_eq!((entries(&meta, 1), peak(&meta)), (18, 3));
    assert!(read_lamports(&env, &pm.creator_vault) > 0, "topped up");
}

/// Exits on the curve: a close is its buy leg and event — 12 entries without a creator fee, 13
/// with one; 15 / 16 normalising; 17 with the creator-vault top-up inside `B`; a completing
/// buyback adds pump's completion event and short.win's `VenueStateChanged` (15). One pump CPI,
/// peak 3.
#[test]
fn close_short() {
    for coin in VARIANTS {
        let (mut env, pm, b, nonce, _) = with_position(coin);
        let creator = has_creator(&env, &pm);
        assert_eq!(creator, coin != SPL, "legacy115 has no creator");
        let ixs = client(close_ix(&pm, &b.key(), nonce));
        let meta = send(&mut env, &ixs, &[&b.kp]);
        let _: Close = events::one(&meta);
        let s = shown(&env, &format!("{coin:?} close"), &ixs, &meta, 1);
        assert_eq!(s, exit_shape(false, creator, false, false), "{coin:?}");
        assert_eq!(pump_calls(&s), 1);
        assert_eq!(
            (entries(&meta, 1), peak(&meta)),
            (12 + usize::from(creator), 3)
        );

        let (mut env, pm, b, nonce, _) = with_position(coin);
        donate_to_trader(&mut env, &pm);
        let ixs = client(close_ix(&pm, &b.key(), nonce));
        let meta = send(&mut env, &ixs, &[&b.kp]);
        let s = shown(
            &env,
            &format!("{coin:?} close (normalising)"),
            &ixs,
            &meta,
            1,
        );
        assert_eq!(s, exit_shape(true, creator, false, false), "{coin:?}");
        assert_eq!(entries(&meta, 1), 15 + usize::from(creator));
    }

    // The top-up of an empty creator vault inside the buy (normalising, T22 with a creator).
    let (mut env, mut pm, b, nonce, _) = with_position(T22);
    change_creator(&mut env, &mut pm);
    donate_to_trader(&mut env, &pm);
    let ixs = client(close_ix(&pm, &b.key(), nonce));
    let meta = send(&mut env, &ixs, &[&b.kp]);
    let s = shown(&env, "close (normalising, top-up)", &ixs, &meta, 1);
    assert_eq!(s, exit_shape(true, true, true, false));
    assert_eq!((entries(&meta, 1), peak(&meta)), (17, 3));

    // A completing buyback: `→ PumpMigrating` in the instruction, its events before `Close`.
    let (mut env, pm, b, nonce, q) = with_position(NEAR);
    buy_down_to(&mut env, &pm.curve, q);
    env.warp(1);
    let ixs = client(close_ix(&pm, &b.key(), nonce));
    let meta = send(&mut env, &ixs, &[&b.kp]);
    let ev: VenueStateChanged = events::one(&meta);
    assert_eq!(ev.to, VenueKind::PumpMigrating as u8);
    let s = shown(&env, "close (completing)", &ixs, &meta, 1);
    assert_eq!(s, exit_shape(false, true, false, true), "{s:?}");
    assert_eq!((entries(&meta, 1), peak(&meta)), (15, 3));
}

/// A liquidation after a real pump and an expiry at the deadline: the exit shape (12 / 13).
#[test]
fn liquidate_and_expire() {
    for coin in VARIANTS {
        let (mut env, pm, b, nonce, keeper) = liquidatable(coin);
        env.crank(&pm);
        let creator = has_creator(&env, &pm);
        let ixs = keeper_tx(liquidate_ix(&pm, &keeper.pubkey(), &b.key(), nonce));
        let meta = send(&mut env, &ixs, &[&keeper]);
        let _: Liquidate = events::one(&meta);
        let s = shown(&env, &format!("{coin:?} liquidate"), &ixs, &meta, 1);
        assert_eq!(s, exit_shape(false, creator, false, false), "{coin:?}");
        assert_eq!(
            (entries(&meta, 1), peak(&meta)),
            (12 + usize::from(creator), 3)
        );

        let (mut env, pm, b, nonce, keeper) = expired(coin);
        let ixs = keeper_tx(expire_ix(&pm, &keeper.pubkey(), &b.key(), nonce));
        let meta = send(&mut env, &ixs, &[&keeper]);
        let _: Expire = events::one(&meta);
        let s = shown(&env, &format!("{coin:?} expire"), &ixs, &meta, 1);
        assert_eq!(s, exit_shape(false, creator, false, false), "{coin:?}");
        assert_eq!(
            (entries(&meta, 1), peak(&meta)),
            (12 + usize::from(creator), 3)
        );
    }
}

/// `sol_settle`: the healthy no-op emits nothing (1 entry, peak 1); S3 on `PumpMigrating` (the
/// stuck migration at 70 min) and on `None` (graduated elsewhere, the detection set) pay in SOL:
/// their event only (2 entries, peak 2), no venue CPI.
#[test]
fn sol_settle() {
    for coin in VARIANTS {
        let (mut env, pm, b, nonce, _) = with_position(coin);
        let keeper = env.funded_keypair();
        let ixs = [sol_settle_ix(&env, &pm, &keeper.pubkey(), &b.key(), nonce)];
        let meta = send(&mut env, &ixs, &[&keeper]);
        let s = shown(
            &env,
            &format!("{coin:?} sol_settle (no-op)"),
            &ixs,
            &meta,
            0,
        );
        assert!(s.is_empty(), "the healthy no-op emits nothing: {s:?}");
        assert_eq!(peak(&meta), 1);
    }
    let (mut env, pm, b, nonce, keeper) = s3_migrating();
    env.crank(&pm);
    let ixs = [sol_settle_ix(&env, &pm, &keeper.pubkey(), &b.key(), nonce)];
    let meta = send(&mut env, &ixs, &[&keeper]);
    assert_eq!(events::one::<SolSettle>(&meta).condition, 3);
    let s = shown(&env, "sol_settle S3 (PumpMigrating)", &ixs, &meta, 0);
    assert_eq!(s, [(P::Own, 2)]);
    assert_eq!((entries(&meta, 0), peak(&meta)), (2, 2));

    let (mut env, pm, b, nonce, keeper) = s3_none();
    let none = AsNone(&pm);
    env.crank(&none);
    let ixs = [sol_settle_ix(
        &env,
        &none,
        &keeper.pubkey(),
        &b.key(),
        nonce,
    )];
    let meta = send(&mut env, &ixs, &[&keeper]);
    assert_eq!(events::one::<SolSettle>(&meta).condition, 3);
    let s = shown(&env, "sol_settle S3 (None)", &ixs, &meta, 0);
    assert_eq!(s, [(P::Own, 2)]);
    assert_eq!((entries(&meta, 0), peak(&meta)), (2, 2));
}

/// `sweep_trader`'s pump token leg (ARCH §3.9, M5): the trader's lamports to `market_vault`, the
/// tokens of `trader_base_ata` to the lending vault, the event (4 entries, peak 2); nothing to
/// sweep: the event only.
#[test]
fn sweep_trader() {
    for coin in VARIANTS {
        let (mut env, pm, _) = setup(coin);
        donate_to_trader(&mut env, &pm);
        let ixs = [pump::sweep_trader_pump_ix(&env, &pm)];
        let meta = send(&mut env, &ixs, &[]);
        let ev: TraderSwept = events::one(&meta);
        assert_eq!((ev.tokens, ev.lamports), (777, 1_000_000));
        let s = shown(&env, &format!("{coin:?} sweep_trader"), &ixs, &meta, 0);
        assert_eq!(s, [(P::System, 2), (P::Token, 2), (P::Own, 2)]);
        assert_eq!((entries(&meta, 0), peak(&meta)), (4, 2));
        env.warp(1);
        let ixs = [pump::sweep_trader_pump_ix(&env, &pm)];
        let meta = send(&mut env, &ixs, &[]);
        let s = shown(
            &env,
            &format!("{coin:?} sweep_trader (empty)"),
            &ixs,
            &meta,
            0,
        );
        assert_eq!(s, [(P::Own, 2)]);
    }
}

/// The latch instructions on the dumped pump, pump-fees and PumpSwap ProgramData: their event
/// only (2 entries, peak 2).
#[test]
fn latch_instructions() {
    let (mut env, _) = Env::fork_pump_coin(T22);
    let ea = env.pause_authority.insecure_clone();
    for (program, venue) in [
        (PumpProgram::Pump, GUARD_PUMP),
        (PumpProgram::Fees, GUARD_PUMP_FEES),
        (PumpProgram::Amm, GUARD_PUMP_AMM),
    ] {
        let dumped = fork::FIXTURE_GUARD_SLOTS[usize::from(venue)];
        fork::set_pump_deploy_slot(&mut env.svm, program, dumped + 1);
        let ixs = [ix::flag_venue_upgraded(venue)];
        let meta = send(&mut env, &ixs, &[]);
        let s = shown(
            &env,
            &format!("{program:?} flag_venue_upgraded"),
            &ixs,
            &meta,
            0,
        );
        assert_eq!(s, [(P::Own, 2)]);
        let ixs = [ix::revalidate_venue(&ea.pubkey(), venue, dumped + 1)];
        let meta = send(&mut env, &ixs, &[&ea]);
        let s = shown(
            &env,
            &format!("{program:?} revalidate_venue"),
            &ixs,
            &meta,
            0,
        );
        assert_eq!(s, [(P::Own, 2)]);
    }
}

// ---- worst-case transactions ----------------------------------------------------------------

/// Sends a worst-case transaction; asserts ≤ 64 entries and the stack peak; returns the
/// entries.
fn worst(env: &mut Env, what: &str, ixs: &[Instruction], signers: &[&Keypair], depth: u8) -> usize {
    let meta = send(env, ixs, signers);
    let n = tx_entries(&meta, ixs.len());
    println!(
        "{what}: {n} entries, peak {}, {} CU",
        peak(&meta),
        meta.compute_units_consumed
    );
    assert!(n <= MAX_TRACE, "{what}: {n} entries");
    assert_eq!(peak(&meta), depth, "{what}");
    assert!(peak(&meta) <= MAX_DEPTH);
    n
}

/// The largest client transactions: a wallet's first open, normalising, with the creator-vault
/// top-up (2 + 18) and a normalising close with the top-up (2 + 17); an atomic sandwich of a
/// normalising open with the curve's own trades (a user's `buy_v2` 7 entries, 8 with a creator
/// fee; `sell_v2` 4).
#[test]
fn worst_case_client_transactions() {
    let (mut env, mut pm, _) = setup(NEAR);
    let q = pm.curve.state(&env.svm).virtual_token_reserves / 100;
    change_creator(&mut env, &mut pm);
    env.tick(&pm, 1);
    donate_to_trader(&mut env, &pm);
    let b = env.borrower();
    let ixs = with_cb(vec![open_ix(&pm, &b.key(), 0, q)]);
    let n = worst(
        &mut env,
        "CB×2 + open (normalising, top-up)",
        &ixs,
        &[&b.kp],
        3,
    );
    assert_eq!(n, 2 + 18);
    env.tick(&pm, 1);
    change_creator(&mut env, &mut pm);
    donate_to_trader(&mut env, &pm);
    let ixs = with_cb(vec![close_ix(&pm, &b.key(), 0)]);
    let n = worst(
        &mut env,
        "CB×2 + close (normalising, top-up)",
        &ixs,
        &[&b.kp],
        3,
    );
    assert_eq!(n, 2 + 17);

    for coin in VARIANTS {
        let (mut env, pm, q) = setup(coin);
        let attacker = env.funded_keypair();
        let payer = env.payer.pubkey();
        let program = pm.curve.token_program.id();
        ok(env.send(
            &[
                fork::create_ata_idempotent_ix(&payer, &attacker.pubkey(), &pm.keys.mint, &program),
                fork::pump_init_uva_ix(&payer, &attacker.pubkey()),
            ],
            &[],
        ));
        let ata = pda::ata(&attacker.pubkey(), &pm.keys.mint, &program);
        let x = pm.curve.state(&env.svm).virtual_token_reserves / 50;
        let fr = fork::pump_fee_recipient(&env.svm, 1);
        let bb = fork::pump_buyback_fee_recipient(&env.svm, 1);
        let a = attacker.pubkey();
        let front = pm
            .curve
            .v2_ix(&env.svm, PumpV2::Buy, &a, &ata, &fr, &bb, x, u64::MAX);
        let back = pm
            .curve
            .v2_ix(&env.svm, PumpV2::Sell, &a, &ata, &fr, &bb, x, 0);
        env.tick(&pm, 1);
        donate_to_trader(&mut env, &pm);
        let v = env.borrower();
        let creator = has_creator(&env, &pm);
        let ixs = with_cb(vec![front, open_ix(&pm, &v.key(), 0, q), back]);
        let n = worst(
            &mut env,
            &format!("{coin:?} CB×2 + [buy_v2, open (normalising), sell_v2]"),
            &ixs,
            &[&attacker, &v.kp],
            3,
        );
        assert_eq!(n, 2 + (7 + usize::from(creator)) + 17 + 4);
    }
}

/// Four opens of one wallet fit the trace (2 + 14 + 3·13 = 55); a fifth (68) is refused by the
/// runtime itself with `MaxInstructionTraceLengthExceeded`, before any hook could blame
/// short.win. (Each of `vT/800` on the near-complete curve: four stay under the wallet cap.)
#[test]
fn the_most_opens_of_one_wallet() {
    let (mut env, pm, _) = setup(NEAR);
    let q = pm.curve.state(&env.svm).virtual_token_reserves / 800;
    env.tick(&pm, 1);
    let b = env.borrower();
    let opens = |k: u64| with_cb((0..k).map(|n| open_ix(&pm, &b.key(), n, q)).collect());
    let res = env.send(&opens(5), &[&b.kp]);
    let err = res.expect_err("68 entries");
    assert!(
        format!("{:?}", err.err).contains("MaxInstructionTraceLengthExceeded"),
        "{:?}",
        err.err
    );
    let n = worst(&mut env, "CB×2 + open ×4", &opens(4), &[&b.kp], 3);
    assert_eq!(n, 2 + 14 + 3 * 13);
}

/// A keeper batch of four cranks on the four live curves of the dump, each just completed by a
/// third party: every crank moves its market to `PumpMigrating` (4 entries): 2 + 4·4 = 18.
#[test]
fn a_crank_batch_of_four_transitions() {
    let (mut env, first) = Env::fork_pump_coin(SPL);
    let mut markets = vec![add_market(&mut env, first).0];
    for coin in [T22, PumpCoin::Fresh, NEAR] {
        let c = env.load_pump_coin(coin);
        if coin == PumpCoin::Fresh {
            // As `pump_it`: the fresh curve's `rQ` is one lamport; buys first.
            let u = CurveUser::new(&mut env, &c);
            let q = u64::try_from(c.curve_state(&env.svm).vt / 7).unwrap();
            ok(u.buy(&mut env, &c, q, u64::MAX));
        }
        markets.push(add_market(&mut env, c).0);
    }
    for pm in &markets {
        env.crank(pm);
        buy_down_to(&mut env, &pm.curve, 0);
    }
    env.warp(1);
    let ixs = with_cb(markets.iter().map(crank_price_ix).collect());
    let n = worst(
        &mut env,
        "CB×2 + crank_price ×4 (→ PumpMigrating)",
        &ixs,
        &[],
        2,
    );
    assert_eq!(n, 2 + 4 * 4);
    for pm in &markets {
        assert_eq!(
            env.market(&pm.market()).venue_kind(),
            Some(VenueKind::PumpMigrating)
        );
    }
}

/// The keeper's exits with their crank: `[CB×2, crank, liquidate | expire]` (2 + 2 + 12 / 13),
/// `[CB×2, crank, sol_settle S3]` on `PumpMigrating` and on `None` (2 + 2 + 2); the latch duty
/// (§7.1 duty 10): the three pump programs flagged in one transaction (2 + 3·2).
#[test]
fn worst_case_keeper_transactions() {
    for coin in VARIANTS {
        let (mut env, pm, b, nonce, keeper) = liquidatable(coin);
        let creator = usize::from(has_creator(&env, &pm));
        let ixs = with_cb(vec![
            crank_price_ix(&pm),
            liquidate_ix(&pm, &keeper.pubkey(), &b.key(), nonce),
        ]);
        let n = worst(
            &mut env,
            &format!("{coin:?} CB×2 + crank + liquidate"),
            &ixs,
            &[&keeper],
            3,
        );
        assert_eq!(n, 2 + 2 + 12 + creator);

        let (mut env, pm, b, nonce, keeper) = expired(coin);
        env.warp(1);
        let ixs = with_cb(vec![
            crank_price_ix(&pm),
            expire_ix(&pm, &keeper.pubkey(), &b.key(), nonce),
        ]);
        let n = worst(
            &mut env,
            &format!("{coin:?} CB×2 + crank + expire"),
            &ixs,
            &[&keeper],
            3,
        );
        assert_eq!(n, 2 + 2 + 12 + creator);
    }

    let (mut env, pm, b, nonce, keeper) = s3_migrating();
    let ixs = with_cb(vec![
        crank_price_ix(&pm),
        sol_settle_ix(&env, &pm, &keeper.pubkey(), &b.key(), nonce),
    ]);
    let n = worst(
        &mut env,
        "CB×2 + crank + sol_settle S3 (PumpMigrating)",
        &ixs,
        &[&keeper],
        2,
    );
    assert_eq!(n, 2 + 2 + 2);

    let (mut env, pm, b, nonce, keeper) = s3_none();
    let none = AsNone(&pm);
    let ixs = with_cb(vec![
        crank_price_ix(&none),
        sol_settle_ix(&env, &none, &keeper.pubkey(), &b.key(), nonce),
    ]);
    let n = worst(
        &mut env,
        "CB×2 + crank + sol_settle S3 (None)",
        &ixs,
        &[&keeper],
        2,
    );
    assert_eq!(n, 2 + 2 + 2);

    let (mut env, _) = Env::fork_pump_coin(T22);
    for (program, venue) in [
        (PumpProgram::Pump, GUARD_PUMP),
        (PumpProgram::Fees, GUARD_PUMP_FEES),
        (PumpProgram::Amm, GUARD_PUMP_AMM),
    ] {
        let dumped = fork::FIXTURE_GUARD_SLOTS[usize::from(venue)];
        fork::set_pump_deploy_slot(&mut env.svm, program, dumped + 1);
    }
    let ixs = with_cb(vec![
        ix::flag_venue_upgraded(GUARD_PUMP),
        ix::flag_venue_upgraded(GUARD_PUMP_FEES),
        ix::flag_venue_upgraded(GUARD_PUMP_AMM),
    ]);
    let n = worst(
        &mut env,
        "CB×2 + flag_venue_upgraded ×3 (pump)",
        &ixs,
        &[],
        2,
    );
    assert_eq!(n, 2 + 3 * 2);
}

/// The coins the shapes name: SPL Token without a creator, Token-2022 with one.
#[test]
fn variants_cover_both_token_programs() {
    let programs: Vec<Pubkey> = VARIANTS
        .iter()
        .map(|c| Env::fork_pump_coin(*c).1.token_program.id())
        .collect();
    assert_eq!(programs, [TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID]);
}
