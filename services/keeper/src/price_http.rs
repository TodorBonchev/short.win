//! Read-only, bounded Jupiter Build transport for the fixed-q search foundation.
//!
//! Only the ordinary V2 ExactIn scalar header and selected fixed account bindings
//! are checked. Remaining route data, scratch identities, setup/cleanup, executable
//! identity, mint admission, net deltas and market-data age are NOT qualified here.
//! The returned type remains ApiQuote; nothing is signed, sent, simulated or priced.

use std::{str::FromStr, sync::Arc, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::{Client, StatusCode, Url, header::HeaderValue, redirect};
use serde::Deserialize;
use shortwin_jupiter_route::{
    AccountMeta, EVENT_AUTHORITY, JUPITER_PROGRAM, ParsedHeader, RouteKind,
};
use solana_pubkey::Pubkey;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::price_budget::{BudgetError, RequestClass, SharedRequestBudget};

use crate::price_search::{
    Address, ApiQuote, ApiQuoteFields, ObservationFailure, QuoteRequest, ResponseRejection,
    SolLamports, SwapMode, TokenUnits,
};

pub const JUPITER_BUILD_URL: &str = "https://api.jup.ag/swap/v2/build";
/// Mainnet genesis pinned by the release's read-only cluster identity evidence.
pub const MAINNET_GENESIS: Address = [
    69, 41, 105, 152, 166, 248, 226, 167, 132, 219, 93, 159, 149, 225, 143, 194, 63, 112, 68, 26,
    16, 57, 68, 104, 1, 8, 152, 121, 176, 140, 126, 240,
];
/// Conservative implementation ceilings, not complete route authorization.
pub const MAX_SWAP_ACCOUNTS: usize = 128;
pub const MAX_SWAP_DATA_BYTES: usize = 16_384;
pub const MAX_ROUTE_STEPS: u32 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientConfigError {
    InvalidApiKey,
    ClientInitialization,
}

/// Safe-to-log categories only. Provider bodies, API keys and reqwest errors are
/// deliberately absent; reqwest errors can otherwise expose URLs/context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildError {
    InvalidLocalTime,
    UnsupportedChain,
    Busy,
    Timeout,
    Transport,
    RateLimited,
    Budget(BudgetError),
    Redirect,
    HttpStatus(u16),
    BodyTooLarge,
    InvalidJson,
    InvalidAddress,
    InvalidBase64,
    InstructionResourceLimit,
    InvalidHeader,
    UnsupportedRoute,
    ScalarMismatch,
    FixedAccountMismatch,
    FeeDeclared,
    QuoteRejected(ResponseRejection),
}

impl std::fmt::Display for ClientConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Jupiter Build configuration: {self:?}")
    }
}
impl std::error::Error for ClientConfigError {}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Jupiter Build source: {self:?}")
    }
}
impl std::error::Error for BuildError {}

impl BuildError {
    /// All source failures stay Unknown in SearchSession. API text, including a
    /// no-route string, is never promoted to authenticated illiquidity evidence.
    pub fn observation_failure(self) -> ObservationFailure {
        match self {
            Self::Timeout | Self::InvalidLocalTime => ObservationFailure::Timeout,
            Self::RateLimited | Self::Budget(BudgetError::RateLimited { .. }) => {
                ObservationFailure::RateLimited
            }
            Self::BodyTooLarge => ObservationFailure::Rejected(ResponseRejection::BodyTooLarge),
            Self::QuoteRejected(error) => ObservationFailure::Rejected(error),
            _ => ObservationFailure::Transport,
        }
    }
}

/// Private credential/header and fixed endpoint. Deliberately does not implement
/// Debug or expose a general URL constructor. One operation per client, no queue;
/// public construction requires the shared local budget. Outside API consumers and monthly credits remain the operator’s responsibility.
pub struct JupiterBuildClient {
    client: Client,
    api_key: HeaderValue,
    endpoint: Url,
    active: Arc<Semaphore>,
    budget: Option<(Arc<SharedRequestBudget>, RequestClass)>,
}

impl JupiterBuildClient {
    pub fn new(
        api_key: &str,
        budget: Arc<SharedRequestBudget>,
        class: RequestClass,
    ) -> Result<Self, ClientConfigError> {
        let api_key = private_header(api_key)?;
        let client = Client::builder()
            .https_only(true)
            .redirect(redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .map_err(|_| ClientConfigError::ClientInitialization)?;
        let endpoint =
            Url::parse(JUPITER_BUILD_URL).map_err(|_| ClientConfigError::ClientInitialization)?;
        Ok(Self {
            client,
            api_key,
            endpoint,
            active: Arc::new(Semaphore::new(1)),
            budget: Some((budget, class)),
        })
    }

    /// `now_ms` must use the same monotonic clock as the SearchSession. The timeout
    /// encloses headers AND the entire streamed body; it does not restart per chunk.
    /// Dropping this future cancels the request. There are no automatic retries.
    pub async fn fetch(&self, request: QuoteRequest, now_ms: u64) -> Result<ApiQuote, BuildError> {
        if request.identity().chain_genesis != MAINNET_GENESIS {
            return Err(BuildError::UnsupportedChain);
        }
        if now_ms < request.started_ms() || now_ms >= request.deadline_ms() {
            return Err(BuildError::InvalidLocalTime);
        }
        let permit = self
            .active
            .clone()
            .try_acquire_owned()
            .map_err(|_| BuildError::Busy)?;
        let timeout = Duration::from_millis(request.deadline_ms() - now_ms);
        tokio::time::timeout(timeout, self.fetch_reserved(request, timeout, permit))
            .await
            .map_err(|_| BuildError::Timeout)?
    }

    async fn fetch_reserved(
        &self,
        request: QuoteRequest,
        timeout: Duration,
        permit: OwnedSemaphorePermit,
    ) -> Result<ApiQuote, BuildError> {
        let _permit = if let Some((budget, class)) = &self.budget {
            let budget = budget.clone();
            let class = *class;
            // The owned one-operation permit stays with this blocking job even if
            // the HTTP deadline cancels our future. No queue of stranded file jobs.
            let (result, permit) =
                tokio::task::spawn_blocking(move || (budget.try_reserve(class), permit))
                    .await
                    .map_err(|_| BuildError::Transport)?;
            result.map_err(BuildError::Budget)?;
            permit
        } else {
            permit
        }; // Private loopback tests only; public construction requires a budget.
        self.fetch_inner(request, timeout).await
    }

    async fn fetch_inner(
        &self,
        request: QuoteRequest,
        timeout: Duration,
    ) -> Result<ApiQuote, BuildError> {
        let id = request.identity();
        let query = [
            (
                "inputMint",
                Pubkey::new_from_array(id.input_mint).to_string(),
            ),
            (
                "outputMint",
                Pubkey::new_from_array(id.output_mint).to_string(),
            ),
            ("amount", request.input_lamports().0.to_string()),
            ("taker", Pubkey::new_from_array(id.taker).to_string()),
            ("slippageBps", request.policy().slippage_bps.to_string()),
            ("platformFeeBps", "0".to_owned()),
            ("maxAccounts", "64".to_owned()),
            ("wrapAndUnwrapSol", "false".to_owned()),
            ("transactionVersion", "0".to_owned()),
        ];
        let mut response = self
            .client
            .get(self.endpoint.clone())
            .header("x-api-key", self.api_key.clone())
            .query(&query)
            .timeout(timeout)
            .send()
            .await
            .map_err(map_transport_error)?;
        match response.status() {
            StatusCode::TOO_MANY_REQUESTS => return Err(BuildError::RateLimited),
            status if status.is_redirection() => return Err(BuildError::Redirect),
            status if !status.is_success() => return Err(BuildError::HttpStatus(status.as_u16())),
            _ => {}
        }
        let limit = request.policy().max_response_bytes;
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(BuildError::BodyTooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(map_transport_error)? {
            if chunk.len() > limit - bytes.len() {
                return Err(BuildError::BodyTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        decode_build_response(request, &bytes)
    }
}

fn private_header(value: &str) -> Result<HeaderValue, ClientConfigError> {
    if value.is_empty() {
        return Err(ClientConfigError::InvalidApiKey);
    }
    let mut header = HeaderValue::from_str(value).map_err(|_| ClientConfigError::InvalidApiKey)?;
    header.set_sensitive(true);
    Ok(header)
}

fn map_transport_error(error: reqwest::Error) -> BuildError {
    if error.is_timeout() {
        BuildError::Timeout
    } else {
        BuildError::Transport
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildResponse {
    input_mint: String,
    output_mint: String,
    in_amount: String,
    out_amount: String,
    other_amount_threshold: String,
    swap_mode: String,
    slippage_bps: u16,
    swap_instruction: BuildInstruction,
    error: Option<serde::de::IgnoredAny>,
    error_code: Option<serde::de::IgnoredAny>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildInstruction {
    program_id: String,
    accounts: Vec<BuildAccount>,
    data: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildAccount {
    pubkey: String,
    is_writable: bool,
    is_signer: bool,
}

fn address(raw: &str) -> Result<Address, BuildError> {
    Pubkey::from_str(raw)
        .map(|key| key.to_bytes())
        .map_err(|_| BuildError::InvalidAddress)
}

/// Deserialize the SAME body retained as candidate evidence. Missing/duplicate
/// required fields and wrong types fail closed. Unknown optional API fields remain
/// in the raw evidence but are not interpreted or executed by this adapter.
pub fn decode_build_response(request: QuoteRequest, body: &[u8]) -> Result<ApiQuote, BuildError> {
    if request.identity().chain_genesis != MAINNET_GENESIS {
        return Err(BuildError::UnsupportedChain);
    }
    if body.len() > request.policy().max_response_bytes {
        return Err(BuildError::BodyTooLarge);
    }
    let response: BuildResponse =
        serde_json::from_slice(body).map_err(|_| BuildError::InvalidJson)?;
    if response.error.is_some() || response.error_code.is_some() {
        return Err(BuildError::InvalidJson);
    }
    let input_mint = address(&response.input_mint)?;
    let output_mint = address(&response.output_mint)?;
    if response.swap_mode != "ExactIn" {
        return Err(BuildError::UnsupportedRoute);
    }
    let input = SolLamports::parse(&response.in_amount)
        .map_err(|error| BuildError::QuoteRejected(ResponseRejection::InvalidAmount(error)))?;
    let expected = TokenUnits::parse(&response.out_amount)
        .map_err(|error| BuildError::QuoteRejected(ResponseRejection::InvalidAmount(error)))?;
    if response.swap_instruction.accounts.len() < 10
        || response.swap_instruction.accounts.len() > MAX_SWAP_ACCOUNTS
        || response.swap_instruction.data.len() > MAX_SWAP_DATA_BYTES.div_ceil(3) * 4
    {
        return Err(BuildError::InstructionResourceLimit);
    }
    if address(&response.swap_instruction.program_id)? != JUPITER_PROGRAM {
        return Err(BuildError::FixedAccountMismatch);
    }
    let data = STANDARD
        .decode(&response.swap_instruction.data)
        .map_err(|_| BuildError::InvalidBase64)?;
    if data.len() > MAX_SWAP_DATA_BYTES {
        return Err(BuildError::InstructionResourceLimit);
    }
    let header = ParsedHeader::parse(&data).map_err(|_| BuildError::InvalidHeader)?;
    if header.kind() != RouteKind::ExactIn {
        return Err(BuildError::UnsupportedRoute);
    }
    if header.declared_route_steps() > MAX_ROUTE_STEPS {
        return Err(BuildError::InstructionResourceLimit);
    }
    if header.amount() != input.0
        || input != request.input_lamports()
        || header.quoted_amount() != expected.0
        || header.slippage_bps() != response.slippage_bps
        || response.slippage_bps != request.policy().slippage_bps
    {
        return Err(BuildError::ScalarMismatch);
    }
    if header.platform_fee_bps() != 0 || header.positive_slippage_bps() != 0 {
        return Err(BuildError::FeeDeclared);
    }
    let accounts = response
        .swap_instruction
        .accounts
        .iter()
        .map(|account| {
            Ok(AccountMeta {
                key: address(&account.pubkey)?,
                is_signer: account.is_signer,
                is_writable: account.is_writable,
            })
        })
        .collect::<Result<Vec<_>, BuildError>>()?;
    check_fixed_accounts(&accounts, &request)?;
    ApiQuote::parse(
        request,
        ApiQuoteFields {
            input_mint,
            output_mint,
            in_amount: &response.in_amount,
            expected_out_amount: &response.out_amount,
            other_amount_threshold: &response.other_amount_threshold,
            swap_mode: SwapMode::ExactIn,
            slippage_bps: response.slippage_bps,
            // Taken from decoded instruction bytes, never invented from absent JSON fee fields.
            platform_fee_bps: header.platform_fee_bps(),
            positive_slippage_fee_bps: header.positive_slippage_bps(),
            taker: accounts[0].key,
            account_policy_digest: request.identity().account_policy_digest,
        },
        body,
    )
    .map_err(BuildError::QuoteRejected)
}

fn check_fixed_accounts(
    accounts: &[AccountMeta],
    request: &QuoteRequest,
) -> Result<(), BuildError> {
    let id = request.identity();
    let fixed = [
        (0, id.taker),
        (3, id.input_mint),
        (4, id.output_mint),
        (5, id.input_token_program),
        (6, id.output_token_program),
        (8, EVENT_AUTHORITY),
        (9, JUPITER_PROGRAM),
    ];
    if fixed
        .iter()
        .any(|(index, key)| accounts[*index].key != *key)
        || !accounts[0].is_signer
        || !accounts[1].is_writable
        || !accounts[2].is_writable
        || accounts[1].key == accounts[2].key
        || accounts[1].key == [0; 32]
        || accounts[2].key == [0; 32]
        || (accounts[7].key != JUPITER_PROGRAM && accounts[7].key != accounts[2].key)
    {
        return Err(BuildError::FixedAccountMismatch);
    }
    let readonly = [
        id.taker,
        id.input_mint,
        id.output_mint,
        id.input_token_program,
        id.output_token_program,
        EVENT_AUTHORITY,
        JUPITER_PROGRAM,
    ];
    if accounts.iter().any(|account| {
        (account.is_signer && account.key != id.taker)
            || (account.is_writable && readonly.contains(&account.key))
    }) {
        return Err(BuildError::FixedAccountMismatch);
    }
    // Scratch account identity/ownership is intentionally not inferred from this
    // API response. The later executor must bind fresh isolated accounts itself.
    if readonly.contains(&accounts[1].key) || readonly.contains(&accounts[2].key) {
        return Err(BuildError::FixedAccountMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_search::{
        SearchIdentity, SearchOutcome, SearchPolicy, SearchSession, TOKEN_2022_PROGRAM,
        TOKEN_PROGRAM, WSOL_MINT,
    };
    use serde_json::{Value, json};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    fn request(timeout_ms: u64, response_limit: usize) -> QuoteRequest {
        let identity = SearchIdentity {
            chain_genesis: MAINNET_GENESIS,
            program: [2; 32],
            configuration: [3; 32],
            market: [4; 32],
            input_mint: WSOL_MINT,
            input_token_program: TOKEN_PROGRAM,
            input_decimals: 9,
            output_mint: [5; 32],
            output_token_program: TOKEN_2022_PROGRAM,
            output_decimals: 6,
            exact_debt: TokenUnits(100),
            policy_digest: [6; 32],
            policy_version: 1,
            account_policy_digest: [7; 32],
            taker: [8; 32],
            model_epoch: 1,
            observation_epoch: 2,
            report_sequence: 3,
            window_start_ms: 1_000,
            window_end_ms: 1_000 + timeout_ms,
        };
        let policy = SearchPolicy {
            max_input_lamports: SolLamports(100),
            max_requests: 1,
            max_response_bytes: response_limit,
            request_timeout_ms: timeout_ms,
            max_window_ms: timeout_ms,
            slippage_bps: 50,
        };
        SearchSession::new(identity, policy, &[SolLamports(100)])
            .unwrap()
            .next_request(1_000)
            .unwrap()
            .unwrap()
    }

    fn public_key(key: Address) -> String {
        Pubkey::new_from_array(key).to_string()
    }

    fn instruction_data(expected_output: u64) -> Vec<u8> {
        let mut data = shortwin_jupiter_route::ROUTE_V2.to_vec();
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(&expected_output.to_le_bytes());
        data.extend_from_slice(&50u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        // Minimal opaque plan; accepting a scalar header is not route authorization.
        data.extend_from_slice(&[0, 0x10, 0x27, 0, 1]);
        data
    }

    fn build_response(request: &QuoteRequest, expected_output: u64, minimum_output: u64) -> Value {
        let id = request.identity();
        let keys = [
            id.taker,
            [10; 32],
            [11; 32],
            id.input_mint,
            id.output_mint,
            id.input_token_program,
            id.output_token_program,
            JUPITER_PROGRAM,
            EVENT_AUTHORITY,
            JUPITER_PROGRAM,
        ];
        let accounts: Vec<_> = keys
            .iter()
            .enumerate()
            .map(|(index, key)| {
                json!({
                    "pubkey":public_key(*key),"isWritable":index==1||index==2,"isSigner":index==0
                })
            })
            .collect();
        json!({ "inputMint":public_key(id.input_mint),"outputMint":public_key(id.output_mint),
            "inAmount":"100","outAmount":expected_output.to_string(),
            "otherAmountThreshold":minimum_output.to_string(),"swapMode":"ExactIn","slippageBps":50,
            "swapInstruction":{"programId":public_key(JUPITER_PROGRAM),"accounts":accounts,
                "data":STANDARD.encode(instruction_data(expected_output))},
            "setupInstructions":[],"cleanupInstruction":null,
            "blockhashWithMetadata":{"fetchedAt":{"secs_since_epoch":1234,"nanos_since_epoch":0}}
        })
    }

    fn decode(request: QuoteRequest, value: &Value) -> Result<ApiQuote, BuildError> {
        decode_build_response(request, &serde_json::to_vec(value).unwrap())
    }

    fn replace_data(value: &mut Value, change: impl FnOnce(&mut Vec<u8>)) {
        let mut data = STANDARD
            .decode(value["swapInstruction"]["data"].as_str().unwrap())
            .unwrap();
        change(&mut data);
        value["swapInstruction"]["data"] = json!(STANDARD.encode(data));
    }

    #[test]
    fn typed_decode_retains_same_bytes_and_does_not_turn_blockhash_time_into_price_context() {
        let request = request(1_000, 16_384);
        let body = serde_json::to_vec(&build_response(&request, 200, 199)).unwrap();
        let quote = decode_build_response(request, &body).unwrap();
        let mut session =
            SearchSession::new(*request.identity(), *request.policy(), &[SolLamports(100)])
                .unwrap();
        assert_eq!(session.next_request(1_000).unwrap(), Some(request));
        session.record_quote(quote, 1_001).unwrap();
        let SearchOutcome::ApiCandidate { candidate, .. } = session.finish(1_001).unwrap() else {
            panic!("candidate")
        };
        assert_eq!(candidate.raw_response(), body);
        assert_eq!(candidate.minimum_output(), TokenUnits(199));
        assert!(!candidate.executable_minimum_verified());
        assert_eq!(candidate.expected_output(), TokenUnits(200));
        assert_eq!(candidate.request_started_ms(), 1_000);
        assert_eq!(candidate.received_ms(), 1_001);
    }

    #[test]
    fn expected_and_minimum_are_independent_and_only_minimum_covers_debt() {
        let request = request(1_000, 16_384);
        let quote = decode(request, &build_response(&request, 100, 99)).unwrap();
        let mut session =
            SearchSession::new(*request.identity(), *request.policy(), &[SolLamports(100)])
                .unwrap();
        session.next_request(1_000).unwrap();
        session.record_quote(quote, 1_001).unwrap();
        assert!(matches!(
            session.finish(1_001).unwrap(),
            SearchOutcome::UncoveredWithinSearchBudget(_)
        ));
    }

    #[test]
    fn optimistic_json_minimum_never_proves_executable_v2_minimum() {
        let request = request(1_000, 16_384);
        // Header has nonzero slippage; JSON declares the full expected amount as
        // minimum. No unverified V2 rounding formula is inferred from these fields.
        let quote = decode(request, &build_response(&request, 100, 100)).unwrap();
        let mut session =
            SearchSession::new(*request.identity(), *request.policy(), &[SolLamports(100)])
                .unwrap();
        session.next_request(1_000).unwrap();
        session.record_quote(quote, 1_001).unwrap();
        let SearchOutcome::ApiCandidate { candidate, .. } = session.finish(1_001).unwrap() else {
            panic!("API candidate");
        };
        assert_eq!(candidate.minimum_output(), TokenUnits(100));
        assert!(!candidate.executable_minimum_verified());
    }

    #[test]
    fn json_types_canonical_amounts_missing_fields_and_duplicate_fields_fail_closed() {
        let request = request(1_000, 16_384);
        for (field, value) in [
            ("inAmount", json!(100)),
            ("inAmount", json!("0100")),
            ("outAmount", json!("18446744073709551616")),
            ("otherAmountThreshold", json!("1e2")),
            ("slippageBps", json!(50.0)),
            ("slippageBps", json!("50")),
        ] {
            let mut response = build_response(&request, 200, 199);
            response[field] = value;
            assert!(decode(request, &response).is_err(), "field {field}");
        }
        for field in ["error", "errorCode"] {
            let mut response = build_response(&request, 200, 199);
            response[field] = json!("provider error");
            assert_eq!(
                decode(request, &response).unwrap_err(),
                BuildError::InvalidJson
            );
        }
        let mut missing = build_response(&request, 200, 199);
        missing.as_object_mut().unwrap().remove("swapInstruction");
        assert_eq!(
            decode(request, &missing).unwrap_err(),
            BuildError::InvalidJson
        );
        let mut body = serde_json::to_vec(&build_response(&request, 200, 199)).unwrap();
        body.pop();
        body.extend_from_slice(b",\"inAmount\":\"100\"}");
        assert_eq!(
            decode_build_response(request, &body).unwrap_err(),
            BuildError::InvalidJson
        );
        for body in [
            b"[]".as_slice(),
            b"null",
            b"{\"error\":\"No routes found\"}",
            b"broken",
        ] {
            assert_eq!(
                decode_build_response(request, body).unwrap_err(),
                BuildError::InvalidJson
            );
        }
    }

    #[test]
    fn program_mints_accounts_and_instruction_scalars_must_match_request() {
        let request = request(1_000, 16_384);
        for (path, value) in [
            ("inputMint", json!(public_key([99; 32]))),
            ("outputMint", json!(public_key([99; 32]))),
            ("inputMint", json!("invalid")),
            ("inAmount", json!("101")),
            ("outAmount", json!("201")),
            ("slippageBps", json!(51)),
            ("swapMode", json!("ExactOut")),
        ] {
            let mut response = build_response(&request, 200, 199);
            response[path] = value;
            assert!(decode(request, &response).is_err(), "field {path}");
        }
        let mut wrong_program = build_response(&request, 200, 199);
        wrong_program["swapInstruction"]["programId"] = json!(public_key([99; 32]));
        assert_eq!(
            decode(request, &wrong_program).unwrap_err(),
            BuildError::FixedAccountMismatch
        );
        for index in [0, 3, 4, 5, 6, 7, 8, 9] {
            let mut response = build_response(&request, 200, 199);
            response["swapInstruction"]["accounts"][index]["pubkey"] = json!(public_key([99; 32]));
            assert_eq!(
                decode(request, &response).unwrap_err(),
                BuildError::FixedAccountMismatch,
                "index {index}"
            );
        }
        for index in [0, 3, 4, 5, 6, 8, 9] {
            let mut response = build_response(&request, 200, 199);
            response["swapInstruction"]["accounts"][index]["isWritable"] = json!(true);
            assert_eq!(
                decode(request, &response).unwrap_err(),
                BuildError::FixedAccountMismatch
            );
        }
        let mut wrong_signer = build_response(&request, 200, 199);
        wrong_signer["swapInstruction"]["accounts"][1]["isSigner"] = json!(true);
        assert_eq!(
            decode(request, &wrong_signer).unwrap_err(),
            BuildError::FixedAccountMismatch
        );
        for scalar_offset in [8, 16, 24] {
            let mut response = build_response(&request, 200, 199);
            replace_data(&mut response, |data| data[scalar_offset] += 1);
            assert_eq!(
                decode(request, &response).unwrap_err(),
                BuildError::ScalarMismatch
            );
        }
    }

    #[test]
    fn fee_evidence_comes_from_header_not_missing_json_fields() {
        let request = request(1_000, 16_384);
        let response = build_response(&request, 200, 199);
        assert!(response.get("platformFeeBps").is_none());
        assert!(decode(request, &response).is_ok());
        for offset in [26, 28] {
            let mut response = response.clone();
            replace_data(&mut response, |data| data[offset] = 1);
            assert_eq!(
                decode(request, &response).unwrap_err(),
                BuildError::FeeDeclared
            );
        }
    }

    #[test]
    fn malformed_shared_exactout_and_resource_excess_are_rejected() {
        let request = request(1_000, 16_384);
        let mut bad = build_response(&request, 200, 199);
        bad["swapInstruction"]["data"] = json!("not base64!");
        assert_eq!(
            decode(request, &bad).unwrap_err(),
            BuildError::InvalidBase64
        );
        for discriminator in [
            shortwin_jupiter_route::EXACT_OUT_ROUTE_V2,
            shortwin_jupiter_route::SHARED_ACCOUNTS_ROUTE_V2,
            [0; 8],
        ] {
            let mut response = build_response(&request, 200, 199);
            replace_data(&mut response, |data| {
                data[..8].copy_from_slice(&discriminator);
                if discriminator == shortwin_jupiter_route::SHARED_ACCOUNTS_ROUTE_V2 {
                    data.insert(8, 0);
                }
            });
            assert!(matches!(
                decode(request, &response),
                Err(BuildError::UnsupportedRoute | BuildError::InvalidHeader)
            ));
        }
        let mut oversized = build_response(&request, 200, 199);
        let account = oversized["swapInstruction"]["accounts"][1].clone();
        oversized["swapInstruction"]["accounts"] = json!(vec![account; MAX_SWAP_ACCOUNTS + 1]);
        assert!(matches!(
            decode(request, &oversized),
            Err(BuildError::InstructionResourceLimit | BuildError::BodyTooLarge)
        ));
        let mut truncated = build_response(&request, 200, 199);
        replace_data(&mut truncated, |data| data.truncate(10));
        assert_eq!(
            decode(request, &truncated).unwrap_err(),
            BuildError::InvalidHeader
        );
        let mut steps = build_response(&request, 200, 199);
        replace_data(&mut steps, |data| {
            data[30..34].copy_from_slice(&(MAX_ROUTE_STEPS + 1).to_le_bytes());
            data.resize(34 + (MAX_ROUTE_STEPS as usize + 1) * 5, 0);
        });
        assert_eq!(
            decode(request, &steps).unwrap_err(),
            BuildError::InstructionResourceLimit
        );
    }

    const TEST_API_KEY: &str = "dummy-test-credential-never-live";

    fn test_client(endpoint: Url) -> JupiterBuildClient {
        assert_eq!(endpoint.host_str(), Some("127.0.0.1"));
        JupiterBuildClient {
            client: Client::builder()
                .redirect(redirect::Policy::none())
                .retry(reqwest::retry::never())
                .no_proxy()
                .build()
                .unwrap(),
            api_key: private_header(TEST_API_KEY).unwrap(),
            endpoint,
            active: Arc::new(Semaphore::new(1)),
            budget: None,
        }
    }

    /// Private loopback-only test server, no external network or real credentials.
    async fn serve(parts: Vec<(u64, Vec<u8>)>) -> (Url, JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = Url::parse(&format!(
            "http://{}/swap/v2/build",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1_024];
            while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).await.unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                assert!(request.len() <= 8_192);
            }
            for (delay, part) in parts {
                tokio::time::sleep(Duration::from_millis(delay)).await;
                if stream.write_all(&part).await.is_err() {
                    break;
                }
            }
            request
        });
        (endpoint, task)
    }

    fn response(status: &str, body: &[u8]) -> Vec<u8> {
        let mut bytes=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).into_bytes();
        bytes.extend_from_slice(body);
        bytes
    }

    #[test]
    fn credential_header_is_sensitive_and_errors_never_capture_secret_text() {
        let header = private_header(TEST_API_KEY).unwrap();
        assert!(header.is_sensitive());
        assert!(!format!("{header:?}").contains(TEST_API_KEY));
        assert!(matches!(
            private_header(""),
            Err(ClientConfigError::InvalidApiKey)
        ));
        assert!(matches!(
            private_header("bad\nheader"),
            Err(ClientConfigError::InvalidApiKey)
        ));
        assert!(!format!("{:?}", BuildError::InvalidJson).contains(TEST_API_KEY));
        assert_eq!(JUPITER_BUILD_URL, "https://api.jup.ag/swap/v2/build");
        assert_eq!(
            public_key(MAINNET_GENESIS),
            "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
        );
    }

    #[tokio::test]
    async fn http_success_uses_policy_query_header_and_exact_body() {
        let request = request(2_000, 16_384);
        let body = serde_json::to_vec(&build_response(&request, 200, 199)).unwrap();
        let (endpoint, task) = serve(vec![(0, response("200 OK", &body))]).await;
        let client = test_client(endpoint);
        let quote = client.fetch(request, 1_000).await.unwrap();
        let raw_request = String::from_utf8(task.await.unwrap()).unwrap();
        let first_line = raw_request.lines().next().unwrap();
        assert!(first_line.starts_with("GET /swap/v2/build?"));
        for query in [
            "amount=100",
            "slippageBps=50",
            "platformFeeBps=0",
            "maxAccounts=64",
            "wrapAndUnwrapSol=false",
            "transactionVersion=0",
        ] {
            assert!(first_line.contains(query));
        }
        assert!(first_line.contains(&format!("taker={}", public_key(request.identity().taker))));
        assert!(!first_line.contains(TEST_API_KEY));
        assert!(!first_line.contains("apiKey"));
        assert!(
            raw_request
                .to_ascii_lowercase()
                .contains(&format!("x-api-key: {TEST_API_KEY}"))
        );
        let mut session =
            SearchSession::new(*request.identity(), *request.policy(), &[SolLamports(100)])
                .unwrap();
        session.next_request(1_000).unwrap();
        session.record_quote(quote, 1_001).unwrap();
        let SearchOutcome::ApiCandidate { candidate, .. } = session.finish(1_001).unwrap() else {
            panic!("candidate")
        };
        assert_eq!(candidate.raw_response(), body);
    }

    #[tokio::test]
    async fn http_redirect_is_not_followed_or_forwarded_with_credential() {
        let other = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let location = format!("http://{}/credential-trap", other.local_addr().unwrap());
        let head = format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        let (endpoint, task) = serve(vec![(0, head.into_bytes())]).await;
        assert_eq!(
            test_client(endpoint)
                .fetch(request(2_000, 16_384), 1_000)
                .await
                .unwrap_err(),
            BuildError::Redirect
        );
        task.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(30), other.accept())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn http_429_is_rate_limited_and_400_no_route_is_unknown() {
        for (status, body, expected) in [
            (
                "429 Too Many Requests",
                b"rate limited".as_slice(),
                BuildError::RateLimited,
            ),
            (
                "400 Bad Request",
                b"{\"error\":\"No routes found\"}".as_slice(),
                BuildError::HttpStatus(400),
            ),
        ] {
            let (endpoint, task) = serve(vec![(0, response(status, body))]).await;
            let error = test_client(endpoint)
                .fetch(request(2_000, 16_384), 1_000)
                .await
                .unwrap_err();
            assert_eq!(error, expected);
            task.await.unwrap();
            let observation = error.observation_failure();
            let request = request(2_000, 16_384);
            let mut session =
                SearchSession::new(*request.identity(), *request.policy(), &[SolLamports(100)])
                    .unwrap();
            session.next_request(1_000).unwrap();
            session.record_failure(request, observation, 1_001).unwrap();
            assert!(matches!(
                session.finish(1_001).unwrap(),
                SearchOutcome::Unknown(_)
            ));
        }
    }

    #[tokio::test]
    async fn streaming_chunked_body_limit_applies_without_content_length() {
        let chunks=b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n10\r\n0123456789abcdef\r\n11\r\n0123456789abcdefg\r\n0\r\n\r\n";
        let (endpoint, task) = serve(vec![(0, chunks.to_vec())]).await;
        assert_eq!(
            test_client(endpoint)
                .fetch(request(2_000, 32), 1_000)
                .await
                .unwrap_err(),
            BuildError::BodyTooLarge
        );
        task.await.unwrap();
    }

    #[tokio::test]
    async fn declared_oversize_is_rejected_before_decoding_body() {
        let head = b"HTTP/1.1 200 OK\r\nContent-Length: 999999\r\nConnection: close\r\n\r\n";
        let (endpoint, task) = serve(vec![(0, head.to_vec())]).await;
        assert_eq!(
            test_client(endpoint)
                .fetch(request(2_000, 32), 1_000)
                .await
                .unwrap_err(),
            BuildError::BodyTooLarge
        );
        task.await.unwrap();
    }

    #[tokio::test]
    async fn whole_body_timeout_does_not_restart_after_headers_or_each_chunk() {
        let head =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n1\r\n{\r\n";
        let (endpoint, task) = serve(vec![
            (0, head.to_vec()),
            (100, b"1\r\n}\r\n0\r\n\r\n".to_vec()),
        ])
        .await;
        assert_eq!(
            test_client(endpoint)
                .fetch(request(20, 16_384), 1_000)
                .await
                .unwrap_err(),
            BuildError::Timeout
        );
        task.await.unwrap();
    }

    #[tokio::test]
    async fn timeout_also_covers_wait_for_response_headers() {
        let (endpoint, task) = serve(vec![(100, response("200 OK", b"{}"))]).await;
        assert_eq!(
            test_client(endpoint)
                .fetch(request(20, 16_384), 1_000)
                .await
                .unwrap_err(),
            BuildError::Timeout
        );
        task.await.unwrap();
    }

    #[tokio::test]
    async fn expired_request_or_busy_client_never_opens_network_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = Url::parse(&format!(
            "http://{}/swap/v2/build",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let client = test_client(endpoint);
        let request = request(2_000, 16_384);
        assert_eq!(
            client.fetch(request, 999).await.unwrap_err(),
            BuildError::InvalidLocalTime
        );
        assert_eq!(
            client.fetch(request, 3_000).await.unwrap_err(),
            BuildError::InvalidLocalTime
        );
        let mut wrong_chain = *request.identity();
        wrong_chain.chain_genesis = [1; 32];
        let bad_request = SearchSession::new(wrong_chain, *request.policy(), &[SolLamports(100)])
            .unwrap()
            .next_request(1_000)
            .unwrap()
            .unwrap();
        assert_eq!(
            client.fetch(bad_request, 1_000).await.unwrap_err(),
            BuildError::UnsupportedChain
        );
        assert_eq!(
            decode_build_response(bad_request, b"{} ").unwrap_err(),
            BuildError::UnsupportedChain
        );
        let _held = client.active.try_acquire().unwrap();
        assert_eq!(
            client.fetch(request, 1_000).await.unwrap_err(),
            BuildError::Busy
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(30), listener.accept())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn remaining_request_deadline_limits_transport_not_original_full_timeout() {
        let (endpoint, task) = serve(vec![(100, response("200 OK", b"{}"))]).await;
        // Original timeout1000ms, but the SearchSession deadline has only10ms left.
        assert_eq!(
            test_client(endpoint)
                .fetch(request(1_000, 16_384), 1_990)
                .await
                .unwrap_err(),
            BuildError::Timeout
        );
        task.await.unwrap();
    }

    #[cfg(target_os = "linux")]
    struct BudgetDirectory(std::path::PathBuf);

    #[cfg(target_os = "linux")]
    impl BudgetDirectory {
        fn new() -> Self {
            use std::os::unix::fs::DirBuilderExt;
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "shortwin-http-budget-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .unwrap();
            Self(path)
        }
        fn budget(&self) -> Arc<SharedRequestBudget> {
            Arc::new(
                SharedRequestBudget::new(crate::price_budget::BudgetConfig {
                    state_file: self.0.join("requests.state"),
                    limits: crate::price_budget::BudgetLimits {
                        total_per_second: 2,
                        background_per_second: 1,
                        reserved_exit_per_second: 1,
                    },
                })
                .unwrap(),
            )
        }
    }
    #[cfg(target_os = "linux")]
    impl Drop for BudgetDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn budget_reserves_before_http_and_429_is_not_refunded_or_an_illiquidity_mark() {
        let directory = BudgetDirectory::new();
        let budget = directory.budget();
        let (endpoint, task) =
            serve(vec![(0, response("429 Too Many Requests", b"limited"))]).await;
        let mut client = test_client(endpoint);
        client.budget = Some((budget.clone(), RequestClass::Background));
        let request = request(2_000, 16_384);
        let first = client.fetch(request, 1_000).await.unwrap_err();
        assert_eq!(first, BuildError::RateLimited);
        task.await.unwrap();
        let second = client.fetch(request, 1_000).await.unwrap_err();
        assert!(matches!(
            second,
            BuildError::Budget(BudgetError::RateLimited { .. })
        ));
        // The failed HTTP call remains charged. Exit capacity remains available.
        assert_eq!(
            budget.try_reserve(RequestClass::Exit).unwrap().used_total,
            2
        );
        for error in [first, second] {
            let mut session =
                SearchSession::new(*request.identity(), *request.policy(), &[SolLamports(100)])
                    .unwrap();
            session.next_request(1_000).unwrap();
            session
                .record_failure(request, error.observation_failure(), 1_001)
                .unwrap();
            assert!(matches!(
                session.finish(1_001).unwrap(),
                SearchOutcome::Unknown(_)
            ));
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn corrupt_budget_fails_closed_before_any_http_connection() {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let directory = BudgetDirectory::new();
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(directory.0.join("requests.state"))
            .unwrap();
        file.write_all(b"corrupt-do-not-reset").unwrap();
        drop(file);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = Url::parse(&format!(
            "http://{}/swap/v2/build",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let mut client = test_client(endpoint);
        client.budget = Some((directory.budget(), RequestClass::Background));
        let error = client
            .fetch(request(2_000, 16_384), 1_000)
            .await
            .unwrap_err();
        assert_eq!(error, BuildError::Budget(BudgetError::Corrupt));
        assert_eq!(error.observation_failure(), ObservationFailure::Transport);
        assert!(
            tokio::time::timeout(Duration::from_millis(30), listener.accept())
                .await
                .is_err()
        );
        assert_eq!(
            std::fs::read(directory.0.join("requests.state")).unwrap(),
            b"corrupt-do-not-reset"
        );
    }

    #[tokio::test]
    async fn failed_transport_does_not_retry_another_http_attempt() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = Url::parse(&format!(
            "http://{}/swap/v2/build",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = [0u8; 8192];
            let count = stream.read(&mut bytes).await.unwrap();
            assert!(count > 0);
            drop(stream); // Connection ends before any HTTP response.
            tokio::time::timeout(Duration::from_millis(50), listener.accept())
                .await
                .is_err()
        });
        assert_eq!(
            test_client(endpoint)
                .fetch(request(2_000, 16_384), 1_000)
                .await
                .unwrap_err(),
            BuildError::Transport
        );
        assert!(task.await.unwrap());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cancelled_queued_budget_job_retains_one_operation_permit_and_never_sends_http() {
        let directory = BudgetDirectory::new();
        let budget = directory.budget();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let (started_tx, started_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            });
            started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = Url::parse(&format!(
                "http://{}/swap/v2/build",
                listener.local_addr().unwrap()
            ))
            .unwrap();
            let mut client = test_client(endpoint);
            client.budget = Some((budget.clone(), RequestClass::Background));
            assert_eq!(
                client.fetch(request(20, 16_384), 1_000).await.unwrap_err(),
                BuildError::Timeout
            );
            assert_eq!(
                client
                    .fetch(request(2_000, 16_384), 1_000)
                    .await
                    .unwrap_err(),
                BuildError::Busy
            );
            release_tx.send(()).unwrap();
            blocker.await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                while client.active.available_permits() == 0 {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            })
            .await
            .unwrap();
            // A reservation which finishes after cancellation is conservatively
            // charged; the cancelled future cannot proceed into HTTP.
            assert_eq!(
                budget.try_reserve(RequestClass::Exit).unwrap().used_total,
                2
            );
            assert!(
                tokio::time::timeout(Duration::from_millis(30), listener.accept())
                    .await
                    .is_err()
            );
        });
    }
}
