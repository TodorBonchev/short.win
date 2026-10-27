//! Bounded, fixed-debt-quantity Jupiter quote search, before transport integration.
//!
//! This module only selects an **API-declared** candidate. It neither simulates a
//! transaction nor establishes net vault deltas, source-state freshness, a fair-price
//! lower reference, authenticated no-route evidence, or a liquidation decision.
//! An adapter must faithfully derive the typed fields from the retained response
//! bytes and original request, enforce an HTTP timeout and a streaming body limit,
//! and validate instruction/account/fee semantics before upgrading this evidence.
//! A locally fresh fetch does not prove that the provider's market data is fresh.
//!
//! `SearchSession` accepts an injected source's observations one at a time. The
//! adapter can await its HTTP future outside this state machine. At most one request
//! and one retained response body exist here; there is no unbounded request queue.
//! There is deliberately no conversion to a financial risk/reference type.

use std::{collections::BTreeSet, fmt};

pub type Address = [u8; 32];

/// Implementation resource ceilings, not approved production/risk settings.
pub const MAX_CANDIDATES: usize = 64;
pub const MAX_RESPONSE_BYTES: usize = 1_048_576;

pub const WSOL_MINT: Address = [
    6, 155, 136, 87, 254, 171, 129, 132, 251, 104, 127, 99, 70, 24, 192, 53, 218, 196, 57, 220, 26,
    235, 59, 85, 152, 160, 240, 0, 0, 0, 0, 1,
];
pub const TOKEN_PROGRAM: Address = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
];
pub const TOKEN_2022_PROGRAM: Address = [
    6, 221, 246, 225, 238, 117, 143, 222, 24, 66, 93, 188, 228, 108, 205, 218, 182, 26, 252, 77,
    131, 185, 13, 39, 254, 189, 249, 40, 216, 161, 139, 252,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmountError {
    NotCanonicalUnsignedDecimal,
    Overflow,
}

fn parse_raw_u64(raw: &str) -> Result<u64, AmountError> {
    // Bound work even for a hostile, already-buffered wire value.
    if raw.len() > 20 {
        return Err(AmountError::Overflow);
    }
    if raw.is_empty()
        || (raw.len() > 1 && raw.starts_with('0'))
        || !raw.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AmountError::NotCanonicalUnsignedDecimal);
    }
    raw.bytes().try_fold(0_u64, |value, digit| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(digit - b'0')))
            .ok_or(AmountError::Overflow)
    })
}

/// Raw base-mint units. Decimal display/float conversion never enters the search.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TokenUnits(pub u64);

impl TokenUnits {
    pub fn parse(raw: &str) -> Result<Self, AmountError> {
        parse_raw_u64(raw).map(Self)
    }
}

/// WSOL input lamports, distinct from raw base-token output units.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SolLamports(pub u64);

impl SolLamports {
    pub fn parse(raw: &str) -> Result<Self, AmountError> {
        parse_raw_u64(raw).map(Self)
    }
}

/// Immutable identity for one search. The digests identify a reviewed immutable
/// fee/account/taker/search policy mapping; this module does not authenticate it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchIdentity {
    pub chain_genesis: Address,
    pub program: Address,
    pub configuration: Address,
    pub market: Address,
    pub input_mint: Address,
    pub input_token_program: Address,
    pub input_decimals: u8,
    pub output_mint: Address,
    pub output_token_program: Address,
    pub output_decimals: u8,
    pub exact_debt: TokenUnits,
    pub policy_digest: Address,
    pub policy_version: u32,
    pub account_policy_digest: Address,
    pub taker: Address,
    pub model_epoch: u64,
    pub observation_epoch: u64,
    pub report_sequence: u64,
    /// Half-open local request window, in the adapter's monotonic millisecond clock.
    pub window_start_ms: u64,
    pub window_end_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchPolicy {
    pub max_input_lamports: SolLamports,
    pub max_requests: usize,
    pub max_response_bytes: usize,
    pub request_timeout_ms: u64,
    pub max_window_ms: u64,
    pub slippage_bps: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanError {
    MissingIdentity,
    UnsupportedInputUnits,
    UnsupportedOutputTokenProgram,
    SameMint,
    ZeroDebt,
    InvalidPolicy,
    InvalidWindow,
    EmptyCandidateSet,
    TooManyCandidates,
    ZeroInput,
    InputAboveCap,
    DuplicateInput,
}

fn validate_plan(
    identity: &SearchIdentity,
    policy: &SearchPolicy,
    inputs: &[SolLamports],
) -> Result<(), PlanError> {
    let required = [
        identity.chain_genesis,
        identity.program,
        identity.configuration,
        identity.market,
        identity.output_mint,
        identity.policy_digest,
        identity.account_policy_digest,
        identity.taker,
    ];
    if required.contains(&[0; 32]) || identity.policy_version == 0 {
        return Err(PlanError::MissingIdentity);
    }
    if identity.input_mint != WSOL_MINT
        || identity.input_token_program != TOKEN_PROGRAM
        || identity.input_decimals != 9
    {
        return Err(PlanError::UnsupportedInputUnits);
    }
    if ![TOKEN_PROGRAM, TOKEN_2022_PROGRAM].contains(&identity.output_token_program) {
        return Err(PlanError::UnsupportedOutputTokenProgram);
    }
    if identity.output_mint == identity.input_mint {
        return Err(PlanError::SameMint);
    }
    if identity.exact_debt.0 == 0 {
        return Err(PlanError::ZeroDebt);
    }
    if policy.max_input_lamports.0 == 0
        || policy.max_requests == 0
        || policy.max_requests > MAX_CANDIDATES
        || policy.max_response_bytes == 0
        || policy.max_response_bytes > MAX_RESPONSE_BYTES
        || policy.request_timeout_ms == 0
        || policy.max_window_ms == 0
        || policy.slippage_bps >= 10_000
    {
        return Err(PlanError::InvalidPolicy);
    }
    if identity.window_end_ms <= identity.window_start_ms
        || identity.window_end_ms - identity.window_start_ms > policy.max_window_ms
    {
        return Err(PlanError::InvalidWindow);
    }
    if inputs.is_empty() {
        return Err(PlanError::EmptyCandidateSet);
    }
    if inputs.len() > MAX_CANDIDATES {
        return Err(PlanError::TooManyCandidates);
    }
    let mut unique = BTreeSet::new();
    for input in inputs {
        if input.0 == 0 {
            return Err(PlanError::ZeroInput);
        }
        if *input > policy.max_input_lamports {
            return Err(PlanError::InputAboveCap);
        }
        if !unique.insert(*input) {
            return Err(PlanError::DuplicateInput);
        }
    }
    Ok(())
}

/// Only a session can issue a request. Its complete value correlates a response
/// with this exact quantity, identity, policy, attempt and local fetch window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuoteRequest {
    identity: SearchIdentity,
    policy: SearchPolicy,
    input_lamports: SolLamports,
    ordinal: usize,
    started_ms: u64,
    deadline_ms: u64,
}

impl QuoteRequest {
    pub fn identity(&self) -> &SearchIdentity {
        &self.identity
    }
    pub fn policy(&self) -> &SearchPolicy {
        &self.policy
    }
    pub fn input_lamports(&self) -> SolLamports {
        self.input_lamports
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn started_ms(&self) -> u64 {
        self.started_ms
    }
    pub fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwapMode {
    ExactIn,
    ExactOut,
}

/// Adapter-normalized fields from the same raw response/original request.
/// `taker` and `account_policy_digest` correlate request context; they are not
/// claims that Jupiter authenticated those fields or validated execution accounts.
#[derive(Clone, Copy, Debug)]
pub struct ApiQuoteFields<'a> {
    pub input_mint: Address,
    pub output_mint: Address,
    pub in_amount: &'a str,
    pub expected_out_amount: &'a str,
    pub other_amount_threshold: &'a str,
    pub swap_mode: SwapMode,
    pub slippage_bps: u16,
    pub platform_fee_bps: u16,
    pub positive_slippage_fee_bps: u16,
    pub taker: Address,
    pub account_policy_digest: Address,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseRejection {
    EmptyBody,
    BodyTooLarge,
    InvalidAmount(AmountError),
    IdentityMismatch,
    InputAmountMismatch,
    SwapModeMismatch,
    SlippageMismatch,
    AdditionalFeeDeclared,
    MinimumAboveExpected,
}

#[derive(Clone, Eq, PartialEq)]
struct ResponseBytes(Vec<u8>);

impl fmt::Debug for ResponseBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Avoid dumping provider responses into routine logs.
        f.debug_struct("ResponseBytes")
            .field("len", &self.0.len())
            .finish()
    }
}

/// Parsed API declaration, never a simulated execution. The adapter must enforce
/// the body limit while reading; this constructor also refuses oversized buffers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiQuote {
    request: QuoteRequest,
    expected_output: TokenUnits,
    minimum_output: TokenUnits,
    response: ResponseBytes,
}

impl ApiQuote {
    pub fn parse(
        request: QuoteRequest,
        fields: ApiQuoteFields<'_>,
        raw_response: &[u8],
    ) -> Result<Self, ResponseRejection> {
        if raw_response.is_empty() {
            return Err(ResponseRejection::EmptyBody);
        }
        if raw_response.len() > request.policy.max_response_bytes {
            return Err(ResponseRejection::BodyTooLarge);
        }
        if fields.input_mint != request.identity.input_mint
            || fields.output_mint != request.identity.output_mint
            || fields.taker != request.identity.taker
            || fields.account_policy_digest != request.identity.account_policy_digest
        {
            return Err(ResponseRejection::IdentityMismatch);
        }
        let input =
            SolLamports::parse(fields.in_amount).map_err(ResponseRejection::InvalidAmount)?;
        let expected_output = TokenUnits::parse(fields.expected_out_amount)
            .map_err(ResponseRejection::InvalidAmount)?;
        let minimum_output = TokenUnits::parse(fields.other_amount_threshold)
            .map_err(ResponseRejection::InvalidAmount)?;
        if input != request.input_lamports {
            return Err(ResponseRejection::InputAmountMismatch);
        }
        if fields.swap_mode != SwapMode::ExactIn {
            return Err(ResponseRejection::SwapModeMismatch);
        }
        if fields.slippage_bps != request.policy.slippage_bps {
            return Err(ResponseRejection::SlippageMismatch);
        }
        if fields.platform_fee_bps != 0 || fields.positive_slippage_fee_bps != 0 {
            return Err(ResponseRejection::AdditionalFeeDeclared);
        }
        if minimum_output > expected_output {
            return Err(ResponseRejection::MinimumAboveExpected);
        }
        Ok(Self {
            request,
            expected_output,
            minimum_output,
            response: ResponseBytes(raw_response.to_vec()),
        })
    }
}

/// Least observed qualifying input in this bounded search, API-declared only.
/// Raw bytes are retained for a later response commitment and independent replay.
/// Fees, output extensions, accounts, deltas and upstream data age still require
/// validation. There is no lower mark or attested-no-route conversion here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiCandidateAskUpper {
    quote: ApiQuote,
    received_ms: u64,
}

impl ApiCandidateAskUpper {
    pub fn identity(&self) -> &SearchIdentity {
        self.quote.request.identity()
    }
    pub fn policy(&self) -> &SearchPolicy {
        self.quote.request.policy()
    }
    pub fn input_lamports(&self) -> SolLamports {
        self.quote.request.input_lamports
    }
    pub fn expected_output(&self) -> TokenUnits {
        self.quote.expected_output
    }
    /// Provider-declared minimum, not a verified executable V2 minimum. The
    /// pinned interface does not establish the header's exact rounding semantics.
    pub fn minimum_output(&self) -> TokenUnits {
        self.quote.minimum_output
    }
    /// API declarations never prove the executable minimum or full-q net credit.
    /// The later executor must independently enforce the actual full-q postcondition.
    pub fn executable_minimum_verified(&self) -> bool {
        false
    }
    pub fn request_started_ms(&self) -> u64 {
        self.quote.request.started_ms
    }
    pub fn received_ms(&self) -> u64 {
        self.received_ms
    }
    pub fn raw_response(&self) -> &[u8] {
        &self.quote.response.0
    }
    /// Local window check only. It says nothing about upstream market-state age.
    pub fn within_local_window(&self, now_ms: u64) -> bool {
        now_ms >= self.received_ms && now_ms < self.identity().window_end_ms
    }
}

/// No free-form provider text: a "no routes" string is just an unknown request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationFailure {
    Transport,
    Timeout,
    RateLimited,
    ApiNoRoutes,
    Rejected(ResponseRejection),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    AllInputsAttempted,
    RequestBudget,
    WindowExpired,
    ClockRegression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchSummary {
    pub planned_inputs: usize,
    pub issued_requests: usize,
    pub completed_requests: usize,
    pub qualifying_quotes: usize,
    pub insufficient_quotes: usize,
    /// Bounded by issued_requests <= MAX_CANDIDATES; no error strings/raw bodies.
    pub unknown_observations: Vec<ObservationFailure>,
    pub stop_reason: StopReason,
}

impl SearchSummary {
    /// Full observation of this finite candidate set, never an exhaustive route claim.
    pub fn all_planned_inputs_observed(&self) -> bool {
        self.stop_reason == StopReason::AllInputsAttempted
            && self.completed_requests == self.planned_inputs
            && self.unknown_observations.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchOutcome {
    ApiCandidate {
        candidate: Box<ApiCandidateAskUpper>,
        summary: SearchSummary,
    },
    Unknown(SearchSummary),
    /// Only the finite observed input set/budget was insufficient. Not no-route.
    UncoveredWithinSearchBudget(SearchSummary),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionError {
    BeforeWindow,
    RequestInFlight,
    RequestMismatch,
    ClockRegression,
    NotFinished,
}

#[derive(Debug)]
pub struct SearchSession {
    identity: SearchIdentity,
    policy: SearchPolicy,
    inputs: Vec<SolLamports>,
    pending: Option<QuoteRequest>,
    best: Option<Box<ApiCandidateAskUpper>>,
    last_now_ms: u64,
    issued_requests: usize,
    completed_requests: usize,
    qualifying_quotes: usize,
    insufficient_quotes: usize,
    unknown_observations: Vec<ObservationFailure>,
    stop_reason: Option<StopReason>,
}

impl SearchSession {
    pub fn new(
        identity: SearchIdentity,
        policy: SearchPolicy,
        inputs: &[SolLamports],
    ) -> Result<Self, PlanError> {
        validate_plan(&identity, &policy, inputs)?;
        Ok(Self {
            identity,
            policy,
            inputs: inputs.to_vec(),
            pending: None,
            best: None,
            last_now_ms: identity.window_start_ms,
            issued_requests: 0,
            completed_requests: 0,
            qualifying_quotes: 0,
            insufficient_quotes: 0,
            unknown_observations: Vec::new(),
            stop_reason: None,
        })
    }

    fn observe_time(&mut self, now_ms: u64) -> Result<(), SessionError> {
        if now_ms < self.identity.window_start_ms && self.issued_requests == 0 {
            return Err(SessionError::BeforeWindow);
        }
        if now_ms < self.last_now_ms {
            self.stop_reason = Some(StopReason::ClockRegression);
            self.pending = None;
            self.best = None;
            return Err(SessionError::ClockRegression);
        }
        self.last_now_ms = now_ms;
        Ok(())
    }

    fn expire_or_finish(&mut self, now_ms: u64) {
        if self.stop_reason == Some(StopReason::ClockRegression) {
            return;
        }
        if self
            .pending
            .is_some_and(|request| now_ms >= request.deadline_ms)
        {
            self.pending = None;
            self.completed_requests += 1;
            self.unknown_observations.push(ObservationFailure::Timeout);
        }
        if now_ms >= self.identity.window_end_ms {
            self.stop_reason = Some(StopReason::WindowExpired);
            self.best = None;
        } else if self.pending.is_none() && self.stop_reason.is_none() {
            if self.issued_requests == self.inputs.len() {
                self.stop_reason = Some(StopReason::AllInputsAttempted);
            } else if self.issued_requests == self.policy.max_requests {
                self.stop_reason = Some(StopReason::RequestBudget);
            }
        }
    }

    /// Return at most one new request. No retry/duplicate scheduling is implicit.
    /// When a deadline passes, any pending request becomes Unknown; the transport
    /// must also cancel/finish it before starting the next network operation.
    pub fn next_request(&mut self, now_ms: u64) -> Result<Option<QuoteRequest>, SessionError> {
        self.observe_time(now_ms)?;
        self.expire_or_finish(now_ms);
        if self.stop_reason.is_some() {
            return Ok(None);
        }
        if self.pending.is_some() {
            return Err(SessionError::RequestInFlight);
        }
        let request = QuoteRequest {
            identity: self.identity,
            policy: self.policy,
            input_lamports: self.inputs[self.issued_requests],
            ordinal: self.issued_requests,
            started_ms: now_ms,
            deadline_ms: now_ms
                .saturating_add(self.policy.request_timeout_ms)
                .min(self.identity.window_end_ms),
        };
        self.issued_requests += 1;
        self.pending = Some(request);
        Ok(Some(request))
    }

    fn accept_request(&mut self, request: QuoteRequest, now_ms: u64) -> Result<bool, SessionError> {
        // A stale response may not change the active request or its clock state.
        if self.pending != Some(request) {
            return Err(SessionError::RequestMismatch);
        }
        self.observe_time(now_ms)?;
        self.pending = None;
        self.completed_requests += 1;
        if now_ms >= request.deadline_ms {
            self.unknown_observations.push(ObservationFailure::Timeout);
            self.expire_or_finish(now_ms);
            return Ok(false);
        }
        Ok(true)
    }

    /// Correlate an already parsed response and retain only the cheapest observed
    /// input whose **minimum** output covers exact_debt. Search ordering is free.
    pub fn record_quote(&mut self, quote: ApiQuote, now_ms: u64) -> Result<(), SessionError> {
        if !self.accept_request(quote.request, now_ms)? {
            return Ok(());
        }
        if quote.minimum_output >= self.identity.exact_debt {
            self.qualifying_quotes += 1;
            if self
                .best
                .as_ref()
                .is_none_or(|best| quote.request.input_lamports < best.input_lamports())
            {
                self.best = Some(Box::new(ApiCandidateAskUpper {
                    quote,
                    received_ms: now_ms,
                }));
            }
        } else {
            self.insufficient_quotes += 1;
        }
        self.expire_or_finish(now_ms);
        Ok(())
    }

    pub fn record_failure(
        &mut self,
        request: QuoteRequest,
        failure: ObservationFailure,
        now_ms: u64,
    ) -> Result<(), SessionError> {
        if self.accept_request(request, now_ms)? {
            self.unknown_observations.push(failure);
            self.expire_or_finish(now_ms);
        }
        Ok(())
    }

    /// Consume a stopped session. A candidate is discarded after window expiry
    /// or a clock regression; an API failure never becomes an illiquidity proof.
    pub fn finish(mut self, now_ms: u64) -> Result<SearchOutcome, SessionError> {
        match self.observe_time(now_ms) {
            Ok(()) | Err(SessionError::ClockRegression) => {}
            Err(error) => return Err(error),
        }
        self.expire_or_finish(now_ms);
        let stop_reason = self.stop_reason.ok_or(SessionError::NotFinished)?;
        let summary = SearchSummary {
            planned_inputs: self.inputs.len(),
            issued_requests: self.issued_requests,
            completed_requests: self.completed_requests,
            qualifying_quotes: self.qualifying_quotes,
            insufficient_quotes: self.insufficient_quotes,
            unknown_observations: self.unknown_observations,
            stop_reason,
        };
        if let Some(candidate) = self.best {
            return Ok(SearchOutcome::ApiCandidate { candidate, summary });
        }
        if !summary.unknown_observations.is_empty()
            || matches!(
                stop_reason,
                StopReason::WindowExpired | StopReason::ClockRegression
            )
        {
            Ok(SearchOutcome::Unknown(summary))
        } else {
            Ok(SearchOutcome::UncoveredWithinSearchBudget(summary))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type IdentityCase = (fn(&mut SearchIdentity), PlanError);
    type ResponseCase = (fn(&mut ApiQuoteFields<'_>), ResponseRejection);

    fn identity() -> SearchIdentity {
        SearchIdentity {
            chain_genesis: [1; 32],
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
            model_epoch: 3,
            observation_epoch: 4,
            report_sequence: 5,
            window_start_ms: 1_000,
            window_end_ms: 2_000,
        }
    }

    fn policy() -> SearchPolicy {
        SearchPolicy {
            max_input_lamports: SolLamports(u64::MAX),
            max_requests: MAX_CANDIDATES,
            max_response_bytes: 1_024,
            request_timeout_ms: 100,
            max_window_ms: 1_000,
            slippage_bps: 50,
        }
    }

    fn fields<'a>(
        request: &QuoteRequest,
        input: &'a str,
        expected: &'a str,
        minimum: &'a str,
    ) -> ApiQuoteFields<'a> {
        ApiQuoteFields {
            input_mint: request.identity.input_mint,
            output_mint: request.identity.output_mint,
            in_amount: input,
            expected_out_amount: expected,
            other_amount_threshold: minimum,
            swap_mode: SwapMode::ExactIn,
            slippage_bps: request.policy.slippage_bps,
            platform_fee_bps: 0,
            positive_slippage_fee_bps: 0,
            taker: request.identity.taker,
            account_policy_digest: request.identity.account_policy_digest,
        }
    }

    fn quote(request: QuoteRequest, expected: u64, minimum: u64) -> ApiQuote {
        ApiQuote::parse(
            request,
            fields(
                &request,
                &request.input_lamports.0.to_string(),
                &expected.to_string(),
                &minimum.to_string(),
            ),
            b"retained exact response",
        )
        .unwrap()
    }

    /// Injected source/clock driver. Asynchronous adapters drive the public session
    /// directly and enforce actual cancellation and streaming body caps.
    fn run_fake(
        id: SearchIdentity,
        pol: SearchPolicy,
        inputs: &[u64],
        mut source: impl FnMut(QuoteRequest) -> Result<ApiQuote, ObservationFailure>,
    ) -> SearchOutcome {
        let inputs: Vec<_> = inputs.iter().copied().map(SolLamports).collect();
        let mut session = SearchSession::new(id, pol, &inputs).unwrap();
        let mut now = id.window_start_ms;
        while let Some(request) = session.next_request(now).unwrap() {
            now += 1;
            match source(request) {
                Ok(response) => session.record_quote(response, now).unwrap(),
                Err(error) => session.record_failure(request, error, now).unwrap(),
            }
        }
        session.finish(now).unwrap()
    }

    #[test]
    fn raw_units_are_canonical_lossless_and_checked() {
        for (raw, expected) in [
            ("0", 0),
            ("9007199254740993", 9_007_199_254_740_993),
            ("18446744073709551615", u64::MAX),
        ] {
            assert_eq!(TokenUnits::parse(raw), Ok(TokenUnits(expected)));
            assert_eq!(SolLamports::parse(raw), Ok(SolLamports(expected)));
        }
        for raw in ["", "00", "01", "+1", "-1", "1.0", "1e2", " 1", "1 ", "١"] {
            assert_eq!(
                TokenUnits::parse(raw),
                Err(AmountError::NotCanonicalUnsignedDecimal),
                "{raw}"
            );
        }
        for raw in ["18446744073709551616", "999999999999999999999999999999"] {
            assert_eq!(TokenUnits::parse(raw), Err(AmountError::Overflow));
        }
    }

    #[test]
    fn plan_refuses_invalid_units_identity_quantity_and_window() {
        let cases: &[IdentityCase] = &[
            (|id| id.market = [0; 32], PlanError::MissingIdentity),
            (|id| id.policy_version = 0, PlanError::MissingIdentity),
            (
                |id| id.input_mint = [99; 32],
                PlanError::UnsupportedInputUnits,
            ),
            (|id| id.input_decimals = 6, PlanError::UnsupportedInputUnits),
            (
                |id| id.input_token_program = TOKEN_2022_PROGRAM,
                PlanError::UnsupportedInputUnits,
            ),
            (
                |id| id.output_token_program = [99; 32],
                PlanError::UnsupportedOutputTokenProgram,
            ),
            (|id| id.output_mint = WSOL_MINT, PlanError::SameMint),
            (|id| id.exact_debt = TokenUnits(0), PlanError::ZeroDebt),
            (
                |id| id.window_end_ms = id.window_start_ms,
                PlanError::InvalidWindow,
            ),
            (|id| id.window_end_ms = u64::MAX, PlanError::InvalidWindow),
        ];
        for (mutate, expected) in cases {
            let mut id = identity();
            mutate(&mut id);
            assert_eq!(
                SearchSession::new(id, policy(), &[SolLamports(1)]).unwrap_err(),
                *expected
            );
        }
    }

    #[test]
    fn plan_refuses_unbounded_duplicate_zero_or_over_cap_work() {
        for (inputs, expected) in [
            (vec![], PlanError::EmptyCandidateSet),
            (
                vec![SolLamports(1); MAX_CANDIDATES + 1],
                PlanError::TooManyCandidates,
            ),
            (vec![SolLamports(0)], PlanError::ZeroInput),
            (
                vec![SolLamports(1), SolLamports(1)],
                PlanError::DuplicateInput,
            ),
        ] {
            assert_eq!(
                SearchSession::new(identity(), policy(), &inputs).unwrap_err(),
                expected
            );
        }
        let mut pol = policy();
        pol.max_input_lamports = SolLamports(9);
        assert_eq!(
            SearchSession::new(identity(), pol, &[SolLamports(10)]).unwrap_err(),
            PlanError::InputAboveCap
        );
        let changes: &[fn(&mut SearchPolicy)] = &[
            |p| p.max_requests = 0,
            |p| p.max_requests = MAX_CANDIDATES + 1,
            |p| p.max_response_bytes = 0,
            |p| p.max_response_bytes = MAX_RESPONSE_BYTES + 1,
            |p| p.request_timeout_ms = 0,
            |p| p.max_window_ms = 0,
            |p| p.slippage_bps = 10_000,
            |p| p.max_input_lamports = SolLamports(0),
        ];
        for change in changes {
            let mut pol = policy();
            change(&mut pol);
            assert_eq!(
                SearchSession::new(identity(), pol, &[SolLamports(1)]).unwrap_err(),
                PlanError::InvalidPolicy
            );
        }
    }

    #[test]
    fn scans_nonmonotone_unsorted_quotes_and_selects_least_observed_input() {
        let mut observed = Vec::new();
        let outcome = run_fake(identity(), policy(), &[300, 100, 200], |request| {
            observed.push(request.input_lamports.0);
            let output = match request.input_lamports.0 {
                300 => 100,
                100 => 99,
                200 => 120,
                _ => unreachable!(),
            };
            Ok(quote(request, output, output))
        });
        assert_eq!(observed, vec![300, 100, 200]);
        let SearchOutcome::ApiCandidate { candidate, summary } = outcome else {
            panic!("candidate expected")
        };
        assert_eq!(candidate.input_lamports(), SolLamports(200));
        assert_eq!(candidate.minimum_output(), TokenUnits(120));
        assert_eq!(candidate.raw_response(), b"retained exact response");
        assert_eq!(summary.qualifying_quotes, 2);
        assert!(summary.all_planned_inputs_observed());
    }

    #[test]
    fn expected_output_cannot_replace_slippage_minimum() {
        let outcome = run_fake(identity(), policy(), &[100], |r| Ok(quote(r, 10_000, 99)));
        let SearchOutcome::UncoveredWithinSearchBudget(summary) = outcome else {
            panic!("uncovered expected")
        };
        assert_eq!(summary.insufficient_quotes, 1);
        assert!(summary.all_planned_inputs_observed());
    }

    #[test]
    fn input_budget_is_finite_and_does_not_assert_no_route() {
        let mut pol = policy();
        pol.max_requests = 2;
        let mut calls = 0;
        let outcome = run_fake(identity(), pol, &[1, 2, 3, 4], |request| {
            calls += 1;
            Ok(quote(request, 99, 99))
        });
        let SearchOutcome::UncoveredWithinSearchBudget(summary) = outcome else {
            panic!("uncovered expected")
        };
        assert_eq!(calls, 2);
        assert_eq!(summary.stop_reason, StopReason::RequestBudget);
        assert_eq!(summary.completed_requests, 2);
        assert!(!summary.all_planned_inputs_observed());
    }

    #[test]
    fn api_errors_no_routes_and_rate_limits_are_unknown() {
        for failure in [
            ObservationFailure::ApiNoRoutes,
            ObservationFailure::RateLimited,
            ObservationFailure::Timeout,
            ObservationFailure::Transport,
        ] {
            let outcome = run_fake(identity(), policy(), &[1, 2], |_| Err(failure));
            let SearchOutcome::Unknown(summary) = outcome else {
                panic!("unknown expected")
            };
            assert_eq!(summary.unknown_observations, vec![failure, failure]);
            assert!(!summary.all_planned_inputs_observed());
        }
    }

    #[test]
    fn known_candidate_survives_other_api_failure_without_complete_search_claim() {
        let outcome = run_fake(identity(), policy(), &[1, 2], |request| {
            if request.ordinal == 0 {
                Ok(quote(request, 100, 100))
            } else {
                Err(ObservationFailure::RateLimited)
            }
        });
        let SearchOutcome::ApiCandidate { candidate, summary } = outcome else {
            panic!("candidate expected")
        };
        assert_eq!(candidate.identity(), &identity());
        assert_eq!(candidate.policy(), &policy());
        assert_eq!(summary.unknown_observations.len(), 1);
        assert!(!summary.all_planned_inputs_observed());
    }

    #[test]
    fn response_identity_fee_resource_and_expected_minimum_checks_fail_closed() {
        let mut session = SearchSession::new(identity(), policy(), &[SolLamports(1)]).unwrap();
        let request = session.next_request(1_000).unwrap().unwrap();
        let changes: &[ResponseCase] = &[
            (
                |f| f.input_mint = [99; 32],
                ResponseRejection::IdentityMismatch,
            ),
            (
                |f| f.output_mint = [99; 32],
                ResponseRejection::IdentityMismatch,
            ),
            (|f| f.taker = [99; 32], ResponseRejection::IdentityMismatch),
            (
                |f| f.account_policy_digest = [99; 32],
                ResponseRejection::IdentityMismatch,
            ),
            (
                |f| f.in_amount = "2",
                ResponseRejection::InputAmountMismatch,
            ),
            (
                |f| f.in_amount = "1.0",
                ResponseRejection::InvalidAmount(AmountError::NotCanonicalUnsignedDecimal),
            ),
            (
                |f| f.swap_mode = SwapMode::ExactOut,
                ResponseRejection::SwapModeMismatch,
            ),
            (|f| f.slippage_bps = 51, ResponseRejection::SlippageMismatch),
            (
                |f| f.platform_fee_bps = 1,
                ResponseRejection::AdditionalFeeDeclared,
            ),
            (
                |f| f.positive_slippage_fee_bps = 1,
                ResponseRejection::AdditionalFeeDeclared,
            ),
            (
                |f| f.other_amount_threshold = "101",
                ResponseRejection::MinimumAboveExpected,
            ),
            (
                |f| f.other_amount_threshold = "18446744073709551616",
                ResponseRejection::InvalidAmount(AmountError::Overflow),
            ),
        ];
        for (change, expected) in changes {
            let mut fields = fields(&request, "1", "100", "100");
            change(&mut fields);
            assert_eq!(
                ApiQuote::parse(request, fields, b"body").unwrap_err(),
                *expected
            );
        }
        assert_eq!(
            ApiQuote::parse(request, fields(&request, "1", "100", "100"), b"").unwrap_err(),
            ResponseRejection::EmptyBody
        );
        let large = vec![b'x'; policy().max_response_bytes + 1];
        assert_eq!(
            ApiQuote::parse(request, fields(&request, "1", "100", "100"), &large).unwrap_err(),
            ResponseRejection::BodyTooLarge
        );
        let max = vec![b'x'; policy().max_response_bytes];
        let parsed = ApiQuote::parse(request, fields(&request, "1", "100", "100"), &max).unwrap();
        session.record_quote(parsed, 1_001).unwrap();
        let SearchOutcome::ApiCandidate { candidate, .. } = session.finish(1_001).unwrap() else {
            panic!("candidate")
        };
        assert_eq!(candidate.raw_response(), max);
    }

    #[test]
    fn malformed_response_becomes_unknown_in_source_driver() {
        let outcome = run_fake(identity(), policy(), &[1], |request| {
            ApiQuote::parse(request, fields(&request, "1", "100", "101"), b"body")
                .map_err(ObservationFailure::Rejected)
        });
        assert!(matches!(outcome, SearchOutcome::Unknown(_)));
    }

    #[test]
    fn only_one_request_is_in_flight_and_stale_responses_cannot_clear_it() {
        let mut session =
            SearchSession::new(identity(), policy(), &[SolLamports(1), SolLamports(2)]).unwrap();
        let first = session.next_request(1_000).unwrap().unwrap();
        assert_eq!(
            session.next_request(1_001).unwrap_err(),
            SessionError::RequestInFlight
        );
        session.record_quote(quote(first, 100, 100), 1_002).unwrap();
        let second = session.next_request(1_003).unwrap().unwrap();
        assert_eq!(
            session
                .record_quote(quote(first, 100, 100), 1_004)
                .unwrap_err(),
            SessionError::RequestMismatch
        );
        assert_eq!(
            session.next_request(1_004).unwrap_err(),
            SessionError::RequestInFlight
        );
        session
            .record_quote(quote(second, 100, 100), 1_005)
            .unwrap();
        let SearchOutcome::ApiCandidate { summary, .. } = session.finish(1_005).unwrap() else {
            panic!("candidate")
        };
        assert_eq!(summary.completed_requests, 2);
    }

    #[test]
    fn quantity_policy_epoch_and_mint_replays_are_not_accepted() {
        for change in [
            (|id: &mut SearchIdentity| id.exact_debt = TokenUnits(101)) as fn(&mut SearchIdentity),
            |id| id.policy_digest = [9; 32],
            |id| id.policy_version += 1,
            |id| id.observation_epoch += 1,
            |id| id.model_epoch += 1,
            |id| id.report_sequence += 1,
            |id| id.output_mint = [9; 32],
            |id| id.output_decimals += 1,
            |id| id.chain_genesis = [9; 32],
        ] {
            let mut original = SearchSession::new(identity(), policy(), &[SolLamports(1)]).unwrap();
            let original_request = original.next_request(1_000).unwrap().unwrap();
            let mut changed = identity();
            change(&mut changed);
            let mut session = SearchSession::new(changed, policy(), &[SolLamports(1)]).unwrap();
            let request = session.next_request(1_000).unwrap().unwrap();
            assert_eq!(
                session
                    .record_quote(quote(original_request, 101, 101), 1_001)
                    .unwrap_err(),
                SessionError::RequestMismatch
            );
            session
                .record_quote(quote(request, 101, 101), 1_001)
                .unwrap();
        }
        let mut original = SearchSession::new(identity(), policy(), &[SolLamports(1)]).unwrap();
        let request = original.next_request(1_000).unwrap().unwrap();
        let mut changed_policy = policy();
        changed_policy.slippage_bps += 1;
        let mut changed =
            SearchSession::new(identity(), changed_policy, &[SolLamports(1)]).unwrap();
        changed.next_request(1_000).unwrap();
        assert_eq!(
            changed
                .record_quote(quote(request, 100, 100), 1_001)
                .unwrap_err(),
            SessionError::RequestMismatch
        );
    }

    #[test]
    fn late_response_is_unknown_and_new_request_is_not_its_replacement() {
        let mut session =
            SearchSession::new(identity(), policy(), &[SolLamports(1), SolLamports(2)]).unwrap();
        let first = session.next_request(1_000).unwrap().unwrap();
        assert_eq!(first.deadline_ms(), 1_100);
        let second = session.next_request(1_100).unwrap().unwrap();
        assert_eq!(
            session
                .record_quote(quote(first, 100, 100), 1_101)
                .unwrap_err(),
            SessionError::RequestMismatch
        );
        session.record_quote(quote(second, 99, 99), 1_102).unwrap();
        let SearchOutcome::Unknown(summary) = session.finish(1_102).unwrap() else {
            panic!("unknown")
        };
        assert_eq!(
            summary.unknown_observations,
            vec![ObservationFailure::Timeout]
        );
    }

    #[test]
    fn exact_deadline_response_cannot_be_qualifying() {
        let mut session = SearchSession::new(identity(), policy(), &[SolLamports(1)]).unwrap();
        let request = session.next_request(1_000).unwrap().unwrap();
        session
            .record_quote(quote(request, 100, 100), 1_100)
            .unwrap();
        assert!(matches!(
            session.finish(1_100).unwrap(),
            SearchOutcome::Unknown(_)
        ));
    }

    #[test]
    fn expired_window_cannot_be_repaired_by_a_fresh_signature_or_fetch() {
        let mut session = SearchSession::new(identity(), policy(), &[SolLamports(1)]).unwrap();
        let request = session.next_request(1_000).unwrap().unwrap();
        session
            .record_quote(quote(request, 100, 100), 1_001)
            .unwrap();
        let SearchOutcome::Unknown(summary) = session.finish(2_000).unwrap() else {
            panic!("expired candidate")
        };
        assert_eq!(summary.stop_reason, StopReason::WindowExpired);
    }

    #[test]
    fn clock_regression_discards_prior_candidate() {
        let mut session =
            SearchSession::new(identity(), policy(), &[SolLamports(1), SolLamports(2)]).unwrap();
        let request = session.next_request(1_000).unwrap().unwrap();
        session
            .record_quote(quote(request, 100, 100), 1_010)
            .unwrap();
        assert_eq!(
            session.next_request(1_009).unwrap_err(),
            SessionError::ClockRegression
        );
        let SearchOutcome::Unknown(summary) = session.finish(1_011).unwrap() else {
            panic!("clock regression")
        };
        assert_eq!(summary.stop_reason, StopReason::ClockRegression);
    }

    #[test]
    fn no_early_finish_and_saturating_deadline_has_no_overflow() {
        let session = SearchSession::new(identity(), policy(), &[SolLamports(1)]).unwrap();
        assert_eq!(
            session.finish(1_000).unwrap_err(),
            SessionError::NotFinished
        );
        let mut id = identity();
        id.window_start_ms = u64::MAX - 20;
        id.window_end_ms = u64::MAX;
        let mut session = SearchSession::new(id, policy(), &[SolLamports(1)]).unwrap();
        assert_eq!(
            session.next_request(id.window_start_ms - 1).unwrap_err(),
            SessionError::BeforeWindow
        );
        let request = session.next_request(id.window_start_ms).unwrap().unwrap();
        assert_eq!(request.deadline_ms(), u64::MAX);
        session
            .record_quote(quote(request, u64::MAX, u64::MAX), u64::MAX - 1)
            .unwrap();
        assert!(matches!(
            session.finish(u64::MAX - 1).unwrap(),
            SearchOutcome::ApiCandidate { .. }
        ));
    }

    #[test]
    fn maximum_u64_values_do_not_use_float_inversion() {
        let mut id = identity();
        id.exact_debt = TokenUnits(u64::MAX);
        let outcome = run_fake(id, policy(), &[u64::MAX], |r| {
            Ok(quote(r, u64::MAX, u64::MAX))
        });
        let SearchOutcome::ApiCandidate { candidate, .. } = outcome else {
            panic!("candidate")
        };
        assert_eq!(candidate.minimum_output(), TokenUnits(u64::MAX));
        assert_eq!(candidate.input_lamports(), SolLamports(u64::MAX));
    }
}
