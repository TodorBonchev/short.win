//! T-STRADDLE, curve part (ARCH §5.4 case 2, §6.1, §6.2, §6.4, §8.2 T-STRADDLE, §12 U8 / U22;
//! A14, A66; R3B-11): the `complete-curve` helper (`programs/complete_curve`) and the straddle
//! bundle's first two transactions on the mainnet fork — the release short.win, the **dumped**
//! pump, pump-fees and PumpSwap ELFs, the helper's own SBF build — with the invariant hooks after
//! every instruction (every transaction goes through `Env::send`).
//!
//! The bundle of ARCH §5.4 for a position whose `Q` exceeds the curve's `rT` (pump reverts any buy
//! above `rT` with 6021 and never fills partially):
//!
//! | tx | instructions | here |
//! |---|---|---|
//! | tx1 | ComputeBudget ×2 + `complete_curve(max_sol_cost)` | executed: reads `rT` at execution, `buy_v2(rT_live)` by the signer; a no-op on a complete curve |
//! | tx2 | ComputeBudget ×2 + `migrate_v2` top-level (+ the Jito tip when it is the bundle's last) | executed on the real pump / PumpSwap ELFs |
//! | tx3 | ComputeBudget ×2 + the pool accounts the existence requirement names (PumpSwap UVA, coin-creator vault ATA, recipients' ATAs) + `crank_price` (→ PumpSwap) + `crank_price` (the pool's first observation) (+ the tip) — or a borrower's `close_short` after the transition crank | **M6**: executed in the bundle; the keeper's exit follows at the post-change warm-up's gate (below) |
//!
//! **When the exit on the pool lands (M6 as built; ARCH §5.4 puts it in tx3).** The `→ PumpSwap`
//! transition marks the post-change warm-up (`ema_warm = 8`), and a keeper's `liquidate` /
//! `expire` fails `EmaWarmingUp` until the prologue's roll integrates the eighth post-transition
//! bucket; one bucket per observed second, the transition itself recording none. So:
//!
//! - a tx3 with the keeper's exit reverts the **whole** bundle (`EmaWarmingUp`), nothing committed;
//! - tx3 = the accounts, the transition crank and the pool's first observation (the keeper's, bucket
//!   `T`) lands; with an observation in every second after, the exit lands at **`T + 8`**, and not
//!   one second earlier; without tx3's own observation at `T + 9`; every second without an
//!   observation delays it by one;
//! - the borrower's own `close_short` is not gated: it lands in tx3 after the transition crank.
//!
//! A Jito bundle executes its transactions "sequentially and atomically, all-or-nothing"
//! (U22, docs.jito.wtf): [`bundle`] runs them in order through `Env::send` and restores the VM
//! if one fails.
//!
//! What is checked:
//!
//! - tx1 completes a live curve **at the `rT` it reads when it runs** (the completer holds
//!   exactly that many tokens, `CurveCompleted` reports them and the lamports spent), and is a
//!   no-op on a curve a third party completed or migrated first (no CPI at all);
//! - ★ **a curve trade between the keeper's read and the bundle** (a buy, a sale, a third-party
//!   completion, a third-party completion and migration): the bundle built on the stale read
//!   still lands, the pool `migrate_v2` creates is the one the read predicted (`X_p` does not
//!   depend on the path), and v0.3's tx1 (`buy_v2(rT_read)` direct) reverts the whole bundle in
//!   the same situations — 6021 after a buy, `migrate_v2` on an incomplete curve after a sale —
//!   leaving every account as it was (R3B-11);
//! - `max_sol_cost` bounds what the completer pays: a sale that raises the completion cost above
//!   it reverts the bundle at tx1 (pump 6002), nothing committed;
//! - the whole path for a straddled short.win position: the close refuses `CurveStraddle`, the
//!   program's `B̂_str` is stable across a curve trade (the post-migration pool is path
//!   independent), the bundle leaves every short.win account untouched, and tx3 (M6) — the
//!   venue accounts, then `crank_price` moving the market `PumpCurve → PumpSwap` and the
//!   borrower's `close_short` on the pool in one transaction — buys `Q` on the new pool for at
//!   most `B̂_str`;
//! - a borrower's self-made straddle (it buys `rT − q + 1` itself; M5-SEC-4): no exit on the
//!   curve, the bundle, the pool pumped ×3 before any observation, tx3's crank `→ PumpSwap`, and
//!   the keeper's liquidation on the pool once the warm-up has run;
//! - ★ the whole three-transaction bundle, tx3 built before the trade, against a curve trade
//!   between the keeper's read and the bundle (a buy, a sale, a third-party completion, a
//!   completion and migration): it lands and moves the market onto the pool every time; a third
//!   party's own transition crank in between makes tx3 refuse (`VenueAccountMismatch`: the curve's
//!   ReadSet on a PumpSwap market) and the bundle revert — nothing lost, the market is on its pool;
//! - the helper's own checks (A66): the CPI target must be pump, the curve pump's PDA of the
//!   mint, `user` a signer — short.win's `trader` PDA cannot be named as a non-signing `user`;
//!   trailing accounts pass through to pump (an additive pump interface change);
//! - T-TRACE / T-CU / T-SIZE per transaction (the table with `-- --nocapture`): trace entries,
//!   peak stack height, compute units, v0 size through the protocol ALT (A28) and inline.
//!
//! The helper's build: `anchor build --tools-version v1.57 --arch v3` writes
//! `target/deploy/complete_curve.so`; `$COMPLETE_CURVE_SO` overrides it (as `$SHORTWIN_SO` does
//! short.win's). Its id and instruction are the harness's own (`shortwin_litesvm_tests::pump::
//! {COMPLETE_CURVE_ID, load_complete_curve, complete_curve_ix}`; the harness does not link the
//! helper crate).

// `TxResult` is LiteSVM's own `TransactionResult` (as in the harness library).
#![allow(clippy::result_large_err)]

use anchor_lang::__private::base64::{self, Engine};
use anchor_lang::error::ErrorCode as AnchorErrorCode;
use anchor_lang::prelude::{Pubkey, pubkey};
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use litesvm::types::{FailedTransactionMetadata, TransactionMetadata};
use shortwin::constants::{
    PUMP_AMM_EVENT_AUTHORITY, PUMP_AMM_FEE_CONFIG, PUMP_AMM_GLOBAL_CONFIG,
    PUMP_AMM_GLOBAL_VOLUME_ACCUMULATOR, PUMP_AMM_PROGRAM_ID, PUMP_EVENT_AUTHORITY, PUMP_FEE_CONFIG,
    PUMP_FEES_PROGRAM_ID, PUMP_GLOBAL, PUMP_GLOBAL_VOLUME_ACCUMULATOR, PUMP_PROGRAM_ID,
};
use shortwin::errors::ShortwinError as E;
use shortwin::events::VenueStateChanged;
use shortwin::state::VenueKind;
use shortwin_litesvm_tests::engine::{Borrower, VenueMarket, crank_price_ix};
use shortwin_litesvm_tests::errors::{assert_err, custom_code, ok};
use shortwin_litesvm_tests::fork::{self, PumpCoin, PumpCurve, PumpV2};
use shortwin_litesvm_tests::pump::{self, CurveUser, PumpMarket, PumpSpec};
use shortwin_litesvm_tests::{Env, TxResult, events, pda, raydium, read};
use solana_keypair::Keypair;
use solana_signer::Signer;

// ---- the helper ---------------------------------------------------------------------------------

/// The helper's errors (`#[error_code(offset = 8000)]`).
const HELPER_ERR_NOT_PUMP: u32 = 8_000;
const HELPER_ERR_BAD_CURVE: u32 = 8_001;

/// A Jito tip account (`getTipAccounts`, docs.jito.wtf): the bundle's last transaction pays it.
const JITO_TIP_ACCOUNT: Pubkey = pubkey!("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5");
/// The tip the tests pay (any amount: the tip account is pre-funded).
const TIP: u64 = 100_000;

/// The key of the A28 table the sizes are computed against (sizes only; nothing is installed).
const ALT_KEY: Pubkey = Pubkey::new_from_array([0xA2; 32]);

const COMPUTE_BUDGET: Pubkey = pubkey!("ComputeBudget111111111111111111111111111111");

/// ARCH §5.4 tx1's compute-unit limit here (a completing `buy_v2` is 89–97 k CU, R-PUMP §6.6).
const TX1_CU: u32 = 200_000;
/// tx2's (`migrate_v2` ≈ 330 k CU on the fork, 322–358 k on mainnet).
const TX2_CU: u32 = 450_000;
/// tx3's (M6: up to five account creations and two `crank_price`).
const TX3_CU: u32 = 400_000;

/// Deploys the helper (`pump::load_complete_curve`: `$COMPLETE_CURVE_SO`, else
/// `target/deploy/complete_curve.so`) and pre-funds the Jito tip account.
fn load_helper(env: &mut Env) {
    pump::load_complete_curve(env);
    env.fund(&JITO_TIP_ACCOUNT, 1_000_000_000);
}

fn sha8(preimage: &str) -> [u8; 8] {
    fork::sha256(preimage.as_bytes())[..8].try_into().unwrap()
}

/// `complete_curve(max_sol_cost)` by `user` receiving into `user_base`: pump `buy_v2`'s 27
/// accounts in its order (`pump::complete_curve_ix`: the harness's own list,
/// `PumpCurve::v2_accounts`, with the live creator vault and the third normal / buyback
/// recipients of the live `Global`).
fn complete_curve_ix(
    env: &Env,
    c: &PumpCurve,
    user: &Pubkey,
    user_base: &Pubkey,
    max_sol_cost: u64,
) -> Instruction {
    pump::complete_curve_ix(&env.svm, c, user, user_base, 2, max_sol_cost)
}

/// `SetComputeUnitLimit(units)` and `SetComputeUnitPrice(1 µL)`: ARCH §5.4's "ComputeBudget ×2".
fn compute_budget(units: u32) -> [Instruction; 2] {
    let mut price = vec![3u8];
    price.extend_from_slice(&1u64.to_le_bytes());
    [
        pump::cu_limit_ix(units),
        Instruction {
            program_id: COMPUTE_BUDGET,
            accounts: vec![],
            data: price,
        },
    ]
}

/// A System `Transfer` of `lamports` from `from` to the Jito tip account.
fn tip_ix(from: &Pubkey, lamports: u64) -> Instruction {
    let mut data = vec![2u8, 0, 0, 0];
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: anchor_lang::system_program::ID,
        accounts: vec![
            AccountMeta::new(*from, true),
            AccountMeta::new(JITO_TIP_ACCOUNT, false),
        ],
        data,
    }
}

/// The custom error code of a failed transaction.
fn code_of(f: &FailedTransactionMetadata) -> Option<u32> {
    custom_code(&Err(f.clone()))
}

/// The helper's `emit!` log events.
#[derive(Clone, Debug, PartialEq, Eq)]
enum HelperEvent {
    Completed {
        mint: Pubkey,
        curve: Pubkey,
        user: Pubkey,
        tokens: u64,
        lamports_spent: u64,
    },
    AlreadyComplete {
        mint: Pubkey,
        curve: Pubkey,
        virtual_token_reserves: u64,
    },
}

fn helper_events(meta: &TransactionMetadata) -> Vec<HelperEvent> {
    let engine = base64::engine::general_purpose::STANDARD;
    let key = |b: &[u8], o: usize| Pubkey::new_from_array(b[o..o + 32].try_into().unwrap());
    let u64_at = |b: &[u8], o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
    let completed = sha8("event:CurveCompleted");
    let already = sha8("event:CurveAlreadyComplete");
    meta.logs
        .iter()
        .filter_map(|l| l.strip_prefix("Program data: "))
        .filter_map(|b| engine.decode(b.trim()).ok())
        .filter(|d| d.len() >= 8)
        .filter_map(|d| {
            let (disc, b) = d.split_at(8);
            if disc == completed && b.len() == 112 {
                Some(HelperEvent::Completed {
                    mint: key(b, 0),
                    curve: key(b, 32),
                    user: key(b, 64),
                    tokens: u64_at(b, 96),
                    lamports_spent: u64_at(b, 104),
                })
            } else if disc == already && b.len() == 72 {
                Some(HelperEvent::AlreadyComplete {
                    mint: key(b, 0),
                    curve: key(b, 32),
                    virtual_token_reserves: u64_at(b, 64),
                })
            } else {
                None
            }
        })
        .collect()
}

// ---- actors -------------------------------------------------------------------------------------

/// The completer of the curve (the keeper, or the borrower's client): a funded keypair with its
/// base ATA (pump does not create it) and, if `with_uva`, its pump volume accumulator (else
/// `buy_v2` creates it, paid by the completer).
struct Completer {
    kp: Keypair,
    ata: Pubkey,
}

impl Completer {
    fn new(env: &mut Env, c: &PumpCurve, with_uva: bool) -> Self {
        let kp = env.funded_keypair();
        let payer = env.payer.pubkey();
        let program = c.token_program.id();
        let mut ixs = vec![fork::create_ata_idempotent_ix(
            &payer,
            &kp.pubkey(),
            &c.mint,
            &program,
        )];
        if with_uva {
            ixs.push(fork::pump_init_uva_ix(&payer, &kp.pubkey()));
        }
        ok(env.send(&ixs, &[]));
        Self {
            ata: pda::ata(&kp.pubkey(), &c.mint, &program),
            kp,
        }
    }

    fn key(&self) -> Pubkey {
        self.kp.pubkey()
    }

    /// tx1: ComputeBudget ×2 + `complete_curve(max_sol_cost)`.
    fn tx1(&self, env: &Env, c: &PumpCurve, max_sol_cost: u64) -> Vec<Instruction> {
        let mut v = compute_budget(TX1_CU).to_vec();
        v.push(complete_curve_ix(
            env,
            c,
            &self.key(),
            &self.ata,
            max_sol_cost,
        ));
        v
    }

    /// v0.3's tx1 (R3B-11): ComputeBudget ×2 + pump `buy_v2(rt_read, max_sol_cost)` direct.
    fn stale_tx1(
        &self,
        env: &Env,
        c: &PumpCurve,
        rt_read: u64,
        max_sol_cost: u64,
    ) -> Vec<Instruction> {
        let fr = fork::pump_fee_recipient(&env.svm, 2);
        let bb = fork::pump_buyback_fee_recipient(&env.svm, 2);
        let mut v = compute_budget(TX1_CU).to_vec();
        v.push(c.v2_ix(
            &env.svm,
            PumpV2::Buy,
            &self.key(),
            &self.ata,
            &fr,
            &bb,
            rt_read,
            max_sol_cost,
        ));
        v
    }

    /// tx2: ComputeBudget ×2 + `migrate_v2` top-level (+ the Jito tip when `tip`).
    fn tx2(&self, env: &Env, c: &PumpCurve, tip: bool) -> Vec<Instruction> {
        let mut v = compute_budget(TX2_CU).to_vec();
        v.push(pump::migrate_v2_ix(&env.svm, &self.key(), c));
        if tip {
            v.push(tip_ix(&self.key(), TIP));
        }
        v
    }
}

// ---- the bundle ---------------------------------------------------------------------------------

type BundleResult = Result<Vec<TransactionMetadata>, (usize, FailedTransactionMetadata)>;

/// Runs `txs` as a Jito bundle (U22): in order, each through `Env::send` (the invariant hooks
/// after every instruction), all-or-nothing — the VM is restored when one fails, and the index
/// of the failing transaction is returned with its metadata.
fn bundle(env: &mut Env, txs: &[(Vec<Instruction>, &Keypair)]) -> BundleResult {
    let before = env.svm.clone();
    let mut metas = Vec::with_capacity(txs.len());
    for (i, (ixs, signer)) in txs.iter().enumerate() {
        match env.send(ixs, &[*signer]) {
            Ok(m) => metas.push(m),
            Err(f) => {
                env.svm = before;
                return Err((i, f));
            }
        }
    }
    Ok(metas)
}

fn ok_bundle(r: BundleResult) -> Vec<TransactionMetadata> {
    r.unwrap_or_else(|(i, f)| panic!("tx{} failed {:?}\n{}", i + 1, f.err, f.meta.pretty_logs()))
}

/// Lamports, data and owner of `keys` (`None`: missing), for "nothing committed" and "untouched".
fn snapshot(env: &Env, keys: &[Pubkey]) -> Vec<Option<(u64, Vec<u8>, Pubkey)>> {
    keys.iter()
        .map(|k| {
            env.svm
                .get_account(k)
                .map(|a| (a.lamports, a.data, a.owner))
        })
        .collect()
}

/// The accounts the bundle writes: the curve and its ATAs, the completer's accounts, the pool
/// `migrate_v2` creates.
fn touched(c: &PumpCurve, who: &Completer) -> Vec<Pubkey> {
    let np = pump::NewPool::of(c);
    vec![
        c.curve,
        c.curve_base_ata,
        pump::wsol_ata(&c.curve),
        who.key(),
        who.ata,
        fork::pump_user_volume_accumulator(&who.key()),
        np.pool,
        np.base_vault,
        np.quote_vault,
        np.lp_mint,
        np.boost_vault_ata,
    ]
}

// ---- measurements -------------------------------------------------------------------------------

/// A transaction's T-TRACE / T-CU / T-SIZE figures.
#[derive(Clone, Copy, Debug)]
struct Stats {
    /// Compute units consumed.
    cu: u64,
    /// Instruction-trace entries: top-level instructions (ComputeBudget included) + every inner
    /// instruction (ARCH §6.2; `MAX_INSTRUCTION_TRACE_LENGTH` = 64).
    trace: usize,
    /// Peak stack height (1 = top level).
    height: u8,
    /// v0 through A28, the signer paying (bytes, accounts).
    v0_alt: (usize, usize),
    /// Legacy, every key inline (bytes, accounts).
    inline: (usize, usize),
}

fn stats(env: &Env, meta: &TransactionMetadata, ixs: &[Instruction], payer: &Pubkey) -> Stats {
    let inner: usize = meta.inner_instructions.iter().map(Vec::len).sum();
    Stats {
        cu: meta.compute_units_consumed,
        trace: ixs.len() + inner,
        height: meta
            .inner_instructions
            .iter()
            .flatten()
            .map(|i| i.stack_height)
            .max()
            .unwrap_or(1),
        v0_alt: raydium::v0_size(payer, ixs, &ALT_KEY, &a28(env)),
        inline: raydium::legacy_size(payer, ixs),
    }
}

/// A28 as ARCH §6.4 enumerates it: the Raydium part (`raydium::protocol_alt_addresses`: programs,
/// authorities, SPL Token, Token-2022, ATA, System, WSOL, the CPMM configs, short.win's static
/// PDAs and event authority), the pump part (pump, PumpSwap and pump-fees; pump's `global`, event
/// authority, `fee_config` and GVA; PumpSwap's four) and the live pump fee and buyback recipients
/// with their WSOL ATAs. PumpSwap's recipients (no curve transaction names them) are left out.
fn a28(env: &Env) -> Vec<Pubkey> {
    let mut v = raydium::protocol_alt_addresses();
    v.extend([
        PUMP_PROGRAM_ID,
        PUMP_AMM_PROGRAM_ID,
        PUMP_FEES_PROGRAM_ID,
        PUMP_GLOBAL,
        PUMP_EVENT_AUTHORITY,
        PUMP_FEE_CONFIG,
        PUMP_GLOBAL_VOLUME_ACCUMULATOR,
        PUMP_AMM_GLOBAL_CONFIG,
        PUMP_AMM_EVENT_AUTHORITY,
        PUMP_AMM_FEE_CONFIG,
        PUMP_AMM_GLOBAL_VOLUME_ACCUMULATOR,
    ]);
    for i in 0..8 {
        for k in [
            fork::pump_fee_recipient(&env.svm, i),
            fork::pump_buyback_fee_recipient(&env.svm, i),
        ] {
            v.push(k);
            v.push(pump::wsol_ata(&k));
        }
    }
    v
}

fn row(name: &str, s: &Stats) {
    eprintln!(
        "| {name} | {} | {} | {} | {} B / {} | {} B / {} |",
        s.trace, s.height, s.cu, s.v0_alt.0, s.v0_alt.1, s.inline.0, s.inline.1
    );
}

// ---- setup --------------------------------------------------------------------------------------

/// The fork on `coin` with the helper deployed.
fn fork_with_helper(coin: PumpCoin) -> (Env, PumpCurve) {
    let (mut env, c) = Env::fork_pump_coin(coin);
    load_helper(&mut env);
    (env, c)
}

/// What `buy_v2(rT)` charges on the live curve (pump's own formula, `PumpCurve::charge_buy`), plus
/// the creator-vault top-up pump takes first when the vault is below `Rent.minimum_balance(0)`
/// (R-PUMP Update U6) and the curve's growth to 166 bytes on the 2026-10-08 build (76,200 on the
/// 151-byte `near` curve, `PumpCurve::growth_top_up`).
fn completion_cost(env: &Env, c: &PumpCurve) -> u64 {
    let rt = c.state(&env.svm).real_token_reserves;
    let charge = c.charge_buy(&env.svm, rt).expect("a live curve").cost;
    let vault = c.creator_vault(&env.svm);
    let min = fork::pump_rent().minimum_balance(0);
    charge + min.saturating_sub(read::lamports(&env.svm, &vault)) + c.growth_top_up(&env.svm)
}

/// An external user buys until `rt_left` real tokens remain (0 completes the curve).
fn buy_down_to(env: &mut Env, c: &PumpCurve, rt_left: u64) -> CurveUser {
    let u = CurveUser::new(env, c);
    let rt = c.state(&env.svm).real_token_reserves;
    ok(u.buy(env, c, rt - rt_left, u64::MAX));
    assert_eq!(c.state(&env.svm).real_token_reserves, rt_left);
    u
}

/// A third party's `migrate_v2` (anyone may call it, R-PUMP §7.4).
fn migrate(env: &mut Env, c: &PumpCurve) {
    let m = env.funded_keypair();
    let ix = pump::migrate_v2_ix(&env.svm, &m.pubkey(), c);
    ok(env.send(&[pump::cu_limit_ix(TX2_CU), ix], &[&m]));
}

fn tokens(env: &Env, key: &Pubkey) -> u64 {
    read::token_amount(&env.svm, key).unwrap_or(0)
}

fn lamports(env: &Env, key: &Pubkey) -> u64 {
    read::lamports(&env.svm, key)
}

// ---- tx1 ----------------------------------------------------------------------------------------

/// tx1 on a live curve: `complete_curve` buys the `rT` it reads **when it runs**, by the signer
/// (pump's `user`), and the curve is complete; the completer holds exactly those tokens and paid
/// `buy_v2(rT)`'s charge (plus the creator-vault top-up if due and the curve's growth to 166
/// bytes, plus its pump UVA's rent when pump creates it); `CurveCompleted` reports both. The helper's one CPI is pump's `buy_v2(rT, max)`,
/// 24 bytes (no cap byte), at height 2; nothing runs deeper than 3.
#[test]
fn tx1_completes_a_live_curve_at_the_rt_it_reads() {
    for with_uva in [true, false] {
        let (mut env, c) = fork_with_helper(PumpCoin::Near);
        let who = Completer::new(&mut env, &c, with_uva);
        let rt = c.state(&env.svm).real_token_reserves;
        assert!(rt > 0 && !c.state(&env.svm).complete);
        let cost = completion_cost(&env, &c);
        let l0 = lamports(&env, &who.key());
        let ixs = who.tx1(&env, &c, cost + 10_000_000);
        let meta = ok(env.send(&ixs, &[&who.kp]));

        let after = c.state(&env.svm);
        assert!(after.complete, "the curve is complete");
        assert_eq!(after.real_token_reserves, 0);
        assert!(after.virtual_token_reserves > 0, "migrate_v2 has not run");
        assert_eq!(tokens(&env, &who.ata), rt, "the completer holds rT");
        let spent = l0 - lamports(&env, &who.key());
        let uva_rent = if with_uva {
            0
        } else {
            let uva = fork::pump_user_volume_accumulator(&who.key());
            let len = env.svm.get_account(&uva).expect("UVA created").data.len();
            fork::pump_rent().minimum_balance(len)
        };
        eprintln!("with UVA {with_uva}: spent {spent}, charge {cost}, UVA rent {uva_rent}");
        assert_eq!(
            spent,
            cost + uva_rent,
            "buy_v2(rT)'s charge (+ the UVA if created)"
        );
        assert_eq!(
            helper_events(&meta),
            [HelperEvent::Completed {
                mint: c.mint,
                curve: c.curve,
                user: who.key(),
                tokens: rt,
                lamports_spent: spent,
            }]
        );
        let first = &meta.inner_instructions[2][0];
        assert_eq!(first.stack_height, 2);
        assert_eq!(first.instruction.data.len(), 24, "no trailing cap byte");
        assert_eq!(first.instruction.data[..8], fork::PUMP_BUY_V2);
        assert_eq!(
            u64::from_le_bytes(first.instruction.data[8..16].try_into().unwrap()),
            rt
        );
        let s = stats(&env, &meta, &ixs, &who.key());
        assert_eq!(s.height, 3, "helper 1 → pump 2 → System / Token / pfee 3");
        eprintln!("tx1 (UVA exists: {with_uva}): {s:?}");
    }
}

/// tx1 + tx2 on every live SOL curve of the dump — Token-2022 125- and 115-byte curves, a
/// legacy SPL-Token curve without a creator, the 2024 49-byte layout pump grows on its first v2
/// trade, the near-complete 151-byte curve — from wherever the curve stands: the helper buys the
/// whole `rT`, the curve completes, `migrate_v2` seeds the canonical pool with exactly the
/// curve's remaining base tokens (`curve_base_ata − rT` as read), and the completer holds `rT`.
#[test]
fn the_bundle_completes_and_migrates_every_live_sol_curve() {
    for coin in PumpCoin::LIVE_SOL {
        let (mut env, c) = fork_with_helper(coin);
        let who = Completer::new(&mut env, &c, false);
        let rt = c.state(&env.svm).real_token_reserves;
        let xp = tokens(&env, &c.curve_base_ata) - rt;
        // Generous: a 49-byte curve's growth to 125 bytes and the completer's UVA are paid by
        // the completer on top of pump's charge.
        let max = completion_cost(&env, &c) * 105 / 100 + 10_000_000;
        let tx1 = who.tx1(&env, &c, max);
        let tx2 = who.tx2(&env, &c, true);
        let metas = bundle(&mut env, &[(tx1.clone(), &who.kp), (tx2.clone(), &who.kp)])
            .unwrap_or_else(|(i, f)| {
                panic!(
                    "{coin:?}: tx{} failed {:?}\n{}",
                    i + 1,
                    f.err,
                    f.meta.pretty_logs()
                )
            });
        let s = c.state(&env.svm);
        assert!(s.complete && s.virtual_token_reserves == 0, "{coin:?}");
        assert_eq!(tokens(&env, &who.ata), rt, "{coin:?}");
        let pool = pump::NewPool::of(&c)
            .reserves(&env.svm)
            .expect("the canonical pool");
        assert_eq!(pool.base, xp, "{coin:?}: X_p");
        eprintln!(
            "{coin:?} ({:?}): rT {rt}; tx1 {:?}; tx2 {:?}",
            c.token_program,
            stats(&env, &metas[0], &tx1, &who.key()),
            stats(&env, &metas[1], &tx2, &who.key())
        );
    }
}

/// tx1 on a curve a third party completed — before and after its `migrate_v2` — succeeds as a
/// **no-op**: no CPI at all, no lamport or token of the completer moves, `CurveAlreadyComplete`
/// reports the curve (`vT > 0` while migrating, 0 once migrated).
#[test]
fn tx1_is_a_no_op_on_a_completed_curve() {
    let (mut env, c) = fork_with_helper(PumpCoin::Near);
    let who = Completer::new(&mut env, &c, true);
    buy_down_to(&mut env, &c, 0);
    let vt = c.state(&env.svm).virtual_token_reserves;
    assert!(c.state(&env.svm).complete && vt > 0);
    for migrated in [false, true] {
        if migrated {
            migrate(&mut env, &c);
            assert_eq!(c.state(&env.svm).virtual_token_reserves, 0, "migrated");
        }
        let l0 = lamports(&env, &who.key());
        let ixs = who.tx1(&env, &c, 1);
        let meta = ok(env.send(&ixs, &[&who.kp]));
        assert!(meta.inner_instructions[2].is_empty(), "no CPI");
        assert_eq!(lamports(&env, &who.key()), l0);
        assert_eq!(tokens(&env, &who.ata), 0);
        assert_eq!(
            helper_events(&meta),
            [HelperEvent::AlreadyComplete {
                mint: c.mint,
                curve: c.curve,
                virtual_token_reserves: if migrated { 0 } else { vt },
            }]
        );
        let s = stats(&env, &meta, &ixs, &who.key());
        assert_eq!(
            s.trace, 3,
            "ComputeBudget ×2 + the helper, nothing below it"
        );
        eprintln!("tx1 no-op (migrated: {migrated}): {s:?}");
    }
}

// ---- ★ a curve trade between the keeper's read and the bundle -----------------------------------

/// What lands between the keeper's read and its bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Between {
    Nothing,
    /// A buy of a third of `rT` (rT falls: v0.3's `buy_v2(rT_read)` reverts 6021).
    Buy,
    /// A sale of 1 % of `rT` (rT rises: v0.3's buy leaves the curve incomplete).
    Sell,
    /// A third party completes the curve (tx1 becomes a no-op).
    Complete,
    /// … and migrates it (tx1 a no-op, `migrate_v2` idempotent).
    CompleteAndMigrate,
}

fn land(env: &mut Env, c: &PumpCurve, rt_read: u64, what: Between) {
    match what {
        Between::Nothing => {}
        Between::Buy => {
            let u = CurveUser::new(env, c);
            ok(u.buy(env, c, rt_read / 3, u64::MAX));
        }
        Between::Sell => {
            let u = CurveUser::new(env, c);
            ok(u.sell(env, c, rt_read / 100, 0));
        }
        Between::Complete | Between::CompleteAndMigrate => {
            buy_down_to(env, c, 0);
            if what == Between::CompleteAndMigrate {
                migrate(env, c);
            }
        }
    }
}

/// ★ T-STRADDLE's "a curve trade lands between the keeper's read and the bundle" (R3B-11): the
/// keeper reads the curve and builds tx1 (`max_sol_cost` = the read's completion cost + 5 %) and
/// tx2 (`migrate_v2` + the tip); then a buy, a sale, a third-party completion or a completion and
/// migration lands; then the bundle. It lands every time: tx1 buys the `rT` that is live then (or
/// does nothing), `migrate_v2` creates (or has created) the canonical pool with exactly the `X_p`
/// the read predicted (`curve_base_ata − rT` does not move with curve trades), and the completer
/// holds what it bought.
#[test]
fn a_curve_trade_before_the_bundle_does_not_break_it() {
    for what in [
        Between::Nothing,
        Between::Buy,
        Between::Sell,
        Between::Complete,
        Between::CompleteAndMigrate,
    ] {
        let (mut env, c) = fork_with_helper(PumpCoin::Near);
        let who = Completer::new(&mut env, &c, true);
        // The keeper's read.
        let rt_read = c.state(&env.svm).real_token_reserves;
        let xp_read = tokens(&env, &c.curve_base_ata) - rt_read;
        let max = completion_cost(&env, &c) * 105 / 100;
        let tx1 = who.tx1(&env, &c, max);
        let tx2 = who.tx2(&env, &c, true);
        // The trade.
        land(&mut env, &c, rt_read, what);
        let rt_live = c.state(&env.svm).real_token_reserves;
        // The bundle.
        let metas = bundle(&mut env, &[(tx1.clone(), &who.kp), (tx2.clone(), &who.kp)])
            .unwrap_or_else(|(i, f)| {
                panic!(
                    "{what:?}: tx{} failed {:?}\n{}",
                    i + 1,
                    f.err,
                    f.meta.pretty_logs()
                )
            });
        let s = c.state(&env.svm);
        assert!(
            s.complete && s.real_token_reserves == 0,
            "{what:?}: complete"
        );
        assert_eq!(s.virtual_token_reserves, 0, "{what:?}: migrated");
        let np = pump::NewPool::of(&c);
        let pool = np.reserves(&env.svm).expect("the canonical pool");
        assert_eq!(pool.base, xp_read, "{what:?}: X_p as read");
        assert_eq!(
            tokens(&env, &who.ata),
            rt_live,
            "{what:?}: bought the live rT"
        );
        let ev = helper_events(&metas[0]);
        match what {
            Between::Complete | Between::CompleteAndMigrate => assert!(
                matches!(ev[..], [HelperEvent::AlreadyComplete { .. }]),
                "{what:?}"
            ),
            _ => assert!(
                matches!(ev[..], [HelperEvent::Completed { tokens, .. }] if tokens == rt_live),
                "{what:?}"
            ),
        }
        eprintln!(
            "{what:?}: rT read {rt_read}, live {rt_live}; tx1 {:?}; tx2 {:?}",
            stats(&env, &metas[0], &tx1, &who.key()),
            stats(&env, &metas[1], &tx2, &who.key())
        );
    }
}

/// The same trades against **v0.3's tx1** (`buy_v2(rT_read)` direct, R3B-11): after a buy pump
/// reverts it (6021 `NotEnoughTokensToBuy`); after a sale it lands but leaves the curve incomplete,
/// so `migrate_v2` fails and the bundle reverts. Either way nothing is committed — every account
/// the bundle writes is as before it — and the helper's bundle, built on the same stale read,
/// lands.
#[test]
fn v0_3s_stale_tx1_reverts_the_bundle() {
    for what in [Between::Buy, Between::Sell] {
        let (mut env, c) = fork_with_helper(PumpCoin::Near);
        let who = Completer::new(&mut env, &c, true);
        let rt_read = c.state(&env.svm).real_token_reserves;
        let max = completion_cost(&env, &c) * 105 / 100;
        let stale = who.stale_tx1(&env, &c, rt_read, max);
        let tx1 = who.tx1(&env, &c, max);
        let tx2 = who.tx2(&env, &c, true);
        land(&mut env, &c, rt_read, what);
        let keys = touched(&c, &who);
        let before = snapshot(&env, &keys);
        let (i, f) =
            bundle(&mut env, &[(stale, &who.kp), (tx2.clone(), &who.kp)]).expect_err("reverted");
        match what {
            Between::Buy => {
                assert_eq!(i, 0, "tx1 fails");
                assert_eq!(code_of(&f), Some(fork::PUMP_ERR_NOT_ENOUGH_TOKENS_TO_BUY));
            }
            _ => {
                assert_eq!(i, 1, "tx1 lands, migrate_v2 fails on the incomplete curve");
                eprintln!("migrate_v2 on an incomplete curve: {:?}", f.err);
            }
        }
        assert_eq!(snapshot(&env, &keys), before, "{what:?}: nothing committed");
        assert!(!c.state(&env.svm).complete);
        ok_bundle(bundle(&mut env, &[(tx1, &who.kp), (tx2, &who.kp)]));
        assert_eq!(c.state(&env.svm).virtual_token_reserves, 0, "migrated");
    }
}

/// `max_sol_cost` bounds what the completer pays for whatever `rT` is live: a sale large enough to
/// raise the completion cost above the keeper's bound makes pump revert tx1 (6002
/// `TooMuchSolRequired`, pump's own code: the helper's start at 8000), the bundle reverts and
/// nothing is committed; rebuilt on a fresh read, it lands.
#[test]
fn max_sol_cost_bounds_the_completion() {
    let (mut env, c) = fork_with_helper(PumpCoin::Near);
    let who = Completer::new(&mut env, &c, true);
    let rt_read = c.state(&env.svm).real_token_reserves;
    let max = completion_cost(&env, &c) * 105 / 100;
    let tx1 = who.tx1(&env, &c, max);
    let tx2 = who.tx2(&env, &c, true);
    // A sale of a fifth of rT: completing now costs more than 5 % above the read.
    let u = CurveUser::new(&mut env, &c);
    ok(u.sell(&mut env, &c, rt_read / 5, 0));
    assert!(completion_cost(&env, &c) > max);
    let keys = touched(&c, &who);
    let before = snapshot(&env, &keys);
    let (i, f) = bundle(&mut env, &[(tx1, &who.kp), (tx2, &who.kp)]).expect_err("reverted");
    assert_eq!(i, 0);
    assert_eq!(
        code_of(&f),
        Some(fork::PUMP_ERR_TOO_MUCH_SOL_REQUIRED),
        "{}",
        f.meta.pretty_logs()
    );
    assert!(
        f.meta
            .logs
            .iter()
            .any(|l| l.contains(&format!("Program {PUMP_PROGRAM_ID} failed"))),
        "pump's 6002"
    );
    assert_eq!(snapshot(&env, &keys), before, "nothing committed");
    let max = completion_cost(&env, &c) * 105 / 100;
    let tx1 = who.tx1(&env, &c, max);
    let tx2 = who.tx2(&env, &c, true);
    ok_bundle(bundle(&mut env, &[(tx1, &who.kp), (tx2, &who.kp)]));
}

// ---- the straddled position ---------------------------------------------------------------------

/// The program's own venue state of `pm` now (the host build of `venues::read_state` on the
/// ReadSet as the VM holds it: what the next instruction reads).
fn host_state(env: &Env, pm: &PumpMarket) -> shortwin::venues::VenueState {
    use anchor_lang::prelude::AccountInfo;
    let mut accs: Vec<(Pubkey, u64, Vec<u8>, Pubkey)> = pm
        .read_set(false)
        .iter()
        .map(|a| {
            let acc = env.svm.get_account(&a.pubkey).unwrap_or_default();
            (a.pubkey, acc.lamports, acc.data, acc.owner)
        })
        .collect();
    let infos: Vec<AccountInfo> = accs
        .iter_mut()
        .map(|(k, l, d, o)| AccountInfo::new(k, false, false, l, d, o, false))
        .collect();
    let m = env.market(&pm.market());
    let kind = m.venue_kind().expect("venue");
    shortwin::venues::read_state(&m, kind, &infos, env.now())
}

/// A pump market on the near-complete curve with `40·q` lent, `q = vT/200`, the helper deployed.
fn market_with_helper() -> (Env, PumpMarket, u64) {
    let (mut env, c) = fork_with_helper(PumpCoin::Near);
    ok(env.list_pump(&c));
    let q = c.state(&env.svm).virtual_token_reserves / 200;
    let pm = env.pump_market_listed(PumpSpec::new(PumpCoin::Near, q * 40), c);
    (env, pm, q)
}

/// short.win's accounts of `pm` and of `b`'s position `nonce`: the bundle never touches them.
fn shortwin_accounts(pm: &PumpMarket, b: &Borrower, nonce: u64) -> Vec<Pubkey> {
    let m = pm.market();
    vec![
        m,
        pm.keys.market_vault,
        pm.keys.pool_vault,
        pm.keys.trader,
        pm.trader_base_ata,
        pda::position(&m, &b.key(), nonce).0,
        pda::exposure(&m, &b.key()).0,
        fork::pump_user_volume_accumulator(&pm.keys.trader),
        pda::config(),
    ]
}

/// The whole curve part of the straddle for a short.win position (ARCH §4.3, §5.4; V1-9): `q >
/// rT` on a live curve, so the close refuses `CurveStraddle`; the program's `B̂_str(q)` (the live
/// leg: all of `q` on the deterministic post-migration pool) barely moves when a curve buy lands
/// before the bundle (the pool `migrate_v2` creates does not depend on the path); the bundle
/// (tx1 `complete_curve`, tx2 `migrate_v2` + tip) names and touches no short.win account; the
/// tx3 (M6) — the venue accounts, then `crank_price` moving the market `PumpCurve → PumpSwap`
/// and the borrower's `close_short` on the pool in one transaction — buys all of `q` on the new
/// pool for at most `B̂_str` (within a few lamports).
#[test]
fn a_straddled_position_prices_the_bundle_it_waits_for() {
    let (mut env, pm, q) = market_with_helper();
    let c = pm.curve;
    let mut b = env.borrower();
    let (res, nonce) = env.open(&pm, &mut b, q);
    ok(res);
    env.warp(2);
    buy_down_to(&mut env, &c, q / 2);
    env.warp(1);
    ok(env.send(&[crank_price_ix(&pm)], &[]));
    assert_err(&env.close(&pm, &b, nonce), E::CurveStraddle);

    // The program's B̂_str on the keeper's read, and after a buy lands before the bundle.
    let b_str_read = host_state(&env, &pm).buy(q).expect("B̂_str").cost;
    let who = Completer::new(&mut env, &c, true);
    let max = completion_cost(&env, &c) * 105 / 100;
    let tx1 = who.tx1(&env, &c, max);
    let tx2 = who.tx2(&env, &c, true);
    let u = CurveUser::new(&mut env, &c);
    ok(u.buy(&mut env, &c, q / 8, u64::MAX));
    let st = host_state(&env, &pm);
    assert!(st.executable && st.curve.as_ref().unwrap().live.rt < u128::from(q));
    let b_str = st.buy(q).expect("B̂_str").cost;
    eprintln!("B̂_str on the read {b_str_read}, after a buy {b_str}");
    assert!(
        b_str.abs_diff(b_str_read) <= 4,
        "the post-migration pool does not depend on the curve's path"
    );

    // The bundle, with short.win's accounts watched: tx1 and tx2 name and touch none of them.
    for ix in tx1.iter().chain(&tx2) {
        assert_ne!(ix.program_id, shortwin::ID);
        assert!(ix.accounts.iter().all(|a| a.pubkey != pm.market()
            && a.pubkey != pm.keys.trader
            && a.pubkey != pm.keys.pool_vault));
    }
    let keys = shortwin_accounts(&pm, &b, nonce);
    let before = snapshot(&env, &keys);
    let saved = env.svm.clone();
    ok_bundle(bundle(
        &mut env,
        &[(tx1.clone(), &who.kp), (tx2.clone(), &who.kp)],
    ));
    assert_eq!(snapshot(&env, &keys), before, "no short.win account moved");
    env.svm = saved;

    // M6: the borrower's client bundles its own tx3, built before the pool exists — the pool
    // accounts the existence requirement names (paid by the borrower), then `crank_price` on the
    // curve's ReadSet (the valid canonical pool: `PumpCurve → PumpSwap`, the ring rewrite, the
    // warm-up) and the borrower's `close_short` on the pool, which the warm-up does not gate: all
    // of `q` bought on the pool the bundle created, for at most `B̂_str`.
    let pam = pool_market_before_migration(&env, &pm);
    let mut tx3 = vec![pump::cu_limit_ix(700_000)];
    tx3.extend(pool_account_ixs(&env, &b.key(), &pam, &pm));
    tx3.push(crank_price_ix(&pm));
    tx3.push(shortwin_litesvm_tests::engine::close_short_ix(
        &pam,
        &b.key(),
        nonce,
        u64::MAX,
        shortwin::venues::ExtrasSel::NONE,
        &[],
    ));
    let kp = b.kp.insecure_clone();
    let metas = ok_bundle(bundle(
        &mut env,
        &[(tx1, &who.kp), (tx2, &who.kp), (tx3.clone(), &kp)],
    ));
    let meta = &metas[2];
    let ev: VenueStateChanged = events::one(meta);
    assert_eq!(
        (ev.from, ev.to),
        (VenueKind::PumpCurve as u8, VenueKind::PumpSwap as u8)
    );
    assert_eq!(
        pam.coin_creator,
        shortwin_litesvm_tests::pamm::PammMarket::graduated(&env.svm, &pm).coin_creator,
        "the pool's coin creator is the curve's"
    );
    let close: shortwin::events::Close = events::one(meta);
    let spent = close.buyback_cost;
    eprintln!("tx3 on the pool: B̂_str {b_str}, executed {spent}");
    assert!(spent <= b_str, "B̂_str covers the executed leg");
    assert!(b_str - spent <= 8, "within a few lamports");
    assert_eq!(env.market(&pm.market()).open_positions, 0);
    let s3 = stats(&env, meta, &tx3, &b.key());
    eprintln!("the borrower's tx3 (accounts, crank → PumpSwap, close_short): {s3:?}");
    assert!(
        s3.trace <= 64 && s3.v0_alt.0 <= 1_232,
        "T-SIZE / T-TRACE: {s3:?}"
    );
}

/// A borrower's **self-made straddle** (M5-SEC-4's second case, ARCH §5.4, §9 M6): with its
/// position open the borrower buys `rT − q + 1` itself (any wallet), so `q > rT` and every exit on
/// the curve refuses `CurveStraddle` — the borrower's close and a keeper's liquidation alike. The
/// keeper's bundle (tx1 `complete_curve`, tx2 `migrate_v2`) completes and migrates the curve; the
/// pool then trades up ×3 before short.win observes anything. tx3's `crank_price` moves the market
/// `PumpCurve → PumpSwap` (the ring at the pool's `lp_supply`, the warm-up), keeper liquidations
/// wait `EmaWarmingUp` for the eight integrations of the pool's own buckets, and then liquidate the
/// straddled position **on the pool**: the straddle bought the borrower nothing but the wait.
#[test]
fn a_self_made_straddle_is_liquidated_on_the_pool_after_the_bundle() {
    let (mut env, pm, q) = market_with_helper();
    let c = pm.curve;
    let mut b = env.borrower();
    let (res, nonce) = env.open(&pm, &mut b, q);
    ok(res);
    let rt = c.state(&env.svm).real_token_reserves;
    assert!(rt > q, "the position fits the curve");
    env.warp(2);
    // The borrower's own buy of rT − q + 1: rT = q − 1 < q.
    buy_down_to(&mut env, &c, q - 1);
    env.warp(1);
    ok(env.send(&[crank_price_ix(&pm)], &[]));
    assert_err(&env.close(&pm, &b, nonce), E::CurveStraddle);
    let kp = env.funded_keypair();
    fn liq(env: &mut Env, mm: &impl VenueMarket, kp: &Keypair, b: &Pubkey, nonce: u64) -> TxResult {
        let ix = shortwin_litesvm_tests::engine::liquidate_ix(mm, &kp.pubkey(), b, nonce);
        env.send(&[pump::cu_limit_ix(400_000), ix], &[kp])
    }
    assert_err(&liq(&mut env, &pm, &kp, &b.key(), nonce), E::CurveStraddle);

    // The keeper's bundle.
    let who = Completer::new(&mut env, &c, true);
    let max = completion_cost(&env, &c) * 105 / 100;
    let tx1 = who.tx1(&env, &c, max);
    let tx2 = who.tx2(&env, &c, true);
    ok_bundle(bundle(&mut env, &[(tx1, &who.kp), (tx2, &who.kp)]));
    assert!(c.state(&env.svm).is_migrated());

    // The pool trades up ×3 before any observation.
    let pam = shortwin_litesvm_tests::pamm::PammMarket::graduated(&env.svm, &pm);
    let payer = env.payer.pubkey();
    let ixs = shortwin_litesvm_tests::pamm::venue_account_ixs(&env.svm, &payer, &pam);
    ok(env.send(&ixs, &[]));
    let u = shortwin_litesvm_tests::pamm::PammUser::new(&mut env, &pam.pool, 5_000 * 1_000_000_000);
    ok(u.push_price(&mut env, &pam.pool, 3, 1));

    // tx3: the crank moves the market onto the pool; keeper liquidations wait for the warm-up.
    env.warp(1);
    let meta = ok(env.send(&[crank_price_ix(&pm)], &[]));
    let ev: VenueStateChanged = events::one(&meta);
    assert_eq!(
        (ev.from, ev.to),
        (VenueKind::PumpCurve as u8, VenueKind::PumpSwap as u8)
    );
    let m = env.market(&pm.market());
    assert_eq!((m.ema_warm, m.ema_guard), (8, 1));
    let vault0 = tokens(&env, &pm.keys.pool_vault);
    let mut liquidated = None;
    for sec in 1..=300 {
        env.warp(1);
        ok(env.send(&[crank_price_ix(&pam)], &[]));
        let warm = env.market(&pm.market()).ema_warm;
        match liq(&mut env, &pam, &kp, &b.key(), nonce) {
            Ok(meta) => {
                assert_eq!(warm, 0, "+{sec} s: never while warming up");
                liquidated = Some((sec, events::one::<shortwin::events::Liquidate>(&meta)));
                break;
            }
            Err(f) => {
                let want = if warm > 0 {
                    E::EmaWarmingUp
                } else {
                    E::NotLiquidatable
                };
                assert_eq!(custom_code(&Err(f)), Some(u32::from(want)), "+{sec} s");
            }
        }
    }
    let (sec, ev) = liquidated.expect("liquidated on the pool");
    eprintln!(
        "self-made straddle: liquidated on the pool {sec} s after the transition, buyback {}",
        ev.buyback_cost
    );
    assert!(sec >= 8, "after the warm-up's eight integrations");
    assert!(ev.buyback_cost > 0 && ev.penalty > 0);
    assert_eq!(tokens(&env, &pm.keys.pool_vault) - vault0, q);
    assert_eq!(env.market(&pm.market()).open_positions, 0);
}

// ---- M6: tx3 (the transition) and the exit on the pool -------------------------------------------

/// The PumpSwap market `pm` becomes at its graduation, built **before** the pool exists (what a
/// keeper or a borrower's client does when it builds tx3): the pool keys `migrate_v2` creates
/// (`pump::NewPool`), the live curve's creator as the pool's coin creator, the live
/// `GlobalConfig`'s first protocol-fee and buyback recipients.
fn pool_market_before_migration(
    env: &Env,
    pm: &PumpMarket,
) -> shortwin_litesvm_tests::pamm::PammMarket {
    use shortwin_litesvm_tests::pamm::{self as pamm, PammMarket, PammPool};
    let np = pump::NewPool::of(&pm.curve);
    PammMarket {
        keys: pm.keys,
        pool: PammPool {
            mint: np.mint,
            base_program: np.base_program,
            pool: np.pool,
            base_vault: np.base_vault,
            quote_vault: np.quote_vault,
            lp_mint: np.lp_mint,
            boost_vault: np.boost_vault,
            boost_vault_ata: np.boost_vault_ata,
        },
        trader_base_ata: pm.trader_base_ata,
        protocol_recipient: pamm::protocol_fee_recipient(&env.svm, 0),
        buyback_recipient: pamm::buyback_fee_recipient(&env.svm, 0),
        coin_creator: Pubkey::new_from_array(pm.curve.state(&env.svm).creator),
        lender: pm.lender.insecure_clone(),
        lender_account: pm.lender_account,
    }
}

/// The pool accounts ARCH §3.12's existence requirement names for `pam`'s trades that do not exist
/// yet, created by `payer` (`pamm::venue_account_ixs` without reading the pool, which does not
/// exist when tx3 is built): the trader's pAMM UVA, the coin-creator vault's WSOL ATA, the two
/// recipients' WSOL ATAs, and on a cashback coin the trader's cashback ATA.
fn pool_account_ixs(
    env: &Env,
    payer: &Pubkey,
    pam: &shortwin_litesvm_tests::pamm::PammMarket,
    pm: &PumpMarket,
) -> Vec<Instruction> {
    use shortwin::constants::{TOKEN_PROGRAM_ID, WSOL_MINT};
    use shortwin_litesvm_tests::pamm;
    let exists = |k: &Pubkey| env.svm.get_account(k).is_some_and(|a| !a.data.is_empty());
    let mut v = Vec::new();
    if !exists(&pam.trader_uva()) {
        v.push(pamm::pamm_init_uva_ix(payer, &pam.trader()));
    }
    let (auth, _) = pam.creator_vault();
    let mut owners = vec![auth, pam.protocol_recipient, pam.buyback_recipient];
    if pm.curve.state(&env.svm).is_cashback_coin {
        owners.push(pam.trader_uva());
    }
    for owner in owners {
        if !exists(&pump::wsol_ata(&owner)) {
            v.push(fork::create_ata_idempotent_ix(
                payer,
                &owner,
                &WSOL_MINT,
                &TOKEN_PROGRAM_ID,
            ));
        }
    }
    v
}

/// The keeper's tx3 (`services/keeper` `graduation::transition_ixs`): ComputeBudget ×2, the missing
/// pool accounts (paid by `who`), `crank_price` on the curve's ReadSet (`→ PumpSwap`), then — if
/// `observe` — `crank_price` on the pool's ReadSet (its first observation), and the tip.
fn keeper_tx3(env: &Env, who: &Completer, pm: &PumpMarket, observe: bool) -> Vec<Instruction> {
    let pam = pool_market_before_migration(env, pm);
    let mut v = compute_budget(TX3_CU).to_vec();
    v.extend(pool_account_ixs(env, &who.key(), &pam, pm));
    v.push(crank_price_ix(pm));
    if observe {
        v.push(crank_price_ix(&pam));
    }
    v.push(tip_ix(&who.key(), TIP));
    v
}

/// An expired straddled position on the near curve (`q > rT` after an outside buy to `rT = q/2`,
/// the deadline passed): liquidatable, every exit on the curve refused `CurveStraddle`.
fn expired_straddle() -> (Env, PumpMarket, u64, Borrower, u64) {
    let (mut env, pm, q) = market_with_helper();
    let mut b = env.borrower();
    let (res, nonce) = env.open(&pm, &mut b, q);
    ok(res);
    env.warp(2);
    buy_down_to(&mut env, &pm.curve, q / 2);
    let deadline = env.position(&pm, &b.key(), nonce).deadline;
    let now = env.now();
    env.warp(deadline - now + 1);
    ok(env.send(&[crank_price_ix(&pm)], &[]));
    assert_err(&env.close(&pm, &b, nonce), E::CurveStraddle);
    (env, pm, q, b, nonce)
}

/// How tx3 is built in [`the_exit_on_the_pool_lands_exactly_at_the_warm_up_gate`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tx3 {
    /// The keeper's: the transition and the pool's first observation.
    WithObservation,
    /// The transition crank alone.
    CrankOnly,
    /// The keeper's, then one second (`T + 3`) without any observation.
    SkippedSecond,
}

/// ★ M6, "exactly when tx3 can land" (ARCH §5.4 tx3; the post-change warm-up, M4R): on an expired
/// straddled position (liquidatable: `expire` is due at once on any executable venue),
///
/// - a tx3 carrying the keeper's `expire` after the transition crank reverts the **whole** bundle
///   (`EmaWarmingUp` at tx3), and nothing of it is committed — the curve stays live;
/// - the keeper's tx3 (the pool accounts, the transition crank, the pool's first observation, the
///   tip) lands in the bundle at second `T` (`ema_warm = 8`, bucket `T` open); from `T + 1` the
///   keeper tries `expire` every second (refused `EmaWarmingUp`, nothing committed) and cranks when
///   it is refused: `expire` lands at **`T + 8`**, the first second whose roll integrates the eighth
///   bucket (`T` … `T + 7`);
/// - tx3 without the pool's first observation: at `T + 9`;
/// - one second without any observation (`T + 3`): one second later, `T + 9`.
///
/// Each expiry buys all of `q` on the pool; tx3's trace, CU and size are measured (T-TRACE, T-CU,
/// T-SIZE).
#[test]
fn the_exit_on_the_pool_lands_exactly_at_the_warm_up_gate() {
    for variant in [Tx3::WithObservation, Tx3::CrankOnly, Tx3::SkippedSecond] {
        let (mut env, pm, q, b, nonce) = expired_straddle();
        let c = pm.curve;
        let who = Completer::new(&mut env, &c, true);
        let max = completion_cost(&env, &c) * 105 / 100;
        let tx1 = who.tx1(&env, &c, max);
        let tx2 = who.tx2(&env, &c, false);
        let pam = pool_market_before_migration(&env, &pm);
        let kp = env.funded_keypair();
        let expire_ix = |pam: &shortwin_litesvm_tests::pamm::PammMarket, kp: &Keypair| {
            vec![
                pump::cu_limit_ix(400_000),
                shortwin_litesvm_tests::engine::expire_ix(pam, &kp.pubkey(), &b.key(), nonce),
            ]
        };
        if variant == Tx3::WithObservation {
            // ARCH §5.4's tx3 as written: the transition crank and the keeper's exit.
            let mut bad = compute_budget(TX3_CU).to_vec();
            bad.extend(pool_account_ixs(&env, &who.key(), &pam, &pm));
            bad.push(crank_price_ix(&pm));
            bad.extend(expire_ix(&pam, &who.kp)[1..].iter().cloned());
            bad.push(tip_ix(&who.key(), TIP));
            let mut keys = shortwin_accounts(&pm, &b, nonce);
            keys.extend(touched(&c, &who));
            let before = snapshot(&env, &keys);
            let (i, f) = bundle(
                &mut env,
                &[
                    (tx1.clone(), &who.kp),
                    (tx2.clone(), &who.kp),
                    (bad, &who.kp),
                ],
            )
            .expect_err("tx3 with the exit reverts the bundle");
            assert_eq!(i, 2);
            assert_eq!(code_of(&f), Some(u32::from(E::EmaWarmingUp)));
            assert_eq!(snapshot(&env, &keys), before, "nothing committed");
            assert!(!c.state(&env.svm).complete, "the curve is live");
        }
        let observe = variant != Tx3::CrankOnly;
        let tx3 = keeper_tx3(&env, &who, &pm, observe);
        let metas = ok_bundle(bundle(
            &mut env,
            &[(tx1, &who.kp), (tx2, &who.kp), (tx3.clone(), &who.kp)],
        ));
        let t = env.now();
        let ev: VenueStateChanged = events::one(&metas[2]);
        assert_eq!(
            (ev.from, ev.to),
            (VenueKind::PumpCurve as u8, VenueKind::PumpSwap as u8)
        );
        let m = env.market(&pm.market());
        assert_eq!((m.ema_warm, m.ema_guard), (8, 1));
        assert_eq!(m.b_lo.get() != 0, observe, "tx3's own bucket T");
        let s3 = stats(&env, &metas[2], &tx3, &who.key());
        row(&format!("tx3 {variant:?} (+ tip)"), &s3);
        assert!(
            s3.trace <= 64 && s3.v0_alt.0 <= 1_232 && s3.v0_alt.1 <= 64,
            "{s3:?}"
        );
        assert!(s3.cu < u64::from(TX3_CU));

        let pam = shortwin_litesvm_tests::pamm::PammMarket::graduated(&env.svm, &pm);
        let vault0 = tokens(&env, &pm.keys.pool_vault);
        let mut landed = None;
        for sec in 1..=12 {
            env.warp(1);
            if variant == Tx3::SkippedSecond && sec == 3 {
                continue;
            }
            match env.send(&expire_ix(&pam, &kp), &[&kp]) {
                Ok(meta) => {
                    landed = Some((sec, events::one::<shortwin::events::Expire>(&meta)));
                    break;
                }
                Err(f) => {
                    assert_eq!(
                        custom_code(&Err(f)),
                        Some(u32::from(E::EmaWarmingUp)),
                        "{variant:?} +{sec} s"
                    );
                    ok(env.send(&[crank_price_ix(&pam)], &[]));
                }
            }
        }
        let (sec, ev) = landed.expect("expired on the pool");
        eprintln!(
            "{variant:?}: tx3 at {t}, expire on the pool at T + {sec} (buyback {})",
            ev.buyback_cost
        );
        let want = match variant {
            Tx3::WithObservation => 8,
            Tx3::CrankOnly | Tx3::SkippedSecond => 9,
        };
        assert_eq!(sec, want, "{variant:?}");
        assert_eq!(tokens(&env, &pm.keys.pool_vault) - vault0, q);
        assert_eq!(env.market(&pm.market()).open_positions, 0);
    }
}

/// What lands between the build of the whole bundle and the bundle, for
/// [`the_three_transaction_bundle_survives_a_curve_trade_before_it`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Before3 {
    Curve(Between),
    /// A third party completes, migrates **and** cranks the market onto its pool.
    Transitioned,
}

/// ★ T-STRADDLE, the whole bundle (M6): the keeper reads the curve, builds tx1, tx2 and tx3 (the
/// pool accounts, the transition crank, the pool's first observation, the tip); then a buy, a sale,
/// a third-party completion or a completion and migration lands; then the bundle. It lands every
/// time, and tx3 moves the market onto the pool `migrate_v2` created (`X_p` as read) with the
/// warm-up marked. A third party's transition crank before it (the market already `PumpSwap`)
/// makes tx3 refuse (`VenueAccountMismatch`: the curve's ReadSet no longer fits the market) and the
/// bundle revert with nothing committed — the market is on its pool already, and the keeper's next
/// second treats the position as an ordinary PumpSwap position.
#[test]
fn the_three_transaction_bundle_survives_a_curve_trade_before_it() {
    for what in [
        Before3::Curve(Between::Nothing),
        Before3::Curve(Between::Buy),
        Before3::Curve(Between::Sell),
        Before3::Curve(Between::Complete),
        Before3::Curve(Between::CompleteAndMigrate),
        Before3::Transitioned,
    ] {
        let (mut env, pm, _q, b, nonce) = expired_straddle();
        let c = pm.curve;
        let who = Completer::new(&mut env, &c, true);
        let rt_read = c.state(&env.svm).real_token_reserves;
        let xp_read = tokens(&env, &c.curve_base_ata) - rt_read;
        let max = completion_cost(&env, &c) * 105 / 100;
        let tx1 = who.tx1(&env, &c, max);
        let tx2 = who.tx2(&env, &c, false);
        let tx3 = keeper_tx3(&env, &who, &pm, true);
        match what {
            Before3::Curve(between) => land(&mut env, &c, rt_read, between),
            Before3::Transitioned => {
                land(&mut env, &c, rt_read, Between::CompleteAndMigrate);
                env.warp(1);
                ok(env.send(&[crank_price_ix(&pm)], &[]));
                assert_eq!(
                    env.market(&pm.market()).venue_kind(),
                    Some(VenueKind::PumpSwap)
                );
            }
        }
        let mut keys = shortwin_accounts(&pm, &b, nonce);
        keys.extend(touched(&c, &who));
        let before = snapshot(&env, &keys);
        let res = bundle(&mut env, &[(tx1, &who.kp), (tx2, &who.kp), (tx3, &who.kp)]);
        if what == Before3::Transitioned {
            let (i, f) = res.expect_err("tx3 refuses on a PumpSwap market");
            assert_eq!(i, 2, "tx1 and tx2 are no-ops, tx3 refuses");
            assert_eq!(code_of(&f), Some(u32::from(E::VenueAccountMismatch)));
            assert_eq!(snapshot(&env, &keys), before, "nothing committed");
            continue;
        }
        let metas = res.unwrap_or_else(|(i, f)| {
            panic!(
                "{what:?}: tx{} failed {:?}\n{}",
                i + 1,
                f.err,
                f.meta.pretty_logs()
            )
        });
        let ev: VenueStateChanged = events::one(&metas[2]);
        assert_eq!(
            (ev.from, ev.to),
            (VenueKind::PumpCurve as u8, VenueKind::PumpSwap as u8),
            "{what:?}"
        );
        let np = pump::NewPool::of(&c);
        assert_eq!(ev.pool, np.pool);
        let pool = np.reserves(&env.svm).expect("the canonical pool");
        assert_eq!(pool.base, xp_read, "{what:?}: X_p as read");
        let m = env.market(&pm.market());
        assert_eq!((m.ema_warm, m.ema_guard), (8, 1), "{what:?}");
        assert!(m.b_lo.get() != 0, "{what:?}: tx3's own bucket");
        assert_eq!(m.open_positions, 1);
    }
}

// ---- the helper's own checks (A66) --------------------------------------------------------------

/// Sends `ComputeBudget ×2 + ix` (signed by `signer`, if any).
fn send_one(env: &mut Env, ix: Instruction, signer: Option<&Keypair>) -> TxResult {
    let mut ixs = compute_budget(TX1_CU).to_vec();
    ixs.push(ix);
    match signer {
        Some(kp) => env.send(&ixs, &[kp]),
        None => env.send(&ixs, &[]),
    }
}

/// The helper trusts nothing it interprets: a CPI target other than pump (8000 `NotPump`),
/// another mint's curve (Anchor `ConstraintSeeds`), a curve account pump does not own (8001
/// `BadCurve`); each refusal leaves the curve live. Trailing accounts pass through to pump, which
/// accepts them (R-PUMP Update U4): an additive pump interface change needs no helper upgrade.
#[test]
fn the_helper_checks_what_it_relies_on() {
    let (mut env, c) = fork_with_helper(PumpCoin::Near);
    let who = Completer::new(&mut env, &c, true);
    let max = completion_cost(&env, &c) * 105 / 100;
    let good = complete_curve_ix(&env, &c, &who.key(), &who.ata, max);

    // The CPI target (slot 26) is not pump.
    let mut ix = good.clone();
    ix.accounts[26].pubkey = PUMP_AMM_PROGRAM_ID;
    assert_eq!(
        custom_code(&send_one(&mut env, ix, Some(&who.kp))),
        Some(HELPER_ERR_NOT_PUMP)
    );

    // Another mint's curve: the fresh coin's curve under the near coin's mint.
    let fresh = env.load_pump_coin(PumpCoin::Fresh);
    let mut ix = good.clone();
    ix.accounts[10].pubkey = fresh.curve;
    assert_eq!(
        custom_code(&send_one(&mut env, ix, Some(&who.kp))),
        Some(u32::from(AnchorErrorCode::ConstraintSeeds))
    );

    // The curve's key under another owner (rewritten under System, restored after).
    let saved = env.svm.get_account(&c.curve).unwrap();
    let mut forged = saved.clone();
    forged.owner = anchor_lang::system_program::ID;
    env.svm.set_account(c.curve, forged).unwrap();
    assert_eq!(
        custom_code(&send_one(&mut env, good.clone(), Some(&who.kp))),
        Some(HELPER_ERR_BAD_CURVE)
    );
    env.svm.set_account(c.curve, saved).unwrap();
    assert!(
        !c.state(&env.svm).complete,
        "every refusal left the curve live"
    );

    // A trailing account passes through to pump.
    let mut ix = good;
    ix.accounts.push(AccountMeta::new_readonly(
        PUMP_GLOBAL_VOLUME_ACCUMULATOR,
        false,
    ));
    let meta = ok(send_one(&mut env, ix, Some(&who.kp)));
    let cpi = &meta.inner_instructions[2][0];
    assert_eq!(
        cpi.instruction.accounts.len(),
        28,
        "27 + the trailing account"
    );
    assert!(c.state(&env.svm).complete);
}

/// short.win's `trader` PDA can never be the helper's `user`: it signs only through short.win's
/// `invoke_signed`, which never invokes the helper (A4, A66), and named as a non-signing `user`
/// the helper refuses it (Anchor `AccountNotSigner`) before any CPI — no lamport of the trader, no
/// token of the market moves, and the curve stays live.
#[test]
fn short_win_s_trader_cannot_be_the_helpers_user() {
    let (mut env, pm, _q) = market_with_helper();
    let c = pm.curve;
    let max = completion_cost(&env, &c) * 105 / 100;
    let trader = pm.trader();
    let mut ix = complete_curve_ix(&env, &c, &trader, &pm.trader_base_ata, max);
    assert_eq!(ix.accounts[13].pubkey, trader);
    ix.accounts[13].is_signer = false;
    let keys = [
        trader,
        pm.trader_base_ata,
        pm.keys.pool_vault,
        pm.keys.market_vault,
    ];
    let before = snapshot(&env, &keys);
    assert_eq!(
        custom_code(&send_one(&mut env, ix, None)),
        Some(u32::from(AnchorErrorCode::AccountNotSigner))
    );
    assert_eq!(snapshot(&env, &keys), before);
    assert!(!c.state(&env.svm).complete);
}

// ---- T-TRACE / T-CU / T-SIZE --------------------------------------------------------------------

/// Every transaction of the curve part, measured (ARCH §5.4, §6.1, §6.2, §6.4; the table with
/// `-- --nocapture`):
///
/// - tx1 (`complete_curve`, the completing `buy_v2` below it): peak height 3; ≤ ARCH §6.2's
///   11 entries + 1 for pump's `CompleteEvent` self-CPI + 1 for the 151-byte `near` curve's
///   growth to 166 bytes on the 2026-10-08 build (pump's System transfer from the completer);
/// - tx1 with the completer's ATA created in it and its pump UVA created by pump;
/// - tx1 as a no-op: 3 entries;
/// - tx2 (`migrate_v2` top-level + the tip): 2 + 51 + 1 = 54 entries — `migrate_v2` stays alone
///   with its ComputeBudget instructions and the tip (ARCH §5.4);
/// - tx1 and tx2 merged into one transaction: measured (the room left under 64 entries);
/// - every transaction ≤ 1,232 B and ≤ 64 accounts as v0 through A28 (T-SIZE).
#[test]
fn trace_cu_and_size_per_transaction() {
    eprintln!("| tx | trace | height | CU | v0 via A28 | inline (legacy) |");
    eprintln!("|---|---|---|---|---|---|");
    // tx1 with the completer's UVA and ATA in place, tx2 with the tip.
    let (mut env, c) = fork_with_helper(PumpCoin::Near);
    let who = Completer::new(&mut env, &c, true);
    let max = completion_cost(&env, &c) * 105 / 100;
    let tx1 = who.tx1(&env, &c, max);
    let tx2 = who.tx2(&env, &c, true);
    let metas = ok_bundle(bundle(
        &mut env,
        &[(tx1.clone(), &who.kp), (tx2.clone(), &who.kp)],
    ));
    let s1 = stats(&env, &metas[0], &tx1, &who.key());
    let s2 = stats(&env, &metas[1], &tx2, &who.key());
    row("tx1 complete_curve (UVA exists)", &s1);
    row("tx2 migrate_v2 + tip", &s2);
    assert_eq!(s1.height, 3);
    assert!(
        s1.trace <= 13,
        "ARCH §6.2: ≈ 1 + 8 + 2, + pump's CompleteEvent CPI, + the 151-byte curve's growth"
    );
    assert_eq!(
        s2.trace,
        2 + 51 + 1,
        "2 ComputeBudget + migrate_v2's 51 + the tip"
    );
    for s in [&s1, &s2] {
        assert!(s.v0_alt.0 <= 1_232 && s.v0_alt.1 <= 64, "T-SIZE: {s:?}");
    }
    assert!(s1.cu < u64::from(TX1_CU) && s2.cu < u64::from(TX2_CU));

    // tx1 when pump creates the completer's UVA; its ATA created in the same transaction.
    let (mut env, c) = fork_with_helper(PumpCoin::Near);
    let kp = env.funded_keypair();
    let ata = pda::ata(&kp.pubkey(), &c.mint, &c.token_program.id());
    let max = completion_cost(&env, &c) * 105 / 100 + 10_000_000;
    let mut tx1 = compute_budget(TX1_CU).to_vec();
    tx1.push(fork::create_ata_idempotent_ix(
        &kp.pubkey(),
        &kp.pubkey(),
        &c.mint,
        &c.token_program.id(),
    ));
    tx1.push(complete_curve_ix(&env, &c, &kp.pubkey(), &ata, max));
    let meta = ok(env.send(&tx1, &[&kp]));
    let s = stats(&env, &meta, &tx1, &kp.pubkey());
    row("tx1 ATA create + complete_curve (UVA created)", &s);
    assert!(s.trace <= 64 && s.v0_alt.0 <= 1_232);

    // tx1 as a no-op.
    let who = Completer::new(&mut env, &c, true);
    let tx1 = who.tx1(&env, &c, 1);
    let meta = ok(env.send(&tx1, &[&who.kp]));
    let s = stats(&env, &meta, &tx1, &who.key());
    row("tx1 no-op (curve complete)", &s);
    assert_eq!(s.trace, 3);

    // tx1 + tx2 merged: one transaction of the completion, the migration and the tip.
    let (mut env, c) = fork_with_helper(PumpCoin::Near);
    let who = Completer::new(&mut env, &c, true);
    let max = completion_cost(&env, &c) * 105 / 100;
    let mut merged = compute_budget(TX1_CU + TX2_CU).to_vec();
    merged.push(complete_curve_ix(&env, &c, &who.key(), &who.ata, max));
    merged.push(pump::migrate_v2_ix(&env.svm, &who.key(), &c));
    merged.push(tip_ix(&who.key(), TIP));
    match env.send(&merged, &[&who.kp]) {
        Ok(meta) => {
            let s = stats(&env, &meta, &merged, &who.key());
            row("tx1 + tx2 merged (+ tip)", &s);
            assert!(s.trace <= 64);
        }
        Err(f) => eprintln!(
            "| tx1 + tx2 merged (+ tip) | refused: {:?} | | | | |",
            f.err
        ),
    }
}
