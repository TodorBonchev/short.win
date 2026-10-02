//! The pump curve on the fork (M5) and its graduation (M6): a live curve (the `fresh` fixture
//! taken to a random progress by a real `buy_v2`, or `legacy115` / `t22_115` / `near`), other
//! users moving it with `buy_v2` / `sell_v2`, the completion (another user buys the remaining
//! real tokens), pump's real `migrate_v2`, the graduation halt (PumpSwap `disable_flags` bit 0,
//! M6-SEC-3) — and, once a crank moves the market `→ PumpSwap`, the PumpSwap venue of
//! [`super::pswap`] on the pool `migrate_v2` created.

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use shortwin::constants::{GUARD_PUMP, GUARD_PUMP_AMM, GUARD_PUMP_FEES, PUMP_FEE_CONFIG};
use shortwin::state::VenueKind;
use shortwin_litesvm_tests::engine::VenueMarket;
use shortwin_litesvm_tests::fork::{self, PumpCoin, PumpV2};
use shortwin_litesvm_tests::pamm::{self, PammMarket};
use shortwin_litesvm_tests::pump::{self, CurveUser, PumpMarket, PumpSpec};
use shortwin_litesvm_tests::{Env, errors, read};
use solana_keypair::Keypair;
use solana_signer::Signer;

use super::pswap::{
    FeeSpecs, PammVenue, disable_flags, program_data_slot, pump_fee_shock, pump_program,
};
use super::{Move, Venue, factor};
use crate::action::Kind;
use crate::rng::Rng;
use crate::stats::Ctx;
use crate::world::Slot;

/// pump `Global`'s initial reserves (the `fresh` fixture undone to `at_progress(0)`).
const PUMP_VT0: u64 = 1_073_000_000_000_000;
const PUMP_VS0: u64 = 30_000_000_000;
const PUMP_RT0: u64 = 793_100_000_000_000;
/// `migrate_v2`'s compute budget (graduation_support's).
const MIGRATE_CU: u32 = 600_000;

/// A curve market as the fuzzer drives it, before and after its graduation.
pub struct CurveVenue {
    pub pm: PumpMarket,
    pub crowd: CurveUser,
    /// The PumpSwap venue once the market moved there.
    pub grad: Option<PammVenue>,
    pub label: String,
    /// pump's `FeeConfig` as a fee shock found it (restored at level 0; after the graduation
    /// the shocks move to PumpSwap's, [`PammVenue::fee_saved`]).
    pub fee_saved: Option<FeeSpecs>,
}

impl CurveVenue {
    fn state(&self, env: &Env) -> Option<shortwin_math::quote::CurveState> {
        env.svm.get_account(&self.pm.curve.curve)?;
        Some(self.pm.curve.curve_state(&env.svm))
    }

    fn v2(&self, env: &Env, kind: PumpV2, a: u64, b: u64) -> Instruction {
        let fr = fork::pump_fee_recipient(&env.svm, 1);
        let bb = fork::pump_buyback_fee_recipient(&env.svm, 1);
        self.pm.curve.v2_ix(
            &env.svm,
            kind,
            &self.crowd.kp.pubkey(),
            &self.crowd.ata,
            &fr,
            &bb,
            a,
            b,
        )
    }

    fn ensure_tokens(&self, env: &mut Env, q: u64) {
        let have = read::token_amount(&env.svm, &self.crowd.ata).unwrap_or(0);
        if have < q {
            fork::set_token_amount(&mut env.svm, &self.crowd.ata, q);
        }
    }

    /// Moves to the PumpSwap venue if the market is on PumpSwap now.
    fn sync(&mut self, cx: &mut Ctx) {
        if self.grad.is_some() {
            return;
        }
        let m = cx.env.market(&self.pm.market());
        if m.venue_kind() != Some(VenueKind::PumpSwap) {
            return;
        }
        let pam = PammMarket::graduated(&cx.env.svm, &self.pm);
        let payer = cx.payer();
        let ixs = pamm::venue_account_ixs(&cx.env.svm, &payer, &pam);
        cx.send(&ixs, &[]);
        *cx.stats.events.entry("fuzz:graduated".into()).or_default() += 1;
        self.grad = Some(PammVenue::new(cx.env, pam, "graduated"));
    }
}

impl Venue for CurveVenue {
    fn label(&self) -> String {
        self.label.clone()
    }

    fn vm(&self) -> &dyn VenueMarket {
        match &self.grad {
            Some(g) => &g.pm,
            None => &self.pm,
        }
    }

    fn as_curve(&self) -> Option<&PumpMarket> {
        self.grad.is_none().then_some(&self.pm)
    }

    fn seed_lender(&self) -> (Keypair, Pubkey) {
        (self.pm.lender.insecure_clone(), self.pm.lender_account)
    }

    fn fund_tokens(&mut self, env: &mut Env, owner: &Pubkey, amount: u64) -> Pubkey {
        fork::fund_token(
            env,
            owner,
            &self.pm.curve.mint,
            self.pm.curve.token_program,
            amount,
        )
    }

    fn top_up(&mut self, env: &mut Env, account: &Pubkey, amount: u64) {
        let have = read::token_amount(&env.svm, account).unwrap_or(0);
        fork::set_token_amount(&mut env.svm, account, have.saturating_add(amount));
    }

    fn reserves(&self, env: &Env) -> Option<(u64, u64)> {
        if let Some(g) = &self.grad {
            return g.reserves(env);
        }
        let s = self.state(env)?;
        if s.complete || s.vt == 0 {
            return None;
        }
        Some((u64::try_from(s.vt).ok()?, u64::try_from(s.vq).ok()?))
    }

    fn push(&mut self, env: &mut Env, up: bool, k_ppm: u64) -> Option<Move> {
        if let Some(g) = &mut self.grad {
            return g.push(env, up, k_ppm);
        }
        let s = self.state(env)?;
        if s.complete || s.vt == 0 {
            return None;
        }
        let k = factor(k_ppm).sqrt();
        let vt = s.vt as f64;
        let signer = self.crowd.kp.insecure_clone();
        if up {
            // A buy of `vT·(1 − 1/√k)` tokens, never the whole `rT` (completion is its own
            // action).
            let q = ((vt * (1.0 - 1.0 / k)) as u64).min((s.rt as u64).saturating_mul(98) / 100);
            if q == 0 {
                return None;
            }
            Some(Move {
                front: self.v2(env, PumpV2::Buy, q, u64::MAX),
                back: Some(self.v2(env, PumpV2::Sell, q, 0)),
                signer,
            })
        } else {
            let q = (vt * (k - 1.0) * 1.02) as u64;
            if q == 0 || q > 10_000_000_000_000_000 {
                return None;
            }
            self.ensure_tokens(env, q);
            Some(Move {
                front: self.v2(env, PumpV2::Sell, q, 0),
                back: Some(self.v2(env, PumpV2::Buy, q, u64::MAX)),
                signer,
            })
        }
    }

    fn trade(&mut self, env: &mut Env, buy: bool, size: u64) -> Option<Move> {
        if let Some(g) = &mut self.grad {
            return g.trade(env, buy, size);
        }
        let s = self.state(env)?;
        if s.complete || s.vt == 0 {
            return None;
        }
        let signer = self.crowd.kp.insecure_clone();
        if buy {
            let q = (s.vt * u128::from(size) / (s.vq + u128::from(size))) as u64;
            let q = q.min((s.rt as u64).saturating_mul(98) / 100);
            (q > 0).then(|| Move {
                front: self.v2(env, PumpV2::Buy, q, u64::MAX),
                back: None,
                signer,
            })
        } else {
            self.ensure_tokens(env, size);
            Some(Move {
                front: self.v2(env, PumpV2::Sell, size, 0),
                back: None,
                signer,
            })
        }
    }

    fn lp_pair(&mut self, env: &mut Env, bps: u32) -> Option<(Instruction, Instruction, Keypair)> {
        self.grad.as_mut()?.lp_pair(env, bps)
    }

    fn halt(&mut self, cx: &mut Ctx, level: u32) -> bool {
        // PumpSwap's flags halt the pool after the graduation and `create_pool` (so
        // `migrate_v2`) before it.
        let flags = (level % 16) as u8;
        pamm::set_disable_flags(&mut cx.env.svm, flags);
        cx.env.assert_invariants();
        true
    }

    fn fee_shock(&mut self, cx: &mut Ctx, level: u32) -> bool {
        // pump's `FeeConfig` prices the curve (and its sale into `migrate_v2`'s pool); once the
        // market is on PumpSwap, PumpSwap's prices the pool. Level 0 restores both.
        match (&mut self.grad, level) {
            (Some(g), 0) => {
                pump_fee_shock(cx, &PUMP_FEE_CONFIG, &mut self.fee_saved, 0, None);
                g.fee_shock(cx, 0)
            }
            (Some(g), _) => g.fee_shock(cx, level),
            (None, _) => pump_fee_shock(cx, &PUMP_FEE_CONFIG, &mut self.fee_saved, level, None),
        }
    }

    fn special(&mut self, cx: &mut Ctx, kind: Kind, a: u32, _b: u32) -> Option<()> {
        match kind {
            Kind::CompleteCurve => {
                let s = self.state(cx.env)?;
                if s.complete || s.rt == 0 {
                    return None;
                }
                let ix = self.v2(cx.env, PumpV2::Buy, s.rt as u64, u64::MAX);
                let kp = self.crowd.kp.insecure_clone();
                cx.send(&[pump::cu_limit_ix(pump::CURVE_TRADE_CU), ix], &[&kp]);
                Some(())
            }
            Kind::Migrate => {
                let s = self.state(cx.env)?;
                if !s.complete || s.vt == 0 {
                    return None;
                }
                let migrator = cx.env.funded_keypair();
                let ix = pump::migrate_v2_ix(&cx.env.svm, &migrator.pubkey(), &self.pm.curve);
                cx.send(&[pump::cu_limit_ix(MIGRATE_CU), ix], &[&migrator]);
                Some(())
            }
            Kind::GraduationHalt => {
                let f = disable_flags(cx.env);
                let f = if a % 2 == 1 { f | 1 } else { f & !1 };
                pamm::set_disable_flags(&mut cx.env.svm, f);
                cx.env.assert_invariants();
                Some(())
            }
            _ => None,
        }
    }

    fn after_crank(&mut self, cx: &mut Ctx) {
        self.sync(cx);
    }

    fn sweep_ix(&self, env: &Env) -> Instruction {
        match &self.grad {
            Some(g) => g.sweep_ix(env),
            None => pump::sweep_trader_pump_ix(env, &self.pm),
        }
    }

    fn donation_targets(&self) -> Vec<Pubkey> {
        self.grad
            .as_ref()
            .map_or_else(Vec::new, Venue::donation_targets)
    }

    fn exit_cu(&self) -> Option<u32> {
        Some(400_000)
    }

    fn guard_programs(&self) -> Vec<u8> {
        vec![GUARD_PUMP, GUARD_PUMP_AMM, GUARD_PUMP_FEES]
    }

    fn set_deploy_slot(&mut self, env: &mut Env, index: u8, slot: u64) {
        fork::set_pump_deploy_slot(&mut env.svm, pump_program(index), slot);
    }

    fn deploy_slot(&self, env: &Env, index: u8) -> Option<u64> {
        program_data_slot(env, index)
    }

    fn restore(&mut self, cx: &mut Ctx) {
        pamm::set_disable_flags(&mut cx.env.svm, 0);
        self.fee_shock(cx, 0);
    }

    fn disturbed(&self, env: &Env) -> bool {
        disable_flags(env) != 0
            || self.fee_saved.is_some()
            || self.grad.as_ref().is_some_and(|g| g.fee_saved.is_some())
    }
}

/// The curve profile: `fresh` at a random progress (a real `buy_v2` from `at_progress(0)`), or
/// `legacy115` / `t22_115` / `near` as dumped; `rT/10` lent; a fresh oracle three times in four.
pub fn setup(rng: &mut Rng) -> (Env, Vec<Slot>) {
    let coin = [
        PumpCoin::Fresh,
        PumpCoin::Fresh,
        PumpCoin::Fresh,
        PumpCoin::Legacy115,
        PumpCoin::T22_115,
        PumpCoin::Near,
    ][rng.below(6) as usize];
    let mut env = crate::setup::release_env(rng);
    env.load_pump_programs();
    env.load_pump_statics();
    let c = env.load_pump_coin(coin);
    env.set_pump_rent();
    env.set_pump_clock();
    let crowd = CurveUser::new(&mut env, &c);
    let mut label = format!("curve({})", coin.role());
    if coin == PumpCoin::Fresh {
        fork::set_curve_reserves(&mut env.svm, &c.curve, PUMP_VT0, PUMP_VS0, PUMP_RT0, 0);
        let progress = [0u64, 10, 30, 50, 70, 85, 95][rng.below(7) as usize];
        let sold = PUMP_RT0 / 100 * progress;
        if sold > 0 {
            errors::ok(crowd.buy(&mut env, &c, sold, u64::MAX));
        }
        label = format!("curve(fresh@{progress}%)");
    }
    let rt = c.curve_state(&env.svm).rt as u64;
    let lend = (rt / 10).max(1_000_000_000);
    errors::ok(env.list_pump(&c));
    let pm = env.pump_market_listed(PumpSpec::new(coin, lend), c);
    if rng.chance(3, 4) {
        env.fresh_oracle();
    }
    let v = CurveVenue {
        pm,
        crowd,
        grad: None,
        label,
        fee_saved: None,
    };
    let slot = crate::setup::slot(&mut env, Box::new(v), lend / 4);
    (env, vec![slot])
}
