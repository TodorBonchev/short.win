//! The instructions the keeper sends (ARCHITECTURE §3.6–§3.9, pool selection included), built
//! from the program's own Anchor account and instruction types (`shortwin::accounts::*`,
//! `shortwin::instruction::*`), with the venue's remaining accounts from [`crate::venue`].

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::{InstructionData, ToAccountMetas};

use shortwin::state::{Market, VenueKind};

use crate::pda;
use crate::venue::{self, ExtrasChoice};
use crate::view::{MarketView, PositionView};

fn build(
    accounts: impl ToAccountMetas,
    data: impl InstructionData,
    rest: Vec<AccountMeta>,
) -> Instruction {
    let mut metas = accounts.to_account_metas(None);
    metas.extend(rest);
    Instruction {
        program_id: shortwin::ID,
        accounts: metas,
        data: data.data(),
    }
}

/// The ReadSet of `crank_price` and `sol_settle`: every slot read-only.
fn read_only(mv: &MarketView) -> Vec<AccountMeta> {
    mv.read_keys
        .iter()
        .map(|k| AccountMeta::new_readonly(*k, false))
        .collect()
}

/// `crank_price()` with the market's ReadSet (read-only).
pub fn crank_price(mv: &MarketView) -> Instruction {
    build(
        shortwin::accounts::CrankPrice {
            config: pda::config(),
            market: mv.key,
            mint: mv.market.mint,
            oracle_cache: pda::oracle_cache(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::CrankPrice {},
        read_only(mv),
    )
}

/// `ReadSet ++ TradeExtras ++ Extras` of a trading exit: the ReadSet writable per the venue's
/// trade marks ([`venue::read_writable`]).
fn trade_remaining(mv: &MarketView, extras: &ExtrasChoice) -> anyhow::Result<Vec<AccountMeta>> {
    let mut v = venue::read_metas(mv.kind, &mv.read_keys, true)?;
    v.extend(venue::trade_extras(mv.kind, &mv.market)?);
    v.extend(extras.metas.iter().cloned());
    Ok(v)
}

/// `flag_venue_upgraded(venue)` (ARCH §3.1, §3.9; A7): anyone records a venue program upgrade
/// in the `VenueGuard` latch; `venue` is a `VenueGuard` index, and the ProgramData account the
/// compile-time constant for it.
pub fn flag_venue_upgraded(venue: u8) -> anyhow::Result<Instruction> {
    let program_data = shortwin::constants::venue_program_data(venue)
        .ok_or_else(|| anyhow::anyhow!("venue index {venue} out of range"))?;
    Ok(build(
        shortwin::accounts::FlagVenueUpgraded {
            venue_guard: pda::venue_guard(),
            program_data,
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::FlagVenueUpgraded { venue },
        Vec::new(),
    ))
}

fn read_only_keys(keys: &[Pubkey]) -> Vec<AccountMeta> {
    keys.iter()
        .map(|k| AccountMeta::new_readonly(*k, false))
        .collect()
}

/// `propose_pool(candidate_kind)` on `market` (ARCH §2.6, §3.9; M4): no signer. Remaining, all
/// read-only: `prefix` — a list-only market's detection set `[bonding_curve, canonical_pool]`
/// ([`detection_set`]) or a Raydium market's own ReadSet — then the candidate's ReadSet in its
/// adapter's order (AMM v4 `[pool, coin vault, pc vault, LP mint]`, CPMM `[pool, amm_config,
/// token 0 vault, token 1 vault, LP mint]`). `candidate_kind` is 4 (AMM v4) or 5 (CPMM).
pub fn propose_pool(
    market_key: &Pubkey,
    mint: &Pubkey,
    prefix: &[Pubkey],
    candidate_kind: VenueKind,
    candidate: &[Pubkey],
) -> Instruction {
    let mut rest = read_only_keys(prefix);
    rest.extend(read_only_keys(candidate));
    build(
        shortwin::accounts::ProposePool {
            config: pda::config(),
            venue_guard: pda::venue_guard(),
            market: *market_key,
            mint: *mint,
            oracle_cache: pda::oracle_cache(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::ProposePool {
            candidate_kind: candidate_kind as u8,
        },
        rest,
    )
}

/// `execute_pool_switch()` on `mv`'s market (collection windows only; ARCH §2.6 rule 3, §3.9):
/// the market's ReadSet, then the collection window's candidate —
/// `fp_keys[..read_len(fp_venue)]` ([`crate::pool_monitor::window_candidate`]) — all read-only.
pub fn execute_pool_switch(mv: &MarketView, candidate: &[Pubkey]) -> Instruction {
    execute_pool_switch_with(&mv.key, &mv.market.mint, &mv.read_keys, candidate)
}

/// `execute_pool_switch()` on `market_key` with the incumbent's accounts `prefix` — a Raydium
/// market's ReadSet, or a list-only market's detection set (a window of a list-only market with
/// positions, M4-SEC-4) — then the window candidate's ReadSet, all read-only.
pub fn execute_pool_switch_with(
    market_key: &Pubkey,
    mint: &Pubkey,
    prefix: &[Pubkey],
    candidate: &[Pubkey],
) -> Instruction {
    let mut rest = read_only_keys(prefix);
    rest.extend(read_only_keys(candidate));
    build(
        shortwin::accounts::ExecutePoolSwitch {
            config: pda::config(),
            venue_guard: pda::venue_guard(),
            market: *market_key,
            mint: *mint,
            oracle_cache: pda::oracle_cache(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::ExecutePoolSwitch {},
        rest,
    )
}

/// A list-only market's detection set `[bonding_curve, canonical_pool]` (the market's stored
/// PDAs; `propose_pool` key-checks them and runs `classify_pump` on them, D1).
pub fn detection_set(market: &Market) -> [Pubkey; 2] {
    [market.bonding_curve, market.canonical_pool]
}

/// `liquidate(extras)` of `pos` by `keeper` (ARCH §3.6).
pub fn liquidate(
    keeper: &Pubkey,
    mv: &MarketView,
    pos: &PositionView,
    extras: &ExtrasChoice,
) -> anyhow::Result<Instruction> {
    let m = &mv.market;
    Ok(build(
        shortwin::accounts::Liquidate {
            keeper: *keeper,
            borrower: pos.pos.borrower,
            config: pda::config(),
            market: mv.key,
            position: pos.key,
            exposure: pda::exposure(&mv.key, &pos.pos.borrower),
            mint: m.mint,
            pool_vault: m.pool_vault,
            market_vault: m.market_vault,
            trader: m.trader,
            token_program: m.token_program,
            system_program: anchor_lang::system_program::ID,
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::Liquidate { extras: extras.sel },
        trade_remaining(mv, extras)?,
    ))
}

/// `expire(extras)` of `pos` by `keeper` (ARCH §3.7).
pub fn expire(
    keeper: &Pubkey,
    mv: &MarketView,
    pos: &PositionView,
    extras: &ExtrasChoice,
) -> anyhow::Result<Instruction> {
    let m = &mv.market;
    Ok(build(
        shortwin::accounts::Expire {
            keeper: *keeper,
            borrower: pos.pos.borrower,
            config: pda::config(),
            market: mv.key,
            position: pos.key,
            exposure: pda::exposure(&mv.key, &pos.pos.borrower),
            mint: m.mint,
            pool_vault: m.pool_vault,
            market_vault: m.market_vault,
            trader: m.trader,
            token_program: m.token_program,
            system_program: anchor_lang::system_program::ID,
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::Expire { extras: extras.sel },
        trade_remaining(mv, extras)?,
    ))
}

/// `sol_settle()` of `pos` by `keeper` (ARCH §3.8): ReadSet only; the `ShareEpoch` record
/// of the market's current epoch (created only if this settlement rolls it).
pub fn sol_settle(keeper: &Pubkey, mv: &MarketView, pos: &PositionView) -> Instruction {
    let m = &mv.market;
    build(
        shortwin::accounts::SolSettle {
            keeper: *keeper,
            config: pda::config(),
            market: mv.key,
            position: pos.key,
            borrower: pos.pos.borrower,
            exposure: pda::exposure(&mv.key, &pos.pos.borrower),
            market_vault: m.market_vault,
            mint: m.mint,
            share_epoch_rec: pda::share_epoch(&mv.key, m.share_epoch),
            system_program: anchor_lang::system_program::ID,
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::SolSettle {},
        read_only(mv),
    )
}
