//! PumpSwap (pump AMM) `Pool` (ARCH §2.5 `canonical_pool_valid`, §4.1; R-VEN §1.2–§1.3).
//!
//! **M3 scope: detection only.** `classify_pump` (ARCH §2.5) needs to know whether the mint's
//! canonical PumpSwap pool exists and is valid, so that a Raydium listing honours D1's
//! precedence (a pump graduate trades on its canonical pool). The PumpSwap adapter (M6)
//! prices and trades on the same reader.
//!
//! **Layout** (Anchor, discriminator `f19a6d0411b16dbc` = `sha256("account:Pool")[..8]`; 300 or
//! 301 bytes today, older pools 211–261 bytes, whose missing trailing fields read as 0 /
//! false — this reader zero-pads):
//!
//! | Offset | Field | | Offset | Field |
//! |---|---|---|---|---|
//! | 8 | `pool_bump` u8 | | 171 | `pool_quote_token_account` |
//! | 9 | `index` u16 | | 203 | `lp_supply` u64 |
//! | 11 | `creator` | | 211 | `coin_creator` |
//! | 43 | `base_mint` | | 243 | `is_mayhem_mode` bool |
//! | 75 | `quote_mint` | | 244 | `is_cashback_coin` bool |
//! | 107 | `lp_mint` | | 245 | `virtual_quote_reserves` i128 |
//! | 139 | `pool_base_token_account` | | 261 | `creator_fee_bps` u64 |
//!
//! The canonical pool of a mint is `PDA(pAMM, ["pool", 0u16 LE, pool_authority, mint, WSOL])`
//! with `pool_authority = PDA(pump, ["pool-authority", mint])` (R-VEN §1.3): its address alone
//! makes it unspoofable, and `pool.creator == pool_authority` is the "canonical" test.

use crate::bytes::{has_discriminator, i128_at, key_at, u8_at, u16_at, u64_at};
use crate::{Key, ReadError};

/// PumpSwap program `pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA`.
pub const PROGRAM: Key = [
    12, 20, 222, 252, 130, 94, 198, 118, 148, 37, 8, 24, 187, 101, 64, 101, 244, 41, 141, 49, 86,
    213, 113, 180, 212, 248, 9, 12, 24, 233, 168, 99,
];

/// `sha256("account:Pool")[..8]`.
pub const POOL_DISCRIMINATOR: [u8; 8] = [0xf1, 0x9a, 0x6d, 0x04, 0x11, 0xb1, 0x6d, 0xbc];

/// The index of the canonical (migration) pool.
pub const CANONICAL_POOL_INDEX: u16 = 0;

/// `pool_bump` (u8).
pub const OFF_POOL_BUMP: usize = 8;
/// `index` (u16).
pub const OFF_INDEX: usize = 9;
/// `creator` (key).
pub const OFF_CREATOR: usize = 11;
/// `base_mint` (key).
pub const OFF_BASE_MINT: usize = 43;
/// `quote_mint` (key).
pub const OFF_QUOTE_MINT: usize = 75;
/// `lp_mint` (key).
pub const OFF_LP_MINT: usize = 107;
/// `pool_base_token_account` (key).
pub const OFF_POOL_BASE_TOKEN_ACCOUNT: usize = 139;
/// `pool_quote_token_account` (key).
pub const OFF_POOL_QUOTE_TOKEN_ACCOUNT: usize = 171;
/// `lp_supply` (u64).
pub const OFF_LP_SUPPLY: usize = 203;
/// `coin_creator` (key).
pub const OFF_COIN_CREATOR: usize = 211;
/// `is_mayhem_mode` (bool).
pub const OFF_IS_MAYHEM_MODE: usize = 243;
/// `is_cashback_coin` (bool).
pub const OFF_IS_CASHBACK_COIN: usize = 244;
/// `virtual_quote_reserves` (i128).
pub const OFF_VIRTUAL_QUOTE_RESERVES: usize = 245;
/// `creator_fee_bps` (u64).
pub const OFF_CREATOR_FEE_BPS: usize = 261;

/// The fields of a PumpSwap `Pool` short.win reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pool {
    pub pool_bump: u8,
    pub index: u16,
    pub creator: Key,
    pub base_mint: Key,
    pub quote_mint: Key,
    pub lp_mint: Key,
    pub pool_base_token_account: Key,
    pub pool_quote_token_account: Key,
    pub lp_supply: u64,
    pub coin_creator: Key,
    pub is_mayhem_mode: bool,
    pub is_cashback_coin: bool,
    pub virtual_quote_reserves: i128,
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

/// Reads a `Pool`: owner == [`PROGRAM`], discriminator [`POOL_DISCRIMINATOR`], every `bool` 0
/// or 1; fields past the end of a short (older) pool read as zero.
pub fn read_pool(owner: &Key, data: &[u8]) -> Result<Pool, ReadError> {
    if *owner != PROGRAM {
        return Err(ReadError::WrongOwner);
    }
    if !has_discriminator(data, &POOL_DISCRIMINATOR) {
        return Err(ReadError::WrongTag);
    }
    Ok(Pool {
        pool_bump: u8_at(data, OFF_POOL_BUMP),
        index: u16_at(data, OFF_INDEX),
        creator: key_at(data, OFF_CREATOR),
        base_mint: key_at(data, OFF_BASE_MINT),
        quote_mint: key_at(data, OFF_QUOTE_MINT),
        lp_mint: key_at(data, OFF_LP_MINT),
        pool_base_token_account: key_at(data, OFF_POOL_BASE_TOKEN_ACCOUNT),
        pool_quote_token_account: key_at(data, OFF_POOL_QUOTE_TOKEN_ACCOUNT),
        lp_supply: u64_at(data, OFF_LP_SUPPLY),
        coin_creator: key_at(data, OFF_COIN_CREATOR),
        is_mayhem_mode: bool_at(data, OFF_IS_MAYHEM_MODE)?,
        is_cashback_coin: bool_at(data, OFF_IS_CASHBACK_COIN)?,
        virtual_quote_reserves: i128_at(data, OFF_VIRTUAL_QUOTE_RESERVES),
        creator_fee_bps: u64_at(data, OFF_CREATOR_FEE_BPS),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn put(d: &mut [u8], off: usize, bytes: &[u8]) {
        d[off..off + bytes.len()].copy_from_slice(bytes);
    }

    #[test]
    fn reads_the_documented_offsets() {
        let mut d = [0u8; 301];
        put(&mut d, 0, &POOL_DISCRIMINATOR);
        d[OFF_POOL_BUMP] = 254;
        put(&mut d, OFF_INDEX, &7u16.to_le_bytes());
        put(&mut d, OFF_CREATOR, &[1; 32]);
        put(&mut d, OFF_BASE_MINT, &[2; 32]);
        put(&mut d, OFF_QUOTE_MINT, &[3; 32]);
        put(&mut d, OFF_LP_MINT, &[4; 32]);
        put(&mut d, OFF_POOL_BASE_TOKEN_ACCOUNT, &[5; 32]);
        put(&mut d, OFF_POOL_QUOTE_TOKEN_ACCOUNT, &[6; 32]);
        put(&mut d, OFF_LP_SUPPLY, &9u64.to_le_bytes());
        put(&mut d, OFF_COIN_CREATOR, &[7; 32]);
        d[OFF_IS_MAYHEM_MODE] = 1;
        put(
            &mut d,
            OFF_VIRTUAL_QUOTE_RESERVES,
            &17_584_505_333i128.to_le_bytes(),
        );
        put(&mut d, OFF_CREATOR_FEE_BPS, &30u64.to_le_bytes());
        let p = read_pool(&PROGRAM, &d).unwrap();
        assert_eq!((p.pool_bump, p.index), (254, 7));
        assert_eq!(
            (p.creator, p.base_mint, p.quote_mint, p.lp_mint),
            ([1; 32], [2; 32], [3; 32], [4; 32])
        );
        assert_eq!(
            (p.pool_base_token_account, p.pool_quote_token_account),
            ([5; 32], [6; 32])
        );
        assert_eq!((p.lp_supply, p.coin_creator), (9, [7; 32]));
        assert!(p.is_mayhem_mode && !p.is_cashback_coin);
        assert_eq!(p.virtual_quote_reserves, 17_584_505_333);
        assert_eq!(p.creator_fee_bps, 30);
        // An older 211-byte pool reads the appended fields as zero.
        let old = read_pool(&PROGRAM, &d[..211]).unwrap();
        assert!(!old.is_mayhem_mode);
        assert_eq!((old.coin_creator, old.virtual_quote_reserves), ([0; 32], 0));
    }

    #[test]
    fn refusals() {
        let mut d = [0u8; 300];
        put(&mut d, 0, &POOL_DISCRIMINATOR);
        assert!(read_pool(&PROGRAM, &d).is_ok());
        assert_eq!(read_pool(&[9; 32], &d), Err(ReadError::WrongOwner));
        let mut bad = d;
        bad[7] ^= 1;
        assert_eq!(read_pool(&PROGRAM, &bad), Err(ReadError::WrongTag));
        let mut bad = d;
        bad[OFF_IS_CASHBACK_COIN] = 2;
        assert_eq!(read_pool(&PROGRAM, &bad), Err(ReadError::Malformed));
    }
}
