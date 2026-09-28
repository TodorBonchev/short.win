//! A market's venue as short.win's adapter must see it, computed by the harness on its own from
//! the venue-state readers and `shortwin-math`'s quotes (never by calling the program's
//! adapter): the hooks' I13 re-quotes and I16 route predicates (`super::positions`).
//!
//! - **Mock** (M2; `tests/mock_cpamm/INTERFACE.md` "Reader contract"): executable iff the pool
//!   parses and both reserves `vault − counter` are defined and non-zero; `quote::cpmm` with the
//!   pool's fee.
//! - **Raydium AMM v4 / CPMM** (M3; ARCH §4.1–§4.3, §5.3): the ReadSet the market's pool cache
//!   names parses (pool, vaults under their token programs with the pool's mints, the CPMM
//!   config, the LP mint), still describes the adoption (the vaults, the LP mint, the WSOL side,
//!   the market's mint and token program; AMM v4 nonce 254; the CPMM observation), the fee
//!   resolves, and the reader's venue predicate holds at the clock's `unix_timestamp` (on CPMM
//!   with the config: a buy must be able to split its fee, M3-SEC-1); pricing
//!   `quote::ray_v4` / `quote::cpmm` (per direction) on `vault − need_take_pnl` / `vault −
//!   counters`.
//!
//! A venue that cannot execute has no reserves and `fee_ppm` 0, as the program's
//! `VenueState::not_executable` (the route predicates then hold whatever the fee).

use anchor_lang::prelude::Pubkey;
use litesvm::LiteSVM;
use shortwin::constants::{RAYDIUM_V4_AUTHORITY_NONCE, TOKEN_PROGRAM_ID};
use shortwin::state::{MARKET_FLAG_SOL_IS_TOKEN0_OR_COIN, Market, VenueKind, raydium_cache};
use shortwin_math::fees::{self, CpmmConfig, CpmmPoolFees, Direction};
use shortwin_math::quote::Pricing;
use shortwin_venue_state::{mint, raydium_cpmm, raydium_v4, token_account};
use solana_clock::Clock;

use crate::read;

/// A venue as short.win's adapter sees it: executable, pricing reserves `(X, Y)` and fees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VenueView {
    /// `venue_executable` (ARCH §5.3).
    pub executable: bool,
    /// Pricing reserves `(X, Y)` (token, SOL) when executable.
    pub reserves: Option<(u64, u64)>,
    /// One-way fee φ in ppm (0 when not executable).
    pub fee_ppm: u32,
    /// The resolved fees (execution quotes) when executable.
    pub pricing: Option<Pricing>,
}

impl VenueView {
    /// A venue that cannot execute.
    pub const DEAD: Self = Self {
        executable: false,
        reserves: None,
        fee_ppm: 0,
        pricing: None,
    };

    /// `B̂_live(q)`: the exact-out buy of `q` tokens on this state.
    pub fn buy(&self, q: u64) -> Option<u64> {
        let (x, y) = self.reserves?;
        self.pricing?
            .buy(q, u128::from(x), u128::from(y))
            .map(|b| b.cost)
    }

    /// `S_exp(q)`: the exact-in sale of `q` tokens on this state.
    pub fn sell(&self, q: u64) -> Option<u64> {
        let (x, y) = self.reserves?;
        self.pricing?
            .sell(q, u128::from(x), u128::from(y), y)
            .map(|s| s.proceeds)
    }
}

/// The view of `m`'s venue now, `None` for a venue the harness has no view of (list-only,
/// the pump venues until their milestones).
pub fn view(svm: &LiteSVM, m: &Market) -> Option<VenueView> {
    match m.venue_kind()? {
        VenueKind::Mock => Some(mock(svm, m)),
        VenueKind::RaydiumV4 => Some(ray_v4(svm, m).unwrap_or(VenueView::DEAD)),
        VenueKind::RaydiumCpmm => Some(ray_cpmm(svm, m).unwrap_or(VenueView::DEAD)),
        _ => None,
    }
}

fn mock(svm: &LiteSVM, m: &Market) -> VenueView {
    use shortwin_venue_state::mock_cpamm;
    let pool = m.pool_cache[raydium_cache::RAY_POOL];
    let Some(acc) = svm.get_account(&pool) else {
        return VenueView::DEAD;
    };
    let Ok(p) = mock_cpamm::read(&acc.owner.to_bytes(), &acc.data) else {
        return VenueView::DEAD;
    };
    let base = read::token_amount(svm, &Pubkey::new_from_array(p.base_vault)).unwrap_or(0);
    let quote = read::token_amount(svm, &Pubkey::new_from_array(p.quote_vault)).unwrap_or(0);
    let reserves = mock_cpamm::reserves(&p, base, quote);
    let f = crate::mock::cpmm_fees(p.fee_ppm);
    VenueView {
        executable: reserves.is_some(),
        reserves,
        fee_ppm: p.fee_ppm,
        pricing: Some(Pricing::Cpmm { buy: f, sell: f }),
    }
}

fn now(svm: &LiteSVM) -> i64 {
    svm.get_sysvar::<Clock>().unix_timestamp
}

fn cache(m: &Market, i: usize) -> Pubkey {
    m.pool_cache[i]
}

fn sol_first(m: &Market) -> bool {
    m.has_flag(MARKET_FLAG_SOL_IS_TOKEN0_OR_COIN)
}

/// A token account of `program` (`None` otherwise).
fn token(svm: &LiteSVM, key: &Pubkey, program: &Pubkey) -> Option<token_account::TokenAccount> {
    let acc = svm.get_account(key)?;
    if acc.owner != *program {
        return None;
    }
    token_account::read(&acc.owner.to_bytes(), &acc.data).ok()
}

/// An SPL Token LP mint's supply.
fn lp_supply(svm: &LiteSVM, key: &Pubkey) -> Option<u64> {
    let acc = svm.get_account(key)?;
    if acc.owner != TOKEN_PROGRAM_ID {
        return None;
    }
    mint::read_base(&acc.owner.to_bytes(), &acc.data)
        .ok()
        .map(|b| b.supply)
}

fn ray_v4(svm: &LiteSVM, m: &Market) -> Option<VenueView> {
    let acc = svm.get_account(&cache(m, raydium_cache::RAY_POOL))?;
    let info = raydium_v4::read(&acc.owner.to_bytes(), &acc.data).ok()?;
    let sol_is_coin = sol_first(m);
    let (sol_vault, tok_vault) = (
        cache(m, raydium_cache::RAY_VAULT_SOL),
        cache(m, raydium_cache::RAY_VAULT_TOK),
    );
    let (coin_vault, pc_vault) = if sol_is_coin {
        (sol_vault, tok_vault)
    } else {
        (tok_vault, sol_vault)
    };
    let tok_mint = if sol_is_coin {
        info.pc_vault_mint
    } else {
        info.coin_vault_mint
    };
    if info.coin_vault != coin_vault.to_bytes()
        || info.pc_vault != pc_vault.to_bytes()
        || info.lp_mint != cache(m, raydium_cache::RAY_LP_MINT).to_bytes()
        || info.nonce != u64::from(RAYDIUM_V4_AUTHORITY_NONCE)
        || info.sol_is_coin() != Some(sol_is_coin)
        || tok_mint != m.mint.to_bytes()
        || m.token_program != TOKEN_PROGRAM_ID
    {
        return None;
    }
    let coin = token(svm, &coin_vault, &TOKEN_PROGRAM_ID)?;
    let pc = token(svm, &pc_vault, &TOKEN_PROGRAM_ID)?;
    if coin.mint != info.coin_vault_mint || pc.mint != info.pc_vault_mint {
        return None;
    }
    lp_supply(svm, &cache(m, raydium_cache::RAY_LP_MINT))?;
    let fee = fees::ray_v4(info.swap_fee_numerator, info.swap_fee_denominator).ok()?;
    if !info.venue_executable(now(svm), coin.amount, pc.amount) {
        return None;
    }
    let (c, p) = info.reserves(coin.amount, pc.amount)?;
    Some(VenueView {
        executable: true,
        reserves: Some(if sol_is_coin { (p, c) } else { (c, p) }),
        fee_ppm: fee.oneway_ppm(),
        pricing: Some(Pricing::RayV4(fee)),
    })
}

fn ray_cpmm(svm: &LiteSVM, m: &Market) -> Option<VenueView> {
    let acc = svm.get_account(&cache(m, raydium_cache::RAY_POOL))?;
    let pool = raydium_cpmm::read_pool(&acc.owner.to_bytes(), &acc.data).ok()?;
    let cfg_key = cache(m, raydium_cache::RAY_AMM_CONFIG);
    let cfg_acc = svm.get_account(&cfg_key)?;
    let cfg = raydium_cpmm::read_config(&cfg_acc.owner.to_bytes(), &cfg_acc.data).ok()?;
    let sol0 = sol_first(m);
    let (sol_vault, tok_vault) = (
        cache(m, raydium_cache::RAY_VAULT_SOL),
        cache(m, raydium_cache::RAY_VAULT_TOK),
    );
    let (v0_key, v1_key) = if sol0 {
        (sol_vault, tok_vault)
    } else {
        (tok_vault, sol_vault)
    };
    let (tok_mint, sol_prog, tok_prog) = if sol0 {
        (
            pool.token_1_mint,
            pool.token_0_program,
            pool.token_1_program,
        )
    } else {
        (
            pool.token_0_mint,
            pool.token_1_program,
            pool.token_0_program,
        )
    };
    if pool.amm_config != cfg_key.to_bytes()
        || pool.token_0_vault != v0_key.to_bytes()
        || pool.token_1_vault != v1_key.to_bytes()
        || pool.lp_mint != cache(m, raydium_cache::RAY_LP_MINT).to_bytes()
        || pool.observation_key != cache(m, raydium_cache::RAY_OBSERVATION).to_bytes()
        || pool.sol_is_token0() != Some(sol0)
        || tok_mint != m.mint.to_bytes()
        || sol_prog != TOKEN_PROGRAM_ID.to_bytes()
        || tok_prog != m.token_program.to_bytes()
    {
        return None;
    }
    let v0 = token(svm, &v0_key, &Pubkey::new_from_array(pool.token_0_program))?;
    let v1 = token(svm, &v1_key, &Pubkey::new_from_array(pool.token_1_program))?;
    if v0.mint != pool.token_0_mint || v1.mint != pool.token_1_mint {
        return None;
    }
    lp_supply(svm, &cache(m, raydium_cache::RAY_LP_MINT))?;
    let fee_ppm = pool.fee_ppm(&cfg)?;
    let resolve = |dir| {
        fees::cpmm(
            &CpmmConfig {
                trade_fee_rate: cfg.trade_fee_rate,
                protocol_fee_rate: cfg.protocol_fee_rate,
                fund_fee_rate: cfg.fund_fee_rate,
                creator_fee_rate: cfg.creator_fee_rate,
            },
            &CpmmPoolFees {
                enable_creator_fee: pool.enable_creator_fee,
                creator_fee_on: pool.creator_fee_on,
                sol_is_token0: sol0,
            },
            dir,
        )
        .ok()
    };
    let (buy, sell) = (resolve(Direction::Buy)?, resolve(Direction::Sell)?);
    // The reader's predicate on the config, M3-SEC-1's fee split included (a buy whose creator
    // fee is on its input at `t + c = 0` reverts in the venue).
    if !pool.venue_executable(&cfg, now(svm), v0.amount, v1.amount) {
        return None;
    }
    let (r0, r1) = pool.reserves(v0.amount, v1.amount)?;
    Some(VenueView {
        executable: true,
        reserves: Some(if sol0 { (r1, r0) } else { (r0, r1) }),
        fee_ppm,
        pricing: Some(Pricing::Cpmm { buy, sell }),
    })
}
