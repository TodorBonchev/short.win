//! In-house exact-quantity report state. No HTTP, SDK, signature verification or liquidation.
//!
//! The caller MUST obtain clocks and authenticated signer keys from the actual runtime,
//! verify program ownership/PDA/governance at the account boundary, and maintain outstanding
//! counts atomically with actual debt. This library cannot establish those facts.
//! An authorized report is an operator assertion, not proof of fair price or liquidity.
#![no_std]
#![forbid(unsafe_code)]

pub mod codec;

pub type Address = [u8; 32];
pub const REPORT_VERSION: u16 = 1;
/// Storage/work ceilings only; no economic or quorum defaults.
pub const MAX_PUBLISHERS: usize = 8;
pub const HISTORY_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidConfiguration,
    InvalidClock,
    ClockRegression,
    DomainMismatch,
    QuantityMismatch,
    ModelMismatch,
    ConfigurationEpochMismatch,
    Version,
    UnauthorizedPublisher,
    PublisherMismatch,
    Sequence,
    SequenceExhausted,
    InvalidWindow,
    FutureObservation,
    StaleObservation,
    WindowTooWide,
    ObservationBeforeConfiguration,
    SourceRegression,
    WrongObservationEpoch,
    MissingCommitment,
    InvalidMarks,
    Unknown,
    NoObservation,
    HistoryWarming,
    HistoryCadenceExpired,
    ObservationCountExhausted,
    OutstandingCountExhausted,
    OutstandingCountUnderflow,
    ImmutableBinding,
    ConfigurationEpochNotAdvanced,
    ModelEpochNotAdvanced,
    PolicyChangeRequiresModelEpoch,
    OutstandingPolicyChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeClock {
    pub slot: u64,
    pub unix_timestamp: i64,
}
impl RuntimeClock {
    fn seconds(self) -> Result<u64, Error> {
        u64::try_from(self.unix_timestamp).map_err(|_| Error::InvalidClock)
    }
    fn not_before(self, other: Self) -> bool {
        self.slot >= other.slot && self.unix_timestamp >= other.unix_timestamp
    }
}

/// Qualified source observation bounds asserted by the publisher, not signing time.
/// Build responses do not inherently authenticate this metadata; source policy remains trust.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObservationWindow {
    pub start: RuntimeClock,
    pub end: RuntimeClock,
}

/// Trusted expected identities. Comparing these does not admit a mint, derive a PDA,
/// validate a token program or prove that a configured genesis hash is the current chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Domain {
    pub genesis_hash: Address,
    pub program: Address,
    pub config: Address,
    pub market: Address,
    pub base_mint: Address,
    pub base_token_program: Address,
    pub quote_mint: Address,
    pub quote_token_program: Address,
    pub base_decimals: u8,
    pub quote_decimals: u8,
}
impl Domain {
    fn valid(self) -> bool {
        [
            self.genesis_hash,
            self.program,
            self.config,
            self.market,
            self.base_mint,
            self.base_token_program,
            self.quote_mint,
            self.quote_token_program,
        ]
        .iter()
        .all(|key| *key != [0; 32])
            && self.base_mint != self.quote_mint
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelBinding {
    pub version: u32,
    pub epoch: u64,
    /// Producer/search/economic-reference policy commitment; not computed here.
    pub policy_hash: Address,
}
impl ModelBinding {
    fn valid(self) -> bool {
        self.version > 0 && self.epoch > 0 && self.policy_hash != [0; 32]
    }
}

/// All temporal/history settings are explicit reviewed caller configuration.
/// This stores raw bucket statistics; it selects no EMA, median, CR or payout policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryPolicy {
    pub max_age_slots: u64,
    pub max_age_seconds: u64,
    pub max_window_slots: u64,
    pub max_window_seconds: u64,
    pub max_update_gap_slots: u64,
    pub max_update_gap_seconds: u64,
    pub epoch_seconds: u64,
    pub minimum_completed_epochs: u8,
}
impl HistoryPolicy {
    fn valid(self) -> bool {
        self.epoch_seconds > 0
            && self.max_update_gap_slots > 0
            && self.max_update_gap_seconds > 0
            && self.minimum_completed_epochs > 0
            && usize::from(self.minimum_completed_epochs) <= HISTORY_CAPACITY
    }
    fn epoch(self, clock: RuntimeClock) -> Result<u64, Error> {
        Ok(clock.seconds()? / self.epoch_seconds)
    }
    fn epoch_start(self, epoch: u64) -> Result<u64, Error> {
        epoch
            .checked_mul(self.epoch_seconds)
            .ok_or(Error::InvalidClock)
    }
    fn validate_window(self, window: ObservationWindow, now: RuntimeClock) -> Result<(), Error> {
        let start_seconds = window.start.seconds()?;
        let end_seconds = window.end.seconds()?;
        let now_seconds = now.seconds()?;
        if !window.end.not_before(window.start) {
            return Err(Error::InvalidWindow);
        }
        if !now.not_before(window.end) {
            return Err(Error::FutureObservation);
        }
        if window.end.slot - window.start.slot > self.max_window_slots
            || end_seconds - start_seconds > self.max_window_seconds
        {
            return Err(Error::WindowTooWide);
        }
        // Age from the earliest source observation, not only the latest/signature time.
        if now.slot - window.start.slot > self.max_age_slots
            || now_seconds - start_seconds > self.max_age_seconds
        {
            return Err(Error::StaleObservation);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configuration {
    domain: Domain,
    quantity: u64,
    model: ModelBinding,
    epoch: u64,
    publishers: [Address; MAX_PUBLISHERS],
    publisher_count: u8,
    policy: HistoryPolicy,
}
impl Configuration {
    pub fn new(
        domain: Domain,
        quantity: u64,
        model: ModelBinding,
        epoch: u64,
        publishers: &[Address],
        policy: HistoryPolicy,
    ) -> Result<Self, Error> {
        if !domain.valid()
            || quantity == 0
            || !model.valid()
            || epoch == 0
            || !policy.valid()
            || publishers.is_empty()
            || publishers.len() > MAX_PUBLISHERS
        {
            return Err(Error::InvalidConfiguration);
        }
        let mut keys = [[0; 32]; MAX_PUBLISHERS];
        for (index, key) in publishers.iter().enumerate() {
            if *key == [0; 32] || publishers[..index].contains(key) {
                return Err(Error::InvalidConfiguration);
            }
            keys[index] = *key;
        }
        Ok(Self {
            domain,
            quantity,
            model,
            epoch,
            publishers: keys,
            publisher_count: u8::try_from(publishers.len())
                .map_err(|_| Error::InvalidConfiguration)?,
            policy,
        })
    }
    pub const fn domain(&self) -> Domain {
        self.domain
    }
    pub const fn quantity(&self) -> u64 {
        self.quantity
    }
    pub const fn model(&self) -> ModelBinding {
        self.model
    }
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    pub const fn policy(&self) -> HistoryPolicy {
        self.policy
    }
    pub fn publishers(&self) -> &[Address] {
        &self.publishers[..usize::from(self.publisher_count)]
    }
    fn authorizes(&self, key: &Address) -> bool {
        self.publishers().contains(key)
    }
}

/// Value boundary only: the caller MUST first check the actual runtime signer flag.
/// Constructing this wrapper does not verify a transaction, signature or account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallerAuthenticatedSigner(Address);
impl CallerAuthenticatedSigner {
    pub const fn from_verified_runtime_signer(key: Address) -> Self {
        Self(key)
    }
    pub const fn key(self) -> Address {
        self.0
    }
}

/// Structural interval only. No route/market observation can be proven by ordering numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CostMarks {
    lower: u64,
    central: u64,
    upper: u64,
}
impl CostMarks {
    pub fn new(lower: u64, central: u64, upper: u64) -> Result<Self, Error> {
        if lower == 0 || lower > central || central > upper {
            return Err(Error::InvalidMarks);
        }
        Ok(Self {
            lower,
            central,
            upper,
        })
    }
    pub const fn lower(self) -> u64 {
        self.lower
    }
    pub const fn central(self) -> u64 {
        self.central
    }
    pub const fn upper(self) -> u64 {
        self.upper
    }
}

/// V1 deliberately has no attested-no-route or arbitrary liquidation boolean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportStatus {
    Finite(CostMarks),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportMetadata {
    pub sequence: u64,
    pub publisher: Address,
    pub window: ObservationWindow,
    pub observation_digest: Address,
    pub observation_epoch: u64,
}

/// Raw statistics from one completed timely epoch, not a chosen financial reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletedBucket {
    pub epoch: u64,
    pub first_observed: RuntimeClock,
    pub last_observed: RuntimeClock,
    pub lower_min: u64,
    pub central_last: u64,
    pub upper_max: u64,
    pub observations: u64,
}
impl CompletedBucket {
    fn first(epoch: u64, window: ObservationWindow, marks: CostMarks) -> Self {
        Self {
            epoch,
            first_observed: window.start,
            last_observed: window.end,
            lower_min: marks.lower,
            central_last: marks.central,
            upper_max: marks.upper,
            observations: 1,
        }
    }
    fn extended(self, window: ObservationWindow, marks: CostMarks) -> Result<Self, Error> {
        Ok(Self {
            last_observed: window.end,
            lower_min: self.lower_min.min(marks.lower),
            central_last: marks.central,
            upper_max: self.upper_max.max(marks.upper),
            observations: self
                .observations
                .checked_add(1)
                .ok_or(Error::ObservationCountExhausted)?,
            ..self
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct History {
    buckets: [Option<CompletedBucket>; HISTORY_CAPACITY],
    head: u8,
    count: u8,
}
impl History {
    const fn empty() -> Self {
        Self {
            buckets: [None; HISTORY_CAPACITY],
            head: 0,
            count: 0,
        }
    }
    fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
    }
    fn push(&mut self, bucket: CompletedBucket) {
        self.buckets[usize::from(self.head)] = Some(bucket);
        self.head = (self.head + 1) % (HISTORY_CAPACITY as u8);
        self.count = self.count.saturating_add(1).min(HISTORY_CAPACITY as u8);
    }
    fn newest(&self, offset: usize) -> Option<&CompletedBucket> {
        if offset >= usize::from(self.count) {
            return None;
        }
        let index = (usize::from(self.head) + HISTORY_CAPACITY - 1 - offset) % HISTORY_CAPACITY;
        self.buckets[index].as_ref()
    }
}

/// Per-exact-q state, deliberately not a wire/account ABI. No global quantity/lot cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportState {
    configuration: Configuration,
    configured_at: RuntimeClock,
    last_runtime_clock: RuntimeClock,
    last_sequence: u64,
    latest: Option<ReportMetadata>,
    live: ReportStatus,
    pending_epoch: Option<u64>,
    pending: Option<CompletedBucket>,
    pending_tainted: bool,
    history: History,
    outstanding_positions: u64,
}

struct PreparedUpdate {
    metadata: ReportMetadata,
    live: ReportStatus,
    pending: Option<CompletedBucket>,
    pending_tainted: bool,
    clear_history: bool,
    completed: Option<CompletedBucket>,
}

impl ReportState {
    pub fn new(configuration: Configuration, now: RuntimeClock) -> Result<Self, Error> {
        now.seconds()?;
        Ok(Self {
            configuration,
            configured_at: now,
            last_runtime_clock: now,
            last_sequence: 0,
            latest: None,
            live: ReportStatus::Unknown,
            pending_epoch: None,
            pending: None,
            pending_tainted: true,
            history: History::empty(),
            outstanding_positions: 0,
        })
    }
    pub const fn configuration(&self) -> &Configuration {
        &self.configuration
    }
    pub const fn last_sequence(&self) -> u64 {
        self.last_sequence
    }
    pub const fn latest_metadata(&self) -> Option<ReportMetadata> {
        self.latest
    }
    pub const fn outstanding_positions(&self) -> u64 {
        self.outstanding_positions
    }
    pub const fn can_retire(&self) -> bool {
        self.outstanding_positions == 0
    }
    pub const fn completed_history_len(&self) -> u8 {
        self.history.count
    }

    /// Internal debt lifecycle hook. Caller must pair it atomically with a real admitted open.
    /// This is not admission, a borrowing cap or permission to create debt during an outage.
    pub fn register_outstanding(&mut self) -> Result<(), Error> {
        let next = self
            .outstanding_positions
            .checked_add(1)
            .ok_or(Error::OutstandingCountExhausted)?;
        self.outstanding_positions = next;
        Ok(())
    }
    /// Internal lifecycle hook after actual full repayment or authorized default accounting.
    pub fn release_outstanding(&mut self) -> Result<(), Error> {
        let next = self
            .outstanding_positions
            .checked_sub(1)
            .ok_or(Error::OutstandingCountUnderflow)?;
        self.outstanding_positions = next;
        Ok(())
    }

    fn check_clock(&self, now: RuntimeClock) -> Result<(), Error> {
        now.seconds()?;
        if !now.not_before(self.last_runtime_clock) {
            return Err(Error::ClockRegression);
        }
        Ok(())
    }
    fn fresh(&self, now: RuntimeClock) -> Result<ReportMetadata, Error> {
        self.check_clock(now)?;
        let latest = self.latest.ok_or(Error::NoObservation)?;
        self.configuration
            .policy
            .validate_window(latest.window, now)?;
        Ok(latest)
    }
    pub fn live_marks_at(&self, now: RuntimeClock) -> Result<CostMarks, Error> {
        self.fresh(now)?;
        match self.live {
            ReportStatus::Finite(marks) => Ok(marks),
            ReportStatus::Unknown => Err(Error::Unknown),
        }
    }
    fn check_risk_cadence(&self, now: RuntimeClock) -> Result<(), Error> {
        let latest = self.fresh(now)?;
        let policy = self.configuration.policy;
        if now.slot - self.last_runtime_clock.slot > policy.max_update_gap_slots
            || now.seconds()? - self.last_runtime_clock.seconds()? > policy.max_update_gap_seconds
            || now.slot - latest.window.end.slot > policy.max_update_gap_slots
            || now.seconds()? - latest.window.end.seconds()? > policy.max_update_gap_seconds
        {
            return Err(Error::HistoryCadenceExpired);
        }
        Ok(())
    }
    /// Requires current finite freshness plus the configured consecutive completed prefix.
    /// This supplies data only: the caller still computes and authorizes every risk action.
    pub fn risk_evidence_at(&self, now: RuntimeClock) -> Result<RiskEvidence<'_>, Error> {
        self.live_marks_at(now)?;
        self.check_risk_cadence(now)?;
        if self.history.count < self.configuration.policy.minimum_completed_epochs {
            return Err(Error::HistoryWarming);
        }
        Ok(RiskEvidence { state: self })
    }

    /// Validate every fallible operation before mutating. Any Err preserves the entire state.
    pub fn accept(
        &mut self,
        signer: CallerAuthenticatedSigner,
        report: &Report,
        now: RuntimeClock,
    ) -> Result<(), Error> {
        let update = self.prepare(signer, report, now)?;
        // Infallible commit; no account/HTTP/signature operations or callbacks occur here.
        if update.clear_history {
            self.history.clear();
        }
        if let Some(bucket) = update.completed {
            self.history.push(bucket);
        }
        self.last_sequence = report.sequence;
        self.last_runtime_clock = now;
        self.latest = Some(update.metadata);
        self.live = update.live;
        self.pending_epoch = Some(update.metadata.observation_epoch);
        self.pending = update.pending;
        self.pending_tainted = update.pending_tainted;
        Ok(())
    }

    fn prepare(
        &self,
        signer: CallerAuthenticatedSigner,
        report: &Report,
        now: RuntimeClock,
    ) -> Result<PreparedUpdate, Error> {
        self.check_clock(now)?;
        let cfg = &self.configuration;
        if report.version != REPORT_VERSION {
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
        if !cfg.authorizes(&signer.0) {
            return Err(Error::UnauthorizedPublisher);
        }
        if report.publisher != signer.0 {
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
        // No historical/backfilled epochs can be inserted using fresh signatures or sequences.
        let epoch = cfg.policy.epoch(now)?;
        if cfg.policy.epoch(report.window.start)? != epoch
            || cfg.policy.epoch(report.window.end)? != epoch
        {
            return Err(Error::WrongObservationEpoch);
        }
        let epoch_start = cfg.policy.epoch_start(epoch)?;
        let gap_seconds = cfg.policy.max_update_gap_seconds;
        let begins_in_time = report.window.start.seconds()? - epoch_start <= gap_seconds
            && now.seconds()? - epoch_start <= gap_seconds
            && self.configured_at.seconds()? <= epoch_start;
        let same = self.pending_epoch == Some(epoch);
        let adjacent = self.pending_epoch.and_then(|old| old.checked_add(1)) == Some(epoch);
        let continuous = if let Some(previous) = self.latest {
            report.window.end.slot - previous.window.end.slot <= cfg.policy.max_update_gap_slots
                && report.window.end.seconds()? - previous.window.end.seconds()? <= gap_seconds
                && now.slot - self.last_runtime_clock.slot <= cfg.policy.max_update_gap_slots
                && now.seconds()? - self.last_runtime_clock.seconds()? <= gap_seconds
        } else {
            false
        };
        let mut clear_history = false;
        let mut completed = None;
        let mut pending = if same { self.pending } else { None };
        let mut tainted = if same {
            self.pending_tainted
        } else {
            !begins_in_time
        };
        if self.pending_epoch.is_some() && !same {
            let final_in_time = if let Some(bucket) = self.pending {
                epoch_start
                    .checked_sub(bucket.last_observed.seconds()?)
                    .is_some_and(|age| age <= gap_seconds)
                    && epoch_start
                        .checked_sub(self.last_runtime_clock.seconds()?)
                        .is_some_and(|age| age <= gap_seconds)
            } else {
                false
            };
            if adjacent && continuous && !self.pending_tainted && final_in_time {
                completed = self.pending;
            } else {
                clear_history = true;
            }
        } else if same && !continuous {
            clear_history = true;
            tainted = true;
            pending = None;
        }
        match report.status {
            ReportStatus::Unknown => {
                clear_history = true;
                completed = None;
                pending = None;
                tainted = true;
            }
            ReportStatus::Finite(marks) => {
                pending = Some(match pending {
                    Some(bucket) => bucket.extended(report.window, marks)?,
                    None => CompletedBucket::first(epoch, report.window, marks),
                });
            }
        }
        Ok(PreparedUpdate {
            metadata: ReportMetadata {
                sequence: report.sequence,
                publisher: report.publisher,
                window: report.window,
                observation_digest: report.observation_digest,
                observation_epoch: epoch,
            },
            live: report.status,
            pending,
            pending_tainted: tainted,
            clear_history,
            completed,
        })
    }

    /// Trusted-governance transition only; the library cannot authenticate governance.
    /// No identity/q rotation; outstanding debt forbids model or temporal-policy changes.
    /// Publisher rotation is allowed with debt, but resets current data/history and warmup.
    pub fn reconfigure(&mut self, next: Configuration, now: RuntimeClock) -> Result<(), Error> {
        self.check_clock(now)?;
        let old = &self.configuration;
        if next.domain != old.domain || next.quantity != old.quantity {
            return Err(Error::ImmutableBinding);
        }
        if old.epoch.checked_add(1) != Some(next.epoch) {
            return Err(Error::ConfigurationEpochNotAdvanced);
        }
        let model_changed = next.model != old.model;
        let policy_changed = next.policy != old.policy;
        if (model_changed || policy_changed) && self.outstanding_positions != 0 {
            return Err(Error::OutstandingPolicyChange);
        }
        if policy_changed && !model_changed {
            return Err(Error::PolicyChangeRequiresModelEpoch);
        }
        if model_changed && old.model.epoch.checked_add(1) != Some(next.model.epoch) {
            return Err(Error::ModelEpochNotAdvanced);
        }
        // All fallible checks precede the mutation boundary.
        self.configuration = next;
        self.configured_at = now;
        self.last_runtime_clock = now;
        self.latest = None;
        self.live = ReportStatus::Unknown;
        self.pending_epoch = None;
        self.pending = None;
        self.pending_tainted = true;
        self.history.clear();
        Ok(())
    }
}

/// Borrowed view cannot outlive or be mutably reconfigured independently of its state.
/// Every accessor rechecks freshness at actual use. No liquidation permission is returned.
pub struct RiskEvidence<'a> {
    state: &'a ReportState,
}
impl RiskEvidence<'_> {
    pub fn live_marks_at(&self, now: RuntimeClock) -> Result<CostMarks, Error> {
        let marks = self.state.live_marks_at(now)?;
        self.state.check_risk_cadence(now)?;
        if self.state.history.count < self.state.configuration.policy.minimum_completed_epochs {
            return Err(Error::HistoryWarming);
        }
        Ok(marks)
    }
    pub fn completed_bucket_at(
        &self,
        newest_offset: usize,
        now: RuntimeClock,
    ) -> Result<Option<&CompletedBucket>, Error> {
        self.state.live_marks_at(now)?;
        self.state.check_risk_cadence(now)?;
        if self.state.history.count < self.state.configuration.policy.minimum_completed_epochs {
            return Err(Error::HistoryWarming);
        }
        Ok(self.state.history.newest(newest_offset))
    }
    pub fn len_at(&self, now: RuntimeClock) -> Result<u8, Error> {
        self.state.live_marks_at(now)?;
        self.state.check_risk_cadence(now)?;
        if self.state.history.count < self.state.configuration.policy.minimum_completed_epochs {
            return Err(Error::HistoryWarming);
        }
        Ok(self.state.history.count)
    }
    pub const fn quantity(&self) -> u64 {
        self.state.configuration.quantity
    }
    pub const fn model(&self) -> ModelBinding {
        self.state.configuration.model
    }
    pub const fn configuration_epoch(&self) -> u64 {
        self.state.configuration.epoch
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(t: i64) -> RuntimeClock {
        RuntimeClock {
            slot: 100 + u64::try_from(t).unwrap() * 4,
            unix_timestamp: t,
        }
    }
    fn domain() -> Domain {
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
        }
    }
    fn model() -> ModelBinding {
        ModelBinding {
            version: 1,
            epoch: 1,
            policy_hash: [9; 32],
        }
    }
    fn policy() -> HistoryPolicy {
        HistoryPolicy {
            max_age_slots: 400,
            max_age_seconds: 100,
            max_window_slots: 20,
            max_window_seconds: 5,
            max_update_gap_slots: 12,
            max_update_gap_seconds: 3,
            epoch_seconds: 10,
            minimum_completed_epochs: 2,
        }
    }
    fn config_with(policy: HistoryPolicy) -> Configuration {
        Configuration::new(domain(), 1_000, model(), 1, &[[10; 32]], policy).unwrap()
    }
    fn state() -> ReportState {
        ReportState::new(config_with(policy()), clock(0)).unwrap()
    }
    fn signer() -> CallerAuthenticatedSigner {
        CallerAuthenticatedSigner::from_verified_runtime_signer([10; 32])
    }
    fn marks(t: i64) -> CostMarks {
        let n = u64::try_from(t).unwrap();
        CostMarks::new(n + 10, n + 11, n + 12).unwrap()
    }
    fn report(state: &ReportState, t: i64, status: ReportStatus) -> Report {
        let c = state.configuration;
        Report {
            version: REPORT_VERSION,
            domain: c.domain,
            quantity: c.quantity,
            model: c.model,
            configuration_epoch: c.epoch,
            publisher: [10; 32],
            sequence: state.last_sequence + 1,
            window: ObservationWindow {
                start: clock(t),
                end: clock(t),
            },
            observation_digest: [11; 32],
            status,
        }
    }
    fn finite(state: &mut ReportState, t: i64) {
        let r = report(state, t, ReportStatus::Finite(marks(t)));
        state.accept(signer(), &r, clock(t)).unwrap();
    }
    fn assert_rejected(
        state: &mut ReportState,
        r: &Report,
        signer: CallerAuthenticatedSigner,
        now: RuntimeClock,
        expected: Error,
    ) {
        let before = state.clone();
        assert_eq!(state.accept(signer, r, now), Err(expected));
        assert_eq!(*state, before);
    }
    fn warmed() -> ReportState {
        let mut s = state();
        for t in 0..=20 {
            finite(&mut s, t);
        }
        assert_eq!(s.completed_history_len(), 2);
        s
    }

    #[test]
    fn constructors_reject_unset_duplicate_zero_and_unordered_inputs() {
        for args in [
            (0, 1, model(), policy()),
            (1, 0, model(), policy()),
            (
                1,
                1,
                ModelBinding {
                    epoch: 0,
                    ..model()
                },
                policy(),
            ),
            (
                1,
                1,
                ModelBinding {
                    version: 0,
                    ..model()
                },
                policy(),
            ),
            (
                1,
                1,
                ModelBinding {
                    policy_hash: [0; 32],
                    ..model()
                },
                policy(),
            ),
            (
                1,
                1,
                model(),
                HistoryPolicy {
                    epoch_seconds: 0,
                    ..policy()
                },
            ),
            (
                1,
                1,
                model(),
                HistoryPolicy {
                    minimum_completed_epochs: 0,
                    ..policy()
                },
            ),
            (
                1,
                1,
                model(),
                HistoryPolicy {
                    minimum_completed_epochs: 9,
                    ..policy()
                },
            ),
            (
                1,
                1,
                model(),
                HistoryPolicy {
                    max_update_gap_seconds: 0,
                    ..policy()
                },
            ),
        ] {
            assert_eq!(
                Configuration::new(domain(), args.0, args.2, args.1, &[[10; 32]], args.3),
                Err(Error::InvalidConfiguration)
            );
        }
        for keys in [
            &[][..],
            &[[0; 32]][..],
            &[[10; 32], [10; 32]][..],
            &[[10; 32]; 9][..],
        ] {
            assert_eq!(
                Configuration::new(domain(), 1, model(), 1, keys, policy()),
                Err(Error::InvalidConfiguration)
            );
        }
        assert_eq!(
            Configuration::new(
                Domain {
                    base_mint: domain().quote_mint,
                    ..domain()
                },
                1,
                model(),
                1,
                &[[10; 32]],
                policy()
            ),
            Err(Error::InvalidConfiguration)
        );
        assert_eq!(CostMarks::new(0, 1, 2), Err(Error::InvalidMarks));
        assert_eq!(CostMarks::new(2, 1, 3), Err(Error::InvalidMarks));
        assert_eq!(CostMarks::new(1, 3, 2), Err(Error::InvalidMarks));
        assert_eq!(
            CostMarks::new(u64::MAX, u64::MAX, u64::MAX)
                .unwrap()
                .upper(),
            u64::MAX
        );
    }

    #[test]
    fn all_domain_fields_quantity_model_and_authority_are_bound_atomically() {
        let mut s = state();
        let good = report(&s, 0, ReportStatus::Finite(marks(0)));
        let ds = [
            Domain {
                genesis_hash: [99; 32],
                ..domain()
            },
            Domain {
                program: [99; 32],
                ..domain()
            },
            Domain {
                config: [99; 32],
                ..domain()
            },
            Domain {
                market: [99; 32],
                ..domain()
            },
            Domain {
                base_mint: [99; 32],
                ..domain()
            },
            Domain {
                quote_mint: [99; 32],
                ..domain()
            },
            Domain {
                base_token_program: [99; 32],
                ..domain()
            },
            Domain {
                quote_token_program: [99; 32],
                ..domain()
            },
            Domain {
                base_decimals: 7,
                ..domain()
            },
            Domain {
                quote_decimals: 8,
                ..domain()
            },
        ];
        for d in ds {
            assert_rejected(
                &mut s,
                &Report { domain: d, ..good },
                signer(),
                clock(0),
                Error::DomainMismatch,
            );
        }
        for m in [
            ModelBinding {
                version: 2,
                ..model()
            },
            ModelBinding {
                epoch: 2,
                ..model()
            },
            ModelBinding {
                policy_hash: [99; 32],
                ..model()
            },
        ] {
            assert_rejected(
                &mut s,
                &Report { model: m, ..good },
                signer(),
                clock(0),
                Error::ModelMismatch,
            );
        }
        assert_rejected(
            &mut s,
            &Report {
                quantity: 1,
                ..good
            },
            signer(),
            clock(0),
            Error::QuantityMismatch,
        );
        assert_rejected(
            &mut s,
            &Report {
                configuration_epoch: 2,
                ..good
            },
            signer(),
            clock(0),
            Error::ConfigurationEpochMismatch,
        );
        assert_rejected(
            &mut s,
            &Report { version: 2, ..good },
            signer(),
            clock(0),
            Error::Version,
        );
        assert_rejected(
            &mut s,
            &Report {
                publisher: [12; 32],
                ..good
            },
            signer(),
            clock(0),
            Error::PublisherMismatch,
        );
        assert_rejected(
            &mut s,
            &good,
            CallerAuthenticatedSigner::from_verified_runtime_signer([12; 32]),
            clock(0),
            Error::UnauthorizedPublisher,
        );
        assert_rejected(
            &mut s,
            &Report {
                observation_digest: [0; 32],
                ..good
            },
            signer(),
            clock(0),
            Error::MissingCommitment,
        );
    }

    #[test]
    fn source_window_uses_earliest_age_and_rejects_future_wide_and_invalid_time() {
        let mut s = state();
        let r = report(&s, 0, ReportStatus::Finite(marks(0)));
        assert_rejected(
            &mut s,
            &Report {
                window: ObservationWindow {
                    start: clock(2),
                    end: clock(1),
                },
                ..r
            },
            signer(),
            clock(2),
            Error::InvalidWindow,
        );
        assert_rejected(
            &mut s,
            &Report {
                window: ObservationWindow {
                    start: clock(2),
                    end: clock(2),
                },
                ..r
            },
            signer(),
            clock(1),
            Error::FutureObservation,
        );
        assert_rejected(
            &mut s,
            &Report {
                window: ObservationWindow {
                    start: clock(0),
                    end: clock(6),
                },
                ..r
            },
            signer(),
            clock(6),
            Error::WindowTooWide,
        );
        assert_rejected(
            &mut s,
            &r,
            signer(),
            RuntimeClock {
                slot: 100,
                unix_timestamp: -1,
            },
            Error::InvalidClock,
        );
        let mut limited = ReportState::new(
            config_with(HistoryPolicy {
                max_age_seconds: 2,
                max_age_slots: 400,
                ..policy()
            }),
            clock(0),
        )
        .unwrap();
        let r = report(&limited, 0, ReportStatus::Finite(marks(0)));
        assert_rejected(
            &mut limited,
            &Report {
                window: ObservationWindow {
                    start: clock(0),
                    end: clock(3),
                },
                ..r
            },
            signer(),
            clock(3),
            Error::StaleObservation,
        );
        let mut later = ReportState::new(config_with(policy()), clock(2)).unwrap();
        let r = report(&later, 3, ReportStatus::Finite(marks(0)));
        assert_rejected(
            &mut later,
            &Report {
                window: ObservationWindow {
                    start: clock(1),
                    end: clock(3),
                },
                ..r
            },
            signer(),
            clock(3),
            Error::ObservationBeforeConfiguration,
        );
    }

    #[test]
    fn strict_sequences_and_source_progress_never_wrap_or_replay() {
        let mut s = state();
        let first = report(&s, 0, ReportStatus::Finite(marks(0)));
        assert_rejected(
            &mut s,
            &Report {
                sequence: 2,
                ..first
            },
            signer(),
            clock(0),
            Error::Sequence,
        );
        s.accept(signer(), &first, clock(0)).unwrap();
        assert_rejected(&mut s, &first, signer(), clock(0), Error::Sequence);
        let next = Report {
            sequence: 2,
            ..first
        };
        assert_rejected(&mut s, &next, signer(), clock(0), Error::SourceRegression);
        let next = report(&s, 1, ReportStatus::Finite(marks(1)));
        assert_rejected(
            &mut s,
            &next,
            signer(),
            RuntimeClock {
                slot: 99,
                unix_timestamp: 1,
            },
            Error::ClockRegression,
        );
        s.last_sequence = u64::MAX;
        assert_rejected(
            &mut s,
            &Report {
                sequence: 0,
                ..next
            },
            signer(),
            clock(1),
            Error::SequenceExhausted,
        );
    }

    #[test]
    fn only_completed_timely_epochs_enter_history_and_current_marks_are_excluded() {
        let mut s = state();
        for t in 0..=9 {
            finite(&mut s, t);
        }
        assert_eq!(s.completed_history_len(), 0);
        assert!(matches!(
            s.risk_evidence_at(clock(9)),
            Err(Error::HistoryWarming)
        ));
        finite(&mut s, 10);
        assert_eq!(s.completed_history_len(), 1);
        let bucket = s.history.newest(0).unwrap();
        assert_eq!(bucket.epoch, 0);
        assert_eq!(bucket.lower_min, 10);
        assert_eq!(bucket.upper_max, 21);
        assert_eq!(bucket.central_last, 20);
        assert_eq!(bucket.observations, 10);
        for t in 11..=20 {
            finite(&mut s, t);
        }
        let view = s.risk_evidence_at(clock(20)).unwrap();
        assert_eq!(view.quantity(), 1_000);
        assert_eq!(view.model(), model());
        assert_eq!(view.configuration_epoch(), 1);
        assert_eq!(view.len_at(clock(20)), Ok(2));
        assert_eq!(
            view.completed_bucket_at(0, clock(20))
                .unwrap()
                .unwrap()
                .epoch,
            1
        );
        assert_eq!(view.live_marks_at(clock(20)), Ok(marks(20)));
        assert_eq!(view.completed_bucket_at(2, clock(20)), Ok(None));
        // Same epoch, advancing source slots but unchanged time cannot manufacture epochs.
        for n in 1..=8 {
            let mut r = report(&s, 20, ReportStatus::Finite(marks(20)));
            r.window.start.slot += n;
            r.window.end.slot += n;
            let now = r.window.end;
            s.accept(signer(), &r, now).unwrap();
            assert_eq!(s.completed_history_len(), 2);
        }
    }

    #[test]
    fn current_epoch_only_prevents_backfill_and_mid_epoch_configuration_is_not_history() {
        let mut s = state();
        let r = report(&s, 10, ReportStatus::Finite(marks(10)));
        assert_rejected(
            &mut s,
            &r,
            signer(),
            clock(20),
            Error::WrongObservationEpoch,
        );
        let r = Report {
            window: ObservationWindow {
                start: clock(9),
                end: clock(10),
            },
            ..r
        };
        assert_rejected(
            &mut s,
            &r,
            signer(),
            clock(10),
            Error::WrongObservationEpoch,
        );
        let mut s = ReportState::new(config_with(policy()), clock(5)).unwrap();
        for t in 5..=20 {
            finite(&mut s, t);
        }
        assert_eq!(s.completed_history_len(), 1);
        assert_eq!(s.history.newest(0).unwrap().epoch, 1);
    }

    #[test]
    fn unknown_clears_history_even_at_rollover_and_taints_the_entire_epoch() {
        for unknown_at in [20, 21] {
            let mut s = state();
            for t in 0..unknown_at {
                finite(&mut s, t);
            }
            let r = report(&s, unknown_at, ReportStatus::Unknown);
            s.accept(signer(), &r, clock(unknown_at)).unwrap();
            assert_eq!(s.completed_history_len(), 0);
            assert_eq!(s.live_marks_at(clock(unknown_at)), Err(Error::Unknown));
            for t in unknown_at + 1..=30 {
                finite(&mut s, t);
            }
            assert_eq!(
                s.completed_history_len(),
                0,
                "tainted epoch2 must not become completed history"
            );
            assert!(matches!(
                s.risk_evidence_at(clock(30)),
                Err(Error::HistoryWarming)
            ));
            for t in 31..=40 {
                finite(&mut s, t);
            }
            assert_eq!(s.completed_history_len(), 1);
        }
    }

    #[test]
    fn publication_or_source_cadence_expiry_blocks_cached_risk_and_restarts_history() {
        let mut s = warmed();
        let view = s.risk_evidence_at(clock(20)).unwrap();
        // Numeric quote is still inside max_age=100, but history cadence max_gap=3 expired.
        assert_eq!(s.live_marks_at(clock(24)), Ok(marks(20)));
        assert!(matches!(
            s.risk_evidence_at(clock(24)),
            Err(Error::HistoryCadenceExpired)
        ));
        assert_eq!(
            view.live_marks_at(clock(24)),
            Err(Error::HistoryCadenceExpired)
        );
        assert_eq!(view.len_at(clock(24)), Err(Error::HistoryCadenceExpired));
        assert_eq!(
            view.completed_bucket_at(0, clock(24)),
            Err(Error::HistoryCadenceExpired)
        );
        assert_eq!(s.live_marks_at(clock(121)), Err(Error::StaleObservation));
        finite(&mut s, 24);
        assert_eq!(s.completed_history_len(), 0);
        for t in 25..=30 {
            finite(&mut s, t);
        }
        assert_eq!(s.completed_history_len(), 0);
        for t in 31..=40 {
            finite(&mut s, t);
        }
        assert_eq!(s.completed_history_len(), 1);
        finite(&mut s, 60);
        assert_eq!(
            s.completed_history_len(),
            0,
            "missing entire epochs cannot be filled"
        );
    }

    #[test]
    fn ring_retains_only_latest_consecutive_completed_buckets() {
        let mut s = state();
        for t in 0..=150 {
            finite(&mut s, t);
        }
        let view = s.risk_evidence_at(clock(150)).unwrap();
        assert_eq!(view.len_at(clock(150)), Ok(8));
        assert_eq!(
            view.completed_bucket_at(0, clock(150))
                .unwrap()
                .unwrap()
                .epoch,
            14
        );
        assert_eq!(
            view.completed_bucket_at(7, clock(150))
                .unwrap()
                .unwrap()
                .epoch,
            7
        );
        assert_eq!(view.completed_bucket_at(8, clock(150)), Ok(None));
    }

    #[test]
    fn publisher_rotation_preserves_debt_quantity_but_resets_prices_and_requires_fresh_source() {
        let mut s = warmed();
        s.register_outstanding().unwrap();
        s.register_outstanding().unwrap();
        let next = Configuration::new(domain(), 1_000, model(), 2, &[[12; 32]], policy()).unwrap();
        s.reconfigure(next, clock(21)).unwrap();
        assert_eq!(s.outstanding_positions(), 2);
        assert!(!s.can_retire());
        assert_eq!(s.completed_history_len(), 0);
        assert_eq!(s.live_marks_at(clock(21)), Err(Error::NoObservation));
        let mut r = report(&s, 21, ReportStatus::Finite(marks(21)));
        r.publisher = [12; 32];
        assert_rejected(
            &mut s,
            &r,
            signer(),
            clock(21),
            Error::UnauthorizedPublisher,
        );
        let auth = CallerAuthenticatedSigner::from_verified_runtime_signer([12; 32]);
        assert_rejected(
            &mut s,
            &Report {
                window: ObservationWindow {
                    start: clock(20),
                    end: clock(21),
                },
                ..r
            },
            auth,
            clock(21),
            Error::ObservationBeforeConfiguration,
        );
        assert_rejected(
            &mut s,
            &Report {
                configuration_epoch: 1,
                ..r
            },
            auth,
            clock(21),
            Error::ConfigurationEpochMismatch,
        );
        s.accept(auth, &r, clock(21)).unwrap();
        assert!(matches!(
            s.risk_evidence_at(clock(21)),
            Err(Error::HistoryWarming)
        ));
        s.release_outstanding().unwrap();
        assert!(!s.can_retire());
        s.release_outstanding().unwrap();
        assert!(s.can_retire());
        let before = s.clone();
        assert_eq!(
            s.release_outstanding(),
            Err(Error::OutstandingCountUnderflow)
        );
        assert_eq!(s, before);
        s.outstanding_positions = u64::MAX;
        let before = s.clone();
        assert_eq!(
            s.register_outstanding(),
            Err(Error::OutstandingCountExhausted)
        );
        assert_eq!(s, before);
    }

    #[test]
    fn reconfiguration_freezes_economic_policy_with_debt_and_never_changes_domain_or_q() {
        let mut s = state();
        let advanced = ModelBinding {
            epoch: 2,
            policy_hash: [55; 32],
            ..model()
        };
        for next in [
            Configuration::new(
                Domain {
                    market: [99; 32],
                    ..domain()
                },
                1_000,
                model(),
                2,
                &[[10; 32]],
                policy(),
            )
            .unwrap(),
            Configuration::new(domain(), 2_000, model(), 2, &[[10; 32]], policy()).unwrap(),
        ] {
            let before = s.clone();
            assert_eq!(s.reconfigure(next, clock(1)), Err(Error::ImmutableBinding));
            assert_eq!(s, before);
        }
        let changed_policy = HistoryPolicy {
            epoch_seconds: 20,
            ..policy()
        };
        let next =
            Configuration::new(domain(), 1_000, advanced, 2, &[[10; 32]], changed_policy).unwrap();
        s.register_outstanding().unwrap();
        let before = s.clone();
        assert_eq!(
            s.reconfigure(next, clock(1)),
            Err(Error::OutstandingPolicyChange)
        );
        assert_eq!(s, before);
        s.release_outstanding().unwrap();
        let same_model =
            Configuration::new(domain(), 1_000, model(), 2, &[[10; 32]], changed_policy).unwrap();
        let before = s.clone();
        assert_eq!(
            s.reconfigure(same_model, clock(1)),
            Err(Error::PolicyChangeRequiresModelEpoch)
        );
        assert_eq!(s, before);
        for epoch in [1, 3, u64::MAX] {
            let bad = Configuration::new(
                domain(),
                1_000,
                advanced,
                epoch,
                &[[10; 32]],
                changed_policy,
            )
            .unwrap();
            let before = s.clone();
            assert_eq!(
                s.reconfigure(bad, clock(1)),
                Err(Error::ConfigurationEpochNotAdvanced)
            );
            assert_eq!(s, before);
        }
        let wrong_model = ModelBinding {
            epoch: 3,
            ..advanced
        };
        let bad = Configuration::new(domain(), 1_000, wrong_model, 2, &[[10; 32]], changed_policy)
            .unwrap();
        let before = s.clone();
        assert_eq!(
            s.reconfigure(bad, clock(1)),
            Err(Error::ModelEpochNotAdvanced)
        );
        assert_eq!(s, before);
        s.reconfigure(next, clock(1)).unwrap();
        assert_eq!(s.configuration().model(), advanced);
        assert_eq!(s.last_sequence(), 0);
    }

    #[test]
    fn late_fallible_bucket_extension_preserves_every_state_field() {
        let mut s = state();
        finite(&mut s, 0);
        s.pending.as_mut().unwrap().observations = u64::MAX;
        let r = report(&s, 1, ReportStatus::Finite(marks(1)));
        assert_rejected(
            &mut s,
            &r,
            signer(),
            clock(1),
            Error::ObservationCountExhausted,
        );
    }

    #[test]
    fn exhaustive_unknown_finite_sequences_match_independent_epoch_model() {
        let p = HistoryPolicy {
            epoch_seconds: 2,
            minimum_completed_epochs: 1,
            max_update_gap_seconds: 1,
            max_update_gap_slots: 4,
            ..policy()
        };
        for mask in 0u32..4096 {
            let mut s = ReportState::new(config_with(p), clock(0)).unwrap();
            let mut expected = 0u8;
            for t in 0u32..12 {
                let is_finite = mask & (1 << t) != 0;
                if t > 0 && t % 2 == 0 {
                    let previous_whole = mask & (1 << (t - 1)) != 0 && mask & (1 << (t - 2)) != 0;
                    if previous_whole {
                        expected = (expected + 1).min(8);
                    } else {
                        expected = 0;
                    }
                }
                if !is_finite {
                    expected = 0;
                }
                let time = i64::from(t);
                let status = if is_finite {
                    ReportStatus::Finite(marks(time))
                } else {
                    ReportStatus::Unknown
                };
                let r = report(&s, time, status);
                s.accept(signer(), &r, clock(time)).unwrap();
                let mut encoded = [0; codec::STATE_BYTES];
                codec::encode(&s, &mut encoded).unwrap();
                let restored = codec::decode(&encoded).unwrap();
                let mut encoded_again = [0; codec::STATE_BYTES];
                codec::encode(&restored, &mut encoded_again).unwrap();
                assert_eq!(encoded, encoded_again);
                assert_eq!(restored.completed_history_len(), expected);
                assert_eq!(s.completed_history_len(), expected, "mask={mask} t={t}");
                assert_eq!(
                    s.risk_evidence_at(clock(time)).is_ok(),
                    is_finite && expected >= 1,
                    "mask={mask} t={t}"
                );
            }
        }
    }

    #[test]
    fn integer_boundaries_do_not_wrap_clock_epoch_or_cost_units() {
        let start = RuntimeClock {
            slot: u64::MAX - 8,
            unix_timestamp: i64::MAX - 3,
        };
        let p = HistoryPolicy {
            epoch_seconds: 2,
            minimum_completed_epochs: 1,
            max_age_slots: u64::MAX,
            max_age_seconds: u64::MAX,
            max_update_gap_slots: 4,
            max_update_gap_seconds: 1,
            ..policy()
        };
        let mut s = ReportState::new(config_with(p), start).unwrap();
        for n in 0..=2 {
            let now = RuntimeClock {
                slot: start.slot + n * 4,
                unix_timestamp: start.unix_timestamp + i64::try_from(n).unwrap(),
            };
            let r = Report {
                version: 1,
                domain: domain(),
                quantity: 1_000,
                model: model(),
                configuration_epoch: 1,
                publisher: [10; 32],
                sequence: n + 1,
                window: ObservationWindow {
                    start: now,
                    end: now,
                },
                observation_digest: [11; 32],
                status: ReportStatus::Finite(CostMarks::new(u64::MAX, u64::MAX, u64::MAX).unwrap()),
            };
            s.accept(signer(), &r, now).unwrap();
        }
        assert_eq!(
            s.completed_history_len(),
            1,
            "an exact boundary can complete a timely epoch near integer limits"
        );
        let now = RuntimeClock {
            slot: u64::MAX,
            unix_timestamp: i64::MAX - 1,
        };
        let r = Report {
            sequence: 4,
            ..report(&s, 0, ReportStatus::Unknown)
        };
        assert_rejected(
            &mut s,
            &Report {
                window: ObservationWindow {
                    start: now,
                    end: now,
                },
                ..r
            },
            signer(),
            now,
            Error::SourceRegression,
        );
    }
    #[test]
    fn codec_rejects_bad_lengths_tags_padding_and_semantic_corruption() {
        use codec::{STATE_BYTES, decode, encode};
        let mut empty = [0; STATE_BYTES];
        encode(&state(), &mut empty).unwrap();
        assert_eq!(STATE_BYTES, 1488);
        for size in 0..STATE_BYTES {
            assert!(decode(&empty[..size]).is_err());
        }
        let mut extra = [0; STATE_BYTES + 1];
        extra[..STATE_BYTES].copy_from_slice(&empty);
        assert_eq!(decode(&extra), Err(codec::CodecError::Length));
        for index in [
            0, 8, 328, 361, 682, 683, 795, 796, 820, 821, 829, 830, 902, 903, 904,
        ] {
            let mut bad = empty;
            bad[index] = 2;
            assert!(
                decode(&bad).is_err(),
                "noncanonical/invalid byte at {index}"
            );
        }
        let mut current = state();
        finite(&mut current, 0);
        let mut bytes = [0; STATE_BYTES];
        encode(&current, &mut bytes).unwrap();
        for (offset, length) in [(683, 8), (691, 32), (755, 32), (894, 8)] {
            let mut bad = bytes;
            bad[offset..offset + length].fill(0);
            assert!(decode(&bad).is_err(), "invalid semantic field at {offset}");
        }
        // Late invalid state must not partially write caller storage.
        current.pending.as_mut().unwrap().observations = 0;
        let mut output = [0xA5; STATE_BYTES];
        assert!(encode(&current, &mut output).is_err());
        assert_eq!(output, [0xA5; STATE_BYTES]);
    }

    #[test]
    fn codec_roundtrips_wrapped_history_and_debt_preserving_rotation_canonically() {
        let mut s = state();
        s.register_outstanding().unwrap();
        for t in 0..=120 {
            finite(&mut s, t);
        }
        let mut bytes = [0; codec::STATE_BYTES];
        codec::encode(&s, &mut bytes).unwrap();
        let restored = codec::decode(&bytes).unwrap();
        assert_eq!(restored.configuration(), s.configuration());
        assert_eq!(restored.outstanding_positions(), 1);
        assert_eq!(restored.last_sequence(), s.last_sequence());
        let old = s.risk_evidence_at(clock(120)).unwrap();
        let new = restored.risk_evidence_at(clock(120)).unwrap();
        for i in 0..HISTORY_CAPACITY {
            assert_eq!(
                old.completed_bucket_at(i, clock(120)),
                new.completed_bucket_at(i, clock(120))
            );
        }
        let next = Configuration::new(domain(), 1_000, model(), 2, &[[12; 32]], policy()).unwrap();
        s.reconfigure(next, clock(121)).unwrap();
        codec::encode(&s, &mut bytes).unwrap();
        let restored = codec::decode(&bytes).unwrap();
        assert_eq!(restored.outstanding_positions(), 1);
        assert_eq!(restored.last_sequence(), s.last_sequence());
        assert_eq!(
            restored.live_marks_at(clock(121)),
            Err(Error::NoObservation)
        );
        let mut again = [0; codec::STATE_BYTES];
        codec::encode(&restored, &mut again).unwrap();
        assert_eq!(bytes, again);
    }
}
