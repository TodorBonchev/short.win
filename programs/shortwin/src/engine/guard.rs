//! Guards (ARCH §2.1 `engine/guard.rs`): the mint guard of ARCH §3.0 step 3 (M1) and the open
//! guards of ARCH §3.4 steps 1 and 3 (M2: global / market pauses, `paused_coverage`, the
//! route-broken flags, venue enabled, the `VenueGuard` mask, I17). **Exits never call the open
//! guards** (I10): closes, liquidations, expiries and settlements read no pause flag, no
//! route-broken flag, no latch and no oracle.
//!
//! The mint guard is shared by `create_market` / `deposit` / `open_short` (entry paths:
//! `check_mint_safety` must pass, and a market flagged `mint_unsafe` refuses) and by
//! `withdraw` / exits (exit paths: never revert on the filter; the caller emits
//! `MintSafetyTripped` when it reports `false`) [R-MINT §6, §9.4].

use anchor_lang::prelude::*;

use crate::errors::ShortwinError;
use crate::mint_filter::check_mint_safety;
use crate::state::{
    GlobalConfig, Market, VenueGuard, VenueKind, MARKET_FLAG_MINT_UNSAFE,
    MARKET_FLAG_PAUSED_COVERAGE, MARKET_FLAG_PAUSED_OPENS,
};

/// Which side of the mint guard an instruction is on (ARCH §3.0 step 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MintPath {
    /// `create_market`, `deposit`, `open_short`: the filter must pass.
    Entry,
    /// `withdraw`, `close_short`, `liquidate`, `expire`, `sol_settle`: never blocked by the
    /// filter.
    Exit,
}

/// The mint guard: `mint.key == market.mint`, `mint.owner == market.token_program ==
/// token_program.key` (always; a mismatch is `AccountMismatch`), then on an entry path
/// `!mint_unsafe` (`MintUnsafeFlagged`) and `check_mint_safety` (its own error). Returns
/// whether the mint passes the filter (always `true` on an entry path; on an exit path `false`
/// means "emit `MintSafetyTripped` and continue").
pub fn mint_guard(
    market: &Market,
    mint: &AccountInfo,
    token_program: &Pubkey,
    path: MintPath,
) -> Result<bool> {
    require_keys_eq!(mint.key(), market.mint, ShortwinError::AccountMismatch);
    require_keys_eq!(
        *mint.owner,
        market.token_program,
        ShortwinError::AccountMismatch
    );
    require_keys_eq!(
        *token_program,
        market.token_program,
        ShortwinError::AccountMismatch
    );
    match path {
        MintPath::Entry => {
            require!(
                !market.has_flag(MARKET_FLAG_MINT_UNSAFE),
                ShortwinError::MintUnsafeFlagged
            );
            check_mint_safety(mint)?;
            Ok(true)
        }
        MintPath::Exit => Ok(check_mint_safety(mint).is_ok()),
    }
}

/// ARCH §3.4 step 1, the static open guards (in this order): opens paused globally or on the
/// market (`OpensPaused`, SPEC §11); paused for keeper coverage (`CoveragePaused`, A69); the
/// route-broken flag of the market or of its venue (`RouteBroken`, A40); a venue that takes
/// opens (`NoSupportedVenue` for `None`, `VenueMigrating` while `PumpMigrating`) and is enabled
/// (`VenueDisabled`). The mint guard (entry path, `MintUnsafeFlagged` first) is
/// [`mint_guard`]'s.
pub fn open_static(config: &GlobalConfig, market: &Market, kind: VenueKind) -> Result<()> {
    require!(
        !config.paused_opens() && !market.has_flag(MARKET_FLAG_PAUSED_OPENS),
        ShortwinError::OpensPaused
    );
    require!(
        !market.has_flag(MARKET_FLAG_PAUSED_COVERAGE),
        ShortwinError::CoveragePaused
    );
    require!(
        !crate::engine::route::route_broken(config, market, kind),
        ShortwinError::RouteBroken
    );
    match kind {
        VenueKind::None => return err!(ShortwinError::NoSupportedVenue),
        VenueKind::PumpMigrating => return err!(ShortwinError::VenueMigrating),
        _ => {}
    }
    require!(
        kind.venue_bit() != 0 && config.venue_enabled & kind.venue_bit() != 0,
        ShortwinError::VenueDisabled
    );
    Ok(())
}

/// ARCH §3.4 step 3, the venue-upgrade latch (A7; masks v0.4.1, R4B-7; I17): no open while a
/// bit of the venue's mask is set in `VenueGuard.upgraded` (`VenueUpgraded`).
pub fn latch(guard: &VenueGuard, kind: VenueKind) -> Result<()> {
    require!(
        !guard.blocks(kind.guard_mask()),
        ShortwinError::VenueUpgraded
    );
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::constants::TOKEN_PROGRAM_ID;
    use bytemuck::Zeroable;

    fn code(e: &anchor_lang::error::Error) -> Option<u32> {
        match e {
            anchor_lang::error::Error::AnchorError(a) => Some(a.error_code_number),
            anchor_lang::error::Error::ProgramError(_) => None,
        }
    }

    #[test]
    fn keys_first_then_the_path() {
        let mint_key = Pubkey::new_unique();
        let mut market = Market::zeroed();
        market.mint = mint_key;
        market.token_program = TOKEN_PROGRAM_ID;
        let (mut lamports, mut data) = (1u64, [0u8; 82]);
        let mint = AccountInfo::new(
            &mint_key,
            false,
            false,
            &mut lamports,
            &mut data,
            &TOKEN_PROGRAM_ID,
            false,
        );
        let mismatch = u32::from(ShortwinError::AccountMismatch);

        let wrong_program = Pubkey::new_unique();
        let e = mint_guard(&market, &mint, &wrong_program, MintPath::Exit).unwrap_err();
        assert_eq!(code(&e), Some(mismatch));

        let mut other = market;
        other.mint = Pubkey::new_unique();
        let e = mint_guard(&other, &mint, &TOKEN_PROGRAM_ID, MintPath::Exit).unwrap_err();
        assert_eq!(code(&e), Some(mismatch));

        // Exit never reverts on the filter: the zeroed 82-byte mint is uninitialized, so the
        // filter fails (`MintMalformed`) and the guard reports it tripped.
        assert!(matches!(
            mint_guard(&market, &mint, &TOKEN_PROGRAM_ID, MintPath::Exit),
            Ok(false)
        ));

        // Entry refuses a flagged market before running the filter.
        let mut flagged = market;
        flagged.set_flag(MARKET_FLAG_MINT_UNSAFE, true);
        let e = mint_guard(&flagged, &mint, &TOKEN_PROGRAM_ID, MintPath::Entry).unwrap_err();
        assert_eq!(code(&e), Some(u32::from(ShortwinError::MintUnsafeFlagged)));
    }
}
