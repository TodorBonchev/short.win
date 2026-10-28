//! Exact-q on-demand cache whose historical observations are trusted operator assertions.
//! This immutable prototype does not itself admit a market or authorize a financial action.
use super::{Market, PriceReportModelArgs, PriceReportPolicyArgs, PriceReportStatusArgs};
use crate::constants::{TOKEN_PROGRAM_ID, WSOL_MINT};
use crate::errors::ShortwinError;
use anchor_lang::prelude::*;
use shortwin_price_report::{self as core, trusted_snapshot as snapshot};

pub const PRICE_SNAPSHOT_VERSION: u16 = snapshot::TRUSTED_SNAPSHOT_VERSION;
#[account]
#[derive(InitSpace)]
pub struct PriceSnapshot {
    pub version: u16,
    pub bump: u8,
    pub market: Pubkey,
    pub quantity: u64,
    pub model_epoch: u64,
    /// Per-feed observation/search settings commitment, distinct from the global financial model.
    pub producer_commitment: [u8; 32],
    pub state_bytes: [u8; 1533],
}
impl PriceSnapshot {
    pub const SEED: &'static [u8] = b"price_snapshot";
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
    #[inline(never)]
    pub fn restore(
        &self,
        config: &Pubkey,
        market_key: &Pubkey,
        market: &Market,
    ) -> Result<Box<snapshot::SnapshotState>> {
        require_eq!(
            self.version,
            PRICE_SNAPSHOT_VERSION,
            ShortwinError::ConfigVersion
        );
        require!(
            self.producer_commitment != [0; 32],
            ShortwinError::AccountMismatch
        );
        let state =
            Box::new(snapshot::codec::decode(&self.state_bytes).map_err(snapshot_codec_error)?);
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
pub struct InitializePriceSnapshotArgs {
    pub genesis_hash: [u8; 32],
    pub producer_commitment: [u8; 32],
    pub quantity: u64,
    pub model: PriceReportModelArgs,
    pub policy: PriceReportPolicyArgs,
    pub publisher_count: u8,
    pub publishers: [Pubkey; 8],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
pub struct AttestedPriceBucketArgs {
    pub epoch: u64,
    pub first_slot: u64,
    pub first_ts: i64,
    pub last_slot: u64,
    pub last_ts: i64,
    pub lower_min: u64,
    pub central_last: u64,
    pub upper_max: u64,
    pub observations: u64,
    pub largest_gap_slots: u64,
    pub largest_gap_seconds: u64,
}
impl AttestedPriceBucketArgs {
    pub fn core(&self) -> snapshot::AttestedBucket {
        snapshot::AttestedBucket {
            bucket: core::CompletedBucket {
                epoch: self.epoch,
                first_observed: core::RuntimeClock {
                    slot: self.first_slot,
                    unix_timestamp: self.first_ts,
                },
                last_observed: core::RuntimeClock {
                    slot: self.last_slot,
                    unix_timestamp: self.last_ts,
                },
                lower_min: self.lower_min,
                central_last: self.central_last,
                upper_max: self.upper_max,
                observations: self.observations,
            },
            largest_gap_slots: self.largest_gap_slots,
            largest_gap_seconds: self.largest_gap_seconds,
        }
    }
    pub fn is_zero(&self) -> bool {
        self.epoch == 0
            && self.first_slot == 0
            && self.first_ts == 0
            && self.last_slot == 0
            && self.last_ts == 0
            && self.lower_min == 0
            && self.central_last == 0
            && self.upper_max == 0
            && self.observations == 0
            && self.largest_gap_slots == 0
            && self.largest_gap_seconds == 0
    }
}

/// Domain, quantity, model and allowed publisher are bound by the immutable account.
/// Omitting redundant identities keeps the full H8 update within a normal transaction packet.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct PublishPriceSnapshotArgs {
    pub version: u16,
    pub sequence: u64,
    pub start_slot: u64,
    pub start_ts: i64,
    pub end_slot: u64,
    pub end_ts: i64,
    pub observation_digest: [u8; 32],
    pub status: PriceReportStatusArgs,
    pub history_count: u8,
    pub history: [AttestedPriceBucketArgs; 8],
}
impl PublishPriceSnapshotArgs {
    #[inline(never)]
    pub fn core(
        &self,
        state: &snapshot::SnapshotState,
        publisher: Pubkey,
    ) -> Result<Box<snapshot::SnapshotReport>> {
        let count = usize::from(self.history_count);
        require!(
            count <= core::HISTORY_CAPACITY,
            ShortwinError::InvalidParams
        );
        let mut buckets = [None; core::HISTORY_CAPACITY];
        for (index, bucket) in self.history.iter().enumerate() {
            if index < count {
                buckets[index] = Some(bucket.core());
            } else {
                require!(bucket.is_zero(), ShortwinError::InvalidParams);
            }
        }
        let history = snapshot::AttestedHistory::new(buckets)
            .map_err(|_| error!(ShortwinError::InvalidParams))?;
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
        let c = state.configuration();
        Ok(Box::new(snapshot::SnapshotReport {
            version: self.version,
            domain: c.domain(),
            quantity: c.quantity(),
            model: c.model(),
            configuration_epoch: c.epoch(),
            publisher: publisher.to_bytes(),
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
            history,
        }))
    }
}
pub fn snapshot_codec_error(error: core::codec::CodecError) -> anchor_lang::error::Error {
    msg!("Invalid trusted snapshot persistence: {:?}", error);
    error!(ShortwinError::AccountMismatch)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_account_is_versioned_and_update_size_is_bounded() {
        assert_eq!(snapshot::codec::STATE_BYTES, 1533);
        assert_eq!(PriceSnapshot::SPACE, 1624);
        let args = PublishPriceSnapshotArgs {
            version: PRICE_SNAPSHOT_VERSION,
            sequence: 1,
            start_slot: 1,
            start_ts: 1,
            end_slot: 1,
            end_ts: 1,
            observation_digest: [1; 32],
            status: PriceReportStatusArgs::Finite {
                lower: 1,
                central: 2,
                upper: 3,
            },
            history_count: 8,
            history: [AttestedPriceBucketArgs::default(); 8],
        };
        let mut bytes = Vec::new();
        args.serialize(&mut bytes).unwrap();
        assert_eq!(bytes.len(), 804);
        assert_ne!(PriceSnapshot::SEED, super::super::PriceReport::SEED);
    }
}
