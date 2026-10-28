//! Trusted in-house, two-sided Jupiter API price ESTIMATES for one exact q.
//!
//! Bid and ask are independently requested. They are not a proof of global best price,
//! authenticated upstream state age, fair acquisition cost or executable net-vault output.
//! Configured buffers/spread/time bounds are committed into the operator model policy.
//! Only the service-owned fixed-endpoint transport feeds this pipeline; no arbitrary
//! transaction signing endpoint or submission exists. Financial execution must separately
//! enforce actual full-q delivery, spending bounds and forced-action authorization.

use crate::{
    offchain_price_history::OperatorObservation,
    price_budget::{RequestClass, SharedRequestBudget},
    price_http::{
        JUPITER_BUILD_URL, JupiterBuildClient, MAINNET_GENESIS, MAX_ROUTE_STEPS, MAX_SWAP_ACCOUNTS,
        MAX_SWAP_DATA_BYTES,
    },
    price_search::{
        ApiCandidateAskUpper, SearchIdentity, SearchOutcome, SearchPolicy, SearchSession,
        SolLamports, TOKEN_2022_PROGRAM, TOKEN_PROGRAM, TokenUnits, WSOL_MINT,
    },
};
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::{Client, StatusCode, Url, header::HeaderValue, redirect};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use shortwin_jupiter_route::{
    AccountMeta, EVENT_AUTHORITY, JUPITER_PROGRAM, ParsedHeader, RouteKind,
};
use shortwin_price_report::{Address, Configuration, CostMarks, ObservationWindow, RuntimeClock};
use shortwin_services_common::Chain;
use solana_pubkey::Pubkey;
use std::{
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceError {
    Policy,
    Configuration,
    Chain,
    Busy,
    Budget,
    Transport,
    Timeout,
    HttpStatus,
    BodyLimit,
    Decode,
    Identity,
    Amount,
    Instruction,
    Fees,
    LocalWindow,
    Unknown,
    Crossed,
    Spread,
    Arithmetic,
    Evidence,
}
impl std::fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "trusted Jupiter reference: {self:?}")
    }
}
impl std::error::Error for ReferenceError {}

/// Every value is an explicitly reviewed operator setting; none is an approved
/// mainnet default. This policy accepts the API's market-state freshness on trust.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrustedReferencePolicy {
    pub bid_haircut_bps: u16,
    pub ask_buffer_bps: u16,
    pub maximum_spread_bps: u16,
    pub maximum_observation_slots: u64,
    pub maximum_observation_seconds: u64,
    pub minimum_request_spacing_ms: u64,
    pub api_market_state_age_is_trusted: bool,
}
#[derive(Clone)]
pub struct TrustedJupiterPlan {
    pub configuration: Configuration,
    /// Immutable per-feed commitment read from the governed snapshot wrapper.
    pub expected_producer_commitment: Address,
    pub reference: TrustedReferencePolicy,
    pub search: SearchPolicy,
    pub ask_inputs: Vec<SolLamports>,
    pub taker: Address,
    pub account_policy_digest: Address,
}
impl TrustedJupiterPlan {
    /// Canonical policy commitment includes identities, model version/epoch, q,
    /// history/freshness settings, exact search list, taker and indicative semantics.
    /// The shared financial policy_hash is included; expected_producer_commitment is
    /// excluded. The global financial policy must not recursively hash this result.
    pub fn policy_commitment(&self) -> Address {
        let c = self.configuration;
        let d = c.domain();
        let p = c.policy();
        let mut h = Sha256::new();
        h.update(b"shortwin-trusted-jupiter-bid-ask-estimates-v1");
        h.update(JUPITER_BUILD_URL.as_bytes());
        for key in [
            d.genesis_hash,
            d.program,
            d.config,
            d.market,
            d.base_mint,
            d.base_token_program,
            d.quote_mint,
            d.quote_token_program,
            self.taker,
            self.account_policy_digest,
        ] {
            h.update(key);
        }
        h.update([d.base_decimals, d.quote_decimals]);
        h.update(c.quantity().to_le_bytes());
        h.update(c.model().version.to_le_bytes());
        h.update(c.model().epoch.to_le_bytes());
        h.update(c.model().policy_hash);
        h.update(c.epoch().to_le_bytes());
        for n in [
            p.max_age_slots,
            p.max_age_seconds,
            p.max_window_slots,
            p.max_window_seconds,
            p.max_update_gap_slots,
            p.max_update_gap_seconds,
            p.epoch_seconds,
        ] {
            h.update(n.to_le_bytes());
        }
        h.update([p.minimum_completed_epochs]);
        for n in [
            self.reference.bid_haircut_bps,
            self.reference.ask_buffer_bps,
            self.reference.maximum_spread_bps,
            self.search.slippage_bps,
        ] {
            h.update(n.to_le_bytes());
        }
        for n in [
            self.reference.maximum_observation_slots,
            self.reference.maximum_observation_seconds,
            self.reference.minimum_request_spacing_ms,
            self.search.max_input_lamports.0,
            self.search.max_requests as u64,
            self.search.max_response_bytes as u64,
            self.search.request_timeout_ms,
            self.search.max_window_ms,
            self.ask_inputs.len() as u64,
        ] {
            h.update(n.to_le_bytes());
        }
        h.update([u8::from(self.reference.api_market_state_age_is_trusted)]);
        for amount in &self.ask_inputs {
            h.update(amount.0.to_le_bytes());
        }
        h.finalize().into()
    }
    pub fn validate(&self) -> Result<(), ReferenceError> {
        let d = self.configuration.domain();
        let r = self.reference;
        if d.genesis_hash != MAINNET_GENESIS
            || d.quote_mint != WSOL_MINT
            || d.quote_token_program != TOKEN_PROGRAM
            || d.quote_decimals != 9
            || ![TOKEN_PROGRAM, TOKEN_2022_PROGRAM].contains(&d.base_token_program)
            || self.taker == [0; 32]
            || self.account_policy_digest == [0; 32]
            || r.bid_haircut_bps >= 10_000
            || r.ask_buffer_bps > 10_000
            || r.maximum_spread_bps == 0
            || r.maximum_spread_bps >= 10_000
            || r.maximum_observation_slots == 0
            || r.maximum_observation_seconds == 0
            || !r.api_market_state_age_is_trusted
            || r.maximum_observation_slots > self.configuration.policy().max_window_slots
            || r.maximum_observation_seconds > self.configuration.policy().max_window_seconds
            || self.search.request_timeout_ms > 30_000
            || self.search.max_window_ms > 300_000
            || self.ask_inputs.len() > 63
            || self.ask_inputs.is_empty()
            || self.search.max_requests < self.ask_inputs.len()
            || self.policy_commitment() != self.expected_producer_commitment
        {
            return Err(ReferenceError::Policy);
        }
        SearchSession::new(self.identity(0, 1, 1), self.search, &self.ask_inputs)
            .map_err(|_| ReferenceError::Policy)?;
        Ok(())
    }
    fn identity(&self, start_ms: u64, epoch: u64, local_sequence: u64) -> SearchIdentity {
        let c = self.configuration;
        let d = c.domain();
        SearchIdentity {
            chain_genesis: d.genesis_hash,
            program: d.program,
            configuration: d.config,
            market: d.market,
            input_mint: d.quote_mint,
            input_token_program: d.quote_token_program,
            input_decimals: d.quote_decimals,
            output_mint: d.base_mint,
            output_token_program: d.base_token_program,
            output_decimals: d.base_decimals,
            exact_debt: TokenUnits(c.quantity()),
            policy_digest: c.model().policy_hash,
            policy_version: c.model().version,
            account_policy_digest: self.account_policy_digest,
            taker: self.taker,
            model_epoch: c.model().epoch,
            observation_epoch: epoch,
            report_sequence: local_sequence,
            window_start_ms: start_ms,
            window_end_ms: start_ms.saturating_add(self.search.max_window_ms),
        }
    }
}

/// Exact raw bodies are retained for the caller's durable evidence journal, but
/// deliberately excluded from Debug and routine logging.
pub struct CollectedReference {
    pub observation: OperatorObservation,
    pub window: ObservationWindow,
    pub bid_lamports: u64,
    pub ask_lamports: u64,
    pub marks: CostMarks,
    pub digest: Address,
    pub bid_raw_response: Vec<u8>,
    pub ask_raw_response: Vec<u8>,
}
/// Service-owned HTTP clients only. One operation at a time; every API attempt
/// reserves the shared account request budget, including the independent bid.
pub struct TrustedJupiterSource {
    ask: JupiterBuildClient,
    client: Client,
    api_key: HeaderValue,
    endpoint: Url,
    budget: Arc<SharedRequestBudget>,
    active: Arc<Semaphore>,
}
impl TrustedJupiterSource {
    pub fn new(api_key: &str, budget: Arc<SharedRequestBudget>) -> Result<Self, ReferenceError> {
        let ask = JupiterBuildClient::new(api_key, budget.clone(), RequestClass::Background)
            .map_err(|_| ReferenceError::Configuration)?;
        let mut header =
            HeaderValue::from_str(api_key).map_err(|_| ReferenceError::Configuration)?;
        if api_key.is_empty() {
            return Err(ReferenceError::Configuration);
        }
        header.set_sensitive(true);
        Ok(Self {
            ask,
            client: Client::builder()
                .https_only(true)
                .redirect(redirect::Policy::none())
                .retry(reqwest::retry::never())
                .build()
                .map_err(|_| ReferenceError::Configuration)?,
            api_key: header,
            endpoint: Url::parse(JUPITER_BUILD_URL).map_err(|_| ReferenceError::Configuration)?,
            budget,
            active: Arc::new(Semaphore::new(1)),
        })
    }
    /// Actual configured RPC clock bounds collection. This is LOCAL timing of API
    /// responses, not authenticated Jupiter pool-state age. Genesis is independently
    /// verified by the watcher before this method is used.
    pub async fn collect<C: Chain>(
        &self,
        plan: &TrustedJupiterPlan,
        chain: &C,
        local_sequence: u64,
    ) -> Result<CollectedReference, ReferenceError> {
        plan.validate()?;
        if local_sequence == 0 {
            return Err(ReferenceError::Configuration);
        }
        let permit = self
            .active
            .clone()
            .try_acquire_owned()
            .map_err(|_| ReferenceError::Busy)?;
        let start = Instant::now();
        tokio::time::timeout(
            Duration::from_millis(plan.search.max_window_ms),
            self.collect_inner(plan, chain, local_sequence, start, permit),
        )
        .await
        .map_err(|_| ReferenceError::Timeout)?
    }
    async fn collect_inner<C: Chain>(
        &self,
        plan: &TrustedJupiterPlan,
        chain: &C,
        local_sequence: u64,
        started: Instant,
        permit: OwnedSemaphorePermit,
    ) -> Result<CollectedReference, ReferenceError> {
        let begin = chain.clock().await.map_err(|_| ReferenceError::Chain)?;
        let begin = RuntimeClock {
            slot: begin.slot,
            unix_timestamp: begin.unix_timestamp,
        };
        let epoch = u64::try_from(begin.unix_timestamp).map_err(|_| ReferenceError::Chain)?
            / plan.configuration.policy().epoch_seconds;
        let (bid, bid_body, _permit) = self.bid(plan, permit).await?;
        let now = elapsed_ms(started)?;
        let id = plan.identity(0, epoch, local_sequence);
        let mut session = SearchSession::new(id, plan.search, &plan.ask_inputs)
            .map_err(|_| ReferenceError::Policy)?;
        let mut last_started = now;
        loop {
            let next = last_started.saturating_add(plan.reference.minimum_request_spacing_ms);
            let now = elapsed_ms(started)?;
            if now < next {
                tokio::time::sleep(Duration::from_millis(next - now)).await;
            }
            let now = elapsed_ms(started)?;
            let Some(request) = session
                .next_request(now)
                .map_err(|_| ReferenceError::LocalWindow)?
            else {
                break;
            };
            last_started = now;
            match self.ask.fetch(request, now).await {
                Ok(quote) => session
                    .record_quote(quote, elapsed_ms(started)?)
                    .map_err(|_| ReferenceError::Unknown)?,
                Err(error) => session
                    .record_failure(request, error.observation_failure(), elapsed_ms(started)?)
                    .map_err(|_| ReferenceError::Unknown)?,
            }
            if request.ordinal() + 1 >= plan.ask_inputs.len() {
                break;
            }
        }
        let outcome = session
            .finish(elapsed_ms(started)?)
            .map_err(|_| ReferenceError::Unknown)?;
        let SearchOutcome::ApiCandidate { candidate, summary } = outcome else {
            return Err(ReferenceError::Unknown);
        };
        if !summary.all_planned_inputs_observed() {
            return Err(ReferenceError::Unknown);
        }
        let end = chain.clock().await.map_err(|_| ReferenceError::Chain)?;
        let window = ObservationWindow {
            start: begin,
            end: RuntimeClock {
                slot: end.slot,
                unix_timestamp: end.unix_timestamp,
            },
        };
        evaluate(
            plan,
            bid,
            &bid_body,
            &candidate,
            window,
            elapsed_ms(started)?,
        )
    }
    async fn bid(
        &self,
        plan: &TrustedJupiterPlan,
        permit: OwnedSemaphorePermit,
    ) -> Result<(u64, Vec<u8>, OwnedSemaphorePermit), ReferenceError> {
        let budget = self.budget.clone();
        let (reserved, permit) = tokio::task::spawn_blocking(move || {
            (budget.try_reserve(RequestClass::Background), permit)
        })
        .await
        .map_err(|_| ReferenceError::Budget)?;
        reserved.map_err(|_| ReferenceError::Budget)?;
        // Permit remains held through both bid and subsequent ask collection.
        let d = plan.configuration.domain();
        let query = [
            ("inputMint", Pubkey::new_from_array(d.base_mint).to_string()),
            (
                "outputMint",
                Pubkey::new_from_array(d.quote_mint).to_string(),
            ),
            ("amount", plan.configuration.quantity().to_string()),
            ("taker", Pubkey::new_from_array(plan.taker).to_string()),
            ("slippageBps", plan.search.slippage_bps.to_string()),
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
            .timeout(Duration::from_millis(plan.search.request_timeout_ms))
            .send()
            .await
            .map_err(|_| ReferenceError::Transport)?;
        if response.status() != StatusCode::OK {
            return Err(ReferenceError::HttpStatus);
        }
        let limit = plan.search.max_response_bytes;
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err(ReferenceError::BodyLimit);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ReferenceError::Transport)?
        {
            if chunk.len() > limit - body.len() {
                return Err(ReferenceError::BodyLimit);
            }
            body.extend_from_slice(&chunk);
        }
        let bid = decode_bid(plan, &body)?;
        Ok((bid, body, permit))
    }
}
fn elapsed_ms(start: Instant) -> Result<u64, ReferenceError> {
    u64::try_from(start.elapsed().as_millis()).map_err(|_| ReferenceError::LocalWindow)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BidResponse {
    input_mint: String,
    output_mint: String,
    in_amount: String,
    out_amount: String,
    other_amount_threshold: String,
    swap_mode: String,
    slippage_bps: u16,
    swap_instruction: BidInstruction,
    error: Option<serde::de::IgnoredAny>,
    error_code: Option<serde::de::IgnoredAny>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BidInstruction {
    program_id: String,
    accounts: Vec<BidAccount>,
    data: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BidAccount {
    pubkey: String,
    is_writable: bool,
    is_signer: bool,
}
fn address(raw: &str) -> Result<Address, ReferenceError> {
    Pubkey::from_str(raw)
        .map(|p| p.to_bytes())
        .map_err(|_| ReferenceError::Identity)
}
fn decode_bid(plan: &TrustedJupiterPlan, body: &[u8]) -> Result<u64, ReferenceError> {
    if body.is_empty() || body.len() > plan.search.max_response_bytes {
        return Err(ReferenceError::BodyLimit);
    }
    let r: BidResponse = serde_json::from_slice(body).map_err(|_| ReferenceError::Decode)?;
    let d = plan.configuration.domain();
    if r.error.is_some()
        || r.error_code.is_some()
        || r.swap_mode != "ExactIn"
        || address(&r.input_mint)? != d.base_mint
        || address(&r.output_mint)? != d.quote_mint
        || r.slippage_bps != plan.search.slippage_bps
    {
        return Err(ReferenceError::Identity);
    }
    let input = TokenUnits::parse(&r.in_amount)
        .map_err(|_| ReferenceError::Amount)?
        .0;
    let out = SolLamports::parse(&r.out_amount)
        .map_err(|_| ReferenceError::Amount)?
        .0;
    let min = SolLamports::parse(&r.other_amount_threshold)
        .map_err(|_| ReferenceError::Amount)?
        .0;
    if input != plan.configuration.quantity() || out == 0 || min > out {
        return Err(ReferenceError::Amount);
    }
    if r.swap_instruction.accounts.len() < 10
        || r.swap_instruction.accounts.len() > MAX_SWAP_ACCOUNTS
        || r.swap_instruction.data.len() > MAX_SWAP_DATA_BYTES.div_ceil(3) * 4
        || address(&r.swap_instruction.program_id)? != JUPITER_PROGRAM
    {
        return Err(ReferenceError::Instruction);
    }
    let bytes = STANDARD
        .decode(&r.swap_instruction.data)
        .map_err(|_| ReferenceError::Decode)?;
    if bytes.len() > MAX_SWAP_DATA_BYTES {
        return Err(ReferenceError::Instruction);
    }
    let header = ParsedHeader::parse(&bytes).map_err(|_| ReferenceError::Instruction)?;
    if header.kind() != RouteKind::ExactIn
        || header.declared_route_steps() > MAX_ROUTE_STEPS
        || header.amount() != input
        || header.quoted_amount() != out
        || header.slippage_bps() != r.slippage_bps
    {
        return Err(ReferenceError::Instruction);
    }
    if header.platform_fee_bps() != 0 || header.positive_slippage_bps() != 0 {
        return Err(ReferenceError::Fees);
    }
    let accounts = r
        .swap_instruction
        .accounts
        .iter()
        .map(|a| {
            Ok(AccountMeta {
                key: address(&a.pubkey)?,
                is_signer: a.is_signer,
                is_writable: a.is_writable,
            })
        })
        .collect::<Result<Vec<_>, ReferenceError>>()?;
    let fixed = [
        (0, plan.taker),
        (3, d.base_mint),
        (4, d.quote_mint),
        (5, d.base_token_program),
        (6, d.quote_token_program),
        (8, EVENT_AUTHORITY),
        (9, JUPITER_PROGRAM),
    ];
    let readonly = [
        plan.taker,
        d.base_mint,
        d.quote_mint,
        d.base_token_program,
        d.quote_token_program,
        EVENT_AUTHORITY,
        JUPITER_PROGRAM,
    ];
    if fixed.iter().any(|(i, k)| accounts[*i].key != *k)
        || !accounts[0].is_signer
        || !accounts[1].is_writable
        || !accounts[2].is_writable
        || accounts[1].key == accounts[2].key
        || accounts[1].key == [0; 32]
        || accounts[2].key == [0; 32]
        || (accounts[7].key != JUPITER_PROGRAM && accounts[7].key != accounts[2].key)
        || readonly.contains(&accounts[1].key)
        || readonly.contains(&accounts[2].key)
        || accounts.iter().any(|a| {
            (a.is_signer && a.key != plan.taker) || (a.is_writable && readonly.contains(&a.key))
        })
    {
        return Err(ReferenceError::Identity);
    }
    Ok(out)
}
fn evaluate(
    plan: &TrustedJupiterPlan,
    bid: u64,
    bid_body: &[u8],
    ask: &ApiCandidateAskUpper,
    window: ObservationWindow,
    now_ms: u64,
) -> Result<CollectedReference, ReferenceError> {
    let d = plan.configuration.domain();
    let id = ask.identity();
    if id.chain_genesis != d.genesis_hash
        || id.program != d.program
        || id.configuration != d.config
        || id.market != d.market
        || id.output_mint != d.base_mint
        || id.output_token_program != d.base_token_program
        || id.output_decimals != d.base_decimals
        || id.input_mint != d.quote_mint
        || id.input_token_program != d.quote_token_program
        || id.input_decimals != d.quote_decimals
        || id.exact_debt.0 != plan.configuration.quantity()
        || id.policy_digest != plan.configuration.model().policy_hash
        || id.model_epoch != plan.configuration.model().epoch
        || id.taker != plan.taker
        || id.account_policy_digest != plan.account_policy_digest
        || !ask.within_local_window(now_ms)
    {
        return Err(ReferenceError::Identity);
    }
    let seconds = window
        .end
        .unix_timestamp
        .checked_sub(window.start.unix_timestamp)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(ReferenceError::LocalWindow)?;
    let slots = window
        .end
        .slot
        .checked_sub(window.start.slot)
        .ok_or(ReferenceError::LocalWindow)?;
    if window.start.unix_timestamp < 0
        || slots > plan.reference.maximum_observation_slots
        || seconds > plan.reference.maximum_observation_seconds
        || window.start.unix_timestamp as u64 / plan.configuration.policy().epoch_seconds
            != window.end.unix_timestamp as u64 / plan.configuration.policy().epoch_seconds
    {
        return Err(ReferenceError::LocalWindow);
    }
    let ask_cost = ask.input_lamports().0;
    if bid == 0 || bid > ask_cost {
        return Err(ReferenceError::Crossed);
    }
    if u128::from(ask_cost - bid) * 10_000
        > u128::from(bid) * u128::from(plan.reference.maximum_spread_bps)
    {
        return Err(ReferenceError::Spread);
    }
    let lower = u128::from(bid) * u128::from(10_000 - plan.reference.bid_haircut_bps) / 10_000;
    let upper = (u128::from(ask_cost) * u128::from(10_000 + plan.reference.ask_buffer_bps))
        .div_ceil(10_000);
    let central = (lower + upper) / 2;
    let marks = CostMarks::new(
        u64::try_from(lower).map_err(|_| ReferenceError::Arithmetic)?,
        u64::try_from(central).map_err(|_| ReferenceError::Arithmetic)?,
        u64::try_from(upper).map_err(|_| ReferenceError::Arithmetic)?,
    )
    .map_err(|_| ReferenceError::Arithmetic)?;
    let mut h = Sha256::new();
    h.update(b"shortwin-private-jupiter-two-sided-observation-v1");
    h.update(plan.policy_commitment());
    h.update(id.report_sequence.to_le_bytes());
    h.update(Sha256::digest(bid_body));
    h.update(Sha256::digest(ask.raw_response()));
    for clock in [window.start, window.end] {
        h.update(clock.slot.to_le_bytes());
        h.update(clock.unix_timestamp.to_le_bytes());
    }
    let digest: Address = h.finalize().into();
    let observation = OperatorObservation::qualified_finite(window, digest, marks)
        .map_err(|_| ReferenceError::Evidence)?;
    Ok(CollectedReference {
        observation,
        window,
        bid_lamports: bid,
        ask_lamports: ask_cost,
        marks,
        digest,
        bid_raw_response: bid_body.to_vec(),
        ask_raw_response: ask.raw_response().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_search::{ApiQuote, ApiQuoteFields, SwapMode};
    use serde_json::{Value, json};
    use shortwin_price_report::{Domain, HistoryPolicy, ModelBinding};
    fn plan() -> TrustedJupiterPlan {
        let configuration = Configuration::new(
            Domain {
                genesis_hash: MAINNET_GENESIS,
                program: [2; 32],
                config: [3; 32],
                market: [4; 32],
                base_mint: [5; 32],
                base_token_program: TOKEN_2022_PROGRAM,
                quote_mint: WSOL_MINT,
                quote_token_program: TOKEN_PROGRAM,
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
                max_age_slots: 40,
                max_age_seconds: 10,
                max_window_slots: 20,
                max_window_seconds: 5,
                max_update_gap_slots: 20,
                max_update_gap_seconds: 5,
                epoch_seconds: 10,
                minimum_completed_epochs: 2,
            },
        )
        .unwrap();
        let mut plan = TrustedJupiterPlan {
            configuration,
            expected_producer_commitment: [0; 32],
            reference: TrustedReferencePolicy {
                bid_haircut_bps: 100,
                ask_buffer_bps: 200,
                maximum_spread_bps: 3_000,
                maximum_observation_slots: 20,
                maximum_observation_seconds: 5,
                minimum_request_spacing_ms: 0,
                api_market_state_age_is_trusted: true,
            },
            search: SearchPolicy {
                max_input_lamports: SolLamports(1_000),
                max_requests: 1,
                max_response_bytes: 65_536,
                request_timeout_ms: 1_000,
                max_window_ms: 5_000,
                slippage_bps: 50,
            },
            ask_inputs: vec![SolLamports(110)],
            taker: [8; 32],
            account_policy_digest: [7; 32],
        };
        bind(&mut plan);
        plan
    }
    fn bind(plan: &mut TrustedJupiterPlan) {
        plan.expected_producer_commitment = plan.policy_commitment();
    }

    fn ask(plan: &TrustedJupiterPlan) -> Box<ApiCandidateAskUpper> {
        let id = plan.identity(0, 2, 1);
        let mut session = SearchSession::new(id, plan.search, &plan.ask_inputs).unwrap();
        let request = session.next_request(0).unwrap().unwrap();
        let input = request.input_lamports().0.to_string();
        let quote = ApiQuote::parse(
            request,
            ApiQuoteFields {
                input_mint: id.input_mint,
                output_mint: id.output_mint,
                in_amount: &input,
                expected_out_amount: "1100",
                other_amount_threshold: "1000",
                swap_mode: SwapMode::ExactIn,
                slippage_bps: 50,
                platform_fee_bps: 0,
                positive_slippage_fee_bps: 0,
                taker: id.taker,
                account_policy_digest: id.account_policy_digest,
            },
            b"test-only ask API declaration",
        )
        .unwrap();
        session.record_quote(quote, 1).unwrap();
        match session.finish(1).unwrap() {
            SearchOutcome::ApiCandidate { candidate, .. } => candidate,
            _ => panic!(),
        }
    }
    fn pk(key: Address) -> String {
        Pubkey::new_from_array(key).to_string()
    }
    fn bid_body(plan: &TrustedJupiterPlan) -> Value {
        let d = plan.configuration.domain();
        let keys = [
            plan.taker,
            [11; 32],
            [12; 32],
            d.base_mint,
            d.quote_mint,
            d.base_token_program,
            d.quote_token_program,
            JUPITER_PROGRAM,
            EVENT_AUTHORITY,
            JUPITER_PROGRAM,
        ];
        let accounts: Vec<_> = keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                json!({
                    "pubkey":pk(*key),"isWritable":i==1||i==2,"isSigner":i==0
                })
            })
            .collect();
        let mut data = shortwin_jupiter_route::ROUTE_V2.to_vec();
        data.extend_from_slice(&plan.configuration.quantity().to_le_bytes());
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(&50u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&[0, 0x10, 0x27, 0, 1]);
        json!({"inputMint":pk(d.base_mint),"outputMint":pk(d.quote_mint),"inAmount":"1000",
            "outAmount":"100","otherAmountThreshold":"99","swapMode":"ExactIn","slippageBps":50,
            "swapInstruction":{"programId":pk(JUPITER_PROGRAM),"accounts":accounts,"data":STANDARD.encode(data)},
            "blockhashWithMetadata":{"fetchedAt":{"secs_since_epoch":9876543210u64}}})
    }
    fn window() -> ObservationWindow {
        ObservationWindow {
            start: RuntimeClock {
                slot: 100,
                unix_timestamp: 20,
            },
            end: RuntimeClock {
                slot: 104,
                unix_timestamp: 21,
            },
        }
    }
    #[test]
    fn strict_bid_binds_direction_exact_quantity_token_programs_scalars_and_actual_instruction_fees()
     {
        let p = plan();
        let valid = bid_body(&p);
        assert_eq!(
            decode_bid(&p, &serde_json::to_vec(&valid).unwrap()),
            Ok(100)
        );
        for field in ["inAmount", "outAmount", "otherAmountThreshold"] {
            let mut bad = valid.clone();
            bad[field] = json!("0100");
            assert!(decode_bid(&p, &serde_json::to_vec(&bad).unwrap()).is_err());
        }
        for (field, value) in [
            ("inputMint", json!(pk(WSOL_MINT))),
            ("outputMint", json!(pk([99; 32]))),
            ("inAmount", json!("999")),
            ("outAmount", json!("101")),
            ("swapMode", json!("ExactOut")),
            ("slippageBps", json!(51)),
            ("error", json!("untrusted provider text")),
        ] {
            let mut bad = valid.clone();
            bad[field] = value;
            assert!(decode_bid(&p, &serde_json::to_vec(&bad).unwrap()).is_err());
        }
        for (index, field, value) in [
            (0, "isSigner", json!(false)),
            (5, "pubkey", json!(pk(TOKEN_PROGRAM))),
            (7, "pubkey", json!(pk([90; 32]))),
            (9, "isWritable", json!(true)),
        ] {
            let mut bad = valid.clone();
            bad["swapInstruction"]["accounts"][index][field] = value;
            assert!(decode_bid(&p, &serde_json::to_vec(&bad).unwrap()).is_err());
        }
        let mut bad = valid;
        let mut data = STANDARD
            .decode(bad["swapInstruction"]["data"].as_str().unwrap())
            .unwrap();
        data[26] = 1; // platform fee in decoded header, JSON alone cannot hide it
        bad["swapInstruction"]["data"] = json!(STANDARD.encode(data));
        assert!(decode_bid(&p, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
    #[test]
    fn duplicate_required_fields_and_unbounded_bodies_are_unknown_not_prices() {
        let p = plan();
        let bytes = serde_json::to_string(&bid_body(&p)).unwrap();
        let duplicate = format!("{{\"inAmount\":\"1000\",{}", &bytes[1..]);
        assert_eq!(
            decode_bid(&p, duplicate.as_bytes()),
            Err(ReferenceError::Decode)
        );
        assert_eq!(
            decode_bid(&p, &vec![0; p.search.max_response_bytes + 1]),
            Err(ReferenceError::BodyLimit)
        );
    }
    #[test]
    fn two_sided_estimates_apply_reviewed_buffers_midpoint_and_are_not_executable_proofs() {
        let p = plan();
        p.validate().unwrap();
        let ask = ask(&p);
        let result = evaluate(
            &p,
            100,
            b"real retained bid body fixture",
            &ask,
            window(),
            2,
        )
        .unwrap();
        assert_eq!(result.bid_lamports, 100);
        assert_eq!(result.ask_lamports, 110);
        assert_eq!(result.marks.lower(), 99);
        assert_eq!(result.marks.upper(), 113);
        assert_eq!(result.marks.central(), 106);
        assert!(!ask.executable_minimum_verified());
        assert_ne!(result.digest, [0; 32]);
        assert!(matches!(
            evaluate(&p, 111, b"bid", &ask, window(), 2),
            Err(ReferenceError::Crossed)
        ));
        assert!(matches!(
            evaluate(&p, 50, b"bid", &ask, window(), 2),
            Err(ReferenceError::Spread)
        ));
        assert!(evaluate(&p, 100, b"bid", &ask, window(), 5_000).is_err());
    }
    #[test]
    fn full_policy_identity_source_semantics_and_history_settings_are_committed() {
        let p = plan();
        p.validate().unwrap();
        let mut altered = p.clone();
        altered.reference.bid_haircut_bps += 1;
        assert_ne!(p.policy_commitment(), altered.policy_commitment());
        assert!(altered.validate().is_err());
        bind(&mut altered);
        altered.validate().unwrap();
        altered.reference.api_market_state_age_is_trusted = false;
        bind(&mut altered);
        assert_eq!(altered.validate(), Err(ReferenceError::Policy));
        let mut altered = p.clone();
        altered.ask_inputs.push(SolLamports(120));
        bind(&mut altered);
        assert!(altered.validate().is_err()); // no hidden unbudgeted candidate list
        let old = p.configuration;
        let mut hist = old.policy();
        hist.max_age_seconds += 1;
        let mut altered = p.clone();
        altered.configuration = Configuration::new(
            old.domain(),
            old.quantity(),
            old.model(),
            old.epoch(),
            old.publishers(),
            hist,
        )
        .unwrap();
        assert_ne!(p.policy_commitment(), altered.policy_commitment());
        let mut wrong = p.clone();
        let mut domain = old.domain();
        domain.base_token_program = [99; 32];
        wrong.configuration = Configuration::new(
            domain,
            old.quantity(),
            old.model(),
            old.epoch(),
            old.publishers(),
            old.policy(),
        )
        .unwrap();
        bind(&mut wrong);
        assert!(wrong.validate().is_err());
    }
    #[test]
    fn clock_regression_overwide_or_cross_epoch_observation_is_not_accepted() {
        let p = plan();
        let ask = ask(&p);
        for w in [
            ObservationWindow {
                end: RuntimeClock {
                    slot: 99,
                    ..window().end
                },
                ..window()
            },
            ObservationWindow {
                end: RuntimeClock {
                    unix_timestamp: 19,
                    ..window().end
                },
                ..window()
            },
            ObservationWindow {
                end: RuntimeClock {
                    slot: 121,
                    ..window().end
                },
                ..window()
            },
            ObservationWindow {
                end: RuntimeClock {
                    unix_timestamp: 30,
                    ..window().end
                },
                ..window()
            },
        ] {
            assert!(matches!(
                evaluate(&p, 100, b"bid", &ask, w, 2),
                Err(ReferenceError::LocalWindow)
            ));
        }
    }
    #[test]
    fn quantity_specific_producer_commitments_do_not_break_global_financial_model_coherence() {
        let first = plan();
        let old = first.configuration;
        let mut second = first.clone();
        second.configuration = Configuration::new(
            old.domain(),
            2_000,
            old.model(),
            old.epoch(),
            old.publishers(),
            old.policy(),
        )
        .unwrap();
        assert_eq!(first.configuration.model(), second.configuration.model());
        assert_ne!(first.policy_commitment(), second.policy_commitment());
        assert!(second.validate().is_err()); // old per-feed pin does not cover this q
        bind(&mut second);
        second.validate().unwrap();
        assert_eq!(
            first.configuration.model().policy_hash,
            second.configuration.model().policy_hash
        );
        let mut third = first.clone();
        let mut model = old.model();
        model.policy_hash = [91; 32];
        third.configuration = Configuration::new(
            old.domain(),
            old.quantity(),
            model,
            old.epoch(),
            old.publishers(),
            old.policy(),
        )
        .unwrap();
        assert_ne!(first.policy_commitment(), third.policy_commitment());
        assert!(third.validate().is_err());
    }

    #[cfg(target_os = "linux")]
    mod transport {
        use super::*;
        use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        struct Directory(PathBuf);
        impl Directory {
            fn new() -> Self {
                let nonce = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let path = std::env::temp_dir()
                    .join(format!("shortwin-bid-http-{}-{nonce}", std::process::id()));
                fs::create_dir(&path).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                Self(path)
            }
        }
        impl Drop for Directory {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        fn source(endpoint: Url, directory: &Directory) -> TrustedJupiterSource {
            assert_eq!(endpoint.host_str(), Some("127.0.0.1"));
            let budget = Arc::new(
                SharedRequestBudget::new(crate::price_budget::BudgetConfig {
                    state_file: directory.0.join("budget"),
                    limits: crate::price_budget::BudgetLimits {
                        total_per_second: 10,
                        background_per_second: 6,
                        reserved_exit_per_second: 3,
                    },
                })
                .unwrap(),
            );
            let mut source = TrustedJupiterSource::new("dummy-never-live-secret", budget).unwrap();
            source.client = Client::builder()
                .no_proxy()
                .redirect(redirect::Policy::none())
                .retry(reqwest::retry::never())
                .build()
                .unwrap();
            source.endpoint = endpoint;
            source
        }
        async fn server(parts: Vec<(u64, Vec<u8>)>) -> (Url, tokio::task::JoinHandle<Vec<u8>>) {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = Url::parse(&format!(
                "http://{}/swap/v2/build",
                listener.local_addr().unwrap()
            ))
            .unwrap();
            let task = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = stream.read(&mut buffer).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..n]);
                    assert!(request.len() < 8192);
                }
                for (ms, bytes) in parts {
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    if stream.write_all(&bytes).await.is_err() {
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
        async fn bid(
            source: &TrustedJupiterSource,
            plan: &TrustedJupiterPlan,
        ) -> Result<(u64, Vec<u8>, OwnedSemaphorePermit), ReferenceError> {
            for _ in 0..100 {
                let permit = source.active.clone().try_acquire_owned().unwrap();
                match source.bid(plan, permit).await {
                    Err(ReferenceError::Budget) => {
                        tokio::time::sleep(Duration::from_millis(1)).await
                    }
                    r => return r,
                }
            }
            Err(ReferenceError::Budget)
        }
        #[tokio::test]
        async fn bid_http_uses_reverse_direction_exact_q_fixed_policy_and_retains_same_body() {
            let directory = Directory::new();
            let plan = plan();
            let body = serde_json::to_vec(&bid_body(&plan)).unwrap();
            let (endpoint, task) = server(vec![(0, response("200 OK", &body))]).await;
            let source = source(endpoint, &directory);
            assert!(source.api_key.is_sensitive());
            assert!(!format!("{:?}", source.api_key).contains("dummy-never-live-secret"));
            let (value, retained, _permit) = bid(&source, &plan).await.unwrap();
            assert_eq!(value, 100);
            assert_eq!(retained, body);
            let request = String::from_utf8(task.await.unwrap()).unwrap();
            assert!(request.contains(&format!(
                "inputMint={}",
                pk(plan.configuration.domain().base_mint)
            )));
            assert!(request.contains(&format!("outputMint={}", pk(WSOL_MINT))));
            assert!(request.contains("amount=1000"));
            assert!(request.contains("platformFeeBps=0"));
            assert!(request.contains("wrapAndUnwrapSol=false"));
            assert!(
                request
                    .to_lowercase()
                    .contains("x-api-key: dummy-never-live-secret")
            );
            assert!(source.active.clone().try_acquire_owned().is_err()); // guard survives bid through ask phase
        }
        #[tokio::test]
        async fn redirects_oversized_and_slow_bodies_fail_without_becoming_financial_marks() {
            let cases=vec![
                (vec![(0,b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/steal\r\nContent-Length: 0\r\n\r\n".to_vec())],ReferenceError::HttpStatus),
                (vec![(0,b"HTTP/1.1 200 OK\r\nContent-Length: 9999999\r\n\r\n".to_vec())],ReferenceError::BodyLimit),
                (vec![(0,b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n".to_vec()),(200,b"slow bytes".to_vec())],ReferenceError::Transport),
            ];
            for (parts, expected) in cases {
                let directory = Directory::new();
                let (endpoint, task) = server(parts).await;
                let source = source(endpoint, &directory);
                let mut plan = plan();
                plan.search.request_timeout_ms = 50;
                assert!(matches!(bid(&source,&plan).await,Err(error)if error==expected));
                task.await.unwrap();
            }
        }
    }
}
