//! Canonical fixed v1 persistence. No Rust memory layout or caller-supplied ring indices.
use super::*;

pub const STATE_BYTES: usize = 1488;
pub const CODEC_VERSION: u16 = 1;
const MAGIC: &[u8; 8] = b"SWPRSTAT";
const BUCKET_BYTES: usize = 72;
const METADATA_BYTES: usize = 112;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    Length,
    Version,
    NonCanonical,
    InvalidState,
    Configuration(Error),
}

struct Writer<'a> {
    bytes: &'a mut [u8],
    at: usize,
}
impl Writer<'_> {
    fn put(&mut self, value: &[u8]) -> Result<(), CodecError> {
        let end = self.at.checked_add(value.len()).ok_or(CodecError::Length)?;
        self.bytes
            .get_mut(self.at..end)
            .ok_or(CodecError::Length)?
            .copy_from_slice(value);
        self.at = end;
        Ok(())
    }
    fn zero(&mut self, count: usize) -> Result<(), CodecError> {
        let end = self.at.checked_add(count).ok_or(CodecError::Length)?;
        self.bytes
            .get_mut(self.at..end)
            .ok_or(CodecError::Length)?
            .fill(0);
        self.at = end;
        Ok(())
    }
    fn byte(&mut self, value: u8) -> Result<(), CodecError> {
        self.put(&[value])
    }
    fn u64(&mut self, value: u64) -> Result<(), CodecError> {
        self.put(&value.to_le_bytes())
    }
    fn clock(&mut self, clock: RuntimeClock) -> Result<(), CodecError> {
        self.u64(clock.slot)?;
        self.put(&clock.unix_timestamp.to_le_bytes())
    }
    fn window(&mut self, window: ObservationWindow) -> Result<(), CodecError> {
        self.clock(window.start)?;
        self.clock(window.end)
    }
    fn bucket(&mut self, b: &CompletedBucket) -> Result<(), CodecError> {
        self.u64(b.epoch)?;
        self.clock(b.first_observed)?;
        self.clock(b.last_observed)?;
        for n in [b.lower_min, b.central_last, b.upper_max, b.observations] {
            self.u64(n)?;
        }
        Ok(())
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Reader<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], CodecError> {
        let end = self.at.checked_add(count).ok_or(CodecError::Length)?;
        let value = self.bytes.get(self.at..end).ok_or(CodecError::Length)?;
        self.at = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        self.take(N)?.try_into().map_err(|_| CodecError::Length)
    }
    fn zero(&mut self, count: usize) -> Result<(), CodecError> {
        if self.take(count)?.iter().any(|b| *b != 0) {
            return Err(CodecError::NonCanonical);
        }
        Ok(())
    }
    fn byte(&mut self) -> Result<u8, CodecError> {
        Ok(self.array::<1>()?[0])
    }
    fn tag(&mut self) -> Result<bool, CodecError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CodecError::NonCanonical),
        }
    }
    fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn clock(&mut self) -> Result<RuntimeClock, CodecError> {
        Ok(RuntimeClock {
            slot: self.u64()?,
            unix_timestamp: i64::from_le_bytes(self.array()?),
        })
    }
    fn window(&mut self) -> Result<ObservationWindow, CodecError> {
        Ok(ObservationWindow {
            start: self.clock()?,
            end: self.clock()?,
        })
    }
    fn bucket(&mut self) -> Result<CompletedBucket, CodecError> {
        Ok(CompletedBucket {
            epoch: self.u64()?,
            first_observed: self.clock()?,
            last_observed: self.clock()?,
            lower_min: self.u64()?,
            central_last: self.u64()?,
            upper_max: self.u64()?,
            observations: self.u64()?,
        })
    }
}

/// Exact output length is mandatory. No failure mutates a valid output buffer: all validation
/// is done first, then fixed checked writes of the proven v1 shape. Invalid-length buffers stay intact.
pub fn encode(state: &ReportState, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != STATE_BYTES {
        return Err(CodecError::Length);
    }
    validate(state)?;
    let mut w = Writer {
        bytes: output,
        at: 0,
    };
    w.put(MAGIC)?;
    w.put(&CODEC_VERSION.to_le_bytes())?;
    let c = &state.configuration;
    let d = c.domain;
    for key in [
        d.genesis_hash,
        d.program,
        d.config,
        d.market,
        d.base_mint,
        d.base_token_program,
        d.quote_mint,
        d.quote_token_program,
    ] {
        w.put(&key)?;
    }
    w.byte(d.base_decimals)?;
    w.byte(d.quote_decimals)?;
    w.u64(c.quantity)?;
    w.put(&c.model.version.to_le_bytes())?;
    w.u64(c.model.epoch)?;
    w.put(&c.model.policy_hash)?;
    w.u64(c.epoch)?;
    w.byte(c.publisher_count)?;
    for (i, key) in c.publishers.iter().enumerate() {
        if i < usize::from(c.publisher_count) {
            w.put(key)?;
        } else {
            w.zero(32)?;
        }
    }
    let p = c.policy;
    for n in [
        p.max_age_slots,
        p.max_age_seconds,
        p.max_window_slots,
        p.max_window_seconds,
        p.max_update_gap_slots,
        p.max_update_gap_seconds,
        p.epoch_seconds,
    ] {
        w.u64(n)?;
    }
    w.byte(p.minimum_completed_epochs)?;
    w.clock(state.configured_at)?;
    w.clock(state.last_runtime_clock)?;
    w.u64(state.last_sequence)?;
    if let Some(m) = state.latest {
        w.byte(1)?;
        w.u64(m.sequence)?;
        w.put(&m.publisher)?;
        w.window(m.window)?;
        w.put(&m.observation_digest)?;
        w.u64(m.observation_epoch)?;
    } else {
        w.byte(0)?;
        w.zero(METADATA_BYTES)?;
    }
    match state.live {
        ReportStatus::Unknown => {
            w.byte(0)?;
            w.zero(24)?;
        }
        ReportStatus::Finite(m) => {
            w.byte(1)?;
            w.u64(m.lower)?;
            w.u64(m.central)?;
            w.u64(m.upper)?;
        }
    }
    if let Some(epoch) = state.pending_epoch {
        w.byte(1)?;
        w.u64(epoch)?;
    } else {
        w.byte(0)?;
        w.zero(8)?;
    }
    if let Some(bucket) = &state.pending {
        w.byte(1)?;
        w.bucket(bucket)?;
    } else {
        w.byte(0)?;
        w.zero(BUCKET_BYTES)?;
    }
    w.byte(u8::from(state.pending_tainted))?;
    w.byte(state.history.count)?;
    for i in 0..HISTORY_CAPACITY {
        if i < usize::from(state.history.count) {
            let offset = usize::from(state.history.count)
                .checked_sub(i + 1)
                .ok_or(CodecError::InvalidState)?;
            w.bucket(
                state
                    .history
                    .newest(offset)
                    .ok_or(CodecError::InvalidState)?,
            )?;
        } else {
            w.zero(BUCKET_BYTES)?;
        }
    }
    w.u64(state.outstanding_positions)?;
    if w.at != STATE_BYTES {
        return Err(CodecError::Length);
    }
    Ok(())
}

/// Exact length, canonical tags/padding and restored semantic invariants are required.
/// This restores state; it never authenticates the account owner, PDA, signer or clock.
pub fn decode(input: &[u8]) -> Result<ReportState, CodecError> {
    if input.len() != STATE_BYTES {
        return Err(CodecError::Length);
    }
    let mut r = Reader {
        bytes: input,
        at: 0,
    };
    if r.array::<8>()? != *MAGIC || u16::from_le_bytes(r.array()?) != CODEC_VERSION {
        return Err(CodecError::Version);
    }
    let configuration = read_configuration(&mut r)?;
    restore_body(configuration, &mut r)
}

// Separate bounded frames prevent domain/publisher temporaries overlapping the restored state.
#[inline(never)]
fn read_configuration(r: &mut Reader<'_>) -> Result<Configuration, CodecError> {
    let domain = Domain {
        genesis_hash: r.array()?,
        program: r.array()?,
        config: r.array()?,
        market: r.array()?,
        base_mint: r.array()?,
        base_token_program: r.array()?,
        quote_mint: r.array()?,
        quote_token_program: r.array()?,
        base_decimals: r.byte()?,
        quote_decimals: r.byte()?,
    };
    let quantity = r.u64()?;
    let model = ModelBinding {
        version: u32::from_le_bytes(r.array()?),
        epoch: r.u64()?,
        policy_hash: r.array()?,
    };
    let epoch = r.u64()?;
    let publisher_count = r.byte()?;
    if publisher_count == 0 || usize::from(publisher_count) > MAX_PUBLISHERS {
        return Err(CodecError::InvalidState);
    }
    let mut publishers = [[0; 32]; MAX_PUBLISHERS];
    for (i, key) in publishers.iter_mut().enumerate() {
        if i < usize::from(publisher_count) {
            *key = r.array()?;
        } else {
            r.zero(32)?;
        }
    }
    let policy = HistoryPolicy {
        max_age_slots: r.u64()?,
        max_age_seconds: r.u64()?,
        max_window_slots: r.u64()?,
        max_window_seconds: r.u64()?,
        max_update_gap_slots: r.u64()?,
        max_update_gap_seconds: r.u64()?,
        epoch_seconds: r.u64()?,
        minimum_completed_epochs: r.byte()?,
    };
    Configuration::new(
        domain,
        quantity,
        model,
        epoch,
        &publishers[..usize::from(publisher_count)],
        policy,
    )
    .map_err(CodecError::Configuration)
}

#[inline(never)]
fn restore_body(
    configuration: Configuration,
    r: &mut Reader<'_>,
) -> Result<ReportState, CodecError> {
    let mut state = ReportState {
        configuration,
        configured_at: r.clock()?,
        last_runtime_clock: RuntimeClock {
            slot: 0,
            unix_timestamp: 0,
        },
        last_sequence: 0,
        latest: None,
        live: ReportStatus::Unknown,
        pending_epoch: None,
        pending: None,
        pending_tainted: true,
        history: History::empty(),
        outstanding_positions: 0,
    };
    state.last_runtime_clock = r.clock()?;
    state.last_sequence = r.u64()?;
    state.latest = if r.tag()? {
        Some(ReportMetadata {
            sequence: r.u64()?,
            publisher: r.array()?,
            window: r.window()?,
            observation_digest: r.array()?,
            observation_epoch: r.u64()?,
        })
    } else {
        r.zero(METADATA_BYTES)?;
        None
    };
    state.live = if r.tag()? {
        ReportStatus::Finite(
            CostMarks::new(r.u64()?, r.u64()?, r.u64()?).map_err(CodecError::Configuration)?,
        )
    } else {
        r.zero(24)?;
        ReportStatus::Unknown
    };
    state.pending_epoch = if r.tag()? {
        Some(r.u64()?)
    } else {
        r.zero(8)?;
        None
    };
    state.pending = if r.tag()? {
        Some(r.bucket()?)
    } else {
        r.zero(BUCKET_BYTES)?;
        None
    };
    state.pending_tainted = r.tag()?;
    let count = r.byte()?;
    if usize::from(count) > HISTORY_CAPACITY {
        return Err(CodecError::InvalidState);
    }
    for i in 0..HISTORY_CAPACITY {
        if i < usize::from(count) {
            state.history.push(r.bucket()?);
        } else {
            r.zero(BUCKET_BYTES)?;
        }
    }
    state.outstanding_positions = r.u64()?;
    if r.at != STATE_BYTES {
        return Err(CodecError::Length);
    }
    validate(&state)?;
    Ok(state)
}
fn valid_bucket(b: &CompletedBucket, configured_at: RuntimeClock, policy: HistoryPolicy) -> bool {
    let Some(following) = b.observations.checked_sub(1) else {
        return false;
    };
    let max_slot_span = policy
        .max_window_slots
        .saturating_add(following.saturating_mul(policy.max_update_gap_slots));
    let max_time_span = policy
        .max_window_seconds
        .saturating_add(following.saturating_mul(policy.max_update_gap_seconds));
    b.lower_min > 0
        && b.lower_min <= b.central_last
        && b.central_last <= b.upper_max
        && b.last_observed.not_before(b.first_observed)
        && b.first_observed.not_before(configured_at)
        && policy.epoch(b.first_observed).ok() == Some(b.epoch)
        && policy.epoch(b.last_observed).ok() == Some(b.epoch)
        && b.last_observed
            .slot
            .checked_sub(b.first_observed.slot)
            .is_some_and(|span| span <= max_slot_span)
        && b.last_observed
            .seconds()
            .ok()
            .zip(b.first_observed.seconds().ok())
            .is_some_and(|(last, first)| last.saturating_sub(first) <= max_time_span)
        && b.last_observed
            .slot
            .checked_sub(b.first_observed.slot)
            .and_then(|n| n.checked_add(1))
            .is_none_or(|maximum| b.observations <= maximum)
}
fn timely_begin(b: &CompletedBucket, configured_at: RuntimeClock, policy: HistoryPolicy) -> bool {
    policy.epoch_start(b.epoch).ok().is_some_and(|start| {
        configured_at
            .seconds()
            .is_ok_and(|configured| configured <= start)
            && b.first_observed.seconds().is_ok_and(|first| {
                first
                    .checked_sub(start)
                    .is_some_and(|gap| gap <= policy.max_update_gap_seconds)
            })
    })
}
fn timely_end(b: &CompletedBucket, policy: HistoryPolicy) -> bool {
    b.epoch
        .checked_add(1)
        .and_then(|e| policy.epoch_start(e).ok())
        .is_some_and(|end| {
            b.last_observed.seconds().is_ok_and(|last| {
                end.checked_sub(last)
                    .is_some_and(|gap| gap <= policy.max_update_gap_seconds)
            })
        })
}
fn validate(s: &ReportState) -> Result<(), CodecError> {
    let bad = CodecError::InvalidState;
    let c = &s.configuration;
    if usize::from(c.publisher_count) > MAX_PUBLISHERS || c.publisher_count == 0 {
        return Err(bad);
    }
    Configuration::new(
        c.domain,
        c.quantity,
        c.model,
        c.epoch,
        c.publishers(),
        c.policy,
    )
    .map_err(CodecError::Configuration)?;
    if s.configured_at.seconds().is_err()
        || !s.last_runtime_clock.not_before(s.configured_at)
        || s.last_runtime_clock.seconds().is_err()
        || usize::from(s.history.count) > HISTORY_CAPACITY
        || usize::from(s.history.head) >= HISTORY_CAPACITY
    {
        return Err(bad);
    }
    let Some(latest) = s.latest else {
        if s.live != ReportStatus::Unknown
            || s.pending_epoch.is_some()
            || s.pending.is_some()
            || !s.pending_tainted
            || s.history.count != 0
        {
            return Err(bad);
        }
        return Ok(());
    };
    if latest.sequence == 0
        || latest.sequence != s.last_sequence
        || !c.authorizes(&latest.publisher)
        || latest.observation_digest == [0; 32]
        || !latest.window.start.not_before(s.configured_at)
        || c.policy
            .validate_window(latest.window, s.last_runtime_clock)
            .is_err()
        || c.policy.epoch(s.last_runtime_clock).ok() != Some(latest.observation_epoch)
        || c.policy.epoch(latest.window.start).ok() != Some(latest.observation_epoch)
        || c.policy.epoch(latest.window.end).ok() != Some(latest.observation_epoch)
        || s.pending_epoch != Some(latest.observation_epoch)
    {
        return Err(bad);
    }
    match s.live {
        ReportStatus::Unknown => {
            if s.pending.is_some() || !s.pending_tainted || s.history.count != 0 {
                return Err(bad);
            }
        }
        ReportStatus::Finite(marks) => {
            let b = s.pending.ok_or(bad)?;
            if !valid_bucket(&b, s.configured_at, c.policy)
                || b.epoch != latest.observation_epoch
                || b.last_observed != latest.window.end
                || b.lower_min > marks.lower
                || b.central_last != marks.central
                || b.upper_max < marks.upper
                || (b.observations == 1
                    && (b.lower_min != marks.lower || b.upper_max != marks.upper))
                || (!s.pending_tainted && !timely_begin(&b, s.configured_at, c.policy))
            {
                return Err(bad);
            }
        }
    }
    let mut newer_epoch = latest.observation_epoch;
    let mut newer_end = latest.window.end;
    for offset in 0..usize::from(s.history.count) {
        let b = s.history.newest(offset).ok_or(bad)?;
        if !valid_bucket(b, s.configured_at, c.policy)
            || !timely_begin(b, s.configured_at, c.policy)
            || !timely_end(b, c.policy)
            || b.epoch.checked_add(1) != Some(newer_epoch)
            || b.last_observed.slot >= newer_end.slot
            || b.last_observed.unix_timestamp > newer_end.unix_timestamp
            || s.pending_tainted
        {
            return Err(bad);
        }
        newer_epoch = b.epoch;
        newer_end = b.last_observed;
    }
    Ok(())
}
