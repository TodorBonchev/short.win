//! Lender pool plumbing (ARCH §3.0, §3.3; ECON §9; `sim/pool_model.py` v0.4.1 through
//! `shortwin_math::pool`).
//!
//! The six lender instructions are thin wrappers: they load the math state
//! ([`Market::pool`], [`crate::state::MarketVault::vault`] (boxed, 2 KB),
//! [`LenderPosition::lender`]), call `shortwin_math::pool`, store the result back
//! ([`Market::set_pool`], [`crate::state::MarketVault::set_vault`],
//! [`LenderPosition::set_lender`]) and then move tokens or lamports. This module holds what
//! they share:
//!
//! - the market prologue that `deposit` and `withdraw` run first (ARCH §3.0 steps 1–5, A16) is
//!   [`crate::engine::prologue`]'s;
//! - [`load_records`]: the `ShareEpoch` records a lender is behind on, from
//!   `remaining_accounts`, key-checked against `["share_epoch", market, epoch u32 LE]` with
//!   the record's stored bump (`create_program_address`, no bump search; A39).
//! - [`init_if_new`]: the fields of a `LenderPosition` that `deposit` just created.
//! - [`rewards_required`]: which post-wipe sweeps a deposit into a live wipe record waits for
//!   (R4A-7).
//! - [`pay_sol`], [`check_vault_floor`]: lender SOL out of `MarketVault`, never below its I2
//!   floor.
//!
//! The math crate's functions are atomic (a refusal changes nothing) and the handlers store
//! only after every check passed, so a failing instruction writes nothing even before the
//! runtime reverts it.

use anchor_lang::prelude::*;
use anchor_lang::AccountDeserialize;
use shortwin_math::pool::{Lender, Pool, ShareEpoch as MathShareEpoch, Vault};

use crate::errors::{CheckedExt, ShortwinError};
use crate::lamports;
use crate::state::{LenderPosition, Market, ShareEpoch, VenueKind};

/// The `ShareEpoch` records among the first `limit` of `accounts`, in order, as math records.
///
/// Each must be a short.win `ShareEpoch` of `market` at the address
/// `create_program_address(["share_epoch", market, epoch u32 LE, bump])` it claims, and the
/// records must be consecutive by epoch; otherwise `EpochRecordMismatch`. Which epochs are
/// needed is `pool::touch`'s business: records it does not need are ignored, missing ones
/// give `LenderBehindEpochs`.
pub fn load_records(
    market: &Pubkey,
    accounts: &[AccountInfo],
    limit: usize,
) -> Result<Vec<MathShareEpoch>> {
    let n = accounts.len().min(limit);
    let mut out: Vec<MathShareEpoch> = Vec::with_capacity(n);
    for info in accounts.iter().take(n) {
        require_keys_eq!(*info.owner, crate::ID, ShortwinError::EpochRecordMismatch);
        let rec = {
            let data = info.try_borrow_data()?;
            ShareEpoch::try_deserialize(&mut &data[..])
                .map_err(|_| error!(ShortwinError::EpochRecordMismatch))?
        };
        require_keys_eq!(rec.market, *market, ShortwinError::EpochRecordMismatch);
        let epoch_le = rec.epoch.to_le_bytes();
        let expected = Pubkey::create_program_address(
            &[ShareEpoch::SEED, market.as_ref(), &epoch_le, &[rec.bump]],
            &crate::ID,
        )
        .map_err(|_| error!(ShortwinError::EpochRecordMismatch))?;
        require_keys_eq!(info.key(), expected, ShortwinError::EpochRecordMismatch);
        if let Some(prev) = out.last() {
            require!(
                prev.epoch.checked_add(1) == Some(rec.epoch),
                ShortwinError::EpochRecordMismatch
            );
        }
        out.push(rec.record());
    }
    Ok(out)
}

/// Fills a `LenderPosition` that `deposit` created in this instruction (`init_if_needed`
/// leaves it zeroed, so `owner == Pubkey::default()`): owner, market, bump and the math state
/// of `pool::Lender::new` (snapshots at the pool's current `acc` / `acc_r` / epoch — after the
/// prologue's accrual, as `pool_model` creates a lender after its release — `lock_pi = WAD`,
/// `lock_epoch = NO_LOCK_EPOCH`). Returns whether the position was new.
pub fn init_if_new(
    position: &mut LenderPosition,
    owner: &Pubkey,
    market: &Pubkey,
    bump: u8,
    pool: &Pool,
) -> bool {
    if position.owner != Pubkey::default() {
        return false;
    }
    position.owner = *owner;
    position.market = *market;
    position.bump = bump;
    position.set_lender(&Lender::new(pool));
    true
}

/// Whether a deposit into a live full-wipe record also waits for `wipe_rewards_claimed`
/// (R4A-7; ARCH §3.3 `deposit`): on the pump venues, which pay venue cashback. The trader
/// sweep is always required. ARCH exempts a pump venue that cannot execute; telling that needs
/// the venue's ReadSet, which `deposit` does not carry, so the bit is required on every
/// `PumpCurve`, `PumpMigrating` and `PumpSwap` market.
///
/// **`PumpMigrating` (M6-SEC-5).** A full wipe can settle during the migration pause (S3 is the
/// only settlement there, ARCH §5.3), with the trader's curve-era pump cashback still in its pump
/// UVA; without the requirement a deposit ended that wipe record at once and the cashback went to
/// the next epoch's lenders. `sweep_trader` treats `PumpMigrating` as a curve kind (the 5-account
/// token leg with the pump UVA), so the bit is set there exactly as on the curve — no deadlock.
///
/// **Curve (M5-SEC-1).** ARCH has `claim_venue_rewards` (M6) set the bit; on the curve kinds
/// `sweep_trader` sets it whenever the trader's pump UVA holds no claimable cashback, and pump's
/// permissionless `claim_cashback` bundled before it empties the UVA into the trader
/// (`instructions::sweep_trader` module docs). Neither needs the venue to execute nor a pump
/// CPI, so the requirement can always be met — the M1-FID-12 deadlock (a deposit waiting for
/// ever on a venue that cannot execute) does not arise on the curve.
///
/// **PumpSwap (M6; the M1-FID-12 remainder).** PumpSwap's cashback sits in pAMM's UVA WSOL ATA:
/// `sweep_trader`'s PumpSwap token leg reads it and sets the bit when it is missing or empty (and
/// the trader's pump UVA holds nothing claimable), and pAMM's permissionless `claim_cashback`
/// bundled before it moves any balance into `ATA(trader, WSOL)`, which the sweep closes and
/// credits (`instructions::sweep_trader` module docs). No PumpSwap CPI either, so the requirement
/// can always be met, whether or not the venue executes.
pub fn rewards_required(market: &Market) -> bool {
    matches!(
        market.venue_kind(),
        Some(VenueKind::PumpCurve | VenueKind::PumpMigrating | VenueKind::PumpSwap)
    )
}

/// `lender_sol + protocol_fees` of `vault`: the lamports `MarketVault` must hold above its
/// rent-exempt minimum (I2).
pub fn tracked(vault: &Vault) -> Result<u64> {
    vault.lender_sol.checked_add(vault.protocol_fees).or_math()
}

/// Pays `amount` lamports out of `market_vault` to `to`, keeping the vault at or above
/// `rent_min + tracked_after`, with `tracked_after` = `lender_sol + protocol_fees` **after**
/// the payment was booked ([`tracked`]; I2). The caller must have released its `MarketVault`
/// data borrow.
pub fn pay_sol(
    market_vault: &AccountInfo,
    to: &AccountInfo,
    amount: u64,
    tracked_after: u64,
) -> Result<()> {
    lamports::move_lamports_above(market_vault, to, amount, tracked_after)
}

/// Post-condition (ARCH §3.0 step 10; I2): `market_vault` holds at least `rent_min +
/// tracked` ([`tracked`]). The caller must have released its `MarketVault` data borrow.
pub fn check_vault_floor(market_vault: &AccountInfo, tracked: u64) -> Result<()> {
    let floor = lamports::rent_min(market_vault.data_len())?
        .checked_add(tracked)
        .or_math()?;
    require_gte!(market_vault.lamports(), floor, ShortwinError::MathOverflow);
    Ok(())
}

/// `real` a claim added on top of a touch at the same `now`: the pot draw (the claim's own
/// touch adds nothing then; `pool::touch` is idempotent at a given `now`).
pub fn pot_draw(touched: &Lender, claimed: &Lender) -> u64 {
    claimed.real.saturating_sub(touched.real)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;
    use shortwin_math::num::WAD;

    #[test]
    fn a_new_position_snapshots_the_pool() {
        let mut pos = LenderPosition {
            owner: Pubkey::default(),
            market: Pubkey::default(),
            shares: 0,
            snap: 0,
            pending: 0,
            snap_r: 0,
            real: 0,
            claimed: 0,
            epoch: 0,
            lock: 0,
            lock_pi: 0,
            lock_epoch: 0,
            lock_term: 0,
            lock_until: 0,
            bump: 0,
            _reserved: [0; 32],
        };
        let mut p = Pool::new();
        p.acc = 7;
        p.acc_r = 5;
        p.share_epoch = 3;
        let (owner, market) = (Pubkey::new_unique(), Pubkey::new_unique());
        assert!(init_if_new(&mut pos, &owner, &market, 254, &p));
        assert_eq!((pos.snap, pos.snap_r, pos.epoch), (7, 5, 3));
        assert_eq!(
            (pos.lock_pi, pos.lock_epoch),
            (WAD, shortwin_math::pool::NO_LOCK_EPOCH)
        );
        assert_eq!((pos.owner, pos.market, pos.bump), (owner, market, 254));
        // An existing position is left alone.
        p.acc = 9;
        assert!(!init_if_new(&mut pos, &owner, &market, 254, &p));
        assert_eq!(pos.snap, 7);
    }

    #[test]
    fn only_pump_venues_wait_for_the_reward_claim() {
        let mut m = Market::zeroed();
        for (venue, want) in [
            (VenueKind::None, false),
            (VenueKind::PumpCurve, true),
            // M6-SEC-5: a wipe settled during the migration pause waits for the curve cashback.
            (VenueKind::PumpMigrating, true),
            (VenueKind::PumpSwap, true),
            (VenueKind::RaydiumV4, false),
            (VenueKind::RaydiumCpmm, false),
        ] {
            m.venue = venue as u8;
            assert_eq!(rewards_required(&m), want, "{venue:?}");
        }
    }
}
