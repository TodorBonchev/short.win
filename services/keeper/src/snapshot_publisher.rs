//! Explicit, single-attempt, snapshot-only transaction publisher.
//!
//! Never called by the polling loop. All instructions/accounts are service-built;
//! no arbitrary caller transaction/message or transfer recipient is accepted. The
//! same dedicated allowed reporter pays this transaction's fee (one signature).
//! A durable worst-fee/burst reservation precedes send. No new blockhash/signature
//! retry follows a lost reply. Reconcile the recorded exact signature first.
use crate::publication_budget::{
    PendingPublication, PublicationBudget, PublicationError, PublicationResolution, ResolutionKind,
};
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use shortwin::state::{
    AttestedPriceBucketArgs, GlobalConfig, PriceReportStatusArgs, PriceSnapshot,
    PublishPriceSnapshotArgs,
};
use shortwin_price_report::{
    Address, CallerAuthenticatedSigner, ReportStatus, RuntimeClock,
    trusted_snapshot::{SnapshotReport, codec},
};
use shortwin_services_common::{
    Chain, TxFormat,
    chain::TxError,
    rpc::RpcChain,
    tx::{self, ComputeBudget},
};
use solana_commitment_config::CommitmentConfig;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_signer::Signer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublisherError {
    Settings,
    Receiver,
    Publisher,
    Snapshot,
    Packet,
    Chain,
    Simulation,
    FeeCap,
    Funding,
    Budget(PublicationError),
    ConfirmationUncertain,
    ConfirmedFailure,
    ReconciliationPending,
}
impl std::fmt::Display for PublisherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "snapshot publisher: {self:?}")
    }
}
impl std::error::Error for PublisherError {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationSettings {
    pub compute_unit_limit: u32,
    pub micro_lamports_per_cu: u64,
    pub maximum_fee_lamports: u64,
    pub minimum_payer_balance_after_lamports: u64,
    pub operation_timeout_seconds: u64,
}
impl PublicationSettings {
    pub fn validate(self) -> Result<(), PublisherError> {
        if self.operation_timeout_seconds == 0
            || self.operation_timeout_seconds > 120
            || self.compute_unit_limit == 0
            || self.compute_unit_limit > 200_000
            || self.maximum_fee_lamports == 0
            || tx::priority_fee_lamports(&self.compute()) > self.maximum_fee_lamports
        {
            return Err(PublisherError::Settings);
        }
        Ok(())
    }
    fn compute(self) -> ComputeBudget {
        ComputeBudget {
            unit_limit: self.compute_unit_limit,
            unit_price: self.micro_lamports_per_cu,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationReceipt {
    pub signature: String,
    pub slot: u64,
    pub actual_fee_lamports: Option<u64>,
    pub reserved_fee_lamports: u64,
    pub sequence: u64,
    pub success: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconciliationReceipt {
    pub had_pending: bool,
    pub resolution: Option<PublicationResolution>,
}

/// Pure conversion retains exact public integer values and canonical zero padding.
pub fn snapshot_args(report: &SnapshotReport) -> Result<PublishPriceSnapshotArgs, PublisherError> {
    let mut history = [AttestedPriceBucketArgs::default(); 8];
    for (index, entry) in report.history.iter().enumerate() {
        let b = entry.bucket;
        history[index] = AttestedPriceBucketArgs {
            epoch: b.epoch,
            first_slot: b.first_observed.slot,
            first_ts: b.first_observed.unix_timestamp,
            last_slot: b.last_observed.slot,
            last_ts: b.last_observed.unix_timestamp,
            lower_min: b.lower_min,
            central_last: b.central_last,
            upper_max: b.upper_max,
            observations: b.observations,
            largest_gap_slots: entry.largest_gap_slots,
            largest_gap_seconds: entry.largest_gap_seconds,
        };
    }
    let status = match report.status {
        ReportStatus::Unknown => PriceReportStatusArgs::Unknown,
        ReportStatus::Finite(m) => PriceReportStatusArgs::Finite {
            lower: m.lower(),
            central: m.central(),
            upper: m.upper(),
        },
    };
    Ok(PublishPriceSnapshotArgs {
        version: report.version,
        sequence: report.sequence,
        start_slot: report.window.start.slot,
        start_ts: report.window.start.unix_timestamp,
        end_slot: report.window.end.slot,
        end_ts: report.window.end.unix_timestamp,
        observation_digest: report.observation_digest,
        status,
        history_count: report.history.len(),
        history,
    })
}
fn instruction(report: &SnapshotReport, receiver: Pubkey) -> Result<Instruction, PublisherError> {
    let args = snapshot_args(report)?;
    let accounts = shortwin::accounts::PublishPriceSnapshot {
        publisher: anchor_lang::prelude::Pubkey::new_from_array(report.publisher),
        config: anchor_lang::prelude::Pubkey::new_from_array(report.domain.config),
        market: anchor_lang::prelude::Pubkey::new_from_array(report.domain.market),
        price_snapshot: anchor_lang::prelude::Pubkey::new_from_array(receiver.to_bytes()),
    };
    Ok(Instruction {
        program_id: shortwin::ID,
        accounts: accounts.to_account_metas(None),
        data: shortwin::instruction::PublishPriceSnapshot { args }.data(),
    })
}
fn unsigned_clock(
    clock: shortwin_services_common::ChainClock,
) -> Result<(RuntimeClock, u64), PublisherError> {
    let seconds = u64::try_from(clock.unix_timestamp).map_err(|_| PublisherError::Chain)?;
    Ok((
        RuntimeClock {
            slot: clock.slot,
            unix_timestamp: clock.unix_timestamp,
        },
        seconds,
    ))
}
/// Only explicit local action invocation may call this. The receiver and report
/// are freshly checked again here; source-policy matching happened during the
/// trusted watcher's snapshot preparation, and its immutable commitment is pinned.
pub async fn publish_snapshot(
    chain: &RpcChain,
    budget: &PublicationBudget,
    payer: &Keypair,
    receiver: Pubkey,
    expected_producer_commitment: Address,
    report: &SnapshotReport,
    settings: PublicationSettings,
) -> Result<PublicationReceipt, PublisherError> {
    settings.validate()?;
    tokio::time::timeout(
        std::time::Duration::from_secs(settings.operation_timeout_seconds),
        publish_inner(
            chain,
            budget,
            payer,
            receiver,
            expected_producer_commitment,
            report,
            settings,
        ),
    )
    .await
    .map_err(|_| PublisherError::ConfirmationUncertain)?
}
async fn publish_inner(
    chain: &RpcChain,
    budget: &PublicationBudget,
    payer: &Keypair,
    receiver: Pubkey,
    expected_producer_commitment: Address,
    report: &SnapshotReport,
    settings: PublicationSettings,
) -> Result<PublicationReceipt, PublisherError> {
    settings.validate()?;
    verify_mainnet(chain).await?;
    if report.domain.genesis_hash != crate::price_http::MAINNET_GENESIS {
        return Err(PublisherError::Receiver);
    }
    if report.publisher != payer.pubkey().to_bytes() {
        return Err(PublisherError::Publisher);
    }
    if budget.pending().map_err(PublisherError::Budget)?.is_some() {
        return Err(PublisherError::ReconciliationPending);
    }
    let accounts = chain
        .accounts(&[receiver])
        .await
        .map_err(|_| PublisherError::Chain)?;
    let account = accounts
        .first()
        .and_then(Option::as_ref)
        .ok_or(PublisherError::Receiver)?;
    if account.owner.to_bytes() != shortwin::ID.to_bytes()
        || account.executable
        || account.data.len() != PriceSnapshot::SPACE
    {
        return Err(PublisherError::Receiver);
    }
    let wrapper = PriceSnapshot::try_deserialize(&mut account.data.as_slice())
        .map_err(|_| PublisherError::Receiver)?;
    if wrapper.version != 2
        || expected_producer_commitment == [0; 32]
        || wrapper.producer_commitment != expected_producer_commitment
    {
        return Err(PublisherError::Receiver);
    }
    let state = codec::decode(&wrapper.state_bytes).map_err(|_| PublisherError::Receiver)?;
    let c = state.configuration();
    let d = c.domain();
    let (pda, bump) = Pubkey::find_program_address(
        &[
            PriceSnapshot::SEED,
            &d.market,
            &c.quantity().to_le_bytes(),
            &c.model().epoch.to_le_bytes(),
        ],
        &Pubkey::new_from_array(d.program),
    );
    let (config_pda, _) =
        Pubkey::find_program_address(&[GlobalConfig::SEED], &Pubkey::new_from_array(d.program));
    if d.program != shortwin::ID.to_bytes()
        || receiver != pda
        || wrapper.bump != bump
        || wrapper.market.to_bytes() != d.market
        || wrapper.quantity != c.quantity()
        || wrapper.model_epoch != c.model().epoch
        || d.config != config_pda.to_bytes()
    {
        return Err(PublisherError::Receiver);
    }
    let signer = CallerAuthenticatedSigner::from_verified_runtime_signer(payer.pubkey().to_bytes());
    let (now, _) = unsigned_clock(chain.clock().await.map_err(|_| PublisherError::Chain)?)?;
    state
        .clone()
        .accept(signer, report, now)
        .map_err(|_| PublisherError::Snapshot)?;
    let ix = instruction(report, receiver)?;
    let (blockhash, last_valid_block_height) = chain
        .client()
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
        .await
        .map_err(|_| PublisherError::Chain)?;
    let tx = tx::build(
        TxFormat::V0,
        payer,
        &[],
        &[ix],
        &settings.compute(),
        blockhash,
        &[],
    )
    .map_err(|_| PublisherError::Packet)?;
    if tx::size(&tx) > tx::V0_MAX_TX_BYTES || tx.message.header().num_required_signatures != 1 {
        return Err(PublisherError::Packet);
    }
    let simulated = chain
        .simulate(&tx)
        .await
        .map_err(|_| PublisherError::Chain)?;
    if !simulated.ok() || simulated.units_consumed > u64::from(settings.compute_unit_limit) {
        return Err(PublisherError::Simulation);
    }
    let fee = match &tx.message {
        solana_message::VersionedMessage::V0(message) => {
            chain.client().get_fee_for_message(message).await
        }
        _ => return Err(PublisherError::Packet),
    }
    .map_err(|_| PublisherError::Chain)?;
    if fee > settings.maximum_fee_lamports {
        return Err(PublisherError::FeeCap);
    }
    let balance = chain
        .client()
        .get_balance(&payer.pubkey())
        .await
        .map_err(|_| PublisherError::Chain)?;
    if balance
        < settings
            .maximum_fee_lamports
            .checked_add(settings.minimum_payer_balance_after_lamports)
            .ok_or(PublisherError::Settings)?
    {
        return Err(PublisherError::Funding);
    }
    let (now, seconds) = unsigned_clock(chain.clock().await.map_err(|_| PublisherError::Chain)?)?;
    // Freshness rechecked AFTER simulation, fee and funding round trips.
    state
        .clone()
        .accept(signer, report, now)
        .map_err(|_| PublisherError::Snapshot)?;
    let signature = tx.signatures[0];
    budget
        .reserve(
            seconds,
            PendingPublication {
                signature: signature.as_ref().to_vec(),
                recent_blockhash: blockhash.to_bytes(),
                last_valid_block_height,
                receiver: receiver.to_bytes(),
                sequence: report.sequence,
                reserved_fee_lamports: settings.maximum_fee_lamports,
            },
        )
        .map_err(PublisherError::Budget)?;
    // ONE send invocation with this exact signed message. Uncertain result keeps
    // both the reservation and pending signature; no hidden rebuild/re-sign loop.
    let outcome = chain
        .send(&tx)
        .await
        .map_err(|_| PublisherError::ConfirmationUncertain)?;
    if outcome.signature != signature {
        return Err(PublisherError::ConfirmationUncertain);
    }
    if matches!(outcome.err, Some(TxError::Dropped(_))) {
        return Err(PublisherError::ConfirmationUncertain);
    }
    if outcome.slot == 0 {
        return Err(PublisherError::ConfirmationUncertain);
    }
    let (_, seconds) = unsigned_clock(
        chain
            .clock()
            .await
            .map_err(|_| PublisherError::ConfirmationUncertain)?,
    )?;
    let success = outcome.ok();
    budget
        .complete(
            PublicationResolution {
                signature: signature.as_ref().to_vec(),
                receiver: receiver.to_bytes(),
                sequence: report.sequence,
                reserved_fee_lamports: settings.maximum_fee_lamports,
                slot: Some(outcome.slot),
                actual_fee_lamports: (outcome.fee > 0).then_some(outcome.fee),
                kind: if success {
                    ResolutionKind::ConfirmedSuccess
                } else {
                    ResolutionKind::ConfirmedFailure
                },
            },
            seconds,
        )
        .map_err(PublisherError::Budget)?;
    Ok(PublicationReceipt {
        signature: signature.to_string(),
        slot: outcome.slot,
        actual_fee_lamports: (outcome.fee > 0).then_some(outcome.fee),
        reserved_fee_lamports: settings.maximum_fee_lamports,
        sequence: report.sequence,
        success,
    })
}
/// Read-only reconciliation of the recorded signature. No resend/re-sign occurs.
/// A None status is insufficient by itself: finalized blockheight must be beyond
/// the stored validity horizon before an absent transaction can be cleared.
pub async fn reconcile_publication(
    chain: &RpcChain,
    budget: &PublicationBudget,
) -> Result<ReconciliationReceipt, PublisherError> {
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        reconcile_inner(chain, budget),
    )
    .await
    .map_err(|_| PublisherError::ReconciliationPending)?
}
async fn reconcile_inner(
    chain: &RpcChain,
    budget: &PublicationBudget,
) -> Result<ReconciliationReceipt, PublisherError> {
    verify_mainnet(chain).await?;
    let Some(pending) = budget.pending().map_err(PublisherError::Budget)? else {
        return Ok(ReconciliationReceipt {
            had_pending: false,
            resolution: budget.last_resolution().map_err(PublisherError::Budget)?,
        });
    };
    let signature = Signature::try_from(pending.signature.as_slice())
        .map_err(|_| PublisherError::Budget(PublicationError::Corrupt))?;
    let statuses = chain
        .client()
        .get_signature_statuses_with_history(&[signature])
        .await
        .map_err(|_| PublisherError::Chain)?;
    let status = statuses.value.first().and_then(Option::as_ref);
    let disposition = if let Some(status) = status {
        if !status.satisfies_commitment(CommitmentConfig::confirmed()) {
            return Err(PublisherError::ReconciliationPending);
        }
        if status.err.is_none() {
            ResolutionKind::ConfirmedSuccess
        } else {
            ResolutionKind::ConfirmedFailure
        }
    } else {
        let finalized_height = chain
            .client()
            .get_block_height_with_commitment(CommitmentConfig::finalized())
            .await
            .map_err(|_| PublisherError::Chain)?;
        if finalized_height <= pending.last_valid_block_height {
            return Err(PublisherError::ReconciliationPending);
        }
        ResolutionKind::ExpiredAbsent
    };
    let (_, seconds) = unsigned_clock(chain.clock().await.map_err(|_| PublisherError::Chain)?)?;
    let resolution = PublicationResolution {
        signature: pending.signature.clone(),
        receiver: pending.receiver,
        sequence: pending.sequence,
        reserved_fee_lamports: pending.reserved_fee_lamports,
        slot: status.map(|s| s.slot),
        actual_fee_lamports: None,
        kind: disposition,
    };
    budget
        .complete(resolution.clone(), seconds)
        .map_err(PublisherError::Budget)?;
    Ok(ReconciliationReceipt {
        had_pending: true,
        resolution: Some(resolution),
    })
}

async fn verify_mainnet(chain: &RpcChain) -> Result<(), PublisherError> {
    let genesis = chain
        .client()
        .get_genesis_hash()
        .await
        .map_err(|_| PublisherError::Chain)?;
    if genesis.to_bytes() != crate::price_http::MAINNET_GENESIS {
        return Err(PublisherError::Receiver);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::AnchorSerialize;
    use shortwin_price_report::{
        CompletedBucket, CostMarks, Domain, ModelBinding, ObservationWindow,
        trusted_snapshot::{AttestedBucket, AttestedHistory},
    };
    fn report(publisher: Address) -> SnapshotReport {
        let mut entries = [None; 8];
        for (i, entry) in entries.iter_mut().enumerate() {
            *entry = Some(AttestedBucket {
                bucket: CompletedBucket {
                    epoch: i as u64,
                    first_observed: RuntimeClock {
                        slot: 1 + i as u64 * 4,
                        unix_timestamp: i as i64,
                    },
                    last_observed: RuntimeClock {
                        slot: 1 + i as u64 * 4,
                        unix_timestamp: i as i64,
                    },
                    lower_min: 10,
                    central_last: 11,
                    upper_max: 12,
                    observations: 1,
                },
                largest_gap_slots: 0,
                largest_gap_seconds: 0,
            });
        }
        SnapshotReport {
            version: 2,
            domain: Domain {
                genesis_hash: crate::price_http::MAINNET_GENESIS,
                program: shortwin::ID.to_bytes(),
                config: [3; 32],
                market: [4; 32],
                base_mint: [5; 32],
                base_token_program: [6; 32],
                quote_mint: [7; 32],
                quote_token_program: [8; 32],
                base_decimals: 6,
                quote_decimals: 9,
            },
            quantity: u64::MAX,
            model: ModelBinding {
                version: 1,
                epoch: 1,
                policy_hash: [9; 32],
            },
            configuration_epoch: 17,
            publisher,
            sequence: 9,
            window: ObservationWindow {
                start: RuntimeClock {
                    slot: 40,
                    unix_timestamp: 9,
                },
                end: RuntimeClock {
                    slot: 40,
                    unix_timestamp: 9,
                },
            },
            observation_digest: [11; 32],
            status: ReportStatus::Finite(CostMarks::new(10, 11, 12).unwrap()),
            history: AttestedHistory::new(entries).unwrap(),
        }
    }
    #[test]
    fn fixed_snapshot_instruction_and_real_signed_v0_packet_have_only_expected_authority() {
        let payer = Keypair::new();
        let report = report(payer.pubkey().to_bytes());
        let receiver = Pubkey::new_unique();
        let args = snapshot_args(&report).unwrap();
        let mut bytes = Vec::new();
        args.serialize(&mut bytes).unwrap();
        assert_eq!(bytes.len(), 804);
        assert_eq!(args.history_count, 8);
        assert_eq!(args.sequence, 9);
        let ix = instruction(&report, receiver).unwrap();
        assert_eq!(ix.program_id, shortwin::ID);
        assert_eq!(ix.accounts.len(), 4);
        assert_eq!(ix.accounts.iter().filter(|a| a.is_signer).count(), 1);
        assert_eq!(ix.accounts.iter().filter(|a| a.is_writable).count(), 1);
        assert_eq!(ix.accounts[3].pubkey.to_bytes(), receiver.to_bytes());
        let built = tx::build(
            TxFormat::V0,
            &payer,
            &[],
            &[ix],
            &ComputeBudget {
                unit_limit: 50_000,
                unit_price: 10_000,
            },
            solana_hash::Hash::new_unique(),
            &[],
        )
        .unwrap();
        assert_eq!(built.message.header().num_required_signatures, 1);
        assert!(tx::size(&built) <= 1_232);
        assert!(built.signatures[0].verify(payer.pubkey().as_ref(), &built.message.serialize()));
        assert_eq!(built.message.instructions().len(), 3); // CUlimit,CUprice,fixedpublisher
    }
    #[test]
    fn unknown_and_short_history_are_canonically_zero_padded_and_settings_bound_cost() {
        let mut report = report([1; 32]);
        report.history = AttestedHistory::empty();
        report.status = ReportStatus::Unknown;
        let args = snapshot_args(&report).unwrap();
        assert_eq!(args.history_count, 0);
        assert!(args.history.iter().all(AttestedPriceBucketArgs::is_zero));
        let valid = PublicationSettings {
            compute_unit_limit: 50_000,
            micro_lamports_per_cu: 10_000,
            maximum_fee_lamports: 10_000,
            minimum_payer_balance_after_lamports: 100_000,
            operation_timeout_seconds: 15,
        };
        valid.validate().unwrap();
        for invalid in [
            PublicationSettings {
                compute_unit_limit: 0,
                ..valid
            },
            PublicationSettings {
                compute_unit_limit: 200_001,
                ..valid
            },
            PublicationSettings {
                maximum_fee_lamports: 0,
                ..valid
            },
            PublicationSettings {
                micro_lamports_per_cu: u64::MAX,
                ..valid
            },
            PublicationSettings {
                operation_timeout_seconds: 0,
                ..valid
            },
            PublicationSettings {
                operation_timeout_seconds: 121,
                ..valid
            },
        ] {
            assert!(invalid.validate().is_err());
        }
    }
}
