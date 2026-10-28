//! Read-only bridge from actual owned trusted-history cache accounts to financial arithmetic.
//! No arbitrary caller history or legacy public-history account is accepted. A successful
//! summary authenticates reference inputs; financial eligibility/execution remains separate.
use crate::constants::{CONFIG_VERSION, MARKET_VERSION};
use crate::errors::ShortwinError;
use crate::state::{
    GlobalConfig, Market, PriceSnapshot, QuotePolicy, MARKET_FLAG_QUOTE_NATIVE,
    MARKET_FLAG_QUOTE_PRICE_PAUSED,
};
use anchor_lang::prelude::*;
use shortwin_math::quote_native as math;
use shortwin_price_report::{self as report, trusted_snapshot::SnapshotEvidence};

/// Legacy financial mutators must use this before consuming reserve-native rent/price fields.
/// Initialization of a prospective QuotePolicy never sets the flag this guard reads.
pub fn require_legacy_finance_guard(market: &Market) -> Result<()> {
    require!(
        !market.has_flag(MARKET_FLAG_QUOTE_NATIVE),
        ShortwinError::AccountMismatch
    );
    Ok(())
}

/// The clock is read from the actual runtime here, never from instruction data or a caller body.
/// The typed loaders have already authenticated program ownership/discriminators; their
/// canonical PDA/version/size binding is independently checked before reading a snapshot.
pub fn summary_from_snapshot<'info>(
    policy_account: &AccountInfo<'info>,
    config: &AccountLoader<'info, GlobalConfig>,
    market: &AccountLoader<'info, Market>,
    quantity: u64,
    snapshot_account: &AccountInfo<'info>,
    minimum_sequence: u64,
) -> Result<math::Summary> {
    let c = config.load()?;
    let m = market.load()?;
    require_eq!(c.version, CONFIG_VERSION, ShortwinError::ConfigVersion);
    require_eq!(m.version, MARKET_VERSION, ShortwinError::ConfigVersion);
    require_eq!(
        config.to_account_info().data_len(),
        GlobalConfig::SPACE,
        ShortwinError::AccountMismatch
    );
    require_eq!(
        market.to_account_info().data_len(),
        Market::SPACE,
        ShortwinError::AccountMismatch
    );
    let config_pda = Pubkey::create_program_address(&[GlobalConfig::SEED, &[c.bump]], &crate::ID)
        .map_err(|_| error!(ShortwinError::AccountMismatch))?;
    require!(
        config_pda == config.key() && m.is_pda(&market.key()),
        ShortwinError::AccountMismatch
    );
    let clock = Clock::get()?;
    summary_at(
        policy_account,
        &config.key(),
        &market.key(),
        &m,
        quantity,
        snapshot_account,
        minimum_sequence,
        report::RuntimeClock {
            slot: clock.slot,
            unix_timestamp: clock.unix_timestamp,
        },
    )
}

#[inline(never)]
fn policy_from_account(
    account: &AccountInfo<'_>,
    config: &Pubkey,
    market_key: &Pubkey,
    market: &Market,
) -> Result<Box<QuotePolicy>> {
    require_keys_eq!(*account.owner, crate::ID, ShortwinError::AccountMismatch);
    require!(
        !account.executable && account.data_len() == QuotePolicy::SPACE,
        ShortwinError::AccountMismatch
    );
    let policy = Box::new(QuotePolicy::try_deserialize(
        &mut account.try_borrow_data()?.as_ref(),
    )?);
    let expected = Pubkey::create_program_address(
        &[
            QuotePolicy::SEED,
            market_key.as_ref(),
            &policy.model_epoch.to_le_bytes(),
            &[policy.bump],
        ],
        &crate::ID,
    )
    .map_err(|_| error!(ShortwinError::AccountMismatch))?;
    require_keys_eq!(expected, account.key(), ShortwinError::AccountMismatch);
    policy.validate_binding(config, market_key, market)?;
    Ok(policy)
}
#[inline(never)]
fn snapshot_from_account(account: &AccountInfo<'_>) -> Result<Box<PriceSnapshot>> {
    require_keys_eq!(*account.owner, crate::ID, ShortwinError::AccountMismatch);
    require!(
        !account.executable && account.data_len() == PriceSnapshot::SPACE,
        ShortwinError::AccountMismatch
    );
    Ok(Box::new(PriceSnapshot::try_deserialize(
        &mut account.try_borrow_data()?.as_ref(),
    )?))
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn summary_at(
    policy_account: &AccountInfo<'_>,
    config: &Pubkey,
    market_key: &Pubkey,
    market: &Market,
    quantity: u64,
    snapshot_account: &AccountInfo<'_>,
    minimum_sequence: u64,
    now: report::RuntimeClock,
) -> Result<math::Summary> {
    require!(
        !market.has_flag(MARKET_FLAG_QUOTE_PRICE_PAUSED),
        ShortwinError::QuotePriceActionsPaused
    );
    let policy = policy_from_account(policy_account, config, market_key, market)?;
    let binding = policy.feed(quantity)?;
    require_keys_eq!(
        binding.snapshot,
        snapshot_account.key(),
        ShortwinError::AccountMismatch
    );
    let snapshot = snapshot_from_account(snapshot_account)?;
    require!(
        snapshot.producer_commitment == binding.producer_commitment
            && snapshot.quantity == quantity
            && snapshot.model_epoch == policy.model_epoch,
        ShortwinError::AccountMismatch
    );
    let actual_snapshot = Pubkey::create_program_address(
        &[
            PriceSnapshot::SEED,
            market_key.as_ref(),
            &quantity.to_le_bytes(),
            &policy.model_epoch.to_le_bytes(),
            &[snapshot.bump],
        ],
        &crate::ID,
    )
    .map_err(|_| error!(ShortwinError::AccountMismatch))?;
    require_keys_eq!(
        actual_snapshot,
        snapshot_account.key(),
        ShortwinError::AccountMismatch
    );
    let state = snapshot.restore(config, market_key, market)?;
    let c = state.configuration();
    require!(
        c.domain() == policy.domain.core()
            && c.quantity() == quantity
            && c.model().version == policy.model_version
            && c.model().epoch == policy.model_epoch
            && c.model().policy_hash == policy.policy_hash
            && c.epoch() == 1
            && c.policy() == policy.history.core(),
        ShortwinError::AccountMismatch
    );
    let evidence = state.evidence_at(now, minimum_sequence).map_err(|err| {
        msg!("Unavailable trusted quote evidence: {:?}", err);
        error!(ShortwinError::InvalidParams)
    })?;
    adapt_summary(&policy, quantity, &evidence, now)
}
fn clock(c: report::RuntimeClock) -> math::Clock {
    math::Clock {
        slot: c.slot,
        unix_timestamp: c.unix_timestamp,
    }
}
fn marks(m: report::CostMarks) -> Result<math::Marks> {
    math::Marks::new(m.lower(), m.central(), m.upper())
        .map_err(|_| error!(ShortwinError::AccountMismatch))
}
#[inline(never)]
fn adapt_summary(
    policy: &QuotePolicy,
    quantity: u64,
    evidence: &SnapshotEvidence<'_>,
    now: report::RuntimeClock,
) -> Result<math::Summary> {
    let key = math::ReferenceKey::new(
        policy.domain.digest(),
        policy.policy_hash,
        policy.model_version,
        policy.model_epoch,
        quantity,
    )
    .map_err(|_| error!(ShortwinError::InvalidParams))?;
    let metadata = evidence.metadata();
    let live = math::LiveReference {
        key,
        epoch: metadata.observation_epoch,
        source: math::Window {
            start: clock(metadata.window.start),
            end: clock(metadata.window.end),
        },
        status: math::Status::Finite(marks(evidence.live_marks())?),
    };
    let count = evidence.history().len();
    let needed = usize::from(policy.history.minimum_completed_epochs);
    require!(
        count >= needed && count <= report::HISTORY_CAPACITY,
        ShortwinError::AccountMismatch
    );
    let empty = math::CompletedReference {
        key,
        epoch: 0,
        first_observed: clock(now),
        last_observed: clock(now),
        observations: 0,
        status: math::Status::Unknown,
    };
    let mut history = [empty; report::HISTORY_CAPACITY];
    let first = count
        .checked_sub(needed)
        .ok_or(ShortwinError::AccountMismatch)?;
    for (index, entry) in evidence.history().enumerate() {
        if index >= first {
            let b = entry.bucket;
            let offset = count
                .checked_sub(index)
                .and_then(|n| n.checked_sub(1))
                .ok_or(ShortwinError::AccountMismatch)?;
            let target = history
                .get_mut(offset)
                .ok_or(ShortwinError::AccountMismatch)?;
            *target = math::CompletedReference {
                key,
                epoch: b.epoch,
                first_observed: clock(b.first_observed),
                last_observed: clock(b.last_observed),
                observations: b.observations,
                status: math::Status::Finite(
                    math::Marks::new(b.lower_min, b.central_last, b.upper_max)
                        .map_err(|_| error!(ShortwinError::AccountMismatch))?,
                ),
            };
        }
    }
    math::summarize_trusted_history(
        key,
        live,
        &history[..needed],
        policy.history.evidence_policy(),
        clock(now),
    )
    .map_err(|err| {
        msg!("Refused trusted financial summary: {:?}", err);
        error!(ShortwinError::InvalidParams)
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::state::quote_policy::tests as policy_fixture;
    use anchor_lang::AccountSerialize;
    use shortwin_price_report::{
        trusted_snapshot::{codec, AttestedBucket, AttestedHistory, SnapshotReport, SnapshotState},
        CallerAuthenticatedSigner, CompletedBucket, Configuration, CostMarks, ModelBinding,
        ObservationWindow, ReportStatus,
    };
    struct Case {
        market: Box<Market>,
        config: Pubkey,
        market_key: Pubkey,
        policy_key: Pubkey,
        snapshot_key: Pubkey,
        policy: Box<QuotePolicy>,
        snapshot: PriceSnapshot,
    }
    fn at(t: i64) -> report::RuntimeClock {
        report::RuntimeClock {
            slot: u64::try_from(t).unwrap() * 2,
            unix_timestamp: t,
        }
    }
    fn cache(c: Configuration, status: ReportStatus) -> SnapshotState {
        let mut state = SnapshotState::new(c, at(0)).unwrap();
        let mut buckets = [None; 8];
        if matches!(status, ReportStatus::Finite(_)) {
            for (index, b) in buckets.iter_mut().take(2).enumerate() {
                let epoch = 2 + index as u64;
                *b = Some(AttestedBucket {
                    bucket: CompletedBucket {
                        epoch,
                        first_observed: at((epoch * 10) as i64),
                        last_observed: at((epoch * 10 + 9) as i64),
                        lower_min: 80 + index as u64,
                        central_last: 100,
                        upper_max: 120 - index as u64,
                        observations: 10,
                    },
                    largest_gap_slots: 2,
                    largest_gap_seconds: 1,
                });
            }
        }
        let r = SnapshotReport {
            version: 2,
            domain: c.domain(),
            quantity: c.quantity(),
            model: c.model(),
            configuration_epoch: c.epoch(),
            publisher: [10; 32],
            sequence: 1,
            window: ObservationWindow {
                start: at(40),
                end: at(40),
            },
            observation_digest: [11; 32],
            status,
            history: AttestedHistory::new(buckets).unwrap(),
        };
        state
            .accept(
                CallerAuthenticatedSigner::from_verified_runtime_signer([10; 32]),
                &r,
                at(40),
            )
            .unwrap();
        state
    }
    fn configuration(policy: &QuotePolicy) -> Configuration {
        Configuration::new(
            policy.domain.core(),
            1000,
            ModelBinding {
                version: policy.model_version,
                epoch: policy.model_epoch,
                policy_hash: policy.policy_hash,
            },
            1,
            &[[10; 32]],
            policy.history.core(),
        )
        .unwrap()
    }
    fn case() -> Case {
        let (market, config, market_key, args) = policy_fixture::fixture();
        let (policy_key, bump) = Pubkey::find_program_address(
            &[
                QuotePolicy::SEED,
                market_key.as_ref(),
                &args.model_epoch.to_le_bytes(),
            ],
            &crate::ID,
        );
        let policy = QuotePolicy::from_args(config, market_key, &market, &args, bump).unwrap();
        let feed = *policy.feed(1000).unwrap();
        let (snapshot_key, snapshot_bump) = Pubkey::find_program_address(
            &[
                PriceSnapshot::SEED,
                market_key.as_ref(),
                &1000u64.to_le_bytes(),
                &args.model_epoch.to_le_bytes(),
            ],
            &crate::ID,
        );
        let mut snapshot = PriceSnapshot {
            version: 2,
            bump: snapshot_bump,
            market: market_key,
            quantity: 1000,
            model_epoch: 1,
            producer_commitment: feed.producer_commitment,
            state_bytes: [0; 1533],
        };
        let state = cache(
            configuration(&policy),
            ReportStatus::Finite(CostMarks::new(90, 100, 110).unwrap()),
        );
        codec::encode(&state, &mut snapshot.state_bytes).unwrap();
        Case {
            market,
            config,
            market_key,
            policy_key,
            snapshot_key,
            policy,
            snapshot,
        }
    }
    fn bytes<T: AccountSerialize>(value: &T) -> Vec<u8> {
        let mut b = Vec::new();
        value.try_serialize(&mut b).unwrap();
        b
    }
    impl Case {
        fn inspect(&self, now: report::RuntimeClock, min: u64) -> Result<math::Summary> {
            self.inspect_bytes(
                bytes(&*self.policy),
                bytes(&self.snapshot),
                crate::ID,
                crate::ID,
                now,
                min,
            )
        }
        fn inspect_bytes(
            &self,
            mut policy_bytes: Vec<u8>,
            mut snapshot_bytes: Vec<u8>,
            policy_owner: Pubkey,
            snapshot_owner: Pubkey,
            now: report::RuntimeClock,
            min: u64,
        ) -> Result<math::Summary> {
            let (mut pl, mut sl) = (100_000, 100_000);
            let pi = AccountInfo::new(
                &self.policy_key,
                false,
                false,
                &mut pl,
                &mut policy_bytes,
                &policy_owner,
                false,
            );
            let si = AccountInfo::new(
                &self.snapshot_key,
                false,
                false,
                &mut sl,
                &mut snapshot_bytes,
                &snapshot_owner,
                false,
            );
            summary_at(
                &pi,
                &self.config,
                &self.market_key,
                &self.market,
                1000,
                &si,
                min,
                now,
            )
        }
    }
    #[test]
    fn actual_owned_account_bytes_adapt_trusted_history_with_minimum_sequence_and_real_age() {
        let c = case();
        let s = c.inspect(at(40), 1).unwrap();
        assert_eq!(s.history_mode(), math::HistoryMode::TrustedOffchain);
        assert_eq!(s.key().quantity(), 1000);
        assert_eq!(s.opening_upper(), 120);
        assert_eq!(s.lower_base(), 80);
        assert_eq!(s.acquisition_reference(), 110);
        assert!(c.inspect(at(41), 0).is_ok());
        assert!(c.inspect(at(40), 2).is_err());
        assert!(c.inspect(at(45), 1).is_err());
    }
    #[test]
    fn wrong_owners_discriminators_tails_and_named_pdas_are_refused() {
        let c = case();
        assert!(c
            .inspect_bytes(
                bytes(&*c.policy),
                bytes(&c.snapshot),
                Pubkey::new_unique(),
                crate::ID,
                at(40),
                1
            )
            .is_err());
        assert!(c
            .inspect_bytes(
                bytes(&*c.policy),
                bytes(&c.snapshot),
                crate::ID,
                Pubkey::new_unique(),
                at(40),
                1
            )
            .is_err());
        for field in 0..4 {
            let mut pb = bytes(&*c.policy);
            let mut sb = bytes(&c.snapshot);
            match field {
                0 => pb.push(0),
                1 => sb.push(0),
                2 => pb[0] ^= 1,
                _ => sb[0] ^= 1,
            };
            assert!(c
                .inspect_bytes(pb, sb, crate::ID, crate::ID, at(40), 1)
                .is_err());
        }
        let mut c = case();
        c.snapshot_key = Pubkey::new_unique();
        assert!(c.inspect(at(40), 1).is_err());
        let mut c = case();
        c.policy_key = Pubkey::new_unique();
        assert!(c.inspect(at(40), 1).is_err());
    }
    #[test]
    fn producer_global_model_domain_common_policy_and_unknown_are_separate_checks() {
        for variant in 0..7 {
            let mut c = case();
            match variant {
                0 => c.snapshot.producer_commitment = [99; 32],
                1 => c.snapshot.bump ^= 1,
                2 => c.snapshot.quantity = 2000,
                3 => {
                    let old = configuration(&c.policy);
                    let mut m = old.model();
                    m.policy_hash = [99; 32];
                    let cfg = Configuration::new(
                        old.domain(),
                        old.quantity(),
                        m,
                        old.epoch(),
                        old.publishers(),
                        old.policy(),
                    )
                    .unwrap();
                    codec::encode(
                        &cache(
                            cfg,
                            ReportStatus::Finite(CostMarks::new(90, 100, 110).unwrap()),
                        ),
                        &mut c.snapshot.state_bytes,
                    )
                    .unwrap();
                }
                4 => {
                    let old = configuration(&c.policy);
                    let mut d = old.domain();
                    d.genesis_hash = [99; 32];
                    let cfg = Configuration::new(
                        d,
                        old.quantity(),
                        old.model(),
                        old.epoch(),
                        old.publishers(),
                        old.policy(),
                    )
                    .unwrap();
                    codec::encode(
                        &cache(
                            cfg,
                            ReportStatus::Finite(CostMarks::new(90, 100, 110).unwrap()),
                        ),
                        &mut c.snapshot.state_bytes,
                    )
                    .unwrap();
                }
                5 => {
                    let old = configuration(&c.policy);
                    let mut p = old.policy();
                    p.max_age_slots += 1;
                    let cfg = Configuration::new(
                        old.domain(),
                        old.quantity(),
                        old.model(),
                        old.epoch(),
                        old.publishers(),
                        p,
                    )
                    .unwrap();
                    codec::encode(
                        &cache(
                            cfg,
                            ReportStatus::Finite(CostMarks::new(90, 100, 110).unwrap()),
                        ),
                        &mut c.snapshot.state_bytes,
                    )
                    .unwrap();
                }
                _ => {
                    codec::encode(
                        &cache(configuration(&c.policy), ReportStatus::Unknown),
                        &mut c.snapshot.state_bytes,
                    )
                    .unwrap();
                }
            }
            assert!(c.inspect(at(40), 1).is_err(), "variant {variant}");
        }
    }
    #[test]
    fn legacy_guard_refuses_quote_backend_without_mutating_any_state() {
        let mut c = case();
        assert!(require_legacy_finance_guard(&c.market).is_ok());
        c.market.set_flag(MARKET_FLAG_QUOTE_NATIVE, true);
        let before = c.market.flags;
        assert!(require_legacy_finance_guard(&c.market).is_err());
        assert_eq!(c.market.flags, before);
        c.market.set_flag(MARKET_FLAG_QUOTE_NATIVE, false);
        c.market.set_flag(MARKET_FLAG_QUOTE_PRICE_PAUSED, true);
        assert!(c.inspect(at(40), 1).is_err());
        c.market.set_flag(MARKET_FLAG_QUOTE_PRICE_PAUSED, false);
        assert!(c.inspect(at(40), 1).is_ok());
    }
}
