use crate::errors::SimpleError;
use anchor_lang::prelude::*;

pub const STATE_VERSION: u16 = 1;
pub const CONFIG_SEED: &[u8] = b"config";
pub const POLICY_SEED: &[u8] = b"policy";
pub const MARKET_SEED: &[u8] = b"market";
pub const POOL_SEED: &[u8] = b"pool";
pub const TOKEN_VAULT_SEED: &[u8] = b"token_vault";
pub const LENDER_SEED: &[u8] = b"lender";
pub const LOAN_SEED: &[u8] = b"loan";
pub const EXPOSURE_SEED: &[u8] = b"exposure";
pub const REFERENCE_SEED: &[u8] = b"reference";
pub const MAX_TERM_SECONDS: u32 = 72 * 60 * 60;
pub const DEFAULT_GRACE_SECONDS: u32 = 24 * 60 * 60;
pub const MAX_MARKET_NOTIONAL: u64 = 25_000_000_000;
pub const WSOL_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

/// Economics are fixed per policy version. Reporter authorization is separate in Config.
#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinancialPolicy {
    pub term_seconds: u32,
    pub default_grace_seconds: u32,
    pub term_fee_bps: u16,
    pub protocol_share_bps: u16,
    pub cr_open_bps: u32,
    pub cr_liq_bps: u32,
    pub min_notional_lamports: u64,
    pub position_notional_cap_lamports: u64,
    pub market_notional_cap_lamports: u64,
    pub utilization_cap_bps: u16,
    pub wallet_notional_cap_bps: u16,
    pub keeper_fixed_lamports: u64,
    pub keeper_bps: u16,
    pub keeper_cap_lamports: u64,
    pub buyback_premium_bps: u16,
    pub reference_max_age_seconds: u32,
    pub reference_max_age_slots: u64,
    pub history_epoch_seconds: u32,
    pub minimum_history_epochs: u8,
    pub max_spread_bps: u16,
    pub cr_confirmation_delay_seconds: u32,
    pub cr_confirmation_delay_slots: u64,
    pub price_policy_hash: [u8; 32],
}
impl FinancialPolicy {
    pub fn validate(&self) -> Result<()> {
        require!(
            self.term_seconds > 0
                && self.term_seconds <= MAX_TERM_SECONDS
                && self.default_grace_seconds == DEFAULT_GRACE_SECONDS
                && self.term_fee_bps <= 10_000
                && self.protocol_share_bps <= 10_000
                && self.cr_liq_bps > 10_000
                && self.cr_open_bps > self.cr_liq_bps
                && self.cr_open_bps <= 1_000_000
                && self.min_notional_lamports > 0
                && self.position_notional_cap_lamports >= self.min_notional_lamports
                && self.market_notional_cap_lamports >= self.position_notional_cap_lamports
                && self.market_notional_cap_lamports <= MAX_MARKET_NOTIONAL
                && self.utilization_cap_bps > 0
                && self.utilization_cap_bps < 10_000
                && self.wallet_notional_cap_bps > 0
                && self.wallet_notional_cap_bps <= 10_000
                && self.keeper_bps <= 10_000
                && self.buyback_premium_bps <= 10_000
                && self.reference_max_age_seconds > 0
                && self.reference_max_age_seconds <= 60
                && self.reference_max_age_slots > 0
                && self.reference_max_age_slots <= 1_000
                && self.history_epoch_seconds > 0
                && self.history_epoch_seconds <= 300
                && (2..=8).contains(&self.minimum_history_epochs)
                && self.max_spread_bps <= 10_000
                && self.cr_confirmation_delay_seconds > 0
                && self.cr_confirmation_delay_seconds <= 60
                && self.cr_confirmation_delay_slots > 0
                && self.cr_confirmation_delay_slots <= 150
                && self.price_policy_hash != [0; 32],
            SimpleError::InvalidPolicy
        );
        Ok(())
    }
}

#[account]
#[derive(InitSpace, Debug)]
pub struct Config {
    pub version: u16,
    pub bump: u8,
    pub governance: Pubkey,
    pub emergency: Pubkey,
    pub reporter: Pubkey,
    pub fee_recipient: Pubkey,
    pub genesis_hash: [u8; 32],
    pub current_policy: Pubkey,
    pub current_policy_version: u64,
    pub reporter_epoch: u64,
    pub opens_paused: bool,
    pub prices_paused: bool,
    pub market_creation_fee: u64,
    /// Governance's reviewed Jupiter upgrade instance, not a fresh hash per trade.
    pub jupiter_deploy_slot: u64,
    pub jupiter_program_data_len: u32,
    pub jupiter_elf_hash: [u8; 32],
}
impl Config {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
    pub fn validate(&self) -> Result<()> {
        require!(
            self.version == STATE_VERSION
                && self.governance != Pubkey::default()
                && self.emergency != Pubkey::default()
                && self.reporter != Pubkey::default()
                && self.fee_recipient != Pubkey::default()
                && self.genesis_hash != [0; 32]
                && self.current_policy != Pubkey::default()
                && self.current_policy_version > 0
                && self.reporter_epoch > 0
                && self.reporter != self.governance
                && self.reporter != self.emergency
                && self.market_creation_fee <= 100_000_000
                && self.jupiter_deploy_slot > 0
                && self.jupiter_program_data_len >= 45
                && self.jupiter_elf_hash != [0; 32],
            SimpleError::InvalidPolicy
        );
        Ok(())
    }
}

#[account]
#[derive(InitSpace, Debug)]
pub struct Policy {
    pub version: u16,
    pub bump: u8,
    pub config: Pubkey,
    pub policy_version: u64,
    pub terms: FinancialPolicy,
}
impl Policy {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
    pub fn validate(&self, config: &Pubkey) -> Result<()> {
        require!(
            self.version == STATE_VERSION && self.config == *config && self.policy_version > 0,
            SimpleError::AccountMismatch
        );
        self.terms.validate()
    }
}

#[account]
#[derive(InitSpace, Debug)]
pub struct Market {
    pub version: u16,
    pub bump: u8,
    pub config: Pubkey,
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub decimals: u8,
    pub active_generation: u64,
    /// Monotonic across generations, so a closed loan address is never reused.
    pub next_loan_nonce: u64,
}
impl Market {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace, Debug)]
pub struct PoolGeneration {
    pub version: u16,
    pub bump: u8,
    pub token_vault_bump: u8,
    pub market: Pubkey,
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub generation: u64,
    pub cash: u64,
    pub principal_out: u64,
    pub shares: u128,
    pub loan_count: u32,
    pub sol_reserve: u64,
    pub sol_index: [u64; 4],
    pub retired: bool,
    pub booked_notional_out: u64,
}
impl PoolGeneration {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
    pub fn core(&self) -> shortwin_pooled_core::Pool {
        shortwin_pooled_core::Pool {
            generation: self.generation,
            cash: self.cash,
            principal_out: self.principal_out,
            shares: self.shares,
            loan_count: self.loan_count,
            sol_reserve: self.sol_reserve,
            sol_index: self.sol_index,
            retired: self.retired,
        }
    }
    pub fn store(&mut self, pool: &shortwin_pooled_core::Pool) -> Result<()> {
        require_eq!(
            self.generation,
            pool.generation,
            SimpleError::AccountMismatch
        );
        self.cash = pool.cash;
        self.principal_out = pool.principal_out;
        self.shares = pool.shares;
        self.loan_count = pool.loan_count;
        self.sol_reserve = pool.sol_reserve;
        self.sol_index = pool.sol_index;
        self.retired = pool.retired;
        Ok(())
    }
}

#[account]
#[derive(InitSpace, Debug)]
pub struct LenderReceipt {
    pub version: u16,
    pub bump: u8,
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub generation: u64,
    pub shares: u128,
    pub checkpoint: [u64; 4],
    pub fraction: u128,
    pub pending_sol: u64,
}
impl LenderReceipt {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
    pub fn core(&self) -> shortwin_pooled_core::Lender {
        shortwin_pooled_core::Lender {
            generation: self.generation,
            shares: self.shares,
            checkpoint: self.checkpoint,
            fraction: self.fraction,
            pending_sol: self.pending_sol,
        }
    }
    pub fn store(&mut self, lender: &shortwin_pooled_core::Lender) -> Result<()> {
        require_eq!(
            self.generation,
            lender.generation,
            SimpleError::AccountMismatch
        );
        self.shares = lender.shares;
        self.checkpoint = lender.checkpoint;
        self.fraction = lender.fraction;
        self.pending_sol = lender.pending_sol;
        Ok(())
    }
}

#[account]
#[derive(InitSpace, Debug)]
pub struct Loan {
    pub version: u16,
    pub bump: u8,
    pub market: Pubkey,
    pub pool: Pubkey,
    pub borrower: Pubkey,
    pub policy: Pubkey,
    pub nonce: u64,
    pub q: u64,
    pub sale_proceeds: u64,
    pub margin: u64,
    pub collateral: u64,
    pub opening_notional: u64,
    pub term_fee: u64,
    pub lender_fee: u64,
    pub protocol_fee: u64,
    pub opened_at: i64,
    pub deadline: i64,
    pub default_at: i64,
    pub cr_arm_timestamp: i64,
    pub cr_arm_slot: u64,
    pub cr_arm_sequence: u64,
    pub cr_arm_reporter_epoch: u64,
}
impl Loan {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace, Debug)]
pub struct Exposure {
    pub version: u16,
    pub bump: u8,
    pub pool: Pubkey,
    pub borrower: Pubkey,
    pub q: u64,
    pub notional: u64,
}
impl Exposure {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

/// Trusted compact summaries of private history; no independent history proof is claimed.
#[account]
#[derive(InitSpace, Debug)]
pub struct PriceReference {
    pub version: u16,
    pub bump: u8,
    pub market: Pubkey,
    pub policy: Pubkey,
    pub quantity: u64,
    pub reporter_epoch: u64,
    pub sequence: u64,
    pub finite: bool,
    pub source_slot: u64,
    pub source_ts: i64,
    pub live_bid_lower: u64,
    pub live_ask_upper: u64,
    pub history_ask_upper_max: u64,
    pub history_bid_lower_max: u64,
    pub acquisition_reference: u64,
    pub completed_epochs: u8,
    pub history_start_ts: i64,
    pub history_end_ts: i64,
    pub evidence_hash: [u8; 32],
}
impl PriceReference {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}
