//! The pump curve's differential exit oracle (M10, G-I11-real; ARCH §3.6, §3.7, §8.3 I11 /
//! I-E11): every keeper `liquidate` / `expire` the fuzzer sends on a live curve market is decided
//! first from `shortwin-math` and the venue-state readers alone — the curve E2E's keeper view
//! ([`crate::curve_support::status`]: the robust-spot and EMA legs, `B̂_live` at the live fee,
//! "normal buyback allowed") and the references and bounds of `shortwin_math::bounds` on the
//! curve's `CurvePricing` at the position's hard cap (`B̂_ref = B̂(Q; reprice(L_b, P_ref))`, `L_b =
//! max(L_live, max(L_ema', L_slow))` with no depth cap off Raydium and PumpSwap) — on the state
//! after a same-second `crank_price` on a clone of the VM (the exit's own prologue is exactly a
//! crank's), in the program's order (ARCH §3.6 precondition 1, 1b, steps 2–6; §3.7):
//!
//! `NoRoute` (not executable) → `CurveStraddle` (`q > rT`) → `EmaWarmingUp` → `ReferenceStale` →
//! `NotLiquidatable` / `NotExpired` … `UseSolSettle` → `UseExpire` / `UseLiquidate` →
//! `ExitBoundNotMet` (an undefined reference quote, or `B̂_live > max_in`) → executes.
//!
//! The exit must then do exactly that: land when the prediction executes (with the event's
//! `b_live`, `b_ref` and `max_in` equal to the prediction's), or be refused with the predicted code.
//! In particular no `ExitBoundNotMet` while `B̂_live ≤ max_in = min(⌊B̂_ref·(10⁴ + δ)/10⁴⌋, …)`
//! with a fresh bucket (I-E11's liveness clause, which the hooks cannot see on a refusal). Its
//! domain is a market still on the live curve after the crank (a completion or a graduation in
//! the crank moves it: not modelled, the exit goes out unchecked).

use anchor_lang::prelude::Pubkey;
use shortwin::errors::ShortwinError as E;
use shortwin::events::{Expire, Liquidate};
use shortwin::state::{ShortPosition, VenueKind};
use shortwin_litesvm_tests::engine::crank_price_ix;
use shortwin_litesvm_tests::pump::PumpMarket;
use shortwin_litesvm_tests::{Env, TxResult, events, pda, read};
use shortwin_math::bounds;
use shortwin_math::quote;

use crate::curve_support::{self as cs, CurveCtx, Status};

/// Which keeper exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Liquidate,
    Expire,
}

/// A keeper exit's predicted decision and the figures it rests on.
#[derive(Clone, Debug)]
pub struct Prediction {
    pub which: Which,
    /// The refusal, or the bound the leg runs under.
    pub decision: Result<Bound, E>,
    pub status: Status,
    /// `B̂_ref` and `max_in` of the exit's reference (`None`: undefined).
    pub b_ref: Option<u64>,
    pub max_in: Option<u64>,
    pub ema_warm: u8,
    pub q: u64,
    pub rt: u128,
}

/// An executing exit's bound (the event's figures).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bound {
    pub b_live: u64,
    pub b_ref: u64,
    pub max_in: u64,
}

/// The prediction for `borrower`'s position `nonce` on the curve market `pm`, on a clone after a
/// same-second crank (the VM is restored); `None` outside the oracle's domain (module docs).
pub fn predict(
    env: &mut Env,
    pm: &PumpMarket,
    borrower: &Pubkey,
    nonce: u64,
    which: Which,
) -> Option<Prediction> {
    let saved = env.svm.clone();
    let crank = env.send(&[crank_price_ix(pm)], &[]);
    let p = crank
        .is_ok()
        .then(|| decide(env, pm, borrower, nonce, which));
    env.svm = saved;
    p.flatten()
}

fn decide(
    env: &Env,
    pm: &PumpMarket,
    borrower: &Pubkey,
    nonce: u64,
    which: Which,
) -> Option<Prediction> {
    let m = env.market(&pm.market());
    if m.venue_kind() != Some(VenueKind::PumpCurve) {
        return None;
    }
    let key = pda::position(&pm.market(), borrower, nonce).0;
    let pos: ShortPosition = read::borsh(&env.svm, &key)?;
    let st = cs::status(env, pm, borrower, nonce);
    let live = cs::curve(env, pm);
    let ctx = CurveCtx::read(env, pm);
    let obs = m.observation();
    let now = env.now();
    let cap = u64::from(pos.venue_fee_hard_cap_bps);
    let q = pos.q;
    // The reference: `L_b` with no depth cap (the curve has no locked depth), the risk fee.
    let l_live = if st.executable {
        quote::liquidity(live.vt, live.vq).ok()?
    } else {
        obs.ema.liq
    };
    let l_b = bounds::ref_liquidity(l_live, &obs.ema, &obs.ring, None).ok()?;
    let prm = pos.exit_params();
    let p_ref = match which {
        Which::Liquidate => bounds::liq_reference(&obs.ema, &obs.ring, &obs.last, now, &prm),
        Which::Expire => bounds::expiry_reference(&obs.ema, &obs.ring, &obs.last, now, &prm),
    }
    .ok()?;
    let b_ref = match (&ctx, p_ref) {
        (Some(c), Some(p)) => c.buy_repriced(q, l_b, p, cap).map(|b| b.cost),
        _ => None,
    };
    let max_in = b_ref.and_then(|b| match which {
        Which::Liquidate => bounds::max_in_liq(b, pos.delta_liq_bps, pos.v, st.k_s, pos.ert),
        Which::Expire => {
            let pi = bounds::pi_ref(b, pos.expiry_fee_bps).ok()?;
            bounds::max_in_exp(
                b,
                pos.delta_exp_bps,
                pos.exec_tol_bps,
                pos.v,
                st.k_s,
                st.r_due,
                pi,
                pos.ert,
            )
        }
    });
    let decision = (|| {
        if !st.executable {
            return Err(E::NoRoute);
        }
        if u128::from(q) > live.rt {
            return Err(E::CurveStraddle);
        }
        if m.ema_warm != 0 {
            return Err(E::EmaWarmingUp);
        }
        match which {
            Which::Liquidate => {
                if !st.fresh {
                    return Err(E::ReferenceStale);
                }
                if !(st.cr_liq() || st.expired) {
                    return Err(E::NotLiquidatable);
                }
                if !st.buyback_allowed {
                    return Err(E::UseSolSettle);
                }
                if st.expired && !st.cr_liq() {
                    return Err(E::UseExpire);
                }
            }
            Which::Expire => {
                if !st.expired {
                    return Err(E::NotExpired);
                }
                if !st.fresh {
                    return Err(E::ReferenceStale);
                }
                if st.cr_liq() {
                    return Err(E::UseLiquidate);
                }
                if !st.buyback_allowed {
                    return Err(E::UseSolSettle);
                }
            }
        }
        p_ref.ok_or(E::ReferenceStale)?;
        let b_ref = b_ref.ok_or(E::ExitBoundNotMet)?;
        let max_in = max_in.ok_or(E::ExitBoundNotMet)?;
        let b_live = st.b_live.ok_or(E::UseSolSettle)?;
        if b_live > max_in {
            return Err(E::ExitBoundNotMet);
        }
        Ok(Bound {
            b_live,
            b_ref,
            max_in,
        })
    })();
    Some(Prediction {
        which,
        decision,
        status: st,
        b_ref,
        max_in,
        ema_warm: m.ema_warm,
        q,
        rt: live.rt,
    })
}

/// Compares the exit's outcome `res` (`code`: its short.win error code, if it failed in
/// short.win) with the prediction. `Err` describes a mismatch; `Ok` names the outcome for the
/// summary.
pub fn check(pred: &Prediction, res: &TxResult, code: Option<u32>) -> Result<String, String> {
    let ctx = format!(
        "prediction: {:?} (b_ref {:?}, max_in {:?}, q {}, rT {}, ema_warm {})\nstatus: {:?}",
        pred.decision, pred.b_ref, pred.max_in, pred.q, pred.rt, pred.ema_warm, pred.status
    );
    check_outcome("curve", pred.which, &pred.decision, &ctx, res, code)
}

/// The comparison of [`check`] for any venue's decision-level prediction (the curve's, and
/// since M10 PumpSwap's, [`crate::pswap_exit`]): an executing prediction must land with the
/// event's `(b_live, b_ref, max_in)`, a refusal must be the predicted code; a venue refusal of
/// a leg predicted to execute fails (I-E11 / I13), a compute-budget exhaustion does not (an
/// anomaly). `venue` tags the outcome (`liquidate:<venue>-differential-…`); `ctx` describes the
/// prediction for a mismatch.
pub fn check_outcome(
    venue: &str,
    which: Which,
    decision: &Result<Bound, E>,
    ctx: &str,
    res: &TxResult,
    code: Option<u32>,
) -> Result<String, String> {
    let name = match which {
        Which::Liquidate => "liquidate",
        Which::Expire => "expire",
    };
    let mismatch = |what: String| format!("{what}\n{ctx}");
    match (res, decision) {
        (Ok(meta), Ok(want)) => {
            let got = match which {
                Which::Liquidate => events::of::<Liquidate>(meta).first().map(|e| Bound {
                    b_live: e.b_live,
                    b_ref: e.b_ref,
                    max_in: e.max_in,
                }),
                Which::Expire => events::of::<Expire>(meta).first().map(|e| Bound {
                    b_live: e.b_live,
                    b_ref: e.b_ref,
                    max_in: e.max_in,
                }),
            };
            if got != Some(*want) {
                return Err(mismatch(format!(
                    "{name} landed with (b_live, b_ref, max_in) {got:?}, predicted {want:?}"
                )));
            }
            Ok(format!("{name}:{venue}-differential-landed"))
        }
        (Ok(_), Err(e)) => Err(mismatch(format!(
            "{name} landed; predicted the refusal {e:?}"
        ))),
        (Err(_), Ok(_)) => match code {
            // I-E11: an executable, fresh, liquidatable, buyback-allowed position with
            // `B̂_live ≤ max_in` must not wait (nor be refused otherwise by short.win).
            Some(c) if venue_failed(res) => Err(mismatch(format!(
                "{name}: the venue refused the buyback the program chose to run (code {c}); \
                 predicted to execute (I-E11 / I13)"
            ))),
            Some(c) => Err(mismatch(format!(
                "{name} refused {} ({c}); predicted to execute (I-E11 for ExitBoundNotMet)",
                crate::world::code_name(c)
            ))),
            // The compute budget ran out (`Ctx::send` records it as an anomaly): not a decision.
            None if compute_exhausted(res) => {
                Ok(format!("{name}:{venue}-differential-compute-exhausted"))
            }
            // Any other failure without a code (an account, funds or CPI error) refused the leg
            // the program chose to run: its `B̂_live` or `max_in` wrong for the venue (I-E11, I13).
            None => Err(mismatch(format!(
                "{name} failed with {} (no short.win code); predicted to execute: the leg the \
                 program chose to run was refused (I-E11 / I13)",
                match res {
                    Err(f) => format!("{:?}", f.err),
                    Ok(_) => String::new(),
                }
            ))),
        },
        (Err(_), Err(e)) => {
            if code != Some(u32::from(*e)) {
                return Err(mismatch(format!(
                    "{name} refused {}; predicted {e:?}",
                    code.map_or_else(|| "outside short.win".into(), crate::world::code_name)
                )));
            }
            Ok(format!("{name}:{venue}-differential-refused:{e:?}"))
        }
    }
}

/// Whether `res` failed on the compute budget (the error, or the runtime's log line: a CPI that
/// runs out surfaces as its caller's failure).
fn compute_exhausted(res: &TxResult) -> bool {
    use solana_instruction::error::InstructionError as I;
    use solana_transaction_error::TransactionError as T;
    let Err(f) = res else { return false };
    matches!(
        f.err,
        T::InstructionError(
            _,
            I::ComputationalBudgetExceeded | I::ProgramFailedToComplete
        )
    ) || f.meta.logs.iter().any(|l| l.contains("exceeded CUs meter"))
}

/// Whether a program other than short.win failed inside `res` (a CPI's own error, as `Ctx::send`
/// tells a venue refusal from a framework error).
fn venue_failed(res: &TxResult) -> bool {
    let Err(f) = res else { return false };
    let me = shortwin::ID.to_string();
    f.meta
        .logs
        .iter()
        .any(|l| l.contains("failed: ") && l.starts_with("Program ") && !l.contains(&me))
}
