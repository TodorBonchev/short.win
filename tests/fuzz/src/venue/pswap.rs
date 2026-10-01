//! PumpSwap on the fork (M6): a canonical pool of the `pamm_state` dump (`fresh` with a running
//! boost, `mature` with third-party LP, `cashback`, `legacy261`), a trader, and a third-party
//! LP whose `deposit` / `withdraw` are the LP-wrapped bundles of M6R-1; `disable_flags` as the
//! venue's halt (bit 3 trading, bit 0 `create_pool`).

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use shortwin::constants::{
    GUARD_PUMP_AMM, GUARD_PUMP_FEES, PUMP_AMM_GLOBAL_CONFIG, VENUE_PROGRAM_DATA,
};
use shortwin_litesvm_tests::engine::VenueMarket;
use shortwin_litesvm_tests::fork::{self, PumpProgram};
use shortwin_litesvm_tests::pamm::{
    self, GC_DISABLE_FLAGS, PammCoin, PammLp, PammMarket, PammTrade, PammUser, pamm_trade_ix,
};
use shortwin_litesvm_tests::{Env, errors, read};
use solana_keypair::Keypair;
use solana_signer::Signer;

use super::{Move, Venue, factor};
use crate::action::Kind;
use crate::rng::Rng;
use crate::stats::Ctx;
use crate::world::Slot;

/// The largest swap input the fuzzer funds (10⁷ SOL or 10¹⁷ raw tokens).
const MAX_SWAP: u64 = 10_000_000_000_000_000;

/// A PumpSwap market as the fuzzer drives it.
pub struct PammVenue {
    pub pm: PammMarket,
    pub user: PammUser,
    pub lp: PammLp,
    pub coin: String,
}

/// The pAMM `GlobalConfig.disable_flags` byte.
pub fn disable_flags(env: &Env) -> u8 {
    env.svm
        .get_account(&PUMP_AMM_GLOBAL_CONFIG)
        .map_or(0, |a| a.data[GC_DISABLE_FLAGS])
}

/// The live deploy slot of a venue program's ProgramData.
pub fn program_data_slot(env: &Env, index: u8) -> Option<u64> {
    let key = VENUE_PROGRAM_DATA.get(usize::from(index))?;
    let acc = env.svm.get_account(key)?;
    Some(shortwin_venue_state::bytes::u64_at(&acc.data, 4))
}

/// The pump program of a guard index.
pub fn pump_program(index: u8) -> PumpProgram {
    match index {
        0 => PumpProgram::Pump,
        1 => PumpProgram::Amm,
        _ => PumpProgram::Fees,
    }
}

impl PammVenue {
    /// The venue of `pm`: a trader and a third-party LP holding a real 5 % deposit.
    pub fn new(env: &mut Env, pm: PammMarket, coin: &str) -> Self {
        let user = PammUser::new(env, &pm.pool, 1_000_000_000_000_000);
        let lp = PammLp::funded_for(env, &pm.pool, 4_000);
        let minted = PammLp::lp_for(&env.svm, &pm.pool, 500);
        let kp = lp.kp.insecure_clone();
        // A pool whose deposits are disabled leaves the LP empty (its moves are then skipped).
        let _ = env.send(&[lp.deposit_ix(&pm.pool, minted)], &[&kp]);
        Self {
            pm,
            user,
            lp,
            coin: coin.to_string(),
        }
    }

    fn fund_user(&self, env: &mut Env, wsol: u64, base: u64) {
        if wsol > 0 {
            let have = read::token_amount(&env.svm, &self.user.wsol).unwrap_or(0);
            if have < wsol {
                fork::wrap(
                    env,
                    &self.user.wsol,
                    (wsol - have).saturating_add(wsol / 10),
                );
            }
        }
        if base > 0 {
            let have = read::token_amount(&env.svm, &self.user.base).unwrap_or(0);
            if have < base {
                fork::set_token_amount(
                    &mut env.svm,
                    &self.user.base,
                    base.saturating_add(base / 10),
                );
            }
        }
    }

    fn trade_ix(&self, env: &Env, kind: PammTrade, a: u64, b: u64) -> Instruction {
        pamm_trade_ix(
            &env.svm,
            &self.pm.pool,
            kind,
            &self.user.kp.pubkey(),
            &self.user.base,
            &self.user.wsol,
            a,
            b,
        )
    }

    fn pool_xy(&self, env: &Env) -> Option<(u64, u64)> {
        let r = self.pm.pool.reserves(&env.svm)?;
        let y = u64::try_from(r.effective_quote).ok()?;
        (r.base > 0 && y > 0).then_some((r.base, y))
    }
}

impl Venue for PammVenue {
    fn label(&self) -> String {
        format!("pumpswap({})", self.coin)
    }

    fn vm(&self) -> &dyn VenueMarket {
        &self.pm
    }

    fn seed_lender(&self) -> (Keypair, Pubkey) {
        (self.pm.lender.insecure_clone(), self.pm.lender_account)
    }

    fn fund_tokens(&mut self, env: &mut Env, owner: &Pubkey, amount: u64) -> Pubkey {
        let program = self.pm.pool.token_program();
        fork::fund_token(env, owner, &self.pm.pool.mint, program, amount)
    }

    fn top_up(&mut self, env: &mut Env, account: &Pubkey, amount: u64) {
        let have = read::token_amount(&env.svm, account).unwrap_or(0);
        fork::set_token_amount(&mut env.svm, account, have.saturating_add(amount));
    }

    fn reserves(&self, env: &Env) -> Option<(u64, u64)> {
        self.pool_xy(env)
    }

    fn push(&mut self, env: &mut Env, up: bool, k_ppm: u64) -> Option<Move> {
        let (x, y) = self.pool_xy(env)?;
        let k = factor(k_ppm).sqrt();
        let signer = self.user.kp.insecure_clone();
        if up {
            let out = (x as f64 * (1.0 - 1.0 / k)) as u64;
            if out == 0 || out >= x {
                return None;
            }
            let cost = ((y as f64) * (k - 1.0) * 1.1) as u64;
            if cost > MAX_SWAP {
                return None;
            }
            self.fund_user(env, cost.max(1), out);
            Some(Move {
                front: self.trade_ix(env, PammTrade::Buy, out, u64::MAX),
                back: Some(self.trade_ix(env, PammTrade::Sell, out, 0)),
                signer,
            })
        } else {
            let inp = (x as f64 * (k - 1.0) * 1.02) as u64;
            if inp == 0 || inp > MAX_SWAP {
                return None;
            }
            self.fund_user(env, y.saturating_mul(2).min(MAX_SWAP), inp);
            Some(Move {
                front: self.trade_ix(env, PammTrade::Sell, inp, 0),
                back: Some(self.trade_ix(env, PammTrade::Buy, inp, u64::MAX)),
                signer,
            })
        }
    }

    fn trade(&mut self, env: &mut Env, buy: bool, size: u64) -> Option<Move> {
        let (x, y) = self.pool_xy(env)?;
        let signer = self.user.kp.insecure_clone();
        if buy {
            let out =
                (u128::from(x) * u128::from(size) / u128::from(y.saturating_add(size))) as u64;
            if out == 0 {
                return None;
            }
            self.fund_user(env, size.saturating_mul(2).min(MAX_SWAP), 0);
            Some(Move {
                front: self.trade_ix(env, PammTrade::Buy, out, u64::MAX),
                back: None,
                signer,
            })
        } else {
            self.fund_user(env, 0, size.min(MAX_SWAP));
            Some(Move {
                front: self.trade_ix(env, PammTrade::Sell, size.min(MAX_SWAP), 0),
                back: None,
                signer,
            })
        }
    }

    fn lp_pair(&mut self, env: &mut Env, bps: u32) -> Option<(Instruction, Instruction, Keypair)> {
        let have = self.lp.lp_balance(&env.svm);
        let lp = PammLp::lp_for(&env.svm, &self.pm.pool, u64::from(bps)).min(have);
        if lp == 0 {
            return None;
        }
        // Funds for the re-deposit (the pool may have moved since the LP was funded).
        let r = self.pm.pool.reserves(&env.svm)?;
        let base_need = r.base.saturating_mul(u64::from(bps)) / 5_000 + 1;
        let sol_need = r.real_quote.saturating_mul(u64::from(bps)) / 5_000 + 1;
        let hb = read::token_amount(&env.svm, &self.lp.base).unwrap_or(0);
        if hb < base_need {
            fork::set_token_amount(&mut env.svm, &self.lp.base, base_need);
        }
        let hw = read::token_amount(&env.svm, &self.lp.wsol).unwrap_or(0);
        if hw < sol_need {
            fork::wrap(env, &self.lp.wsol, sol_need - hw);
        }
        Some((
            self.lp.withdraw_ix(&self.pm.pool, lp),
            self.lp.deposit_ix(&self.pm.pool, lp),
            self.lp.kp.insecure_clone(),
        ))
    }

    fn halt(&mut self, cx: &mut Ctx, level: u32) -> bool {
        // Bits 0..3 of `disable_flags` (bit 3 halts trading; bit 0 halts `create_pool`).
        let flags = (level % 16) as u8;
        pamm::set_disable_flags(&mut cx.env.svm, flags);
        cx.env.assert_invariants();
        true
    }

    fn special(&mut self, cx: &mut Ctx, kind: Kind, a: u32, _b: u32) -> Option<()> {
        match kind {
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

    fn sweep_ix(&self, env: &Env) -> Instruction {
        pamm::sweep_trader_pamm_ix(env, &self.pm)
    }

    fn donation_targets(&self) -> Vec<Pubkey> {
        vec![self.pm.pool.boost_vault_ata]
    }

    fn exit_cu(&self) -> Option<u32> {
        Some(400_000)
    }

    fn guard_programs(&self) -> Vec<u8> {
        vec![GUARD_PUMP_AMM, GUARD_PUMP_FEES]
    }

    fn set_deploy_slot(&mut self, env: &mut Env, index: u8, slot: u64) {
        fork::set_pump_deploy_slot(&mut env.svm, pump_program(index), slot);
    }

    fn deploy_slot(&self, env: &Env, index: u8) -> Option<u64> {
        program_data_slot(env, index)
    }

    fn restore(&mut self, cx: &mut Ctx) {
        pamm::set_disable_flags(&mut cx.env.svm, 0);
        cx.env.assert_invariants();
    }

    fn disturbed(&self, env: &Env) -> bool {
        disable_flags(env) != 0
    }
}

/// The PumpSwap profile: a random pAMM coin listed on the fork, `40·q` lent (as `pamm_it`), a
/// fresh oracle three times in four.
pub fn setup(rng: &mut Rng) -> (Env, Vec<Slot>) {
    let coin = [
        PammCoin::Fresh,
        PammCoin::Mature,
        PammCoin::Cashback,
        PammCoin::Legacy261,
    ][rng.below(4) as usize];
    let (mut env, p) = Env::fork_pamm_coin_with(coin, crate::setup::tweak_args(rng));
    let r = p.reserves(&env.svm).expect("reserves");
    let base = u128::from(r.base);
    let q = (base / 100).min(base * 10_000_000_000 / r.effective_quote.max(1)) as u64;
    let curve = shortwin_litesvm_tests::pda::bonding_curve(&p.mint);
    if env
        .svm
        .get_account(&curve)
        .is_some_and(|a| a.data.len() < shortwin_venue_state::pump::CURVE_MIN_LEN)
    {
        let payer = env.payer.pubkey();
        errors::ok(env.send(&[pamm::pump_extend_account_ix(&payer, &curve)], &[]));
    }
    errors::ok(env.list_pamm(&p.mint, &p.base_program));
    let pm = env.pamm_market_listed(&p.mint, q.saturating_mul(40), 60);
    if rng.chance(3, 4) {
        env.fresh_oracle();
    }
    let v = PammVenue::new(&mut env, pm, coin.role());
    let slot = crate::setup::slot(&mut env, Box::new(v), q.saturating_mul(10));
    (env, vec![slot])
}
