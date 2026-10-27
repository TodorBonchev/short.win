//! Cooperative, shared rolling-one-second request reservations on one Linux host.
//!
//! All managed keeper/price processes for one provider account must use the SAME
//! local file, limits and OS service user. This is an operating throttle, not an
//! authorization boundary against that user, outside API consumers or host clock
//! manipulation. Monthly credits are not counted. No credential/request body is
//! stored: only limits, a checked clock and bounded timestamp/class records.
//!
//! File::try_lock (stable since Rust1.89) never waits for a contended lock. Writes
//! stay on the locked inode and sync before a reservation is returned. Existing
//! empty/corrupt/oversized/mismatched state is never reset. Call from a bounded
//! blocking worker in asynchronous code; filesystem I/O itself is synchronous.

use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const WINDOW_MS: u64 = 1_000;
pub const MAX_REQUESTS_PER_WINDOW: u16 = 1_000;
pub const MAX_STATE_BYTES: usize = 16_384;
const MAGIC: &[u8; 8] = b"SWJQ0001";
const HEADER_BYTES: usize = 28;
const RECORD_BYTES: usize = 9;
const CHECKSUM_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestClass {
    Background,
    Exit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetLimits {
    pub total_per_second: u16,
    pub background_per_second: u16,
    pub reserved_exit_per_second: u16,
}
impl BudgetLimits {
    pub fn validate(self) -> Result<(), BudgetError> {
        if self.total_per_second == 0
            || self.total_per_second > MAX_REQUESTS_PER_WINDOW
            || self.background_per_second == 0
            || self.reserved_exit_per_second == 0
            || u32::from(self.background_per_second) + u32::from(self.reserved_exit_per_second)
                > u32::from(self.total_per_second)
        {
            return Err(BudgetError::InvalidConfig);
        }
        Ok(())
    }
}

/// Private paths are not included in Debug/error output. Limits must match across processes.
#[derive(Clone)]
pub struct BudgetConfig {
    pub state_file: PathBuf,
    pub limits: BudgetLimits,
}
impl BudgetConfig {
    pub fn validate(&self) -> Result<(), BudgetError> {
        self.limits.validate()?;
        if !self.state_file.is_absolute()
            || self.state_file.file_name().is_none()
            || self.state_file.as_os_str().as_encoded_bytes().len() > 4096
            || self.state_file.as_os_str().as_encoded_bytes().contains(&0)
            || self
                .state_file
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Err(BudgetError::UnsafePath);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetError {
    InvalidConfig,
    UnsupportedPlatform,
    UnsafePath,
    UnsafeParent,
    UnsafeFile,
    PathChanged,
    Busy,
    Io,
    Oversized,
    Corrupt,
    PolicyMismatch,
    Clock,
    ClockRegression,
    RateLimited { retry_after_ms: u64 },
}
impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Jupiter request budget: {self:?}")
    }
}
impl std::error::Error for BudgetError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reservation {
    pub at_unix_ms: u64,
    pub class: RequestClass,
    pub used_total: u16,
    pub used_background: u16,
}

/// Share with Arc inside a process; the file serializes distinct independent processes.
/// There is no retry loop or in-memory queue. The parent directory must already be private0700.
pub struct SharedRequestBudget {
    config: BudgetConfig,
}
impl SharedRequestBudget {
    /// Pure configuration check; permissions/inode/state are rechecked on every reservation.
    pub fn new(config: BudgetConfig) -> Result<Self, BudgetError> {
        config.validate()?;
        Ok(Self { config })
    }
    pub fn limits(&self) -> BudgetLimits {
        self.config.limits
    }
    pub fn try_reserve(&self, class: RequestClass) -> Result<Reservation, BudgetError> {
        self.with_clock(class, unix_ms)
    }
    fn with_clock(
        &self,
        class: RequestClass,
        clock: impl FnOnce() -> Result<u64, BudgetError>,
    ) -> Result<Reservation, BudgetError> {
        let uid = validate_parent(&self.config.state_file)?;
        let (mut file, created) = open_private(&self.config.state_file, uid)?;
        file.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => BudgetError::Busy,
            TryLockError::Error(_) => BudgetError::Io,
        })?;
        validate_parent(&self.config.state_file)?;
        validate_file(&file, &self.config.state_file, uid)?;
        // Read time AFTER acquiring the lock: competing processes cannot make a
        // pre-lock timestamp appear to regress merely by winning the lock first.
        let now = clock()?;
        if now.checked_add(WINDOW_MS).is_none() {
            return Err(BudgetError::Clock);
        }
        let metadata = file.metadata().map_err(|_| BudgetError::Io)?;
        if metadata.len() > MAX_STATE_BYTES as u64 {
            return Err(BudgetError::Oversized);
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_STATE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| BudgetError::Io)?;
        if bytes.len() > MAX_STATE_BYTES {
            return Err(BudgetError::Oversized);
        }
        let mut state = if created {
            if !bytes.is_empty() {
                return Err(BudgetError::Corrupt);
            }
            State {
                last_checked_ms: now,
                records: Vec::new(),
            }
        } else {
            State::decode(&bytes, self.config.limits)?
        };
        if now < state.last_checked_ms {
            return Err(BudgetError::ClockRegression);
        }
        state.last_checked_ms = now;
        state
            .records
            .retain(|record| now - record.at_ms < WINDOW_MS);
        let background = state
            .records
            .iter()
            .filter(|record| record.class == RequestClass::Background)
            .count();
        let total_full = state.records.len() >= usize::from(self.config.limits.total_per_second);
        let background_full = class == RequestClass::Background
            && background >= usize::from(self.config.limits.background_per_second);
        let result = if total_full || background_full {
            let total_wait = if total_full {
                remaining_ms(state.records[0].at_ms, now)?
            } else {
                0
            };
            let background_wait = if background_full {
                remaining_ms(
                    state
                        .records
                        .iter()
                        .find(|record| record.class == RequestClass::Background)
                        .ok_or(BudgetError::Corrupt)?
                        .at_ms,
                    now,
                )?
            } else {
                0
            };
            Err(BudgetError::RateLimited {
                retry_after_ms: total_wait.max(background_wait),
            })
        } else {
            state.records.push(Record { at_ms: now, class });
            Ok(Reservation {
                at_unix_ms: now,
                class,
                used_total: state.records.len() as u16,
                used_background: (background + usize::from(class == RequestClass::Background))
                    as u16,
            })
        };
        // Denial still advances the checked clock and prunes expired records. No
        // HTTP is authorized unless this same-inode durable write completes.
        let encoded = state.encode(self.config.limits);
        file.seek(SeekFrom::Start(0)).map_err(|_| BudgetError::Io)?;
        file.write_all(&encoded).map_err(|_| BudgetError::Io)?;
        file.set_len(encoded.len() as u64)
            .map_err(|_| BudgetError::Io)?;
        file.sync_all().map_err(|_| BudgetError::Io)?;
        validate_file(&file, &self.config.state_file, uid)?;
        if created {
            File::open(
                self.config
                    .state_file
                    .parent()
                    .ok_or(BudgetError::UnsafePath)?,
            )
            .and_then(|directory| directory.sync_all())
            .map_err(|_| BudgetError::Io)?;
        }
        result
    }
    #[cfg(test)]
    fn at(&self, class: RequestClass, now: u64) -> Result<Reservation, BudgetError> {
        self.with_clock(class, || Ok(now))
    }
}

fn unix_ms() -> Result<u64, BudgetError> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| BudgetError::Clock)?
            .as_millis(),
    )
    .map_err(|_| BudgetError::Clock)
}
fn remaining_ms(at: u64, now: u64) -> Result<u64, BudgetError> {
    at.checked_add(WINDOW_MS)
        .and_then(|expiry| expiry.checked_sub(now))
        .ok_or(BudgetError::Corrupt)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Record {
    at_ms: u64,
    class: RequestClass,
}
#[derive(Debug)]
struct State {
    last_checked_ms: u64,
    records: Vec<Record>,
}
impl State {
    fn encode(&self, limits: BudgetLimits) -> Vec<u8> {
        let mut bytes =
            Vec::with_capacity(HEADER_BYTES + self.records.len() * RECORD_BYTES + CHECKSUM_BYTES);
        bytes.extend_from_slice(MAGIC);
        for limit in [
            limits.total_per_second,
            limits.background_per_second,
            limits.reserved_exit_per_second,
            0,
        ] {
            bytes.extend_from_slice(&limit.to_le_bytes());
        }
        bytes.extend_from_slice(&self.last_checked_ms.to_le_bytes());
        bytes.extend_from_slice(&(self.records.len() as u32).to_le_bytes());
        for record in &self.records {
            bytes.extend_from_slice(&record.at_ms.to_le_bytes());
            bytes.push(u8::from(record.class == RequestClass::Exit));
        }
        bytes.extend_from_slice(&checksum(&bytes).to_le_bytes());
        bytes
    }
    fn decode(bytes: &[u8], limits: BudgetLimits) -> Result<Self, BudgetError> {
        if bytes.len() > MAX_STATE_BYTES {
            return Err(BudgetError::Oversized);
        }
        if bytes.len() < HEADER_BYTES + CHECKSUM_BYTES || &bytes[..8] != MAGIC {
            return Err(BudgetError::Corrupt);
        }
        let u16_at =
            |start| u16::from_le_bytes(bytes[start..start + 2].try_into().expect("checked header"));
        let stored = BudgetLimits {
            total_per_second: u16_at(8),
            background_per_second: u16_at(10),
            reserved_exit_per_second: u16_at(12),
        };
        if stored.validate().is_err() || u16_at(14) != 0 {
            return Err(BudgetError::Corrupt);
        }
        let count = u32::from_le_bytes(bytes[24..28].try_into().expect("checked header")) as usize;
        if count > usize::from(stored.total_per_second)
            || bytes.len() != HEADER_BYTES + count * RECORD_BYTES + CHECKSUM_BYTES
        {
            return Err(BudgetError::Corrupt);
        }
        let checked_len = bytes.len() - CHECKSUM_BYTES;
        let stored_checksum =
            u64::from_le_bytes(bytes[checked_len..].try_into().expect("checked checksum"));
        if stored_checksum != checksum(&bytes[..checked_len]) {
            return Err(BudgetError::Corrupt);
        }
        if stored != limits {
            return Err(BudgetError::PolicyMismatch);
        }
        let last_checked_ms = u64::from_le_bytes(bytes[16..24].try_into().expect("checked header"));
        if last_checked_ms.checked_add(WINDOW_MS).is_none() {
            return Err(BudgetError::Corrupt);
        }
        let mut records = Vec::with_capacity(count);
        let mut previous = 0;
        let mut background = 0;
        for index in 0..count {
            let offset = HEADER_BYTES + index * RECORD_BYTES;
            let at_ms = u64::from_le_bytes(
                bytes[offset..offset + 8]
                    .try_into()
                    .expect("checked record"),
            );
            let class = match bytes[offset + 8] {
                0 => RequestClass::Background,
                1 => RequestClass::Exit,
                _ => return Err(BudgetError::Corrupt),
            };
            if at_ms < previous || at_ms > last_checked_ms || last_checked_ms - at_ms >= WINDOW_MS {
                return Err(BudgetError::Corrupt);
            }
            if class == RequestClass::Background {
                background += 1;
            }
            previous = at_ms;
            records.push(Record { at_ms, class });
        }
        if background > usize::from(limits.background_per_second) {
            return Err(BudgetError::Corrupt);
        }
        Ok(Self {
            last_checked_ms,
            records,
        })
    }
}

/// Accidental-corruption check only; no cryptographic authentication is implied.
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(target_os = "linux")]
fn current_uid() -> Result<u32, BudgetError> {
    use std::os::unix::fs::MetadataExt;
    // procfs supplies the effective process owner without an FFI/dependency.
    fs::metadata("/proc/self")
        .map(|metadata| metadata.uid())
        .map_err(|_| BudgetError::Io)
}
#[cfg(target_os = "linux")]
fn validate_parent(path: &Path) -> Result<u32, BudgetError> {
    use std::os::unix::fs::MetadataExt;
    let uid = current_uid()?;
    let parent = path.parent().ok_or(BudgetError::UnsafePath)?;
    let mut prefix = PathBuf::new();
    for component in parent.components() {
        prefix.push(component);
        let metadata = fs::symlink_metadata(&prefix).map_err(|_| BudgetError::UnsafeParent)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || (metadata.uid() != 0 && metadata.uid() != uid)
            || (metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0)
        {
            return Err(BudgetError::UnsafeParent);
        }
    }
    let metadata = fs::symlink_metadata(parent).map_err(|_| BudgetError::UnsafeParent)?;
    if metadata.uid() != uid || metadata.mode() & 0o7777 != 0o700 {
        return Err(BudgetError::UnsafeParent);
    }
    Ok(uid)
}
#[cfg(target_os = "linux")]
fn open_private(path: &Path, uid: u32) -> Result<(File, bool), BudgetError> {
    use std::os::unix::fs::OpenOptionsExt;
    // Linux O_NOFOLLOW=0400000 and O_NONBLOCK=04000. The latter avoids a FIFO
    // open hanging if a noncooperating owner swaps a checked path before open.
    let flags = 0o400000 | 0o4000;
    let created = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(flags)
        .open(path);
    let (file, created) = match created {
        Ok(file) => (file, true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (
            OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(flags)
                .open(path)
                .map_err(|_| BudgetError::UnsafeFile)?,
            false,
        ),
        Err(_) => return Err(BudgetError::Io),
    };
    validate_file(&file, path, uid)?;
    Ok((file, created))
}
#[cfg(target_os = "linux")]
fn validate_file(file: &File, path: &Path, uid: u32) -> Result<(), BudgetError> {
    use std::os::unix::fs::MetadataExt;
    let opened = file.metadata().map_err(|_| BudgetError::Io)?;
    let current = fs::symlink_metadata(path).map_err(|_| BudgetError::PathChanged)?;
    if !opened.is_file()
        || opened.uid() != uid
        || opened.mode() & 0o7777 != 0o600
        || opened.nlink() != 1
        || current.file_type().is_symlink()
    {
        return Err(BudgetError::UnsafeFile);
    }
    if opened.ino() != current.ino() || opened.dev() != current.dev() {
        return Err(BudgetError::PathChanged);
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn validate_parent(_: &Path) -> Result<u32, BudgetError> {
    Err(BudgetError::UnsupportedPlatform)
}
#[cfg(not(target_os = "linux"))]
fn open_private(_: &Path, _: u32) -> Result<(File, bool), BudgetError> {
    Err(BudgetError::UnsupportedPlatform)
}
#[cfg(not(target_os = "linux"))]
fn validate_file(_: &File, _: &Path, _: u32) -> Result<(), BudgetError> {
    Err(BudgetError::UnsupportedPlatform)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::{MetadataExt, PermissionsExt},
        process::{Child, Command, Output, Stdio},
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::{Duration, Instant},
    };
    const BG: RequestClass = RequestClass::Background;
    const EXIT: RequestClass = RequestClass::Exit;
    fn limits() -> BudgetLimits {
        BudgetLimits {
            total_per_second: 10,
            background_per_second: 6,
            reserved_exit_per_second: 3,
        }
    }
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "shortwin-price-budget-{}-{time}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
        fn file(&self) -> PathBuf {
            self.0.join("requests.state")
        }
        fn budget(&self, limits: BudgetLimits) -> TestBudget {
            TestBudget(
                SharedRequestBudget::new(BudgetConfig {
                    state_file: self.file(),
                    limits,
                })
                .unwrap(),
            )
        }
    }
    struct TestBudget(SharedRequestBudget);
    impl TestBudget {
        fn at(&self, class: RequestClass, now: u64) -> Result<Reservation, BudgetError> {
            // Other concurrently running tests can briefly fork/exec while a
            // CLOEXEC descriptor exists. Busy is valid; only these semantic tests
            // retry it. The production API and child contention test never wait.
            for _ in 0..200 {
                match self.0.at(class, now) {
                    Err(BudgetError::Busy) => thread::sleep(Duration::from_millis(1)),
                    result => return result,
                }
            }
            Err(BudgetError::Busy)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn exact_rolling_window_and_reserved_exit_capacity() {
        let directory = Directory::new();
        let budget = directory.budget(limits());
        for count in 1..=6 {
            assert_eq!(budget.at(BG, 10_000).unwrap().used_background, count);
        }
        assert_eq!(
            budget.at(BG, 10_999),
            Err(BudgetError::RateLimited { retry_after_ms: 1 })
        );
        for count in 7..=10 {
            assert_eq!(budget.at(EXIT, 10_999).unwrap().used_total, count);
        }
        assert_eq!(
            budget.at(EXIT, 10_999),
            Err(BudgetError::RateLimited { retry_after_ms: 1 })
        );
        // Only the six reservations at10000expire; the four later exits stay charged.
        let next = budget.at(BG, 11_000).unwrap();
        assert_eq!(next.used_total, 5);
        assert_eq!(next.used_background, 1);
    }

    #[test]
    fn persistence_and_same_inode_writes_do_not_refund_consumed_requests() {
        let directory = Directory::new();
        let first = directory.budget(limits());
        first.at(BG, 10_000).unwrap();
        let inode = fs::metadata(directory.file()).unwrap().ino();
        let second = directory.budget(limits());
        assert_eq!(second.at(BG, 10_001).unwrap().used_total, 2);
        assert_eq!(fs::metadata(directory.file()).unwrap().ino(), inode);
        let state = State::decode(&fs::read(directory.file()).unwrap(), limits()).unwrap();
        assert_eq!(state.records.len(), 2);
        assert_eq!(fs::metadata(&directory.0).unwrap().mode() & 0o7777, 0o700);
        assert_eq!(
            fs::metadata(directory.file()).unwrap().mode() & 0o7777,
            0o600
        );
    }

    #[test]
    fn regression_and_policy_mismatch_preserve_state_and_do_not_reset_usage() {
        let directory = Directory::new();
        let budget = directory.budget(limits());
        budget.at(BG, 10_000).unwrap();
        let before = fs::read(directory.file()).unwrap();
        assert_eq!(budget.at(BG, 9_999), Err(BudgetError::ClockRegression));
        assert_eq!(fs::read(directory.file()).unwrap(), before);
        let other = directory.budget(BudgetLimits {
            total_per_second: 11,
            ..limits()
        });
        assert_eq!(other.at(BG, 10_001), Err(BudgetError::PolicyMismatch));
        assert_eq!(fs::read(directory.file()).unwrap(), before);
        assert_eq!(budget.at(BG, 10_001).unwrap().used_total, 2);
    }

    #[test]
    fn corrupt_empty_oversized_or_tampered_existing_state_fails_closed_without_writes() {
        let directory = Directory::new();
        let budget = directory.budget(limits());
        budget.at(BG, 10_000).unwrap();
        let valid = fs::read(directory.file()).unwrap();
        let mut damaged = valid.clone();
        damaged[8] ^= 1;
        for (bytes, error) in [
            (Vec::new(), BudgetError::Corrupt),
            (
                b"unrelated file must survive".to_vec(),
                BudgetError::Corrupt,
            ),
            (damaged, BudgetError::Corrupt),
            (vec![0; MAX_STATE_BYTES + 1], BudgetError::Oversized),
        ] {
            fs::write(directory.file(), &bytes).unwrap();
            assert_eq!(budget.at(BG, 10_001), Err(error));
            assert_eq!(fs::read(directory.file()).unwrap(), bytes);
            assert_eq!(budget.at(EXIT, 11_000), Err(error));
            assert_eq!(fs::read(directory.file()).unwrap(), bytes);
        }
        fs::write(directory.file(), valid).unwrap();
        assert_eq!(budget.at(EXIT, 10_001).unwrap().used_total, 2);
    }

    #[test]
    fn semantic_state_validation_is_strict_even_with_a_matching_checksum() {
        for state in [
            State {
                last_checked_ms: 10_000,
                records: vec![Record {
                    at_ms: 10_001,
                    class: BG,
                }],
            },
            State {
                last_checked_ms: 11_000,
                records: vec![Record {
                    at_ms: 10_000,
                    class: BG,
                }],
            },
            State {
                last_checked_ms: 10_001,
                records: vec![
                    Record {
                        at_ms: 10_001,
                        class: BG,
                    },
                    Record {
                        at_ms: 10_000,
                        class: BG,
                    },
                ],
            },
            State {
                last_checked_ms: 10_000,
                records: vec![
                    Record {
                        at_ms: 10_000,
                        class: BG
                    };
                    7
                ],
            },
        ] {
            assert!(matches!(
                State::decode(&state.encode(limits()), limits()),
                Err(BudgetError::Corrupt)
            ));
        }
        let state = State {
            last_checked_ms: 10_000,
            records: vec![Record {
                at_ms: 10_000,
                class: BG,
            }],
        };
        let valid = state.encode(limits());
        for index in [14, HEADER_BYTES + 8] {
            let mut changed = valid.clone();
            changed[index] = 255;
            let len = changed.len() - CHECKSUM_BYTES;
            let sum = checksum(&changed[..len]);
            changed[len..].copy_from_slice(&sum.to_le_bytes());
            assert!(matches!(
                State::decode(&changed, limits()),
                Err(BudgetError::Corrupt)
            ));
        }
    }

    #[test]
    fn unsafe_permissions_symlinks_hardlinks_and_replaced_inodes_are_refused() {
        let directory = Directory::new();
        let budget = directory.budget(limits());
        budget.at(BG, 10_000).unwrap();
        let before = fs::read(directory.file()).unwrap();
        fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(budget.at(BG, 10_001), Err(BudgetError::UnsafeParent));
        fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(directory.file(), fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(budget.at(BG, 10_001), Err(BudgetError::UnsafeFile));
        fs::set_permissions(directory.file(), fs::Permissions::from_mode(0o600)).unwrap();
        let alias = directory.0.join("alias");
        fs::hard_link(directory.file(), &alias).unwrap();
        assert_eq!(budget.at(BG, 10_001), Err(BudgetError::UnsafeFile));
        fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(directory.file(), &alias).unwrap();
        let linked = SharedRequestBudget::new(BudgetConfig {
            state_file: alias.clone(),
            limits: limits(),
        })
        .unwrap();
        assert_eq!(linked.at(BG, 10_001), Err(BudgetError::UnsafeFile));
        fs::remove_file(&alias).unwrap();
        let parent_alias = directory.0.join("parent-alias");
        std::os::unix::fs::symlink(&directory.0, &parent_alias).unwrap();
        let linked = SharedRequestBudget::new(BudgetConfig {
            state_file: parent_alias.join("requests.state"),
            limits: limits(),
        })
        .unwrap();
        assert_eq!(linked.at(BG, 10_001), Err(BudgetError::UnsafeParent));
        let open = OpenOptions::new()
            .read(true)
            .write(true)
            .open(directory.file())
            .unwrap();
        open.try_lock().unwrap();
        fs::rename(directory.file(), directory.0.join("old-inode")).unwrap();
        fs::write(directory.file(), &before).unwrap();
        fs::set_permissions(directory.file(), fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            validate_file(&open, &directory.file(), current_uid().unwrap()),
            Err(BudgetError::PathChanged)
        );
        assert_eq!(fs::read(directory.file()).unwrap(), before);
    }

    #[test]
    fn resource_configuration_and_clock_overflow_are_bounded() {
        for limits in [
            BudgetLimits {
                total_per_second: 0,
                ..limits()
            },
            BudgetLimits {
                total_per_second: 1001,
                ..limits()
            },
            BudgetLimits {
                background_per_second: 0,
                ..limits()
            },
            BudgetLimits {
                reserved_exit_per_second: 0,
                ..limits()
            },
            BudgetLimits {
                background_per_second: 8,
                ..limits()
            },
        ] {
            assert_eq!(limits.validate(), Err(BudgetError::InvalidConfig));
        }
        for path in [
            PathBuf::from("relative/state"),
            PathBuf::from("/tmp/../state"),
            PathBuf::from("/"),
        ] {
            assert!(matches!(
                SharedRequestBudget::new(BudgetConfig {
                    state_file: path,
                    limits: limits()
                }),
                Err(BudgetError::UnsafePath)
            ));
        }
        let directory = Directory::new();
        let budget = directory.budget(limits());
        assert_eq!(budget.at(BG, u64::MAX), Err(BudgetError::Clock));
        // A failed initial write must not silently turn the existing empty inode into a new budget.
        assert_eq!(budget.at(BG, 10_000), Err(BudgetError::Corrupt));
    }

    fn child_name() -> String {
        format!(
            "{}::child_process",
            module_path!().split_once("::").unwrap().1
        )
    }
    fn spawn_child(
        directory: &Directory,
        class: RequestClass,
        count: usize,
        at: Option<u64>,
    ) -> Child {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", &child_name(), "--nocapture"])
            .env("SHORTWIN_BUDGET_TEST_PATH", directory.file())
            .env(
                "SHORTWIN_BUDGET_TEST_CLASS",
                if class == BG { "background" } else { "exit" },
            )
            .env("SHORTWIN_BUDGET_TEST_COUNT", count.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(at) = at {
            command.env("SHORTWIN_BUDGET_TEST_AT_MS", at.to_string());
        } else {
            command.env_remove("SHORTWIN_BUDGET_TEST_AT_MS");
        }
        command.spawn().unwrap()
    }
    fn finish_child(mut child: Child) -> Output {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if child.try_wait().unwrap().is_some() {
                return child.wait_with_output().unwrap();
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "budget child blocked: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            thread::sleep(Duration::from_millis(2));
        }
    }
    fn counts(output: Output) -> (usize, usize, usize) {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        let line = text
            .lines()
            .find(|line| line.starts_with("BUDGET_RESULT "))
            .unwrap_or_else(|| panic!("missing child result: {text}"));
        let fields: Vec<_> = line
            .split_whitespace()
            .skip(1)
            .map(|value| value.parse::<usize>().unwrap())
            .collect();
        (fields[0], fields[1], fields[2])
    }

    #[test]
    fn independent_processes_share_capacity_and_preserve_reserved_exits_across_restart() {
        let directory = Directory::new();
        directory.budget(limits()).at(BG, 10_000).unwrap();
        let children: Vec<_> = (0..4)
            .map(|_| spawn_child(&directory, BG, 8, Some(10_000)))
            .collect();
        let mut succeeded = 0;
        for child in children {
            let (ok, _, _) = counts(finish_child(child));
            succeeded += ok;
        }
        assert_eq!(
            succeeded, 5,
            "parent plus all children share one six-background cap"
        );
        assert_eq!(
            counts(finish_child(spawn_child(&directory, EXIT, 8, Some(10_000)))).0,
            4
        );
        assert_eq!(
            counts(finish_child(spawn_child(&directory, BG, 1, Some(10_999)))),
            (0, 1, 0)
        );
        assert_eq!(
            counts(finish_child(spawn_child(&directory, BG, 1, Some(11_000)))).0,
            1
        );
    }

    #[test]
    fn independent_process_lock_contention_is_nonblocking_and_never_consumes_capacity() {
        let directory = Directory::new();
        directory.budget(limits()).at(BG, 10_000).unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(directory.file())
            .unwrap();
        lock.try_lock().unwrap();
        // Child reports Busy once (no retry in this special mode), not a blocking flock wait.
        let mut command = Command::new(std::env::current_exe().unwrap());
        let child = command
            .args(["--exact", &child_name(), "--nocapture"])
            .env("SHORTWIN_BUDGET_TEST_PATH", directory.file())
            .env("SHORTWIN_BUDGET_TEST_CLASS", "background")
            .env("SHORTWIN_BUDGET_TEST_COUNT", "1")
            .env("SHORTWIN_BUDGET_TEST_AT_MS", "10000")
            .env("SHORTWIN_BUDGET_TEST_BUSY_ONCE", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        assert_eq!(counts(finish_child(child)), (0, 0, 1));
        drop(lock);
        assert_eq!(
            directory
                .budget(limits())
                .at(BG, 10_001)
                .unwrap()
                .used_total,
            2
        );
    }

    #[test]
    fn child_process() {
        let Some(path) = std::env::var_os("SHORTWIN_BUDGET_TEST_PATH") else {
            return;
        };
        let class = if std::env::var("SHORTWIN_BUDGET_TEST_CLASS").unwrap() == "exit" {
            EXIT
        } else {
            BG
        };
        let count = std::env::var("SHORTWIN_BUDGET_TEST_COUNT")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(count <= 32);
        let at = std::env::var("SHORTWIN_BUDGET_TEST_AT_MS")
            .ok()
            .map(|value| value.parse::<u64>().unwrap());
        let once = std::env::var_os("SHORTWIN_BUDGET_TEST_BUSY_ONCE").is_some();
        let budget = SharedRequestBudget::new(BudgetConfig {
            state_file: PathBuf::from(path),
            limits: limits(),
        })
        .unwrap();
        let (mut ok, mut limited, mut busy) = (0, 0, 0);
        for _ in 0..count {
            for attempt in 0..200 {
                let result = if let Some(at) = at {
                    budget.at(class, at)
                } else {
                    budget.try_reserve(class)
                };
                match result {
                    Ok(_) => {
                        ok += 1;
                        break;
                    }
                    Err(BudgetError::RateLimited { .. }) => {
                        limited += 1;
                        break;
                    }
                    Err(BudgetError::Busy) => {
                        if once {
                            busy += 1;
                            break;
                        }
                        assert!(attempt < 199, "bounded child retry exhausted");
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("child budget failure: {error:?}"),
                }
            }
        }
        println!("BUDGET_RESULT {ok} {limited} {busy}");
    }
}
