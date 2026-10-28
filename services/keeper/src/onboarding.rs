//! Ordinary off-chain onboarding work coordination, without transaction authority.
//!
//! Inputs must come from the caller's validated chain/source readers. This module does
//! not authenticate accounts, choose financial terms, send transactions, or enable a
//! market. A lease coordinates work, not provider quota: the shared request limiter
//! must still approve every real request. Existing-debt work precedes new admission.
use serde::{Deserialize, Serialize};

const VERSION: u16 = 1;
const MAX_ENTRIES: usize = 4096;
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub genesis_hash: [u8; 32],
    pub program: [u8; 32],
    pub config: [u8; 32],
    pub market: [u8; 32],
    pub mint: [u8; 32],
    pub token_program: [u8; 32],
    pub model_version: u32,
    pub model_epoch: u64,
    pub policy_hash: [u8; 32],
}
impl Identity {
    fn same_source(self, other: Self) -> bool {
        self.genesis_hash == other.genesis_hash
            && self.program == other.program
            && self.market == other.market
            && self.model_epoch == other.model_epoch
    }

    fn valid(self) -> bool {
        [
            self.genesis_hash,
            self.program,
            self.config,
            self.market,
            self.mint,
            self.token_program,
            self.policy_hash,
        ]
        .iter()
        .all(|key| *key != [0; 32])
            && self.model_version > 0
            && self.model_epoch > 0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_entries: u16,
    pub max_concurrent_jobs: u16,
    /// Background budget already excludes the independently reserved exit budget.
    pub background_rps: u32,
    pub evidence_max_age_seconds: u32,
    pub lease_seconds: u32,
    pub retry_initial_seconds: u32,
    pub retry_max_seconds: u32,
}
impl Limits {
    fn valid(self) -> bool {
        self.max_entries > 0
            && usize::from(self.max_entries) <= MAX_ENTRIES
            && self.max_concurrent_jobs > 0
            && self.max_concurrent_jobs <= self.max_entries
            && self.background_rps > 0
            && self.evidence_max_age_seconds > 0
            && self.lease_seconds > 0
            && self.retry_initial_seconds > 0
            && self.retry_max_seconds >= self.retry_initial_seconds
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub identity: Identity,
    /// Read from the prospective fixed policy, never freely chosen by an API caller.
    pub minimum_quantity_raw: u64,
    pub required_history_epochs: u8,
    /// Measured all-feed background demand for the requested sampling schedule.
    pub background_rps: u32,
}
impl Request {
    fn valid(self) -> bool {
        self.identity.valid()
            && self.minimum_quantity_raw > 0
            && (2..=8).contains(&self.required_history_epochs)
            && self.background_rps > 0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MintSupport {
    Unchecked,
    Supported,
    Unsupported,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Binding {
    Missing,
    Matched,
    Mismatched,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Routes {
    Unchecked,
    Available,
    Unavailable,
}

/// A single dated observation bundle, prepared by actual read-only adapters.
/// The coordinator cannot convert local quotations into observed chain activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub observed_at_seconds: u64,
    pub observed_slot: u64,
    pub mint: MintSupport,
    pub template_binding: Binding,
    pub inventory_raw: u64,
    pub routes: Routes,
    pub provider_available: bool,
    pub feeds_binding: Binding,
    pub consecutive_finite_history: bool,
    pub completed_history_epochs: u8,
    pub backend_initialized_on_chain: bool,
    pub policy_and_feeds_match_on_chain: bool,
    pub opens_enabled_on_chain: bool,
    pub has_open_debt_on_chain: bool,
    /// These reflect actual governance/emergency state. Scheduler completion,
    /// retry, expiry and readiness never clear them.
    pub governance_hold_on_chain: bool,
    pub emergency_hold_on_chain: bool,
}
impl Observation {
    fn valid(self, now: u64) -> bool {
        self.observed_at_seconds <= now
            && self.observed_slot > 0
            && self.completed_history_epochs <= 8
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Discovered,
    CheckingMint,
    UnsupportedMint,
    AwaitingTemplate,
    BindingMismatch,
    AwaitingInventory,
    CheckingRoutes,
    NoUsableRoute,
    ProviderUnavailable,
    QueuedForCapacity,
    PreparingFeedPlan,
    WarmingHistory { completed: u8, required: u8 },
    AwaitingActivation,
    Active,
    GovernanceHold,
    EmergencyHold,
    StaleEvidence,
    RetryBackoff { retry_at_seconds: u64 },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Work {
    Inspect,
    InspectTemplate,
    PrepareFeedPlan,
    CollectHistory,
    ObserveActivation,
    ObserveActive,
    /// Read/refresh existing obligations first; not an instruction to liquidate.
    MaintainDebtCoverage,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lease {
    pub identity: Identity,
    pub id: u64,
    pub worker: String,
    pub until_seconds: u64,
    pub work: Work,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    request: Request,
    discovered_at_seconds: u64,
    observation: Option<Observation>,
    lease: Option<Lease>,
    failures: u16,
    retry_at_seconds: u64,
    last_served_lease_id: u64,
}
impl Entry {
    fn committed(&self) -> bool {
        self.observation.is_some_and(|o| {
            o.has_open_debt_on_chain
                || (o.backend_initialized_on_chain
                    && o.policy_and_feeds_match_on_chain
                    && o.opens_enabled_on_chain)
        })
    }
    fn has_debt(&self) -> bool {
        self.observation.is_some_and(|o| o.has_open_debt_on_chain)
    }
    fn leased(&self, now: u64) -> bool {
        self.lease
            .as_ref()
            .is_some_and(|lease| lease.until_seconds > now)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coordinator {
    version: u16,
    limits: Limits,
    last_clock_seconds: u64,
    next_lease_id: u64,
    entries: Vec<Entry>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Configuration,
    Identity,
    QueueFull,
    Unknown,
    RequestConflict,
    Evidence,
    EvidenceRegression,
    ClockRegression,
    LeaseMismatch,
    Worker,
    Overflow,
    Corrupt,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "onboarding coordinator: {self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Discovery {
    Inserted,
    Existing,
}

impl Coordinator {
    pub fn new(limits: Limits) -> Result<Self, Error> {
        if !limits.valid() {
            return Err(Error::Configuration);
        }
        Ok(Self {
            version: VERSION,
            limits,
            last_clock_seconds: 0,
            next_lease_id: 1,
            entries: Vec::new(),
        })
    }
    fn at(&mut self, now: u64) -> Result<(), Error> {
        if now < self.last_clock_seconds {
            return Err(Error::ClockRegression);
        }
        self.last_clock_seconds = now;
        Ok(())
    }
    fn index(&self, identity: Identity) -> Result<usize, Error> {
        self.entries
            .iter()
            .position(|e| e.request.identity == identity)
            .ok_or(Error::Unknown)
    }
    pub fn discover(&mut self, request: Request, now: u64) -> Result<Discovery, Error> {
        if !request.valid() {
            return Err(Error::Identity);
        }
        if let Some(existing) = self
            .entries
            .iter()
            .find(|entry| entry.request.identity.same_source(request.identity))
        {
            if existing.request != request {
                return Err(Error::RequestConflict);
            }
            self.at(now)?;
            return Ok(Discovery::Existing);
        }
        if self.entries.len() >= usize::from(self.limits.max_entries) {
            return Err(Error::QueueFull);
        }
        self.at(now)?;
        self.entries.push(Entry {
            request,
            discovered_at_seconds: now,
            observation: None,
            lease: None,
            failures: 0,
            retry_at_seconds: 0,
            last_served_lease_id: 0,
        });
        Ok(Discovery::Inserted)
    }
    /// Record a bundle from the validated actual-chain reader. This is the sole
    /// API that may reflect an observed governance clear or chain activation.
    pub fn observe(
        &mut self,
        identity: Identity,
        observation: Observation,
        now: u64,
    ) -> Result<(), Error> {
        let i = self.index(identity)?;
        if !observation.valid(now) {
            return Err(Error::Evidence);
        }
        if self.entries[i].observation.is_some_and(|previous| {
            observation.observed_at_seconds < previous.observed_at_seconds
                || observation.observed_slot < previous.observed_slot
        }) {
            return Err(Error::EvidenceRegression);
        }
        self.at(now)?;
        self.entries[i].observation = Some(observation);
        Ok(())
    }
    /// Minimum committed/background-in-flight demand. It remains reserved when
    /// evidence expires or a held market still has debt; newcomers cannot evict it.
    pub fn committed_background_rps(&self) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.committed())
            .map(|e| u64::from(e.request.background_rps))
            .sum()
    }
    fn used_background_rps(&self, now: u64, except: Option<usize>) -> u64 {
        self.entries
            .iter()
            .enumerate()
            .filter(|(i, e)| Some(*i) != except && (e.committed() || e.leased(now)))
            .map(|(_, e)| u64::from(e.request.background_rps))
            .sum()
    }
    fn base_phase(&self, i: usize, now: u64) -> Phase {
        let e = &self.entries[i];
        let Some(o) = e.observation else {
            return Phase::Discovered;
        };
        // Preserve an observed manual hold even after its evidence becomes old.
        if o.emergency_hold_on_chain {
            return Phase::EmergencyHold;
        }
        if o.governance_hold_on_chain {
            return Phase::GovernanceHold;
        }
        if now < o.observed_at_seconds
            || now - o.observed_at_seconds > u64::from(self.limits.evidence_max_age_seconds)
        {
            return Phase::StaleEvidence;
        }
        match o.mint {
            MintSupport::Unchecked => return Phase::CheckingMint,
            MintSupport::Unsupported => return Phase::UnsupportedMint,
            MintSupport::Supported => {}
        }
        match o.template_binding {
            Binding::Missing => return Phase::AwaitingTemplate,
            Binding::Mismatched => return Phase::BindingMismatch,
            Binding::Matched => {}
        }
        if o.inventory_raw < e.request.minimum_quantity_raw {
            return Phase::AwaitingInventory;
        }
        match o.routes {
            Routes::Unchecked => return Phase::CheckingRoutes,
            Routes::Unavailable => return Phase::NoUsableRoute,
            Routes::Available => {}
        }
        if !o.provider_available {
            return Phase::ProviderUnavailable;
        }
        if !e.committed()
            && self.used_background_rps(now, Some(i)) + u64::from(e.request.background_rps)
                > u64::from(self.limits.background_rps)
        {
            return Phase::QueuedForCapacity;
        }
        match o.feeds_binding {
            Binding::Missing => return Phase::PreparingFeedPlan,
            Binding::Mismatched => return Phase::BindingMismatch,
            Binding::Matched => {}
        }
        let completed = if o.consecutive_finite_history {
            o.completed_history_epochs
        } else {
            0
        };
        if completed < e.request.required_history_epochs {
            return Phase::WarmingHistory {
                completed,
                required: e.request.required_history_epochs,
            };
        }
        if !(o.backend_initialized_on_chain
            && o.policy_and_feeds_match_on_chain
            && o.opens_enabled_on_chain)
        {
            return Phase::AwaitingActivation;
        }
        Phase::Active
    }
    pub fn phase(&self, identity: Identity, now: u64) -> Result<Phase, Error> {
        if now < self.last_clock_seconds {
            return Err(Error::ClockRegression);
        }
        let i = self.index(identity)?;
        let phase = self.base_phase(i, now);
        // A retry timer never masks a real manual hold.
        if !matches!(phase, Phase::GovernanceHold | Phase::EmergencyHold)
            && self.entries[i].retry_at_seconds > now
        {
            return Ok(Phase::RetryBackoff {
                retry_at_seconds: self.entries[i].retry_at_seconds,
            });
        }
        Ok(phase)
    }
    /// FIFO within each priority. Existing debt is serviced before new requests,
    /// including when observed coverage exceeds the configured background budget.
    /// Actual requests still require the independent shared provider limiter.
    pub fn lease_next(&mut self, worker: &str, now: u64) -> Result<Option<Lease>, Error> {
        if worker.is_empty()
            || worker.len() > 64
            || !worker
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-./".contains(&c))
        {
            return Err(Error::Worker);
        }
        if now < self.last_clock_seconds {
            return Err(Error::ClockRegression);
        }
        if self.entries.iter().filter(|e| e.leased(now)).count()
            >= usize::from(self.limits.max_concurrent_jobs)
        {
            return Ok(None);
        }
        let mut chosen: Option<(usize, Work)> = None;
        for debt_priority in [true, false] {
            for (i, e) in self.entries.iter().enumerate() {
                if e.has_debt() != debt_priority || e.leased(now) || e.retry_at_seconds > now {
                    continue;
                }
                let phase = self.base_phase(i, now);
                let work = if debt_priority {
                    Work::MaintainDebtCoverage
                } else {
                    match phase {
                        Phase::GovernanceHold
                        | Phase::EmergencyHold
                        | Phase::UnsupportedMint
                        | Phase::BindingMismatch
                        | Phase::QueuedForCapacity => continue,
                        Phase::AwaitingTemplate => Work::InspectTemplate,
                        Phase::PreparingFeedPlan => Work::PrepareFeedPlan,
                        Phase::WarmingHistory { .. } => Work::CollectHistory,
                        Phase::AwaitingActivation => Work::ObserveActivation,
                        Phase::Active => Work::ObserveActive,
                        _ => Work::Inspect,
                    }
                };
                // Inspection/unknown stages also consume bounded worker capacity.
                if !e.committed()
                    && self.used_background_rps(now, Some(i)) + u64::from(e.request.background_rps)
                        > u64::from(self.limits.background_rps)
                {
                    continue;
                }
                if chosen.as_ref().is_none_or(|(old, _)| {
                    (e.last_served_lease_id, i) < (self.entries[*old].last_served_lease_id, *old)
                }) {
                    chosen = Some((i, work));
                }
            }
            if chosen.is_some() {
                break;
            }
        }
        let Some((i, work)) = chosen else {
            return Ok(None);
        };
        let until = now
            .checked_add(u64::from(self.limits.lease_seconds))
            .ok_or(Error::Overflow)?;
        let next = self.next_lease_id.checked_add(1).ok_or(Error::Overflow)?;
        self.at(now)?;
        let lease = Lease {
            identity: self.entries[i].request.identity,
            id: self.next_lease_id,
            worker: worker.to_string(),
            until_seconds: until,
            work,
        };
        self.next_lease_id = next;
        self.entries[i].lease = Some(lease.clone());
        self.entries[i].last_served_lease_id = lease.id;
        Ok(Some(lease))
    }
    fn leased_index(&self, lease: &Lease, now: u64) -> Result<usize, Error> {
        let i = self.index(lease.identity)?;
        if self.entries[i].lease.as_ref() != Some(lease) || lease.until_seconds <= now {
            return Err(Error::LeaseMismatch);
        }
        Ok(i)
    }
    pub fn complete(
        &mut self,
        lease: &Lease,
        observation: Observation,
        now: u64,
    ) -> Result<(), Error> {
        let i = self.leased_index(lease, now)?;
        let retry = now
            .checked_add(u64::from(self.limits.retry_initial_seconds))
            .ok_or(Error::Overflow)?;
        // A worker completion cannot turn local readiness into chain activation
        // or clear a manual hold. A fresh authoritative observe() must do that.
        let previous = self.entries[i].observation;
        let chain_fields = |o: Observation| {
            (
                o.backend_initialized_on_chain,
                o.policy_and_feeds_match_on_chain,
                o.opens_enabled_on_chain,
                o.has_open_debt_on_chain,
                o.governance_hold_on_chain,
                o.emergency_hold_on_chain,
            )
        };
        if previous.map(chain_fields) != Some(chain_fields(observation)) {
            return Err(Error::Evidence);
        }
        self.observe(lease.identity, observation, now)?;
        self.entries[i].lease = None;
        self.entries[i].failures = 0;
        self.entries[i].retry_at_seconds = retry;
        Ok(())
    }
    pub fn fail(&mut self, lease: &Lease, now: u64) -> Result<(), Error> {
        let i = self.leased_index(lease, now)?;
        let failures = self.entries[i].failures.saturating_add(1);
        let shift = u32::from(failures.saturating_sub(1).min(31));
        let delay = u64::from(self.limits.retry_initial_seconds)
            .saturating_mul(1u64 << shift)
            .min(u64::from(self.limits.retry_max_seconds));
        let retry = now.checked_add(delay).ok_or(Error::Overflow)?;
        self.at(now)?;
        self.entries[i].lease = None;
        self.entries[i].failures = failures;
        self.entries[i].retry_at_seconds = retry;
        Ok(())
    }
    pub fn requests(&self) -> impl Iterator<Item = &Request> {
        self.entries.iter().map(|e| &e.request)
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Corrupt)?;
        if bytes.len() > MAX_BYTES {
            return Err(Error::Corrupt);
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8], expected: Limits) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::Corrupt);
        }
        let state: Self = serde_json::from_slice(bytes).map_err(|_| Error::Corrupt)?;
        if state.version != VERSION
            || state.limits != expected
            || !expected.valid()
            || state.entries.len() > usize::from(expected.max_entries)
            || state.next_lease_id == 0
        {
            return Err(Error::Corrupt);
        }
        for (i, e) in state.entries.iter().enumerate() {
            if !e.request.valid()
                || e.discovered_at_seconds > state.last_clock_seconds
                || e.observation
                    .is_some_and(|o| !o.valid(state.last_clock_seconds))
                || state.entries[..i]
                    .iter()
                    .any(|previous| previous.request.identity.same_source(e.request.identity))
                || e.last_served_lease_id >= state.next_lease_id
                || e.lease.as_ref().is_some_and(|lease| {
                    lease.identity != e.request.identity
                        || lease.id != e.last_served_lease_id
                        || lease.id == 0
                        || lease.id >= state.next_lease_id
                        || lease.worker.is_empty()
                        || lease.worker.len() > 64
                        || lease.until_seconds == 0
                })
            {
                return Err(Error::Corrupt);
            }
        }
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> Limits {
        Limits {
            max_entries: 8,
            max_concurrent_jobs: 2,
            background_rps: 6,
            evidence_max_age_seconds: 4,
            lease_seconds: 10,
            retry_initial_seconds: 2,
            retry_max_seconds: 8,
        }
    }
    fn request(m: u8) -> Request {
        Request {
            identity: Identity {
                genesis_hash: [1; 32],
                program: [2; 32],
                config: [3; 32],
                market: [m; 32],
                mint: [m + 1; 32],
                token_program: [4; 32],
                model_version: 1,
                model_epoch: 1,
                policy_hash: [5; 32],
            },
            minimum_quantity_raw: 1000,
            required_history_epochs: 2,
            background_rps: 4,
        }
    }
    fn observation(now: u64) -> Observation {
        Observation {
            observed_at_seconds: now,
            observed_slot: now + 1,
            mint: MintSupport::Supported,
            template_binding: Binding::Matched,
            inventory_raw: 1000,
            routes: Routes::Available,
            provider_available: true,
            feeds_binding: Binding::Matched,
            consecutive_finite_history: true,
            completed_history_epochs: 2,
            backend_initialized_on_chain: false,
            policy_and_feeds_match_on_chain: false,
            opens_enabled_on_chain: false,
            has_open_debt_on_chain: false,
            governance_hold_on_chain: false,
            emergency_hold_on_chain: false,
        }
    }
    #[test]
    fn local_readiness_never_invents_chain_activation() {
        let mut c = Coordinator::new(limits()).unwrap();
        let r = request(10);
        c.discover(r, 10).unwrap();
        assert_eq!(c.phase(r.identity, 10), Ok(Phase::Discovered));
        let mut o = observation(10);
        o.template_binding = Binding::Missing;
        c.observe(r.identity, o, 10).unwrap();
        assert_eq!(c.phase(r.identity, 10), Ok(Phase::AwaitingTemplate));
        o.template_binding = Binding::Matched;
        o.inventory_raw = 0;
        c.observe(r.identity, o, 10).unwrap();
        assert_eq!(c.phase(r.identity, 10), Ok(Phase::AwaitingInventory));
        o.inventory_raw = 1000;
        o.feeds_binding = Binding::Missing;
        c.observe(r.identity, o, 10).unwrap();
        assert_eq!(c.phase(r.identity, 10), Ok(Phase::PreparingFeedPlan));
        o.feeds_binding = Binding::Matched;
        o.completed_history_epochs = 1;
        c.observe(r.identity, o, 10).unwrap();
        assert_eq!(
            c.phase(r.identity, 10),
            Ok(Phase::WarmingHistory {
                completed: 1,
                required: 2
            })
        );
        o.completed_history_epochs = 2;
        c.observe(r.identity, o, 10).unwrap();
        assert_eq!(c.phase(r.identity, 10), Ok(Phase::AwaitingActivation));
        o.opens_enabled_on_chain = true;
        c.observe(r.identity, o, 10).unwrap();
        assert_eq!(c.phase(r.identity, 10), Ok(Phase::AwaitingActivation));
        o.backend_initialized_on_chain = true;
        o.policy_and_feeds_match_on_chain = true;
        c.observe(r.identity, o, 10).unwrap();
        assert_eq!(c.phase(r.identity, 10), Ok(Phase::Active));
        assert_eq!(c.phase(r.identity, 15), Ok(Phase::StaleEvidence));
    }
    #[test]
    fn manual_holds_survive_retry_expiry_and_stale_evidence() {
        let mut c = Coordinator::new(limits()).unwrap();
        let r = request(10);
        c.discover(r, 10).unwrap();
        let mut o = observation(10);
        o.governance_hold_on_chain = true;
        c.observe(r.identity, o, 10).unwrap();
        assert_eq!(c.phase(r.identity, 20), Ok(Phase::GovernanceHold));
        assert!(c.lease_next("worker", 20).unwrap().is_none());
        o.observed_at_seconds = 20;
        o.observed_slot = 21;
        o.emergency_hold_on_chain = true;
        o.has_open_debt_on_chain = true;
        c.observe(r.identity, o, 20).unwrap();
        let lease = c.lease_next("worker", 20).unwrap().unwrap();
        assert_eq!(lease.work, Work::MaintainDebtCoverage);
        c.fail(&lease, 21).unwrap();
        assert_eq!(c.phase(r.identity, 21), Ok(Phase::EmergencyHold));
        assert_eq!(c.phase(r.identity, 100), Ok(Phase::EmergencyHold));
    }
    #[test]
    fn capacity_queue_cannot_displace_open_debt_and_dedup_cannot_rewrite_terms() {
        let mut c = Coordinator::new(limits()).unwrap();
        let a = request(10);
        let b = request(20);
        c.discover(a, 10).unwrap();
        c.discover(b, 10).unwrap();
        let mut existing = observation(10);
        existing.has_open_debt_on_chain = true;
        c.observe(b.identity, existing, 10).unwrap();
        c.observe(a.identity, observation(10), 10).unwrap();
        assert_eq!(c.phase(a.identity, 10), Ok(Phase::QueuedForCapacity));
        let lease = c.lease_next("worker", 10).unwrap().unwrap();
        assert_eq!(lease.identity, b.identity);
        assert_eq!(c.discover(a, 10), Ok(Discovery::Existing));
        let mut changed = a;
        changed.minimum_quantity_raw = 1;
        assert_eq!(c.discover(changed, 10), Err(Error::RequestConflict));
        let mut changed_hash = a;
        changed_hash.identity.policy_hash = [77; 32];
        assert_eq!(c.discover(changed_hash, 10), Err(Error::RequestConflict));
        let mut changed_mint = a;
        changed_mint.identity.mint = [78; 32];
        assert_eq!(c.discover(changed_mint, 10), Err(Error::RequestConflict));
        assert_eq!(c.committed_background_rps(), 4);
        assert_eq!(c.phase(a.identity, 20), Ok(Phase::StaleEvidence));
        assert_eq!(
            c.committed_background_rps(),
            4,
            "old evidence cannot free existing obligations"
        );
    }
    #[test]
    fn leases_expire_but_old_workers_cannot_complete_new_work() {
        let mut c = Coordinator::new(limits()).unwrap();
        let r = request(10);
        c.discover(r, 10).unwrap();
        let a = c.lease_next("first", 10).unwrap().unwrap();
        assert!(c.lease_next("second", 11).unwrap().is_none());
        let b = c.lease_next("second", 20).unwrap().unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(
            c.complete(&a, observation(20), 20),
            Err(Error::LeaseMismatch)
        );
        c.observe(r.identity, observation(20), 20).unwrap();
        c.complete(&b, observation(20), 20).unwrap();
        assert_eq!(
            c.phase(r.identity, 20),
            Ok(Phase::RetryBackoff {
                retry_at_seconds: 22
            })
        );
        assert_eq!(c.phase(r.identity, 22), Ok(Phase::AwaitingActivation));
    }
    #[test]
    fn workers_cannot_clear_a_hold_or_claim_activation_and_debt_jobs_are_fair() {
        let mut c = Coordinator::new(limits()).unwrap();
        let a = request(10);
        let b = request(20);
        for r in [a, b] {
            c.discover(r, 10).unwrap();
            let mut o = observation(10);
            o.has_open_debt_on_chain = true;
            o.governance_hold_on_chain = true;
            c.observe(r.identity, o, 10).unwrap();
        }
        let lease = c.lease_next("worker", 10).unwrap().unwrap();
        assert_eq!(lease.identity, a.identity);
        let before = c.clone();
        let mut fake = observation(10);
        fake.has_open_debt_on_chain = true;
        fake.opens_enabled_on_chain = true;
        fake.backend_initialized_on_chain = true;
        fake.policy_and_feeds_match_on_chain = true;
        assert_eq!(c.complete(&lease, fake, 10), Err(Error::Evidence));
        assert_eq!(c, before);
        c.fail(&lease, 10).unwrap();
        let second = c.lease_next("worker", 12).unwrap().unwrap();
        assert_eq!(second.identity, b.identity);
        assert_eq!(c.phase(a.identity, 12), Ok(Phase::GovernanceHold));
    }
    #[test]
    fn observation_identity_time_and_slots_do_not_regress() {
        let mut c = Coordinator::new(limits()).unwrap();
        let r = request(10);
        c.discover(r, 10).unwrap();
        c.observe(r.identity, observation(11), 11).unwrap();
        let before = c.clone();
        assert_eq!(
            c.observe(r.identity, observation(10), 11),
            Err(Error::EvidenceRegression)
        );
        assert_eq!(c, before);
        let mut future = observation(12);
        future.observed_slot = 1;
        assert_eq!(
            c.observe(r.identity, future, 12),
            Err(Error::EvidenceRegression)
        );
        assert_eq!(c, before);
        assert_eq!(
            c.observe(request(20).identity, observation(12), 12),
            Err(Error::Unknown)
        );
        assert_eq!(c.discover(r, 9), Err(Error::ClockRegression));
        assert_eq!(c, before);
    }
    #[test]
    fn journal_roundtrip_retains_holds_leases_and_pending_backoff() {
        let mut c = Coordinator::new(limits()).unwrap();
        let r = request(10);
        c.discover(r, 10).unwrap();
        let mut o = observation(10);
        o.governance_hold_on_chain = true;
        o.has_open_debt_on_chain = true;
        c.observe(r.identity, o, 10).unwrap();
        let lease = c.lease_next("worker", 10).unwrap().unwrap();
        let restored = Coordinator::decode(&c.encode().unwrap(), limits()).unwrap();
        assert_eq!(c, restored);
        c.fail(&lease, 11).unwrap();
        let restored = Coordinator::decode(&c.encode().unwrap(), limits()).unwrap();
        assert_eq!(restored.phase(r.identity, 12), Ok(Phase::GovernanceHold));
        let mut different = limits();
        different.background_rps = 100;
        assert_eq!(
            Coordinator::decode(&c.encode().unwrap(), different),
            Err(Error::Corrupt)
        );
        let mut value: serde_json::Value = serde_json::from_slice(&c.encode().unwrap()).unwrap();
        let duplicate = value["entries"][0].clone();
        value["entries"].as_array_mut().unwrap().push(duplicate);
        assert_eq!(
            Coordinator::decode(&serde_json::to_vec(&value).unwrap(), limits()),
            Err(Error::Corrupt)
        );
    }
    #[test]
    fn queue_is_bounded_and_failure_backoff_is_capped() {
        let mut l = limits();
        l.max_entries = 1;
        l.max_concurrent_jobs = 1;
        let mut c = Coordinator::new(l).unwrap();
        let r = request(10);
        c.discover(r, 10).unwrap();
        assert_eq!(c.discover(request(20), 10), Err(Error::QueueFull));
        let mut now = 10;
        for delay in [2, 4, 8, 8] {
            let lease = c.lease_next("worker", now).unwrap().unwrap();
            c.fail(&lease, now).unwrap();
            assert_eq!(
                c.phase(r.identity, now),
                Ok(Phase::RetryBackoff {
                    retry_at_seconds: now + delay
                })
            );
            now += delay;
        }
    }
}
