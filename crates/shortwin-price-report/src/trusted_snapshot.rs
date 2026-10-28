//! Operator-attested off-chain history supplied only when an action needs a fresh cache.
//!
//! Unlike `ReportState`, this mode does not witness historical publication on chain.
//! An authorized operator can assert private history; neither a digest nor these structural
//! checks proves that observations existed at the claimed times or omitted no gaps.
use crate::*;

pub mod codec;
pub const TRUSTED_SNAPSHOT_VERSION: u16 = 2;

/// Maximum SOURCE-endpoint gaps INTERNAL to this bucket (zero for one observation).
/// Cross-bucket endpoint gaps are checked separately. These are operator assertions,
/// like every historical price; collection-runtime cadence remains producer policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttestedBucket {
    pub bucket: CompletedBucket,
    pub largest_gap_slots: u64,
    pub largest_gap_seconds: u64,
}

/// Canonical oldest-first prefix. No circular indices or caller-selected active length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttestedHistory {
    buckets: [Option<AttestedBucket>; HISTORY_CAPACITY],
    len: u8,
}
impl AttestedHistory {
    pub const fn empty() -> Self {
        Self {
            buckets: [None; HISTORY_CAPACITY],
            len: 0,
        }
    }
    pub fn new(buckets: [Option<AttestedBucket>; HISTORY_CAPACITY]) -> Result<Self, Error> {
        let mut len = 0;
        let mut ended = false;
        for bucket in &buckets {
            match bucket {
                Some(_) if ended => return Err(Error::InvalidConfiguration),
                Some(_) => len += 1,
                None => ended = true,
            }
        }
        Ok(Self { buckets, len })
    }
    pub const fn len(&self) -> u8 {
        self.len
    }
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn buckets(&self) -> &[Option<AttestedBucket>; HISTORY_CAPACITY] {
        &self.buckets
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &AttestedBucket> {
        self.buckets[..usize::from(self.len)].iter().map(|bucket| {
            // Construction and decoding validate a contiguous present prefix.
            bucket.as_ref().expect("canonical history prefix")
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotReport {
    pub version: u16,
    pub domain: Domain,
    pub quantity: u64,
    pub model: ModelBinding,
    pub configuration_epoch: u64,
    pub publisher: Address,
    pub sequence: u64,
    pub window: ObservationWindow,
    pub observation_digest: Address,
    pub status: ReportStatus,
    pub history: AttestedHistory,
}

/// Immutable exact-q policy and cache. Separate magic/version prevent old history decoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotState {
    configuration: Configuration,
    configured_at: RuntimeClock,
    last_runtime_clock: RuntimeClock,
    last_sequence: u64,
    latest: Option<ReportMetadata>,
    live: ReportStatus,
    history: AttestedHistory,
    outstanding_positions: u64,
}

pub struct SnapshotEvidence<'a> {
    state: &'a SnapshotState,
    marks: CostMarks,
    metadata: ReportMetadata,
}
impl SnapshotEvidence<'_> {
    pub const fn live_marks(&self) -> CostMarks {
        self.marks
    }
    pub const fn metadata(&self) -> ReportMetadata {
        self.metadata
    }
    pub const fn configuration(&self) -> &Configuration {
        &self.state.configuration
    }
    pub fn history(&self) -> impl ExactSizeIterator<Item = &AttestedBucket> {
        self.state.history.iter()
    }
}

impl SnapshotState {
    pub fn new(configuration: Configuration, now: RuntimeClock) -> Result<Self, Error> {
        now.seconds()?;
        Ok(Self {
            configuration,
            configured_at: now,
            last_runtime_clock: now,
            last_sequence: 0,
            latest: None,
            live: ReportStatus::Unknown,
            history: AttestedHistory::empty(),
            outstanding_positions: 0,
        })
    }
    pub const fn configuration(&self) -> &Configuration {
        &self.configuration
    }
    pub const fn configured_at(&self) -> RuntimeClock {
        self.configured_at
    }
    pub const fn last_sequence(&self) -> u64 {
        self.last_sequence
    }
    pub const fn latest_metadata(&self) -> Option<ReportMetadata> {
        self.latest
    }
    pub const fn completed_history_len(&self) -> u8 {
        self.history.len
    }
    pub const fn outstanding_positions(&self) -> u64 {
        self.outstanding_positions
    }
    pub const fn can_retire(&self) -> bool {
        self.outstanding_positions == 0
    }

    /// Internal lifecycle hooks; runtime callers must pair these with actual admitted debt.
    pub fn register_outstanding(&mut self) -> Result<(), Error> {
        self.outstanding_positions = self
            .outstanding_positions
            .checked_add(1)
            .ok_or(Error::OutstandingCountExhausted)?;
        Ok(())
    }
    pub fn release_outstanding(&mut self) -> Result<(), Error> {
        self.outstanding_positions = self
            .outstanding_positions
            .checked_sub(1)
            .ok_or(Error::OutstandingCountUnderflow)?;
        Ok(())
    }
    fn check_clock(&self, now: RuntimeClock) -> Result<(), Error> {
        now.seconds()?;
        if !now.not_before(self.last_runtime_clock) {
            return Err(Error::ClockRegression);
        }
        Ok(())
    }
    fn fresh_metadata(
        &self,
        now: RuntimeClock,
        minimum_sequence: u64,
    ) -> Result<ReportMetadata, Error> {
        self.check_clock(now)?;
        let metadata = self.latest.ok_or(Error::NoObservation)?;
        if metadata.sequence < minimum_sequence {
            return Err(Error::Sequence);
        }
        self.configuration
            .policy
            .validate_window(metadata.window, now)?;
        Ok(metadata)
    }
    /// Latest/minimum sequence semantics permit an already signed bounded action to consume
    /// a fresher cache update. The financial caller must separately bind this exact account,
    /// domain, quantity, model and policy, and enforce all monetary/action bounds.
    pub fn live_marks_at(
        &self,
        now: RuntimeClock,
        minimum_sequence: u64,
    ) -> Result<CostMarks, Error> {
        self.fresh_metadata(now, minimum_sequence)?;
        match self.live {
            ReportStatus::Finite(marks) => Ok(marks),
            ReportStatus::Unknown => Err(Error::Unknown),
        }
    }
    pub fn evidence_at(
        &self,
        now: RuntimeClock,
        minimum_sequence: u64,
    ) -> Result<SnapshotEvidence<'_>, Error> {
        let metadata = self.fresh_metadata(now, minimum_sequence)?;
        let marks = self.live_marks_at(now, minimum_sequence)?;
        if self.history.len < self.configuration.policy.minimum_completed_epochs {
            return Err(Error::HistoryWarming);
        }
        Ok(SnapshotEvidence {
            state: self,
            marks,
            metadata,
        })
    }
    /// All fallible checks precede mutation. There is intentionally no publication-cadence
    /// check: idle cache accounts need no transaction; the operator supplies private history.
    pub fn accept(
        &mut self,
        signer: CallerAuthenticatedSigner,
        report: &SnapshotReport,
        now: RuntimeClock,
    ) -> Result<(), Error> {
        self.check_clock(now)?;
        let cfg = &self.configuration;
        if report.version != TRUSTED_SNAPSHOT_VERSION {
            return Err(Error::Version);
        }
        if report.domain != cfg.domain {
            return Err(Error::DomainMismatch);
        }
        if report.quantity != cfg.quantity {
            return Err(Error::QuantityMismatch);
        }
        if report.model != cfg.model {
            return Err(Error::ModelMismatch);
        }
        if report.configuration_epoch != cfg.epoch {
            return Err(Error::ConfigurationEpochMismatch);
        }
        if !cfg.authorizes(&signer.key()) {
            return Err(Error::UnauthorizedPublisher);
        }
        if report.publisher != signer.key() {
            return Err(Error::PublisherMismatch);
        }
        let expected = self
            .last_sequence
            .checked_add(1)
            .ok_or(Error::SequenceExhausted)?;
        if report.sequence != expected {
            return Err(Error::Sequence);
        }
        if report.observation_digest == [0; 32] {
            return Err(Error::MissingCommitment);
        }
        cfg.policy.validate_window(report.window, now)?;
        if !report.window.start.not_before(self.configured_at) {
            return Err(Error::ObservationBeforeConfiguration);
        }
        if let Some(previous) = self.latest
            && (report.window.end.slot <= previous.window.end.slot
                || report.window.end.unix_timestamp < previous.window.end.unix_timestamp)
        {
            return Err(Error::SourceRegression);
        }
        let epoch = cfg.policy.epoch(report.window.end)?;
        if cfg.policy.epoch(report.window.start)? != epoch
            || cfg.policy.epoch(report.window.end)? != epoch
        {
            return Err(Error::WrongObservationEpoch);
        }
        match report.status {
            ReportStatus::Unknown if !report.history.is_empty() => return Err(Error::Unknown),
            ReportStatus::Unknown => {}
            ReportStatus::Finite(_) => {
                validate_history(cfg, self.configured_at, &report.history, report.window)?
            }
        }
        self.last_sequence = report.sequence;
        self.last_runtime_clock = now;
        self.latest = Some(ReportMetadata {
            sequence: report.sequence,
            publisher: report.publisher,
            window: report.window,
            observation_digest: report.observation_digest,
            observation_epoch: epoch,
        });
        self.live = report.status;
        self.history.clone_from(&report.history);
        Ok(())
    }
}

fn validate_history(
    cfg: &Configuration,
    configured_at: RuntimeClock,
    history: &AttestedHistory,
    current: ObservationWindow,
) -> Result<(), Error> {
    let p = cfg.policy;
    let current_epoch = p.epoch(current.start)?;
    let count = u64::from(history.len);
    let first_epoch = current_epoch
        .checked_sub(count)
        .ok_or(Error::WrongObservationEpoch)?;
    let mut previous: Option<CompletedBucket> = None;
    for (index, attested) in history.iter().enumerate() {
        let b = attested.bucket;
        let expected_epoch = first_epoch
            .checked_add(index as u64)
            .ok_or(Error::InvalidClock)?;
        if b.epoch != expected_epoch
            || p.epoch(b.first_observed)? != b.epoch
            || p.epoch(b.last_observed)? != b.epoch
        {
            return Err(Error::WrongObservationEpoch);
        }
        if !b.first_observed.not_before(configured_at) {
            return Err(Error::ObservationBeforeConfiguration);
        }
        if !b.last_observed.not_before(b.first_observed) || b.observations == 0 {
            return Err(Error::InvalidWindow);
        }
        CostMarks::new(b.lower_min, b.central_last, b.upper_max)?;
        let start = p.epoch_start(b.epoch)?;
        let end = start
            .checked_add(p.epoch_seconds)
            .ok_or(Error::InvalidClock)?;
        if b.first_observed.seconds()? - start > p.max_update_gap_seconds
            || end - b.last_observed.seconds()? > p.max_update_gap_seconds
            || attested.largest_gap_slots > p.max_update_gap_slots
            || attested.largest_gap_seconds > p.max_update_gap_seconds
        {
            return Err(Error::HistoryCadenceExpired);
        }
        // Gap maxima refer to source endpoints INTERNAL to this bucket. The first
        // observation can itself be a bounded interval, not necessarily a point.
        let span_slots = b.last_observed.slot - b.first_observed.slot;
        let span_seconds = b.last_observed.seconds()? - b.first_observed.seconds()?;
        if (b.observations == 1
            && (span_slots > p.max_window_slots
                || span_seconds > p.max_window_seconds
                || attested.largest_gap_slots != 0
                || attested.largest_gap_seconds != 0))
            || attested.largest_gap_slots > span_slots
            || attested.largest_gap_seconds > span_seconds
            || u128::from(b.observations) > u128::from(span_slots) + 1
        {
            return Err(Error::InvalidWindow);
        }
        if b.observations > 1 {
            let gaps = b.observations - 1;
            // Total span includes the initial observation window as well as endpoint
            // gaps. Bounds do not prove the asserted intermediate observations existed.
            if u128::from(attested.largest_gap_slots) * u128::from(gaps)
                + u128::from(p.max_window_slots)
                < u128::from(span_slots)
                || u128::from(attested.largest_gap_seconds) * u128::from(gaps)
                    + u128::from(p.max_window_seconds)
                    < u128::from(span_seconds)
            {
                return Err(Error::HistoryCadenceExpired);
            }
        }
        if let Some(old) = previous {
            validate_gap(p, old.last_observed, b.first_observed)?;
        }
        previous = Some(b);
    }
    // The current epoch is incomplete and is not a historical bucket. Its intermediate
    // samples are private producer state: directly bounding the last completed bucket to
    // the live point would wrongly reject every valid mid-epoch on-demand refresh.
    // Current freshness is independently checked; current-period continuity remains trust.
    Ok(())
}
fn validate_gap(policy: HistoryPolicy, old: RuntimeClock, new: RuntimeClock) -> Result<(), Error> {
    if !new.not_before(old) {
        return Err(Error::SourceRegression);
    }
    if new.slot - old.slot > policy.max_update_gap_slots
        || new.seconds()? - old.seconds()? > policy.max_update_gap_seconds
    {
        return Err(Error::HistoryCadenceExpired);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clock(seconds: u64) -> RuntimeClock {
        RuntimeClock {
            slot: seconds * 2,
            unix_timestamp: seconds as i64,
        }
    }
    fn configuration() -> Configuration {
        Configuration::new(
            Domain {
                genesis_hash: [1; 32],
                program: [2; 32],
                config: [3; 32],
                market: [4; 32],
                base_mint: [5; 32],
                base_token_program: [6; 32],
                quote_mint: [7; 32],
                quote_token_program: [8; 32],
                base_decimals: 6,
                quote_decimals: 9,
            },
            100,
            ModelBinding {
                version: 1,
                epoch: 1,
                policy_hash: [9; 32],
            },
            1,
            &[[10; 32]],
            HistoryPolicy {
                max_age_slots: 8,
                max_age_seconds: 4,
                max_window_slots: 4,
                max_window_seconds: 2,
                max_update_gap_slots: 8,
                max_update_gap_seconds: 4,
                epoch_seconds: 10,
                minimum_completed_epochs: 2,
            },
        )
        .unwrap()
    }
    fn history(epoch: u64) -> AttestedHistory {
        let mut buckets = [None; 8];
        for (i, entry) in buckets.iter_mut().take(2).enumerate() {
            let e = epoch - 2 + i as u64;
            *entry = Some(AttestedBucket {
                bucket: CompletedBucket {
                    epoch: e,
                    first_observed: clock(e * 10),
                    last_observed: clock(e * 10 + 9),
                    lower_min: 90,
                    central_last: 100,
                    upper_max: 110,
                    observations: 10,
                },
                largest_gap_slots: 2,
                largest_gap_seconds: 1,
            });
        }
        AttestedHistory::new(buckets).unwrap()
    }
    fn report(epoch: u64) -> SnapshotReport {
        let c = configuration();
        SnapshotReport {
            version: TRUSTED_SNAPSHOT_VERSION,
            domain: c.domain(),
            quantity: c.quantity(),
            model: c.model(),
            configuration_epoch: 1,
            publisher: [10; 32],
            sequence: 1,
            window: ObservationWindow {
                start: clock(epoch * 10),
                end: clock(epoch * 10),
            },
            observation_digest: [11; 32],
            status: ReportStatus::Finite(CostMarks::new(90, 100, 110).unwrap()),
            history: history(epoch),
        }
    }
    fn signer() -> CallerAuthenticatedSigner {
        CallerAuthenticatedSigner::from_verified_runtime_signer([10; 32])
    }
    fn state() -> SnapshotState {
        SnapshotState::new(configuration(), clock(0)).unwrap()
    }
    #[test]
    fn private_history_warms_in_one_authenticated_action_without_prior_chain_heartbeats() {
        let mut s = state();
        let r = report(40);
        s.accept(signer(), &r, clock(400)).unwrap();
        let e = s.evidence_at(clock(402), 1).unwrap();
        assert_eq!(e.live_marks().upper(), 110);
        assert_eq!(e.history().count(), 2);
        assert_eq!(e.metadata().sequence, 1);
    }
    #[test]
    fn snapshots_are_distinct_from_public_history_codec() {
        let mut s = state();
        s.accept(signer(), &report(4), clock(40)).unwrap();
        let mut bytes = [0; codec::STATE_BYTES];
        codec::encode(&s, &mut bytes).unwrap();
        assert_eq!(codec::decode(&bytes).unwrap(), s);
        assert!(crate::codec::decode(&bytes).is_err());
        assert_eq!(
            codec::decode(&bytes[..1488]),
            Err(crate::codec::CodecError::Length)
        );
    }
    #[test]
    fn fresh_source_epoch_can_cross_runtime_boundary_and_mid_epoch_reads_need_no_chain_cadence() {
        let mut s = state();
        let mut r = report(4);
        r.window = ObservationWindow {
            start: clock(49),
            end: clock(49),
        };
        s.accept(signer(), &r, clock(50)).unwrap();
        assert!(s.evidence_at(clock(51), 1).is_ok());
        assert_eq!(
            s.evidence_at(clock(54), 1).err(),
            Some(Error::StaleObservation)
        );
    }
    #[test]
    fn interval_observations_and_internal_endpoint_gap_bounds_are_compatible() {
        let original = configuration();
        let mut policy = original.policy();
        policy.epoch_seconds = 2;
        let cfg = Configuration::new(
            original.domain(),
            original.quantity(),
            original.model(),
            original.epoch(),
            original.publishers(),
            policy,
        )
        .unwrap();
        let mut s = SnapshotState::new(cfg, clock(0)).unwrap();
        let mut r = report(4);
        r.window = ObservationWindow {
            start: clock(8),
            end: clock(8),
        };
        let mut buckets = [None; HISTORY_CAPACITY];
        for (i, bucket) in buckets.iter_mut().take(2).enumerate() {
            let epoch = 2 + i as u64;
            *bucket = Some(AttestedBucket {
                bucket: CompletedBucket {
                    epoch,
                    first_observed: clock(epoch * 2),
                    last_observed: clock(epoch * 2 + 1),
                    lower_min: 90,
                    central_last: 100,
                    upper_max: 110,
                    observations: 1,
                },
                largest_gap_slots: 0,
                largest_gap_seconds: 0,
            });
        }
        r.history = AttestedHistory::new(buckets).unwrap();
        s.accept(signer(), &r, clock(8)).unwrap();
        assert!(s.evidence_at(clock(8), 1).is_ok());
        let mut s = state();
        let mut r = report(4);
        let mut buckets = *r.history.buckets();
        for bucket in buckets.iter_mut().flatten() {
            bucket.bucket.observations = 5;
            bucket.largest_gap_slots = 4;
            bucket.largest_gap_seconds = 2;
        }
        r.history = AttestedHistory::new(buckets).unwrap();
        s.accept(signer(), &r, clock(40)).unwrap();
        assert!(s.evidence_at(clock(40), 1).is_ok());
    }
    #[test]
    fn failure_never_mutates_or_consumes_sequence() {
        let mut s = state();
        let original = s.clone();
        let mut r = report(4);
        r.domain.market = [99; 32];
        assert_eq!(
            s.accept(signer(), &r, clock(40)),
            Err(Error::DomainMismatch)
        );
        assert_eq!(s, original);
        r = report(4);
        s.accept(signer(), &r, clock(40)).unwrap();
        let prior = s.clone();
        assert_eq!(s.accept(signer(), &r, clock(40)), Err(Error::Sequence));
        assert_eq!(s, prior);
    }
    #[test]
    fn runtime_signer_and_payload_publisher_are_both_checked() {
        let mut s = state();
        let r = report(4);
        assert_eq!(
            s.accept(
                CallerAuthenticatedSigner::from_verified_runtime_signer([99; 32]),
                &r,
                clock(40)
            ),
            Err(Error::UnauthorizedPublisher)
        );
        let mut r = r;
        r.publisher = [99; 32];
        assert_eq!(
            s.accept(signer(), &r, clock(40)),
            Err(Error::PublisherMismatch)
        );
    }
    #[test]
    fn all_immutable_domain_quantity_model_policy_bindings_are_checked() {
        for mutation in 0..5 {
            let mut s = state();
            let mut r = report(4);
            match mutation {
                0 => r.quantity += 1,
                1 => r.model.epoch += 1,
                2 => r.model.policy_hash = [88; 32],
                3 => r.configuration_epoch += 1,
                _ => r.version = 1,
            };
            assert!(s.accept(signer(), &r, clock(40)).is_err());
            assert_eq!(s.last_sequence(), 0);
        }
    }
    #[test]
    fn read_uses_minimum_sequence_but_never_relaxes_freshness() {
        let mut s = state();
        s.accept(signer(), &report(4), clock(40)).unwrap();
        assert!(s.evidence_at(clock(40), 0).is_ok());
        assert_eq!(s.evidence_at(clock(40), 2).err(), Some(Error::Sequence));
        assert_eq!(
            s.evidence_at(clock(45), 1).err(),
            Some(Error::StaleObservation)
        );
        let mut r = report(4);
        r.sequence = 2;
        r.window = ObservationWindow {
            start: clock(41),
            end: clock(41),
        };
        s.accept(signer(), &r, clock(41)).unwrap();
        assert_eq!(s.evidence_at(clock(41), 1).unwrap().metadata().sequence, 2);
    }
    #[test]
    fn unknown_clears_all_history_and_cannot_smuggle_buckets() {
        let mut s = state();
        s.accept(signer(), &report(4), clock(40)).unwrap();
        let mut r = report(4);
        r.sequence = 2;
        r.window = ObservationWindow {
            start: clock(41),
            end: clock(41),
        };
        r.status = ReportStatus::Unknown;
        let prior = s.clone();
        assert_eq!(s.accept(signer(), &r, clock(41)), Err(Error::Unknown));
        assert_eq!(s, prior);
        r.history = AttestedHistory::empty();
        s.accept(signer(), &r, clock(41)).unwrap();
        assert_eq!(s.completed_history_len(), 0);
        assert_eq!(s.evidence_at(clock(41), 2).err(), Some(Error::Unknown));
    }
    #[test]
    fn finite_without_enough_history_is_only_warming() {
        let mut s = state();
        let mut r = report(4);
        r.history = AttestedHistory::empty();
        s.accept(signer(), &r, clock(40)).unwrap();
        assert!(s.live_marks_at(clock(40), 1).is_ok());
        assert_eq!(
            s.evidence_at(clock(40), 1).err(),
            Some(Error::HistoryWarming)
        );
    }
    #[test]
    fn omitted_internal_or_boundary_gaps_fail_closed() {
        for variant in 0..5 {
            let mut s = state();
            let mut r = report(4);
            let mut buckets = *r.history.buckets();
            let b = buckets[0].as_mut().unwrap();
            match variant {
                0 => b.largest_gap_seconds = 5,
                1 => b.largest_gap_slots = 9,
                2 => b.bucket.first_observed = clock(25),
                3 => b.bucket.last_observed = clock(24),
                _ => b.largest_gap_slots = 0,
            };
            r.history = AttestedHistory::new(buckets).unwrap();
            assert!(
                s.accept(signer(), &r, clock(40)).is_err(),
                "variant {variant}"
            );
            assert_eq!(s.last_sequence(), 0);
        }
    }
    #[test]
    fn current_future_old_reordered_and_preconfiguration_history_are_refused() {
        for variant in 0..5 {
            let mut s = state();
            let mut r = report(4);
            let mut buckets = *r.history.buckets();
            match variant {
                0 => buckets[1].as_mut().unwrap().bucket.epoch = 4,
                1 => buckets.swap(0, 1),
                2 => buckets[0].as_mut().unwrap().bucket.epoch = 1,
                3 => r.window.end = clock(41),
                _ => s = SnapshotState::new(configuration(), clock(21)).unwrap(),
            };
            r.history = AttestedHistory::new(buckets).unwrap();
            assert!(s.accept(signer(), &r, clock(40)).is_err());
        }
    }
    #[test]
    fn canonical_history_prefix_and_persistence_reject_hidden_tails() {
        let mut r = report(4);
        let mut buckets = *r.history.buckets();
        buckets.swap(1, 7);
        assert!(AttestedHistory::new(buckets).is_err());
        r.history = AttestedHistory::empty();
        let mut s = state();
        s.accept(signer(), &r, clock(40)).unwrap();
        let mut bytes = [0; codec::STATE_BYTES];
        codec::encode(&s, &mut bytes).unwrap();
        bytes[codec::STATE_BYTES - 1] = 1;
        assert_eq!(
            codec::decode(&bytes),
            Err(crate::codec::CodecError::NonCanonical)
        );
    }
    #[test]
    fn source_progression_and_runtime_clock_regression_are_refused() {
        let mut s = state();
        s.accept(signer(), &report(4), clock(40)).unwrap();
        let mut r = report(4);
        r.sequence = 2;
        assert_eq!(
            s.accept(signer(), &r, clock(41)),
            Err(Error::SourceRegression)
        );
        assert_eq!(
            s.evidence_at(clock(39), 1).err(),
            Some(Error::ClockRegression)
        );
    }
    #[test]
    fn debt_hooks_are_checked_and_configuration_has_no_mutation_api() {
        let mut s = state();
        assert!(s.can_retire());
        assert_eq!(
            s.release_outstanding(),
            Err(Error::OutstandingCountUnderflow)
        );
        s.register_outstanding().unwrap();
        assert!(!s.can_retire());
        let mut bytes = [0; codec::STATE_BYTES];
        codec::encode(&s, &mut bytes).unwrap();
        s = codec::decode(&bytes).unwrap();
        assert_eq!(s.outstanding_positions(), 1);
        s.release_outstanding().unwrap();
        assert!(s.can_retire());
    }
    #[test]
    fn every_decodable_single_bit_mutation_reencodes_canonically() {
        let mut s = state();
        s.accept(signer(), &report(4), clock(40)).unwrap();
        let mut original = [0; codec::STATE_BYTES];
        codec::encode(&s, &mut original).unwrap();
        for i in 0..original.len() {
            for bit in 0..8 {
                let mut bytes = original;
                bytes[i] ^= 1 << bit;
                if let Ok(decoded) = codec::decode(&bytes) {
                    let mut encoded = [0; codec::STATE_BYTES];
                    codec::encode(&decoded, &mut encoded).unwrap();
                    assert_eq!(encoded, bytes);
                }
            }
        }
    }
}
