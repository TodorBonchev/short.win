//! Private operator-history collector and action-time snapshot preparation.
//!
//! This reuses the report core's chronological collection rules OFF CHAIN. Persisting
//! these observations does not establish public on-chain timing or fair prices. The
//! trusted operator must qualify exact-q source evidence before calling `record`;
//! there is deliberately no conversion from an API candidate to financial marks.
//! No keypair, HTTP endpoint, transaction signing/submission or idle publication exists
//! here. Observation sequence is local; snapshot sequence comes from current chain state.
//!
//! The Linux store uses a stable separate lock inode plus atomic replace and directory
//! fsync. A crash may leave an unreferenced `.pending-*` evidence file; it is never used
//! automatically. Corrupt/mismatched existing state is never reset. SHA256 checks detect
//! corruption, not forgery by the trusted host/user. Raw source evidence belongs in the
//! operator's retained journal under the supplied evidence digest.

use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use sha2::{Digest, Sha256};
use shortwin_price_report::{
    self as core, Address, CallerAuthenticatedSigner, Configuration, CostMarks, ObservationWindow,
    Report, ReportState, ReportStatus, RuntimeClock,
    trusted_snapshot::{AttestedBucket, AttestedHistory, SnapshotReport},
};

const MAGIC: &[u8; 8] = b"SWHIST02";
const METRIC_COUNT: usize = core::HISTORY_CAPACITY + 1;
/// Fixed format: magic, core codec, nine bounded epoch-gap records and digest chain.
pub const STATE_BYTES: usize = 8 + core::codec::STATE_BYTES + 1 + METRIC_COUNT * 24 + 32 + 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryError {
    InvalidObservation,
    Core(core::Error),
    Codec,
    Corrupt,
    ConfigurationMismatch,
    MissingGapEvidence,
    Snapshot,
    UnsafePath,
    UnsafeParent,
    UnsafeFile,
    PathChanged,
    UnsupportedPlatform,
    AlreadyExists,
    Missing,
    Busy,
    Io,
    Oversized,
}
impl std::fmt::Display for HistoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "off-chain price history: {self:?}")
    }
}
impl std::error::Error for HistoryError {}
impl From<core::Error> for HistoryError {
    fn from(error: core::Error) -> Self {
        Self::Core(error)
    }
}

/// Operator-qualified observation. Construction is an explicit TRUST boundary, not
/// source verification. Exact input/output units, route/account policy, source age,
/// executable bid/ask bounds and retained raw evidence must already be qualified.
/// An ordinary Jupiter Build response alone does not satisfy this contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperatorObservation {
    window: ObservationWindow,
    evidence_digest: Address,
    status: ReportStatus,
}
impl OperatorObservation {
    pub fn qualified_finite(
        window: ObservationWindow,
        evidence_digest: Address,
        marks: CostMarks,
    ) -> Result<Self, HistoryError> {
        Self::new(window, evidence_digest, ReportStatus::Finite(marks))
    }
    /// Timeouts, absent routes, unqualified candidates and unavailable sources are
    /// Unknown. They cannot establish an insolvency/default or liquidation boolean.
    pub fn unknown(
        window: ObservationWindow,
        evidence_digest: Address,
    ) -> Result<Self, HistoryError> {
        Self::new(window, evidence_digest, ReportStatus::Unknown)
    }
    fn new(
        window: ObservationWindow,
        evidence_digest: Address,
        status: ReportStatus,
    ) -> Result<Self, HistoryError> {
        if evidence_digest == [0; 32] {
            return Err(HistoryError::InvalidObservation);
        }
        Ok(Self {
            window,
            evidence_digest,
            status,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EpochGap {
    epoch: u64,
    slots: u64,
    seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryCollector {
    state: ReportState,
    /// Canonical oldest-first bounded prefix, including the current epoch.
    gaps: Vec<EpochGap>,
    chain_digest: Address,
}
impl HistoryCollector {
    pub fn new(configuration: Configuration, now: RuntimeClock) -> Result<Self, HistoryError> {
        Ok(Self {
            state: ReportState::new(configuration, now)?,
            gaps: Vec::new(),
            chain_digest: [0; 32],
        })
    }
    pub fn configuration(&self) -> &Configuration {
        self.state.configuration()
    }
    pub fn observation_sequence(&self) -> u64 {
        self.state.last_sequence()
    }
    pub fn completed_epochs(&self) -> u8 {
        self.state.completed_history_len()
    }
    pub fn evidence_chain_digest(&self) -> Address {
        self.chain_digest
    }

    /// `now` must come from the service's actual observed chain clock, not a request
    /// body's claimed clock. Core validation enforces monotonic runtime/source clocks,
    /// current source epoch, freshness, bounded windows, Unknown/gap warmup resets.
    /// No error mutates the collector. This function performs no on-chain write.
    pub fn record(
        &mut self,
        observation: OperatorObservation,
        now: RuntimeClock,
    ) -> Result<(), HistoryError> {
        let cfg = *self.state.configuration();
        let previous = self.state.latest_metadata();
        let publisher = cfg.publishers()[0];
        let report = Report {
            version: core::REPORT_VERSION,
            domain: cfg.domain(),
            quantity: cfg.quantity(),
            model: cfg.model(),
            configuration_epoch: cfg.epoch(),
            publisher,
            sequence: self
                .state
                .last_sequence()
                .checked_add(1)
                .ok_or(HistoryError::Core(core::Error::SequenceExhausted))?,
            window: observation.window,
            observation_digest: observation.evidence_digest,
            status: observation.status,
        };
        // Stage the full update, including codec/digest work, before committing.
        let mut next = self.clone();
        next.state.accept(
            CallerAuthenticatedSigner::from_verified_runtime_signer(publisher),
            &report,
            now,
        )?;
        let epoch = u64::try_from(now.unix_timestamp)
            .map_err(|_| HistoryError::InvalidObservation)?
            / cfg.policy().epoch_seconds;
        if next.gaps.last().is_none_or(|gap| gap.epoch != epoch) {
            if next.gaps.len() == METRIC_COUNT {
                next.gaps.remove(0);
            }
            next.gaps.push(EpochGap {
                epoch,
                slots: 0,
                seconds: 0,
            });
        } else if let (Some(previous), Some(gap)) = (previous, next.gaps.last_mut()) {
            // Attested maxima describe SOURCE endpoint gaps INTERNAL to one bucket.
            // Receiver checks cross-epoch endpoints separately. Actual collection-runtime
            // cadence is independently enforced by ReportState::accept above.
            gap.slots = gap
                .slots
                .max(observation.window.end.slot - previous.window.end.slot);
            gap.seconds = gap.seconds.max(
                u64::try_from(
                    observation.window.end.unix_timestamp - previous.window.end.unix_timestamp,
                )
                .map_err(|_| HistoryError::InvalidObservation)?,
            );
        }
        if matches!(observation.status, ReportStatus::Unknown) {
            // Core already cleared history; keep only current metrics.
            next.gaps.retain(|gap| gap.epoch == epoch);
        }
        let mut state_bytes = [0; core::codec::STATE_BYTES];
        core::codec::encode(&next.state, &mut state_bytes).map_err(|_| HistoryError::Codec)?;
        let mut hasher = Sha256::new();
        hasher.update(b"shortwin-private-history-v2");
        hasher.update(self.chain_digest);
        hasher.update(observation.evidence_digest);
        hasher.update(state_bytes);
        next.chain_digest = hasher.finalize().into();
        *self = next;
        Ok(())
    }

    /// Build an operator attestation only when an action requests a fresh cache.
    /// `publication_sequence` is independently read as current on-chain sequence+1;
    /// collecting samples does not consume publication sequences or SOL fees.
    pub fn prepare_snapshot(
        &self,
        publisher: Address,
        publication_sequence: u64,
        now: RuntimeClock,
    ) -> Result<SnapshotReport, HistoryError> {
        let cfg = self.state.configuration();
        if !cfg.publishers().contains(&publisher) || publication_sequence == 0 {
            return Err(HistoryError::InvalidObservation);
        }
        let metadata = self
            .state
            .latest_metadata()
            .ok_or(core::Error::NoObservation)?;
        let status = match self.state.live_marks_at(now) {
            Ok(marks) => ReportStatus::Finite(marks),
            Err(core::Error::Unknown) => ReportStatus::Unknown,
            Err(error) => return Err(error.into()),
        };
        let mut buckets = [None; core::HISTORY_CAPACITY];
        if matches!(status, ReportStatus::Finite(_)) {
            match self.state.risk_evidence_at(now) {
                Ok(evidence) => {
                    let count = usize::from(evidence.len_at(now)?);
                    for (oldest, target) in buckets.iter_mut().take(count).enumerate() {
                        let bucket = *evidence
                            .completed_bucket_at(count - 1 - oldest, now)?
                            .ok_or(HistoryError::MissingGapEvidence)?;
                        let gap = self
                            .gaps
                            .iter()
                            .find(|gap| gap.epoch == bucket.epoch)
                            .ok_or(HistoryError::MissingGapEvidence)?;
                        *target = Some(AttestedBucket {
                            bucket,
                            largest_gap_slots: gap.slots,
                            largest_gap_seconds: gap.seconds,
                        });
                    }
                }
                // A finite snapshot with no qualifying history remains warming;
                // consumer evidence_at must still reject it for risk admission.
                Err(core::Error::HistoryWarming | core::Error::HistoryCadenceExpired) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(SnapshotReport {
            version: core::trusted_snapshot::TRUSTED_SNAPSHOT_VERSION,
            domain: cfg.domain(),
            quantity: cfg.quantity(),
            model: cfg.model(),
            configuration_epoch: cfg.epoch(),
            publisher,
            sequence: publication_sequence,
            window: metadata.window,
            observation_digest: self.chain_digest,
            status,
            history: AttestedHistory::new(buckets).map_err(|_| HistoryError::Snapshot)?,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, HistoryError> {
        if self.gaps.len() > METRIC_COUNT {
            return Err(HistoryError::Corrupt);
        }
        let mut bytes = Vec::with_capacity(STATE_BYTES);
        bytes.extend_from_slice(MAGIC);
        let mut state = [0; core::codec::STATE_BYTES];
        core::codec::encode(&self.state, &mut state).map_err(|_| HistoryError::Codec)?;
        bytes.extend_from_slice(&state);
        bytes.push(u8::try_from(self.gaps.len()).map_err(|_| HistoryError::Corrupt)?);
        for index in 0..METRIC_COUNT {
            let gap = self.gaps.get(index).copied().unwrap_or(EpochGap {
                epoch: 0,
                slots: 0,
                seconds: 0,
            });
            for value in [gap.epoch, gap.slots, gap.seconds] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&self.chain_digest);
        let checksum = Sha256::digest(&bytes);
        bytes.extend_from_slice(&checksum);
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8], expected: &Configuration) -> Result<Self, HistoryError> {
        if bytes.len() != STATE_BYTES
            || &bytes[..8] != MAGIC
            || Sha256::digest(&bytes[..STATE_BYTES - 32]).as_slice() != &bytes[STATE_BYTES - 32..]
        {
            return Err(HistoryError::Corrupt);
        }
        let state = core::codec::decode(&bytes[8..8 + core::codec::STATE_BYTES])
            .map_err(|_| HistoryError::Codec)?;
        if state.configuration() != expected {
            return Err(HistoryError::ConfigurationMismatch);
        }
        if state.outstanding_positions() != 0 {
            return Err(HistoryError::Corrupt);
        }
        let mut at = 8 + core::codec::STATE_BYTES;
        let count = usize::from(bytes[at]);
        at += 1;
        if count > METRIC_COUNT {
            return Err(HistoryError::Corrupt);
        }
        let mut gaps = Vec::with_capacity(count);
        for index in 0..METRIC_COUNT {
            let mut fields = [0; 3];
            for field in &mut fields {
                *field = u64::from_le_bytes(
                    bytes[at..at + 8]
                        .try_into()
                        .map_err(|_| HistoryError::Corrupt)?,
                );
                at += 8;
            }
            if index < count {
                if gaps
                    .last()
                    .is_some_and(|prior: &EpochGap| prior.epoch >= fields[0])
                {
                    return Err(HistoryError::Corrupt);
                }
                gaps.push(EpochGap {
                    epoch: fields[0],
                    slots: fields[1],
                    seconds: fields[2],
                });
            } else if fields != [0; 3] {
                return Err(HistoryError::Corrupt);
            }
        }
        let chain_digest: Address = bytes[at..at + 32]
            .try_into()
            .map_err(|_| HistoryError::Corrupt)?;
        if (state.last_sequence() == 0) != (chain_digest == [0; 32])
            || (state.last_sequence() == 0) != gaps.is_empty()
        {
            return Err(HistoryError::Corrupt);
        }
        if let Some(metadata) = state.latest_metadata()
            && gaps
                .last()
                .is_none_or(|gap| gap.epoch != metadata.observation_epoch)
        {
            return Err(HistoryError::Corrupt);
        }
        Ok(Self {
            state,
            gaps,
            chain_digest,
        })
    }
}

/// Stable-lock Linux persistence for one exact-q/config/model collector. The caller
/// must serialize bounded tasks; Busy is returned immediately rather than queued.
/// One private0700 directory is required per store. This API never creates parents.
pub struct HistoryStore {
    path: PathBuf,
    lock_path: PathBuf,
    expected: Configuration,
}
impl HistoryStore {
    pub fn new(path: PathBuf, expected: Configuration) -> Result<Self, HistoryError> {
        if !path.is_absolute()
            || path.file_name().is_none()
            || path.as_os_str().as_encoded_bytes().len() > 4096
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(HistoryError::UnsafePath);
        }
        let lock_path = path.with_extension("history-lock");
        if lock_path == path {
            return Err(HistoryError::UnsafePath);
        }
        Ok(Self {
            path,
            lock_path,
            expected,
        })
    }
    pub fn create(&self, now: RuntimeClock) -> Result<HistoryCollector, HistoryError> {
        let (_lock, uid) = self.lock()?;
        match fs::symlink_metadata(&self.path) {
            Ok(_) => return Err(HistoryError::AlreadyExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(HistoryError::Io),
        }
        let collector = HistoryCollector::new(self.expected, now)?;
        self.replace(&collector.encode()?, uid)?;
        Ok(collector)
    }
    pub fn load(&self) -> Result<HistoryCollector, HistoryError> {
        let (_lock, uid) = self.lock()?;
        self.read(uid)
    }
    pub fn record(
        &self,
        observation: OperatorObservation,
        now: RuntimeClock,
    ) -> Result<HistoryCollector, HistoryError> {
        let (_lock, uid) = self.lock()?;
        let mut collector = self.read(uid)?;
        collector.record(observation, now)?;
        self.replace(&collector.encode()?, uid)?;
        Ok(collector)
    }
    fn lock(&self) -> Result<(File, u32), HistoryError> {
        let uid = validate_parent(&self.path)?;
        let file = open_private(&self.lock_path, uid, true)?;
        file.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => HistoryError::Busy,
            TryLockError::Error(_) => HistoryError::Io,
        })?;
        validate_parent(&self.path)?;
        validate_file(&file, &self.lock_path, uid)?;
        if file.metadata().map_err(|_| HistoryError::Io)?.len() != 0 {
            return Err(HistoryError::Corrupt);
        }
        Ok((file, uid))
    }
    fn read(&self, uid: u32) -> Result<HistoryCollector, HistoryError> {
        let file = open_private(&self.path, uid, false)?;
        if file.metadata().map_err(|_| HistoryError::Io)?.len() > STATE_BYTES as u64 {
            return Err(HistoryError::Oversized);
        }
        let mut bytes = Vec::with_capacity(STATE_BYTES);
        file.take(STATE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| HistoryError::Io)?;
        HistoryCollector::decode(&bytes, &self.expected)
    }
    #[cfg(target_os = "linux")]
    fn replace(&self, bytes: &[u8], uid: u32) -> Result<(), HistoryError> {
        use std::os::unix::fs::OpenOptionsExt;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        validate_parent(&self.path)?;
        if let Ok(metadata) = fs::symlink_metadata(&self.path) {
            if metadata.file_type().is_symlink() {
                return Err(HistoryError::UnsafeFile);
            }
            let opened = open_private(&self.path, uid, false)?;
            validate_file(&opened, &self.path, uid)?;
        }
        let name = self
            .path
            .file_name()
            .ok_or(HistoryError::UnsafePath)?
            .to_string_lossy();
        let temp = self.path.with_file_name(format!(
            ".{name}.pending-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(0o400000 | 0o4000)
            .open(&temp)
            .map_err(|_| HistoryError::Io)?;
        validate_file(&file, &temp, uid)?;
        file.write_all(bytes).map_err(|_| HistoryError::Io)?;
        file.sync_all().map_err(|_| HistoryError::Io)?;
        validate_parent(&self.path)?;
        validate_file(&file, &temp, uid)?;
        fs::rename(&temp, &self.path).map_err(|_| HistoryError::Io)?;
        File::open(self.path.parent().ok_or(HistoryError::UnsafePath)?)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| HistoryError::Io)?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    fn replace(&self, _: &[u8], _: u32) -> Result<(), HistoryError> {
        Err(HistoryError::UnsupportedPlatform)
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn validate_parent(path: &Path) -> Result<u32, HistoryError> {
    use std::os::unix::fs::MetadataExt;
    let uid = fs::metadata("/proc/self")
        .map_err(|_| HistoryError::Io)?
        .uid();
    let parent = path.parent().ok_or(HistoryError::UnsafePath)?;
    let mut prefix = PathBuf::new();
    for component in parent.components() {
        prefix.push(component);
        let metadata = fs::symlink_metadata(&prefix).map_err(|_| HistoryError::UnsafeParent)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || (metadata.uid() != 0 && metadata.uid() != uid)
            || (metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0)
        {
            return Err(HistoryError::UnsafeParent);
        }
    }
    let metadata = fs::symlink_metadata(parent).map_err(|_| HistoryError::UnsafeParent)?;
    if metadata.uid() != uid || metadata.mode() & 0o7777 != 0o700 {
        return Err(HistoryError::UnsafeParent);
    }
    Ok(uid)
}
#[cfg(target_os = "linux")]
pub(crate) fn open_private(path: &Path, uid: u32, create: bool) -> Result<File, HistoryError> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(create)
        .custom_flags(0o400000 | 0o4000);
    let file = if create {
        match options.create_new(true).mode(0o600).open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(0o400000 | 0o4000)
                .open(path)
                .map_err(|_| HistoryError::UnsafeFile)?,
            Err(_) => return Err(HistoryError::Io),
        }
    } else {
        options.open(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                HistoryError::Missing
            } else {
                HistoryError::UnsafeFile
            }
        })?
    };
    validate_file(&file, path, uid)?;
    Ok(file)
}
#[cfg(target_os = "linux")]
pub(crate) fn validate_file(file: &File, path: &Path, uid: u32) -> Result<(), HistoryError> {
    use std::os::unix::fs::MetadataExt;
    let opened = file.metadata().map_err(|_| HistoryError::Io)?;
    let current = fs::symlink_metadata(path).map_err(|_| HistoryError::PathChanged)?;
    if !opened.is_file()
        || opened.uid() != uid
        || opened.mode() & 0o7777 != 0o600
        || opened.nlink() != 1
        || current.file_type().is_symlink()
    {
        return Err(HistoryError::UnsafeFile);
    }
    if opened.ino() != current.ino() || opened.dev() != current.dev() {
        return Err(HistoryError::PathChanged);
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
pub(crate) fn validate_parent(_: &Path) -> Result<u32, HistoryError> {
    Err(HistoryError::UnsupportedPlatform)
}
#[cfg(not(target_os = "linux"))]
pub(crate) fn open_private(_: &Path, _: u32, _: bool) -> Result<File, HistoryError> {
    Err(HistoryError::UnsupportedPlatform)
}
#[cfg(not(target_os = "linux"))]
pub(crate) fn validate_file(_: &File, _: &Path, _: u32) -> Result<(), HistoryError> {
    Err(HistoryError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shortwin_price_report::{
        Domain, HistoryPolicy, ModelBinding, trusted_snapshot::SnapshotState,
    };

    fn clock(t: i64) -> RuntimeClock {
        RuntimeClock {
            slot: 100 + (t as u64) * 4,
            unix_timestamp: t,
        }
    }
    fn config() -> Configuration {
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
            1_000,
            ModelBinding {
                version: 1,
                epoch: 1,
                policy_hash: [9; 32],
            },
            1,
            &[[10; 32]],
            HistoryPolicy {
                max_age_slots: 16,
                max_age_seconds: 4,
                max_window_slots: 4,
                max_window_seconds: 1,
                max_update_gap_slots: 12,
                max_update_gap_seconds: 3,
                epoch_seconds: 10,
                minimum_completed_epochs: 2,
            },
        )
        .unwrap()
    }
    fn observation(t: i64) -> OperatorObservation {
        OperatorObservation::qualified_finite(
            ObservationWindow {
                start: clock(t),
                end: clock(t),
            },
            [11; 32],
            CostMarks::new(90, 100, 110).unwrap(),
        )
        .unwrap()
    }
    fn warmed() -> HistoryCollector {
        let mut state = HistoryCollector::new(config(), clock(0)).unwrap();
        for t in 0..=20 {
            state.record(observation(t), clock(t)).unwrap();
        }
        state
    }
    #[test]
    fn offchain_collection_does_not_consume_publication_sequences() {
        let state = warmed();
        assert_eq!(state.observation_sequence(), 21);
        let first = state.prepare_snapshot([10; 32], 1, clock(20)).unwrap();
        let second = state.prepare_snapshot([10; 32], 2, clock(20)).unwrap();
        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        assert_eq!(first.history, second.history);
        assert_eq!(state.observation_sequence(), 21);
        let mut receiver = SnapshotState::new(config(), clock(0)).unwrap();
        receiver
            .accept(
                CallerAuthenticatedSigner::from_verified_runtime_signer([10; 32]),
                &first,
                clock(20),
            )
            .unwrap();
        assert!(receiver.evidence_at(clock(20), 1).is_ok());
    }
    #[test]
    fn finite_warming_and_unknown_never_borrow_finished_history() {
        let mut state = HistoryCollector::new(config(), clock(0)).unwrap();
        state.record(observation(0), clock(0)).unwrap();
        let snapshot = state.prepare_snapshot([10; 32], 1, clock(0)).unwrap();
        assert!(snapshot.history.buckets().iter().all(Option::is_none));
        let mut state = warmed();
        state
            .record(
                OperatorObservation::unknown(
                    ObservationWindow {
                        start: clock(21),
                        end: clock(21),
                    },
                    [12; 32],
                )
                .unwrap(),
                clock(21),
            )
            .unwrap();
        assert_eq!(state.completed_epochs(), 0);
        let unknown = state.prepare_snapshot([10; 32], 8, clock(21)).unwrap();
        assert_eq!(unknown.status, ReportStatus::Unknown);
        assert!(unknown.history.buckets().iter().all(Option::is_none));
        state.record(observation(22), clock(22)).unwrap();
        assert!(
            state
                .prepare_snapshot([10; 32], 9, clock(22))
                .unwrap()
                .history
                .buckets()
                .iter()
                .all(Option::is_none)
        );
    }
    #[test]
    fn source_or_runtime_gap_resets_warmup_and_source_regression_is_atomic() {
        let mut state = warmed();
        let before = state.clone();
        assert!(state.record(observation(19), clock(21)).is_err());
        assert_eq!(state, before);
        state.record(observation(25), clock(25)).unwrap();
        assert_eq!(state.completed_epochs(), 0);
        assert!(
            state
                .prepare_snapshot([10; 32], 1, clock(25))
                .unwrap()
                .history
                .buckets()
                .iter()
                .all(Option::is_none)
        );
        assert!(state.prepare_snapshot([10; 32], 2, clock(30)).is_err());
    }
    #[test]
    fn bounded_ring_roundtrip_preserves_exact_snapshot_and_gap_assertions() {
        let mut state = HistoryCollector::new(config(), clock(0)).unwrap();
        for t in 0..=100 {
            state.record(observation(t), clock(t)).unwrap();
        }
        assert_eq!(state.completed_epochs(), 8);
        assert_eq!(state.gaps.len(), 9);
        let encoded = state.encode().unwrap();
        assert_eq!(encoded.len(), STATE_BYTES);
        let restored = HistoryCollector::decode(&encoded, &config()).unwrap();
        assert_eq!(restored.encode().unwrap(), encoded);
        let snapshot = restored.prepare_snapshot([10; 32], 3, clock(100)).unwrap();
        for bucket in snapshot.history.buckets().iter().flatten() {
            assert_eq!(bucket.largest_gap_slots, 4);
            assert_eq!(bucket.largest_gap_seconds, 1);
        }
        let mut receiver = SnapshotState::new(config(), clock(0)).unwrap();
        // Actual cache seq requires1, independently of101collected observations.
        let mut first = snapshot;
        first.sequence = 1;
        receiver
            .accept(
                CallerAuthenticatedSigner::from_verified_runtime_signer([10; 32]),
                &first,
                clock(100),
            )
            .unwrap();
        assert!(receiver.evidence_at(clock(100), 1).is_ok());
    }
    #[test]
    fn one_sample_epochs_and_variable_source_windows_match_receiver_gap_contract() {
        let old = config();
        let one = Configuration::new(
            old.domain(),
            old.quantity(),
            old.model(),
            old.epoch(),
            old.publishers(),
            HistoryPolicy {
                epoch_seconds: 1,
                ..old.policy()
            },
        )
        .unwrap();
        let mut collector = HistoryCollector::new(one, clock(0)).unwrap();
        for t in 0..=3 {
            collector.record(observation(t), clock(t)).unwrap();
        }
        let report = collector.prepare_snapshot([10; 32], 1, clock(3)).unwrap();
        for b in report.history.iter() {
            assert_eq!(b.bucket.observations, 1);
            assert_eq!(b.largest_gap_slots, 0);
            assert_eq!(b.largest_gap_seconds, 0);
        }
        let mut receiver = SnapshotState::new(one, clock(0)).unwrap();
        receiver
            .accept(
                CallerAuthenticatedSigner::from_verified_runtime_signer([10; 32]),
                &report,
                clock(3),
            )
            .unwrap();
        assert!(receiver.evidence_at(clock(3), 1).is_ok());

        let mut collector = HistoryCollector::new(config(), clock(0)).unwrap();
        collector
            .record(
                OperatorObservation::qualified_finite(
                    ObservationWindow {
                        start: clock(0),
                        end: clock(1),
                    },
                    [11; 32],
                    CostMarks::new(90, 100, 110).unwrap(),
                )
                .unwrap(),
                clock(1),
            )
            .unwrap();
        for t in 2..=8 {
            collector.record(observation(t), clock(t + 1)).unwrap();
        }
        for t in 9..=20 {
            collector.record(observation(t), clock(t)).unwrap();
        }
        let report = collector.prepare_snapshot([10; 32], 1, clock(20)).unwrap();
        assert_eq!(report.history.iter().next().unwrap().largest_gap_seconds, 1);
        let mut receiver = SnapshotState::new(config(), clock(0)).unwrap();
        receiver
            .accept(
                CallerAuthenticatedSigner::from_verified_runtime_signer([10; 32]),
                &report,
                clock(20),
            )
            .unwrap();
        assert!(receiver.evidence_at(clock(20), 1).is_ok());
    }

    #[test]
    fn corruption_truncation_noncanonical_tail_and_configuration_mismatch_fail_closed() {
        let state = warmed();
        let bytes = state.encode().unwrap();
        for offset in [0, 8, 200, bytes.len() - 1] {
            let mut bad = bytes.clone();
            bad[offset] ^= 1;
            assert!(HistoryCollector::decode(&bad, &config()).is_err());
        }
        for length in [0, 1, STATE_BYTES - 1] {
            assert!(HistoryCollector::decode(&bytes[..length], &config()).is_err());
        }
        let mut other = config();
        other = Configuration::new(
            other.domain(),
            2_000,
            other.model(),
            other.epoch(),
            other.publishers(),
            other.policy(),
        )
        .unwrap();
        assert_eq!(
            HistoryCollector::decode(&bytes, &other),
            Err(HistoryError::ConfigurationMismatch)
        );
        let mut bad = bytes;
        let unused_metric = 8 + core::codec::STATE_BYTES + 1 + 8 * 24;
        bad[unused_metric] = 1;
        let checksum = Sha256::digest(&bad[..STATE_BYTES - 32]);
        bad[STATE_BYTES - 32..].copy_from_slice(&checksum);
        assert_eq!(
            HistoryCollector::decode(&bad, &config()),
            Err(HistoryError::Corrupt)
        );
    }

    #[cfg(target_os = "linux")]
    mod storage {
        use super::*;
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
        use std::time::{SystemTime, UNIX_EPOCH};
        fn retry<T>(
            mut operation: impl FnMut() -> Result<T, HistoryError>,
        ) -> Result<T, HistoryError> {
            for _ in 0..200 {
                match operation() {
                    Err(HistoryError::Busy) => {
                        std::thread::sleep(std::time::Duration::from_millis(1))
                    }
                    result => return result,
                }
            }
            Err(HistoryError::Busy)
        }
        struct Directory(PathBuf);
        impl Directory {
            fn new() -> Self {
                static NEXT: AtomicU64 = AtomicU64::new(0);
                let nonce = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let path = std::env::temp_dir().join(format!(
                    "shortwin-history-{}-{nonce}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir(&path).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                Self(path)
            }
            fn path(&self) -> PathBuf {
                self.0.join("history.state")
            }
            fn store(&self) -> HistoryStore {
                HistoryStore::new(self.path(), config()).unwrap()
            }
        }
        impl Drop for Directory {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        #[test]
        fn persisted_updates_survive_reopen_and_use_atomic_replace_with_stable_lock() {
            let directory = Directory::new();
            let store = directory.store();
            retry(|| store.create(clock(0))).unwrap();
            let lock_inode = fs::metadata(&store.lock_path).unwrap().ino();
            let original_file = File::open(directory.path()).unwrap();
            let initial_inode = original_file.metadata().unwrap().ino();
            for t in 0..=20 {
                retry(|| directory.store().record(observation(t), clock(t))).unwrap();
            }
            let loaded = retry(|| store.load()).unwrap();
            assert_eq!(loaded, warmed());
            assert_ne!(fs::metadata(directory.path()).unwrap().ino(), initial_inode);
            assert_eq!(fs::metadata(&store.lock_path).unwrap().ino(), lock_inode);
            assert_eq!(
                fs::metadata(directory.path()).unwrap().mode() & 0o7777,
                0o600
            );
            assert_eq!(store.create(clock(0)), Err(HistoryError::AlreadyExists));
        }
        #[test]
        fn interrupted_unreferenced_file_does_not_replace_good_state_or_reset_history() {
            let directory = Directory::new();
            let store = directory.store();
            retry(|| store.create(clock(0))).unwrap();
            retry(|| store.record(observation(0), clock(0))).unwrap();
            let before = fs::read(directory.path()).unwrap();
            fs::write(
                directory.0.join(".history.state.pending-crashed"),
                b"partial",
            )
            .unwrap();
            assert_eq!(retry(|| store.load()).unwrap().observation_sequence(), 1);
            assert_eq!(fs::read(directory.path()).unwrap(), before);
            assert!(directory.0.join(".history.state.pending-crashed").exists());
        }
        #[test]
        fn corrupt_existing_state_and_regressed_records_never_overwrite_evidence() {
            let directory = Directory::new();
            let store = directory.store();
            retry(|| store.create(clock(0))).unwrap();
            retry(|| store.record(observation(1), clock(1))).unwrap();
            let before = fs::read(directory.path()).unwrap();
            assert!(store.record(observation(0), clock(2)).is_err());
            assert_eq!(fs::read(directory.path()).unwrap(), before);
            for bytes in [Vec::new(), b"damaged".to_vec(), vec![0; STATE_BYTES + 1]] {
                fs::write(directory.path(), &bytes).unwrap();
                assert!(store.load().is_err());
                assert!(store.record(observation(2), clock(2)).is_err());
                assert_eq!(fs::read(directory.path()).unwrap(), bytes);
                assert_eq!(store.create(clock(0)), Err(HistoryError::AlreadyExists));
            }
        }
        #[test]
        fn lock_contention_unsafe_permissions_links_and_foreign_paths_are_refused() {
            let directory = Directory::new();
            let store = directory.store();
            retry(|| store.create(clock(0))).unwrap();
            let (lock, _) = store.lock().unwrap();
            assert_eq!(store.load(), Err(HistoryError::Busy));
            drop(lock);
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o644)).unwrap();
            assert_eq!(store.load(), Err(HistoryError::UnsafeFile));
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o600)).unwrap();
            let backup = directory.0.join("saved");
            fs::rename(directory.path(), &backup).unwrap();
            symlink(&backup, directory.path()).unwrap();
            assert!(store.load().is_err());
            fs::remove_file(directory.path()).unwrap();
            fs::hard_link(&backup, directory.path()).unwrap();
            assert_eq!(store.load(), Err(HistoryError::UnsafeFile));
            assert!(HistoryStore::new(PathBuf::from("relative"), config()).is_err());
            assert!(HistoryStore::new(PathBuf::from("/tmp/../history"), config()).is_err());
        }
    }
}
