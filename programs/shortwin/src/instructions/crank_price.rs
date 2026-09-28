//! `crank_price()` (ARCH §3.9): anyone runs a market's prologue — accrue (vest release, rent),
//! the observation roll, the venue sync, the record (only while the venue is executable), the
//! route chains — and refreshes the cached tier. The in-house keeper sends one per second near
//! thresholds and batches the rest (ARCH §7.1; ECON §5.8).
//!
//! **Accounts:** `config`, `market` (mut), `mint`, `oracle_cache` (read), event accounts.
//! **Remaining:** the venue's `ReadSet`; for a list-only (`None`) market the detection set
//! `[bonding_curve, canonical_pool]` (key-checked; the pump re-detection of ARCH §2.5 arrives
//! with M5, until then a `None` market stays `None`).
//!
//! **Never reverts on a venue-state account that no longer parses** (A63, V1-8): a correctly
//! keyed account whose owner, discriminator or layout changed makes the venue "not
//! executable" — no observation is recorded and the S3 chain runs. Only a wrong or missing key
//! reverts (`VenueAccountMismatch`, `BadRemainingAccounts`), so a batch of cranks fails only on
//! wrong keys.
//!
//! **No `MarketVault`** (ARCH §3.9's list): the accrual's vault side is run on a scratch copy
//! and must leave it untouched. It always does: the accrual touches the vault only to send a
//! credit to the protocol while `S = 0`, and `S = 0 ⇒ ΣB_Q = 0 ⇒ A_j = 0` (M1-SEC-3,
//! `open_short`'s `S > 0` rule), so the rent credit is 0; the vest never runs with `S = 0`
//! (`pool::release` refuses it as an invariant break).
//!
//! **Events:** `PriceCranked`, plus `RouteChainChanged` when a chain started or cleared.

use anchor_lang::prelude::*;
use shortwin_math::pool::Vault;
use shortwin_math::rent;

use crate::constants::{CONFIG_VERSION, MARKET_VERSION};
use crate::engine::{prologue, tier as engine_tier};
use crate::errors::{MathResultExt, ShortwinError};
use crate::events::{PriceCranked, RouteChainChanged};
use crate::state::{GlobalConfig, Market, OracleCache, VenueKind};
use crate::venues::{self, Carry, VenueState};

#[event_cpi]
#[derive(Accounts)]
pub struct CrankPrice<'info> {
    #[account(
        seeds = [GlobalConfig::SEED],
        bump = config.load()?.bump,
        constraint = config.load()?.version == CONFIG_VERSION @ ShortwinError::ConfigVersion
    )]
    pub config: AccountLoader<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [Market::SEED, mint.key().as_ref()],
        bump = market.load()?.bump,
        constraint = market.load()?.version == MARKET_VERSION @ ShortwinError::ConfigVersion,
        has_one = mint @ ShortwinError::AccountMismatch
    )]
    pub market: AccountLoader<'info, Market>,
    /// CHECK: `== market.mint` (`has_one`) and owned by the market's token program (checked
    /// in the handler); read for nothing else in this build (PumpSwap fee tiers read its
    /// supply, M6).
    pub mint: UncheckedAccount<'info>,
    #[account(seeds = [OracleCache::SEED], bump = oracle_cache.bump)]
    pub oracle_cache: Box<Account<'info, OracleCache>>,
}

pub fn handler<'info>(ctx: Context<'info, CrankPrice<'info>>) -> Result<()> {
    let clock = Clock::get()?;
    let market_key = ctx.accounts.market.key();
    let config = ctx.accounts.config.load()?;
    let mut market = ctx.accounts.market.load_mut()?;
    require_keys_eq!(
        *ctx.accounts.mint.owner,
        market.token_program,
        ShortwinError::AccountMismatch
    );
    let kind = market.venue_kind().ok_or(ShortwinError::NoSupportedVenue)?;
    let state = if kind == VenueKind::None {
        let [bc, cp] = ctx.remaining_accounts else {
            return err!(ShortwinError::BadRemainingAccounts);
        };
        require_keys_eq!(
            bc.key(),
            market.bonding_curve,
            ShortwinError::VenueAccountMismatch
        );
        require_keys_eq!(
            cp.key(),
            market.canonical_pool,
            ShortwinError::VenueAccountMismatch
        );
        VenueState::not_executable(kind)
    } else {
        let rem = venues::parse(&market, kind, ctx.remaining_accounts, Carry::ReadSet, &[])?;
        venues::read_state(&market, kind, rem.read, clock.unix_timestamp)
    };

    let mut scratch = Box::new(Vault::new());
    let pro = prologue::run(
        &config,
        &market_key,
        &mut market,
        &mut scratch,
        Some(state),
        clock.unix_timestamp,
        clock.slot,
    )?;
    require!(
        scratch.lender_sol == 0 && scratch.protocol_fees == 0 && scratch.unmatched == 0,
        ShortwinError::MathOverflow
    );

    if let Some(tier) = engine_tier::assign(
        &config,
        &market,
        &state,
        &ctx.accounts.oracle_cache,
        pro.now,
    ) {
        if state.executable {
            market.tier = tier as u8;
            market.tier_computed_at = pro.now;
        }
    }
    let util_bps = rent::utilization_bps(market.tokens_free, market.borrowed_q).math()?;
    let event = PriceCranked {
        market: market_key,
        slot: clock.slot,
        ts: clock.unix_timestamp,
        spot_wad: state.price().unwrap_or(0),
        ema_price: market.ema_price.get(),
        ema_liq: market.ema_liq.get(),
        liq_slow: market.liq_slow.get(),
        ema_count: market.ema_count,
        lb_lo: market.lb_lo.get(),
        lb_hi: market.lb_hi.get(),
        tier: market.tier,
        util_bps,
        rent_basis: market.basis().j,
        vest_bal: market.vest_bal,
    };
    let chains = RouteChainChanged {
        market: market_key,
        slot: clock.slot,
        ts: clock.unix_timestamp,
        no_route_since: market.no_route_since,
        last_no_route_obs: market.last_no_route_obs,
        pool_fail_since: market.pool_fail_since,
        last_pool_fail_obs: market.last_pool_fail_obs,
    };
    drop(market);
    drop(config);
    emit_cpi!(event);
    if pro.chains_changed {
        emit_cpi!(chains);
    }
    Ok(())
}
