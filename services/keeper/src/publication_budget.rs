//! Crash-safe worst-fee reservations for EXPLICIT on-demand price publications.
//!
//! UTC-day and rolling burst limits are operator configuration, not a security
//! boundary against the trusted service UID or direct key use. Initialize once;
//! missing/corrupt/mismatched state never resets during reserve. Reservations are
//! never refunded, including simulation failures after reservation or lost replies.
//! One pending signature blocks new signatures until confirmed or safely reconciled
//! after finalized blockheight has passed its last-valid blockheight.
use crate::offchain_price_history::{HistoryError, open_private, validate_file, validate_parent};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{Read, Write},
    path::{Component, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const MAGIC: &[u8; 8] = b"SWPUB001";
const MAX_BYTES: usize = 65_536;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationLimits {
    pub daily_max_lamports: u64,
    pub burst_window_seconds: u64,
    pub burst_attempts: u16,
}
impl PublicationLimits {
    pub fn validate(self) -> Result<(), PublicationError> {
        if self.daily_max_lamports == 0
            || self.burst_window_seconds == 0
            || self.burst_window_seconds > 86_400
            || self.burst_attempts == 0
            || self.burst_attempts > 1_000
        {
            return Err(PublicationError::Configuration);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingPublication {
    pub signature: Vec<u8>,
    pub recent_blockhash: [u8; 32],
    pub last_valid_block_height: u64,
    pub receiver: [u8; 32],
    pub sequence: u64,
    pub reserved_fee_lamports: u64,
}
impl PendingPublication {
    fn valid(&self) -> bool {
        self.signature.len() == 64
            && self.signature.iter().any(|b| *b != 0)
            && self.recent_blockhash != [0; 32]
            && self.receiver != [0; 32]
            && self.sequence > 0
            && self.reserved_fee_lamports > 0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionKind {
    ConfirmedSuccess,
    ConfirmedFailure,
    ExpiredAbsent,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationResolution {
    pub signature: Vec<u8>,
    pub receiver: [u8; 32],
    pub sequence: u64,
    pub reserved_fee_lamports: u64,
    pub slot: Option<u64>,
    pub actual_fee_lamports: Option<u64>,
    pub kind: ResolutionKind,
}
impl PublicationResolution {
    fn valid(&self) -> bool {
        self.signature.len() == 64
            && self.signature.iter().any(|b| *b != 0)
            && self.receiver != [0; 32]
            && self.sequence > 0
            && self.reserved_fee_lamports > 0
            && match self.kind {
                ResolutionKind::ExpiredAbsent => {
                    self.slot.is_none() && self.actual_fee_lamports.is_none()
                }
                _ => self.slot.is_some_and(|slot| slot > 0),
            }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationError {
    Configuration,
    UnsafePath,
    File,
    Missing,
    AlreadyExists,
    Busy,
    Io,
    Corrupt,
    PolicyMismatch,
    ClockRegression,
    DailyLimit,
    BurstLimit,
    Pending,
    SignatureMismatch,
}
impl std::fmt::Display for PublicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "price publication budget: {self:?}")
    }
}
impl std::error::Error for PublicationError {}
fn file_error(error: HistoryError) -> PublicationError {
    match error {
        HistoryError::Missing => PublicationError::Missing,
        HistoryError::Busy => PublicationError::Busy,
        _ => PublicationError::File,
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u16,
    limits: PublicationLimits,
    day: u64,
    last_checked_seconds: u64,
    reserved_lamports: u64,
    attempts: Vec<u64>,
    pending: Option<PendingPublication>,
    last_resolution: Option<PublicationResolution>,
}
impl State {
    fn validate(&self, expected: PublicationLimits) -> Result<(), PublicationError> {
        if self.limits != expected {
            return Err(PublicationError::PolicyMismatch);
        }
        if self.last_resolution.as_ref().is_some_and(|r| !r.valid())
            || self.version != 1
            || self.day != self.last_checked_seconds / 86_400
            || self.reserved_lamports > self.limits.daily_max_lamports
            || self.attempts.len() > usize::from(self.limits.burst_attempts)
            || self.attempts.windows(2).any(|w| w[0] > w[1])
            || self
                .attempts
                .last()
                .is_some_and(|t| *t > self.last_checked_seconds)
            || self.pending.as_ref().is_some_and(|p| {
                !p.valid() || p.reserved_fee_lamports > self.limits.daily_max_lamports
            })
        {
            return Err(PublicationError::Corrupt);
        }
        Ok(())
    }
    fn at(&mut self, now: u64) -> Result<(), PublicationError> {
        if now < self.last_checked_seconds {
            return Err(PublicationError::ClockRegression);
        }
        if now / 86_400 != self.day {
            self.day = now / 86_400;
            // An old pending signature may land in this day. Conservatively carry
            // its maximum reservation across every day boundary until reconciled.
            self.reserved_lamports = self.pending.as_ref().map_or(0, |p| p.reserved_fee_lamports);
        }
        self.last_checked_seconds = now;
        Ok(())
    }
    fn encode(&self) -> Result<Vec<u8>, PublicationError> {
        self.validate(self.limits)?;
        let mut bytes = MAGIC.to_vec();
        bytes.extend(serde_json::to_vec(self).map_err(|_| PublicationError::Corrupt)?);
        let sum = Sha256::digest(&bytes);
        bytes.extend_from_slice(&sum);
        if bytes.len() > MAX_BYTES {
            return Err(PublicationError::Corrupt);
        }
        Ok(bytes)
    }
    fn decode(bytes: &[u8], expected: PublicationLimits) -> Result<Self, PublicationError> {
        if bytes.len() < 40
            || bytes.len() > MAX_BYTES
            || &bytes[..8] != MAGIC
            || Sha256::digest(&bytes[..bytes.len() - 32]).as_slice() != &bytes[bytes.len() - 32..]
        {
            return Err(PublicationError::Corrupt);
        }
        let body = &bytes[8..bytes.len() - 32];
        let state: State = serde_json::from_slice(body).map_err(|_| PublicationError::Corrupt)?;
        if serde_json::to_vec(&state).map_err(|_| PublicationError::Corrupt)? != body {
            return Err(PublicationError::Corrupt);
        }
        state.validate(expected)?;
        Ok(state)
    }
}
/// Separate stable lock inode; atomically replaced state file, synced before send.
pub struct PublicationBudget {
    path: PathBuf,
    lock_path: PathBuf,
    limits: PublicationLimits,
}
impl PublicationBudget {
    pub fn new(path: PathBuf, limits: PublicationLimits) -> Result<Self, PublicationError> {
        limits.validate()?;
        if !path.is_absolute()
            || path.file_name().is_none()
            || path.as_os_str().as_encoded_bytes().len() > 4_096
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(PublicationError::UnsafePath);
        }
        let lock_path = path.with_extension("publication-lock");
        if lock_path == path {
            return Err(PublicationError::UnsafePath);
        }
        Ok(Self {
            path,
            lock_path,
            limits,
        })
    }
    /// Explicit commissioning action. Normal reserve never initializes missing state.
    pub fn initialize(&self, now: u64) -> Result<(), PublicationError> {
        let (_lock, uid) = self.lock()?;
        match fs::symlink_metadata(&self.path) {
            Ok(_) => return Err(PublicationError::AlreadyExists),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(PublicationError::Io),
        }
        self.write(
            &State {
                version: 1,
                limits: self.limits,
                day: now / 86_400,
                last_checked_seconds: now,
                reserved_lamports: 0,
                attempts: Vec::new(),
                pending: None,
                last_resolution: None,
            },
            uid,
        )
    }
    pub fn pending(&self) -> Result<Option<PendingPublication>, PublicationError> {
        let (_lock, uid) = self.lock()?;
        Ok(self.read(uid)?.pending)
    }
    pub fn reserve(&self, now: u64, pending: PendingPublication) -> Result<(), PublicationError> {
        if !pending.valid() {
            return Err(PublicationError::Configuration);
        }
        let (_lock, uid) = self.lock()?;
        let mut state = self.read(uid)?;
        if state.pending.is_some() {
            return Err(PublicationError::Pending);
        }
        state.at(now)?;
        state
            .attempts
            .retain(|t| now - *t < self.limits.burst_window_seconds);
        if state.attempts.len() >= usize::from(self.limits.burst_attempts) {
            return Err(PublicationError::BurstLimit);
        }
        let reserved = state
            .reserved_lamports
            .checked_add(pending.reserved_fee_lamports)
            .ok_or(PublicationError::DailyLimit)?;
        if reserved > self.limits.daily_max_lamports {
            return Err(PublicationError::DailyLimit);
        }
        state.reserved_lamports = reserved;
        state.attempts.push(now);
        state.pending = Some(pending);
        self.write(&state, uid)
    }
    /// Call only after confirmed status, or None status AND finalized height beyond
    /// the stored validity horizon. This API itself cannot verify RPC evidence.
    pub(crate) fn complete(
        &self,
        resolution: PublicationResolution,
        now: u64,
    ) -> Result<(), PublicationError> {
        let (_lock, uid) = self.lock()?;
        let mut state = self.read(uid)?;
        if !resolution.valid()
            || state.pending.as_ref().is_none_or(|pending| {
                pending.signature != resolution.signature
                    || pending.receiver != resolution.receiver
                    || pending.sequence != resolution.sequence
                    || pending.reserved_fee_lamports != resolution.reserved_fee_lamports
            })
        {
            return Err(PublicationError::SignatureMismatch);
        }
        state.at(now)?;
        state.last_resolution = Some(resolution);
        state.pending = None;
        self.write(&state, uid)
    }
    /// Last resolution is committed atomically with clearing pending. Process death
    /// before stdout cannot erase the last public signature/fee/status receipt.
    pub fn last_resolution(&self) -> Result<Option<PublicationResolution>, PublicationError> {
        let (_lock, uid) = self.lock()?;
        Ok(self.read(uid)?.last_resolution)
    }

    fn lock(&self) -> Result<(File, u32), PublicationError> {
        let uid = validate_parent(&self.path).map_err(file_error)?;
        let file = open_private(&self.lock_path, uid, true).map_err(file_error)?;
        file.try_lock().map_err(|e| match e {
            TryLockError::WouldBlock => PublicationError::Busy,
            _ => PublicationError::Io,
        })?;
        validate_parent(&self.path).map_err(file_error)?;
        validate_file(&file, &self.lock_path, uid).map_err(file_error)?;
        if file.metadata().map_err(|_| PublicationError::Io)?.len() != 0 {
            return Err(PublicationError::Corrupt);
        }
        Ok((file, uid))
    }
    fn read(&self, uid: u32) -> Result<State, PublicationError> {
        let file = open_private(&self.path, uid, false).map_err(file_error)?;
        if file.metadata().map_err(|_| PublicationError::Io)?.len() > MAX_BYTES as u64 {
            return Err(PublicationError::Corrupt);
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| PublicationError::Io)?;
        State::decode(&bytes, self.limits)
    }
    #[cfg(target_os = "linux")]
    fn write(&self, state: &State, uid: u32) -> Result<(), PublicationError> {
        use std::os::unix::fs::OpenOptionsExt;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let bytes = state.encode()?;
        validate_parent(&self.path).map_err(file_error)?;
        if fs::symlink_metadata(&self.path).is_ok() {
            let file = open_private(&self.path, uid, false).map_err(file_error)?;
            validate_file(&file, &self.path, uid).map_err(file_error)?;
        }
        let filename = self
            .path
            .file_name()
            .ok_or(PublicationError::UnsafePath)?
            .to_string_lossy();
        let temp = self.path.with_file_name(format!(
            ".{filename}.pending-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(0o400000 | 0o4000)
            .open(&temp)
            .map_err(|_| PublicationError::Io)?;
        validate_file(&file, &temp, uid).map_err(file_error)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| PublicationError::Io)?;
        validate_parent(&self.path).map_err(file_error)?;
        validate_file(&file, &temp, uid).map_err(file_error)?;
        fs::rename(&temp, &self.path).map_err(|_| PublicationError::Io)?;
        File::open(self.path.parent().ok_or(PublicationError::UnsafePath)?)
            .and_then(|f| f.sync_all())
            .map_err(|_| PublicationError::Io)
    }
    #[cfg(not(target_os = "linux"))]
    fn write(&self, _: &State, _: u32) -> Result<(), PublicationError> {
        Err(PublicationError::File)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    fn limits() -> PublicationLimits {
        PublicationLimits {
            daily_max_lamports: 100,
            burst_window_seconds: 60,
            burst_attempts: 2,
        }
    }
    fn pending(id: u8, fee: u64) -> PendingPublication {
        PendingPublication {
            signature: vec![id; 64],
            recent_blockhash: [2; 32],
            last_valid_block_height: 100,
            receiver: [3; 32],
            sequence: u64::from(id),
            reserved_fee_lamports: fee,
        }
    }
    fn resolution(id: u8, fee: u64) -> PublicationResolution {
        PublicationResolution {
            signature: vec![id; 64],
            receiver: [3; 32],
            sequence: u64::from(id),
            reserved_fee_lamports: fee,
            slot: Some(123),
            actual_fee_lamports: None,
            kind: ResolutionKind::ConfirmedSuccess,
        }
    }
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "shortwin-publication-budget-{}-{nonce}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
        fn budget(&self) -> PublicationBudget {
            PublicationBudget::new(self.0.join("fees.state"), limits()).unwrap()
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    // Other existingkeepertests fork/exec; inherited locks can briefly beBusy.
    fn retry<T>(
        mut op: impl FnMut() -> Result<T, PublicationError>,
    ) -> Result<T, PublicationError> {
        for _ in 0..200 {
            match op() {
                Err(PublicationError::Busy) => {
                    std::thread::sleep(std::time::Duration::from_millis(1))
                }
                r => return r,
            }
        }
        Err(PublicationError::Busy)
    }
    #[test]
    fn initialize_is_explicit_and_existing_missing_corrupt_state_never_resets_on_reserve() {
        let d = Directory::new();
        let b = d.budget();
        assert_eq!(
            retry(|| b.reserve(10, pending(1, 10))),
            Err(PublicationError::Missing)
        );
        retry(|| b.initialize(10)).unwrap();
        assert_eq!(
            retry(|| b.initialize(10)),
            Err(PublicationError::AlreadyExists)
        );
        for bytes in [Vec::new(), b"broken".to_vec(), vec![0; MAX_BYTES + 1]] {
            fs::write(&b.path, &bytes).unwrap();
            assert!(retry(|| b.reserve(11, pending(1, 10))).is_err());
            assert_eq!(fs::read(&b.path).unwrap(), bytes);
        }
    }
    #[test]
    fn pending_survives_reopen_blocks_new_signatures_and_never_refunds_reservation() {
        let d = Directory::new();
        let b = d.budget();
        retry(|| b.initialize(10)).unwrap();
        retry(|| b.reserve(10, pending(1, 60))).unwrap();
        assert_eq!(
            retry(|| d.budget().pending()).unwrap(),
            Some(pending(1, 60))
        );
        assert_eq!(
            retry(|| b.reserve(11, pending(2, 20))),
            Err(PublicationError::Pending)
        );
        assert_eq!(
            retry(|| b.complete(resolution(2, 60), 12)),
            Err(PublicationError::SignatureMismatch)
        );
        retry(|| b.complete(resolution(1, 60), 12)).unwrap();
        assert_eq!(
            retry(|| b.last_resolution()).unwrap(),
            Some(resolution(1, 60))
        );
        assert_eq!(
            retry(|| b.reserve(13, pending(2, 41))),
            Err(PublicationError::DailyLimit)
        );
        retry(|| b.reserve(13, pending(2, 40))).unwrap();
    }
    #[test]
    fn exact_rolling_burst_bound_survives_completion_and_restart() {
        let d = Directory::new();
        let b = d.budget();
        retry(|| b.initialize(100)).unwrap();
        retry(|| b.reserve(100, pending(1, 1))).unwrap();
        retry(|| b.complete(resolution(1, 1), 100)).unwrap();
        retry(|| b.reserve(101, pending(2, 1))).unwrap();
        retry(|| b.complete(resolution(2, 1), 101)).unwrap();
        assert_eq!(
            retry(|| d.budget().reserve(159, pending(3, 1))),
            Err(PublicationError::BurstLimit)
        );
        retry(|| d.budget().reserve(160, pending(3, 1))).unwrap();
    }
    #[test]
    fn midnight_pending_reservation_is_conservatively_carried_into_landing_day() {
        let d = Directory::new();
        let b = d.budget();
        retry(|| b.initialize(86_399)).unwrap();
        retry(|| b.reserve(86_399, pending(1, 70))).unwrap();
        retry(|| b.complete(resolution(1, 70), 86_401)).unwrap();
        assert_eq!(
            retry(|| b.reserve(86_402, pending(2, 31))),
            Err(PublicationError::DailyLimit)
        );
        retry(|| b.reserve(86_402, pending(2, 30))).unwrap();
        retry(|| b.complete(resolution(2, 30), 86_403)).unwrap();
        retry(|| b.reserve(172_800, pending(3, 100))).unwrap();
    }
    #[test]
    fn clock_policy_signature_and_corrupt_checksum_rejections_preserve_bytes() {
        let d = Directory::new();
        let b = d.budget();
        retry(|| b.initialize(100)).unwrap();
        let before = fs::read(&b.path).unwrap();
        assert_eq!(
            retry(|| b.reserve(99, pending(1, 1))),
            Err(PublicationError::ClockRegression)
        );
        assert_eq!(fs::read(&b.path).unwrap(), before);
        let other = PublicationBudget::new(
            b.path.clone(),
            PublicationLimits {
                daily_max_lamports: 101,
                ..limits()
            },
        )
        .unwrap();
        assert_eq!(
            retry(|| other.reserve(100, pending(1, 1))),
            Err(PublicationError::PolicyMismatch)
        );
        assert_eq!(fs::read(&b.path).unwrap(), before);
        let mut bad = before;
        bad[10] ^= 1;
        fs::write(&b.path, &bad).unwrap();
        assert_eq!(
            retry(|| b.reserve(101, pending(1, 1))),
            Err(PublicationError::Corrupt)
        );
        assert_eq!(fs::read(&b.path).unwrap(), bad);
    }
    #[test]
    fn atomic_replace_stable_lock_and_last_resolution_are_persisted_together() {
        let d = Directory::new();
        let b = d.budget();
        retry(|| b.initialize(100)).unwrap();
        let initial = File::open(&b.path).unwrap();
        let lock_inode = fs::metadata(&b.lock_path).unwrap().ino();
        retry(|| b.reserve(100, pending(1, 20))).unwrap();
        let mut result = resolution(1, 20);
        result.kind = ResolutionKind::ConfirmedFailure;
        result.actual_fee_lamports = Some(5);
        retry(|| b.complete(result.clone(), 101)).unwrap();
        assert_eq!(retry(|| d.budget().pending()).unwrap(), None);
        assert_eq!(
            retry(|| d.budget().last_resolution()).unwrap(),
            Some(result)
        );
        assert_ne!(
            fs::metadata(&b.path).unwrap().ino(),
            initial.metadata().unwrap().ino()
        );
        assert_eq!(fs::metadata(&b.lock_path).unwrap().ino(), lock_inode);
        assert_eq!(fs::metadata(&b.path).unwrap().mode() & 0o7777, 0o600);
    }
}
