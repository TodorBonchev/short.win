//! Read-only one-shot Jupiter quote-search diagnostic. No signing keypair,
//! transaction submission, report publication or financial reference is supported.

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shortwin_keeper::{
    price_budget::{BudgetConfig, BudgetLimits, RequestClass, SharedRequestBudget},
    price_http::{BuildError, JUPITER_BUILD_URL, JupiterBuildClient, MAINNET_GENESIS},
    price_search::{
        Address, ApiCandidateAskUpper, SearchIdentity, SearchOutcome, SearchPolicy, SearchSession,
        SearchSummary, SolLamports, TokenUnits,
    },
};
use solana_pubkey::Pubkey;

const USAGE: &str = "usage: shortwin-price-probe --config <price-probe.toml> --once";
const MAX_CONFIG_BYTES: u64 = 65_536;
const MAX_KEY_BYTES: u64 = 4_096;
const MAX_SEARCH_WINDOW_MS: u64 = 300_000;
const MAX_REQUEST_TIMEOUT_MS: u64 = 30_000;

/// All errors are fixed categories/field names. Never retain parser/file/provider
/// errors, configuration snippets, unknown argument values or credential bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeError {
    Arguments,
    ConfigRead,
    ConfigTooLarge,
    ConfigSyntax,
    InvalidField(&'static str),
    InvalidPlan,
    CredentialRead,
    CredentialFile,
    CredentialValue,
    ClientInitialization,
    RequestBudgetConfig,
    OutputDirectory,
    OutputWrite,
    Clock,
    SearchState,
}
impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "price probe: {self:?}")
    }
}
impl std::error::Error for ProbeError {}

struct Args {
    config: PathBuf,
}
fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Option<Args>, ProbeError> {
    let args: Vec<_> = args.into_iter().take(5).collect();
    if args.len() == 1 && matches!(args[0].to_str(), Some("--help" | "-h")) {
        return Ok(None);
    }
    if args.len() != 3 {
        return Err(ProbeError::Arguments);
    }
    let mut config = None;
    let mut once = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--config") if config.is_none() => {
                let path = args.next().ok_or(ProbeError::Arguments)?;
                if path.is_empty() || path.to_str().is_some_and(|s| s.starts_with('-')) {
                    return Err(ProbeError::Arguments);
                }
                config = Some(PathBuf::from(path));
            }
            Some("--once") if !once => once = true,
            _ => return Err(ProbeError::Arguments),
        }
    }
    if !once {
        return Err(ProbeError::Arguments);
    }
    Ok(Some(Args {
        config: config.ok_or(ProbeError::Arguments)?,
    }))
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    api_key_file: PathBuf,
    request_budget: RawBudget,
    output_directory: Option<PathBuf>,
    chain_genesis: String,
    program: String,
    configuration: String,
    market: String,
    input_mint: String,
    input_token_program: String,
    input_decimals: u8,
    output_mint: String,
    output_token_program: String,
    output_decimals: u8,
    exact_debt_raw: String,
    policy_digest_hex: String,
    policy_version: u32,
    account_policy_digest_hex: String,
    taker: String,
    model_epoch: u64,
    observation_epoch: u64,
    report_sequence: u64,
    candidate_input_lamports: Vec<String>,
    max_input_lamports: String,
    max_requests: usize,
    max_response_bytes: usize,
    request_timeout_ms: u64,
    search_window_ms: u64,
    slippage_bps: u16,
    /// Declared plan budget, not discovered/verified from the provider account.
    declared_account_requests_per_minute: u32,
    reserved_exit_requests_per_minute: u32,
    min_request_spacing_ms: u64,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBudget {
    state_file: PathBuf,
    total_per_second: u16,
    background_per_second: u16,
    reserved_exit_per_second: u16,
}
impl RawBudget {
    fn config(&self) -> BudgetConfig {
        BudgetConfig {
            state_file: self.state_file.clone(),
            limits: BudgetLimits {
                total_per_second: self.total_per_second,
                background_per_second: self.background_per_second,
                reserved_exit_per_second: self.reserved_exit_per_second,
            },
        }
    }
}

struct Config {
    api_key_file: PathBuf,
    request_budget: BudgetConfig,
    output_directory: Option<PathBuf>,
    identity: SearchIdentity,
    policy: SearchPolicy,
    inputs: Vec<SolLamports>,
    min_request_spacing_ms: u64,
    declared_account_requests_per_minute: u32,
    reserved_exit_requests_per_minute: u32,
}

fn address(raw: &str, field: &'static str) -> Result<Address, ProbeError> {
    Pubkey::from_str(raw)
        .map(|key| key.to_bytes())
        .map_err(|_| ProbeError::InvalidField(field))
}
fn digest(raw: &str, field: &'static str) -> Result<Address, ProbeError> {
    if raw.len() != 64 {
        return Err(ProbeError::InvalidField(field));
    }
    let mut value = [0u8; 32];
    for (i, pair) in raw.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let hex = |byte: u8| match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err(ProbeError::InvalidField(field)),
        };
        value[i] = hex(pair[0])? * 16 + hex(pair[1])?;
    }
    if value == [0; 32] {
        return Err(ProbeError::InvalidField(field));
    }
    Ok(value)
}

impl RawConfig {
    fn validate(self) -> Result<Config, ProbeError> {
        let request_budget = self.request_budget.config();
        request_budget
            .validate()
            .map_err(|_| ProbeError::RequestBudgetConfig)?;
        if self.api_key_file.as_os_str().is_empty() {
            return Err(ProbeError::InvalidField("api_key_file"));
        }
        if self
            .output_directory
            .as_ref()
            .is_some_and(|p| p.as_os_str().is_empty())
        {
            return Err(ProbeError::InvalidField("output_directory"));
        }
        if self.policy_version == 0
            || self.model_epoch == 0
            || self.observation_epoch == 0
            || self.report_sequence == 0
        {
            return Err(ProbeError::InvalidField("versions_and_epochs"));
        }
        if self.search_window_ms == 0
            || self.search_window_ms > MAX_SEARCH_WINDOW_MS
            || self.request_timeout_ms == 0
            || self.request_timeout_ms > MAX_REQUEST_TIMEOUT_MS
            || self.request_timeout_ms > self.search_window_ms
        {
            return Err(ProbeError::InvalidField("request_search_time_limits"));
        }
        if self.declared_account_requests_per_minute == 0
            || self.declared_account_requests_per_minute > 60_000
            || self.reserved_exit_requests_per_minute == 0
            || self.reserved_exit_requests_per_minute >= self.declared_account_requests_per_minute
        {
            return Err(ProbeError::InvalidField("declared_request_budget"));
        }
        let allowance = u64::from(
            self.declared_account_requests_per_minute - self.reserved_exit_requests_per_minute,
        );
        if self.min_request_spacing_ms < 60_000u64.div_ceil(allowance)
            || self.min_request_spacing_ms > self.search_window_ms
        {
            return Err(ProbeError::InvalidField("min_request_spacing_ms"));
        }
        let identity = SearchIdentity {
            chain_genesis: address(&self.chain_genesis, "chain_genesis")?,
            program: address(&self.program, "program")?,
            configuration: address(&self.configuration, "configuration")?,
            market: address(&self.market, "market")?,
            input_mint: address(&self.input_mint, "input_mint")?,
            input_token_program: address(&self.input_token_program, "input_token_program")?,
            input_decimals: self.input_decimals,
            output_mint: address(&self.output_mint, "output_mint")?,
            output_token_program: address(&self.output_token_program, "output_token_program")?,
            output_decimals: self.output_decimals,
            exact_debt: TokenUnits::parse(&self.exact_debt_raw)
                .map_err(|_| ProbeError::InvalidField("exact_debt_raw"))?,
            policy_digest: digest(&self.policy_digest_hex, "policy_digest_hex")?,
            policy_version: self.policy_version,
            account_policy_digest: digest(
                &self.account_policy_digest_hex,
                "account_policy_digest_hex",
            )?,
            taker: address(&self.taker, "taker")?,
            model_epoch: self.model_epoch,
            observation_epoch: self.observation_epoch,
            report_sequence: self.report_sequence,
            window_start_ms: 0,
            window_end_ms: self.search_window_ms,
        };
        if identity.chain_genesis != MAINNET_GENESIS {
            return Err(ProbeError::InvalidField("chain_genesis"));
        }
        let policy = SearchPolicy {
            max_input_lamports: SolLamports::parse(&self.max_input_lamports)
                .map_err(|_| ProbeError::InvalidField("max_input_lamports"))?,
            max_requests: self.max_requests,
            max_response_bytes: self.max_response_bytes,
            request_timeout_ms: self.request_timeout_ms,
            max_window_ms: self.search_window_ms,
            slippage_bps: self.slippage_bps,
        };
        if self.candidate_input_lamports.len() > shortwin_keeper::price_search::MAX_CANDIDATES {
            return Err(ProbeError::InvalidPlan);
        }
        let inputs = self
            .candidate_input_lamports
            .iter()
            .map(|raw| {
                SolLamports::parse(raw)
                    .map_err(|_| ProbeError::InvalidField("candidate_input_lamports"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        SearchSession::new(identity, policy, &inputs).map_err(|_| ProbeError::InvalidPlan)?;
        Ok(Config {
            api_key_file: self.api_key_file,
            request_budget,
            output_directory: self.output_directory,
            identity,
            policy,
            inputs,
            min_request_spacing_ms: self.min_request_spacing_ms,
            declared_account_requests_per_minute: self.declared_account_requests_per_minute,
            reserved_exit_requests_per_minute: self.reserved_exit_requests_per_minute,
        })
    }
}

fn read_bounded(
    path: &Path,
    limit: u64,
    read_error: ProbeError,
    size_error: ProbeError,
) -> Result<Vec<u8>, ProbeError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| read_error)?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(size_error);
    }
    let file = File::open(path).map_err(|_| read_error)?;
    if !file.metadata().map_err(|_| read_error)?.is_file() {
        return Err(size_error);
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| read_error)?;
    if bytes.len() as u64 > limit {
        return Err(size_error);
    }
    Ok(bytes)
}

fn load_config(path: &Path) -> Result<Config, ProbeError> {
    let bytes = read_bounded(
        path,
        MAX_CONFIG_BYTES,
        ProbeError::ConfigRead,
        ProbeError::ConfigTooLarge,
    )?;
    let text = std::str::from_utf8(&bytes).map_err(|_| ProbeError::ConfigSyntax)?;
    let raw: RawConfig = toml::from_str(text).map_err(|_| ProbeError::ConfigSyntax)?;
    raw.validate()
}

fn load_credential(path: &Path) -> Result<String, ProbeError> {
    let bytes = read_bounded(
        path,
        MAX_KEY_BYTES,
        ProbeError::CredentialRead,
        ProbeError::CredentialFile,
    )?;
    let text = String::from_utf8(bytes).map_err(|_| ProbeError::CredentialValue)?;
    let value = text.trim_end_matches(['\r', '\n']);
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(ProbeError::CredentialValue);
    }
    Ok(value.to_owned())
}

fn now_ms(started: Instant) -> Result<u64, ProbeError> {
    u64::try_from(started.elapsed().as_millis()).map_err(|_| ProbeError::Clock)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn exclusive_write(path: &Path, bytes: &[u8]) -> Result<(), ProbeError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|_| ProbeError::OutputWrite)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| ProbeError::OutputWrite)
}

fn prepare_output(path: Option<&Path>) -> Result<(), ProbeError> {
    if let Some(path) = path {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(path)
            .map_err(|_| ProbeError::OutputDirectory)?;
    }
    Ok(())
}

fn retain_evidence(
    path: Option<&Path>,
    candidate: Option<&ApiCandidateAskUpper>,
    summary: &Value,
) -> Result<(), ProbeError> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(candidate) = candidate {
        exclusive_write(
            &path.join("candidate-response.json"),
            candidate.raw_response(),
        )?;
        let checksum = format!(
            "{}  candidate-response.json\n",
            sha256(candidate.raw_response())
        );
        exclusive_write(&path.join("candidate-response.sha256"), checksum.as_bytes())?;
    }
    let bytes = serde_json::to_vec_pretty(summary).map_err(|_| ProbeError::OutputWrite)?;
    exclusive_write(&path.join("summary.json"), &bytes)
}

fn public_key(key: Address) -> String {
    Pubkey::new_from_array(key).to_string()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn describe(
    config: &Config,
    outcome: &SearchOutcome,
    elapsed_ms: u64,
    errors: &[BuildError],
) -> (Value, u8) {
    let (classification, candidate, summary, exit) = match outcome {
        SearchOutcome::ApiCandidate { candidate, summary } => {
            ("api_candidate_only", Some(candidate.as_ref()), summary, 0)
        }
        SearchOutcome::Unknown(summary) => ("unknown", None, summary, 2),
        SearchOutcome::UncoveredWithinSearchBudget(summary) => {
            ("uncovered_within_search_budget", None, summary, 3)
        }
    };
    let id = &config.identity;
    let candidate = candidate.map(|candidate| {
        json!({
            "input_lamports":candidate.input_lamports().0.to_string(),
            "api_expected_output_raw":candidate.expected_output().0.to_string(),
            "api_declared_minimum_output_raw":candidate.minimum_output().0.to_string(),
            "executable_minimum_verified":candidate.executable_minimum_verified(),
            "local_request_started_ms":candidate.request_started_ms(),
            "local_received_ms":candidate.received_ms(),
            "raw_response_sha256":sha256(candidate.raw_response()),
        })
    });
    let value = json!({
        "schema":"shortwin-price-probe/v1","classification":classification,
        "endpoint":JUPITER_BUILD_URL,"local_elapsed_ms":elapsed_ms,
        "transaction_simulated":false,"route_authorized":false,"source_state_age_verified":false,
        "financial_reference_qualified":false,"report_published":false,
        "executable_minimum_verified":false,
        "identity_origin":"operator_config_not_onchain_verified",
        "identity":{
            "chain_genesis":public_key(id.chain_genesis),"program":public_key(id.program),
            "configuration":public_key(id.configuration),"market":public_key(id.market),
            "input_mint":public_key(id.input_mint),"input_token_program":public_key(id.input_token_program),
            "input_decimals":id.input_decimals,"output_mint":public_key(id.output_mint),
            "output_token_program":public_key(id.output_token_program),"output_decimals":id.output_decimals,
            "exact_debt_raw":id.exact_debt.0.to_string(),"policy_digest_hex":hex(&id.policy_digest),
            "policy_version":id.policy_version,"account_policy_digest_hex":hex(&id.account_policy_digest),
            "taker":public_key(id.taker),"model_epoch":id.model_epoch,"observation_epoch":id.observation_epoch,
            "report_sequence":id.report_sequence,
        },
        "search_policy":{
            "candidate_input_lamports":config.inputs.iter().map(|x|x.0.to_string()).collect::<Vec<_>>(),
            "max_input_lamports":config.policy.max_input_lamports.0.to_string(),
            "max_requests":config.policy.max_requests,"max_response_bytes":config.policy.max_response_bytes,
            "request_timeout_ms":config.policy.request_timeout_ms,"search_window_ms":config.policy.max_window_ms,
            "slippage_bps":config.policy.slippage_bps,"min_request_spacing_ms":config.min_request_spacing_ms,
            "declared_account_requests_per_minute":config.declared_account_requests_per_minute,
            "reserved_exit_requests_per_minute":config.reserved_exit_requests_per_minute,
            "account_wide_budget_verified":false,
        },
        "shared_local_budget": {
            "enforced": true,
            "total_per_second": config.request_budget.limits.total_per_second,
            "background_per_second": config.request_budget.limits.background_per_second,
            "reserved_exit_per_second": config.request_budget.limits.reserved_exit_per_second,
            "other_api_consumers_included": false, "monthly_credits_enforced": false,
        },
        "search":summary_json(summary),
        "source_error_categories":errors.iter().map(|e|format!("{e:?}")).collect::<Vec<_>>(),
        "candidate":candidate,
    });
    (value, exit)
}

fn summary_json(summary: &SearchSummary) -> Value {
    json!({"planned_inputs":summary.planned_inputs,"issued_requests":summary.issued_requests,
        "completed_requests":summary.completed_requests,"qualifying_quotes":summary.qualifying_quotes,
        "insufficient_quotes":summary.insufficient_quotes,"stop_reason":format!("{:?}",summary.stop_reason),
        "unknown_observations":summary.unknown_observations.iter().map(|e|format!("{e:?}")).collect::<Vec<_>>(),
        "all_planned_inputs_observed":summary.all_planned_inputs_observed()})
}

async fn run(config: Config) -> Result<(Value, u8), ProbeError> {
    let budget = Arc::new(
        SharedRequestBudget::new(config.request_budget.clone())
            .map_err(|_| ProbeError::RequestBudgetConfig)?,
    );
    let client = JupiterBuildClient::new(
        &load_credential(&config.api_key_file)?,
        budget,
        RequestClass::Background,
    )
    .map_err(|_| ProbeError::ClientInitialization)?;
    prepare_output(config.output_directory.as_deref())?;
    let started = Instant::now();
    let mut session = SearchSession::new(config.identity, config.policy, &config.inputs)
        .map_err(|_| ProbeError::InvalidPlan)?;
    let mut last_started_ms: Option<u64> = None;
    let mut errors = Vec::new();
    loop {
        let now = now_ms(started)?;
        if let Some(last) = last_started_ms {
            let next = last
                .saturating_add(config.min_request_spacing_ms)
                .min(config.identity.window_end_ms);
            if now < next {
                tokio::time::sleep(Duration::from_millis(next - now)).await;
            }
        }
        let now = now_ms(started)?;
        let Some(request) = session
            .next_request(now)
            .map_err(|_| ProbeError::SearchState)?
        else {
            break;
        };
        last_started_ms = Some(request.started_ms());
        match client.fetch(request, now).await {
            Ok(quote) => session
                .record_quote(quote, now_ms(started)?)
                .map_err(|_| ProbeError::SearchState)?,
            Err(error) => {
                errors.push(error);
                session
                    .record_failure(request, error.observation_failure(), now_ms(started)?)
                    .map_err(|_| ProbeError::SearchState)?;
            }
        }
        // Do not sleep after the final completed attempt: that could age a valid
        // completed candidate past its window before producing the diagnostic.
        if request.ordinal() + 1 >= config.inputs.len().min(config.policy.max_requests) {
            break;
        }
    }
    let elapsed = now_ms(started)?;
    let outcome = session
        .finish(elapsed)
        .map_err(|_| ProbeError::SearchState)?;
    let (summary, exit) = describe(&config, &outcome, elapsed, &errors);
    let candidate = match &outcome {
        SearchOutcome::ApiCandidate { candidate, .. } => Some(candidate.as_ref()),
        _ => None,
    };
    retain_evidence(config.output_directory.as_deref(), candidate, &summary)?;
    Ok((summary, exit))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = match parse_args(std::env::args_os().skip(1)) {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!(
                "{USAGE}\nRead-only API candidate diagnostic; no simulation, signing, sending or publishing."
            );
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            return ExitCode::from(1);
        }
    };
    let result = match load_config(&args.config) {
        Ok(config) => run(config).await,
        Err(error) => Err(error),
    };
    match result {
        Ok((summary, exit)) => {
            println!("{summary}");
            ExitCode::from(exit)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shortwin_keeper::price_search::{
        ApiQuote, ApiQuoteFields, SwapMode, TOKEN_2022_PROGRAM, TOKEN_PROGRAM, WSOL_MINT,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    const SECRET: &str = "dummy-secret-content-never-live";
    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "shortwin-price-probe-{}-{nonce}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn raw_config() -> RawConfig {
        RawConfig {
            api_key_file: PathBuf::from("dummy-api-key"),
            request_budget: RawBudget {
                state_file: PathBuf::from("/private/not-opened-by-config-tests/requests.state"),
                total_per_second: 10,
                background_per_second: 6,
                reserved_exit_per_second: 3,
            },
            output_directory: None,
            chain_genesis: public_key(MAINNET_GENESIS),
            program: public_key([2; 32]),
            configuration: public_key([3; 32]),
            market: public_key([4; 32]),
            input_mint: public_key(WSOL_MINT),
            input_token_program: public_key(TOKEN_PROGRAM),
            input_decimals: 9,
            output_mint: public_key([5; 32]),
            output_token_program: public_key(TOKEN_2022_PROGRAM),
            output_decimals: 6,
            exact_debt_raw: "100".to_owned(),
            policy_digest_hex: "06".repeat(32),
            policy_version: 1,
            account_policy_digest_hex: "07".repeat(32),
            taker: public_key([8; 32]),
            model_epoch: 1,
            observation_epoch: 2,
            report_sequence: 3,
            candidate_input_lamports: vec!["100".to_owned(), "200".to_owned()],
            max_input_lamports: "200".to_owned(),
            max_requests: 2,
            max_response_bytes: 16_384,
            request_timeout_ms: 1_000,
            search_window_ms: 10_000,
            slippage_bps: 50,
            declared_account_requests_per_minute: 60,
            reserved_exit_requests_per_minute: 30,
            min_request_spacing_ms: 2_000,
        }
    }
    fn valid_toml() -> String {
        let raw = raw_config();
        format!(
            r#"api_key_file = "dummy-api-key"
chain_genesis = "{}"
program = "{}"
configuration = "{}"
market = "{}"
input_mint = "{}"
input_token_program = "{}"
input_decimals = 9
output_mint = "{}"
output_token_program = "{}"
output_decimals = 6
exact_debt_raw = "100"
policy_digest_hex = "{}"
policy_version = 1
account_policy_digest_hex = "{}"
taker = "{}"
model_epoch = 1
observation_epoch = 2
report_sequence = 3
candidate_input_lamports = ["100", "200"]
max_input_lamports = "200"
max_requests = 2
max_response_bytes = 16384
request_timeout_ms = 1000
search_window_ms = 10000
slippage_bps = 50
declared_account_requests_per_minute = 60
reserved_exit_requests_per_minute = 30
min_request_spacing_ms = 2000
[request_budget]
state_file = "/private/not-opened-by-config-tests/requests.state"
total_per_second = 10
background_per_second = 6
reserved_exit_per_second = 3
"#,
            raw.chain_genesis,
            raw.program,
            raw.configuration,
            raw.market,
            raw.input_mint,
            raw.input_token_program,
            raw.output_mint,
            raw.output_token_program,
            raw.policy_digest_hex,
            raw.account_policy_digest_hex,
            raw.taker
        )
    }
    fn args(values: &[&str]) -> Result<Option<Args>, ProbeError> {
        parse_args(values.iter().map(|value| OsString::from(*value)))
    }

    #[test]
    fn only_config_and_once_are_accepted_in_either_order() {
        for values in [
            ["--config", "probe.toml", "--once"],
            ["--once", "--config", "probe.toml"],
        ] {
            assert_eq!(
                args(&values).unwrap().unwrap().config,
                PathBuf::from("probe.toml")
            );
        }
        assert!(args(&["--help"]).unwrap().is_none());
        for values in [
            vec![],
            vec!["--once"],
            vec!["--config", "probe.toml"],
            vec!["--config", "--once", "--once"],
            vec!["--config", "probe.toml", "--send"],
            vec!["--config", "probe.toml", "--publish"],
            vec!["--keypair", SECRET, "--once"],
            vec!["--config", "probe.toml", "--once", SECRET],
            vec!["--config", "probe.toml", "--config", "other.toml", "--once"],
        ] {
            let error = args(&values).err().unwrap();
            assert_eq!(error, ProbeError::Arguments);
            assert!(!format!("{error:?} {error}").contains(SECRET));
        }
    }

    #[test]
    fn explicit_identity_units_digest_and_policy_survive_strict_config() {
        let raw: RawConfig = toml::from_str(&valid_toml()).unwrap();
        let config = raw.validate().unwrap();
        assert_eq!(config.identity.exact_debt, TokenUnits(100));
        assert_eq!(config.identity.input_mint, WSOL_MINT);
        assert_eq!(config.identity.policy_digest, [6; 32]);
        assert_eq!(config.inputs, vec![SolLamports(100), SolLamports(200)]);
        assert_eq!(config.identity.window_start_ms, 0);
        assert_eq!(config.identity.window_end_ms, 10_000);
        assert_eq!(config.min_request_spacing_ms, 2_000);
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn invalid_key_amount_decimal_digest_duplicates_and_zero_limits_refuse() {
        let changes: &[fn(&mut RawConfig)] = &[
            |r| r.chain_genesis = public_key([1; 32]),
            |r| r.request_budget.total_per_second = 0,
            |r| r.request_budget.background_per_second = 8,
            |r| r.request_budget.state_file = PathBuf::from("relative/state"),
            |r| r.market = "invalid".to_owned(),
            |r| r.market = public_key([0; 32]),
            |r| r.exact_debt_raw = "0".to_owned(),
            |r| r.exact_debt_raw = "1.5".to_owned(),
            |r| r.exact_debt_raw = "18446744073709551616".to_owned(),
            |r| r.candidate_input_lamports = vec!["100".to_owned(), "100".to_owned()],
            |r| r.candidate_input_lamports = vec![],
            |r| r.candidate_input_lamports = vec!["0".to_owned()],
            |r| r.max_input_lamports = "99".to_owned(),
            |r| r.max_requests = 0,
            |r| r.max_response_bytes = 0,
            |r| r.max_response_bytes = 1_048_577,
            |r| r.input_decimals = 6,
            |r| r.output_token_program = public_key([99; 32]),
            |r| r.request_timeout_ms = 0,
            |r| r.request_timeout_ms = 30_001,
            |r| r.search_window_ms = 0,
            |r| r.search_window_ms = 300_001,
            |r| r.policy_version = 0,
            |r| r.model_epoch = 0,
            |r| r.observation_epoch = 0,
            |r| r.report_sequence = 0,
            |r| r.policy_digest_hex = "00".repeat(32),
            |r| r.policy_digest_hex = "AA".repeat(32),
            |r| r.account_policy_digest_hex = "not a digest".to_owned(),
            |r| r.slippage_bps = 10_000,
        ];
        for change in changes {
            let mut raw = raw_config();
            change(&mut raw);
            assert!(raw.validate().is_err());
        }
    }

    #[test]
    fn pacing_budget_must_leave_declared_exit_allowance() {
        let changes: &[fn(&mut RawConfig)] = &[
            |r| r.declared_account_requests_per_minute = 0,
            |r| r.reserved_exit_requests_per_minute = 0,
            |r| r.reserved_exit_requests_per_minute = 60,
            |r| r.reserved_exit_requests_per_minute = 61,
            |r| r.min_request_spacing_ms = 1_999,
            |r| r.min_request_spacing_ms = 0,
            |r| r.min_request_spacing_ms = 10_001,
        ];
        for change in changes {
            let mut raw = raw_config();
            change(&mut raw);
            assert!(raw.validate().is_err());
        }
        let mut raw = raw_config();
        raw.declared_account_requests_per_minute = 61;
        raw.reserved_exit_requests_per_minute = 30;
        raw.min_request_spacing_ms = 1_935;
        assert!(raw.clone().validate().is_err());
        raw.min_request_spacing_ms = 1_936;
        assert!(raw.validate().is_ok());
    }

    #[test]
    fn example_placeholders_and_unknown_config_keys_refuse_without_echoing_content() {
        let example = include_str!("../../price-probe.example.toml");
        let raw: RawConfig = toml::from_str(example).unwrap();
        assert!(raw.validate().is_err());
        let directory = TestDirectory::new();
        let path = directory.path("config.toml");
        fs::write(&path, format!("{}\napi_key = \"{SECRET}\"\n", valid_toml())).unwrap();
        let error = load_config(&path).err().unwrap();
        assert_eq!(error, ProbeError::ConfigSyntax);
        assert!(!format!("{error:?} {error}").contains(SECRET));
        fs::write(&path, vec![b'x'; MAX_CONFIG_BYTES as usize + 1]).unwrap();
        assert_eq!(
            load_config(&path).err().unwrap(),
            ProbeError::ConfigTooLarge
        );
    }

    #[test]
    fn bounded_credential_read_refuses_nonregular_oversized_and_invalid_values() {
        let directory = TestDirectory::new();
        let key = directory.path("key");
        fs::write(&key, format!("{SECRET}\n")).unwrap();
        assert_eq!(load_credential(&key).unwrap(), SECRET);
        for bytes in [b"".as_slice(), b"bad\nembedded", b"bad key", b"\xff"] {
            fs::write(&key, bytes).unwrap();
            let error = load_credential(&key).unwrap_err();
            assert!(!format!("{error:?} {error}").contains(SECRET));
        }
        fs::write(&key, vec![b'a'; MAX_KEY_BYTES as usize + 1]).unwrap();
        assert_eq!(
            load_credential(&key).unwrap_err(),
            ProbeError::CredentialFile
        );
        assert_eq!(
            load_credential(&directory.0).unwrap_err(),
            ProbeError::CredentialFile
        );
        #[cfg(unix)]
        {
            let alias = directory.path("alias");
            std::os::unix::fs::symlink(&key, &alias).unwrap();
            assert_eq!(
                load_credential(&alias).unwrap_err(),
                ProbeError::CredentialFile
            );
        }
    }

    fn candidate_outcome(config: &Config) -> SearchOutcome {
        let mut session =
            SearchSession::new(config.identity, config.policy, &[SolLamports(100)]).unwrap();
        let request = session.next_request(0).unwrap().unwrap();
        let quote = ApiQuote::parse(
            request,
            ApiQuoteFields {
                input_mint: config.identity.input_mint,
                output_mint: config.identity.output_mint,
                in_amount: "100",
                expected_out_amount: "101",
                other_amount_threshold: "100",
                swap_mode: SwapMode::ExactIn,
                slippage_bps: 50,
                platform_fee_bps: 0,
                positive_slippage_fee_bps: 0,
                taker: config.identity.taker,
                account_policy_digest: config.identity.account_policy_digest,
            },
            SECRET.as_bytes(),
        )
        .unwrap();
        session.record_quote(quote, 1).unwrap();
        session.finish(1).unwrap()
    }

    #[test]
    fn summary_reports_api_only_without_provider_bodies_or_credentials() {
        let config = raw_config().validate().unwrap();
        let outcome = candidate_outcome(&config);
        let (summary, exit) = describe(&config, &outcome, 1, &[BuildError::RateLimited]);
        assert_eq!(exit, 0);
        assert_eq!(summary["classification"], "api_candidate_only");
        for field in [
            "transaction_simulated",
            "route_authorized",
            "source_state_age_verified",
            "financial_reference_qualified",
            "report_published",
            "executable_minimum_verified",
        ] {
            assert_eq!(summary[field], false);
        }
        assert_eq!(
            summary["search_policy"]["account_wide_budget_verified"],
            false
        );
        assert_eq!(summary["candidate"]["executable_minimum_verified"], false);
        assert_eq!(summary["shared_local_budget"]["enforced"], true);
        assert_eq!(summary["shared_local_budget"]["total_per_second"], 10);
        assert_eq!(summary["shared_local_budget"]["background_per_second"], 6);
        assert_eq!(
            summary["shared_local_budget"]["reserved_exit_per_second"],
            3
        );
        assert_eq!(
            summary["shared_local_budget"]["other_api_consumers_included"],
            false
        );
        assert_eq!(
            summary["shared_local_budget"]["monthly_credits_enforced"],
            false
        );
        assert!(!summary.to_string().contains(SECRET));
        assert!(!summary.to_string().contains("api_key_file"));
        assert_eq!(
            summary["candidate"]["raw_response_sha256"],
            sha256(SECRET.as_bytes())
        );
    }

    #[test]
    fn evidence_uses_exclusive_files_and_retains_exact_bytes_without_overwriting() {
        let directory = TestDirectory::new();
        let output = directory.path("evidence");
        let config = raw_config().validate().unwrap();
        let outcome = candidate_outcome(&config);
        let (summary, _) = describe(&config, &outcome, 1, &[]);
        let SearchOutcome::ApiCandidate { candidate, .. } = &outcome else {
            panic!("candidate")
        };
        prepare_output(Some(&output)).unwrap();
        retain_evidence(Some(&output), Some(candidate), &summary).unwrap();
        assert_eq!(
            fs::read(output.join("candidate-response.json")).unwrap(),
            SECRET.as_bytes()
        );
        assert_eq!(
            fs::read_to_string(output.join("candidate-response.sha256")).unwrap(),
            format!("{}  candidate-response.json\n", sha256(SECRET.as_bytes()))
        );
        assert_eq!(
            prepare_output(Some(&output)).unwrap_err(),
            ProbeError::OutputDirectory
        );
        assert_eq!(
            exclusive_write(&output.join("candidate-response.json"), b"replacement").unwrap_err(),
            ProbeError::OutputWrite
        );
        assert_eq!(
            fs::read(output.join("candidate-response.json")).unwrap(),
            SECRET.as_bytes()
        );
    }
}
