//! ECON §12's attack patterns that apply on the bonding curve, reproduced on the fork against the
//! release short.win and the dumped pump ELFs (ARCH §8.2 "adversarial suite … on every real venue
//! as it lands"; M5): the attacker trades the real curve (`buy_v2` / `sell_v2`) and lands its
//! bundles as single transactions, the invariant hooks check every instruction of every bundle.
//!
//! | ECON | Test |
//! |---|---|
//! | §12.2 `sandwich.py`, expiry, atomic: D@50% honest `0.707 / 0.0226 / 0.0141 / 2.168`, attacked `0.713 / … / 2.161`, attacker `+0.003`, borrower `−0.006`, lenders `+0.0000` | [`sandwich_expiry_d50`] |
//! | §12.2 the same, D@90%: `1.331 → 1.343`, attacker `+0.004`, borrower `−0.012` | [`sandwich_expiry_d90`] |
//! | §12.2 `sandwich.py` / `fixed_delta.py` (d), liquidation, atomic, keeper at once and after 4 s: D@50% `1.678 → 1.709` (`+0.064`, extra `+0.014`, borrower `−0.031`) and `1.711` after 4 s (`+0.065`, `+0.015`, `−0.033`) | [`sandwich_liquidation_d50`] |
//! | §12.4 `ema_atomic.py`, atomic `[pump, crank, dump]` every second against a fresh position: "not liquidatable within 60 s at any push level" (D@50%) | [`atomic_pushes_never_liquidate_a_fresh_position`] |
//! | §12.4 the held model (every observation of the second pumped): D@50% `x3, 11 s`, push cost `9.439`, final sandwich `+0.655`, attacker net `−8.785` | [`held_pushes_liquidate_a_fresh_position_at_a_loss`] |
//! | §12.5 `ema_atomic_multi.py` (a), atomic, the OI-cap basket: "no exit forced within 120 s at any push level" (D@50%) | [`atomic_pushes_force_no_exit_on_the_oi_basket`] |
//!
//! The sandwich and held-push figures are the scripts' own (`sandwich.setup_expiry`,
//! `sandwich.setup_liq`, `sandwich.attack(…, "atomic", wait)`, `common.sandwich_exit`,
//! `ema_atomic.attack("D@50%", model)`, `common.push_second`) on ECON's `D@50%` / `D@90%`
//! markets, replayed step for step (the same cranks, the same exact trade sizes, the scripts'
//! optimal front-run `dX`); the liquidation legs below the rent move by the documented `ΔR`
//! (`pump_curve_support.rs`: `R_min` at 80 % in the sim, the pre-open 0 % in the program), which
//! the attacker's P&L does not contain. The expiry rows' rent is the accrued one: exact.
//!
//! Not on the curve in M5: §12.2's D@90% liquidation rows (the +0.1 %/s ramp completes the curve
//! at 1.65× before the position is liquidatable; the sim then liquidates on the migrated pool,
//! M6) and every D@0% row (no short can open on a curve without real SOL,
//! `pump_curve_scenarios.rs`).

#![allow(clippy::result_large_err)]

#[path = "pump_curve_support.rs"]
mod support;

use shortwin_litesvm_tests::fork::PumpV2;
use shortwin_math::num::{Rounding, mul_div};
use shortwin_math::{quote, risk};
use support::*;

/// ECON's `D@50%` position (`max_open_q()`), its `V` and the sim's `R_min`.
const D50_Q: u64 = 10_071_769_920_663;
const D50_V: u64 = 2_912_898_540;
const D50_RMIN: u64 = 1_294_111;
/// ECON's `D@90%` position and `V`.
const D90_Q: u64 = 5_348_333_909_677;
const D90_V: u64 = 5_485_454_676;

/// The push levels of `ema_atomic.py` / `ema_atomic_multi.py`.
const MULTS: [f64; 6] = [1.25, 1.5, 1.75, 2.0, 2.5, 3.0];

/// A CU limit that covers a bundle `[buy, exit, sell]` on the curve.
const BUNDLE_CU: u32 = 1_000_000;

/// An attacker: a curve user (base ATA and pump UVA created) that is also the keeper of the
/// exits it sandwiches.
struct Attacker {
    user: CurveUser,
}

impl Attacker {
    fn new(s: &mut Sim) -> Self {
        let user = CurveUser::new(&mut s.env, &s.pm.curve);
        Self { user }
    }

    fn key(&self) -> Pubkey {
        self.user.kp.pubkey()
    }

    fn kp(&self) -> Keypair {
        self.user.kp.insecure_clone()
    }

    fn trade_ix(&self, s: &Sim, kind: PumpV2, q: u64, bound: u64) -> Instruction {
        let fr = fork::pump_fee_recipient(&s.env.svm, 2);
        let bb = fork::pump_buyback_fee_recipient(&s.env.svm, 2);
        s.pm.curve.v2_ix(
            &s.env.svm,
            kind,
            &self.key(),
            &self.user.ata,
            &fr,
            &bb,
            q,
            bound,
        )
    }

    /// `buy_v2(q, u64::MAX)`.
    fn buy_ix(&self, s: &Sim, q: u64) -> Instruction {
        self.trade_ix(s, PumpV2::Buy, q, u64::MAX)
    }

    /// `sell_v2(q, 0)`.
    fn sell_ix(&self, s: &Sim, q: u64) -> Instruction {
        self.trade_ix(s, PumpV2::Sell, q, 0)
    }

    fn lamports(&self, s: &Sim) -> i128 {
        i128::from(lamports(&s.env, &self.key()))
    }
}

/// `move_size(v, target)` of `common.py` on the curve: `move_to_price`'s size, a buy capped at
/// `rT − 1`.
fn move_size(s: &Sim, target: u128) -> i128 {
    let d = s.move_size(target);
    let rt = i128::try_from(curve(&s.env, &s.pm).rt).unwrap();
    if d > 0 { d.min(rt - 1) } else { d }
}

/// The attacker's sandwich `[buy dX, exit, sell dX]` in one transaction: the exit's event
/// payload and the attacker's P&L (its lamports; the tokens round-trip), which includes the
/// keeper's pay.
fn sandwich(
    s: &mut Sim,
    a: &Attacker,
    p: &Pos,
    dx: u64,
    expiry: bool,
) -> (TransactionMetadata, i128) {
    let kp = a.kp();
    let exit = if expiry {
        expire_ix(&s.pm, &a.key(), &p.key(), p.nonce)
    } else {
        liquidate_ix(&s.pm, &a.key(), &p.key(), p.nonce)
    };
    let ixs = [
        pump::cu_limit_ix(BUNDLE_CU),
        a.buy_ix(s, dx),
        exit,
        a.sell_ix(s, dx),
    ];
    let l0 = a.lamports(s);
    let meta = ok(s.env.send(&ixs, &[&kp]));
    (meta, a.lamports(s) - l0)
}

/// The honest keeper's exit at this second.
fn honest(s: &mut Sim, p: &Pos, expiry: bool) -> TransactionMetadata {
    let kp = s.keeper.insecure_clone();
    let exit = if expiry {
        expire_ix(&s.pm, &kp.pubkey(), &p.key(), p.nonce)
    } else {
        liquidate_ix(&s.pm, &kp.pubkey(), &p.key(), p.nonce)
    };
    let cu = pump::cu_limit_ix(pump::CURVE_KEEPER_EXIT_CU);
    ok(s.env.send(&[cu, exit], &[&kp]))
}

// ---- §12.2: the exit sandwich --------------------------------------------------------------

/// `sandwich.setup_expiry(key)`: the position at the cap, a crank every 30 s until 120 s before
/// the deadline, then every second until it; returns the sim second (the deadline).
fn setup_expiry(sold: u64, q: u64) -> (Sim, Pos, i64) {
    let mut s = Sim::d_market(sold, lend_for_80(q));
    let p = s.open(q);
    let deadline = s.position(&p).deadline - s.t0;
    let mut t = 0;
    while t < deadline - 120 {
        t = (t + 30).min(deadline - 120);
        s.step_to(t);
    }
    while t < deadline {
        t += 1;
        s.step_to(t);
    }
    (s, p, t)
}

/// An expiry as the sim computed it.
struct SimExp {
    b: u64,
    rent: u64,
    fee: u64,
    keeper: u64,
    borrower: u64,
    b_ref: u64,
    max_in: u64,
}

fn assert_exp(ev: &Expire, sim: &SimExp) {
    assert_eq!(
        (
            ev.buyback_cost,
            ev.rent,
            ev.expiry_fee,
            ev.keeper_reward,
            ev.borrower_payout
        ),
        (sim.b, sim.rent, sim.fee, sim.keeper, sim.borrower),
        "B, rent, expiry fee, K_e, borrower: the sim's"
    );
    assert_eq!(
        (ev.b_ref, ev.max_in),
        (sim.b_ref, sim.max_in),
        "B̂_ref, max_in"
    );
}

/// `sandwich.py`, expiry, model atomic, `D@50%` (ECON §12.2): the honest keeper's expiry at the
/// deadline and the attacker-keeper's best sandwich (`dX` = the script's grid optimum), to the
/// lamport. The expiry bound (`B̂_ref·(1 + δ_exp)` with rent and the expiry fee senior) lets the
/// attacker raise the buyback by 0.9 % of `B̂_ref`: attacker `+0.0027` SOL including `K_e`
/// (`+0.001` over an honest keeper), borrower `−0.0065`, and the rent and expiry fee — the
/// lenders' and the protocol's legs — are paid in full.
#[test]
fn sandwich_expiry_d50() {
    let (mut s, p, t) = setup_expiry(SOLD_D50, D50_Q);
    assert_eq!(t, 259_200);
    let saved = s.env.svm.clone();
    let meta = honest(&mut s, &p, true);
    let h: Expire = events::one(&meta);
    let honest_exp = SimExp {
        b: 706_857_371,
        rent: 22_615_871,
        fee: 14_137_148,
        keeper: 1_706_857,
        borrower: 2_167_581_293,
        b_ref: 706_857_386,
        max_in: 713_925_959,
    };
    assert_exp(&h, &honest_exp);
    s.env.svm = saved;
    let a = Attacker::new(&mut s);
    let (meta, pnl) = sandwich(&mut s, &a, &p, 3_089_347_964_642, true);
    let ev: Expire = events::one(&meta);
    assert_exp(
        &ev,
        &SimExp {
            b: 713_310_296,
            borrower: 2_161_128_368,
            ..honest_exp
        },
    );
    assert_eq!(pnl, 2_701_644, "the attacker's net incl. K_e (ECON +0.003)");
    assert_eq!(sol4(pnl - i128::from(h.keeper_reward)), "+0.0010");
    assert_eq!(
        sol4(i128::from(h.borrower_payout - ev.borrower_payout)),
        "+0.0065",
        "the borrower's loss (ECON 0.006)"
    );
    assert_eq!(
        (ev.rent, ev.expiry_fee),
        (h.rent, h.expiry_fee),
        "lenders and protocol unharmed (+0.0000)"
    );
    assert!(ev.buyback_cost <= ev.max_in);
}

/// `sandwich.py`, expiry, model atomic, `D@90%` (ECON §12.2): honest `1.331 / 0.0426 / 0.0266 /
/// 4.083`, attacked `1.343 / … / 4.071`, attacker `+0.004`, borrower `−0.012`, lenders `−0.0000`
/// (the fee base rises with the reference, by 3 lamports).
#[test]
fn sandwich_expiry_d90() {
    let (mut s, p, t) = setup_expiry(SOLD_D90, D90_Q);
    assert_eq!(t, 259_200);
    assert_eq!(s.position(&p).v, D90_V);
    let saved = s.env.svm.clone();
    let meta = honest(&mut s, &p, true);
    let h: Expire = events::one(&meta);
    let honest_exp = SimExp {
        b: 1_331_125_715,
        rent: 42_589_310,
        fee: 26_622_515,
        keeper: 2_331_125,
        borrower: 4_082_786_011,
        b_ref: 1_331_125_885,
        max_in: 1_344_437_143,
    };
    assert_exp(&h, &honest_exp);
    s.env.svm = saved;
    let a = Attacker::new(&mut s);
    let (meta, pnl) = sandwich(&mut s, &a, &p, 1_640_512_502_593, true);
    let ev: Expire = events::one(&meta);
    assert_exp(
        &ev,
        &SimExp {
            b: 1_343_277_608,
            fee: 26_622_518,
            borrower: 4_070_634_115,
            ..honest_exp
        },
    );
    assert_eq!(pnl, 4_204_470, "the attacker's net incl. K_e (ECON +0.004)");
    assert_eq!(
        sol4(i128::from(h.borrower_payout - ev.borrower_payout)),
        "+0.0122",
        "the borrower's loss (ECON 0.012)"
    );
}

/// `sandwich.setup_liq(key)`: the position at the cap, a crank every second for 120 s, then an
/// honest +0.1 %/s rise (other users' real buys) with a crank every second until the first
/// second the position is CR-liquidatable on both legs.
fn setup_liq() -> (Sim, Pos, i64) {
    let mut s = Sim::d_market(SOLD_D50, lend_for_80(D50_Q));
    let p = s.open(D50_Q);
    let spot_post = s.spot();
    for t in 1..=120 {
        s.step_to(t);
    }
    let mut t = 120;
    let mut k = 1.0f64;
    while k < 5.0 {
        k *= 1.001;
        t += 1;
        s.at(t);
        assert!(s.exo_move((spot_post as f64 * k) as u128).is_none());
        s.crank();
        if s.status(&p).cr_liq() {
            return (s, p, t);
        }
    }
    panic!("never liquidatable");
}

/// A liquidation as the sim computed it (`rent` the sim's `R_min`).
struct SimLiq {
    b: u64,
    pen: u64,
    keeper: u64,
    borrower: u64,
    b_ref: u64,
    max_in: u64,
}

/// `B`, `B̂_ref`, `max_in`, the penalty and the keeper's share exactly; the borrower's remainder
/// moves by `ΔR` (module docs). Returns `ΔR`.
fn assert_liq(ev: &Liquidate, sim: &SimLiq, v: u64) -> u64 {
    assert_eq!(ev.buyback_cost, sim.b, "B");
    assert_eq!(
        (ev.b_ref, ev.max_in),
        (sim.b_ref, sim.max_in),
        "B̂_ref, max_in"
    );
    assert_eq!(ev.penalty, sim.pen, "penalty");
    assert_eq!(ev.penalty_keeper + ev.keeper_topup, sim.keeper, "keeper");
    assert!(ev.rent < D50_RMIN);
    let d = D50_RMIN - ev.rent;
    assert_eq!(
        ev.borrower_payout,
        sim.borrower + d,
        "borrower (rent adjusted)"
    );
    assert_eq!(
        ev.buyback_cost + ev.rent + ev.penalty + ev.keeper_topup + ev.borrower_payout,
        v
    );
    d
}

/// `sandwich.py` liquidation rows and `fixed_delta.py` (d), `D@50%`, model atomic: the honest
/// keeper at the first liquidatable second, and the attacker-keeper's best sandwich at once and
/// after waiting 4 s with nobody exiting. The fixed 2 % bound over the reference caps the
/// front-run: the extraction over an honest keeper is `+0.014` SOL at once and `+0.015` after
/// 4 s (ECON; v0.3's Dutch clock widened with waiting), the borrower loses `0.031` / `0.033`,
/// and the penalty — the lenders' and protocol's legs — is the honest one (`+0.0000`; after 4 s
/// the reference moved up, so the penalty slightly more).
#[test]
fn sandwich_liquidation_d50() {
    let (mut s, p, t) = setup_liq();
    assert_eq!(t, 977, "the sim's first liquidatable second (k = 2.355)");
    let saved = s.env.svm.clone();
    let meta = honest(&mut s, &p, false);
    let h: Liquidate = events::one(&meta);
    let honest_liq = SimLiq {
        b: 1_678_058_339,
        pen: 167_704_608,
        keeper: 50_311_382,
        borrower: 1_065_841_482,
        b_ref: 1_677_046_080,
        max_in: 1_710_587_001,
    };
    let d = assert_liq(&h, &honest_liq, D50_V);

    // The attacker-keeper at t: the script's grid optimum dX.
    s.env.svm = saved.clone();
    let a = Attacker::new(&mut s);
    let (meta, pnl) = sandwich(&mut s, &a, &p, 4_026_193_326_522, false);
    let ev: Liquidate = events::one(&meta);
    assert_eq!(
        assert_liq(
            &ev,
            &SimLiq {
                b: 1_709_033_596,
                borrower: 1_034_866_225,
                ..honest_liq
            },
            D50_V
        ),
        d
    );
    assert_eq!(
        pnl, 64_184_611,
        "attacker net incl. the keeper share (ECON +0.064)"
    );
    let extra = pnl - i128::from(h.penalty_keeper);
    assert_eq!(
        sol4(extra),
        "+0.0139",
        "extra over an honest keeper (ECON +0.014)"
    );
    assert_eq!(
        sol4(i128::from(h.borrower_payout - ev.borrower_payout)),
        "+0.0310",
        "borrower loss (ECON 0.031)"
    );
    assert_eq!(
        (ev.penalty_lender, ev.penalty_protocol, ev.rent),
        (h.penalty_lender, h.penalty_protocol, h.rent),
        "lenders + protocol: +0.0000"
    );
    assert!(u128::from(ev.buyback_cost) * 10_000 <= u128::from(ev.b_ref) * 10_200);

    // After 4 s of waiting (cranks only, nobody exits): the same fixed δ.
    s.env.svm = saved;
    for tt in t + 1..=t + 4 {
        s.step_to(tt);
    }
    let a = Attacker::new(&mut s);
    let (meta, pnl4) = sandwich(&mut s, &a, &p, 4_249_870_733_551, false);
    let ev4: Liquidate = events::one(&meta);
    assert_liq(
        &ev4,
        &SimLiq {
            b: 1_710_779_532,
            pen: 167_825_393,
            keeper: 50_347_617,
            borrower: 1_032_999_504,
            b_ref: 1_678_253_929,
            max_in: 1_711_819_007,
        },
        D50_V,
    );
    assert_eq!(pnl4, 65_007_345, "ECON +0.065 after 4 s");
    assert_eq!(ev4.delta_bps, 200, "the fixed δ");
    let extra4 = pnl4 - i128::from(h.penalty_keeper);
    assert_eq!(sol4(extra4), "+0.0147", "ECON +0.015 after 4 s");
    assert!(
        extra4 * 100 <= extra * 110,
        "waiting does not widen the bound"
    );
}

// ---- §12.4 / §12.5: EMA pushes -------------------------------------------------------------

/// `common.would_liq`: the attacker's test that its own final transaction could liquidate — the
/// EMA leg on the ring now, a fresh bucket, and `CR < L` at the last bucket's low repriced on
/// the live `√(vT·vQ)` (the attacker pumps the live spot above that low inside the transaction).
fn would_liq(s: &Sim, p: &Pos) -> bool {
    let st = s.status(p);
    if !st.ema_leg || !st.fresh {
        return false;
    }
    let pos = s.position(p);
    let m = market(&s.env, &s.pm);
    let lo = m.observation().last.lo;
    let c = curve(&s.env, &s.pm);
    let l_live = quote::liquidity(c.vt, c.vq).unwrap();
    let ctx = CurveCtx::read(&s.env, &s.pm).unwrap();
    let b = ctx
        .buy_repriced(pos.q, l_live, lo, u64::from(pos.venue_fee_hard_cap_bps))
        .map(|b| b.cost);
    risk::cr_below(st.v_eff, b, pos.cr_liq_bps).unwrap()
}

/// One attacker second of `push_second(m, t, target, model)`: atomic — the honest keeper cranks
/// at the true price, then the attacker's `[buy d, crank_price, sell d]`; held — the attacker's
/// bundle is the second's only observation (every observation of the second pumped). Returns
/// the attacker's cost (lamports).
fn push_second(s: &mut Sim, a: &Attacker, t: i64, target: u128, held: bool) -> i128 {
    s.at(t);
    if !held {
        s.crank();
    }
    let d = move_size(s, target);
    if d <= 0 {
        if held {
            s.crank();
        }
        return 0;
    }
    let d = u64::try_from(d).unwrap();
    let kp = a.kp();
    let ixs = [
        pump::cu_limit_ix(BUNDLE_CU),
        a.buy_ix(s, d),
        crank_price_ix(&s.pm),
        a.sell_ix(s, d),
    ];
    let l0 = a.lamports(s);
    ok(s.env.send(&ixs, &[&kp]));
    l0 - a.lamports(s)
}

/// `ema_atomic.attack(key)`'s start: the position at the cap, a crank every second for 60 s;
/// returns the true price.
fn fresh_position() -> (Sim, Pos, u128) {
    let mut s = Sim::d_market(SOLD_D50, lend_for_80(D50_Q));
    let p = s.open(D50_Q);
    for t in 1..=60 {
        s.step_to(t);
    }
    let p_true = s.spot();
    assert_eq!(p_true, 68_298_686_219_984);
    (s, p, p_true)
}

/// `ema_atomic.py`, model atomic, `D@50%` (ECON §12.4: "not liquidatable within 60 s at any push
/// level"): at every push level the attacker's `[pump, crank, dump]` lands every second next to
/// the honest keeper's crank; the bucket straddles the EMA, which stays at the true price, so
/// the EMA leg never holds — `would_liq` is false for 60 s, the keeper's view never
/// CR-liquidatable, and the attacker's own final bundle `[pump, liquidate, dump]` is refused
/// `NotLiquidatable` every second.
#[test]
fn atomic_pushes_never_liquidate_a_fresh_position() {
    let (mut s, p, p_true) = fresh_position();
    let a = Attacker::new(&mut s);
    let saved = s.env.svm.clone();
    let t0 = s.t();
    for mult in MULTS {
        s.env.svm = saved.clone();
        let target = (p_true as f64 * mult) as u128;
        let mut cost = 0;
        for n in 1..=60 {
            let t = t0 + n;
            cost += push_second(&mut s, &a, t, target, false);
            assert!(!would_liq(&s, &p), "mult {mult}: liquidatable at +{n} s");
            assert!(!s.status(&p).cr_liq());
            let d = u64::try_from(move_size(&s, target)).unwrap();
            let ixs = [
                pump::cu_limit_ix(BUNDLE_CU),
                a.buy_ix(&s, d),
                liquidate_ix(&s.pm, &a.key(), &p.key(), p.nonce),
                a.sell_ix(&s, d),
            ];
            assert_err(&s.env.send(&ixs, &[&a.kp()]), E::NotLiquidatable);
        }
        let m = market(&s.env, &s.pm);
        eprintln!(
            "atomic ×{mult}: 60 s cost {} SOL, P_ema/P_true {:.4}",
            sol4(cost),
            m.ema_price.get() as f64 / p_true as f64
        );
        assert!(
            m.ema_price.get() * 1_000 <= p_true * 1_001,
            "the EMA stays at the true price"
        );
    }
    assert_eq!(market(&s.env, &s.pm).open_positions, 1);
}

/// `ema_atomic.py`, model held, `D@50%` (ECON §12.4: `x3, 11 s | 9.439 | +0.655 | -8.785`): with
/// every observation of each second pumped to 3× (a degraded-mode market: no honest observer),
/// the EMA leg holds after 11 s; the attacker then sandwiches the liquidation in the next
/// second with the script's optimal `dX`. The push costs 9.439 SOL of round-trip fees (the
/// sim's, to the lamport), the final bundle earns 0.655 SOL including the keeper share: net
/// −8.785 SOL. The liquidation's legs are the sim's (rent adjusted).
#[test]
fn held_pushes_liquidate_a_fresh_position_at_a_loss() {
    let (mut s, p, p_true) = fresh_position();
    let a = Attacker::new(&mut s);
    let target = (p_true as f64 * 3.0) as u128;
    let t0 = s.t();
    let mut cost = 0;
    let mut n = 0;
    loop {
        n += 1;
        assert!(n <= 60, "never liquidatable");
        let d = move_size(&s, target);
        cost += push_second(&mut s, &a, t0 + n, target, true);
        if n == 1 {
            assert_eq!(d, 290_158_241_252_428, "the sim's first held push size");
        }
        if would_liq(&s, &p) {
            break;
        }
    }
    assert_eq!(n, 11, "11 held seconds (ECON)");
    assert_eq!(
        cost, 9_439_317_239,
        "push cost: 11 round trips of 0.858 SOL (ECON 9.439)"
    );
    s.at(t0 + n + 1);
    let (meta, pnl) = sandwich(&mut s, &a, &p, 297_950_448_145_567, false);
    let ev: Liquidate = events::one(&meta);
    let d = D50_RMIN - ev.rent;
    assert_eq!(
        (
            ev.buyback_cost,
            ev.b_ref,
            ev.max_in,
            ev.penalty,
            ev.penalty_keeper
        ),
        (
            2_231_956_319,
            2_198_650_783,
            2_242_623_798,
            219_865_079,
            65_959_523
        )
    );
    assert_eq!(
        ev.borrower_payout,
        459_783_031 + d,
        "borrower (rent adjusted)"
    );
    assert_eq!(pnl, 654_566_714, "final sandwich + keeper (ECON +0.655)");
    assert_eq!(sol4(pnl - cost), "-8.7848", "attacker net (ECON -8.785)");
}

/// `ema_atomic_multi.py` (a), model atomic, `D@50%` (ECON §12.5: "no exit forced within 120 s at
/// any push level"): the market filled to its OI cap with positions at the per-position cap
/// (every further open refused), then at every push level the attacker's `[pump, crank, dump]`
/// every second next to the honest keeper's crank for 120 s: no position ever becomes
/// CR-liquidatable (the keeper's view of every basket position, every second), and the
/// attacker's final bundle `[pump, liquidate, dump]` is refused `NotLiquidatable` every second.
///
/// Since M5-SEC-3 a curve open is sized on `X₀ = min(vT, vT at P_ema)` (the EMA lags each
/// basket open's own sale, so the next open one second later reads the pre-sale curve): the fill
/// sizes every position on that reserve — `engine::caps::curve_reserves` at the `P_ema` the open
/// reads (a crank in the same second rolls the bucket first) — instead of the live `vT`.
#[test]
fn atomic_pushes_force_no_exit_on_the_oi_basket() {
    // ECON's fill: positions at min(per-position cap, the OI cap left) until the next is refused.
    let lend = lend_for_80(4 * D50_Q);
    let mut s = Sim::d_market(SOLD_D50, lend);
    let mut basket = Vec::new();
    loop {
        s.crank();
        let c = curve(&s.env, &s.pm);
        let (x0, _) = shortwin::engine::caps::curve_reserves(
            &shortwin::engine::caps::CurveCap {
                vt: u64::try_from(c.vt).unwrap(),
                vq: u64::try_from(c.vq).unwrap(),
                rt: u64::try_from(c.rt).unwrap(),
                p_spot: s.spot(),
            },
            market(&s.env, &s.pm).ema_price.get(),
        )
        .unwrap();
        let cap_pos = shortwin_math::caps::max_q_position(x0, 300).unwrap();
        let sum: u64 = basket.iter().map(|p: &Pos| p.open.q).sum();
        // The largest q whose ΣQ + q fits the per-market cap on X₀ + q (bisection).
        let (mut lo, mut hi) = (0u64, cap_pos);
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            let oi = shortwin_math::caps::max_total_q(x0 + mid, 1_000).unwrap();
            if sum + mid <= oi {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let q = lo;
        let notional = mul_div(u128::from(q), s.spot(), W, Rounding::Floor).unwrap();
        if q <= q / 1_000 + 1 || notional < 100_000_000 {
            break;
        }
        basket.push(s.open(q));
        s.env.warp(1);
    }
    assert!(basket.len() >= 3, "a basket of {} positions", basket.len());
    let n = basket.len();
    eprintln!(
        "OI basket: {n} positions, ΣQ {} ({:.3} % of vT)",
        basket.iter().map(|p| p.open.q).sum::<u64>(),
        basket.iter().map(|p| p.open.q).sum::<u64>() as f64 * 100.0
            / curve(&s.env, &s.pm).vt as f64
    );
    // A further open is refused: the per-market cap binds.
    let mut extra = s.env.borrower();
    let (res, _) = s.env.open(&s.pm, &mut extra, D50_Q / 2);
    assert_err(&res, E::CapMarketOi);
    for t in 1..=60 {
        s.step_to(s.t() + 1);
        let _ = t;
    }
    let p_true = s.spot();
    let a = Attacker::new(&mut s);
    let saved = s.env.svm.clone();
    let t0 = s.t();
    for mult in MULTS {
        s.env.svm = saved.clone();
        let target = (p_true as f64 * mult) as u128;
        for k in 1..=120 {
            push_second(&mut s, &a, t0 + k, target, false);
            for p in &basket {
                assert!(!s.status(p).cr_liq(), "mult {mult}: forced at +{k} s");
            }
            let d = u64::try_from(move_size(&s, target)).unwrap();
            let mut ixs = vec![pump::cu_limit_ix(1_400_000), a.buy_ix(&s, d)];
            ixs.push(liquidate_ix(
                &s.pm,
                &a.key(),
                &basket[0].key(),
                basket[0].nonce,
            ));
            ixs.push(a.sell_ix(&s, d));
            assert_err(&s.env.send(&ixs, &[&a.kp()]), E::NotLiquidatable);
        }
    }
    assert_eq!(market(&s.env, &s.pm).open_positions as usize, n);
}
