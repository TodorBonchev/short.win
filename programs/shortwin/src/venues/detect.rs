//! Pump detection (ARCH §2.5 `classify_pump`; D1; C0-15).
//!
//! A pump coin with a live bonding curve, a completed curve that is still migrating, or a valid
//! canonical PumpSwap pool trades on the pump venues, even if a deeper Raydium pool exists (D1
//! precedence, ARCH §2.5): `create_market` rejects a Raydium hint for such a mint
//! (`PumpVenuePrecedence`). A curve that completed with zeroed reserves and no canonical pool —
//! a coin that graduated to Raydium before PumpSwap existed, e.g. Fartcoin (ARCH `[V3]`) — is
//! `GraduatedElsewhere` and maps to its Raydium pool.
//!
//! ```text
//! classify_pump(bc, cp):   // both keys derived on-chain and key-checked by the caller (C0-15)
//!   if bc.owner == pump && bc.disc == BondingCurve:
//!       require !curve.is_mayhem_mode && curve.quote_mint == default   (ADOPTED Q6)
//!       require bc.data_len ≥ 115                                        (else CurveNeedsExtend)
//!       if !curve.complete                 -> PumpCurve
//!       elif canonical_pool_valid(cp)      -> PumpSwap
//!       elif curve.virtual_token_reserves > 0 -> PumpMigrating
//!       else                               -> GraduatedElsewhere
//!   else                                   -> NotPump
//! canonical_pool_valid(cp) := cp.owner == pAMM && disc == Pool && pool.index == 0
//!     && pool.creator == PDA(pump, ["pool-authority", mint]) && pool.base_mint == mint
//!     && pool.quote_mint == WSOL && !pool.is_mayhem_mode
//! ```
//!
//! **M3 scope.** Only the Raydium hints run it (the precedence check). The `Auto` listing and
//! `crank_price`'s re-detection of a `None` market adopt the pump venues with their adapters
//! (M5 / M6); until then `Auto` lists as `None`, as in M1. A pump-owned account at the curve
//! address that carries the discriminator but does not parse cannot be classified, so it is
//! refused as `PumpVenuePrecedence` (a Raydium listing needs the proof that no pump venue
//! applies).

use anchor_lang::prelude::*;
use shortwin_venue_state::{pump, pump_amm};

use crate::constants::{PUMP_AMM_PROGRAM_ID, PUMP_PROGRAM_ID, WSOL_MINT};
use crate::errors::ShortwinError;
use crate::instructions::create_market::PUMP_POOL_AUTHORITY_SEED;

/// What `classify_pump` found (ARCH §2.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PumpClass {
    /// No pump bonding curve for the mint.
    NotPump,
    /// A live curve.
    PumpCurve,
    /// A completed curve whose canonical pool does not exist yet (`vT > 0`).
    PumpMigrating,
    /// A completed curve with a valid canonical PumpSwap pool.
    PumpSwap,
    /// A completed curve with zeroed reserves and no canonical pool.
    GraduatedElsewhere,
}

impl PumpClass {
    /// The pump venues take precedence (D1): a Raydium listing is refused.
    pub const fn pump_venue(self) -> bool {
        matches!(self, Self::PumpCurve | Self::PumpMigrating | Self::PumpSwap)
    }
}

/// `canonical_pool_valid(cp)` (module docs): `cp` is already key-checked as the mint's
/// canonical pool address.
pub fn canonical_pool_valid(cp: &AccountInfo, mint: &Pubkey) -> bool {
    if *cp.owner != PUMP_AMM_PROGRAM_ID {
        return false;
    }
    let Ok(data) = cp.try_borrow_data() else {
        return false;
    };
    let Ok(pool) = pump_amm::read_pool(&cp.owner.to_bytes(), &data) else {
        return false;
    };
    let authority =
        Pubkey::find_program_address(&[PUMP_POOL_AUTHORITY_SEED, mint.as_ref()], &PUMP_PROGRAM_ID)
            .0;
    pool.index == pump_amm::CANONICAL_POOL_INDEX
        && pool.creator == authority.to_bytes()
        && pool.base_mint == mint.to_bytes()
        && pool.quote_mint == WSOL_MINT.to_bytes()
        && !pool.is_mayhem_mode
}

/// `classify_pump(bc, cp)` for `mint` (module docs). `bc` and `cp` must already be key-checked
/// against the mint's curve and canonical-pool PDAs.
///
/// Errors: `CurveMayhemUnsupported`, `CurveQuoteNotSol` (ADOPTED Q6), `CurveNeedsExtend` (a
/// curve shorter than 115 bytes), `PumpVenuePrecedence` (a pump-owned curve account that does
/// not parse).
pub fn classify_pump(bc: &AccountInfo, cp: &AccountInfo, mint: &Pubkey) -> Result<PumpClass> {
    if *bc.owner != PUMP_PROGRAM_ID {
        return Ok(PumpClass::NotPump);
    }
    let data = bc.try_borrow_data()?;
    if data.get(..8) != Some(&pump::BONDING_CURVE_DISCRIMINATOR[..]) {
        return Ok(PumpClass::NotPump);
    }
    let curve = pump::read_curve(&bc.owner.to_bytes(), &data)
        .map_err(|_| error!(ShortwinError::PumpVenuePrecedence))?;
    require!(!curve.is_mayhem_mode, ShortwinError::CurveMayhemUnsupported);
    require!(curve.quote_mint == [0; 32], ShortwinError::CurveQuoteNotSol);
    require!(
        data.len() >= pump::CURVE_MIN_LEN,
        ShortwinError::CurveNeedsExtend
    );
    Ok(if !curve.complete {
        PumpClass::PumpCurve
    } else if canonical_pool_valid(cp, mint) {
        PumpClass::PumpSwap
    } else if curve.virtual_token_reserves > 0 {
        PumpClass::PumpMigrating
    } else {
        PumpClass::GraduatedElsewhere
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn curve(complete: bool, vt: u64, mayhem: bool, quote: [u8; 32], len: usize) -> Vec<u8> {
        let mut d = vec![0u8; len.max(pump::CURVE_MIN_LEN)];
        d[..8].copy_from_slice(&pump::BONDING_CURVE_DISCRIMINATOR);
        d[pump::OFF_VIRTUAL_TOKEN_RESERVES..pump::OFF_VIRTUAL_TOKEN_RESERVES + 8]
            .copy_from_slice(&vt.to_le_bytes());
        d[pump::OFF_COMPLETE] = u8::from(complete);
        d[pump::OFF_IS_MAYHEM_MODE] = u8::from(mayhem);
        d[pump::OFF_QUOTE_MINT..pump::OFF_QUOTE_MINT + 32].copy_from_slice(&quote);
        d.truncate(len);
        d
    }

    fn classify(bc_owner: Pubkey, bc: Vec<u8>) -> Result<PumpClass> {
        let mint = Pubkey::new_unique();
        let (bk, ck) = (Pubkey::new_unique(), Pubkey::new_unique());
        let (mut bl, mut cl) = (1u64, 0u64);
        let mut bd = bc;
        let mut cd: Vec<u8> = Vec::new();
        let system = Pubkey::default();
        let b = AccountInfo::new(&bk, false, false, &mut bl, &mut bd, &bc_owner, false);
        let c = AccountInfo::new(&ck, false, false, &mut cl, &mut cd, &system, false);
        classify_pump(&b, &c, &mint)
    }

    fn code(r: Result<PumpClass>) -> Option<u32> {
        match r {
            Err(anchor_lang::error::Error::AnchorError(e)) => Some(e.error_code_number),
            _ => None,
        }
    }

    #[test]
    fn classifies_curves() {
        let none = [0u8; 32];
        assert_eq!(
            classify(PUMP_PROGRAM_ID, curve(false, 1, false, none, 151)).unwrap(),
            PumpClass::PumpCurve
        );
        assert_eq!(
            classify(
                PUMP_PROGRAM_ID,
                curve(true, 279_900_000_000_000, false, none, 151)
            )
            .unwrap(),
            PumpClass::PumpMigrating
        );
        assert_eq!(
            classify(PUMP_PROGRAM_ID, curve(true, 0, false, none, 150)).unwrap(),
            PumpClass::GraduatedElsewhere
        );
        assert!(PumpClass::PumpCurve.pump_venue() && !PumpClass::GraduatedElsewhere.pump_venue());
        // Not pump's, or not a curve: not a pump coin.
        assert_eq!(
            classify(Pubkey::new_unique(), curve(false, 1, false, none, 151)).unwrap(),
            PumpClass::NotPump
        );
        let mut other = curve(false, 1, false, none, 151);
        other[0] ^= 1;
        assert_eq!(
            classify(PUMP_PROGRAM_ID, other).unwrap(),
            PumpClass::NotPump
        );
        // ADOPTED Q6, the length rule, an unparseable curve.
        assert_eq!(
            code(classify(PUMP_PROGRAM_ID, curve(true, 0, true, none, 151))),
            Some(u32::from(ShortwinError::CurveMayhemUnsupported))
        );
        assert_eq!(
            code(classify(
                PUMP_PROGRAM_ID,
                curve(true, 0, false, [7; 32], 151)
            )),
            Some(u32::from(ShortwinError::CurveQuoteNotSol))
        );
        assert_eq!(
            code(classify(PUMP_PROGRAM_ID, curve(true, 0, false, none, 114))),
            Some(u32::from(ShortwinError::CurveNeedsExtend))
        );
        let mut bad = curve(true, 0, false, none, 151);
        bad[pump::OFF_COMPLETE] = 2;
        assert_eq!(
            code(classify(PUMP_PROGRAM_ID, bad)),
            Some(u32::from(ShortwinError::PumpVenuePrecedence))
        );
    }
}
