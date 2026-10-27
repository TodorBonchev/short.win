//! Safe, dependency-free prevalidation of the published 0.13 oracle Ed25519 envelope.
//!
//! A successful result certifies FORMAT ONLY. It does not verify a signature, program ID,
//! instruction ordering/execution, queue membership, slot hash, freshness, feed identity,
//! value semantics, or financial policy. The caller must independently establish all of them.
//! In particular, an API response or these borrowed views is not authoritative oracle data.
//!
//! Layout source: switchboard-on-demand 0.13.0, `sysvar/ed25519_sysvar.rs`,
//! `sysvar/ix_sysvar.rs`, and `on_demand/oracle_quote/feed_info.rs`. That SDK instruction
//! helper requires all three first-record instruction indices to equal the selected index;
//! its parser checks subsequent indices against the first message index. This validator
//! enforces the combined condition safely before any eventual SDK parser call. It also
//! applies a deliberately stricter local, nonoverlapping, distinct-signer format policy.
#![forbid(unsafe_code)]

const MAX_SIGNATURES: usize = 8;
const MAX_FEEDS: usize = 8;
const OFFSET_RECORD_LEN: usize = 14;
const SIGNATURE_LEN: usize = 64;
const PUBLIC_KEY_LEN: usize = 32;
const MESSAGE_HEADER_LEN: usize = 32;
const FEED_RECORD_LEN: usize = 49;
const SUFFIX_METADATA_LEN: usize = 13;
const MIN_ENVELOPE_LEN: usize = 2
    + OFFSET_RECORD_LEN
    + SIGNATURE_LEN
    + PUBLIC_KEY_LEN
    + MESSAGE_HEADER_LEN
    + FEED_RECORD_LEN
    + 1
    + SUFFIX_METADATA_LEN;

/// Explicit format/resource ceilings, not production quorum or risk defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvelopeLimits {
    pub maximum_data_length: usize,
    pub maximum_signatures: u8,
    pub maximum_feeds: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    InvalidLimits,
    DataLimit,
    Truncated,
    SignatureCount,
    Padding,
    InstructionIndex,
    SuffixMarker,
    Version,
    OracleIndex,
    DuplicateOracleIndex,
    OffsetBounds,
    Overlap,
    MessageMismatch,
    MessageSize,
    DuplicatePublicKey,
    DuplicateFeedId,
    SignedMinimumSamples,
}

/// Unverified bytes of one Ed25519 signature record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignatureRecord<'a> {
    public_key: &'a [u8; PUBLIC_KEY_LEN],
    signature: &'a [u8; SIGNATURE_LEN],
}

impl<'a> SignatureRecord<'a> {
    pub const fn public_key(&self) -> &'a [u8; PUBLIC_KEY_LEN] {
        self.public_key
    }

    pub const fn signature(&self) -> &'a [u8; SIGNATURE_LEN] {
        self.signature
    }
}

/// A complete, bounded packed feed record; the raw value bytes remain uninterpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeedRecord<'a> {
    feed_id: &'a [u8; 32],
    raw_value: &'a [u8; 16],
    signed_minimum_samples: u8,
}

impl<'a> FeedRecord<'a> {
    pub const fn feed_id(&self) -> &'a [u8; 32] {
        self.feed_id
    }

    /// Little-endian i128 wire bytes. No scale, sign, range or unit checks occur here.
    pub const fn raw_value_bytes(&self) -> &'a [u8; 16] {
        self.raw_value
    }

    pub const fn signed_minimum_samples(&self) -> u8 {
        self.signed_minimum_samples
    }
}

/// FORMAT ONLY: private fields can be constructed only by safe bounded prevalidation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatOnlyEnvelope<'a> {
    instruction_data: &'a [u8],
    message: &'a [u8],
    signed_slot_hash: &'a [u8; 32],
    signatures: [Option<SignatureRecord<'a>>; MAX_SIGNATURES],
    feeds: [Option<FeedRecord<'a>>; MAX_FEEDS],
    signature_count: u8,
    feed_count: u8,
    oracle_indices: &'a [u8],
    recent_slot: u64,
    version: u8,
}

impl<'a> FormatOnlyEnvelope<'a> {
    pub const fn instruction_data(&self) -> &'a [u8] {
        self.instruction_data
    }

    pub const fn message(&self) -> &'a [u8] {
        self.message
    }

    pub const fn signed_slot_hash(&self) -> &'a [u8; 32] {
        self.signed_slot_hash
    }

    pub const fn signature_count(&self) -> u8 {
        self.signature_count
    }

    pub const fn feed_count(&self) -> u8 {
        self.feed_count
    }

    pub fn signature(&self, index: usize) -> Option<SignatureRecord<'a>> {
        self.signatures.get(index).copied().flatten()
    }

    pub fn feed(&self, index: usize) -> Option<FeedRecord<'a>> {
        self.feeds.get(index).copied().flatten()
    }

    pub const fn oracle_indices(&self) -> &'a [u8] {
        self.oracle_indices
    }

    /// Unauthenticated suffix slot, pending signature/queue/slot-hash verification.
    pub const fn recent_slot(&self) -> u64 {
        self.recent_slot
    }

    pub const fn version(&self) -> u8 {
        self.version
    }
}

#[derive(Clone, Copy)]
struct Region {
    start: usize,
    end: usize,
}

impl Region {
    fn new(start: usize, length: usize, lower: usize, upper: usize) -> Result<Self, WireError> {
        let end = start.checked_add(length).ok_or(WireError::OffsetBounds)?;
        if start < lower || end > upper {
            return Err(WireError::OffsetBounds);
        }
        Ok(Self { start, end })
    }

    fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

fn read_u16(data: &[u8], start: usize) -> Result<u16, WireError> {
    let end = start.checked_add(2).ok_or(WireError::Truncated)?;
    let bytes: [u8; 2] = data
        .get(start..end)
        .ok_or(WireError::Truncated)?
        .try_into()
        .map_err(|_| WireError::Truncated)?;
    Ok(u16::from_le_bytes(bytes))
}

fn array_ref<const N: usize>(data: &[u8], region: Region) -> Result<&[u8; N], WireError> {
    data.get(region.start..region.end)
        .ok_or(WireError::OffsetBounds)?
        .try_into()
        .map_err(|_| WireError::OffsetBounds)
}

/// Prevalidates a local oracle Ed25519 envelope without authenticating anything.
///
/// `selected_instruction_index` must be the actual securely loaded Ed25519 instruction's
/// transaction index. The SDK-compatible explicit index is required in ALL seven-u16
/// offset records; u16::MAX (Ed25519's current-instruction sentinel) is deliberately refused.
/// Source signature/key/message regions may be in any order but must be local, complete,
/// pairwise nonoverlapping and outside both the offset table and suffix. All signers refer
/// to the same complete message. Gaps are permitted within the caller's data-size ceiling.
/// The suffix is exactly oracle_indices[n] || slot_u64_le || version_u8 || b"SBOD".
/// Queue indices and public keys must each be distinct; every signed feed minimum is in
/// 1..=signature_count. Actual authorized quorum and allowed feed IDs are caller policy.
pub fn prevalidate_ed25519(
    data: &[u8],
    selected_instruction_index: u16,
    expected_version: u8,
    limits: EnvelopeLimits,
) -> Result<FormatOnlyEnvelope<'_>, WireError> {
    if limits.maximum_data_length < MIN_ENVELOPE_LEN
        || limits.maximum_data_length > usize::from(u16::MAX)
        || limits.maximum_signatures == 0
        || usize::from(limits.maximum_signatures) > MAX_SIGNATURES
        || limits.maximum_feeds == 0
        || usize::from(limits.maximum_feeds) > MAX_FEEDS
    {
        return Err(WireError::InvalidLimits);
    }
    if data.len() > limits.maximum_data_length {
        return Err(WireError::DataLimit);
    }
    let (&signature_count, rest) = data.split_first().ok_or(WireError::Truncated)?;
    if signature_count == 0 || signature_count > limits.maximum_signatures {
        return Err(WireError::SignatureCount);
    }
    if rest.first() != Some(&0) {
        return Err(if rest.is_empty() {
            WireError::Truncated
        } else {
            WireError::Padding
        });
    }
    if selected_instruction_index == u16::MAX {
        return Err(WireError::InstructionIndex);
    }
    let n = usize::from(signature_count);
    let table_end = n
        .checked_mul(OFFSET_RECORD_LEN)
        .and_then(|len| len.checked_add(2))
        .ok_or(WireError::Truncated)?;
    let suffix_len = n
        .checked_add(SUFFIX_METADATA_LEN)
        .ok_or(WireError::Truncated)?;
    let suffix_start = data
        .len()
        .checked_sub(suffix_len)
        .ok_or(WireError::Truncated)?;
    if suffix_start < table_end {
        return Err(WireError::Truncated);
    }
    let suffix = data.get(suffix_start..).ok_or(WireError::Truncated)?;
    if suffix.get(n + 9..) != Some(b"SBOD") {
        return Err(WireError::SuffixMarker);
    }
    let version = *suffix.get(n + 8).ok_or(WireError::Truncated)?;
    if version != expected_version {
        return Err(WireError::Version);
    }
    let recent_slot = u64::from_le_bytes(
        suffix
            .get(n..n + 8)
            .ok_or(WireError::Truncated)?
            .try_into()
            .map_err(|_| WireError::Truncated)?,
    );
    let oracle_indices = suffix.get(..n).ok_or(WireError::Truncated)?;
    for (index, oracle_index) in oracle_indices.iter().enumerate() {
        if *oracle_index >= 30 {
            return Err(WireError::OracleIndex);
        }
        if oracle_indices[..index].contains(oracle_index) {
            return Err(WireError::DuplicateOracleIndex);
        }
    }

    // At most 8 * (signature + key), plus the single shared message region.
    let mut regions = [None; MAX_SIGNATURES * 2 + 1];
    let mut region_count = 0;
    let mut signatures: [Option<SignatureRecord<'_>>; MAX_SIGNATURES] = [None; MAX_SIGNATURES];
    let mut shared_message_region: Option<Region> = None;
    for (index, signature_slot) in signatures.iter_mut().enumerate().take(n) {
        // n <= 8 and the complete table has already been bounded above.
        let at = 2 + index * OFFSET_RECORD_LEN;
        let signature_offset = usize::from(read_u16(data, at)?);
        let signature_instruction = read_u16(data, at + 2)?;
        let key_offset = usize::from(read_u16(data, at + 4)?);
        let key_instruction = read_u16(data, at + 6)?;
        let message_offset = usize::from(read_u16(data, at + 8)?);
        let message_size = usize::from(read_u16(data, at + 10)?);
        let message_instruction = read_u16(data, at + 12)?;
        if signature_instruction != selected_instruction_index
            || key_instruction != selected_instruction_index
            || message_instruction != selected_instruction_index
        {
            return Err(WireError::InstructionIndex);
        }
        let signature_region =
            Region::new(signature_offset, SIGNATURE_LEN, table_end, suffix_start)?;
        let key_region = Region::new(key_offset, PUBLIC_KEY_LEN, table_end, suffix_start)?;
        let message_region = Region::new(message_offset, message_size, table_end, suffix_start)?;
        if let Some(first) = shared_message_region {
            if first.start != message_region.start || first.end != message_region.end {
                return Err(WireError::MessageMismatch);
            }
        } else {
            let feed_bytes = message_size
                .checked_sub(MESSAGE_HEADER_LEN)
                .ok_or(WireError::MessageSize)?;
            if feed_bytes == 0
                || feed_bytes % FEED_RECORD_LEN != 0
                || feed_bytes / FEED_RECORD_LEN > usize::from(limits.maximum_feeds)
            {
                return Err(WireError::MessageSize);
            }
            regions[region_count] = Some(message_region);
            region_count += 1;
            shared_message_region = Some(message_region);
        }
        for region in [signature_region, key_region] {
            if regions[..region_count]
                .iter()
                .flatten()
                .any(|previous| region.overlaps(*previous))
            {
                return Err(WireError::Overlap);
            }
            regions[region_count] = Some(region);
            region_count += 1;
        }
        *signature_slot = Some(SignatureRecord {
            public_key: array_ref(data, key_region)?,
            signature: array_ref(data, signature_region)?,
        });
    }
    for (index, current) in signatures.iter().flatten().enumerate() {
        if signatures[..index]
            .iter()
            .flatten()
            .any(|previous| previous.public_key == current.public_key)
        {
            return Err(WireError::DuplicatePublicKey);
        }
    }
    let message_region = shared_message_region.ok_or(WireError::MessageSize)?;
    let message = data
        .get(message_region.start..message_region.end)
        .ok_or(WireError::OffsetBounds)?;
    let signed_slot_hash = message
        .get(..MESSAGE_HEADER_LEN)
        .ok_or(WireError::MessageSize)?
        .try_into()
        .map_err(|_| WireError::MessageSize)?;
    let feed_count = (message.len() - MESSAGE_HEADER_LEN) / FEED_RECORD_LEN;
    let mut feeds: [Option<FeedRecord<'_>>; MAX_FEEDS] = [None; MAX_FEEDS];
    for (index, feed_slot) in feeds.iter_mut().enumerate().take(feed_count) {
        let at = MESSAGE_HEADER_LEN + index * FEED_RECORD_LEN;
        let feed_id = array_ref(message, Region::new(at, 32, 0, message.len())?)?;
        let raw_value = array_ref(message, Region::new(at + 32, 16, 0, message.len())?)?;
        let signed_minimum_samples = *message.get(at + 48).ok_or(WireError::MessageSize)?;
        if signed_minimum_samples == 0 || signed_minimum_samples > signature_count {
            return Err(WireError::SignedMinimumSamples);
        }
        *feed_slot = Some(FeedRecord {
            feed_id,
            raw_value,
            signed_minimum_samples,
        });
    }
    for (index, current) in feeds.iter().flatten().enumerate() {
        if feeds[..index]
            .iter()
            .flatten()
            .any(|previous| previous.feed_id == current.feed_id)
        {
            return Err(WireError::DuplicateFeedId);
        }
    }
    // The feed-count range was bounded to 1..=8 when the message region was accepted.
    let feed_count = u8::try_from(feed_count).map_err(|_| WireError::MessageSize)?;
    Ok(FormatOnlyEnvelope {
        instruction_data: data,
        message,
        signed_slot_hash,
        signatures,
        feeds,
        signature_count,
        feed_count,
        oracle_indices,
        recent_slot,
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    const SELECTED_INDEX: u16 = 7;
    const VERSION: u8 = 3;

    fn limits() -> EnvelopeLimits {
        EnvelopeLimits {
            maximum_data_length: 2048,
            maximum_signatures: 8,
            maximum_feeds: 8,
        }
    }

    fn set_u16(data: &mut [u8], start: usize, value: usize) {
        data[start..start + 2].copy_from_slice(&u16::try_from(value).unwrap().to_le_bytes());
    }

    /// Synthetic wire fixture only: deliberately meaningless signatures are never verified.
    fn fixture(signatures: u8, feeds: u8) -> Vec<u8> {
        let n = usize::from(signatures);
        let table_end = 2 + OFFSET_RECORD_LEN * n;
        let message_start = table_end + n * (SIGNATURE_LEN + PUBLIC_KEY_LEN);
        let message_len = MESSAGE_HEADER_LEN + FEED_RECORD_LEN * usize::from(feeds);
        let mut data = vec![0; table_end];
        data[0] = signatures;
        for i in 0..n {
            let at = 2 + OFFSET_RECORD_LEN * i;
            set_u16(&mut data, at, table_end + i * SIGNATURE_LEN);
            set_u16(&mut data, at + 2, usize::from(SELECTED_INDEX));
            set_u16(
                &mut data,
                at + 4,
                table_end + n * SIGNATURE_LEN + i * PUBLIC_KEY_LEN,
            );
            set_u16(&mut data, at + 6, usize::from(SELECTED_INDEX));
            set_u16(&mut data, at + 8, message_start);
            set_u16(&mut data, at + 10, message_len);
            set_u16(&mut data, at + 12, usize::from(SELECTED_INDEX));
        }
        for i in 0..signatures {
            data.extend_from_slice(&[i + 10; SIGNATURE_LEN]);
        }
        for i in 0..signatures {
            data.extend_from_slice(&[i + 30; PUBLIC_KEY_LEN]);
        }
        data.extend_from_slice(&[0xa5; MESSAGE_HEADER_LEN]);
        for i in 0..feeds {
            data.extend_from_slice(&[i + 50; 32]);
            data.extend_from_slice(&i128::from(i).to_le_bytes());
            data.push(signatures);
        }
        data.extend(0..signatures);
        data.extend_from_slice(&123_456u64.to_le_bytes());
        data.push(VERSION);
        data.extend_from_slice(b"SBOD");
        data
    }

    fn check(data: &[u8]) -> Result<FormatOnlyEnvelope<'_>, WireError> {
        prevalidate_ed25519(data, SELECTED_INDEX, VERSION, limits())
    }

    #[test]
    fn bounded_full_message_and_raw_records_for_every_supported_count() {
        for n in 1..=8 {
            for feeds in 1..=8 {
                let data = fixture(n, feeds);
                let parsed = check(&data).unwrap();
                assert_eq!(parsed.signature_count(), n);
                assert_eq!(parsed.feed_count(), feeds);
                assert_eq!(parsed.instruction_data(), data);
                assert_eq!(parsed.signed_slot_hash(), &[0xa5; 32]);
                assert_eq!(parsed.message().len(), 32 + usize::from(feeds) * 49);
                assert_eq!(parsed.oracle_indices(), (0..n).collect::<Vec<_>>());
                assert_eq!(parsed.recent_slot(), 123_456);
                assert_eq!(parsed.version(), VERSION);
                for i in 0..n {
                    assert_eq!(
                        parsed.signature(usize::from(i)).unwrap().public_key(),
                        &[i + 30; 32]
                    );
                    assert_eq!(
                        parsed.signature(usize::from(i)).unwrap().signature(),
                        &[i + 10; 64]
                    );
                }
                for i in 0..feeds {
                    let feed = parsed.feed(usize::from(i)).unwrap();
                    assert_eq!(feed.feed_id(), &[i + 50; 32]);
                    assert_eq!(feed.raw_value_bytes(), &i128::from(i).to_le_bytes());
                    assert_eq!(feed.signed_minimum_samples(), n);
                }
                assert_eq!(parsed.signature(usize::from(n)), None);
                assert_eq!(parsed.signature(usize::MAX), None);
                assert_eq!(parsed.feed(usize::from(feeds)), None);
                assert_eq!(parsed.feed(usize::MAX), None);
            }
        }
    }

    #[test]
    fn all_truncated_prefixes_fail_without_panics() {
        for n in 1..=8 {
            for feeds in 1..=8 {
                let data = fixture(n, feeds);
                for length in 0..data.len() {
                    assert!(
                        check(&data[..length]).is_err(),
                        "n={n} feeds={feeds} length={length}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_record_requires_all_three_explicit_local_instruction_indices() {
        let data = fixture(8, 1);
        for record in 0..8 {
            for field in [2, 6, 12] {
                for bad in [0, 8, usize::from(u16::MAX)] {
                    let mut changed = data.clone();
                    set_u16(&mut changed, 2 + record * 14 + field, bad);
                    assert_eq!(check(&changed), Err(WireError::InstructionIndex));
                }
            }
        }
        assert_eq!(
            prevalidate_ed25519(&data, u16::MAX, VERSION, limits()),
            Err(WireError::InstructionIndex)
        );
    }

    #[test]
    fn table_padding_marker_version_and_exact_suffix_are_checked() {
        let data = fixture(2, 2);
        for count in [0, 9, u8::MAX] {
            let mut changed = data.clone();
            changed[0] = count;
            assert_eq!(check(&changed), Err(WireError::SignatureCount));
        }
        let mut changed = data.clone();
        changed[1] = 1;
        assert_eq!(check(&changed), Err(WireError::Padding));
        for from_end in 1..=4 {
            let mut changed = data.clone();
            let at = changed.len() - from_end;
            changed[at] ^= 1;
            assert_eq!(check(&changed), Err(WireError::SuffixMarker));
        }
        let mut changed = data.clone();
        let at = changed.len() - 5;
        changed[at] ^= 1;
        assert_eq!(check(&changed), Err(WireError::Version));
        let mut appended = data.clone();
        appended.push(0);
        assert_eq!(check(&appended), Err(WireError::SuffixMarker));
        // The full 14-byte table must precede even a syntactically valid suffix.
        let mut short = vec![8, 0];
        short.extend_from_slice(&[0; 8 + 9]);
        short.extend_from_slice(b"SBOD");
        assert_eq!(check(&short), Err(WireError::Truncated));
    }

    #[test]
    fn suffix_oracle_indices_and_signer_keys_are_distinct_and_bounded() {
        let data = fixture(2, 2);
        let start = data.len() - 2 - 13;
        let mut changed = data.clone();
        changed[start] = 30;
        assert_eq!(check(&changed), Err(WireError::OracleIndex));
        let mut changed = data.clone();
        changed[start + 1] = changed[start];
        assert_eq!(check(&changed), Err(WireError::DuplicateOracleIndex));
        let mut changed = data.clone();
        let keys = 2 + 2 * 14 + 2 * 64;
        let first: [u8; 32] = changed[keys..keys + 32].try_into().unwrap();
        changed[keys + 32..keys + 64].copy_from_slice(&first);
        assert_eq!(check(&changed), Err(WireError::DuplicatePublicKey));
    }

    #[test]
    fn message_sizes_feed_ids_and_signed_minimum_samples_are_checked() {
        let data = fixture(2, 2);
        for size in [0, 31, 32, 33, 80, 82, 32 + 9 * 49, usize::from(u16::MAX)] {
            let mut changed = data.clone();
            for record in 0..2 {
                set_u16(&mut changed, 2 + record * 14 + 10, size);
            }
            assert!(check(&changed).is_err(), "size {size}");
        }
        let mut changed = data.clone();
        let message = usize::from(read_u16(&changed, 2 + 8).unwrap());
        let first: [u8; 32] = changed[message + 32..message + 64].try_into().unwrap();
        changed[message + 32 + 49..message + 64 + 49].copy_from_slice(&first);
        assert_eq!(check(&changed), Err(WireError::DuplicateFeedId));
        for samples in [0, 3, u8::MAX] {
            let mut changed = data.clone();
            changed[message + 32 + 48] = samples;
            assert_eq!(check(&changed), Err(WireError::SignedMinimumSamples));
        }
        let mut changed = data;
        set_u16(&mut changed, 2 + 14 + 8, message + 1);
        assert!(matches!(
            check(&changed),
            Err(WireError::MessageMismatch | WireError::OffsetBounds)
        ));
    }

    #[test]
    fn local_regions_cannot_alias_table_suffix_message_keys_or_signatures() {
        let data = fixture(2, 1);
        let table_end = 2 + 2 * 14;
        let message = usize::from(read_u16(&data, 2 + 8).unwrap());
        let key = usize::from(read_u16(&data, 2 + 4).unwrap());
        let suffix = data.len() - 2 - 13;
        for (field, offset) in [
            (0, 0),
            (0, table_end - 1),
            (0, suffix - 63),
            (0, usize::from(u16::MAX)),
            (4, 0),
            (4, table_end),
            (4, message),
            (4, message - 31),
            (4, suffix),
            (8, 0),
            (8, table_end),
            (8, suffix),
        ] {
            let mut changed = data.clone();
            set_u16(&mut changed, 2 + field, offset);
            assert!(check(&changed).is_err(), "field {field} offset {offset}");
        }
        let mut changed = data.clone();
        set_u16(&mut changed, 2 + 14, table_end);
        assert_eq!(check(&changed), Err(WireError::Overlap));
        let mut changed = data;
        set_u16(&mut changed, 2 + 14 + 4, key);
        assert_eq!(check(&changed), Err(WireError::Overlap));
    }

    #[test]
    fn layouts_with_gaps_and_different_region_order_remain_local_and_bounded() {
        let mut data = fixture(1, 1);
        let message_start = usize::from(read_u16(&data, 2 + 8).unwrap());
        let key_start = usize::from(read_u16(&data, 2 + 4).unwrap());
        let signature_start = usize::from(read_u16(&data, 2).unwrap());
        let signature = data[signature_start..signature_start + 64].to_vec();
        let key = data[key_start..key_start + 32].to_vec();
        data[signature_start..signature_start + 32].copy_from_slice(&key);
        data[signature_start + 32..message_start].copy_from_slice(&signature);
        set_u16(&mut data, 2, signature_start + 32);
        set_u16(&mut data, 2 + 4, signature_start);
        assert!(check(&data).is_ok());
        data.splice(message_start..message_start, [0, 0, 0]);
        set_u16(&mut data, 2 + 8, message_start + 3);
        assert!(check(&data).is_ok());
    }

    #[test]
    fn caller_resource_limits_are_explicit_and_enforced() {
        let data = fixture(2, 2);
        let mut exact = limits();
        exact.maximum_data_length = data.len();
        assert!(prevalidate_ed25519(&data, SELECTED_INDEX, VERSION, exact).is_ok());
        exact.maximum_data_length -= 1;
        assert_eq!(
            prevalidate_ed25519(&data, SELECTED_INDEX, VERSION, exact),
            Err(WireError::DataLimit)
        );
        for (signatures, feeds) in [(0, 8), (9, 8), (8, 0), (8, 9)] {
            let mut invalid = limits();
            invalid.maximum_signatures = signatures;
            invalid.maximum_feeds = feeds;
            assert_eq!(
                prevalidate_ed25519(&data, SELECTED_INDEX, VERSION, invalid),
                Err(WireError::InvalidLimits)
            );
        }
        for size in [MIN_ENVELOPE_LEN - 1, usize::from(u16::MAX) + 1] {
            let mut invalid = limits();
            invalid.maximum_data_length = size;
            assert_eq!(
                prevalidate_ed25519(&data, SELECTED_INDEX, VERSION, invalid),
                Err(WireError::InvalidLimits)
            );
        }
        let mut restricted = limits();
        restricted.maximum_signatures = 1;
        assert_eq!(
            prevalidate_ed25519(&data, SELECTED_INDEX, VERSION, restricted),
            Err(WireError::SignatureCount)
        );
        restricted = limits();
        restricted.maximum_feeds = 1;
        assert_eq!(
            prevalidate_ed25519(&data, SELECTED_INDEX, VERSION, restricted),
            Err(WireError::MessageSize)
        );
    }

    #[test]
    fn arbitrary_and_structured_mutations_do_not_panic() {
        let mut state = 0x2358_1321_9a86_1034u64;
        for length in 0..=2048 {
            let mut data = vec![0; length];
            for byte in &mut data {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = state.to_le_bytes()[0];
            }
            let _ = prevalidate_ed25519(&data, SELECTED_INDEX, VERSION, limits());
        }
        let data = fixture(2, 2);
        for index in 0..data.len() {
            for value in 0..=u8::MAX {
                let mut changed = data.clone();
                changed[index] = value;
                let _ = check(&changed);
            }
        }
    }
}
