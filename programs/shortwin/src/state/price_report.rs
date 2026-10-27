//! Immutable exact-q report persistence. This account grants no financial action authority.
use super::Market;
use crate::constants::{TOKEN_PROGRAM_ID, WSOL_MINT};
use crate::errors::ShortwinError;
use anchor_lang::prelude::*;
use shortwin_price_report as core;

pub const PRICE_REPORT_VERSION: u16 = 1;

#[account]
#[derive(InitSpace)]
pub struct PriceReport {
    pub version: u16,
    pub bump: u8,
    pub market: Pubkey,
    pub quantity: u64,
    pub model_epoch: u64,
    pub state_bytes: [u8; 1488],
}
impl PriceReport {
    pub const SEED: &'static [u8] = b"price_report";
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
    pub fn restore(
        &self,
        config: &Pubkey,
        market_key: &Pubkey,
        market: &Market,
    ) -> Result<core::ReportState> {
        require_eq!(
            self.version,
            PRICE_REPORT_VERSION,
            ShortwinError::ConfigVersion
        );
        let state = core::codec::decode(&self.state_bytes).map_err(core_error)?;
        let c = state.configuration();
        let d = c.domain();
        require!(
            self.market == *market_key
                && self.quantity == c.quantity()
                && self.model_epoch == c.model().epoch
                && d.program == crate::ID.to_bytes()
                && d.config == config.to_bytes()
                && d.market == market_key.to_bytes()
                && d.base_mint == market.mint.to_bytes()
                && d.base_token_program == market.token_program.to_bytes()
                && d.base_decimals == market.decimals
                && d.quote_mint == WSOL_MINT.to_bytes()
                && d.quote_token_program == TOKEN_PROGRAM_ID.to_bytes()
                && d.quote_decimals == 9,
            ShortwinError::AccountMismatch
        );
        Ok(state)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct PriceReportModelArgs {
    pub version: u32,
    pub epoch: u64,
    pub policy_hash: [u8; 32],
}
impl PriceReportModelArgs {
    pub fn core(self) -> core::ModelBinding {
        core::ModelBinding {
            version: self.version,
            epoch: self.epoch,
            policy_hash: self.policy_hash,
        }
    }
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct PriceReportPolicyArgs {
    pub max_age_slots: u64,
    pub max_age_seconds: u64,
    pub max_window_slots: u64,
    pub max_window_seconds: u64,
    pub max_update_gap_slots: u64,
    pub max_update_gap_seconds: u64,
    pub epoch_seconds: u64,
    pub minimum_completed_epochs: u8,
}
impl PriceReportPolicyArgs {
    pub fn core(self) -> core::HistoryPolicy {
        core::HistoryPolicy {
            max_age_slots: self.max_age_slots,
            max_age_seconds: self.max_age_seconds,
            max_window_slots: self.max_window_slots,
            max_window_seconds: self.max_window_seconds,
            max_update_gap_slots: self.max_update_gap_slots,
            max_update_gap_seconds: self.max_update_gap_seconds,
            epoch_seconds: self.epoch_seconds,
            minimum_completed_epochs: self.minimum_completed_epochs,
        }
    }
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct InitializePriceReportArgs {
    pub genesis_hash: [u8; 32],
    pub quantity: u64,
    pub model: PriceReportModelArgs,
    pub policy: PriceReportPolicyArgs,
    pub publisher_count: u8,
    pub publishers: [Pubkey; 8],
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct PriceReportDomainArgs {
    pub genesis_hash: [u8; 32],
    pub program: Pubkey,
    pub config: Pubkey,
    pub market: Pubkey,
    pub base_mint: Pubkey,
    pub base_token_program: Pubkey,
    pub quote_mint: Pubkey,
    pub quote_token_program: Pubkey,
    pub base_decimals: u8,
    pub quote_decimals: u8,
}
impl PriceReportDomainArgs {
    pub fn core(self) -> core::Domain {
        core::Domain {
            genesis_hash: self.genesis_hash,
            program: self.program.to_bytes(),
            config: self.config.to_bytes(),
            market: self.market.to_bytes(),
            base_mint: self.base_mint.to_bytes(),
            base_token_program: self.base_token_program.to_bytes(),
            quote_mint: self.quote_mint.to_bytes(),
            quote_token_program: self.quote_token_program.to_bytes(),
            base_decimals: self.base_decimals,
            quote_decimals: self.quote_decimals,
        }
    }
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub enum PriceReportStatusArgs {
    Unknown,
    Finite {
        lower: u64,
        central: u64,
        upper: u64,
    },
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct PublishPriceReportArgs {
    pub version: u16,
    pub domain: PriceReportDomainArgs,
    pub quantity: u64,
    pub model: PriceReportModelArgs,
    pub configuration_epoch: u64,
    pub publisher: Pubkey,
    pub sequence: u64,
    pub start_slot: u64,
    pub start_ts: i64,
    pub end_slot: u64,
    pub end_ts: i64,
    pub observation_digest: [u8; 32],
    pub status: PriceReportStatusArgs,
}
impl PublishPriceReportArgs {
    pub fn core(self) -> Result<core::Report> {
        let status = match self.status {
            PriceReportStatusArgs::Unknown => core::ReportStatus::Unknown,
            PriceReportStatusArgs::Finite {
                lower,
                central,
                upper,
            } => core::ReportStatus::Finite(
                core::CostMarks::new(lower, central, upper)
                    .map_err(|_| error!(ShortwinError::InvalidParams))?,
            ),
        };
        Ok(core::Report {
            version: self.version,
            domain: self.domain.core(),
            quantity: self.quantity,
            model: self.model.core(),
            configuration_epoch: self.configuration_epoch,
            publisher: self.publisher.to_bytes(),
            sequence: self.sequence,
            window: core::ObservationWindow {
                start: core::RuntimeClock {
                    slot: self.start_slot,
                    unix_timestamp: self.start_ts,
                },
                end: core::RuntimeClock {
                    slot: self.end_slot,
                    unix_timestamp: self.end_ts,
                },
            },
            observation_digest: self.observation_digest,
            status,
        })
    }
}
pub fn core_error(error: core::codec::CodecError) -> anchor_lang::error::Error {
    msg!("Invalid report persistence: {:?}", error);
    error!(ShortwinError::AccountMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_abi_is_fixed_and_small_enough_for_anchor_initialization() {
        assert_eq!(core::codec::STATE_BYTES, 1488);
        assert_eq!(PriceReport::SPACE, 1547);
    }
}
