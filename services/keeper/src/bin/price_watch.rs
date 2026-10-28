//! Trusted VPS price watcher. Reads actual chain identity/config and Jupiter quotes,
//! persists off-chain history/evidence, and exports unsigned action-time cache updates.
//! Polling never signs/sends. Only explicit --publish loads the scoped reporter
//! key and submits one fixed snapshot instruction within persistent fee/rate limits.
use anchor_lang::{AccountDeserialize, Discriminator};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shortwin::{
    constants::{CONFIG_VERSION, MARKET_VERSION},
    state::{GlobalConfig, Market, PriceSnapshot},
};
use shortwin_keeper::{
    offchain_price_history::{HistoryStore, OperatorObservation},
    price_budget::{BudgetConfig, BudgetLimits, SharedRequestBudget},
    price_http::MAINNET_GENESIS,
    price_search::{SearchPolicy, SolLamports},
    publication_budget::{PublicationBudget, PublicationLimits, PublicationResolution},
    snapshot_publisher::{
        PublicationSettings, PublisherError, publish_snapshot, reconcile_publication,
    },
    trusted_jupiter_reference::{
        CollectedReference, TrustedJupiterPlan, TrustedJupiterSource, TrustedReferencePolicy,
    },
};
use shortwin_price_report::{
    Address, CallerAuthenticatedSigner, ObservationWindow, ReportStatus, RuntimeClock,
    trusted_snapshot::{SnapshotReport, SnapshotState, codec},
};
use shortwin_services_common::{Chain, rpc::RpcChain};
use solana_pubkey::Pubkey;
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions, TryLockError},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

const USAGE: &str = "usage: shortwin-price-watch --config <file> (--check | --once | --watch | --snapshot | --publish | --initialize-budget | --reconcile)";
const MAX_CONFIG: u64 = 65_536;
const MAX_SECRET: u64 = 4_096;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WatchError {
    Arguments,
    Config,
    Secret,
    Chain,
    Receiver,
    Policy,
    History,
    Publication(PublisherError),
    Evidence,
    Busy,
    Clock,
}
impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "price watch: {self:?}")
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Check,
    Once,
    Watch,
    Snapshot,
    Publish,
    InitializeBudget,
    Reconcile,
}
struct Args {
    config: PathBuf,
    mode: Mode,
}
fn args(raw: impl IntoIterator<Item = OsString>) -> Result<Option<Args>, WatchError> {
    let raw: Vec<_> = raw.into_iter().take(5).collect();
    if raw.len() == 1 && matches!(raw[0].to_str(), Some("--help" | "-h")) {
        return Ok(None);
    }
    if raw.len() != 3 || raw[0] != "--config" || raw[1].is_empty() {
        return Err(WatchError::Arguments);
    }
    let mode = match raw[2].to_str() {
        Some("--check") => Mode::Check,
        Some("--once") => Mode::Once,
        Some("--watch") => Mode::Watch,
        Some("--snapshot") => Mode::Snapshot,
        Some("--publish") => Mode::Publish,
        Some("--initialize-budget") => Mode::InitializeBudget,
        Some("--reconcile") => Mode::Reconcile,
        _ => return Err(WatchError::Arguments),
    };
    Ok(Some(Args {
        config: PathBuf::from(&raw[1]),
        mode,
    }))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    rpc_url_file: PathBuf,
    api_key_file: PathBuf,
    snapshot_account: String,
    publisher: String,
    state_file: PathBuf,
    evidence_directory: PathBuf,
    evidence_max_bytes: u64,
    poll_interval_ms: u64,
    request_budget: RawBudget,
    taker: String,
    account_policy_digest_hex: String,
    ask_input_lamports: Vec<String>,
    maximum_ask_input_lamports: String,
    slippage_bps: u16,
    request_timeout_ms: u64,
    observation_window_ms: u64,
    maximum_response_bytes: usize,
    minimum_request_spacing_ms: u64,
    bid_haircut_bps: u16,
    ask_buffer_bps: u16,
    maximum_spread_bps: u16,
    maximum_observation_slots: u64,
    maximum_observation_seconds: u64,
    api_market_state_age_is_trusted: bool,
    publication: Option<RawPublication>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPublication {
    reporter_keypair_file: PathBuf,
    budget_state_file: PathBuf,
    daily_max_lamports: u64,
    burst_window_seconds: u64,
    burst_attempts: u16,
    compute_unit_limit: u32,
    micro_lamports_per_cu: u64,
    maximum_fee_lamports: u64,
    minimum_payer_balance_after_lamports: u64,
    operation_timeout_seconds: u64,
}
impl RawPublication {
    fn budget(&self) -> Result<PublicationBudget, WatchError> {
        PublicationBudget::new(
            self.budget_state_file.clone(),
            PublicationLimits {
                daily_max_lamports: self.daily_max_lamports,
                burst_window_seconds: self.burst_window_seconds,
                burst_attempts: self.burst_attempts,
            },
        )
        .map_err(|_| WatchError::Policy)
    }
    fn settings(&self) -> PublicationSettings {
        PublicationSettings {
            compute_unit_limit: self.compute_unit_limit,
            micro_lamports_per_cu: self.micro_lamports_per_cu,
            maximum_fee_lamports: self.maximum_fee_lamports,
            minimum_payer_balance_after_lamports: self.minimum_payer_balance_after_lamports,
            operation_timeout_seconds: self.operation_timeout_seconds,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBudget {
    state_file: PathBuf,
    total_per_second: u16,
    background_per_second: u16,
    reserved_exit_per_second: u16,
}
fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>, WatchError> {
    let file = File::open(path).map_err(|_| WatchError::Config)?;
    if file.metadata().map_err(|_| WatchError::Config)?.len() > max {
        return Err(WatchError::Config);
    }
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| WatchError::Config)?;
    if bytes.len() as u64 > max {
        return Err(WatchError::Config);
    }
    Ok(bytes)
}
fn private_text(path: &Path) -> Result<String, WatchError> {
    let bytes = private_read(path, MAX_SECRET)?;
    let value = String::from_utf8(bytes).map_err(|_| WatchError::Secret)?;
    let value = value.trim_end_matches(['\r', '\n']);
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(WatchError::Secret);
    }
    Ok(value.to_owned())
}
#[cfg(target_os = "linux")]
fn private_read(path: &Path, max: u64) -> Result<Vec<u8>, WatchError> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    private_directory(path.parent().ok_or(WatchError::Secret)?).map_err(|_| WatchError::Secret)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(0o400000 | 0o4000)
        .open(path)
        .map_err(|_| WatchError::Secret)?;
    private_file(path)?;
    let opened = file.metadata().map_err(|_| WatchError::Secret)?;
    let current = fs::symlink_metadata(path).map_err(|_| WatchError::Secret)?;
    if opened.dev() != current.dev() || opened.ino() != current.ino() || opened.len() > max {
        return Err(WatchError::Secret);
    }
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| WatchError::Secret)?;
    if bytes.len() as u64 > max {
        return Err(WatchError::Secret);
    }
    Ok(bytes)
}
#[cfg(not(target_os = "linux"))]
fn private_read(_: &Path, _: u64) -> Result<Vec<u8>, WatchError> {
    Err(WatchError::Secret)
}

fn address(raw: &str) -> Result<Pubkey, WatchError> {
    Pubkey::from_str(raw).map_err(|_| WatchError::Config)
}
fn digest(raw: &str) -> Result<Address, WatchError> {
    if raw.len() != 64 {
        return Err(WatchError::Config);
    }
    let mut result = [0; 32];
    for (i, pair) in raw.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let hex = |b: u8| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(WatchError::Config),
        };
        result[i] = hex(pair[0])? * 16 + hex(pair[1])?;
    }
    if result == [0; 32] {
        return Err(WatchError::Config);
    }
    Ok(result)
}
fn load(path: &Path) -> Result<Config, WatchError> {
    let bytes = read_bounded(path, MAX_CONFIG)?;
    let config: Config =
        toml::from_str(std::str::from_utf8(&bytes).map_err(|_| WatchError::Config)?)
            .map_err(|_| WatchError::Config)?;
    if config.poll_interval_ms == 0
        || config.poll_interval_ms > 60_000
        || config.evidence_max_bytes < 1_048_576
        || config.evidence_max_bytes > 1_099_511_627_776
        || config.ask_input_lamports.is_empty()
        || config.ask_input_lamports.len() > 63
    {
        return Err(WatchError::Config);
    }
    Ok(config)
}
fn plan(
    config: &Config,
    state: &SnapshotState,
    producer_commitment: Address,
) -> Result<TrustedJupiterPlan, WatchError> {
    let plan = TrustedJupiterPlan {
        configuration: *state.configuration(),
        expected_producer_commitment: producer_commitment,
        reference: TrustedReferencePolicy {
            bid_haircut_bps: config.bid_haircut_bps,
            ask_buffer_bps: config.ask_buffer_bps,
            maximum_spread_bps: config.maximum_spread_bps,
            maximum_observation_slots: config.maximum_observation_slots,
            maximum_observation_seconds: config.maximum_observation_seconds,
            minimum_request_spacing_ms: config.minimum_request_spacing_ms,
            api_market_state_age_is_trusted: config.api_market_state_age_is_trusted,
        },
        search: SearchPolicy {
            max_input_lamports: SolLamports::parse(&config.maximum_ask_input_lamports)
                .map_err(|_| WatchError::Config)?,
            max_requests: config.ask_input_lamports.len(),
            max_response_bytes: config.maximum_response_bytes,
            request_timeout_ms: config.request_timeout_ms,
            max_window_ms: config.observation_window_ms,
            slippage_bps: config.slippage_bps,
        },
        ask_inputs: config
            .ask_input_lamports
            .iter()
            .map(|s| SolLamports::parse(s).map_err(|_| WatchError::Config))
            .collect::<Result<Vec<_>, _>>()?,
        taker: address(&config.taker)?.to_bytes(),
        account_policy_digest: digest(&config.account_policy_digest_hex)?,
    };
    plan.validate().map_err(|_| WatchError::Policy)?;
    Ok(plan)
}
async fn receiver(chain: &RpcChain, key: Pubkey) -> Result<(SnapshotState, Address), WatchError> {
    let accounts = chain
        .accounts(&[key])
        .await
        .map_err(|_| WatchError::Chain)?;
    let account = accounts
        .first()
        .and_then(Option::as_ref)
        .ok_or(WatchError::Receiver)?;
    if account.owner.to_bytes() != shortwin::ID.to_bytes()
        || account.executable
        || account.data.len() != PriceSnapshot::SPACE
    {
        return Err(WatchError::Receiver);
    }
    let wrapper = PriceSnapshot::try_deserialize(&mut account.data.as_slice())
        .map_err(|_| WatchError::Receiver)?;
    if wrapper.version != 2 || wrapper.producer_commitment == [0; 32] {
        return Err(WatchError::Receiver);
    }
    let state = codec::decode(&wrapper.state_bytes).map_err(|_| WatchError::Receiver)?;
    let c = state.configuration();
    let d = c.domain();
    if d.program != shortwin::ID.to_bytes()
        || wrapper.market.to_bytes() != d.market
        || wrapper.quantity != c.quantity()
        || wrapper.model_epoch != c.model().epoch
    {
        return Err(WatchError::Receiver);
    }
    let program = Pubkey::new_from_array(d.program);
    let (pda, bump) = Pubkey::find_program_address(
        &[
            PriceSnapshot::SEED,
            &d.market,
            &c.quantity().to_le_bytes(),
            &c.model().epoch.to_le_bytes(),
        ],
        &program,
    );
    let (config_key, _) = Pubkey::find_program_address(&[GlobalConfig::SEED], &program);
    if pda != key || bump != wrapper.bump || config_key.to_bytes() != d.config {
        return Err(WatchError::Receiver);
    }
    let linked = chain
        .accounts(&[config_key, Pubkey::new_from_array(d.market)])
        .await
        .map_err(|_| WatchError::Chain)?;
    let cfg = linked
        .first()
        .and_then(Option::as_ref)
        .ok_or(WatchError::Receiver)?;
    let market = linked
        .get(1)
        .and_then(Option::as_ref)
        .ok_or(WatchError::Receiver)?;
    if cfg.owner != program
        || cfg.executable
        || cfg.data.len() != GlobalConfig::SPACE
        || market.owner != program
        || market.executable
        || market.data.len() != 8 + std::mem::size_of::<Market>()
        || !cfg.data.starts_with(GlobalConfig::DISCRIMINATOR)
        || !market.data.starts_with(Market::DISCRIMINATOR)
    {
        return Err(WatchError::Receiver);
    }
    let cfg: GlobalConfig = bytemuck::pod_read_unaligned(&cfg.data[8..]);
    let m: Market = bytemuck::pod_read_unaligned(&market.data[8..]);
    if cfg.version != CONFIG_VERSION
        || m.version != MARKET_VERSION
        || !m.is_pda(&anchor_lang::prelude::Pubkey::new_from_array(d.market))
        || m.mint.to_bytes() != d.base_mint
        || m.token_program.to_bytes() != d.base_token_program
        || m.decimals != d.base_decimals
    {
        return Err(WatchError::Receiver);
    }
    Ok((state, wrapper.producer_commitment))
}
async fn clock(chain: &RpcChain) -> Result<RuntimeClock, WatchError> {
    let c = tokio::time::timeout(Duration::from_secs(15), chain.clock())
        .await
        .map_err(|_| WatchError::Chain)?
        .map_err(|_| WatchError::Chain)?;
    if c.unix_timestamp < 0 {
        return Err(WatchError::Clock);
    }
    Ok(RuntimeClock {
        slot: c.slot,
        unix_timestamp: c.unix_timestamp,
    })
}
fn pk(key: Address) -> String {
    Pubkey::new_from_array(key).to_string()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn snapshot_json(report: &SnapshotReport, account: Pubkey) -> Value {
    let history:Vec<_>=report.history.buckets().iter().map(|entry|match entry{
        Some(entry)=>{let b=entry.bucket;json!({"epoch":b.epoch.to_string(),"first_slot":b.first_observed.slot.to_string(),
            "first_ts":b.first_observed.unix_timestamp.to_string(),"last_slot":b.last_observed.slot.to_string(),
            "last_ts":b.last_observed.unix_timestamp.to_string(),"lower_min":b.lower_min.to_string(),
            "central_last":b.central_last.to_string(),"upper_max":b.upper_max.to_string(),"observations":b.observations.to_string(),
            "largest_gap_slots":entry.largest_gap_slots.to_string(),"largest_gap_seconds":entry.largest_gap_seconds.to_string()})},
        None=>json!({"epoch":"0","first_slot":"0","first_ts":"0","last_slot":"0","last_ts":"0",
            "lower_min":"0","central_last":"0","upper_max":"0","observations":"0","largest_gap_slots":"0","largest_gap_seconds":"0"}),
    }).collect();
    let status = match report.status {
        ReportStatus::Unknown => json!({"unknown":{}}),
        ReportStatus::Finite(m) => json!({"finite":{
        "lower":m.lower().to_string(),"central":m.central().to_string(),"upper":m.upper().to_string()}}),
    };
    json!({"schema":"shortwin-unsigned-trusted-snapshot/v2","snapshot_account":account.to_string(),
        "publisher":pk(report.publisher),"program":pk(report.domain.program),"configuration":pk(report.domain.config),
        "market":pk(report.domain.market),"quantity_raw":report.quantity.to_string(),"policy_hash_hex":hex(&report.model.policy_hash),
        "operator_attested_history":true,"source_state_age_verified":false,"executable_minimum_verified":false,
        "transaction_signed":false,"transaction_submitted":false,"args":{
        "version":report.version,"sequence":report.sequence.to_string(),
        "start_slot":report.window.start.slot.to_string(),"start_ts":report.window.start.unix_timestamp.to_string(),
        "end_slot":report.window.end.slot.to_string(),"end_ts":report.window.end.unix_timestamp.to_string(),
        "observation_digest":report.observation_digest,"status":status,"history_count":report.history.len(),"history":history}})
}

fn resolution_json(resolution: &PublicationResolution) -> Value {
    let signature = solana_signature::Signature::try_from(resolution.signature.as_slice())
        .ok()
        .map(|s| s.to_string());
    json!({"signature":signature,"receiver":pk(resolution.receiver),"sequence":resolution.sequence.to_string(),
        "reserved_fee_lamports":resolution.reserved_fee_lamports.to_string(),"kind":resolution.kind,
        "slot":resolution.slot.map(|slot|slot.to_string()),"actual_fee_lamports":resolution.actual_fee_lamports.map(|fee|fee.to_string()),
        "actual_fee_known":resolution.actual_fee_lamports.is_some()})
}

/// One supervised process owns this journal. Startup reconciles bytes from actual
/// files, including crash orphans; running updates maintain the in-memory count.
/// Reaching the explicit byte cap stops collection for operator archival. No old
/// evidence is automatically deleted. No secret/API response appears in stdout.
struct EvidenceJournal {
    directory: PathBuf,
    maximum: u64,
    used: u64,
    _lock: File,
}
impl EvidenceJournal {
    fn open(directory: &Path, maximum: u64) -> Result<Self, WatchError> {
        private_directory(directory)?;
        let lock = private_lock(&directory.join(".watch-lock"))?;
        let mut used = 0u64;
        let mut count = 0u32;
        for entry in fs::read_dir(directory).map_err(|_| WatchError::Evidence)? {
            let entry = entry.map_err(|_| WatchError::Evidence)?;
            private_file(&entry.path())?;
            used = used
                .checked_add(entry.metadata().map_err(|_| WatchError::Evidence)?.len())
                .ok_or(WatchError::Evidence)?;
            count += 1;
            if count > 1_000_000 || used > maximum {
                return Err(WatchError::Evidence);
            }
        }
        Ok(Self {
            directory: directory.to_owned(),
            maximum,
            used,
            _lock: lock,
        })
    }
    fn retain(&mut self, reference: &CollectedReference) -> Result<(), WatchError> {
        let receipt=serde_json::to_vec(&json!({"schema":"shortwin-trusted-jupiter-observation/v1",
            "digest_hex":hex(&reference.digest),"bid_lamports":reference.bid_lamports.to_string(),
            "ask_lamports":reference.ask_lamports.to_string(),"lower_estimate":reference.marks.lower().to_string(),
            "central_estimate":reference.marks.central().to_string(),"upper_estimate":reference.marks.upper().to_string(),
            "start_slot":reference.window.start.slot.to_string(),"end_slot":reference.window.end.slot.to_string(),
            "start_ts":reference.window.start.unix_timestamp,"end_ts":reference.window.end.unix_timestamp,
            "bid_sha256":hex(&Sha256::digest(&reference.bid_raw_response)),
            "ask_sha256":hex(&Sha256::digest(&reference.ask_raw_response)),
            "source_state_age_verified":false,"executable_minimum_verified":false,"operator_price_estimate":true,
            "ask_evidence_retention":"selected_candidate_only","all_candidate_bodies_retained":false}))
            .map_err(|_|WatchError::Evidence)?;
        let added = (reference.bid_raw_response.len()
            + reference.ask_raw_response.len()
            + receipt.len()) as u64;
        if self
            .used
            .checked_add(added)
            .is_none_or(|n| n > self.maximum)
        {
            return Err(WatchError::Evidence);
        }
        let stem = hex(&reference.digest);
        for (suffix, bytes) in [
            ("bid.json", reference.bid_raw_response.as_slice()),
            ("ask.json", reference.ask_raw_response.as_slice()),
            ("receipt.json", receipt.as_slice()),
        ] {
            exclusive_write(&self.directory.join(format!("{stem}.{suffix}")), bytes)?;
            self.used += bytes.len() as u64;
        }
        File::open(&self.directory)
            .and_then(|f| f.sync_all())
            .map_err(|_| WatchError::Evidence)
    }
}
#[cfg(target_os = "linux")]
fn private_directory(path: &Path) -> Result<(), WatchError> {
    use std::os::unix::fs::MetadataExt;
    if !path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(WatchError::Evidence);
    }
    let uid = fs::metadata("/proc/self")
        .map_err(|_| WatchError::Evidence)?
        .uid();
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component);
        let m = fs::symlink_metadata(&prefix).map_err(|_| WatchError::Evidence)?;
        if !m.is_dir()
            || m.file_type().is_symlink()
            || (m.uid() != 0 && m.uid() != uid)
            || (m.mode() & 0o022 != 0 && m.mode() & 0o1000 == 0)
        {
            return Err(WatchError::Evidence);
        }
    }
    let m = fs::symlink_metadata(path).map_err(|_| WatchError::Evidence)?;
    if m.uid() != uid || m.mode() & 0o7777 != 0o700 {
        return Err(WatchError::Evidence);
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn private_file(path: &Path) -> Result<(), WatchError> {
    use std::os::unix::fs::MetadataExt;
    let m = fs::symlink_metadata(path).map_err(|_| WatchError::Secret)?;
    let uid = fs::metadata("/proc/self")
        .map_err(|_| WatchError::Secret)?
        .uid();
    if !m.is_file()
        || m.file_type().is_symlink()
        || m.uid() != uid
        || m.mode() & 0o7777 != 0o600
        || m.nlink() != 1
    {
        return Err(WatchError::Secret);
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn private_lock(path: &Path) -> Result<File, WatchError> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(0o400000 | 0o4000)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(0o400000 | 0o4000)
            .open(path)
            .map_err(|_| WatchError::Evidence)?,
        Err(_) => return Err(WatchError::Evidence),
    };
    private_file(path)?;
    file.try_lock().map_err(|e| match e {
        TryLockError::WouldBlock => WatchError::Busy,
        _ => WatchError::Evidence,
    })?;
    if file.metadata().map_err(|_| WatchError::Evidence)?.len() != 0 {
        return Err(WatchError::Evidence);
    }
    Ok(file)
}
#[cfg(target_os = "linux")]
fn exclusive_write(path: &Path, bytes: &[u8]) -> Result<(), WatchError> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(0o400000 | 0o4000)
        .open(path)
        .map_err(|_| WatchError::Evidence)?;
    f.write_all(bytes)
        .and_then(|()| f.sync_all())
        .map_err(|_| WatchError::Evidence)
}
#[cfg(not(target_os = "linux"))]
fn private_directory(_: &Path) -> Result<(), WatchError> {
    Err(WatchError::Evidence)
}
#[cfg(not(target_os = "linux"))]
fn private_file(_: &Path) -> Result<(), WatchError> {
    Err(WatchError::Secret)
}
#[cfg(not(target_os = "linux"))]
fn private_lock(_: &Path) -> Result<File, WatchError> {
    Err(WatchError::Evidence)
}
#[cfg(not(target_os = "linux"))]
fn exclusive_write(_: &Path, _: &[u8]) -> Result<(), WatchError> {
    Err(WatchError::Evidence)
}

async fn run(config: Config, mode: Mode) -> Result<(), WatchError> {
    let rpc_url = private_text(&config.rpc_url_file)?;
    let url = reqwest::Url::parse(&rpc_url).map_err(|_| WatchError::Secret)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(WatchError::Secret);
    }
    let chain = RpcChain::new(rpc_url);
    let key = address(&config.snapshot_account)?;
    let genesis = tokio::time::timeout(Duration::from_secs(15), chain.client().get_genesis_hash())
        .await
        .map_err(|_| WatchError::Chain)?
        .map_err(|_| WatchError::Chain)?;
    if genesis.to_bytes() != MAINNET_GENESIS {
        return Err(WatchError::Receiver);
    }
    let (state, producer_commitment) =
        tokio::time::timeout(Duration::from_secs(15), receiver(&chain, key))
            .await
            .map_err(|_| WatchError::Chain)??;
    let plan = plan(&config, &state, producer_commitment)?;
    let publisher = address(&config.publisher)?.to_bytes();
    if !state.configuration().publishers().contains(&publisher) {
        return Err(WatchError::Policy);
    }
    let budget = Arc::new(
        SharedRequestBudget::new(BudgetConfig {
            state_file: config.request_budget.state_file,
            limits: BudgetLimits {
                total_per_second: config.request_budget.total_per_second,
                background_per_second: config.request_budget.background_per_second,
                reserved_exit_per_second: config.request_budget.reserved_exit_per_second,
            },
        })
        .map_err(|_| WatchError::Policy)?,
    );
    let store = HistoryStore::new(config.state_file, *state.configuration())
        .map_err(|_| WatchError::History)?;
    if mode == Mode::Check {
        println!(
            "{}",
            json!({"schema":"shortwin-price-watch-check/v1","genesis_verified":true,"receiver_verified":true,
            "producer_commitment_hex":hex(&plan.policy_commitment()),"financial_policy_hash_hex":hex(&plan.configuration.model().policy_hash),"source":"trusted_jupiter_bid_ask_estimates",
            "polling_started":false,"transaction_submitted":false})
        );
        return Ok(());
    }
    if matches!(mode, Mode::InitializeBudget | Mode::Reconcile) {
        let publication = config.publication.as_ref().ok_or(WatchError::Policy)?;
        let budget = publication.budget()?;
        let result = if mode == Mode::InitializeBudget {
            let now = clock(&chain).await?;
            budget
                .initialize(u64::try_from(now.unix_timestamp).map_err(|_| WatchError::Clock)?)
                .map_err(|_| WatchError::History)?;
            json!({"disposition":"initialized_no_transaction"})
        } else {
            let receipt = reconcile_publication(&chain, &budget)
                .await
                .map_err(WatchError::Publication)?;
            json!({"checked_pending":receipt.had_pending,"last_resolution":receipt.resolution.as_ref().map(resolution_json)})
        };
        println!(
            "{}",
            json!({"schema":"shortwin-publication-budget/v1","result":result,
            "transaction_submitted":false,"reservations_refunded":false})
        );
        return Ok(());
    }
    if matches!(mode, Mode::Snapshot | Mode::Publish) {
        let collector = store.load().map_err(|_| WatchError::History)?;
        let now = clock(&chain).await?;
        let report = collector
            .prepare_snapshot(
                publisher,
                state
                    .last_sequence()
                    .checked_add(1)
                    .ok_or(WatchError::Receiver)?,
                now,
            )
            .map_err(|_| WatchError::History)?;
        let mut check = state;
        check
            .accept(
                CallerAuthenticatedSigner::from_verified_runtime_signer(publisher),
                &report,
                now,
            )
            .map_err(|_| WatchError::Receiver)?;
        if mode == Mode::Snapshot {
            println!("{}", snapshot_json(&report, key));
        } else {
            use solana_signer::Signer;
            let publication = config.publication.as_ref().ok_or(WatchError::Policy)?;
            publication
                .settings()
                .validate()
                .map_err(|_| WatchError::Policy)?;
            let key_bytes = private_read(&publication.reporter_keypair_file, MAX_SECRET)?;
            let raw: Vec<u8> =
                serde_json::from_slice(&key_bytes).map_err(|_| WatchError::Secret)?;
            let keypair = solana_keypair::Keypair::try_from(raw.as_slice())
                .map_err(|_| WatchError::Secret)?;
            if keypair.pubkey().to_bytes() != publisher {
                return Err(WatchError::Policy);
            }
            let receipt = publish_snapshot(
                &chain,
                &publication.budget()?,
                &keypair,
                key,
                producer_commitment,
                &report,
                publication.settings(),
            )
            .await
            .map_err(WatchError::Publication)?;
            println!(
                "{}",
                json!({"schema":"shortwin-on-demand-publication/v1","signature":receipt.signature,
                "slot":receipt.slot.to_string(),"actual_fee_lamports":receipt.actual_fee_lamports.map(|fee|fee.to_string()),"actual_fee_known":receipt.actual_fee_lamports.is_some(),
                "reserved_fee_lamports":receipt.reserved_fee_lamports.to_string(),"sequence":receipt.sequence.to_string(),
                "confirmed_success":receipt.success,
                "transaction_submitted":true,"new_signature_retry":false})
            );
            if !receipt.success {
                return Err(WatchError::Publication(PublisherError::ConfirmedFailure));
            }
        }
        return Ok(());
    }
    let mut journal = EvidenceJournal::open(&config.evidence_directory, config.evidence_max_bytes)?;
    let mut collector = match store.load() {
        Ok(state) => state,
        Err(shortwin_keeper::offchain_price_history::HistoryError::Missing) => store
            .create(state.configured_at())
            .map_err(|_| WatchError::History)?,
        Err(_) => return Err(WatchError::History),
    };
    let source = TrustedJupiterSource::new(&private_text(&config.api_key_file)?, budget)
        .map_err(|_| WatchError::Config)?;
    loop {
        let result = source
            .collect(
                &plan,
                &chain,
                collector
                    .observation_sequence()
                    .checked_add(1)
                    .ok_or(WatchError::History)?,
            )
            .await;
        let (observation, classification, now, source_error) = match result {
            Ok(reference) => {
                journal.retain(&reference)?;
                (
                    reference.observation,
                    "operator_estimate",
                    reference.window.end,
                    None,
                )
            }
            Err(error) => {
                let now = clock(&chain).await?;
                let digest: Address = Sha256::digest(format!(
                    "shortwin-trusted-source-unknown-v1:{error:?}:{}:{}",
                    now.slot, now.unix_timestamp
                ))
                .into();
                (
                    OperatorObservation::unknown(
                        ObservationWindow {
                            start: now,
                            end: now,
                        },
                        digest,
                    )
                    .map_err(|_| WatchError::History)?,
                    "unknown",
                    now,
                    Some(format!("{error:?}")),
                )
            }
        };
        collector = store
            .record(observation, now)
            .map_err(|_| WatchError::History)?;
        println!(
            "{}",
            json!({"schema":"shortwin-price-watch-tick/v1","classification":classification,
            "slot":now.slot.to_string(),"unix_timestamp":now.unix_timestamp,"local_observations":collector.observation_sequence().to_string(),
            "completed_epochs":collector.completed_epochs(),"source_error_category":source_error,"history_chain_digest_hex":hex(&collector.evidence_chain_digest()),
            "source_state_age_verified":false,"transaction_submitted":false})
        );
        if mode == Mode::Once {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(config.poll_interval_ms)).await;
    }
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = match args(std::env::args_os().skip(1)) {
        Ok(Some(a)) => a,
        Ok(None) => {
            println!(
                "{USAGE}\nTrusted off-chain estimates. Only explicit --publish signs/submits one fixed snapshot update; --watch never publishes."
            );
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(1);
        }
    };
    match load(&args.config) {
        Ok(c) => match run(c, args.mode).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shortwin_price_report::{
        CompletedBucket, Configuration, CostMarks, Domain, HistoryPolicy, ModelBinding,
        trusted_snapshot::{AttestedBucket, AttestedHistory},
    };
    #[test]
    fn command_modes_are_explicit_no_send_or_keypair_arguments_exist() {
        for mode in [
            "--check",
            "--once",
            "--watch",
            "--snapshot",
            "--publish",
            "--initialize-budget",
            "--reconcile",
        ] {
            let parsed = args(["--config", "test.toml", mode].map(OsString::from))
                .unwrap()
                .unwrap();
            assert_eq!(parsed.config, PathBuf::from("test.toml"));
        }
        for raw in [
            vec!["--config", "test.toml", "--send"],
            vec!["--config", "test.toml", "--snapshot", "--keypair"],
            vec!["--config", "test.toml"],
            vec!["--watch"],
        ] {
            assert!(args(raw.into_iter().map(OsString::from)).is_err());
        }
    }
    #[test]
    fn snapshot_export_is_unsigned_exact_integer_args_and_explicit_trust() {
        let domain = Domain {
            genesis_hash: MAINNET_GENESIS,
            program: [2; 32],
            config: [3; 32],
            market: [4; 32],
            base_mint: [5; 32],
            base_token_program: [6; 32],
            quote_mint: [7; 32],
            quote_token_program: [8; 32],
            base_decimals: 6,
            quote_decimals: 9,
        };
        let window = ObservationWindow {
            start: RuntimeClock {
                slot: 1,
                unix_timestamp: 1,
            },
            end: RuntimeClock {
                slot: 2,
                unix_timestamp: 2,
            },
        };
        let mut buckets = [None; 8];
        buckets[0] = Some(AttestedBucket {
            bucket: CompletedBucket {
                epoch: 0,
                first_observed: window.start,
                last_observed: window.end,
                lower_min: 1,
                central_last: 2,
                upper_max: 3,
                observations: 2,
            },
            largest_gap_slots: 1,
            largest_gap_seconds: 1,
        });
        let report = SnapshotReport {
            version: 2,
            domain,
            quantity: 9_007_199_254_740_993,
            model: ModelBinding {
                version: 1,
                epoch: 1,
                policy_hash: [9; 32],
            },
            configuration_epoch: 1,
            publisher: [10; 32],
            sequence: u64::MAX,
            window,
            observation_digest: [11; 32],
            status: ReportStatus::Finite(CostMarks::new(1, 2, 3).unwrap()),
            history: AttestedHistory::new(buckets).unwrap(),
        };
        let value = snapshot_json(&report, Pubkey::new_from_array([12; 32]));
        assert_eq!(value["quantity_raw"], json!("9007199254740993"));
        assert_eq!(value["args"]["sequence"], json!("18446744073709551615"));
        assert_eq!(value["args"]["history"].as_array().unwrap().len(), 8);
        assert_eq!(value["args"]["history"][7]["observations"], json!("0"));
        assert_eq!(value["transaction_signed"], false);
        assert_eq!(value["transaction_submitted"], false);
        assert_eq!(value["operator_attested_history"], true);
        assert_eq!(value["source_state_age_verified"], false);
        assert_eq!(value["executable_minimum_verified"], false);
        // Parse EVERY exported compact argument into the current actual Anchor ABI
        // and compare exact wire bytes. A field omission/rename/type drift fails here.
        use anchor_lang::AnchorSerialize;
        use shortwin::state::{
            AttestedPriceBucketArgs, PriceReportStatusArgs, PublishPriceSnapshotArgs,
        };
        let wire = &value["args"];
        assert!(wire.get("configuration_epoch").is_none()); // immutable account supplies it
        let u = |v: &Value| v.as_str().unwrap().parse::<u64>().unwrap();
        let signed = |v: &Value| v.as_str().unwrap().parse::<i64>().unwrap();
        let mut history = [AttestedPriceBucketArgs::default(); 8];
        for (index, b) in wire["history"].as_array().unwrap().iter().enumerate() {
            history[index] = AttestedPriceBucketArgs {
                epoch: u(&b["epoch"]),
                first_slot: u(&b["first_slot"]),
                first_ts: signed(&b["first_ts"]),
                last_slot: u(&b["last_slot"]),
                last_ts: signed(&b["last_ts"]),
                lower_min: u(&b["lower_min"]),
                central_last: u(&b["central_last"]),
                upper_max: u(&b["upper_max"]),
                observations: u(&b["observations"]),
                largest_gap_slots: u(&b["largest_gap_slots"]),
                largest_gap_seconds: u(&b["largest_gap_seconds"]),
            };
        }
        let finite = &wire["status"]["finite"];
        let parsed = PublishPriceSnapshotArgs {
            version: wire["version"].as_u64().unwrap() as u16,
            sequence: u(&wire["sequence"]),
            start_slot: u(&wire["start_slot"]),
            start_ts: signed(&wire["start_ts"]),
            end_slot: u(&wire["end_slot"]),
            end_ts: signed(&wire["end_ts"]),
            observation_digest: serde_json::from_value(wire["observation_digest"].clone()).unwrap(),
            status: PriceReportStatusArgs::Finite {
                lower: u(&finite["lower"]),
                central: u(&finite["central"]),
                upper: u(&finite["upper"]),
            },
            history_count: wire["history_count"].as_u64().unwrap() as u8,
            history,
        };
        let expected = shortwin_keeper::snapshot_publisher::snapshot_args(&report).unwrap();
        let mut actual_bytes = Vec::new();
        let mut expected_bytes = Vec::new();
        parsed.serialize(&mut actual_bytes).unwrap();
        expected.serialize(&mut expected_bytes).unwrap();
        assert_eq!(actual_bytes, expected_bytes);
        assert_eq!(actual_bytes.len(), 804);
    }
    #[test]
    fn config_template_is_parseable_but_refuses_placeholder_identities() {
        let raw = include_str!("../../price-watch.example.toml");
        let config: Config = toml::from_str(raw).unwrap();
        assert!(address(&config.snapshot_account).is_err());
        assert!(digest(&config.account_policy_digest_hex).is_err());
        let c = Configuration::new(
            Domain {
                genesis_hash: MAINNET_GENESIS,
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
            1,
            ModelBinding {
                version: 1,
                epoch: 1,
                policy_hash: [9; 32],
            },
            1,
            &[[10; 32]],
            HistoryPolicy {
                max_age_slots: 4,
                max_age_seconds: 4,
                max_window_slots: 4,
                max_window_seconds: 4,
                max_update_gap_slots: 4,
                max_update_gap_seconds: 4,
                epoch_seconds: 10,
                minimum_completed_epochs: 2,
            },
        )
        .unwrap();
        assert!(
            plan(
                &config,
                &SnapshotState::new(
                    c,
                    RuntimeClock {
                        slot: 0,
                        unix_timestamp: 0
                    }
                )
                .unwrap(),
                [0; 32],
            )
            .is_err()
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn secrets_and_evidence_require_private_files_quota_and_process_lock() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "shortwin-watch-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let secret = directory.join("secret");
        exclusive_write(&secret, b"dummy-secret-never-live\n").unwrap();
        assert_eq!(private_text(&secret).unwrap(), "dummy-secret-never-live");
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(private_text(&secret).is_err());
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
        let alias = directory.join("alias");
        symlink(&secret, &alias).unwrap();
        assert!(private_text(&alias).is_err());
        fs::remove_file(&alias).unwrap();
        let mut journal = EvidenceJournal::open(&directory, 1_000_000).unwrap();
        assert!(matches!(
            EvidenceJournal::open(&directory, 1_000_000),
            Err(WatchError::Busy)
        ));
        let window = ObservationWindow {
            start: RuntimeClock {
                slot: 1,
                unix_timestamp: 1,
            },
            end: RuntimeClock {
                slot: 2,
                unix_timestamp: 2,
            },
        };
        let marks = CostMarks::new(1, 2, 3).unwrap();
        let reference = CollectedReference {
            observation: OperatorObservation::qualified_finite(window, [1; 32], marks).unwrap(),
            window,
            bid_lamports: 1,
            ask_lamports: 3,
            marks,
            digest: [1; 32],
            bid_raw_response: b"bid bytes".to_vec(),
            ask_raw_response: b"ask bytes".to_vec(),
        };
        journal.maximum = journal.used;
        assert!(journal.retain(&reference).is_err());
        assert!(
            !directory
                .join(format!("{}.bid.json", hex(&[1; 32])))
                .exists()
        );
        journal.maximum = 1_000_000;
        journal.retain(&reference).unwrap();
        let bytes = fs::read(directory.join(format!("{}.bid.json", hex(&[1; 32])))).unwrap();
        assert_eq!(bytes, b"bid bytes");
        drop(journal);
        let reopened = EvidenceJournal::open(&directory, 1_000_000).unwrap();
        assert!(reopened.used > bytes.len() as u64);
        drop(reopened);
        fs::remove_dir_all(directory).unwrap();
    }
}
