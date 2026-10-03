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
//! - **Pump curve** (M5; ARCH §3.12 PumpCurve, §4.1–§4.3, §5.3): the curve parses (owner,
//!   discriminator, bools), is SOL-quoted, not mayhem, at least 115 bytes and not complete, and
//!   pump's `Global` and `FeeConfig`, the curve's base ATA (holding at least `rT`) and PumpSwap's
//!   `FeeConfig` and `GlobalConfig` parse; pricing `(vT, vQ)` with `fees::pump_curve` on the
//!   live tier; the quotes `quote::sol_leg` (ε₁ = 1) up to `rT` (the straddle above it is not
//!   re-quoted here), the sale capped by `rQ`. `venue_rent`: the creator-vault top-up and, since
//!   pump's 2026-10-02 redeploy, a curve under 141 bytes growing to 141 on the trade with its rent
//!   reserve (`lamports − rQ`) topped up to `minimum_balance(141)` from the trader
//!   (`fork::CURVE_LEN_2026_10`; R-PUMP Update U10 for the growth to 125 before). A migrating
//!   curve (complete, `vT > 0`) is not executable but still **observed** (ARCH §3.0 step 7).
//!
//! - **PumpSwap** (M6; ARCH §3.12 PumpSwap, §4.1–§4.2, §5.3): the market's canonical pool parses
//!   (owner, discriminator, bools), is the mint's valid canonical pool (index 0, created by pump's
//!   pool authority, the market's mint against WSOL, not mayhem) with the vaults the market's
//!   cache names (base under the mint's program, WSOL under SPL Token, both owned by the pool),
//!   `GlobalConfig`, `FeeConfig`, the boost vault's ATA and — since M6R-1 — the pool's LP mint
//!   (SPL Token or Token-2022) parse, `vault + VQR` is a `u64`, both reserves non-zero and
//!   **`disable_flags` bit 3 clear** (T-DISABLE); the locked depth `D` from `lp_supply` and the LP
//!   mint's supply on `(base vault, vault + VQR)` (recomputed with integer square roots); pricing `(base vault,
//!   WSOL vault + VQR)` with `fees::pump_amm` at the mint's live supply; the quotes
//!   `quote::sol_leg` (ε₁ = 0). `venue_rent`: a pool under 287 bytes grows to 287 on the trade (271
//!   before pump's 2026-10-02 redeploy, `fork::PumpDeploy`) and tops its rent up to
//!   `minimum_balance(287)` from the trader (R-VEN Update H8; `pamm::POOL_LEN_2026_10`).
//!
//! A venue that cannot execute has no reserves and `fee_ppm` 0, as the program's
//! `VenueState::not_executable` (the route predicates then hold whatever the fee).

use anchor_lang::prelude::Pubkey;
use litesvm::LiteSVM;
use shortwin::constants::{RAYDIUM_V4_AUTHORITY_NONCE, TOKEN_PROGRAM_ID};
use shortwin::state::{MARKET_FLAG_SOL_IS_TOKEN0_OR_COIN, Market, VenueKind, raydium_cache};
use shortwin_math::fees::{self, CpmmConfig, CpmmPoolFees, CurveFeeSchedule, Direction, FeeBps};
use shortwin_math::quote::{CurveState, Pricing, sol_leg};
use shortwin_venue_state::{mint, pump, pump_amm, raydium_cpmm, raydium_v4, token_account};
use solana_clock::Clock;

use crate::read;

/// A venue as short.win's adapter sees it: executable, pricing reserves `(X, Y)` and fees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VenueView {
    /// `venue_executable` (ARCH §5.3).
    pub executable: bool,
    /// Observed by the prologue's record: executable, or a migrating curve (ARCH §3.0 step 7).
    pub observed: bool,
    /// Pricing reserves `(X, Y)` (token, SOL) when executable.
    pub reserves: Option<(u64, u64)>,
    /// One-way fee φ in ppm (0 when not executable).
    pub fee_ppm: u32,
    /// The one-way fee the fee-scaled caps take (M7-SEC-5): on the pump venues `min(φ_live, φ at
    /// the stored P_ema's market cap)` — recomputed here with `fees::pump_amm` / `pump_curve` at
    /// `⌊P_ema·X/WAD⌋` of quote —, elsewhere `fee_ppm`.
    pub cap_fee_ppm: u32,
    /// The resolved fees (execution quotes) when executable (constant-product venues).
    pub pricing: Option<Pricing>,
    /// The pump curve's live state and resolved fees when executable.
    pub curve: Option<(CurveState, FeeBps)>,
    /// The venue-side rent a trade on this state takes from `user` (the trader) besides its
    /// charge: pump's creator-vault top-up to `Rent.minimum_balance(0)` when the curve charges a
    /// creator fee and the vault holds less (R-PUMP U6, Update 2026-09-29; T-RENT) plus the
    /// curve's growth to 141 bytes; a short PumpSwap pool's growth to 287 bytes. 0 elsewhere.
    pub venue_rent: u64,
    /// PumpSwap (M6R-1, owner decision of 2026-09-30): the canonical pool's locked depth `D =
    /// ⌊λ_bps·isqrt(X·(Y + VQR))/10⁴⌋`, `λ_bps = ⌊(Pool.lp_supply − lp_mint.supply)·10⁴/lp_supply⌋`
    /// (recomputed here from the pool and its LP mint) when executable; `None` elsewhere.
    pub locked_depth: Option<u128>,
    /// PumpSwap: the tier's venue inputs on this state (ARCH §4.7) — `R_now = min(WSOL vault,
    /// WSOL vault + VQR)` (the reader's `R`) and `max(VQR, 0)` — when executable; `None`
    /// elsewhere (Raydium and the mock tier on `Y`, without a VQR). Read before an open's own
    /// sale (the snapshot's view): the program tiers on the venue before the trade (ARCH §3.4
    /// step 5; FUZZ-M7-24H-1).
    pub pump_tier: Option<(u64, u64)>,
}

impl VenueView {
    /// A venue that cannot execute.
    pub const DEAD: Self = Self {
        executable: false,
        observed: false,
        reserves: None,
        fee_ppm: 0,
        cap_fee_ppm: 0,
        pricing: None,
        curve: None,
        venue_rent: 0,
        locked_depth: None,
        pump_tier: None,
    };

    /// `B̂_live(q)`: the exact-out buy of `q` tokens on this state (the curve: up to `rT`).
    pub fn buy(&self, q: u64) -> Option<u64> {
        if let Some((c, f)) = self.curve {
            if u128::from(q) > c.rt {
                return None;
            }
            return sol_leg::buy_exact_out(q, c.vt, c.vq, true, &f).map(|b| b.cost);
        }
        let (x, y) = self.reserves?;
        self.pricing?
            .buy(q, u128::from(x), u128::from(y))
            .map(|b| b.cost)
    }

    /// `S_exp(q)`: the exact-in sale of `q` tokens on this state.
    pub fn sell(&self, q: u64) -> Option<u64> {
        if let Some((c, f)) = self.curve {
            let cap = u64::try_from(c.rq).unwrap_or(u64::MAX);
            return sol_leg::sell_exact_in(q, c.vt, c.vq, &f, cap).map(|s| s.proceeds);
        }
        let (x, y) = self.reserves?;
        self.pricing?
            .sell(q, u128::from(x), u128::from(y), y)
            .map(|s| s.proceeds)
    }
}

/// The view of `m`'s venue now, `None` for a venue the harness has no view of (list-only).
pub fn view(svm: &LiteSVM, m: &Market) -> Option<VenueView> {
    match m.venue_kind()? {
        VenueKind::Mock => Some(mock(svm, m)),
        VenueKind::RaydiumV4 => Some(ray_v4(svm, m).unwrap_or(VenueView::DEAD)),
        VenueKind::RaydiumCpmm => Some(ray_cpmm(svm, m).unwrap_or(VenueView::DEAD)),
        VenueKind::PumpCurve | VenueKind::PumpMigrating => Some(pump_curve(svm, m)),
        VenueKind::PumpSwap => Some(pump_swap(svm, m).unwrap_or(VenueView::DEAD)),
        _ => None,
    }
}

/// The PumpSwap view (module docs).
fn pump_swap(svm: &LiteSVM, m: &Market) -> Option<VenueView> {
    use shortwin::state::pump_swap_cache as pc;
    let pool_acc = svm.get_account(&m.canonical_pool)?;
    let p = pump_amm::read_pool(&pool_acc.owner.to_bytes(), &pool_acc.data).ok()?;
    let authority = crate::pda::pump_pool_authority(&m.mint);
    if p.index != 0
        || p.creator != authority.to_bytes()
        || p.base_mint != m.mint.to_bytes()
        || p.quote_mint != shortwin::constants::WSOL_MINT.to_bytes()
        || p.is_mayhem_mode
        || p.pool_base_token_account != cache(m, pc::POOL_BASE_VAULT).to_bytes()
        || p.pool_quote_token_account != cache(m, pc::POOL_QUOTE_VAULT).to_bytes()
    {
        return None;
    }
    let base = token(svm, &cache(m, pc::POOL_BASE_VAULT), &m.token_program)?;
    let quote = token(svm, &cache(m, pc::POOL_QUOTE_VAULT), &TOKEN_PROGRAM_ID)?;
    if base.mint != m.mint.to_bytes()
        || base.owner != m.canonical_pool.to_bytes()
        || quote.mint != shortwin::constants::WSOL_MINT.to_bytes()
        || quote.owner != m.canonical_pool.to_bytes()
    {
        return None;
    }
    let gc_acc = svm.get_account(&shortwin::constants::PUMP_AMM_GLOBAL_CONFIG)?;
    let gc = pump_amm::read_global_config(&gc_acc.owner.to_bytes(), &gc_acc.data).ok()?;
    let fc_acc = svm.get_account(&shortwin::constants::PUMP_AMM_FEE_CONFIG)?;
    let fc = pump::read_fee_config(&fc_acc.owner.to_bytes(), &fc_acc.data).ok()?;
    let boost_vault = Pubkey::find_program_address(
        &[b"boost_vault", m.canonical_pool.as_ref()],
        &shortwin::constants::PUMP_AMM_PROGRAM_ID,
    )
    .0;
    let (bo, bd) = svm
        .get_account(&cache(m, pc::BOOST_VAULT_ATA))
        .map_or(([0u8; 32], Vec::new()), |a| (a.owner.to_bytes(), a.data));
    pump_amm::boost_vault_amount(&bo, &bd, &boost_vault.to_bytes()).ok()?;
    let lp_acc = svm.get_account(&Pubkey::new_from_array(p.lp_mint))?;
    if lp_acc.owner != TOKEN_PROGRAM_ID
        && lp_acc.owner != shortwin::constants::TOKEN_2022_PROGRAM_ID
    {
        return None;
    }
    let lp_supply = mint::read_base(&lp_acc.owner.to_bytes(), &lp_acc.data)
        .ok()?
        .supply;
    let r = p.reserves(base.amount, quote.amount)?;
    let effective = u64::try_from(r.effective_quote).ok()?;
    // M6R-1: λ_bps and D on the pricing reserves, recomputed (not the program's function).
    let lam = if p.lp_supply == 0 || lp_supply > p.lp_supply {
        0
    } else {
        u128::from(p.lp_supply - lp_supply) * 10_000 / u128::from(p.lp_supply)
    };
    let root = (u128::from(r.base) * u128::from(effective)).isqrt();
    let locked_depth = lam * root / 10_000;
    let mint_acc = svm.get_account(&m.mint)?;
    let supply = mint::read_base(&mint_acc.owner.to_bytes(), &mint_acc.data)
        .ok()?
        .supply;
    let tiers: Vec<fees::FeeTier> = fc
        .fee_tiers
        .iter()
        .map(|t| fees::FeeTier {
            threshold: t.threshold,
            fees: FeeBps::new(t.fees.lp, t.fees.protocol, t.fees.creator),
        })
        .collect();
    let sched = fees::PammFeeSchedule {
        tiers: &tiers,
        base_supply: supply,
        creator_fee_configurable: gc.creator_fee_configurable(),
        pool_creator_fee_bps: p.creator_fee_bps,
        coin_creator_is_default: p.coin_creator_is_default(),
        cap_bps: None,
    };
    let f = fees::pump_amm(r.effective_quote, u128::from(r.base), &sched).ok()?;
    // M7-SEC-5: the tier at the stored P_ema's market cap (price·supply).
    let p_ema = m.ema_price.get();
    let at_ema = (p_ema > 0)
        .then(|| {
            p_ema
                .checked_mul(u128::from(r.base))
                .map(|v| v / 1_000_000_000_000_000_000)
        })
        .flatten()
        .and_then(|q| fees::pump_amm(q, u128::from(r.base), &sched).ok())
        .map_or(u32::MAX, |e| e.oneway_ppm());
    if r.base == 0 || effective == 0 || gc.disable_flags() & (1 << 3) != 0 {
        return None;
    }
    let venue_rent = if pool_acc.data.len() < crate::pamm::POOL_LEN_2026_10 {
        svm.minimum_balance_for_rent_exemption(crate::pamm::POOL_LEN_2026_10)
            .saturating_sub(pool_acc.lamports)
    } else {
        0
    };
    Some(VenueView {
        executable: true,
        observed: true,
        reserves: Some((r.base, effective)),
        fee_ppm: f.oneway_ppm(),
        cap_fee_ppm: f.oneway_ppm().min(at_ema),
        pricing: Some(Pricing::PumpSwap(f)),
        curve: None,
        venue_rent,
        locked_depth: Some(locked_depth),
        pump_tier: Some((
            r.real_quote.min(effective),
            u64::try_from(p.virtual_quote_reserves.max(0)).unwrap_or(u64::MAX),
        )),
    })
}

/// The pump curve's view (module docs).
fn pump_curve(svm: &LiteSVM, m: &Market) -> VenueView {
    let Some(acc) = svm.get_account(&m.bonding_curve) else {
        return VenueView::DEAD;
    };
    let Ok(c) = pump::read_curve(&acc.owner.to_bytes(), &acc.data) else {
        return VenueView::DEAD;
    };
    let listable = acc.data.len() >= pump::CURVE_MIN_LEN && c.is_sol_quote() && !c.is_mayhem_mode;
    let state = CurveState {
        vt: u128::from(c.virtual_token_reserves),
        vq: u128::from(c.virtual_quote_reserves),
        rt: u128::from(c.real_token_reserves),
        rq: u128::from(c.real_quote_reserves),
        complete: c.complete,
    };
    let reserves = c.virtual_token_reserves > 0 && c.virtual_quote_reserves > 0;
    let observed = listable && reserves && c.complete;
    let dead = VenueView {
        observed,
        ..VenueView::DEAD
    };
    if !listable || !reserves || c.complete {
        return dead;
    }
    let read = |k: &Pubkey| svm.get_account(k);
    let (Some(g), Some(fc), Some(pfc), Some(gc)) = (
        read(&shortwin::constants::PUMP_GLOBAL),
        read(&shortwin::constants::PUMP_FEE_CONFIG),
        read(&shortwin::constants::PUMP_AMM_FEE_CONFIG),
        read(&shortwin::constants::PUMP_AMM_GLOBAL_CONFIG),
    ) else {
        return dead;
    };
    let Ok(global) = pump::read_global(&g.owner.to_bytes(), &g.data) else {
        return dead;
    };
    let Ok(config) = pump::read_fee_config(&fc.owner.to_bytes(), &fc.data) else {
        return dead;
    };
    if pump::read_fee_config(&pfc.owner.to_bytes(), &pfc.data).is_err()
        || pump_amm::read_global_config(&gc.owner.to_bytes(), &gc.data).is_err()
    {
        return dead;
    }
    let Some(base) = token(svm, &m.curve_base_ata, &m.token_program) else {
        return dead;
    };
    if base.mint != m.mint.to_bytes()
        || base.owner != m.bonding_curve.to_bytes()
        || base.amount < c.real_token_reserves
    {
        return dead;
    }
    let tiers: Vec<fees::FeeTier> = config
        .fee_tiers
        .iter()
        .map(|t| fees::FeeTier {
            threshold: t.threshold,
            fees: FeeBps::new(t.fees.lp, t.fees.protocol, t.fees.creator),
        })
        .collect();
    let sched = CurveFeeSchedule {
        tiers: &tiers,
        creator_fee_configurable: global.creator_fee_configurable(),
        curve_creator_fee_bps: c.creator_fee_bps,
        creator_is_default: c.creator_is_default(),
        cap_bps: None,
    };
    let Ok(f) = fees::pump_curve(state.vt, state.vq, &sched) else {
        return dead;
    };
    // M7-SEC-5: the tier at the stored P_ema's market cap.
    let p_ema = m.ema_price.get();
    let at_ema = (p_ema > 0)
        .then(|| {
            p_ema
                .checked_mul(state.vt)
                .map(|v| v / 1_000_000_000_000_000_000)
        })
        .flatten()
        .and_then(|vq| fees::pump_curve(state.vt, vq, &sched).ok())
        .map_or(u32::MAX, |e| e.oneway_ppm());
    let vault = Pubkey::find_program_address(
        &[b"creator-vault", &c.creator],
        &shortwin::constants::PUMP_PROGRAM_ID,
    )
    .0;
    let held = svm.get_account(&vault).map_or(0, |a| a.lamports);
    let min0 = svm.minimum_balance_for_rent_exemption(0);
    let vault_top_up = if f.creator > 0 {
        min0.saturating_sub(held)
    } else {
        0
    };
    let growth = if acc.data.len() < crate::fork::CURVE_LEN_2026_10 {
        svm.minimum_balance_for_rent_exemption(crate::fork::CURVE_LEN_2026_10)
            .saturating_sub(acc.lamports.saturating_sub(c.real_quote_reserves))
    } else {
        0
    };
    let venue_rent = vault_top_up + growth;
    VenueView {
        executable: true,
        observed: true,
        reserves: Some((c.virtual_token_reserves, c.virtual_quote_reserves)),
        fee_ppm: f.oneway_ppm(),
        cap_fee_ppm: f.oneway_ppm().min(at_ema),
        pricing: None,
        curve: Some((state, f)),
        venue_rent,
        locked_depth: None,
        pump_tier: None,
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
        observed: reserves.is_some(),
        reserves,
        fee_ppm: p.fee_ppm,
        cap_fee_ppm: p.fee_ppm,
        pricing: Some(Pricing::Cpmm { buy: f, sell: f }),
        curve: None,
        venue_rent: 0,
        locked_depth: None,
        pump_tier: None,
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
        observed: true,
        reserves: Some(if sol_is_coin { (p, c) } else { (c, p) }),
        fee_ppm: fee.oneway_ppm(),
        cap_fee_ppm: fee.oneway_ppm(),
        pricing: Some(Pricing::RayV4(fee)),
        curve: None,
        venue_rent: 0,
        locked_depth: None,
        pump_tier: None,
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
        observed: true,
        reserves: Some(if sol0 { (r1, r0) } else { (r0, r1) }),
        fee_ppm,
        cap_fee_ppm: fee_ppm,
        pricing: Some(Pricing::Cpmm { buy, sell }),
        curve: None,
        venue_rent: 0,
        locked_depth: None,
        pump_tier: None,
    })
}
