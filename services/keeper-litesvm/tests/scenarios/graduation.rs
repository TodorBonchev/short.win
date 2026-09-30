//! Keeper duty 6 through the graduation (M5, M6; ARCH §2.5 "On → PumpSwap", §5.1, §5.4, §7.1
//! duty 6; ADOPTED Q14, Q16; A66): the **release** short.win, the **dumped** pump, pump-fees and
//! PumpSwap ELFs and the `complete-curve` helper's own SBF build on the mainnet fork of the program
//! tests (`shortwin_litesvm_tests::pump`, `::pamm`), driven by the keeper with its **default**
//! configuration unless a test says otherwise. Every keeper transaction — each transaction of a
//! bundle too — runs the invariant hooks after every instruction.
//!
//! - **`migrate_v2` and the transition as soon as a curve completes** (M6 default): an outside buy
//!   completes the curve of a covered market; at its next tick the keeper migrates it (its account
//!   list is the harness's own derivation's), then — in the same second — sends the transition
//!   (the pool accounts the trades require, `crank_price` onto the pool, the pool's first
//!   observation): the market is `PumpSwap`, its pAMM accounts exist, the warm-up is marked, and
//!   the keeper observes it every second until the warm-up is over (no positions: M6's keeper
//!   warm-up), once. With `migrate` off, someone else's migration is still moved onto the pool.
//! - **A graduation with positions**: a curve position survives an outside completion; the keeper
//!   migrates and transitions; the pool is pumped; the keeper sends no exit during the warm-up and
//!   liquidates the position **on the pool** in the first second the program accepts it (an outside
//!   key's simulation refused every second before), with the PumpSwap TradeExtras.
//! - **The straddle bundle** (tx1 `complete_curve`, tx2 `migrate_v2`, tx3 the transition + tip):
//!   an expired position with `Q > rT` — the curve bought down by an outsider, or by the borrower
//!   itself (`rT − q + 1`, M5-SEC-4's self-made straddle) — lands as one bundle; the market is on
//!   its pool with tx3's own bucket; the keeper observes every second and expires the position at
//!   **`T + 8`** (the warm-up's gate: an outside key's `expire` is refused `EmaWarmingUp` at every
//!   second before). Trace, CU and size of each transaction.
//! - **V1-9 fallback**: tx3's pre-flight fails → tx1 + tx2 alone (tip on tx2); the next second the
//!   keeper's own transition moves the market onto the pool, and the position is expired at that
//!   transition's gate.
//! - ★ **A curve trade between the keeper's read and its bundle**: a buy lands after the keeper's
//!   pre-flight — the three-transaction bundle still lands (the helper reads `rT` at execution); a
//!   large sale pushes the completion above the keeper's `max_sol_cost` — pump reverts tx1, nothing
//!   of the bundle is committed, and the keeper's bundle of the next second lands.
//! - **No block engine**: the bundle cannot go out; the keeper alerts and settles meanwhile.
//! - **Below the straddle** (`Q ≤ rT`): the keeper expires a curve position on the curve with the
//!   adapter's own account lists (the live recipients, the live creator's vault).
//! - **Graduated elsewhere** (duty 5 on a list-only market, M2-SEC-5): a curve migrated elsewhere
//!   with an expired position open goes `→ None` at the keeper's first crank; the keeper keeps
//!   the S3 chain on the no-route cadence with the detection set as ReadSet and settles the
//!   position in S3 once the chain has run `s3_wait_s`.
//! - **`None → PumpCurve` re-detection**: a list-only market whose detection set shows a live
//!   curve is cranked onto it, and the keeper then creates the trader's base ATA and pump UVA the
//!   re-detection does not create, once; an open lands on the curve.

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use shortwin::errors::ShortwinError;
use shortwin::state::VenueKind;
use shortwin_keeper::config::GraduationConfig;
use shortwin_keeper::graduation::{self, StraddleResult};
use shortwin_keeper_litesvm::load_complete_curve;
use shortwin_litesvm_tests::engine::{Borrower, VenueMarket};
use shortwin_litesvm_tests::fork::{self, PumpCoin, PumpCurve};
use shortwin_litesvm_tests::pamm::{PammMarket, PammUser};
use shortwin_litesvm_tests::pump::{self, CurveUser, PumpMarket, PumpSpec};
use shortwin_litesvm_tests::{pda, read};
use shortwin_services_common::AccountData;
use solana_signer::Signer;
use solana_transaction::Transaction;

use crate::common::*;

/// A funded Jito tip account for every one the keeper may pay (a tip below the rent-exempt
/// minimum cannot create one).
fn fund_tip_accounts(env: &mut Env) {
    for k in graduation::JITO_TIP_ACCOUNTS {
        env.fund(&k.parse().unwrap(), 1_000_000_000);
    }
}

/// The fork on the near-complete curve with the helper, a listed market with `40·q` lent
/// (`q = vT/200`), and a keeper with `cfg`.
fn pump_setup(cfg: KeeperConfig) -> (Harness, PumpMarket, u64) {
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::Near);
    load_complete_curve(&mut env);
    fund_tip_accounts(&mut env);
    ok(env.list_pump(&c));
    let q = c.state(&env.svm).virtual_token_reserves / 200;
    let pm = env.pump_market_listed(PumpSpec::new(PumpCoin::Near, q * 40), c);
    let h = Harness::new(env, cfg);
    // A priority price: every v0 transaction carries both ComputeBudget instructions (ARCH §5.4's
    // "ComputeBudget ×2").
    h.chain().set_priority_fees(vec![5_000]);
    (h, pm, q)
}

/// A keeper configuration with the `migrate_v2` duty off (the transition stays on).
fn without_migrate() -> KeeperConfig {
    KeeperConfig {
        graduation: GraduationConfig {
            migrate: false,
            ..GraduationConfig::default()
        },
        ..KeeperConfig::default()
    }
}

/// An outside user buys until `rt_left` real tokens remain.
fn buy_down_to(env: &mut Env, c: &PumpCurve, rt_left: u64) {
    let u = CurveUser::new(env, c);
    let rt = c.state(&env.svm).real_token_reserves;
    ok(u.buy(env, c, rt - rt_left, u64::MAX));
    assert_eq!(c.state(&env.svm).real_token_reserves, rt_left);
}

/// Who makes the position a straddle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Straddler {
    /// An outside buyer takes the curve to `rT = q/2`.
    Outsider,
    /// The borrower's own wallet buys `rT − q + 1` (M5-SEC-4's self-made straddle).
    Borrower,
}

/// Opens `q` for a new borrower, makes it a straddle (`q > rT`) and warps past the position's
/// deadline (expired: liquidatable).
fn straddled_position(h: &Harness, pm: &PumpMarket, q: u64) -> (Borrower, u64) {
    straddled_by(h, pm, q, Straddler::Outsider)
}

fn straddled_by(h: &Harness, pm: &PumpMarket, q: u64, who: Straddler) -> (Borrower, u64) {
    h.with_env(|env| {
        env.crank(pm);
        env.warp(1);
        let mut b = env.borrower();
        let (res, nonce) = env.open(pm, &mut b, q);
        ok(res);
        env.warp(2);
        match who {
            Straddler::Outsider => buy_down_to(env, &pm.curve, q / 2),
            Straddler::Borrower => {
                // The borrower's own wallet (its base ATA and pump UVA first).
                let c = pm.curve;
                let payer = env.payer.pubkey();
                let kp = b.kp.insecure_clone();
                let program = c.token_program.id();
                ok(env.send(
                    &[
                        fork::create_ata_idempotent_ix(&payer, &kp.pubkey(), &c.mint, &program),
                        fork::pump_init_uva_ix(&payer, &kp.pubkey()),
                    ],
                    &[],
                ));
                let rt = c.state(&env.svm).real_token_reserves;
                assert!(rt > q, "the position fits the curve");
                let ata = pda::ata(&kp.pubkey(), &c.mint, &program);
                let fr = fork::pump_fee_recipient(&env.svm, 0);
                let bb = fork::pump_buyback_fee_recipient(&env.svm, 0);
                let ix = c.v2_ix(
                    &env.svm,
                    fork::PumpV2::Buy,
                    &kp.pubkey(),
                    &ata,
                    &fr,
                    &bb,
                    rt - q + 1,
                    u64::MAX,
                );
                ok(env.send(&[pump::cu_limit_ix(400_000), ix], &[&kp]));
                assert_eq!(c.state(&env.svm).real_token_reserves, q - 1);
            }
        }
        let deadline = env.position(pm, &b.key(), nonce).deadline;
        let now = env.now();
        env.warp(deadline - now + 1);
        (b, nonce)
    })
}

fn market_kind(h: &Harness, pm: &PumpMarket) -> Option<VenueKind> {
    h.with_env(|env| env.market(&pm.market()).venue_kind())
}

fn curve(h: &Harness, pm: &PumpMarket) -> shortwin_venue_state::pump::BondingCurve {
    h.with_env(|env| pm.curve.state(&env.svm))
}

fn tokens(h: &Harness, key: &Pubkey) -> u64 {
    h.with_env(|env| read::token_amount(&env.svm, key).unwrap_or(0))
}

/// The ReadSet accounts of `pm` as the VM holds them (what the keeper reads).
fn read_accounts(h: &Harness, pm: &PumpMarket) -> Vec<Option<AccountData>> {
    h.with_env(|env| {
        pm.read_set(false)
            .iter()
            .map(|a| {
                env.svm.get_account(&a.pubkey).map(|acc| AccountData {
                    lamports: acc.lamports,
                    owner: acc.owner,
                    executable: acc.executable,
                    data: acc.data,
                })
            })
            .collect()
    })
}

/// The keeper's base ATA of the market's mint.
fn keeper_ata(h: &Harness, pm: &PumpMarket) -> Pubkey {
    pda::ata(&h.keeper.payer(), &pm.keys.mint, &pm.keys.token_program)
}

/// Whether `ixs` (signed by a funded outside key, with a compute budget) would be accepted by the
/// program now: `Ok`, or its custom error. The "first second the program accepts it" oracle.
fn accepts(h: &Harness, kp: &solana_keypair::Keypair, ix: Instruction) -> Result<(), Option<u32>> {
    h.with_env(|env| {
        let tx = Transaction::new_signed_with_payer(
            &[pump::cu_limit_ix(600_000), ix],
            Some(&kp.pubkey()),
            &[kp],
            env.svm.latest_blockhash(),
        );
        match env.svm.simulate_transaction(tx) {
            Ok(_) => Ok(()),
            Err(f) => Err(shortwin_litesvm_tests::errors::custom_code(&Err(f))),
        }
    })
}

/// The market as the VM holds it.
fn stored(h: &Harness, pm: &PumpMarket) -> shortwin::state::Market {
    h.with_env(|env| env.market(&pm.market()))
}

/// Every pool account a PumpSwap trade of `pam` requires exists (ARCH §3.12's existence
/// requirement: T14 the trader's pAMM UVA, T11 the coin-creator vault's WSOL ATA; the recipients'
/// are the fork's statics).
fn pool_accounts_exist(h: &Harness, pam: &PammMarket) -> bool {
    let (_, auth_ata) = pam.creator_vault();
    h.with_env(|env| {
        [pam.trader_uva(), auth_ata]
            .iter()
            .all(|k| env.svm.get_account(k).is_some_and(|a| !a.data.is_empty()))
    })
}

/// Duty 6 (ADOPTED Q16) with the **default** configuration (M6: `migrate` on again): an outside buy
/// completes a covered market's curve; at its next tick the keeper sends `migrate_v2` — top-level,
/// alone, with exactly the accounts the harness derives (`pump::migrate_v2_ix`: the 27 IDL accounts
/// and the two boost accounts) — then, in the same second, the transition: the pool accounts the
/// trades require (created: they did not exist), `crank_price` on the curve's ReadSet
/// (`VenueStateChanged PumpCurve → PumpSwap`) and on the pool (its first bucket). The market is on
/// its pool with the warm-up marked; the keeper observes it every second until the warm-up is over
/// (no positions: the first open must find a confirmed `P_ema`), then leaves it alone; once.
#[test]
fn the_keeper_migrates_and_moves_a_completed_curve_onto_its_pool() {
    let (mut h, pm, _q) = pump_setup(KeeperConfig::default());
    assert!(KeeperConfig::default().graduation.migrate, "M6's default");
    h.second();
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpCurve));
    h.with_env(|env| buy_down_to(env, &pm.curve, 0));
    let c = curve(&h, &pm);
    assert!(c.complete && c.virtual_token_reserves > 0, "migrating");

    // The keeper's instruction is the harness's own derivation.
    let payer = h.keeper.payer();
    let market = h.with_env(|env| env.market(&pm.market()));
    let mine = graduation::migrate_v2_ix(&payer, &market, &read_accounts(&h, &pm)).unwrap();
    let theirs = h.with_env(|env| pump::migrate_v2_ix(&env.svm, &payer, &pm.curve));
    assert_eq!(mine.program_id, theirs.program_id);
    assert_eq!(mine.data, theirs.data);
    assert_eq!(mine.accounts, theirs.accounts);

    let r = h.second().clone();
    let t = h.now();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.migrations.len(), 1, "{:?}", r.migrations);
    let out = r.migrations[0].result.landed().expect("migrate_v2 landed");
    eprintln!("migrate_v2: {} CU", out.units_consumed);
    assert_eq!(curve(&h, &pm).virtual_token_reserves, 0, "migrated");
    // The transition, in the same second.
    assert_eq!(r.transitions.len(), 1, "{:?}", r.transitions);
    let tr = &r.transitions[0];
    assert!(!tr.bundled);
    assert!(tr.created >= 1, "the trader's pAMM UVA at least");
    let out = tr.result.landed().expect("the transition landed");
    assert_eq!(
        tr.venue_changes,
        [(VenueKind::PumpCurve as u8, VenueKind::PumpSwap as u8)]
    );
    eprintln!(
        "transition: {} accounts created, {} CU",
        tr.created, out.units_consumed
    );
    assert!(
        r.observed(&pm.market()),
        "the market's observation this second"
    );
    assert!(!r.cranked.contains(&pm.market()), "no second crank");
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpSwap));
    let m = stored(&h, &pm);
    assert_eq!((m.ema_warm, m.ema_guard), (8, 1));
    assert_eq!(m.graduated_at, t);
    assert!(m.b_lo.get() != 0, "the pool's first bucket");
    let pam = h.with_env(|env| PammMarket::graduated(&env.svm, &pm));
    assert!(pool_accounts_exist(&h, &pam));

    // The warm-up, observed every second (no positions: M6's keeper warm-up) while the next
    // instruction would still find it running: buckets T … T + 7, the eighth integrated by
    // whatever instruction comes next (an open's own prologue); then nothing.
    for sec in 1..=10 {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.migrations.is_empty() && r.transitions.is_empty(), "once");
        assert!(r.pamm_accounts.is_empty(), "nothing left to create");
        assert_eq!(
            r.observed(&pm.market()),
            sec <= 7,
            "+{sec} s: warm {} needs {:?}",
            stored(&h, &pm).ema_warm,
            r.needs.get(&pm.market())
        );
    }
    assert_eq!(stored(&h, &pm).ema_warm, 1, "the eighth bucket is pending");
    h.with_env(|env| env.crank(&pam));
    assert_eq!(
        stored(&h, &pm).ema_warm,
        0,
        "the next instruction ends the warm-up"
    );
}

/// With `graduation.migrate` off the keeper sends no `migrate_v2`, but once someone else migrates
/// the curve the transition is still the keeper's (ARCH §7.1 duty 6 "on the canonical pool's
/// appearance"): the market moves onto its pool and its pool accounts are created.
#[test]
fn with_migrate_off_a_third_party_migration_is_still_moved_onto_the_pool() {
    let (mut h, pm, _q) = pump_setup(without_migrate());
    h.second();
    h.with_env(|env| buy_down_to(env, &pm.curve, 0));
    for _ in 0..2 {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.migrations.is_empty(), "migrate off");
        assert!(r.transitions.is_empty());
    }
    h.with_env(|env| {
        let m = env.funded_keypair();
        let ix = pump::migrate_v2_ix(&env.svm, &m.pubkey(), &pm.curve);
        ok(env.send(&[pump::cu_limit_ix(600_000), ix], &[&m]));
    });
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.transitions.len(), 1);
    assert!(r.transitions[0].result.landed().is_some());
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpSwap));
    let pam = h.with_env(|env| PammMarket::graduated(&env.svm, &pm));
    assert!(pool_accounts_exist(&h, &pam));
}

/// Who moves the market onto its pool in [`a_graduation_with_a_position_is_liquidated_on_the_pool_after_the_warm_up`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Transition {
    /// The keeper: its `migrate_v2`, then its transition with the pool's first bucket (`T`).
    Keeper,
    /// A third party migrates and cranks the market onto the pool (the transition records no
    /// bucket) before the keeper's tick: the warm-up's buckets are the keeper's from `T + 1`.
    ThirdParty,
}

/// A graduation with a position open (ARCH §2.5, §5.3; ECON §10): a curve position (`Q ≤ rT`)
/// survives an outside buy that completes the curve; the market is moved onto its pool — by the
/// keeper (`migrate_v2` and its transition with the pool's first bucket, in the same second), or by
/// a third party's migration and crank (no bucket; the keeper then creates the pool accounts) —; a
/// whale pumps the pool ×3 right after. During the warm-up the keeper observes every second and
/// sends **no exit** (an outside key's `liquidate` is refused `EmaWarmingUp`, then
/// `NotLiquidatable` while the EMA catches up); the keeper's `liquidate` — with the PumpSwap
/// ReadSet and TradeExtras — lands in the **first** second the outside key's simulation passes,
/// buying `Q` on the pool. No exit of the keeper is refused on the way. The two variants measure
/// what the keeper's own first bucket (the graduation price, before the pump) costs a genuine
/// pump: the warm-up seeds `P_ema` at the lowest bucket low (M4R), so with it the EMA starts at
/// the graduation price and climbs at the steady step after the gate.
#[test]
fn a_graduation_with_a_position_is_liquidated_on_the_pool_after_the_warm_up() {
    let mut measured = Vec::new();
    for variant in [Transition::Keeper, Transition::ThirdParty] {
        let (mut h, pm, q) = pump_setup(KeeperConfig::default());
        let (b, nonce) = h.with_env(|env| {
            env.crank(&pm);
            env.warp(1);
            let mut b = env.borrower();
            let (res, nonce) = env.open(&pm, &mut b, q);
            ok(res);
            (b, nonce)
        });
        let position = pda::position(&pm.market(), &b.key(), nonce).0;
        h.second();
        h.idle(1);
        h.with_env(|env| buy_down_to(env, &pm.curve, 0));
        let t = h.now();
        match variant {
            Transition::Keeper => {
                let r = h.tick().clone();
                assert!(r.errors.is_empty(), "{:?}", r.errors);
                assert!(r.migrations[0].result.landed().is_some());
                assert!(r.transitions[0].result.landed().is_some());
            }
            Transition::ThirdParty => h.with_env(|env| {
                let m = env.funded_keypair();
                let ix = pump::migrate_v2_ix(&env.svm, &m.pubkey(), &pm.curve);
                ok(env.send(&[pump::cu_limit_ix(600_000), ix], &[&m]));
                env.crank(&pm);
                assert_eq!(
                    env.market(&pm.market()).venue_kind(),
                    Some(VenueKind::PumpSwap)
                );
                assert_eq!(env.market(&pm.market()).b_lo.get(), 0, "no bucket yet");
            }),
        }
        let pam = h.with_env(|env| PammMarket::graduated(&env.svm, &pm));
        // The pool pumped ×3 right after the transition.
        h.with_env(|env| {
            let whale = PammUser::new(env, &pam.pool, 20_000 * 1_000_000_000);
            ok(whale.push_price(env, &pam.pool, 3, 1));
        });
        let outsider = h.with_env(|env| env.funded_keypair());
        let liq = |h: &Harness| {
            let ix = shortwin_litesvm_tests::engine::liquidate_ix(
                &pam,
                &outsider.pubkey(),
                &b.key(),
                nonce,
            );
            accepts(h, &outsider, ix)
        };
        let mut liquidated_at = None;
        for _ in 0..300 {
            h.idle(1);
            let before = liq(&h);
            let warm = stored(&h, &pm).ema_warm;
            let r = h.tick().clone();
            assert!(r.errors.is_empty(), "{:?}", r.errors);
            assert!(
                !r.alerts
                    .iter()
                    .any(|a| matches!(a, Alert::ExitRefused { .. })),
                "{:?}",
                r.alerts
            );
            let landed = r.exits.iter().any(|e| {
                e.position == position
                    && e.kind == ExitKind::Liquidate
                    && e.result.landed().is_some()
            });
            if landed {
                assert_eq!(before, Ok(()), "the program accepted it this second");
                liquidated_at = Some(h.now());
                break;
            }
            // The keeper sent nothing the program would have taken.
            assert_ne!(before, Ok(()), "+{} s: the keeper missed it", h.now() - t);
            if warm > 0 {
                assert!(
                    r.exits.iter().all(|e| e.kind == ExitKind::SolSettle),
                    "no liquidation during the warm-up"
                );
            }
            assert!(r.observed(&pm.market()), "every second");
        }
        assert!(pool_accounts_exist(&h, &pam), "the keeper's duty");
        let at = liquidated_at.expect("liquidated on the pool");
        eprintln!(
            "graduation with a position ({variant:?}): transition at {t}, liquidated on the pool \
             at +{} s",
            at - t
        );
        assert!(at - t >= 8, "not before the warm-up's gate");
        assert!(
            h.with_env(|env| read::borsh::<shortwin::state::ShortPosition>(&env.svm, &position))
                .is_none()
        );
        assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpSwap));
        measured.push(at - t);
    }
    eprintln!(
        "×3 pump right after the graduation: liquidated at +{} s with the keeper's first bucket, \
         +{} s without",
        measured[0], measured[1]
    );
    assert!(measured[0] >= measured[1]);
}

/// The straddle bundle (ARCH §5.4 case 2; M6) with the keeper's default configuration, for a
/// straddle made by an outside buyer and for M5-SEC-4's **self-made** straddle (the borrower buys
/// `rT − q + 1` itself): the expired position is `Plan::Straddle`; the keeper's bundle — tx1
/// (ComputeBudget ×2, its base ATA, `complete_curve`), tx2 (ComputeBudget ×2, `migrate_v2`), tx3
/// (ComputeBudget ×2, the pool accounts, `crank_price` onto the pool, `crank_price` on the pool,
/// the Jito tip) — lands atomically: the keeper holds the curve's last `rT` tokens and paid the
/// tip, the market is `PumpSwap` with tx3's own bucket and the warm-up marked, and tx3 is the
/// market's observation for the second. The keeper then observes every second and expires the
/// position on the pool at **`T + 8`**, the warm-up's gate; an outside key's `expire` is refused
/// `EmaWarmingUp` in every second before. Per-transaction trace, CU and size.
#[test]
fn the_straddle_bundle_moves_the_market_onto_its_pool_and_the_exit_lands_at_the_gate() {
    for who in [Straddler::Outsider, Straddler::Borrower] {
        let (mut h, pm, q) = pump_setup(KeeperConfig::default());
        let (b, nonce) = straddled_by(&h, &pm, q, who);
        let position = pda::position(&pm.market(), &b.key(), nonce).0;
        let rt = curve(&h, &pm).real_token_reserves;
        let tip0: u64 = graduation::JITO_TIP_ACCOUNTS
            .iter()
            .map(|k| h.with_env(|env| read::lamports(&env.svm, &k.parse().unwrap())))
            .sum();
        let r = h.second().clone();
        let t = h.now();
        assert!(r.errors.is_empty(), "{who:?}: {:?}", r.errors);
        assert_eq!(r.straddles.len(), 1, "Plan::Straddle");
        let rec = &r.straddles[0];
        assert_eq!(rec.position, position);
        assert!(
            matches!(rec.result, StraddleResult::Landed { with_tx3: true, .. }),
            "{who:?}: {:?}",
            rec.result
        );
        assert_eq!(rec.rt_read, rt);
        assert_eq!(rec.txs, 3);
        let c = curve(&h, &pm);
        assert!(
            c.complete && c.virtual_token_reserves == 0,
            "completed and migrated"
        );
        assert_eq!(tokens(&h, &keeper_ata(&h, &pm)), rt, "the keeper holds rT");
        let tip1: u64 = graduation::JITO_TIP_ACCOUNTS
            .iter()
            .map(|k| h.with_env(|env| read::lamports(&env.svm, &k.parse().unwrap())))
            .sum();
        assert_eq!(tip1 - tip0, GraduationConfig::default().tip_lamports);
        assert_eq!(r.transitions.len(), 1);
        assert!(r.transitions[0].bundled);
        assert_eq!(
            r.transitions[0].venue_changes,
            [(VenueKind::PumpCurve as u8, VenueKind::PumpSwap as u8)]
        );
        assert!(r.observed(&pm.market()), "tx3 is the observation");
        assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpSwap));
        let m = stored(&h, &pm);
        assert_eq!((m.ema_warm, m.ema_guard), (8, 1));
        assert!(m.b_lo.get() != 0, "tx3's own bucket");
        let pam = h.with_env(|env| PammMarket::graduated(&env.svm, &pm));
        assert!(pool_accounts_exist(&h, &pam));

        let sent = h.chain().bundles_sent();
        assert_eq!(sent.len(), 1);
        let bundle = &sent[0];
        assert!(bundle.landed);
        eprintln!("{who:?}: | tx | top-level | trace | CU | bytes | accounts |");
        for (i, tx) in bundle.txs.iter().enumerate() {
            eprintln!(
                "| tx{} | {} | {} | {} | {} | {} |",
                i + 1,
                bundle.top[i],
                bundle.trace[i],
                bundle.units[i],
                tx.bytes,
                tx.accounts
            );
            assert!(tx.bytes <= 1_232 && tx.accounts <= 64, "v0, T-SIZE: {tx:?}");
            assert!(bundle.trace[i] <= 64);
        }
        assert_eq!(
            bundle.top[1], 3,
            "tx2: ComputeBudget ×2, migrate_v2, nothing else"
        );
        assert_eq!(bundle.trace[1], 2 + 51, "migrate_v2 alone with the budget");
        assert!(
            bundle.top[2] >= 5,
            "tx3: ComputeBudget ×2, the cranks, the tip"
        );
        // tx3: every crank is 1 + its `emit_cpi!` self-CPI; the created accounts add theirs.
        assert!(bundle.trace[2] <= 64);

        // The warm-up: observed every second, no exit before the gate; the expiry at T + 8.
        let outsider = h.with_env(|env| env.funded_keypair());
        let mut expired_at = None;
        for _ in 0..20 {
            h.idle(1);
            let ix = shortwin_litesvm_tests::engine::expire_ix(
                &pam,
                &outsider.pubkey(),
                &b.key(),
                nonce,
            );
            let before = accepts(&h, &outsider, ix);
            let r = h.tick().clone();
            assert!(r.errors.is_empty(), "{:?}", r.errors);
            let landed = r.exits.iter().any(|e| {
                e.position == position && e.kind == ExitKind::Expire && e.result.landed().is_some()
            });
            if landed {
                assert_eq!(before, Ok(()));
                expired_at = Some(h.now() - t);
                break;
            }
            assert_eq!(
                before,
                Err(Some(u32::from(ShortwinError::EmaWarmingUp))),
                "+{} s",
                h.now() - t
            );
            assert!(r.observed(&pm.market()), "every second of the warm-up");
            assert!(r.straddles.is_empty(), "the curve is gone");
        }
        assert_eq!(expired_at, Some(8), "{who:?}: at the warm-up's gate, T + 8");
        assert!(
            h.with_env(|env| read::borsh::<shortwin::state::ShortPosition>(&env.svm, &position))
                .is_none()
        );
    }
}

/// V1-9, the keeper's fallback (ARCH §5.4): tx3's pre-flight fails — the keeper sends tx1 + tx2
/// alone, the tip on tx2 (`with_tx3: false`); the curve is completed and migrated, and right after
/// the bundle — in the same second — the keeper's own transition moves the market onto its pool
/// (the pool accounts, the cranks); the position is expired on the pool at that gate (`T + 8`).
#[test]
fn when_tx3_fails_its_preflight_the_keeper_sends_tx1_and_tx2() {
    let (mut h, pm, q) = pump_setup(KeeperConfig::default());
    let (b, nonce) = straddled_position(&h, &pm, q);
    let position = pda::position(&pm.market(), &b.key(), nonce).0;
    h.chain()
        .fail_next_bundle_simulation(2, u32::from(ShortwinError::VenueAccountMismatch));
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(
        matches!(
            r.straddles[0].result,
            StraddleResult::Landed {
                with_tx3: false,
                ..
            }
        ),
        "{:?}",
        r.straddles[0].result
    );
    assert_eq!(r.straddles[0].txs, 2);
    let sent = h.chain().bundles_sent();
    assert_eq!(sent[0].txs.len(), 2);
    assert_eq!(sent[0].trace[1], 2 + 51 + 1, "the tip on tx2");
    assert!(curve(&h, &pm).virtual_token_reserves == 0, "migrated");
    // The keeper's own transition right after the bundle, in the same second.
    let t = h.now();
    assert_eq!(r.transitions.len(), 1);
    assert!(!r.transitions[0].bundled && r.transitions[0].result.landed().is_some());
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpSwap));
    let pam = h.with_env(|env| PammMarket::graduated(&env.svm, &pm));
    assert!(pool_accounts_exist(&h, &pam));
    let mut expired_at = None;
    for _ in 0..20 {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        if r.exits.iter().any(|e| {
            e.position == position && e.kind == ExitKind::Expire && e.result.landed().is_some()
        }) {
            expired_at = Some(h.now() - t);
            break;
        }
    }
    assert_eq!(expired_at, Some(8));
}

/// ★ T-STRADDLE at the keeper, the whole bundle (M6): a curve trade lands between the keeper's read
/// (and pre-flight) and its bundle. A buy of a third of `rT`: the three-transaction bundle still
/// lands — `complete_curve` buys the `rT` live when it runs, which is what the keeper ends up
/// holding, and tx3 moves the market onto its pool. A sale large enough to lift the completion
/// above the keeper's `max_sol_cost`: pump reverts tx1 (6002), the bundle commits nothing (the
/// curve is as the sale left it, the keeper holds nothing, the market is still on the curve), the
/// keeper alerts, and its bundle of the next second — built on a fresh read — lands.
#[test]
fn a_curve_trade_between_the_keepers_read_and_its_bundle() {
    // A buy.
    let (mut h, pm, q) = pump_setup(KeeperConfig::default());
    straddled_position(&h, &pm, q);
    let c = pm.curve;
    h.chain().before_next_bundle(move |env| {
        let u = CurveUser::new(env, &c);
        ok(u.buy(env, &c, q / 6, u64::MAX));
    });
    let r = h.second().clone();
    assert!(
        matches!(
            r.straddles[0].result,
            StraddleResult::Landed { with_tx3: true, .. }
        ),
        "{:?}",
        r.straddles[0].result
    );
    assert_eq!(r.straddles[0].rt_read, q / 2, "read before the buy");
    assert_eq!(
        tokens(&h, &keeper_ata(&h, &pm)),
        q / 2 - q / 6,
        "bought the rT live at execution"
    );
    assert_eq!(curve(&h, &pm).virtual_token_reserves, 0);
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpSwap));

    // A sale beyond the bound (and still a straddle: rT = 3q/4 < q).
    let (mut h, pm, q) = pump_setup(KeeperConfig::default());
    straddled_position(&h, &pm, q);
    let c = pm.curve;
    let sale = q / 4;
    h.chain().before_next_bundle(move |env| {
        let u = CurveUser::new(env, &c);
        ok(u.sell(env, &c, sale, 0));
    });
    let r = h.second().clone();
    let StraddleResult::Failed { outcome } = &r.straddles[0].result else {
        panic!("{:?}", r.straddles[0].result);
    };
    assert!(!outcome.landed);
    assert_eq!(
        outcome.txs[0].err.as_ref().and_then(|e| e.custom_code()),
        Some(fork::PUMP_ERR_TOO_MUCH_SOL_REQUIRED),
        "{outcome:?}"
    );
    let after_sale = curve(&h, &pm);
    assert!(!after_sale.complete);
    assert_eq!(
        after_sale.real_token_reserves,
        q / 2 + sale,
        "only the sale landed"
    );
    assert_eq!(tokens(&h, &keeper_ata(&h, &pm)), 0, "nothing of the bundle");
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpCurve));
    assert!(r.transitions.is_empty());
    assert!(r.alerts.iter().any(
        |a| matches!(a, Alert::StraddleWaiting { reason, .. } if reason.contains("did not land"))
    ));
    let r = h.second().clone();
    assert!(
        r.straddles[0].result.landed(),
        "{:?}",
        r.straddles[0].result
    );
    assert_eq!(r.straddles[0].rt_read, q / 2 + sale, "a fresh read");
    assert_eq!(tokens(&h, &keeper_ata(&h, &pm)), q / 2 + sale);
    assert_eq!(curve(&h, &pm).virtual_token_reserves, 0);
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpSwap));
}

#[test]
fn below_rt_the_keeper_expires_on_the_curve() {
    let (mut h, pm, q) = pump_setup(KeeperConfig::default());
    let (b, nonce) = h.with_env(|env| {
        env.crank(&pm);
        env.warp(1);
        let mut b = env.borrower();
        let (res, nonce) = env.open(&pm, &mut b, q);
        ok(res);
        let deadline = env.position(&pm, &b.key(), nonce).deadline;
        let now = env.now();
        env.warp(deadline - now + 1);
        (b, nonce)
    });
    let position = pda::position(&pm.market(), &b.key(), nonce).0;
    assert!(u128::from(q) <= u128::from(curve(&h, &pm).real_token_reserves));
    let mut expired = false;
    for _ in 0..5 {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.straddles.is_empty() && r.migrations.is_empty());
        if r.exits.iter().any(|e| {
            e.position == position && e.kind == ExitKind::Expire && e.result.landed().is_some()
        }) {
            expired = true;
            break;
        }
    }
    assert!(expired, "expired on the curve");
    assert!(
        h.with_env(|env| read::borsh::<shortwin::state::ShortPosition>(&env.svm, &position))
            .is_none()
    );
    assert!(!curve(&h, &pm).complete);
}

/// A chain without a block engine cannot take the bundle: the keeper alerts once and calls
/// `sol_settle` for the position meanwhile; nothing touches the curve.
#[test]
fn without_a_block_engine_the_straddle_waits() {
    let (mut h, pm, q) = pump_setup(KeeperConfig::default());
    h.chain().set_bundles(false);
    let (b, nonce) = straddled_position(&h, &pm, q);
    let position = pda::position(&pm.market(), &b.key(), nonce).0;
    let r = h.second().clone();
    let StraddleResult::Waiting { reason } = &r.straddles[0].result else {
        panic!("{:?}", r.straddles[0].result);
    };
    assert!(reason.contains("block engine"), "{reason}");
    assert!(
        r.exits
            .iter()
            .any(|e| e.position == position && e.kind == ExitKind::SolSettle)
    );
    let c = curve(&h, &pm);
    assert!(!c.complete && c.real_token_reserves == q / 2);
}

/// Duty 5 on a list-only market (M5, M2-SEC-5): a covered curve market's curve completes and pump
/// migrates it **elsewhere** (reserves zeroed, no canonical pool; planted on pump's account) with
/// an expired position open. The keeper covers the market with its detection set as ReadSet: its
/// first crank moves it `PumpCurve → None` (graduated elsewhere; the S3 chain starts), it keeps
/// the chain on the no-route cadence (an observation at least every `no_route_interval_s`), sends
/// no migration, no straddle and no trading exit, and settles the position in S3 on the detection
/// set as soon as the chain has run `s3_wait_s`.
#[test]
fn a_curve_that_graduated_elsewhere_is_settled_in_s3_by_the_keeper() {
    let (mut h, pm, q) = pump_setup(KeeperConfig::default());
    let (b, nonce) = h.with_env(|env| {
        env.crank(&pm);
        env.warp(1);
        let mut b = env.borrower();
        let (res, nonce) = env.open(&pm, &mut b, q);
        ok(res);
        env.warp(1);
        // The curve completes and pump migrates it elsewhere before the keeper's next tick.
        buy_down_to(env, &pm.curve, 0);
        fork::set_curve_reserves(&mut env.svm, &pm.curve.curve, 0, 0, 0, 0);
        let deadline = env.position(&pm, &b.key(), nonce).deadline;
        let now = env.now();
        env.warp(deadline - now + 1);
        (b, nonce)
    });
    let position = pda::position(&pm.market(), &b.key(), nonce).0;
    let s3_wait = h.with_env(|env| env.position(&pm, &b.key(), nonce).s3_wait_s);
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::None));
    let since = h.with_env(|env| env.market(&pm.market()).no_route_since);
    assert!(since > 0, "the S3 chain runs");
    let mut last_obs = h.now();
    let mut settled_at = None;
    for _ in 0..(i64::from(s3_wait) + 120) {
        let r = h.second().clone();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.migrations.is_empty() && r.straddles.is_empty());
        assert!(r.exits.iter().all(|e| e.kind == ExitKind::SolSettle));
        let now = h.now();
        if r.cranked.contains(&pm.market()) || r.observed(&pm.market()) {
            last_obs = now;
        }
        assert!(
            now - last_obs <= KeeperConfig::default().coverage.no_route_interval_s,
            "the no-route cadence at {now} (last observation {last_obs})"
        );
        if r.exits.iter().any(|e| {
            e.position == position && e.kind == ExitKind::SolSettle && e.result.landed().is_some()
        }) && h
            .with_env(|env| read::borsh::<shortwin::state::ShortPosition>(&env.svm, &position))
            .is_none()
        {
            settled_at = Some(now);
            break;
        }
    }
    let at = settled_at.expect("settled in S3");
    assert!(
        at >= since + i64::from(s3_wait),
        "not before the chain ran s3_wait_s"
    );
    assert!(
        at <= since + i64::from(s3_wait) + KeeperConfig::default().coverage.s3_probe_interval_s + 1,
        "at the first probe after it ({at} vs {since} + {s3_wait})"
    );
    println!(
        "graduated elsewhere: the keeper kept the S3 chain from {since} and settled at +{} s",
        at - since
    );
}

/// `crank_price`'s `None → PumpCurve` re-detection by the keeper (M5; ARCH §2.5 `None` row): a mint
/// listed before its curve was readable (the curve account removed for the listing, restored
/// after) is list-only; the keeper sees a live curve in its detection set (the program's
/// `classify_pump`) and cranks it onto the curve, then creates the trader's base ATA and pump UVA
/// the curve legs require (the crank has no payer and creates neither), once; a borrower's open
/// then lands on the curve.
#[test]
fn a_list_only_market_is_redetected_and_prepared_by_the_keeper() {
    let (mut env, c) = Env::fork_pump_coin(PumpCoin::T22_115);
    let saved = env.svm.get_account(&c.curve).unwrap();
    env.svm
        .set_account(c.curve, solana_account::Account::default())
        .unwrap();
    let creator = env.funded_keypair();
    let list = shortwin_litesvm_tests::ix::create_market(
        &solana_signer::Signer::pubkey(&creator),
        &c.mint,
        &c.token_program.id(),
        shortwin::instructions::create_market::VENUE_HINT_AUTO,
        &pda::canonical_pool(&c.mint),
    );
    ok(env.send(&[list], &[&creator]));
    env.svm.set_account(c.curve, saved).unwrap();
    let q = c.state(&env.svm).virtual_token_reserves / 200;
    let pm = env.pump_market_listed(PumpSpec::new(PumpCoin::T22_115, q * 40), c);
    assert_eq!(
        env.market(&pm.market()).venue_kind(),
        Some(VenueKind::None),
        "listed without a curve: list-only"
    );
    let mut h = Harness::new(env, KeeperConfig::default());
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(r.cranked.contains(&pm.market()), "the keeper cranks it");
    assert_eq!(market_kind(&h, &pm), Some(VenueKind::PumpCurve));
    let [base, uva] = h.with_env(|env| graduation::trader_account_keys(&env.market(&pm.market())));
    assert_eq!(base, pm.trader_base_ata);
    let missing = |h: &Harness| {
        h.with_env(|env| {
            (
                read::account(&env.svm, &base).is_none(),
                read::account(&env.svm, &uva).is_none(),
            )
        })
    };
    assert_eq!(missing(&h), (true, true), "the crank created nothing");
    let r = h.second().clone();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.curve_accounts.len(), 1);
    assert!(
        r.curve_accounts[0].1.landed().is_some(),
        "{:?}",
        r.curve_accounts
    );
    assert_eq!(missing(&h), (false, false), "the keeper created both");
    for _ in 0..3 {
        let r = h.second().clone();
        assert!(r.curve_accounts.is_empty(), "once");
    }
    // A borrower's open lands on the curve.
    h.with_env(|env| {
        env.warp(i64::from(env.config().params.market_warmup_s));
        env.crank(&pm);
        env.warp(1);
        let mut b = env.borrower();
        let (res, _) = env.open(&pm, &mut b, q);
        ok(res);
    });
}
