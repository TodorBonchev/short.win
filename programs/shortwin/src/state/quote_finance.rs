//! Distinct quote-native bookkeeping sidecars; never reinterpret legacy rent fields.
//!
//! These accounts alone do not enable admission. A market backend transition must
//! first reject every legacy financial mutator and provide all quote-native pool
//! entry/exit paths. No public adoption instruction is supplied by this foundation.
use anchor_lang::prelude::*;
use shortwin_math::quote_rent::{self, persistence, NotionalPosition};

use crate::errors::{MathResultExt, ShortwinError};
use crate::state::Market;

pub const QUOTE_FINANCE_VERSION: u16 = 1;
/// Owner-selected immutable overdue grace for new quote-native debt.
pub const QUOTE_DEFAULT_GRACE_SECONDS: i64 = 86_400;
/// Previously unused flag bit; requires complete dispatch before an initializer sets it.
pub const MARKET_FLAG_QUOTE_NATIVE: u16 = 1 << 9;

#[account]
#[derive(InitSpace)]
pub struct QuoteFinanceMarket {
    pub version: u16,
    pub bump: u8,
    pub market: Pubkey,
    pub config: Pubkey,
    pub model_version: u32,
    pub model_epoch: u64,
    pub policy_hash: [u8; 32],
    pub rent_state: [u8; persistence::MARKET_ENCODED_LEN],
}
impl QuoteFinanceMarket {
    pub const SEED: &'static [u8] = b"quote_finance";
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
    pub fn restore(
        &self,
        config: &Pubkey,
        market_key: &Pubkey,
        market: &Market,
    ) -> Result<persistence::MarketCheckpoint> {
        require_eq!(
            self.version,
            QUOTE_FINANCE_VERSION,
            ShortwinError::ConfigVersion
        );
        require!(
            self.config == *config
                && self.market == *market_key
                && self.model_version > 0
                && self.model_epoch > 0
                && self.policy_hash != [0; 32]
                && market.has_flag(MARKET_FLAG_QUOTE_NATIVE),
            ShortwinError::AccountMismatch
        );
        // Legacy counters are never adopted or reused as notional rent state.
        require!(
            market.rent_aggr.iter().all(|value| value.get() == 0)
                && market.lender_credit_rem.get() == 0,
            ShortwinError::AccountMismatch
        );
        let checkpoint = persistence::decode_market(&self.rent_state)
            .map_err(|_| error!(ShortwinError::AccountMismatch))?;
        let aggregate = checkpoint.aggregate();
        let timestamp = i64::try_from(checkpoint.accrued_through())
            .map_err(|_| error!(ShortwinError::AccountMismatch))?;
        let tier_total = market
            .borrowed_q_by_tier
            .iter()
            .try_fold(0u64, |sum, item| {
                sum.checked_add(*item).ok_or(ShortwinError::MathOverflow)
            })?;
        require!(
            aggregate.positions() == u64::from(market.open_positions)
                && aggregate.opening_notional_total() == u128::from(market.total_notional_open)
                && timestamp == market.last_accrual_ts
                && tier_total == market.borrowed_q
                && (market.open_positions == 0) == (market.borrowed_q == 0)
                && u64::from(market.open_positions) <= market.borrowed_q,
            ShortwinError::AccountMismatch
        );
        Ok(checkpoint)
    }
    pub fn store(&mut self, checkpoint: &persistence::MarketCheckpoint) -> Result<()> {
        persistence::encode_market(checkpoint, &mut self.rent_state)
            .map_err(|_| error!(ShortwinError::AccountMismatch))
    }
}

/// Own escrow/identity. A legacy ShortPosition can never decode as this account or
/// share its PDA namespace; direct repayment cannot accidentally charge legacy rent.
#[account]
#[derive(InitSpace, Debug, PartialEq, Eq)]
pub struct QuoteFinancePosition {
    pub version: u16,
    pub borrower: Pubkey,
    pub market: Pubkey,
    pub finance_market: Pubkey,
    pub nonce: u64,
    pub bump: u8,
    pub tier: u8,
    pub q: u64,
    pub s: u64,
    pub m: u64,
    pub v: u64,
    pub n_open: u64,
    pub fee_open: u64,
    pub t_open: i64,
    pub deadline: i64,
    /// Fixed at open; margin additions never extend it.
    pub default_at: i64,
    pub settle_keeper_fixed: u64,
    pub settle_keeper_bps: u16,
    pub settle_keeper_cap: u64,
    /// Reviewed opening policy, never replaced by a later global config value.
    pub lump_vest_seconds: u32,
    pub model_version: u32,
    pub model_epoch: u64,
    pub policy_hash: [u8; 32],
    pub rent_state: [u8; persistence::POSITION_ENCODED_LEN],
}
impl QuoteFinancePosition {
    pub const SEED: &'static [u8] = b"quote_position";
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
    pub fn is_pda(&self, key: &Pubkey) -> bool {
        Pubkey::create_program_address(
            &[
                Self::SEED,
                self.market.as_ref(),
                self.borrower.as_ref(),
                &self.nonce.to_le_bytes(),
                &[self.bump],
            ],
            &crate::ID,
        )
        .is_ok_and(|pda| pda == *key)
    }
    pub fn restore(
        &self,
        finance_key: &Pubkey,
        finance: &QuoteFinanceMarket,
        checkpoint: &persistence::MarketCheckpoint,
    ) -> Result<NotionalPosition> {
        require_eq!(
            self.version,
            QUOTE_FINANCE_VERSION,
            ShortwinError::ConfigVersion
        );
        require!(
            self.market == finance.market
                && self.finance_market == *finance_key
                && self.borrower != Pubkey::default()
                && self.q > 0
                && self.s > 0
                && self.s.checked_add(self.m) == Some(self.v)
                && self.n_open >= self.s
                && self.tier < 4
                && self.t_open >= 0
                && u64::try_from(self.t_open)
                    .is_ok_and(|opened| opened <= checkpoint.accrued_through())
                && self.deadline >= self.t_open
                && self.deadline.checked_add(QUOTE_DEFAULT_GRACE_SECONDS) == Some(self.default_at)
                && self.settle_keeper_bps <= 10_000
                && self.model_version == finance.model_version
                && self.model_epoch == finance.model_epoch
                && self.policy_hash == finance.policy_hash,
            ShortwinError::AccountMismatch
        );
        let (vesting_minimum, vesting_maximum) = shortwin_math::params::Rule::LumpVestS.bounds();
        require!(
            (vesting_minimum..=vesting_maximum).contains(&u64::from(self.lump_vest_seconds)),
            ShortwinError::InvalidParams
        );
        let position = persistence::decode_position(&self.rent_state)
            .map_err(|_| error!(ShortwinError::AccountMismatch))?;
        require!(
            position.opening_notional().lamports() == self.n_open
                && (checkpoint.aggregate().positions() == 0
                    || checkpoint.aggregate().kink_bps() == Some(position.kink_bps())),
            ShortwinError::AccountMismatch
        );
        position.validate_at_basis(&checkpoint.basis()).math()?;
        Ok(position)
    }
    /// Called only at opening, before the new account is admitted. No update
    /// instruction can shorten the clock or rewrite open debt's reward terms.
    pub fn lock_recovery(&mut self, terms: shortwin_math::risk::SettleKeeperTerms) -> Result<()> {
        require!(terms.bps <= 10_000, ShortwinError::InvalidParams);
        self.default_at = self
            .deadline
            .checked_add(QUOTE_DEFAULT_GRACE_SECONDS)
            .ok_or(ShortwinError::MathOverflow)?;
        self.settle_keeper_fixed = terms.fixed;
        self.settle_keeper_bps =
            u16::try_from(terms.bps).map_err(|_| error!(ShortwinError::InvalidParams))?;
        self.settle_keeper_cap = terms.cap;
        Ok(())
    }
    pub fn lock_vesting(&mut self, lump_vest_seconds: u32) -> Result<()> {
        let (minimum, maximum) = shortwin_math::params::Rule::LumpVestS.bounds();
        require!(
            (minimum..=maximum).contains(&u64::from(lump_vest_seconds)),
            ShortwinError::InvalidParams
        );
        self.lump_vest_seconds = lump_vest_seconds;
        Ok(())
    }
    pub fn default_keeper_terms(&self) -> shortwin_math::risk::SettleKeeperTerms {
        shortwin_math::risk::SettleKeeperTerms {
            fixed: self.settle_keeper_fixed,
            bps: u32::from(self.settle_keeper_bps),
            cap: self.settle_keeper_cap,
        }
    }
    pub fn store_rent(&mut self, position: &quote_rent::NotionalPosition) -> Result<()> {
        require_eq!(
            position.opening_notional().lamports(),
            self.n_open,
            ShortwinError::AccountMismatch
        );
        persistence::encode_position(position, &mut self.rent_state)
            .map_err(|_| error!(ShortwinError::AccountMismatch))
    }
}

#[account]
#[derive(InitSpace, Debug, PartialEq, Eq)]
pub struct QuoteFinanceExposure {
    pub market: Pubkey,
    pub wallet: Pubkey,
    pub q_open: u64,
    pub notional_open: u64,
    pub bump: u8,
}
impl QuoteFinanceExposure {
    pub const SEED: &'static [u8] = b"quote_exposure";
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}
