//! `ShareEpoch` (Borsh, seeds `["share_epoch", market, epoch u32 LE]`, 127 B; ARCH §2.3.6, A39).
//!
//! Written by `sol_settle` when it rolls the epoch (M2; payer keeper, grief-proof creation).
//! Immutable once written, except that the record of a **full wipe** is topped up by SOL
//! credited while the market has no shares, until the next deposit finalises it (V2-12).
//! Lender instructions read the records a lender is behind on from `remaining_accounts`,
//! key-checked against [`ShareEpoch::address`].

use anchor_lang::prelude::*;
use shortwin_math::pool::ShareEpoch as MathShareEpoch;

/// A share-epoch record (ARCH §2.3.6).
#[account]
#[derive(InitSpace, Debug, PartialEq, Eq)]
pub struct ShareEpoch {
    pub market: Pubkey,
    /// The epoch this record closes.
    pub epoch: u32,
    /// `acc` at the roll (topped up while it is a live full wipe).
    pub acc_end: i128,
    /// `acc_r` at the roll.
    pub acc_r_end: u128,
    /// Shares at the roll (> 0).
    pub s_old: u128,
    /// Top-up remainder of `acc_end`.
    pub carry: i128,
    /// Top-up remainder of `acc_r_end`.
    pub carry_r: u128,
    /// Conversion numerator: 0 (full wipe) or 1.
    pub num: u8,
    /// `k` of the denominator `1,000^k`.
    pub den_log1000: u8,
    pub bump: u8,
}

impl ShareEpoch {
    /// PDA seed prefix (`["share_epoch", market, epoch u32 LE]`).
    pub const SEED: &'static [u8] = b"share_epoch";
    /// Account size including the discriminator.
    pub const SPACE: usize = 8 + Self::INIT_SPACE;

    /// The record's address and bump (host only: clients and tests; on-chain, handlers
    /// re-derive with the stored bump via `create_program_address`, never a bump search).
    #[cfg(not(target_os = "solana"))]
    pub fn address(market: &Pubkey, epoch: u32) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[Self::SEED, market.as_ref(), &epoch.to_le_bytes()],
            &crate::ID,
        )
    }

    /// The record as `shortwin_math::pool::ShareEpoch`.
    pub fn record(&self) -> MathShareEpoch {
        MathShareEpoch {
            acc_end: self.acc_end,
            acc_r_end: self.acc_r_end,
            s_old: self.s_old,
            carry: self.carry,
            carry_r: self.carry_r,
            epoch: self.epoch,
            num: self.num,
            den_log1000: self.den_log1000,
            _pad: [0; 10],
        }
    }

    /// Stores `shortwin_math::pool::ShareEpoch` back (market and bump are the caller's).
    pub fn set_record(&mut self, r: &MathShareEpoch) {
        self.acc_end = r.acc_end;
        self.acc_r_end = r.acc_r_end;
        self.s_old = r.s_old;
        self.carry = r.carry;
        self.carry_r = r.carry_r;
        self.epoch = r.epoch;
        self.num = r.num;
        self.den_log1000 = r.den_log1000;
    }
}
