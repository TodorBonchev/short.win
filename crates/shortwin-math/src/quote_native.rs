//! Parameterized quote-native financial arithmetic. No runtime handler uses this module.
//!
//! Inputs are **already accepted contractual reference bands**, not raw API quotes or
//! proofs of globally cheapest acquisition. Construction checks numerical coherence;
//! it does not authenticate an operator, a simulation, a mint, or a policy. The adapter
//! must authenticate the complete report domain and policy, bind quantities and model
//! versions, enforce accepted report-core cadence/taint rules, and supply the actual
//! runtime clock. No legacy undefined-as-infinity or synthetic reserve semantics apply.
//!
//! There are no production defaults, maturity foreclosure, S2/S3, payout, or instruction
//! authorization decisions here. See `docs/design/jupiter/QUOTE_NATIVE_FINANCIAL_CONTRACT.md`.

use crate::{MathError, U256, caps, num::BPS, risk};

/// Bounded numerical history storage; this is not an approved pilot lot count.
pub const MAX_HISTORY: usize = 8;
/// Bounded supported-lot input storage; admission policy must choose its own list.
pub const MAX_LOTS: usize = 8;

/// Local refusals, without changing the stable program-facing `MathError` codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidKey,
    InvalidMarks,
    InvalidPolicy,
    InvalidClock,
    InvalidWindow,
    KeyMismatch,
    QuantityMismatch,
    FutureObservation,
    StaleObservation,
    WindowTooWide,
    HistoryLength,
    HistoryEpoch,
    HistoryOrder,
    Unknown,
    NoRoute,
    ClockMismatch,
    IncoherentReferences,
    InvalidTerms,
    UnsupportedLot,
    PositionQuantityCap,
    MarketQuantityCap,
    WalletQuantityCap,
    Inventory,
    Utilization,
    CapacityCoverage,
    PositionPremium,
    CapacityPremium,
    MinNotional,
    PositionNotional,
    MarketNotional,
    InvalidPersistence,
    InvalidBudget,
    Arithmetic(MathError),
}

impl From<MathError> for Error {
    fn from(value: MathError) -> Self {
        Self::Arithmetic(value)
    }
}

type Result<T> = core::result::Result<T, Error>;

/// Opaque digests must canonically commit to the complete authenticated domain and
/// pricing policy. Their construction and authenticity are caller obligations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceKey {
    domain_digest: [u8; 32],
    policy_digest: [u8; 32],
    model_version: u32,
    model_epoch: u64,
    quantity: u64,
}

impl ReferenceKey {
    pub fn new(
        domain_digest: [u8; 32],
        policy_digest: [u8; 32],
        model_version: u32,
        model_epoch: u64,
        quantity: u64,
    ) -> Result<Self> {
        if domain_digest == [0; 32]
            || policy_digest == [0; 32]
            || model_version == 0
            || model_epoch == 0
            || quantity == 0
        {
            return Err(Error::InvalidKey);
        }
        Ok(Self {
            domain_digest,
            policy_digest,
            model_version,
            model_epoch,
            quantity,
        })
    }

    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    fn same_model(self, other: Self) -> bool {
        self.domain_digest == other.domain_digest
            && self.policy_digest == other.policy_digest
            && self.model_version == other.model_version
            && self.model_epoch == other.model_epoch
    }
}

/// Positive ordered accepted lower/central/upper marks, all in lamports for exact q.
/// A historical bucket uses lower_min / central_last / upper_max in these fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marks {
    lower: u64,
    central: u64,
    upper: u64,
}

impl Marks {
    pub fn new(lower: u64, central: u64, upper: u64) -> Result<Self> {
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

/// All nonfinite statuses refuse these financial helpers. NoRoute does not imply
/// infinite price, cash-settlement eligibility, or fair value; it is included only
/// to make a future adapter's refusal explicit. The initial report core has Unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Finite(Marks),
    Unknown,
    AttestedNoRoute,
}

impl Status {
    fn finite(self) -> Result<Marks> {
        match self {
            Self::Finite(marks) => Ok(marks),
            Self::Unknown => Err(Error::Unknown),
            Self::AttestedNoRoute => Err(Error::NoRoute),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    pub slot: u64,
    pub unix_timestamp: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub start: Clock,
    pub end: Clock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveReference {
    pub key: ReferenceKey,
    pub epoch: u64,
    pub source: Window,
    pub status: Status,
}

/// Only completed, timely, untainted report-core buckets may be adapted here.
/// First/last/observations alone cannot prove all intra-bucket sample gaps or that
/// Unknown never occurred: the authenticated report-state core must enforce that.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletedReference {
    pub key: ReferenceKey,
    pub epoch: u64,
    pub first_observed: Clock,
    pub last_observed: Clock,
    pub observations: u64,
    pub status: Status,
}

/// Explicit evidence policy, with no production default. All bounds are inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvidencePolicy {
    pub epoch_seconds: u64,
    pub history_length: usize,
    pub max_age_seconds: u64,
    pub max_age_slots: u64,
    pub max_source_window_seconds: u64,
    pub max_source_window_slots: u64,
}

impl EvidencePolicy {
    fn validate(self) -> Result<()> {
        if self.epoch_seconds == 0 || self.history_length == 0 || self.history_length > MAX_HISTORY
        {
            return Err(Error::InvalidPolicy);
        }
        Ok(())
    }
}

fn timestamp(clock: Clock) -> Result<u64> {
    u64::try_from(clock.unix_timestamp).map_err(|_| Error::InvalidClock)
}

fn epoch_at(clock: Clock, policy: EvidencePolicy) -> Result<u64> {
    timestamp(clock)?
        .checked_div(policy.epoch_seconds)
        .ok_or(Error::InvalidPolicy)
}

fn ordered_window(window: Window) -> Result<(u64, u64)> {
    let start = timestamp(window.start)?;
    let end = timestamp(window.end)?;
    if window.start.slot > window.end.slot || start > end {
        return Err(Error::InvalidWindow);
    }
    Ok((start, end))
}

/// History trust is explicit and cannot be mixed inside one admission decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryMode {
    /// Earlier adapter: observations and publication must share the runtime epoch.
    RuntimeEpoch,
    /// Operator attests private completed history; freshness is checked at use time.
    TrustedOffchain,
}

/// Immutable summary checked at one actual runtime clock. Every financial helper
/// rechecks that exact clock, preventing reuse of cached eligibility at a later clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    history_mode: HistoryMode,
    key: ReferenceKey,
    epoch: u64,
    checked_at: Clock,
    policy: EvidencePolicy,
    live_source: Window,
    live: Marks,
    history_lower: [u64; MAX_HISTORY],
    opening_upper: u64,
    lower_base: u64,
    acquisition_reference: u64,
}

impl Summary {
    pub const fn history_mode(self) -> HistoryMode {
        self.history_mode
    }
    pub const fn key(self) -> ReferenceKey {
        self.key
    }
    pub const fn epoch(self) -> u64 {
        self.epoch
    }
    pub const fn opening_upper(self) -> u64 {
        self.opening_upper
    }
    pub const fn lower_base(self) -> u64 {
        self.lower_base
    }
    /// min(live ask U, lower median of completed bucket ask maxima), never bid L.
    pub const fn acquisition_reference(self) -> u64 {
        self.acquisition_reference
    }

    fn check(self, quantity: u64, now: Clock) -> Result<()> {
        if quantity == 0 || self.key.quantity != quantity {
            return Err(Error::QuantityMismatch);
        }
        if self.checked_at != now {
            return Err(Error::ClockMismatch);
        }
        Ok(())
    }
}

/// Summarize exactly H completed buckets, newest first, excluding the current epoch.
/// Authentication, accepted band methodology, full source cadence and taint handling
/// remain mandatory caller obligations; this validates only the represented evidence.
pub fn summarize(
    expected: ReferenceKey,
    live: LiveReference,
    history_newest_first: &[CompletedReference],
    policy: EvidencePolicy,
    now: Clock,
) -> Result<Summary> {
    summarize_with_mode(
        expected,
        live,
        history_newest_first,
        policy,
        now,
        HistoryMode::RuntimeEpoch,
    )
}

/// Distinct on-demand mode: history is attested by the configured trusted operator.
/// It ends before the live SOURCE epoch, which may precede the runtime epoch while
/// the observation remains fresh. No claim of public historical timing follows.
/// The caller must authenticate the new trusted snapshot account/codec, not adapt
/// arbitrary user history or reinterpret the earlier public-history account.
pub fn summarize_trusted_history(
    expected: ReferenceKey,
    live: LiveReference,
    history_newest_first: &[CompletedReference],
    policy: EvidencePolicy,
    now: Clock,
) -> Result<Summary> {
    summarize_with_mode(
        expected,
        live,
        history_newest_first,
        policy,
        now,
        HistoryMode::TrustedOffchain,
    )
}

fn summarize_with_mode(
    expected: ReferenceKey,
    live: LiveReference,
    history_newest_first: &[CompletedReference],
    policy: EvidencePolicy,
    now: Clock,
    history_mode: HistoryMode,
) -> Result<Summary> {
    policy.validate()?;
    if live.key != expected {
        return Err(Error::KeyMismatch);
    }
    if history_newest_first.len() != policy.history_length {
        return Err(Error::HistoryLength);
    }
    let current_epoch = match history_mode {
        HistoryMode::RuntimeEpoch => epoch_at(now, policy)?,
        HistoryMode::TrustedOffchain => epoch_at(live.source.end, policy)?,
    };
    let (start, end) = ordered_window(live.source)?;
    let now_time = timestamp(now)?;
    if live.source.end.slot > now.slot || end > now_time {
        return Err(Error::FutureObservation);
    }
    if live.epoch != current_epoch
        || epoch_at(live.source.start, policy)? != current_epoch
        || epoch_at(live.source.end, policy)? != current_epoch
    {
        return Err(Error::HistoryEpoch);
    }
    let age_seconds = now_time
        .checked_sub(start)
        .ok_or(Error::FutureObservation)?;
    let age_slots = now
        .slot
        .checked_sub(live.source.start.slot)
        .ok_or(Error::FutureObservation)?;
    if age_seconds > policy.max_age_seconds || age_slots > policy.max_age_slots {
        return Err(Error::StaleObservation);
    }
    if end.checked_sub(start).ok_or(Error::InvalidWindow)? > policy.max_source_window_seconds
        || live
            .source
            .end
            .slot
            .checked_sub(live.source.start.slot)
            .ok_or(Error::InvalidWindow)?
            > policy.max_source_window_slots
    {
        return Err(Error::WindowTooWide);
    }
    let marks = live.status.finite()?;
    let mut lower = [0; MAX_HISTORY];
    let mut upper = [0; MAX_HISTORY];
    let mut opening_upper = marks.upper;
    let mut lower_base = marks.lower;
    let mut next_epoch = current_epoch;
    let mut newer_start_slot = live.source.start.slot;
    for (i, bucket) in history_newest_first.iter().enumerate() {
        if bucket.key != expected {
            return Err(Error::KeyMismatch);
        }
        next_epoch = next_epoch.checked_sub(1).ok_or(Error::HistoryEpoch)?;
        if bucket.epoch != next_epoch
            || epoch_at(bucket.first_observed, policy)? != next_epoch
            || epoch_at(bucket.last_observed, policy)? != next_epoch
        {
            return Err(Error::HistoryEpoch);
        }
        ordered_window(Window {
            start: bucket.first_observed,
            end: bucket.last_observed,
        })?;
        if bucket.observations == 0 || bucket.last_observed.slot >= newer_start_slot {
            return Err(Error::HistoryOrder);
        }
        newer_start_slot = bucket.first_observed.slot;
        let values = bucket.status.finite()?;
        lower[i] = values.lower;
        upper[i] = values.upper;
        opening_upper = opening_upper.max(values.upper);
        lower_base = lower_base.min(values.lower);
    }
    // At most eight elements: no allocation, and ties retain the exact lower median.
    let used = upper
        .get_mut(..policy.history_length)
        .ok_or(Error::HistoryLength)?;
    used.sort_unstable();
    let median_index = policy
        .history_length
        .checked_sub(1)
        .ok_or(Error::HistoryLength)?
        / 2;
    let median = *used.get(median_index).ok_or(Error::HistoryLength)?;
    Ok(Summary {
        history_mode,
        key: expected,
        epoch: current_epoch,
        checked_at: now,
        policy,
        live_source: live.source,
        live: marks,
        history_lower: lower,
        opening_upper,
        lower_base,
        acquisition_reference: marks.upper.min(median),
    })
}

/// Limits on the combined source windows of position, small-lot bid and capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoherenceLimits {
    pub max_seconds: u64,
    pub max_slots: u64,
}

fn coherent(summaries: &[Summary], limits: CoherenceLimits, now: Clock) -> Result<()> {
    let first = summaries.first().ok_or(Error::IncoherentReferences)?;
    let mut start_slot = first.live_source.start.slot;
    let mut end_slot = first.live_source.end.slot;
    let mut start_time = timestamp(first.live_source.start)?;
    let mut end_time = timestamp(first.live_source.end)?;
    for summary in summaries {
        summary.check(summary.key.quantity, now)?;
        if !summary.key.same_model(first.key)
            || summary.history_mode != first.history_mode
            || summary.policy != first.policy
            || summary.epoch != first.epoch
        {
            return Err(Error::IncoherentReferences);
        }
        start_slot = start_slot.min(summary.live_source.start.slot);
        end_slot = end_slot.max(summary.live_source.end.slot);
        start_time = start_time.min(timestamp(summary.live_source.start)?);
        end_time = end_time.max(timestamp(summary.live_source.end)?);
    }
    if end_slot
        .checked_sub(start_slot)
        .ok_or(Error::InvalidWindow)?
        > limits.max_slots
        || end_time
            .checked_sub(start_time)
            .ok_or(Error::InvalidWindow)?
            > limits.max_seconds
    {
        return Err(Error::IncoherentReferences);
    }
    Ok(())
}

fn product3(a: u64, b: u64, c: u64) -> Result<U256> {
    U256::from_u64(a)
        .checked_mul(U256::from_u64(b))
        .and_then(|p| p.checked_mul(U256::from_u64(c)))
        .ok_or(Error::Arithmetic(MathError::Overflow))
}

/// New all-in fee/spread premium: U(q)*q_ref*10000 <= q*L_ref*(10000+premium).
/// Multi-u64 products are compared in U256, without reducing or assuming fee-free XY.
pub fn all_in_premium_ok(
    upper_cost: u64,
    quantity: u64,
    reference_bid: u64,
    reference_quantity: u64,
    premium_bps: u32,
) -> Result<bool> {
    if upper_cost == 0 || quantity == 0 || reference_bid == 0 || reference_quantity == 0 {
        return Err(Error::InvalidMarks);
    }
    let factor = BPS
        .checked_add(u64::from(premium_bps))
        .ok_or(MathError::Overflow)?;
    Ok(
        product3(upper_cost, reference_quantity, BPS)?
            <= product3(quantity, reference_bid, factor)?,
    )
}

/// Explicit governed opening constraints. Zero utilization/wallet share disables opens;
/// raw quantity and notional ceilings must be positive and finite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmissionTerms<'a> {
    pub supported_position_lots: &'a [u64],
    pub small_lot_quantity: u64,
    pub capacity_quantity: u64,
    pub position_quantity_max: u64,
    pub market_quantity_max: u64,
    pub utilization_bps: u32,
    pub wallet_bps: u32,
    pub position_premium_bps: u32,
    pub capacity_premium_bps: u32,
    pub min_notional: u64,
    pub position_notional_max: u64,
    pub market_notional_max: u64,
    pub cr_open_bps: u32,
    pub open_fee_bps: u32,
}

impl AdmissionTerms<'_> {
    fn validate(self) -> Result<()> {
        if self.supported_position_lots.is_empty()
            || self.supported_position_lots.len() > MAX_LOTS
            || self.small_lot_quantity == 0
            || self.capacity_quantity < self.market_quantity_max
            || self.position_quantity_max == 0
            || self.market_quantity_max == 0
            || self.position_quantity_max > self.market_quantity_max
            || u64::from(self.utilization_bps) > BPS
            || u64::from(self.wallet_bps) > BPS
            || self.min_notional == 0
            || self.position_notional_max < self.min_notional
            || self.market_notional_max < self.position_notional_max
            || u64::from(self.cr_open_bps) < BPS
            || u64::from(self.open_fee_bps) > BPS
        {
            return Err(Error::InvalidTerms);
        }
        let mut previous = 0;
        for lot in self.supported_position_lots {
            if *lot <= previous
                || *lot < self.small_lot_quantity
                || *lot > self.position_quantity_max
            {
                return Err(Error::InvalidTerms);
            }
            previous = *lot;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenInputs {
    pub quantity: u64,
    pub tokens_free: u64,
    pub borrowed_quantity: u64,
    pub wallet_quantity: u64,
    pub booked_notional: u64,
    /// Actual accepted net sale proceeds; the caller measures vault deltas.
    pub actual_sale: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenAmounts {
    pub quantity_after: u64,
    pub wallet_quantity_after: u64,
    pub capacity_covered_quantity: u64,
    pub booked_notional_after: u64,
    pub opening_notional: u64,
    pub margin: u64,
    pub collateral: u64,
    pub open_fee: u64,
    /// Margin plus the open fee, funded by the borrower outside actual sale proceeds.
    pub borrower_funding: u64,
}

/// Complete numerical opening check on exact supported q, an exact small-lot bid,
/// and an upper-only capacity acquisition at q_capacity >= total q after opening.
/// Surplus from that capacity acquisition must be safely returned to lenders by the
/// executor; it supplies no interpolated lower reference for an individual position.
/// This does not authorize a route, sale, report, account, transfer, or market admission.
pub fn check_open(
    position: Summary,
    small_lot: Summary,
    capacity: Summary,
    terms: AdmissionTerms<'_>,
    inputs: OpenInputs,
    limits: CoherenceLimits,
    now: Clock,
) -> Result<OpenAmounts> {
    terms.validate()?;
    position.check(inputs.quantity, now)?;
    coherent(&[position, small_lot, capacity], limits, now)?;
    if small_lot.key.quantity != terms.small_lot_quantity
        || capacity.key.quantity != terms.capacity_quantity
    {
        return Err(Error::QuantityMismatch);
    }
    if !terms.supported_position_lots.contains(&inputs.quantity) {
        return Err(Error::UnsupportedLot);
    }
    if inputs.quantity > terms.position_quantity_max {
        return Err(Error::PositionQuantityCap);
    }
    if inputs.quantity > inputs.tokens_free {
        return Err(Error::Inventory);
    }
    let quantity_after = inputs
        .borrowed_quantity
        .checked_add(inputs.quantity)
        .ok_or(MathError::Overflow)?;
    let wallet_quantity_after = inputs
        .wallet_quantity
        .checked_add(inputs.quantity)
        .ok_or(MathError::Overflow)?;
    if quantity_after > terms.market_quantity_max {
        return Err(Error::MarketQuantityCap);
    }
    if !caps::wallet_ok(
        inputs.wallet_quantity,
        inputs.quantity,
        terms.market_quantity_max,
        terms.wallet_bps,
    )? {
        return Err(Error::WalletQuantityCap);
    }
    if !caps::util_ok(
        inputs.tokens_free,
        inputs.borrowed_quantity,
        inputs.quantity,
        terms.utilization_bps,
    )? {
        return Err(Error::Utilization);
    }
    if capacity.key.quantity < quantity_after {
        return Err(Error::CapacityCoverage);
    }
    if !all_in_premium_ok(
        position.opening_upper,
        inputs.quantity,
        small_lot.lower_base,
        small_lot.key.quantity,
        terms.position_premium_bps,
    )? {
        return Err(Error::PositionPremium);
    }
    if !all_in_premium_ok(
        capacity.opening_upper,
        capacity.key.quantity,
        small_lot.lower_base,
        small_lot.key.quantity,
        terms.capacity_premium_bps,
    )? {
        return Err(Error::CapacityPremium);
    }
    let opening_notional = position.opening_upper.max(inputs.actual_sale);
    if opening_notional < terms.min_notional {
        return Err(Error::MinNotional);
    }
    if opening_notional > terms.position_notional_max {
        return Err(Error::PositionNotional);
    }
    let booked_notional_after = inputs
        .booked_notional
        .checked_add(opening_notional)
        .ok_or(MathError::Overflow)?;
    if !caps::notional_ok(
        opening_notional,
        inputs.booked_notional,
        terms.min_notional,
        terms.market_notional_max,
    ) {
        return Err(Error::MarketNotional);
    }
    let margin = risk::margin_d4(
        inputs.actual_sale,
        opening_notional,
        Some(opening_notional),
        terms.cr_open_bps,
    )?;
    let collateral = margin
        .checked_add(inputs.actual_sale)
        .ok_or(MathError::Overflow)?;
    let open_fee = risk::open_fee(opening_notional, terms.open_fee_bps)?;
    let borrower_funding = margin.checked_add(open_fee).ok_or(MathError::Overflow)?;
    Ok(OpenAmounts {
        quantity_after,
        wallet_quantity_after,
        capacity_covered_quantity: capacity.key.quantity,
        booked_notional_after,
        opening_notional,
        margin,
        collateral,
        open_fee,
        borrower_funding,
    })
}

/// Strict CR eligibility: live bid L and each of the newest n completed lower minima
/// must fail V_eff*10000 >= threshold*L. This is one eligibility leg, not permission
/// to execute or pay a keeper. Current epoch samples never enter historical n.
pub fn cr_liquidatable(
    quantity: u64,
    evidence: Summary,
    now: Clock,
    vault: u64,
    rent_due: u64,
    threshold_bps: u32,
    completed_count: usize,
) -> Result<bool> {
    evidence.check(quantity, now)?;
    if threshold_bps == 0
        || completed_count == 0
        || completed_count > evidence.policy.history_length
    {
        return Err(Error::InvalidPersistence);
    }
    let effective = risk::v_eff(vault, rent_due);
    if !risk::cr_below(effective, Some(evidence.live.lower), threshold_bps)? {
        return Ok(false);
    }
    for lower in evidence.history_lower.iter().take(completed_count) {
        if !risk::cr_below(effective, Some(*lower), threshold_bps)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// For a nonexpired CR liquidation, a measured whole-q acquisition that is healthy
/// at the threshold refutes the liquidation and the entire transaction must roll back.
/// Zero actual cost is healthy. The caller must prove actual full-q token delivery,
/// source/destination ownership and measured net acquisition cost, and supply real expiry.
pub fn actual_fill_refutes_cr(
    expired: bool,
    vault: u64,
    rent_due: u64,
    actual_cost: u64,
    threshold_bps: u32,
) -> Result<bool> {
    if threshold_bps == 0 {
        return Err(Error::InvalidTerms);
    }
    if expired {
        return Ok(false);
    }
    if actual_cost == 0 {
        return Ok(true);
    }
    Ok(!risk::cr_below(
        risk::v_eff(vault, rent_due),
        Some(actual_cost),
        threshold_bps,
    )?)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcquisitionBudget {
    pub reference: u64,
    pub max_input: u64,
}

/// floor(B_ref*(10000+delta)/10000), limited by V-K_s-net_rent. An unavailable
/// budget returns None; it never implies a cash settlement. Refundable scratch rent
/// is excluded from net rent only if the executor separately proves peak funding/refund.
pub fn acquisition_budget(
    quantity: u64,
    evidence: Summary,
    now: Clock,
    delta_bps: u32,
    budget: risk::Budget,
) -> Result<Option<AcquisitionBudget>> {
    evidence.check(quantity, now)?;
    let room = budget
        .v
        .checked_sub(budget.k_s)
        .ok_or(Error::InvalidBudget)?;
    let Some(available) = room.checked_sub(budget.ert) else {
        return Ok(None);
    };
    let factor = BPS
        .checked_add(u64::from(delta_bps))
        .ok_or(MathError::Overflow)?;
    let widened = u128::from(evidence.acquisition_reference)
        .checked_mul(u128::from(factor))
        .ok_or(MathError::Overflow)?
        .checked_div(u128::from(BPS))
        .ok_or(MathError::DivByZero)?;
    let max_input =
        u64::try_from(widened.min(u128::from(available))).map_err(|_| MathError::Overflow)?;
    if max_input == 0 {
        return Ok(None);
    }
    Ok(Some(AcquisitionBudget {
        reference: evidence.acquisition_reference,
        max_input,
    }))
}

/// Finite S1 gap leg only: strict live L and all H completed lower minima exceed
/// V-K_s after net rent. Equality is affordable. The caller must additionally prove
/// normal liquidation/expiry eligibility and distinct completed-epoch persistence,
/// reset the arm on gaps/margin changes/Unknown, and recheck all payout/loss rules.
/// This intentionally implements no S2/S3 or overdue foreclosure.
pub fn finite_s1_gap(
    quantity: u64,
    evidence: Summary,
    now: Clock,
    budget: risk::Budget,
) -> Result<bool> {
    evidence.check(quantity, now)?;
    if budget.k_s > budget.v {
        return Err(Error::InvalidBudget);
    }
    if !budget.exceeded_by(Some(evidence.live.lower))? {
        return Ok(false);
    }
    for lower in evidence
        .history_lower
        .iter()
        .take(evidence.policy.history_length)
    {
        if !budget.exceeded_by(Some(*lower))? {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use std::vec::Vec;

    const NOW: Clock = Clock {
        slot: 1_050,
        unix_timestamp: 105,
    };
    const LIMITS: CoherenceLimits = CoherenceLimits {
        max_seconds: 2,
        max_slots: 20,
    };

    fn key(q: u64) -> ReferenceKey {
        ReferenceKey::new([1; 32], [2; 32], 1, 1, q).unwrap()
    }
    fn marks(l: u64, u: u64) -> Marks {
        Marks::new(l, l + (u - l) / 2, u).unwrap()
    }
    fn policy(h: usize) -> EvidencePolicy {
        EvidencePolicy {
            epoch_seconds: 10,
            history_length: h,
            max_age_seconds: 4,
            max_age_slots: 40,
            max_source_window_seconds: 2,
            max_source_window_slots: 20,
        }
    }
    fn raw(
        q: u64,
        live_lower: u64,
        live_upper: u64,
        history: &[(u64, u64)],
    ) -> (LiveReference, Vec<CompletedReference>) {
        let live = LiveReference {
            key: key(q),
            epoch: 10,
            source: Window {
                start: Clock {
                    slot: 1_010,
                    unix_timestamp: 101,
                },
                end: Clock {
                    slot: 1_030,
                    unix_timestamp: 103,
                },
            },
            status: Status::Finite(marks(live_lower, live_upper)),
        };
        let buckets = history
            .iter()
            .enumerate()
            .map(|(i, (l, u))| {
                let epoch = 9 - i as u64;
                CompletedReference {
                    key: key(q),
                    epoch,
                    observations: 2,
                    first_observed: Clock {
                        slot: epoch * 100 + 10,
                        unix_timestamp: (epoch * 10 + 1) as i64,
                    },
                    last_observed: Clock {
                        slot: epoch * 100 + 90,
                        unix_timestamp: (epoch * 10 + 9) as i64,
                    },
                    status: Status::Finite(marks(*l, *u)),
                }
            })
            .collect();
        (live, buckets)
    }
    fn evidence(q: u64, lower: u64, upper: u64, history: &[(u64, u64)]) -> Summary {
        let (live, buckets) = raw(q, lower, upper, history);
        summarize(key(q), live, &buckets, policy(history.len()), NOW).unwrap()
    }
    fn flat(q: u64) -> Summary {
        evidence(q, q * 10, q * 10, &[(q * 10, q * 10), (q * 10, q * 10)])
    }
    fn terms<'a>(lots: &'a [u64]) -> AdmissionTerms<'a> {
        AdmissionTerms {
            supported_position_lots: lots,
            small_lot_quantity: 1,
            capacity_quantity: 10,
            position_quantity_max: 4,
            market_quantity_max: 10,
            utilization_bps: 9_000,
            wallet_bps: 5_000,
            position_premium_bps: 0,
            capacity_premium_bps: 0,
            min_notional: 1,
            position_notional_max: 1_000,
            market_notional_max: 10_000,
            cr_open_bps: 40_000,
            open_fee_bps: 25,
        }
    }
    fn inputs(q: u64) -> OpenInputs {
        OpenInputs {
            quantity: q,
            tokens_free: 100,
            borrowed_quantity: 0,
            wallet_quantity: 0,
            booked_notional: 0,
            actual_sale: q * 10,
        }
    }

    #[test]
    fn trusted_history_can_cross_landing_epoch_but_never_freshness() {
        let (live, buckets) = raw(1, 10, 12, &[(9, 13), (8, 14)]);
        let mut p = policy(2);
        p.max_age_seconds = 10;
        p.max_age_slots = 100;
        let now = Clock {
            slot: 1_100,
            unix_timestamp: 110,
        };
        assert_eq!(
            summarize(key(1), live, &buckets, p, now),
            Err(Error::HistoryEpoch)
        );
        let accepted = summarize_trusted_history(key(1), live, &buckets, p, now).unwrap();
        assert_eq!(accepted.epoch(), 10);
        assert_eq!(accepted.history_mode(), HistoryMode::TrustedOffchain);
        assert_eq!(accepted.opening_upper(), 14);
        let stale = Clock {
            slot: 1_111,
            unix_timestamp: 112,
        };
        assert_eq!(
            summarize_trusted_history(key(1), live, &buckets, p, stale),
            Err(Error::StaleObservation)
        );
        let mut future = live;
        future.source.end.unix_timestamp = 111;
        assert_eq!(
            summarize_trusted_history(key(1), future, &buckets, p, now),
            Err(Error::FutureObservation)
        );
    }

    #[test]
    fn admission_cannot_mix_public_and_trusted_history_or_reanchor_old_buckets() {
        let (live, buckets) = raw(1, 10, 12, &[(9, 13), (8, 14)]);
        let public = summarize(key(1), live, &buckets, policy(2), NOW).unwrap();
        let trusted = summarize_trusted_history(key(1), live, &buckets, policy(2), NOW).unwrap();
        assert_eq!(
            coherent(&[public, trusted], LIMITS, NOW),
            Err(Error::IncoherentReferences)
        );
        let mut incorrect = buckets.clone();
        incorrect[0].epoch += 1;
        assert_eq!(
            summarize_trusted_history(key(1), live, &incorrect, policy(2), NOW),
            Err(Error::HistoryEpoch)
        );
    }

    #[test]
    fn positive_ordered_marks_and_key_are_not_authentication() {
        for values in [(0, 0, 0), (1, 0, 2), (2, 1, 3), (1, 3, 2)] {
            assert_eq!(
                Marks::new(values.0, values.1, values.2),
                Err(Error::InvalidMarks)
            );
        }
        assert!(Marks::new(u64::MAX, u64::MAX, u64::MAX).is_ok());
        assert_eq!(
            ReferenceKey::new([0; 32], [2; 32], 1, 1, 1),
            Err(Error::InvalidKey)
        );
        assert_eq!(
            ReferenceKey::new([1; 32], [0; 32], 1, 1, 1),
            Err(Error::InvalidKey)
        );
        assert_eq!(
            ReferenceKey::new([1; 32], [2; 32], 0, 1, 1),
            Err(Error::InvalidKey)
        );
        assert_eq!(
            ReferenceKey::new([1; 32], [2; 32], 1, 0, 1),
            Err(Error::InvalidKey)
        );
        assert_eq!(
            ReferenceKey::new([1; 32], [2; 32], 1, 1, 0),
            Err(Error::InvalidKey)
        );
    }

    #[test]
    fn summary_separates_bid_eligibility_from_ask_acquisition_and_snapshot_max() {
        let s = evidence(5, 50, 100, &[(40, 80), (30, 120), (20, 90), (10, 110)]);
        assert_eq!(s.lower_base(), 10);
        assert_eq!(s.opening_upper(), 120);
        assert_eq!(s.acquisition_reference(), 90); // sorted 80,90,110,120 -> lower median 90.
        assert_eq!(s.key().quantity(), 5);
        assert_eq!(s.epoch(), 10);
        let budget = acquisition_budget(
            5,
            s,
            NOW,
            0,
            risk::Budget {
                v: 200,
                k_s: 0,
                ert: 0,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(budget.max_input, 90); // Never the bid L=10 or live bid 50.
        assert_eq!(
            evidence(5, 1, 70, &[(1, 80), (1, 90)]).acquisition_reference(),
            70
        );
    }

    #[test]
    fn lower_median_matches_independent_sorted_reference_exhaustively() {
        for a in 1..=5 {
            for b in 1..=5 {
                for c in 1..=5 {
                    for d in 1..=5 {
                        let mut ordered = std::vec![a, b, c, d];
                        ordered.sort();
                        let s = evidence(1, 1, 6, &[(1, a), (1, b), (1, c), (1, d)]);
                        assert_eq!(s.acquisition_reference(), ordered[1]);
                        assert_eq!(s.opening_upper(), 6);
                    }
                }
            }
        }
        assert_eq!(
            evidence(1, 1, 9, &[(1, 2), (1, 7), (1, 4)]).acquisition_reference(),
            4
        );
    }

    #[test]
    fn current_epoch_history_wrong_keys_and_nonfinite_evidence_refuse() {
        let (live, history) = raw(1, 10, 12, &[(9, 11), (8, 10)]);
        for status in [Status::Unknown, Status::AttestedNoRoute] {
            let expected = if status == Status::Unknown {
                Error::Unknown
            } else {
                Error::NoRoute
            };
            assert_eq!(
                summarize(
                    key(1),
                    LiveReference { status, ..live },
                    &history,
                    policy(2),
                    NOW
                ),
                Err(expected)
            );
            let mut changed = history.clone();
            changed[1].status = status;
            assert_eq!(
                summarize(key(1), live, &changed, policy(2), NOW),
                Err(expected)
            );
        }
        for replacement in [
            key(2),
            ReferenceKey::new([3; 32], [2; 32], 1, 1, 1).unwrap(),
            ReferenceKey::new([1; 32], [3; 32], 1, 1, 1).unwrap(),
            ReferenceKey::new([1; 32], [2; 32], 2, 1, 1).unwrap(),
            ReferenceKey::new([1; 32], [2; 32], 1, 2, 1).unwrap(),
        ] {
            assert_eq!(
                summarize(
                    key(1),
                    LiveReference {
                        key: replacement,
                        ..live
                    },
                    &history,
                    policy(2),
                    NOW
                ),
                Err(Error::KeyMismatch)
            );
            let mut changed = history.clone();
            changed[0].key = replacement;
            assert_eq!(
                summarize(key(1), live, &changed, policy(2), NOW),
                Err(Error::KeyMismatch)
            );
        }
        let mut changed = history.clone();
        changed[0].epoch = 10;
        assert_eq!(
            summarize(key(1), live, &changed, policy(2), NOW),
            Err(Error::HistoryEpoch)
        );
        let mut changed = history.clone();
        changed.swap(0, 1);
        assert_eq!(
            summarize(key(1), live, &changed, policy(2), NOW),
            Err(Error::HistoryEpoch)
        );
        let mut changed = history.clone();
        changed[1] = changed[0];
        assert_eq!(
            summarize(key(1), live, &changed, policy(2), NOW),
            Err(Error::HistoryEpoch)
        );
        let mut changed = history.clone();
        changed[0].observations = 0;
        assert_eq!(
            summarize(key(1), live, &changed, policy(2), NOW),
            Err(Error::HistoryOrder)
        );
        assert_eq!(
            summarize(key(1), live, &history[..1], policy(2), NOW),
            Err(Error::HistoryLength)
        );
        assert_eq!(
            summarize(key(1), live, &history, policy(0), NOW),
            Err(Error::InvalidPolicy)
        );
        assert_eq!(
            summarize(
                key(1),
                live,
                &history,
                EvidencePolicy {
                    epoch_seconds: 0,
                    ..policy(2)
                },
                NOW
            ),
            Err(Error::InvalidPolicy)
        );
        assert_eq!(
            summarize(key(1), live, &history, policy(9), NOW),
            Err(Error::InvalidPolicy)
        );
    }

    #[test]
    fn live_clock_window_epoch_and_completed_order_boundaries_are_inclusive() {
        let (live, history) = raw(1, 10, 12, &[(9, 11), (8, 10)]);
        assert!(summarize(key(1), live, &history, policy(2), NOW).is_ok()); // age4/window2 exactly.
        assert_eq!(
            summarize(
                key(1),
                live,
                &history,
                policy(2),
                Clock { slot: 1_051, ..NOW }
            ),
            Err(Error::StaleObservation)
        );
        assert_eq!(
            summarize(
                key(1),
                live,
                &history,
                policy(2),
                Clock {
                    unix_timestamp: 106,
                    ..NOW
                }
            ),
            Err(Error::StaleObservation)
        );
        assert_eq!(
            summarize(
                key(1),
                live,
                &history,
                policy(2),
                Clock { slot: 1_029, ..NOW }
            ),
            Err(Error::FutureObservation)
        );
        assert_eq!(
            summarize(
                key(1),
                live,
                &history,
                policy(2),
                Clock {
                    unix_timestamp: -1,
                    ..NOW
                }
            ),
            Err(Error::InvalidClock)
        );
        let mut l = live;
        l.source.end.slot = l.source.start.slot - 1;
        assert_eq!(
            summarize(key(1), l, &history, policy(2), NOW),
            Err(Error::InvalidWindow)
        );
        let mut l = live;
        l.source.start.unix_timestamp = 99;
        assert_eq!(
            summarize(key(1), l, &history, policy(2), NOW),
            Err(Error::HistoryEpoch)
        );
        let mut l = live;
        l.source.start.slot -= 1;
        assert_eq!(
            summarize(
                key(1),
                l,
                &history,
                EvidencePolicy {
                    max_age_slots: 100,
                    ..policy(2)
                },
                NOW
            ),
            Err(Error::WindowTooWide)
        );
        let mut h = history.clone();
        h[0].last_observed.slot = live.source.start.slot;
        assert_eq!(
            summarize(key(1), live, &h, policy(2), NOW),
            Err(Error::HistoryOrder)
        );
        let mut h = history.clone();
        h[0].first_observed.unix_timestamp = 89;
        assert_eq!(
            summarize(key(1), live, &h, policy(2), NOW),
            Err(Error::HistoryEpoch)
        );
        let mut h = history.clone();
        h[1].last_observed.slot = h[0].first_observed.slot;
        assert_eq!(
            summarize(key(1), live, &h, policy(2), NOW),
            Err(Error::HistoryOrder)
        );
        let s = summarize(key(1), live, &history, policy(2), NOW).unwrap();
        assert_eq!(
            cr_liquidatable(1, s, Clock { slot: 1_051, ..NOW }, 1, 0, 10_000, 1),
            Err(Error::ClockMismatch)
        );
        assert_eq!(
            finite_s1_gap(
                2,
                s,
                NOW,
                risk::Budget {
                    v: 1,
                    k_s: 0,
                    ert: 0
                }
            ),
            Err(Error::QuantityMismatch)
        );
    }

    #[test]
    fn premium_matches_independent_u128_reference_on_small_spaces() {
        for cost in 1..=24u64 {
            for q in 1..=8u64 {
                for bid in 1..=8u64 {
                    for q_ref in 1..=8u64 {
                        for premium in [0u32, 1, 300, 10_000, 50_000] {
                            let expected = u128::from(cost) * u128::from(q_ref) * 10_000
                                <= u128::from(q) * u128::from(bid) * (10_000 + u128::from(premium));
                            assert_eq!(
                                all_in_premium_ok(cost, q, bid, q_ref, premium).unwrap(),
                                expected
                            );
                        }
                    }
                }
            }
        }
        for zero in [(0, 1, 1, 1), (1, 0, 1, 1), (1, 1, 0, 1), (1, 1, 1, 0)] {
            assert_eq!(
                all_in_premium_ok(zero.0, zero.1, zero.2, zero.3, 0),
                Err(Error::InvalidMarks)
            );
        }
    }

    #[test]
    fn premium_wide_products_match_bigint_and_do_not_wrap_at_u64_max() {
        for cost in [1, u64::MAX - 1, u64::MAX] {
            for q in [1, u64::MAX - 1, u64::MAX] {
                for bid in [1, u64::MAX - 1, u64::MAX] {
                    for q_ref in [1, u64::MAX - 1, u64::MAX] {
                        for premium in [0u32, 1, u32::MAX] {
                            let expected = BigUint::from(cost)
                                * BigUint::from(q_ref)
                                * BigUint::from(10_000u64)
                                <= BigUint::from(q)
                                    * BigUint::from(bid)
                                    * BigUint::from(10_000u64 + u64::from(premium));
                            assert_eq!(
                                all_in_premium_ok(cost, q, bid, q_ref, premium).unwrap(),
                                expected
                            );
                        }
                    }
                }
            }
        }
        assert!(all_in_premium_ok(u64::MAX, u64::MAX, u64::MAX, u64::MAX, 0).unwrap());
        assert!(!all_in_premium_ok(u64::MAX, u64::MAX - 1, u64::MAX, u64::MAX, 0).unwrap());
        assert!(U256::MAX.checked_mul(U256::from_u64(2)).is_none());
    }

    #[test]
    fn snapshot_d4_uses_max_ask_history_and_actual_sale_with_ceiling_fee() {
        let t = terms(&[1, 2, 3, 4]);
        let mut i = inputs(2);
        i.actual_sale = 25;
        let result = check_open(flat(2), flat(1), flat(10), t, i, LIMITS, NOW).unwrap();
        assert_eq!(result.opening_notional, 25);
        assert_eq!(result.margin, 75);
        assert_eq!(result.collateral, 100);
        assert_eq!(result.open_fee, 1);
        assert_eq!(result.borrower_funding, 76);
        assert_eq!(result.booked_notional_after, 25);
        let p = evidence(2, 10, 15, &[(10, 30), (10, 20)]);
        let result = check_open(
            p,
            flat(1),
            flat(10),
            AdmissionTerms {
                position_premium_bps: 5_000,
                ..t
            },
            inputs(2),
            LIMITS,
            NOW,
        )
        .unwrap();
        assert_eq!(result.opening_notional, 30);
        assert_eq!(result.margin, 100);
        assert_eq!(result.collateral, 120);
    }

    #[test]
    fn admission_matches_independent_inventory_raw_wallet_and_utilization_reference() {
        let lots = [1, 2, 3, 4];
        let t = terms(&lots);
        for q in 1..=4 {
            for free in 0..=8 {
                for borrowed in 0..=8 {
                    for wallet in 0..=8 {
                        let i = OpenInputs {
                            tokens_free: free,
                            borrowed_quantity: borrowed,
                            wallet_quantity: wallet,
                            ..inputs(q)
                        };
                        let expected = q <= free
                            && borrowed + q <= 10
                            && (wallet + q) * 10_000 <= 5_000 * 10
                            && (borrowed + q) * 10_000 <= 9_000 * (free + borrowed);
                        let result = check_open(flat(q), flat(1), flat(10), t, i, LIMITS, NOW);
                        assert_eq!(
                            result.is_ok(),
                            expected,
                            "q={q} free={free} borrowed={borrowed} wallet={wallet}: {result:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn admission_refuses_unsupported_lots_unbounded_caps_and_wrong_report_quantities() {
        let lots = [1, 2, 3, 4];
        let t = terms(&lots);
        let i = inputs(2);
        assert_eq!(
            check_open(
                flat(2),
                flat(1),
                flat(10),
                AdmissionTerms {
                    supported_position_lots: &[1, 4],
                    ..t
                },
                i,
                LIMITS,
                NOW
            ),
            Err(Error::UnsupportedLot)
        );
        for bad in [
            AdmissionTerms {
                market_quantity_max: 0,
                ..t
            },
            AdmissionTerms {
                market_notional_max: 0,
                ..t
            },
            AdmissionTerms {
                small_lot_quantity: 0,
                ..t
            },
            AdmissionTerms {
                capacity_quantity: 9,
                ..t
            },
            AdmissionTerms {
                utilization_bps: 10_001,
                ..t
            },
            AdmissionTerms {
                wallet_bps: 10_001,
                ..t
            },
            AdmissionTerms {
                supported_position_lots: &[1, 1],
                ..t
            },
            AdmissionTerms {
                supported_position_lots: &[2, 1],
                ..t
            },
            AdmissionTerms {
                min_notional: 0,
                ..t
            },
            AdmissionTerms {
                cr_open_bps: 9_999,
                ..t
            },
        ] {
            assert_eq!(
                check_open(flat(2), flat(1), flat(10), bad, i, LIMITS, NOW),
                Err(Error::InvalidTerms)
            );
        }
        assert_eq!(
            check_open(flat(2), flat(2), flat(10), t, i, LIMITS, NOW),
            Err(Error::QuantityMismatch)
        );
        assert_eq!(
            check_open(flat(2), flat(1), flat(11), t, i, LIMITS, NOW),
            Err(Error::QuantityMismatch)
        );
        assert_eq!(
            check_open(flat(3), flat(1), flat(10), t, i, LIMITS, NOW),
            Err(Error::QuantityMismatch)
        );
        assert_eq!(
            check_open(
                flat(2),
                flat(1),
                flat(10),
                AdmissionTerms {
                    utilization_bps: 0,
                    ..t
                },
                i,
                LIMITS,
                NOW
            ),
            Err(Error::Utilization)
        );
        assert_eq!(
            check_open(
                flat(2),
                flat(1),
                flat(10),
                AdmissionTerms { wallet_bps: 0, ..t },
                i,
                LIMITS,
                NOW
            ),
            Err(Error::WalletQuantityCap)
        );
    }

    #[test]
    fn admission_checks_premiums_notional_and_cross_feed_coherence() {
        let t = terms(&[1, 2, 3, 4]);
        let i = inputs(2);
        let expensive = evidence(2, 20, 21, &[(20, 21), (20, 21)]);
        assert_eq!(
            check_open(expensive, flat(1), flat(10), t, i, LIMITS, NOW),
            Err(Error::PositionPremium)
        );
        let capacity = evidence(10, 100, 101, &[(100, 101), (100, 101)]);
        assert_eq!(
            check_open(flat(2), flat(1), capacity, t, i, LIMITS, NOW),
            Err(Error::CapacityPremium)
        );
        assert_eq!(
            check_open(
                flat(2),
                flat(1),
                flat(10),
                AdmissionTerms {
                    min_notional: 21,
                    ..t
                },
                i,
                LIMITS,
                NOW
            ),
            Err(Error::MinNotional)
        );
        assert_eq!(
            check_open(
                flat(2),
                flat(1),
                flat(10),
                AdmissionTerms {
                    position_notional_max: 19,
                    ..t
                },
                i,
                LIMITS,
                NOW
            ),
            Err(Error::PositionNotional)
        );
        assert_eq!(
            check_open(
                flat(2),
                flat(1),
                flat(10),
                t,
                OpenInputs {
                    booked_notional: 9_981,
                    ..i
                },
                LIMITS,
                NOW
            ),
            Err(Error::MarketNotional)
        );
        let mut other = flat(1);
        other.key.policy_digest = [3; 32];
        assert_eq!(
            check_open(flat(2), other, flat(10), t, i, LIMITS, NOW),
            Err(Error::IncoherentReferences)
        );
        let mut other = flat(1);
        other.live_source.start.slot -= 1;
        assert_eq!(
            check_open(flat(2), other, flat(10), t, i, LIMITS, NOW),
            Err(Error::IncoherentReferences)
        );
    }

    #[test]
    fn u64_sum_and_margin_overflows_refuse_instead_of_saturating_caps() {
        let t = terms(&[1, 2, 3, 4]);
        let i = inputs(2);
        for changed in [
            OpenInputs {
                borrowed_quantity: u64::MAX,
                ..i
            },
            OpenInputs {
                wallet_quantity: u64::MAX,
                ..i
            },
            OpenInputs {
                booked_notional: u64::MAX,
                ..i
            },
        ] {
            assert_eq!(
                check_open(flat(2), flat(1), flat(10), t, changed, LIMITS, NOW),
                Err(Error::Arithmetic(MathError::Overflow))
            );
        }
        let huge = evidence(
            1,
            u64::MAX,
            u64::MAX,
            &[(u64::MAX, u64::MAX), (u64::MAX, u64::MAX)],
        );
        let wide_terms = AdmissionTerms {
            supported_position_lots: &[1],
            capacity_quantity: 1,
            position_quantity_max: 1,
            market_quantity_max: 1,
            wallet_bps: 10_000,
            position_notional_max: u64::MAX,
            market_notional_max: u64::MAX,
            ..t
        };
        assert_eq!(
            check_open(huge, huge, huge, wide_terms, inputs(1), LIMITS, NOW),
            Err(Error::Arithmetic(MathError::Overflow))
        );
        assert_eq!(
            check_open(
                flat(2),
                flat(1),
                flat(10),
                t,
                OpenInputs {
                    actual_sale: u64::MAX,
                    ..i
                },
                LIMITS,
                NOW
            ),
            Err(Error::PositionNotional)
        );
        let funding_terms = AdmissionTerms {
            cr_open_bps: 10_000,
            open_fee_bps: 10_000,
            ..wide_terms
        };
        assert_eq!(
            check_open(
                huge,
                huge,
                huge,
                funding_terms,
                OpenInputs {
                    actual_sale: 0,
                    ..inputs(1)
                },
                LIMITS,
                NOW
            ),
            Err(Error::Arithmetic(MathError::Overflow))
        );
    }

    #[test]
    fn cr_and_actual_cost_veto_match_independent_threshold_reference() {
        for v in 0..=20u64 {
            for r in 0..=20u64 {
                for cost in 1..=20u64 {
                    let s = evidence(1, cost, cost, &[(cost, cost), (cost, cost)]);
                    for threshold in [1u32, 10_000, 17_500, 40_000] {
                        let effective = v.saturating_sub(r);
                        let below = effective * 10_000 / cost < u64::from(threshold);
                        assert_eq!(
                            cr_liquidatable(1, s, NOW, v, r, threshold, 2).unwrap(),
                            below
                        );
                        assert_eq!(
                            actual_fill_refutes_cr(false, v, r, cost, threshold).unwrap(),
                            !below
                        );
                        assert!(!actual_fill_refutes_cr(true, v, r, cost, threshold).unwrap());
                    }
                }
            }
        }
        let s = evidence(1, 11, 11, &[(11, 11), (9, 9)]);
        assert!(cr_liquidatable(1, s, NOW, 10, 0, 10_000, 1).unwrap());
        assert!(!cr_liquidatable(1, s, NOW, 10, 0, 10_000, 2).unwrap());
        assert_eq!(
            cr_liquidatable(1, s, NOW, 10, 0, 10_000, 0),
            Err(Error::InvalidPersistence)
        );
        assert_eq!(
            cr_liquidatable(1, s, NOW, 10, 0, 10_000, 3),
            Err(Error::InvalidPersistence)
        );
        assert!(actual_fill_refutes_cr(false, 0, 100, 0, 17_500).unwrap());
        assert_eq!(
            actual_fill_refutes_cr(false, 1, 0, 1, 0),
            Err(Error::InvalidTerms)
        );
        assert!(actual_fill_refutes_cr(false, u64::MAX, 0, u64::MAX, 10_000).unwrap());
        assert!(!actual_fill_refutes_cr(false, u64::MAX, 0, u64::MAX, 10_001).unwrap());
    }

    #[test]
    fn finite_s1_is_strict_full_history_budget_predicate_not_default_authorization() {
        let s = evidence(1, 11, 11, &[(11, 11), (11, 11)]);
        assert!(
            finite_s1_gap(
                1,
                s,
                NOW,
                risk::Budget {
                    v: 10,
                    k_s: 0,
                    ert: 0
                }
            )
            .unwrap()
        );
        assert!(
            !finite_s1_gap(
                1,
                s,
                NOW,
                risk::Budget {
                    v: 11,
                    k_s: 0,
                    ert: 0
                }
            )
            .unwrap()
        );
        assert!(
            finite_s1_gap(
                1,
                s,
                NOW,
                risk::Budget {
                    v: 12,
                    k_s: 1,
                    ert: 1
                }
            )
            .unwrap()
        );
        let cheap = evidence(1, 11, 11, &[(11, 11), (10, 10)]);
        assert!(
            !finite_s1_gap(
                1,
                cheap,
                NOW,
                risk::Budget {
                    v: 10,
                    k_s: 0,
                    ert: 0
                }
            )
            .unwrap()
        );
        assert_eq!(
            finite_s1_gap(
                1,
                s,
                NOW,
                risk::Budget {
                    v: 0,
                    k_s: 1,
                    ert: 0
                }
            ),
            Err(Error::InvalidBudget)
        );
        let huge = evidence(
            1,
            u64::MAX,
            u64::MAX,
            &[(u64::MAX, u64::MAX), (u64::MAX, u64::MAX)],
        );
        assert!(
            finite_s1_gap(
                1,
                huge,
                NOW,
                risk::Budget {
                    v: u64::MAX,
                    k_s: 0,
                    ert: 1
                }
            )
            .unwrap()
        );
        assert!(
            !finite_s1_gap(
                1,
                huge,
                NOW,
                risk::Budget {
                    v: u64::MAX,
                    k_s: 0,
                    ert: 0
                }
            )
            .unwrap()
        );
    }

    #[test]
    fn acquisition_budget_matches_reference_and_caps_before_u64_conversion() {
        let s = evidence(1, 1, 101, &[(1, 100), (1, 102)]);
        for delta in [0u32, 1, 100, 500, u32::MAX] {
            for v in 0..=110u64 {
                for k in 0..=v {
                    let rent = k / 2;
                    let available = v.saturating_sub(k).checked_sub(rent);
                    let reference = 100u64;
                    let expected = available
                        .map(|a| {
                            (u128::from(reference) * (10_000 + u128::from(delta)) / 10_000)
                                .min(u128::from(a)) as u64
                        })
                        .filter(|amount| *amount != 0)
                        .map(|max_input| AcquisitionBudget {
                            reference,
                            max_input,
                        });
                    assert_eq!(
                        acquisition_budget(
                            1,
                            s,
                            NOW,
                            delta,
                            risk::Budget {
                                v,
                                k_s: k,
                                ert: rent
                            }
                        )
                        .unwrap(),
                        expected
                    );
                }
            }
        }
        assert_eq!(
            acquisition_budget(
                1,
                s,
                NOW,
                0,
                risk::Budget {
                    v: 1,
                    k_s: 2,
                    ert: 0
                }
            ),
            Err(Error::InvalidBudget)
        );
        let huge = evidence(
            1,
            u64::MAX,
            u64::MAX,
            &[(u64::MAX, u64::MAX), (u64::MAX, u64::MAX)],
        );
        assert_eq!(
            acquisition_budget(
                1,
                huge,
                NOW,
                u32::MAX,
                risk::Budget {
                    v: u64::MAX,
                    k_s: 0,
                    ert: 0
                }
            )
            .unwrap()
            .unwrap()
            .max_input,
            u64::MAX
        );
        assert_eq!(
            acquisition_budget(
                1,
                huge,
                NOW,
                u32::MAX,
                risk::Budget {
                    v: 7,
                    k_s: 1,
                    ert: 2
                }
            )
            .unwrap()
            .unwrap()
            .max_input,
            4
        );
        assert_eq!(
            acquisition_budget(
                1,
                s,
                NOW,
                0,
                risk::Budget {
                    v: 7,
                    k_s: 1,
                    ert: 7
                }
            )
            .unwrap(),
            None
        );
    }
}
