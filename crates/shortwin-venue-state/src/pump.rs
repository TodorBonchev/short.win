//! pump.fun `BondingCurve` (ARCH §2.5 `classify_pump`, §4.1; R-PUMP §2.1).
//!
//! **M3 scope: detection only.** A Raydium listing (`create_market(venue_hint = RaydiumV4 |
//! RaydiumCpmm)`) must honour D1's precedence: a pump coin with a live curve, a curve that
//! completed and is migrating, or a canonical PumpSwap pool trades on the pump venues, never
//! on Raydium (ARCH §2.5). This reader gives `classify_pump` the curve's fields; the pump
//! curve adapter (M5) prices and trades on the same reader.
//!
//! **Layout** (Anchor, discriminator `17b7f83760d8ac60` = `sha256("account:BondingCurve")[..8]`;
//! 150 or 151 bytes today, shorter for curves written before a field existed, which read as
//! 0 / false / default past their end — this reader zero-pads):
//!
//! | Offset | Field | | Offset | Field |
//! |---|---|---|---|---|
//! | 8 | `virtual_token_reserves` u64 | | 49 | `creator` |
//! | 16 | `virtual_quote_reserves` u64 | | 81 | `is_mayhem_mode` bool |
//! | 24 | `real_token_reserves` u64 | | 82 | `is_cashback_coin` bool |
//! | 32 | `real_quote_reserves` u64 | | 83 | `quote_mint` (default = SOL) |
//! | 40 | `token_total_supply` u64 | | 115 | `creator_fee_bps` u64 |
//! | 48 | `complete` bool | | | |
//!
//! [`CURVE_MIN_LEN`]: ARCH §2.5 requires `data_len ≥ 115` (the trade instructions top shorter
//! curves up; the client prepends pump's `extend_account`, `CurveNeedsExtend`).
//!
//! The owner is supplied by the caller and checked here; the key (the curve PDA of the mint)
//! is the program's check.

use crate::bytes::{has_discriminator, key_at, u8_at, u64_at};
use crate::{Key, ReadError};

/// pump program `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`.
pub const PROGRAM: Key = [
    1, 86, 224, 246, 147, 102, 90, 207, 68, 219, 21, 104, 191, 23, 91, 170, 81, 137, 203, 151, 245,
    210, 255, 59, 101, 93, 43, 182, 253, 109, 24, 176,
];

/// `sha256("account:BondingCurve")[..8]`.
pub const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [0x17, 0xb7, 0xf8, 0x37, 0x60, 0xd8, 0xac, 0x60];

/// ARCH §2.5: a curve shorter than this must be extended before short.win lists it.
pub const CURVE_MIN_LEN: usize = 115;

/// `virtual_token_reserves` (u64).
pub const OFF_VIRTUAL_TOKEN_RESERVES: usize = 8;
/// `virtual_quote_reserves` (u64; formerly `virtual_sol_reserves`).
pub const OFF_VIRTUAL_QUOTE_RESERVES: usize = 16;
/// `real_token_reserves` (u64).
pub const OFF_REAL_TOKEN_RESERVES: usize = 24;
/// `real_quote_reserves` (u64).
pub const OFF_REAL_QUOTE_RESERVES: usize = 32;
/// `token_total_supply` (u64).
pub const OFF_TOKEN_TOTAL_SUPPLY: usize = 40;
/// `complete` (bool).
pub const OFF_COMPLETE: usize = 48;
/// `creator` (key).
pub const OFF_CREATOR: usize = 49;
/// `is_mayhem_mode` (bool).
pub const OFF_IS_MAYHEM_MODE: usize = 81;
/// `is_cashback_coin` (bool).
pub const OFF_IS_CASHBACK_COIN: usize = 82;
/// `quote_mint` (key; the default key means SOL).
pub const OFF_QUOTE_MINT: usize = 83;
/// `creator_fee_bps` (u64).
pub const OFF_CREATOR_FEE_BPS: usize = 115;

/// The fields of a `BondingCurve` short.win reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BondingCurve {
    pub virtual_token_reserves: u64,
    pub virtual_quote_reserves: u64,
    pub real_token_reserves: u64,
    pub real_quote_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
    pub creator: Key,
    pub is_mayhem_mode: bool,
    pub is_cashback_coin: bool,
    /// `Pubkey::default()` for a SOL curve.
    pub quote_mint: Key,
    pub creator_fee_bps: u64,
}

/// An Anchor `bool` byte: 0 or 1, anything else is malformed.
fn bool_at(data: &[u8], offset: usize) -> Result<bool, ReadError> {
    match u8_at(data, offset) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ReadError::Malformed),
    }
}

/// Reads a `BondingCurve`: owner == [`PROGRAM`], discriminator
/// [`BONDING_CURVE_DISCRIMINATOR`], every `bool` 0 or 1; fields past the end of a short
/// account read as zero (the length rule is [`CURVE_MIN_LEN`], the caller's).
pub fn read_curve(owner: &Key, data: &[u8]) -> Result<BondingCurve, ReadError> {
    if *owner != PROGRAM {
        return Err(ReadError::WrongOwner);
    }
    if !has_discriminator(data, &BONDING_CURVE_DISCRIMINATOR) {
        return Err(ReadError::WrongTag);
    }
    Ok(BondingCurve {
        virtual_token_reserves: u64_at(data, OFF_VIRTUAL_TOKEN_RESERVES),
        virtual_quote_reserves: u64_at(data, OFF_VIRTUAL_QUOTE_RESERVES),
        real_token_reserves: u64_at(data, OFF_REAL_TOKEN_RESERVES),
        real_quote_reserves: u64_at(data, OFF_REAL_QUOTE_RESERVES),
        token_total_supply: u64_at(data, OFF_TOKEN_TOTAL_SUPPLY),
        complete: bool_at(data, OFF_COMPLETE)?,
        creator: key_at(data, OFF_CREATOR),
        is_mayhem_mode: bool_at(data, OFF_IS_MAYHEM_MODE)?,
        is_cashback_coin: bool_at(data, OFF_IS_CASHBACK_COIN)?,
        quote_mint: key_at(data, OFF_QUOTE_MINT),
        creator_fee_bps: u64_at(data, OFF_CREATOR_FEE_BPS),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    /// Fartcoin's curve `TBHe5tJnuT4CQbHorJ1uVdfUoaYGPKgfCpiv2jgesVN` as mainnet holds it
    /// (`getAccountInfo`, slot 451,197,110; 150 bytes, owner pump): complete, every reserve 0,
    /// supply 10¹⁵, no creator — a coin that graduated to Raydium before PumpSwap (ARCH §2.5
    /// "GraduatedElsewhere", ARCH `[V3]`).
    pub const FARTCOIN_CURVE_HEX: &str = "17b7f83760d8ac6000000000000000000000000000000000000000000000000000000000000000000080c6a47e8d0300010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

    fn hex(s: &str) -> [u8; 150] {
        let mut out = [0u8; 150];
        for (i, o) in out.iter_mut().enumerate() {
            *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
        }
        out
    }

    #[test]
    fn reads_fartcoins_graduated_curve() {
        assert_eq!(FARTCOIN_CURVE_HEX.len(), 300);
        let data = hex(FARTCOIN_CURVE_HEX);
        let c = read_curve(&PROGRAM, &data).unwrap();
        assert!(c.complete);
        assert_eq!(
            (
                c.virtual_token_reserves,
                c.virtual_quote_reserves,
                c.real_token_reserves,
                c.real_quote_reserves
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(c.token_total_supply, 1_000_000_000_000_000);
        assert!(!c.is_mayhem_mode && !c.is_cashback_coin);
        assert_eq!(c.quote_mint, [0; 32]);
        assert_eq!(c.creator, [0; 32]);
        assert!(data.len() >= CURVE_MIN_LEN);
    }

    #[test]
    fn refusals_and_zero_padding() {
        let data = hex(FARTCOIN_CURVE_HEX);
        assert_eq!(read_curve(&[0; 32], &data), Err(ReadError::WrongOwner));
        let mut bad = data;
        bad[0] ^= 1;
        assert_eq!(read_curve(&PROGRAM, &bad), Err(ReadError::WrongTag));
        let mut bad = data;
        bad[OFF_COMPLETE] = 2;
        assert_eq!(read_curve(&PROGRAM, &bad), Err(ReadError::Malformed));
        let mut bad = data;
        bad[OFF_IS_MAYHEM_MODE] = 7;
        assert_eq!(read_curve(&PROGRAM, &bad), Err(ReadError::Malformed));
        // A 49-byte curve (before `creator` existed) reads its trailing fields as zero.
        let short = read_curve(&PROGRAM, &data[..49]).unwrap();
        assert!(short.complete && !short.is_mayhem_mode);
        assert_eq!(short.quote_mint, [0; 32]);
        assert_eq!(read_curve(&PROGRAM, &data[..7]), Err(ReadError::WrongTag));
    }
}
