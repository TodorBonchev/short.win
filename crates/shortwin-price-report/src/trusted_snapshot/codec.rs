//! Fixed canonical persistence for the operator-attested snapshot mode only.
use super::*;
use crate::codec::CodecError;

pub const STATE_BYTES: usize = 1533;
const MAGIC: &[u8; 8] = b"SWPSNAP2";
struct Writer<'a> {
    bytes: &'a mut [u8],
    at: usize,
}
impl Writer<'_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), CodecError> {
        let end = self.at.checked_add(bytes.len()).ok_or(CodecError::Length)?;
        self.bytes
            .get_mut(self.at..end)
            .ok_or(CodecError::Length)?
            .copy_from_slice(bytes);
        self.at = end;
        Ok(())
    }
    fn zero(&mut self, n: usize) -> Result<(), CodecError> {
        let end = self.at.checked_add(n).ok_or(CodecError::Length)?;
        self.bytes
            .get_mut(self.at..end)
            .ok_or(CodecError::Length)?
            .fill(0);
        self.at = end;
        Ok(())
    }
    fn byte(&mut self, n: u8) -> Result<(), CodecError> {
        self.put(&[n])
    }
    fn u64(&mut self, n: u64) -> Result<(), CodecError> {
        self.put(&n.to_le_bytes())
    }
    fn clock(&mut self, c: RuntimeClock) -> Result<(), CodecError> {
        self.u64(c.slot)?;
        self.put(&c.unix_timestamp.to_le_bytes())
    }
    fn window(&mut self, w: ObservationWindow) -> Result<(), CodecError> {
        self.clock(w.start)?;
        self.clock(w.end)
    }
    fn bucket(&mut self, a: &AttestedBucket) -> Result<(), CodecError> {
        let b = a.bucket;
        self.u64(b.epoch)?;
        self.clock(b.first_observed)?;
        self.clock(b.last_observed)?;
        for n in [
            b.lower_min,
            b.central_last,
            b.upper_max,
            b.observations,
            a.largest_gap_slots,
            a.largest_gap_seconds,
        ] {
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
    fn take(&mut self, n: usize) -> Result<&[u8], CodecError> {
        let end = self.at.checked_add(n).ok_or(CodecError::Length)?;
        let data = self.bytes.get(self.at..end).ok_or(CodecError::Length)?;
        self.at = end;
        Ok(data)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        self.take(N)?.try_into().map_err(|_| CodecError::Length)
    }
    fn zero(&mut self, n: usize) -> Result<(), CodecError> {
        if self.take(n)?.iter().any(|b| *b != 0) {
            Err(CodecError::NonCanonical)
        } else {
            Ok(())
        }
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
    fn bucket(&mut self) -> Result<AttestedBucket, CodecError> {
        Ok(AttestedBucket {
            bucket: CompletedBucket {
                epoch: self.u64()?,
                first_observed: self.clock()?,
                last_observed: self.clock()?,
                lower_min: self.u64()?,
                central_last: self.u64()?,
                upper_max: self.u64()?,
                observations: self.u64()?,
            },
            largest_gap_slots: self.u64()?,
            largest_gap_seconds: self.u64()?,
        })
    }
}

pub fn encode(state: &SnapshotState, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != STATE_BYTES {
        return Err(CodecError::Length);
    }
    validate(state)?;
    let mut w = Writer {
        bytes: output,
        at: 0,
    };
    w.put(MAGIC)?;
    w.put(&TRUSTED_SNAPSHOT_VERSION.to_le_bytes())?;
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
    for key in c.publishers {
        w.put(&key)?;
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
    w.u64(state.outstanding_positions)?;
    if let Some(m) = state.latest {
        w.byte(1)?;
        w.u64(m.sequence)?;
        w.put(&m.publisher)?;
        w.window(m.window)?;
        w.put(&m.observation_digest)?;
        w.u64(m.observation_epoch)?;
    } else {
        w.byte(0)?;
        w.zero(112)?;
    }
    match state.live {
        ReportStatus::Unknown => {
            w.byte(0)?;
            w.zero(24)?;
        }
        ReportStatus::Finite(m) => {
            w.byte(1)?;
            w.u64(m.lower())?;
            w.u64(m.central())?;
            w.u64(m.upper())?;
        }
    }
    w.byte(state.history.len)?;
    for bucket in &state.history.buckets {
        if let Some(bucket) = bucket {
            w.bucket(bucket)?;
        } else {
            w.zero(88)?;
        }
    }
    if w.at != STATE_BYTES {
        return Err(CodecError::Length);
    }
    Ok(())
}

pub fn decode(input: &[u8]) -> Result<SnapshotState, CodecError> {
    if input.len() != STATE_BYTES {
        return Err(CodecError::Length);
    }
    let mut r = Reader {
        bytes: input,
        at: 0,
    };
    if r.take(8)? != MAGIC || u16::from_le_bytes(r.array()?) != TRUSTED_SNAPSHOT_VERSION {
        return Err(CodecError::Version);
    }
    let configuration = read_configuration(&mut r)?;
    restore_body(configuration, &mut r)
}

// Bound temporary lifetimes to separate SBF frames, as in the legacy report codec.
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
    let count = usize::from(r.byte()?);
    if count == 0 || count > MAX_PUBLISHERS {
        return Err(CodecError::InvalidState);
    }
    let mut publishers = [[0; 32]; MAX_PUBLISHERS];
    for key in &mut publishers {
        *key = r.array()?;
    }
    if publishers[count..].iter().any(|k| *k != [0; 32]) {
        return Err(CodecError::NonCanonical);
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
    Configuration::new(domain, quantity, model, epoch, &publishers[..count], policy)
        .map_err(CodecError::Configuration)
}

#[inline(never)]
fn restore_body(
    configuration: Configuration,
    r: &mut Reader<'_>,
) -> Result<SnapshotState, CodecError> {
    let mut state = SnapshotState {
        configuration,
        configured_at: r.clock()?,
        last_runtime_clock: RuntimeClock {
            slot: 0,
            unix_timestamp: 0,
        },
        last_sequence: 0,
        latest: None,
        live: ReportStatus::Unknown,
        history: AttestedHistory::empty(),
        outstanding_positions: 0,
    };
    state.last_runtime_clock = r.clock()?;
    state.last_sequence = r.u64()?;
    state.outstanding_positions = r.u64()?;
    state.latest = if r.tag()? {
        Some(ReportMetadata {
            sequence: r.u64()?,
            publisher: r.array()?,
            window: r.window()?,
            observation_digest: r.array()?,
            observation_epoch: r.u64()?,
        })
    } else {
        r.zero(112)?;
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
    let len = r.byte()?;
    if usize::from(len) > HISTORY_CAPACITY {
        return Err(CodecError::InvalidState);
    }
    state.history.len = len;
    for (i, b) in state.history.buckets.iter_mut().enumerate() {
        if i < usize::from(len) {
            *b = Some(r.bucket()?);
        } else {
            r.zero(88)?;
        }
    }
    if r.at != STATE_BYTES {
        return Err(CodecError::Length);
    }
    validate(&state)?;
    Ok(state)
}
#[inline(never)]
fn validate(s: &SnapshotState) -> Result<(), CodecError> {
    let c = &s.configuration;
    let rebuilt = Configuration::new(
        c.domain,
        c.quantity,
        c.model,
        c.epoch,
        c.publishers(),
        c.policy,
    )
    .map_err(CodecError::Configuration)?;
    if rebuilt != *c || usize::from(s.history.len) > HISTORY_CAPACITY {
        return Err(CodecError::NonCanonical);
    }
    for (index, bucket) in s.history.buckets.iter().enumerate() {
        if bucket.is_some() != (index < usize::from(s.history.len)) {
            return Err(CodecError::NonCanonical);
        }
    }
    s.configured_at
        .seconds()
        .map_err(CodecError::Configuration)?;
    if !s.last_runtime_clock.not_before(s.configured_at) {
        return Err(CodecError::InvalidState);
    }
    match s.latest {
        None => {
            if s.last_sequence != 0
                || s.live != ReportStatus::Unknown
                || !s.history.is_empty()
                || s.last_runtime_clock != s.configured_at
            {
                return Err(CodecError::InvalidState);
            }
        }
        Some(m) => {
            if s.last_sequence == 0
                || m.sequence != s.last_sequence
                || !c.authorizes(&m.publisher)
                || m.observation_digest == [0; 32]
                || !m.window.start.not_before(s.configured_at)
            {
                return Err(CodecError::InvalidState);
            }
            c.policy
                .validate_window(m.window, s.last_runtime_clock)
                .map_err(CodecError::Configuration)?;
            let epoch = c
                .policy
                .epoch(m.window.end)
                .map_err(CodecError::Configuration)?;
            if m.observation_epoch != epoch
                || c.policy
                    .epoch(m.window.start)
                    .map_err(CodecError::Configuration)?
                    != epoch
                || c.policy
                    .epoch(m.window.end)
                    .map_err(CodecError::Configuration)?
                    != epoch
            {
                return Err(CodecError::InvalidState);
            }
            match s.live {
                ReportStatus::Unknown => {
                    if !s.history.is_empty() {
                        return Err(CodecError::InvalidState);
                    }
                }
                ReportStatus::Finite(marks) => {
                    CostMarks::new(marks.lower(), marks.central(), marks.upper())
                        .map_err(CodecError::Configuration)?;
                    validate_history(c, s.configured_at, &s.history, m.window)
                        .map_err(CodecError::Configuration)?;
                }
            }
        }
    }
    Ok(())
}
