//! The lender pool against **real** exits (M10, WP-8c; the adversarial matrix's G-F batch:
//! ARCH §8.2's lender cases, ECON §9.4–§9.6, R3B-2, C0-6, C0-9, V2-2, R3A-5). The lender lab
//! (`tests/lender_it`) proves these cases with the borrower side injected through
//! `shortwin_math::pool`; here every lump, rent payment and wipe comes from a real `liquidate`,
//! `expire`, `sol_settle`, `close_short` or `sweep_trader` (the mock venue, test-venue build;
//! and one PumpSwap cashback lump on the fork), and every instruction goes through `Env::send`,
//! so the invariant hooks (I1, I2, I6, I8 with its allowance, claim and liveness clauses, …)
//! run after each one and on every prefix of a bundle.
//!
//! - `jit` (G-F2, R3B-2): a whale deposits 9× the pool around a real lump — a liquidation's
//!   penalty, an expiry's fee, a `sweep_trader` of donated lamports — in **one transaction**
//!   `[deposit, exit, claim_sol, withdraw]` and realises **0** lamports (its tokens come back as
//!   deposited); the honest lender's take equals the run without the whale; a whale that holds
//!   `h` seconds takes its share of the vest released by then (linear in `h`: the control that
//!   shows the vesting is what stops the capture). **Finding M10-8C-1** (`#[ignore]`d,
//!   failing): within `min_rent_s` of the open the exit's `R_min` top-up is credited at once,
//!   and the whale takes its share.
//! - `bundles` (G-F5, G-F7): `[withdraw, sol_settle]` in one transaction: the withdrawal is held
//!   to the pro-rata allowance of the state before the settlement (`AboveProRataAllowance` one
//!   token above it, the whole bundle reverted), the S2 lump vests, the settlement's amounts are
//!   those of the run without the withdrawal, and the withdrawal only exchanges part of the
//!   stayer's token claim for the withdrawer's SOL at the settlement's own price (even at that
//!   price; ECON §12.7(c)'s disclosed residual at a higher one);
//!   `claim_sol ×N` in one transaction after a real liquidation's lump pays once (the second
//!   and later claims pay 0), in the vest and after it.
//! - `wipes` (G-F4, C0-6, V2-12): two consecutive real full wipes (S1, then S2) on one market:
//!   each rolls an epoch into a live record (the S2 lump flushed into it), the post-wipe
//!   deposit (bundled with its `sweep_trader`) mints at the initial rate, shares stay bounded,
//!   each wiped lender claims its own settlement (but dust) through the records.
//! - `rent` (G-F13, R3A-5, A26): `rent_protocol_bps` 15 % → 30 % through the timelock with two
//!   positions open: their snapshots hold, a close and a liquidation split rent at 15 %, a
//!   position opened after the change at 30 %; at zero positions `receivable`, `A_j` and
//!   `rent_aggr` are 0.
//! - `pswap` (G-F2 on a real venue): the JIT whale around PumpSwap's cashback lump `[create
//!   ATA(trader, WSOL), pAMM claim_cashback(user = trader), sweep_trader]` (the as-built
//!   `claim_venue_rewards`, which is not an instruction of its own; ARCH §3.9) realises 0.
//!
//! Every scenario ends with no position open and asserts `receivable = 0` and `A_j =
//! rent_aggr = 0` (I-P7) explicitly, besides the I8 hook.

#![allow(clippy::result_large_err)]

#[path = "settle_support.rs"]
mod support;

/// The PumpSwap adapter tests' helpers (`pamm_it/common.rs`), for `pswap`.
#[allow(unused_imports)]
#[path = "pamm_it/common.rs"]
mod pswap_common;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use litesvm::types::TransactionMetadata;
use shortwin::events::{ClaimSol as ClaimSolEv, Deposit as DepositEv, Withdraw as WithdrawEv};
use shortwin_litesvm_tests::drive::Actor;
use shortwin_litesvm_tests::engine::{expire_ix, liquidate_ix, sweep_trader_ix_for};
use shortwin_litesvm_tests::ix;
use solana_keypair::Keypair;
use solana_signer::Signer;
use support::*;

/// SOL paid to `owner` by the `withdraw` and `claim_sol` instructions of a transaction (their
/// events' `sol_paid`).
fn sol_paid(meta: &TransactionMetadata, owner: &Pubkey) -> u64 {
    let w: u64 = events::of::<WithdrawEv>(meta)
        .iter()
        .filter(|e| e.owner == *owner)
        .map(|e| e.sol_paid)
        .sum();
    let c: u64 = events::of::<ClaimSolEv>(meta)
        .iter()
        .filter(|e| e.owner == *owner)
        .map(|e| e.sol_paid)
        .sum();
    w + c
}

/// Tokens paid to `owner` by the `withdraw` instructions of a transaction (their events'
/// `amount`).
fn tokens_withdrawn(meta: &TransactionMetadata, owner: &Pubkey) -> u64 {
    events::of::<WithdrawEv>(meta)
        .iter()
        .filter(|e| e.owner == *owner)
        .map(|e| e.amount)
        .sum()
}

/// `claim_sol` by `owner` with the `ShareEpoch` records it is behind on; 0 if it has no lender
/// position (left with nothing owed).
fn claim(env: &mut Env, market: &Pubkey, owner: &Keypair) -> u64 {
    let Some(l) = env.lender(market, &owner.pubkey()) else {
        return 0;
    };
    let m = env.market(market);
    let recs = ix::epoch_records(market, l.epoch, m.share_epoch);
    let claim = ix::with_remaining(ix::claim_sol(&owner.pubkey(), market), &recs);
    let meta = ok(env.send(&[claim], &[owner]));
    sol_paid(&meta, &owner.pubkey())
}

/// Two rounds of [`claim`] by every lender in turn (ECON §9.4's wind-down: a lender's surplus
/// reaches the pot at its own touch, so a lender that claimed before it draws it in the second
/// round). What each was paid.
fn claim_all(env: &mut Env, market: &Pubkey, lenders: &[&Keypair]) -> Vec<u64> {
    let mut paid = vec![0; lenders.len()];
    for _ in 0..2 {
        for (i, l) in lenders.iter().enumerate() {
            paid[i] += claim(env, market, l);
        }
    }
    paid
}

/// I-P7 / ECON §13 (R3A-5): with no position open, `receivable = 0` and `A_j` (`rent_aggr`) is
/// 0 in every tier — asserted here explicitly, besides the I8 hook.
fn assert_zero_at_zero_positions(env: &Env, market: &Pubkey) {
    let m = env.market(market);
    assert_eq!(m.open_positions, 0, "no position is open");
    assert_eq!((m.borrowed_q, m.receivable), (0, 0), "receivable = 0");
    let aggr: Vec<u128> = m.rent_aggr.iter().map(|a| a.get()).collect();
    assert_eq!(aggr, [0, 0, 0], "A_j = rent_aggr = 0");
}

/// Whether `ixs` would land now (sent on a copy of the VM; the state is restored).
fn lands(env: &mut Env, ixs: &[Instruction], signers: &[&Keypair]) -> bool {
    let saved = env.svm.clone();
    let landed = env.send(ixs, signers).is_ok();
    env.svm = saved;
    landed
}

/// The lender keys of an actor on a mock market (its base account as the token account).
fn keys_of(mm: &MockMarket, a: &Actor) -> ix::LenderKeys {
    ix::LenderKeys {
        owner: a.key(),
        mint: mm.mint(),
        token_program: mm.keys.token_program,
        owner_token_account: a.base,
    }
}

/// A [`Sc`] on `spec` whose `before_open` (e.g. a second lender's deposit) runs before the
/// position of `q` opens.
fn sc_with(spec: MarketSpec, q: u64, before_open: impl FnOnce(&mut Env, &MockMarket)) -> Sc {
    let (mut env, mm) = setup_with(spec);
    before_open(&mut env, &mm);
    let mut b = env.borrower();
    let (_open, nonce, _) = open_ok(&mut env, &mm, &mut b, q);
    let keeper = env.funded_keypair();
    let whale = env.actor(
        &mm,
        spec.base.saturating_mul(10),
        spec.quote.saturating_mul(10),
    );
    Sc {
        env,
        mm,
        b,
        nonce,
        q,
        keeper,
        whale,
    }
}

/// The lump-vesting window `lump_vest_s` of the config.
fn vest_s(env: &Env) -> i64 {
    i64::from(env.config().params.lump_vest_s)
}

/// A genuine rise (up to 2.4× over 30 s, then held) with one honest crank per second, until
/// `liquidate` of `b`'s position `nonce` would land in the current second.
fn rise_until_liquidatable(sc: &mut Sc, b: &Pubkey, nonce: u64) {
    let p0 = sc.spot() as f64;
    let kp = sc.keeper.insecure_clone();
    for s in 1..=120u32 {
        let m = 2.4f64.powf(f64::from(s.min(30)) / 30.0);
        sc.env.warp(1);
        sc.move_to((p0 * m) as u128);
        sc.crank();
        let liq = liquidate_ix(&sc.mm, &kp.pubkey(), b, nonce);
        if lands(&mut sc.env, &[liq], &[&kp]) {
            return;
        }
    }
    panic!("never liquidatable");
}

mod jit {
    use super::*;
    use shortwin::events::{Expire, Liquidate, SWEEP_TO_VEST, TraderSwept};

    /// What makes the lump.
    #[derive(Clone, Copy, Debug)]
    enum Lump {
        /// `liquidate` after a genuine rise, the position open longer than `min_rent_s`: the
        /// penalty's lender share vests, the rent was credited while it accrued.
        Liquidate,
        /// The same within `min_rent_s` of the open: the rent paid is `R_min`, above what
        /// accrued (finding M10-8C-1).
        LiquidateEarly,
        /// `expire` at the deadline: the expiry fee (100 % lenders) vests.
        Expire,
        /// `sweep_trader` of 5 SOL donated to the trader PDA: the donation vests.
        Sweep,
    }

    const DONATION: u64 = 5_000_000_000;

    /// The default mock market driven to the second in which `kind`'s instruction lands (its
    /// one position liquidatable or at its deadline; for the sweep, closed by its borrower and
    /// 5 SOL donated to the trader PDA), and a JIT whale holding 9× the lent tokens.
    fn ready(kind: Lump) -> (Sc, Actor) {
        let mut sc = Sc::default_market();
        sc.env.tick(&sc.mm, 5);
        match kind {
            Lump::Liquidate | Lump::LiquidateEarly => {
                if matches!(kind, Lump::Liquidate) {
                    // Past every tier's `min_rent_s`: the rent due is what accrued.
                    let min_rent = sc.env.config().tiers.iter().map(|t| t.min_rent_s).max();
                    let past = sc.pos().t_open + i64::from(min_rent.unwrap()) + 60;
                    sc.env.warp(past - sc.env.now());
                    sc.env.tick(&sc.mm, 5);
                }
                let (b, n) = (sc.b.key(), sc.nonce);
                rise_until_liquidatable(&mut sc, &b, n);
            }
            Lump::Expire => {
                // Daily cranks, then one a second up to the deadline (the expiry needs the
                // previous second's completed bucket, `ref_max_age_s`).
                let deadline = sc.pos().deadline;
                while sc.env.now() + 86_400 < deadline - 10 {
                    sc.env.warp(86_400);
                    sc.crank();
                }
                sc.env.warp(deadline - 10 - sc.env.now());
                sc.env.tick(&sc.mm, 9);
                sc.env.warp(1);
            }
            Lump::Sweep => {
                // No position open (the whale may take all its tokens back at once).
                let (b, n) = (sc.b.kp.insecure_clone(), sc.nonce);
                let b = Borrower {
                    kp: b,
                    next_nonce: n + 1,
                };
                close_ok(&mut sc.env, &sc.mm, &b, n);
                let trader = sc.mm.keys.trader;
                sc.env.fund(&trader, DONATION);
            }
        }
        let lend = sc.mm.spec.lend;
        let jit = sc.env.actor(&sc.mm, 9 * lend, 0);
        let exit = exit_ixs(&sc, kind);
        let kp = sc.keeper.insecure_clone();
        assert!(lands(&mut sc.env, &exit, &[&kp]), "{kind:?} lands now");
        assert_eq!(sc.market().vest_bal, 0, "no lump vesting before");
        (sc, jit)
    }

    /// The lump's instruction(s).
    fn exit_ixs(sc: &Sc, kind: Lump) -> Vec<Instruction> {
        let k = sc.keeper.pubkey();
        match kind {
            Lump::Liquidate | Lump::LiquidateEarly => {
                vec![liquidate_ix(&sc.mm, &k, &sc.b.key(), sc.nonce)]
            }
            Lump::Expire => vec![expire_ix(&sc.mm, &k, &sc.b.key(), sc.nonce)],
            Lump::Sweep => vec![sweep_trader_ix_for(&sc.env, &sc.mm.market())],
        }
    }

    /// The lenders' vesting lump of the exit and the rent it paid, from its event.
    fn lump_of(meta: &TransactionMetadata, kind: Lump) -> (u64, u64) {
        match kind {
            Lump::Liquidate | Lump::LiquidateEarly => {
                let ev: Liquidate = events::one(meta);
                (ev.penalty_lender, ev.rent)
            }
            Lump::Expire => {
                let ev: Expire = events::one(meta);
                (ev.expiry_fee, ev.rent)
            }
            Lump::Sweep => {
                let ev: TraderSwept = events::one(meta);
                assert_eq!(ev.to, SWEEP_TO_VEST);
                (ev.lamports, 0)
            }
        }
    }

    /// What one [`run`] measured.
    struct Run {
        /// The lump (lenders' vesting part of the exit).
        lump: u64,
        /// The rent the exit paid (`R_due`).
        rent: u64,
        /// The whale's realised SOL, everything it was ever paid.
        whale: u64,
        /// The whale's shares and the pool's shares just after its deposit.
        whale_frac: (u128, u128),
        /// The honest (seeding) lender's SOL claimed after the vest.
        honest: u64,
    }

    /// One run from [`ready`]: with `hold = Some(h)` the whale deposits 9× in the lump's
    /// transaction and leaves `h` seconds later (with `h = 0` all in that one transaction:
    /// `[deposit, exit, claim_sol, withdraw]`); `None` is the run without the whale. Then
    /// `lump_vest_s + 1` seconds pass from the lump and every lender claims.
    fn run(sc: &mut Sc, jit: &Actor, kind: Lump, hold: Option<i64>) -> Run {
        let market = sc.mm.market();
        let exit = exit_ixs(sc, kind);
        let (kp, wk) = (sc.keeper.insecure_clone(), jit.kp.insecure_clone());
        let mut out = Run {
            lump: 0,
            rent: 0,
            whale: 0,
            whale_frac: (0, 0),
            honest: 0,
        };
        let t0 = sc.env.now();
        match hold {
            None => {
                let meta = ok(sc.env.send(&exit, &[&kp]));
                (out.lump, out.rent) = lump_of(&meta, kind);
            }
            Some(hold) => {
                let keys = keys_of(&sc.mm, jit);
                let amount = 9 * sc.mm.spec.lend;
                let m = sc.market();
                let minted = shortwin_math::pool::shares_for_tokens(&m.pool(), amount).unwrap();
                let mut ixs = vec![ix::deposit(&keys, amount, minted)];
                ixs.extend(exit);
                if hold == 0 {
                    ixs.push(ix::claim_sol(&jit.key(), &market));
                    ixs.push(ix::withdraw(&keys, minted, 0));
                }
                let meta = ok(sc.env.send(&ixs, &[&kp, &wk]));
                let dep: DepositEv = events::one(&meta);
                assert_eq!(dep.shares, minted);
                out.whale_frac = (minted, dep.s_after);
                (out.lump, out.rent) = lump_of(&meta, kind);
                out.whale += sol_paid(&meta, &jit.key());
                let mut back = tokens_withdrawn(&meta, &jit.key());
                if hold > 0 {
                    sc.env.warp(hold);
                    let meta = ok(sc.env.send(
                        &[
                            ix::claim_sol(&jit.key(), &market),
                            ix::withdraw(&keys, minted, 0),
                        ],
                        &[&wk],
                    ));
                    out.whale += sol_paid(&meta, &jit.key());
                    back += tokens_withdrawn(&meta, &jit.key());
                }
                // Every lump is SOL: the whale's tokens come back at most as deposited (less
                // the floors of its mint and burn).
                eprintln!("{kind:?}, h = {hold}: the whale's tokens back {back} of {amount}");
                assert!(
                    back <= amount && amount - back <= 2,
                    "{kind:?}, h = {hold}: the whale's tokens back {back} of {amount}"
                );
            }
        }
        // Everything vests; then every lender claims what it is owed.
        let to = t0 + vest_s(&sc.env) + 1;
        sc.env.warp(to - sc.env.now());
        sc.crank();
        assert_eq!(sc.market().vest_bal, 0, "the lump has vested");
        let honest = sc.mm.lender.insecure_clone();
        let paid = claim_all(&mut sc.env, &market, &[&wk, &honest]);
        out.whale += paid[0];
        out.honest = paid[1];
        assert!(
            sc.env.lender(&market, &jit.key()).is_none(),
            "the whale left with nothing owed"
        );
        out
    }

    /// The whale's share of `released` lamports of vest (its shares over the pool's).
    fn share_of(released: u128, (w, s): (u128, u128)) -> u128 {
        released * w / (s + shortwin_math::pool::VIRTUAL_SHARES)
    }

    /// G-F2 / R3B-2 on one lump: the whale realises 0 at `h = 0`, and the honest lender's take
    /// is the run's without the whale; holding `h` of the vest window it takes its share of what
    /// vested by then (`h/lump_vest_s` of the lump: linear in `h`), the whole share at `h =
    /// lump_vest_s` — the control that shows the vesting (not the bundle's shape) is what stops
    /// the capture.
    fn jit_case(kind: Lump) {
        let (mut sc, jit) = ready(kind);
        let r_min = sc.pos_opt().map(|p| p.r_min);
        let saved = sc.env.svm.clone();
        let control = run(&mut sc, &jit, kind, None);
        eprintln!(
            "{kind:?}: rent paid {} (the position's R_min {r_min:?}), lump {}",
            control.rent, control.lump
        );
        assert!(control.lump > 0, "{kind:?}: a real lump");
        assert!(
            control.honest >= control.lump,
            "the honest lender is paid the lump"
        );

        let vest = vest_s(&sc.env);
        let mut rows = Vec::new();
        for hold in [0, vest / 4, vest / 2, vest] {
            sc.env.svm = saved.clone();
            let r = run(&mut sc, &jit, kind, Some(hold));
            assert_eq!(
                r.lump, control.lump,
                "{kind:?}: the whale does not change the lump"
            );
            let released = u128::from(r.lump) * hold as u128 / vest as u128;
            let want = share_of(released, r.whale_frac);
            eprintln!(
                "{kind:?}: lump {} hold {hold:>5} s: whale {} (share of the vested part {want}), \
                 honest {} (alone {})",
                r.lump, r.whale, r.honest, control.honest
            );
            if hold == 0 {
                assert_eq!(r.whale, 0, "{kind:?}: the JIT whale realises 0 at h = 0");
                assert!(
                    r.honest.abs_diff(control.honest) <= 2,
                    "{kind:?}: the honest take {} is the run's without the whale {}",
                    r.honest,
                    control.honest
                );
            } else {
                assert!(
                    u128::from(r.whale).abs_diff(want) <= 2 + want / 1_000_000,
                    "{kind:?}, h = {hold}: the whale took {} of the vested part's share {want}",
                    r.whale
                );
                assert!(
                    (r.whale + r.honest).abs_diff(control.honest) <= 2,
                    "{kind:?}, h = {hold}: what the whale takes the honest lender loses"
                );
            }
            rows.push(r.whale);
            assert_zero_at_zero_positions(&sc.env, &sc.mm.market());
        }
        assert!(
            rows[1] > 0 && rows[1] < rows[2] && rows[2] < rows[3],
            "{kind:?}: the capture grows with the hold time: {rows:?}"
        );
        assert!(
            rows[2].abs_diff(2 * rows[1]) <= 4 && rows[3].abs_diff(2 * rows[2]) <= 4,
            "{kind:?}: linear in h: {rows:?}"
        );
    }

    /// The liquidation of a position open past `min_rent_s` (its rent was credited while it
    /// accrued): the penalty's lender half vests.
    #[test]
    fn a_jit_whale_around_a_real_liquidation_realises_nothing() {
        jit_case(Lump::Liquidate);
    }

    /// **Finding M10-8C-1** (kept failing; the program is unchanged in this lane). Within
    /// `min_rent_s` of the open an exit pays `R_due = R_min`, above the rent credited while the
    /// position was open, and `pool::repay` credits the difference to `acc` **at once** (ARCH
    /// §3.3 `repay`: "credits the entitlement difference `lender_rent − credited_for(pos)`"),
    /// not through the vest. A whale's `[deposit 9×, liquidate, claim_sol, withdraw]` around the
    /// liquidation of a ≈ 3 SOL position opened a minute before takes ≈ 9/10 of that top-up
    /// atomically: 128,972 of the lenders' 145,549 lamports of its `R_min` (171,233), and the
    /// honest lender loses as much — against ECON §9.4's "the atomic bundle gets nothing". The
    /// same true-up runs for `close_short` and for the immediate part of an S2/S3 settlement
    /// (`⌊Q·P_ema/WAD⌋ + R_due`; ARCH §3.3, by reading — not exercised here). Bounded by the
    /// lenders' part of `min_rent_s` of rent on the position.
    #[test]
    #[ignore = "M10 finding M10-8C-1: the R_min top-up of an exit within min_rent_s is credited at once, a JIT whale takes its share"]
    fn finding_m10_8c_1_a_jit_whale_takes_the_r_min_top_up_of_an_early_liquidation() {
        jit_case(Lump::LiquidateEarly);
    }

    /// The expiry at the deadline: the 2 % expiry fee vests (ECON §12.2's R3B-2 lump).
    #[test]
    fn a_jit_whale_around_a_real_expiry_realises_nothing() {
        jit_case(Lump::Expire);
    }

    /// `sweep_trader` of lamports donated to the trader PDA: the donation vests.
    #[test]
    fn a_jit_whale_around_a_trader_sweep_realises_nothing() {
        jit_case(Lump::Sweep);
    }
}

mod bundles {
    use super::*;
    use shortwin::errors::ShortwinError as E;
    use shortwin::events::{Liquidate, SolSettle};

    /// Two lenders of 1.75·10¹² tokens each (A, the seeding lender, and B) and a position of
    /// [`Q`] (86 % utilisation), on the default pool.
    fn two_lenders() -> (Sc, Actor) {
        let lend = 1_750_000_000_000;
        let spec = MarketSpec {
            lend,
            ..MarketSpec::default()
        };
        let mut lb = None;
        let sc = sc_with(spec, Q, |env, mm| {
            let b = env.actor(mm, lend, 0);
            let kp = b.kp.insecure_clone();
            ok(env.send(&[ix::deposit(&keys_of(mm, &b), lend, 0)], &[&kp]));
            lb = Some(b);
        });
        (sc, lb.unwrap())
    }

    /// The most shares of `owner` whose tokens are at most `tokens` on the market now.
    fn shares_for_at_most(sc: &Sc, owner: &Pubkey, tokens: u64) -> u128 {
        let pool = sc.market().pool();
        let held = sc.env.lender(&sc.mm.market(), owner).unwrap().shares;
        let (mut lo, mut hi) = (0u128, held);
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if shortwin_math::pool::tokens_for_shares(&pool, mid).unwrap() <= u128::from(tokens) {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    }

    /// Lender `owner`'s token claim now (0 without a lender position).
    fn token_claim(sc: &Sc, owner: &Pubkey) -> u128 {
        sc.env.lender(&sc.mm.market(), owner).map_or(0, |l| {
            shortwin_math::pool::token_value(&sc.market().pool(), &l.lender()).unwrap()
        })
    }

    /// Lender `owner`'s pro-rata allowance now, as the program computes it (the lender touched
    /// alone; `pool::allowance_view`).
    fn allowance(sc: &Sc, owner: &Pubkey) -> u64 {
        let market = sc.mm.market();
        let m = sc.market();
        let v = sc.env.market_vault(&market).vault();
        let l = sc.env.lender(&market, owner).unwrap().lender();
        shortwin_math::pool::allowance_view(&m.pool(), &v, &l, &[], sc.env.now()).unwrap()
    }

    /// G-F5 / C0-9: `[withdraw, sol_settle]` in one transaction on a real S2 settlement (an LP
    /// drain, `settle_it::s2_is_capped_at_fair_value`'s model). One token above the pro-rata
    /// allowance of the state before the settlement the bundle reverts with
    /// `AboveProRataAllowance` (nothing settles); at the allowance it lands, the settlement's
    /// amounts are exactly the run's without the withdrawal and its lump vests. After the vest,
    /// against the run without the withdrawal: the SOL and the tokens are all claimed either
    /// way; B has ceded `dt` of its token claim (`0 < dt ≤` what A withdrew) and been paid `ds`
    /// of A's SOL, and `ds` is `dt` at the price the settlement realised for the borrowed tokens
    /// (`lender_amount / Q`, to 10 ppm) — at that price B's total is the control's. At a price
    /// above it (the S2 cap paid the lenders below market) A keeps that discount on `dt`: the
    /// residual of a pro-rata withdrawal ahead of a lossy settlement that ECON §12.7(c)
    /// discloses (+4.3 % in its model; the run residual ARCH §10 accepts for the MVP, O10),
    /// which the allowance (the refusal above) bounds.
    #[test]
    fn withdraw_then_settle_in_one_transaction_keeps_the_pro_rata_part() {
        let (mut sc, lb) = two_lenders();
        sc.env.tick(&sc.mm, 5);
        sc.drain(9_850);
        sc.env.tick(&sc.mm, 90);
        sc.move_by(1.8);
        // Keeper seconds up to the one whose settlement executes.
        let mut found = false;
        for _ in 0..120 {
            let saved = sc.env.svm.clone();
            if sc.second().settled.is_some() {
                sc.env.svm = saved;
                found = true;
                break;
            }
        }
        assert!(found, "an S2 settlement");
        sc.env.warp(1);
        sc.crank();
        let ready = sc.env.svm.clone();
        let market = sc.mm.market();
        let la = sc.mm.lender.insecure_clone();

        // The control: the settlement alone.
        let ctl_ev = Outcome::of(&ok(sc.settle())).settled.expect("settles");
        assert_eq!(ctl_ev.condition, 2, "S2");
        assert!(ctl_ev.lump > 0);
        sc.env.warp(vest_s(&sc.env) + 1);
        sc.crank();
        let ctl = claim_all(&mut sc.env, &market, &[&la, &lb.kp]);
        let (ctl_a, ctl_b) = (ctl[0], ctl[1]);
        let (ctl_ta, ctl_tb) = (token_claim(&sc, &la.pubkey()), token_claim(&sc, &lb.key()));

        // A's bundle one token above its allowance: refused, nothing settled.
        sc.env.svm = ready.clone();
        let a = allowance(&sc, &la.pubkey());
        let free = sc.market().tokens_free;
        assert!(
            a > 0 && a < free,
            "the allowance {a} binds below T = {free}"
        );
        let over = shares_for_at_most(&sc, &la.pubkey(), a + 1);
        assert!(
            shortwin_math::pool::tokens_for_shares(&sc.market().pool(), over).unwrap()
                > u128::from(a)
        );
        let settle = sc.settle_ix();
        let kp = sc.keeper.insecure_clone();
        let keys = sc.mm.lender_keys();
        assert_err(
            &sc.env
                .send(&[ix::withdraw(&keys, over, 0), settle.clone()], &[&la, &kp]),
            E::AboveProRataAllowance,
        );
        assert!(
            sc.pos_opt().is_some(),
            "the bundle reverted: nothing settled"
        );

        // At the allowance: lands; the settlement is the control's.
        let s = shares_for_at_most(&sc, &la.pubkey(), a);
        let vest0 = sc.market().vest_bal;
        let meta = ok(sc
            .env
            .send(&[ix::withdraw(&keys, s, 0), settle], &[&la, &kp]));
        let w: WithdrawEv = events::one(&meta);
        assert!(w.amount <= a, "withdrew {} of the allowance {a}", w.amount);
        let ev: SolSettle = events::one(&meta);
        assert_eq!(
            (ev.condition, ev.lender_amount, ev.lump, ev.fv),
            (
                ctl_ev.condition,
                ctl_ev.lender_amount,
                ctl_ev.lump,
                ctl_ev.fv
            ),
            "the withdrawal does not move the settlement"
        );
        assert_eq!(
            sc.market().vest_bal,
            vest0 + ev.lump,
            "the settlement's lump vests"
        );
        let mut paid_a = w.sol_paid;
        paid_a += claim(&mut sc.env, &market, &la);
        sc.env.warp(vest_s(&sc.env) + 1);
        sc.crank();
        let paid = claim_all(&mut sc.env, &market, &[&la, &lb.kp]);
        paid_a += paid[0];
        let paid_b = paid[1];
        // Each lender's token claim at the end (A's counts what it withdrew).
        let ta = u128::from(w.amount) + token_claim(&sc, &la.pubkey());
        let tb = token_claim(&sc, &lb.key());
        eprintln!(
            "[withdraw {} of {a}, sol_settle S2 {} (lump {})]: A {paid_a} SOL + {ta} tokens, \
             B {paid_b} SOL + {tb} tokens; without the withdrawal A {ctl_a} + {ctl_ta}, \
             B {ctl_b} + {ctl_tb}",
            w.amount, ev.lender_amount, ev.lump
        );
        assert!(
            (paid_a + paid_b).abs_diff(ctl_a + ctl_b) <= 2,
            "the settlement's SOL is all paid either way"
        );
        assert!(
            (ta + tb).abs_diff(ctl_ta + ctl_tb) <= 2,
            "the tokens are all claimed either way"
        );
        // What the withdrawal moved between the two: B ceded `dt` of its token claim to A (A's
        // burn priced its shares' claim on the borrowed tokens at par) and A ceded `ds` of its
        // SOL to B. Valued at the price the settlement realised for the borrowed tokens
        // (`lender_amount` for `Q`), the exchange is even: B's total is the control's.
        let dt = ctl_tb.checked_sub(tb).expect("B's token claim falls");
        let ds = paid_b.checked_sub(ctl_b).expect("B's SOL rises");
        assert!(
            ta - ctl_ta == dt && ctl_a - paid_a == ds,
            "A gains what B cedes"
        );
        assert!(
            dt > 0 && dt <= u128::from(w.amount),
            "A gains at most the tokens it withdrew: {dt} of {}",
            w.amount
        );
        let at_settle = dt * u128::from(ev.lender_amount) / u128::from(Q);
        eprintln!(
            "B ceded {dt} tokens (worth {at_settle} lamports at the settlement's price) for {ds} \
             lamports"
        );
        // 10 ppm: the floors, and the position's rent credited before the bundle (A was paid
        // its part at its touch, at the old shares).
        assert!(
            u128::from(ds).abs_diff(at_settle) <= at_settle / 100_000,
            "B is paid for the tokens it ceded at the settlement's price: {ds} vs {at_settle}"
        );
        assert_zero_at_zero_positions(&sc.env, &market);
    }

    /// G-F7 / V2-2: `claim_sol ×N` in one transaction after a real liquidation's lump pays once
    /// — the first claim what the lender is owed (exactly a single claim's amount), the second
    /// and later ones 0, `lender_sol` down by the first only — in the middle of the vest and
    /// again after it; in the end the sole lender has been paid every lamport of the lenders'
    /// part of the liquidation (rent and penalty) but rounding.
    #[test]
    fn repeated_claims_in_one_transaction_after_a_real_lump_pay_once() {
        let mut sc = Sc::default_market();
        sc.env.tick(&sc.mm, 5);
        let (b, n) = (sc.b.key(), sc.nonce);
        let t_open = sc.pos().t_open;
        rise_until_liquidatable(&mut sc, &b, n);
        let kp = sc.keeper.insecure_clone();
        let market = sc.mm.market();
        let sol0 = sc.env.market_vault(&market).lender_sol;
        let meta = ok(sc
            .env
            .send(&[liquidate_ix(&sc.mm, &kp.pubkey(), &b, n)], &[&kp]));
        let liq: Liquidate = events::one(&meta);
        let lenders = sc.env.market_vault(&market).lender_sol - sol0;
        assert!(liq.penalty_lender > 0 && lenders > liq.penalty_lender);
        let lender = sc.mm.lender.insecure_clone();
        let owner = lender.pubkey();
        let mut total = 0;
        for at in [vest_s(&sc.env) / 2, vest_s(&sc.env) + 1] {
            sc.env.warp(at - (sc.env.now() - liq.ts));
            sc.crank();
            let state = sc.env.svm.clone();
            let mut first = None;
            for n in [1usize, 2, 5] {
                sc.env.svm = state.clone();
                let before = sc.env.market_vault(&market).lender_sol;
                let ixs: Vec<_> = (0..n).map(|_| ix::claim_sol(&owner, &market)).collect();
                let meta = ok(sc.env.send(&ixs, &[&lender]));
                let paid: Vec<u64> = events::of::<ClaimSolEv>(&meta)
                    .iter()
                    .map(|e| e.sol_paid)
                    .collect();
                assert_eq!(paid.len(), n);
                let p0 = *first.get_or_insert(paid[0]);
                assert!(p0 > 0, "the first claim pays");
                assert_eq!(
                    paid[0], p0,
                    "n = {n}: the first claim pays what one claim pays"
                );
                assert!(
                    paid[1..].iter().all(|p| *p == 0),
                    "n = {n}: later claims pay 0: {paid:?}"
                );
                assert_eq!(before - sc.env.market_vault(&market).lender_sol, p0);
            }
            total += first.unwrap();
        }
        eprintln!(
            "lenders' part of the liquidation {lenders} (penalty {}), claimed {total}",
            liq.penalty_lender
        );
        // What is not paid stays in `lender_sol` as dust: the flooring of the rent credit at
        // each of the position's accruals (at most one a second).
        let dust = sc.env.market_vault(&market).lender_sol - sol0;
        let open_s = (liq.ts - t_open) as u64;
        assert!(
            total <= lenders && lenders - total == dust && dust <= open_s + 1,
            "the sole lender is paid the lenders' part but dust: {total} of {lenders} \
             ({open_s} s open)"
        );
        assert_zero_at_zero_positions(&sc.env, &market);
    }
}

mod wipes {
    use super::*;
    use shortwin::state::{MARKET_FLAG_WIPE_LIVE, ShareEpoch};
    use shortwin_math::pool::{R_MAX, VIRTUAL_ASSETS, VIRTUAL_SHARES};

    /// Withdraws every free token of `owner` (the only lender with free tokens).
    fn withdraw_all_free(sc: &mut Sc, owner: &Keypair, keys: &ix::LenderKeys) {
        let m = sc.market();
        let pool = m.pool();
        let want = m.tokens_free;
        let held = sc
            .env
            .lender(&sc.mm.market(), &owner.pubkey())
            .unwrap()
            .shares;
        let (mut lo, mut hi) = (0u128, held);
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if shortwin_math::pool::tokens_for_shares(&pool, mid).unwrap() <= u128::from(want) {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        ok(sc.env.send(&[ix::withdraw(keys, lo, want)], &[owner]));
        assert_eq!(sc.market().tokens_free, 0);
    }

    /// The settlement of the last borrowed tokens, which must wipe: S1 after a gap beyond the
    /// S1 boundary, or S2 after an LP drain and a 1.8× move (`settle_it::s2_is_capped_at_fair_
    /// value`'s model, the FV cap binding), whose lump the roll flushes into the wiped epoch's
    /// record. The settlement, the epoch it closed and the seconds the position was open.
    fn wipe(sc: &mut Sc, s2: bool) -> (SolSettleEv, u32, i64) {
        let epoch = sc.market().share_epoch;
        let t_open = sc.pos().t_open;
        let ev = if s2 {
            sc.drain(9_900);
            sc.env.tick(&sc.mm, 90);
            sc.move_by(1.8);
            let (_s, out) = sc.run_until_settled_out(120).expect("settled");
            let ev = out.settled.unwrap();
            assert_eq!(ev.condition, 2, "S2");
            assert!(ev.lump > 0);
            let rolled = out.rolled.expect("EpochRolled");
            assert_eq!((rolled.epoch, rolled.num, rolled.den), (epoch, 0, 1));
            assert_eq!(
                rolled.vest_flushed, ev.lump,
                "the S2 lump goes to the wiped lenders"
            );
            ev
        } else {
            sc.gap_and_arm(1.05);
            let (_s, out) = sc.run_until_settled_out(80).expect("settled");
            let ev = out.settled.unwrap();
            assert_eq!(ev.condition, 1, "S1");
            let rolled = out.rolled.expect("EpochRolled");
            assert_eq!((rolled.epoch, rolled.num, rolled.den), (epoch, 0, 1));
            ev
        };
        let m = sc.market();
        assert_eq!((m.share_epoch, m.total_shares.get()), (epoch + 1, 0));
        assert!(m.has_flag(MARKET_FLAG_WIPE_LIVE));
        let rec: ShareEpoch =
            read::borsh(&sc.env.svm, &pda::share_epoch(&sc.mm.market(), epoch).0).unwrap();
        assert_eq!((rec.epoch, rec.num), (epoch, 0));
        let open_s = ev.ts - t_open;
        (ev, epoch, open_s)
    }

    use shortwin::events::SolSettle as SolSettleEv;

    /// `[sweep_trader, deposit]` (the post-wipe sweep a depositor bundles, R4A-7) by a new
    /// lender of `amount`: it lands and mints at the initial rate (`amount·VS/VA`, as on an
    /// empty pool), with shares bounded (C0-6).
    fn deposit_after_wipe(sc: &mut Sc, amount: u64) -> (Actor, ix::LenderKeys) {
        let market = sc.mm.market();
        let l = sc.env.actor(&sc.mm, amount, 0);
        let keys = keys_of(&sc.mm, &l);
        let recs = ix::epoch_records(&market, 0, sc.market().share_epoch);
        let dep = ix::with_remaining(ix::deposit(&keys, amount, 0), &recs);
        let kp = l.kp.insecure_clone();
        assert_err(
            &sc.env.send(std::slice::from_ref(&dep), &[&kp]),
            shortwin::errors::ShortwinError::WipeSweepPending,
        );
        let sweep = sweep_trader_ix_for(&sc.env, &market);
        let meta = ok(sc.env.send(&[sweep, dep], &[&kp]));
        let ev: DepositEv = events::one(&meta);
        assert_eq!(
            ev.shares,
            u128::from(amount) * VIRTUAL_SHARES / VIRTUAL_ASSETS,
            "the initial rate"
        );
        let m = sc.market();
        assert!(!m.has_flag(MARKET_FLAG_WIPE_LIVE));
        let p = m.pool();
        assert!(
            p.total_shares + VIRTUAL_SHARES
                <= R_MAX * (u128::from(p.tokens_free) + u128::from(p.borrowed_q) + VIRTUAL_ASSETS),
            "shares bounded"
        );
        (l, keys)
    }

    /// G-F4 / C0-6 / V2-12: two consecutive real full wipes on one market, an S1 then an S2.
    /// Each settlement of the last borrowed tokens (every free token withdrawn) rolls the epoch
    /// into a live record (the S2's vesting lump flushed into it); the next lender's deposit is
    /// refused `WipeSweepPending` alone and, bundled with the post-wipe `sweep_trader`, mints at
    /// the initial rate both times; each wiped lender then claims its own settlement's SOL (but
    /// dust) through the records, the first one two epochs behind.
    #[test]
    fn two_consecutive_real_wipes_roll_two_epochs_and_mint_at_the_initial_rate() {
        let mut sc = Sc::default_market();
        let market = sc.mm.market();
        sc.env.tick(&sc.mm, 5);
        let l0 = sc.mm.lender.insecure_clone();
        let k0 = sc.mm.lender_keys();
        withdraw_all_free(&mut sc, &l0, &k0);
        let (s1, e1, open1) = wipe(&mut sc, false);
        assert_eq!(e1, 0);
        assert_zero_at_zero_positions(&sc.env, &market);

        let lend = sc.mm.spec.lend;
        let (l1, k1) = deposit_after_wipe(&mut sc, lend);
        sc.env.tick(&sc.mm, 5);
        let mut b = sc.env.borrower();
        // A quarter of the first size: the pool's token side is thinner after the gap (the
        // wallet cap on `X₀`).
        let (_, nonce, _) = open_ok(&mut sc.env, &sc.mm, &mut b, Q / 4);
        (sc.b, sc.nonce, sc.q) = (b, nonce, Q / 4);
        sc.env.tick(&sc.mm, 5);
        let l1kp = l1.kp.insecure_clone();
        withdraw_all_free(&mut sc, &l1kp, &k1);
        let (s2, e2, open2) = wipe(&mut sc, true);
        assert_eq!(e2, 1);
        assert_zero_at_zero_positions(&sc.env, &market);

        let (l2, _) = deposit_after_wipe(&mut sc, lend);
        assert_eq!(sc.market().share_epoch, 2);

        // Each wiped lender is paid its own settlement through the records (the sweeps moved
        // nothing), the first one two epochs behind.
        let paid0 = claim(&mut sc.env, &market, &l0);
        let paid1 = claim(&mut sc.env, &market, &l1kp);
        eprintln!(
            "wipe 1: S1 {} → lender 0 paid {paid0}; wipe 2: S2 {} → lender 1 paid {paid1}",
            s1.lender_amount, s2.lender_amount
        );
        // Each is paid its settlement but dust: the flooring of the rent credit at each of the
        // position's accruals (at most one a second; the take includes the rent) and of the
        // flush.
        for (paid, s, open_s) in [(paid0, &s1, open1), (paid1, &s2, open2)] {
            let dust = s.lender_amount - paid.min(s.lender_amount);
            assert!(
                paid <= s.lender_amount && dust <= open_s as u64 + 2,
                "paid {paid} of the settlement's {} ({open_s} s open)",
                s.lender_amount
            );
        }
        // The new epoch's lender owes and is owed nothing from the wipes.
        assert_eq!(claim(&mut sc.env, &market, &l2.kp), 0);
        assert_zero_at_zero_positions(&sc.env, &market);
    }
}

mod rent {
    use super::*;
    use shortwin::events::{Close, Liquidate};

    /// G-F13 / R3A-5 / A26: `rent_protocol_bps` goes from 15 % to 30 % through the timelock
    /// while two positions are open. Their snapshots (every term, `rent_protocol_bps` included)
    /// hold; a position opened after the change snapshots 30 % (the control: the change is
    /// live); the old positions' close and liquidation split rent at 15 %, the new one's close
    /// at 30 %, each lender credit exactly the remainder; with no position open `receivable`,
    /// `A_j` and `rent_aggr` are 0.
    #[test]
    fn rent_protocol_bps_changed_with_positions_open_keeps_each_snapshot() {
        let mut sc = Sc::default_market();
        let market = sc.mm.market();
        let b1 = std::mem::replace(&mut sc.b, sc.env.borrower());
        let n1 = sc.nonce;
        let (_, n2, _) = open_ok(&mut sc.env, &sc.mm, &mut sc.b, Q);
        let b2 = sc.b.key();
        sc.env.tick(&sc.mm, 5);
        let p1 = sc.env.position(&sc.mm, &b1.key(), n1);
        let p2 = sc.env.position(&sc.mm, &b2, n2);
        assert_eq!((p1.rent_protocol_bps, p2.rent_protocol_bps), (1_500, 1_500));

        let gov = sc.env.governance.insecure_clone();
        let mut params = sc.env.config().params;
        params.rent_protocol_bps = 3_000;
        let tiers = sc.env.config().tiers;
        ok(sc.env.send(
            &[ix::propose_config(
                &gov.pubkey(),
                shortwin::state::ConfigChange::Params { params, tiers },
            )],
            &[&gov],
        ));
        let timelock = i64::from(sc.env.config().timelock_s);
        sc.env.warp(timelock);
        sc.crank();
        let apply = sc.env.apply_config_ix();
        ok(sc.env.send(&[apply], &[]));
        assert_eq!(sc.env.config().params.rent_protocol_bps, 3_000);
        assert_eq!(
            sc.env.position(&sc.mm, &b1.key(), n1),
            p1,
            "A26: the snapshot holds"
        );
        assert_eq!(
            sc.env.position(&sc.mm, &b2, n2),
            p2,
            "A26: the snapshot holds"
        );

        // A position opened now snapshots 30 %.
        let mut b3 = sc.env.borrower();
        let (_, n3, _) = open_ok(&mut sc.env, &sc.mm, &mut b3, Q);
        assert_eq!(
            sc.env.position(&sc.mm, &b3.key(), n3).rent_protocol_bps,
            3_000
        );
        sc.env.warp(3_600);
        sc.crank();

        let close = |sc: &mut Sc, b: &Borrower, n: u64, bps: u64| {
            let sol0 = sc.env.market_vault(&market).lender_sol;
            let (ev, _) = close_ok(&mut sc.env, &sc.mm, b, n);
            let ev: Close = ev;
            assert!(ev.rent > 0);
            assert_eq!(
                ev.rent_protocol,
                ev.rent * bps / 10_000,
                "{bps} bps of rent"
            );
            assert_eq!(
                sc.env.market_vault(&market).lender_sol - sol0,
                ev.rent - ev.rent_protocol,
                "the lenders' rent"
            );
            eprintln!(
                "close at {bps} bps: rent {} protocol {}",
                ev.rent, ev.rent_protocol
            );
        };
        close(&mut sc, &b3, n3, 3_000);
        close(&mut sc, &b1, n1, 1_500);

        rise_until_liquidatable(&mut sc, &b2, n2);
        let kp = sc.keeper.insecure_clone();
        let v0 = sc.env.market_vault(&market);
        let meta = ok(sc
            .env
            .send(&[liquidate_ix(&sc.mm, &kp.pubkey(), &b2, n2)], &[&kp]));
        let liq: Liquidate = events::one(&meta);
        let v1 = sc.env.market_vault(&market);
        let rent_protocol = liq.rent * 1_500 / 10_000;
        assert!(liq.rent > 0);
        assert_eq!(
            v1.protocol_fees - v0.protocol_fees,
            rent_protocol + liq.penalty_protocol,
            "the liquidation's rent split at the position's 15 %"
        );
        assert_eq!(
            v1.lender_sol - v0.lender_sol,
            liq.rent - rent_protocol + liq.penalty_lender
        );
        assert_ne!(rent_protocol, liq.rent * 3_000 / 10_000);
        assert_zero_at_zero_positions(&sc.env, &market);
    }
}

mod pswap {
    use crate::pswap_common::*;
    use crate::{assert_zero_at_zero_positions, claim, sol_paid, tokens_withdrawn};
    use shortwin::constants::{TOKEN_PROGRAM_ID, WSOL_MINT};
    use shortwin::events::{Deposit as DepositEv, SWEEP_TO_VEST, TraderSwept};
    use shortwin_litesvm_tests::ix;

    /// G-F2 on PumpSwap: the cashback lump. Short.win's trades on the dumped cashback coin
    /// accrue WSOL cashback in `ATA(pAMM UVA(trader), WSOL)`; a whale's single transaction
    /// `[deposit 9×, create_idempotent ATA(trader, WSOL), pAMM claim_cashback(user = trader),
    /// sweep_trader, claim_sol, withdraw]` (the as-built `claim_venue_rewards`, ARCH §3.9) makes
    /// the cashback a vesting lump (`TraderSwept`, to the vest) and realises 0; the honest
    /// lender's take is the run's without the whale; a whale holding through `lump_vest_s` takes
    /// its share (the control).
    #[test]
    fn a_jit_whale_around_the_pumpswap_cashback_lump_realises_nothing() {
        let (mut env, pm, q) = setup(PammCoin::Cashback);
        let market = pm.market();
        let mut b = env.borrower();
        let (_, n0, _) = open_ok(&mut env, &pm, &mut b, q);
        env.warp(1);
        crank(&mut env, &pm);
        close_ok(&mut env, &pm, &b, n0);
        let cashback = tokens(&env, &pm.cashback_ata());
        assert!(cashback > 0, "the trader's trades accrued cashback");
        assert_eq!(env.market(&market).vest_bal, 0);

        let lend = env.lender(&market, &pm.lender.pubkey()).unwrap();
        let amount = 9 * u64::try_from(
            shortwin_math::pool::tokens_for_shares(&env.market(&market).pool(), lend.shares)
                .unwrap(),
        )
        .unwrap();
        let whale = env.funded_keypair();
        let acc = fork::fund_token(
            &mut env,
            &whale.pubkey(),
            &pm.keys.mint,
            pm.pool.token_program(),
            amount,
        );
        let keys = ix::LenderKeys {
            owner: whale.pubkey(),
            mint: pm.keys.mint,
            token_program: pm.keys.token_program,
            owner_token_account: acc,
        };
        let trader = pm.trader();
        let payer = env.payer.pubkey();
        let lump_ixs = |env: &Env| {
            vec![
                fork::create_ata_idempotent_ix(&payer, &trader, &WSOL_MINT, &TOKEN_PROGRAM_ID),
                pamm::pamm_claim_cashback_ix(&trader, &wsol_ata(&trader)),
                pamm::sweep_trader_pamm_ix(env, &pm),
            ]
        };
        let ata_rent = env.svm.minimum_balance_for_rent_exemption(165);
        let vest = i64::from(env.config().params.lump_vest_s);
        let t0 = env.now();
        let honest = pm.lender.insecure_clone();
        let saved = env.svm.clone();

        // Without the whale.
        let meta = ok(env.send(&lump_ixs(&env), &[]));
        let ev: TraderSwept = events::one(&meta);
        assert_eq!((ev.lamports, ev.to), (cashback + ata_rent, SWEEP_TO_VEST));
        env.warp(vest + 1);
        crank(&mut env, &pm);
        let alone = claim(&mut env, &market, &honest);
        assert!(alone >= ev.lamports, "the honest lender is paid the lump");

        for hold in [0, vest] {
            env.svm = saved.clone();
            let minted =
                shortwin_math::pool::shares_for_tokens(&env.market(&market).pool(), amount)
                    .unwrap();
            let mut ixs = vec![ix::deposit(&keys, amount, minted)];
            ixs.extend(lump_ixs(&env));
            if hold == 0 {
                ixs.push(ix::claim_sol(&whale.pubkey(), &market));
                ixs.push(ix::withdraw(&keys, minted, 0));
            }
            let meta = ok(env.send(&ixs, &[&whale]));
            let ev: TraderSwept = events::one(&meta);
            assert_eq!((ev.lamports, ev.to), (cashback + ata_rent, SWEEP_TO_VEST));
            let dep: DepositEv = events::one(&meta);
            let mut w = sol_paid(&meta, &whale.pubkey());
            let mut back = tokens_withdrawn(&meta, &whale.pubkey());
            if hold > 0 {
                env.warp(hold);
                let meta = ok(env.send(
                    &[
                        ix::claim_sol(&whale.pubkey(), &market),
                        ix::withdraw(&keys, minted, 0),
                    ],
                    &[&whale],
                ));
                w += sol_paid(&meta, &whale.pubkey());
                back += tokens_withdrawn(&meta, &whale.pubkey());
            }
            eprintln!("PumpSwap, h = {hold}: the whale's tokens back {back} of {amount}");
            assert!(
                back <= amount && amount - back <= 2,
                "h = {hold}: the whale's tokens back {back} of {amount}"
            );
            env.warp(t0 + vest + 1 - env.now());
            crank(&mut env, &pm);
            w += claim(&mut env, &market, &whale);
            let h = claim(&mut env, &market, &honest);
            let share = u128::from(ev.lamports) * minted
                / (dep.s_after + shortwin_math::pool::VIRTUAL_SHARES);
            eprintln!(
                "PumpSwap cashback lump {} hold {hold} s: whale {w} (share {share}), honest {h} \
                 (alone {alone})",
                ev.lamports
            );
            if hold == 0 {
                assert_eq!(w, 0, "the JIT whale realises 0 at h = 0");
                assert!(h.abs_diff(alone) <= 2, "honest {h} vs alone {alone}");
            } else {
                assert!(
                    u128::from(w).abs_diff(share) <= 2 + share / 1_000_000,
                    "held through the vest: its share {share}, took {w}"
                );
                assert!((w + h).abs_diff(alone) <= 2, "{w} + {h} vs {alone}");
            }
            assert!(env.lender(&market, &whale.pubkey()).is_none());
            assert_zero_at_zero_positions(&env, &market);
        }
    }
}
