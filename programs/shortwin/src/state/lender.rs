//! `LenderPosition` (Borsh, seeds `["lender", market, owner]`, 241 B; ARCH §2.3.6).
//!
//! Created by `deposit` (`init_if_needed`, payer owner); mutated by `deposit`, `withdraw`,
//! `claim_sol`, `cover_deficit` and `sync_lender`. [`LenderPosition::lender`] /
//! [`LenderPosition::set_lender`] convert to and from `shortwin_math::pool::Lender`.

use anchor_lang::prelude::*;
use shortwin_math::pool::Lender;

/// A lender's position in one market (ARCH §2.3.6).
#[account]
#[derive(InitSpace, Debug, PartialEq, Eq)]
pub struct LenderPosition {
    pub owner: Pubkey,
    pub market: Pubkey,
    /// Shares, in units of `epoch`.
    pub shares: u128,
    /// `acc` snapshot.
    pub snap: i128,
    /// Entitlement not yet paid (negative after a shortfall debit, ECON §9.4).
    pub pending: i64,
    /// `acc_r` snapshot.
    pub snap_r: u128,
    /// Cumulative realised SOL allocated to this lender, including pot draws (V2-2).
    pub real: u64,
    /// Cumulative SOL paid to this lender (V2-2).
    pub claimed: u64,
    /// Share epoch of `shares`.
    pub epoch: u32,
    /// Locked shares at `lock_pi` (ECON §9.3).
    pub lock: u128,
    /// Π when the lock was set.
    pub lock_pi: u128,
    /// Π epoch of `lock` (`u32::MAX` = no lock).
    pub lock_epoch: u32,
    /// The lock's term in the calendar bucket of `lock_until`.
    pub lock_term: u128,
    /// When the lock expires (a calendar day end; R3B-1, R4A-2).
    pub lock_until: i64,
    pub bump: u8,
    pub _reserved: [u8; 32],
}

impl LenderPosition {
    /// PDA seed prefix (`["lender", market, owner]`).
    pub const SEED: &'static [u8] = b"lender";
    /// Account size including the discriminator.
    pub const SPACE: usize = 8 + Self::INIT_SPACE;

    /// The state as `shortwin_math::pool::Lender`.
    pub fn lender(&self) -> Lender {
        Lender {
            shares: self.shares,
            snap: self.snap,
            snap_r: self.snap_r,
            lock: self.lock,
            lock_pi: self.lock_pi,
            lock_term: self.lock_term,
            pending: self.pending,
            real: self.real,
            claimed: self.claimed,
            lock_until: self.lock_until,
            epoch: self.epoch,
            lock_epoch: self.lock_epoch,
            _pad: [0; 8],
        }
    }

    /// Stores `shortwin_math::pool::Lender` back.
    pub fn set_lender(&mut self, l: &Lender) {
        self.shares = l.shares;
        self.snap = l.snap;
        self.snap_r = l.snap_r;
        self.lock = l.lock;
        self.lock_pi = l.lock_pi;
        self.lock_term = l.lock_term;
        self.pending = l.pending;
        self.real = l.real;
        self.claimed = l.claimed;
        self.lock_until = l.lock_until;
        self.epoch = l.epoch;
        self.lock_epoch = l.lock_epoch;
    }
}
