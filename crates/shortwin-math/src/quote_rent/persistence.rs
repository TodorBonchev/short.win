//! Canonical, versioned persistence for quote-native rent only.
//!
//! The distinct magic and exact lengths prevent accidental decoding of legacy rent
//! fields or a different record kind. They are not a signature, account owner/PDA
//! check or migration authorization. A caller must authenticate storage, bind the
//! market/position/model, retain immutable opening terms, and prove membership.
//! No amount of structural validation proves that an aggregate equals its real
//! live positions. Carry remains quote-native and survives an empty aggregate.
use super::{
    Accrual, Basis, LenderCarry, LockedRates, NOTIONAL_RENT_VERSION, NotionalAggregate,
    NotionalPosition, OpeningNotional, accrue,
};
use crate::error::MathError;

const MARKET_MAGIC: [u8; 8] = *b"QNRENTM1";
const POSITION_MAGIC: [u8; 8] = *b"QNRENTP1";
pub const MARKET_ENCODED_LEN: usize = 159;
pub const POSITION_ENCODED_LEN: usize = 94;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Length,
    RecordKind,
    Version,
    NonCanonical,
    InvalidState(MathError),
}
impl From<MathError> for Error {
    fn from(value: MathError) -> Self {
        Self::InvalidState(value)
    }
}

/// One checkpoint of the versioned quote-native market rent backend. The timestamp
/// must originate from the real runtime; codecs cannot authenticate its source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarketCheckpoint {
    basis: Basis,
    aggregate: NotionalAggregate,
    carry: LenderCarry,
    accrued_through: u64,
}
impl MarketCheckpoint {
    /// Only initialize a new quote-native backend; this does not adopt old carry,
    /// convert legacy obligations or authorize switching an existing market.
    pub const fn empty(accrued_through: u64) -> Self {
        Self {
            basis: Basis::zero(),
            aggregate: NotionalAggregate::empty(),
            carry: LenderCarry::zero(),
            accrued_through,
        }
    }
    pub fn from_parts(
        basis: Basis,
        aggregate: NotionalAggregate,
        carry: LenderCarry,
        accrued_through: u64,
    ) -> Result<Self, MathError> {
        Basis::from_indices(basis.indices())?;
        NotionalAggregate::from_parts(
            aggregate.folded_lender_weights(),
            aggregate.positions(),
            aggregate.opening_notional_total(),
            aggregate.kink_bps(),
        )?;
        LenderCarry::from_numerator(carry.numerator())?;
        Ok(Self {
            basis,
            aggregate,
            carry,
            accrued_through,
        })
    }
    pub const fn basis(self) -> Basis {
        self.basis
    }
    pub const fn aggregate(self) -> NotionalAggregate {
        self.aggregate
    }
    pub const fn carry(self) -> LenderCarry {
        self.carry
    }
    pub const fn accrued_through(self) -> u64 {
        self.accrued_through
    }
    /// Derive elapsed time from the persisted timestamp, using pre-instruction
    /// utilization and the locked cohort kink. No quote/history is consumed. On
    /// error all checkpoint fields remain unchanged. The caller must atomically
    /// book the returned lender credit and persist this state with pool::accrue.
    pub fn accrue_through(
        &mut self,
        now: u64,
        utilization_bps: u32,
        kink_bps: u32,
    ) -> Result<Accrual, MathError> {
        let elapsed = now
            .checked_sub(self.accrued_through)
            .ok_or(MathError::Underflow)?;
        let result = accrue(
            &mut self.basis,
            &self.aggregate,
            &mut self.carry,
            utilization_bps,
            kink_bps,
            elapsed,
        )?;
        self.accrued_through = now;
        Ok(result)
    }
}

struct Writer<'a> {
    bytes: &'a mut [u8],
    offset: usize,
}
impl Writer<'_> {
    fn put(&mut self, value: &[u8]) -> Result<(), Error> {
        let end = self.offset.checked_add(value.len()).ok_or(Error::Length)?;
        self.bytes
            .get_mut(self.offset..end)
            .ok_or(Error::Length)?
            .copy_from_slice(value);
        self.offset = end;
        Ok(())
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let end = self.offset.checked_add(N).ok_or(Error::Length)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Error::Length)?
            .try_into()
            .map_err(|_| Error::Length)?;
        self.offset = end;
        Ok(value)
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.take()?))
    }
    fn u128(&mut self) -> Result<u128, Error> {
        Ok(u128::from_le_bytes(self.take()?))
    }
    fn header(&mut self, magic: [u8; 8]) -> Result<(), Error> {
        if self.take::<8>()? != magic {
            return Err(Error::RecordKind);
        }
        if u16::from_le_bytes(self.take()?) != NOTIONAL_RENT_VERSION {
            return Err(Error::Version);
        }
        Ok(())
    }
    fn basis(&mut self) -> Result<Basis, Error> {
        Ok(Basis::from_indices([
            self.u128()?,
            self.u128()?,
            self.u128()?,
        ])?)
    }
}

pub fn encode_market(state: &MarketCheckpoint, bytes: &mut [u8]) -> Result<(), Error> {
    if bytes.len() != MARKET_ENCODED_LEN {
        return Err(Error::Length);
    }
    // Preflight everything before writing; errors never partially replace storage.
    MarketCheckpoint::from_parts(
        state.basis,
        state.aggregate,
        state.carry,
        state.accrued_through,
    )?;
    let mut out = Writer { bytes, offset: 0 };
    out.put(&MARKET_MAGIC)?;
    out.put(&NOTIONAL_RENT_VERSION.to_le_bytes())?;
    for value in state.basis.indices() {
        out.put(&value.to_le_bytes())?;
    }
    for value in state.aggregate.folded_lender_weights() {
        out.put(&value.to_le_bytes())?;
    }
    out.put(&state.aggregate.positions().to_le_bytes())?;
    out.put(&state.aggregate.opening_notional_total().to_le_bytes())?;
    let kink = state.aggregate.kink_bps();
    out.put(&[u8::from(kink.is_some())])?;
    out.put(&kink.unwrap_or(0).to_le_bytes())?;
    out.put(&state.carry.numerator().to_le_bytes())?;
    out.put(&state.accrued_through.to_le_bytes())?;
    Ok(())
}

pub fn decode_market(bytes: &[u8]) -> Result<MarketCheckpoint, Error> {
    if bytes.len() != MARKET_ENCODED_LEN {
        return Err(Error::Length);
    }
    let mut input = Reader { bytes, offset: 0 };
    input.header(MARKET_MAGIC)?;
    let basis = input.basis()?;
    let weights = [input.u128()?, input.u128()?, input.u128()?];
    let count = input.u64()?;
    let total = input.u128()?;
    let tag = input.take::<1>()?[0];
    let value = input.u32()?;
    let kink = match (tag, value) {
        (0, 0) => None,
        (1, value) => Some(value),
        _ => return Err(Error::NonCanonical),
    };
    let aggregate = NotionalAggregate::from_parts(weights, count, total, kink)?;
    let carry = LenderCarry::from_numerator(input.u128()?)?;
    let accrued_through = input.u64()?;
    Ok(MarketCheckpoint::from_parts(
        basis,
        aggregate,
        carry,
        accrued_through,
    )?)
}

pub fn encode_position(state: &NotionalPosition, bytes: &mut [u8]) -> Result<(), Error> {
    if bytes.len() != POSITION_ENCODED_LEN {
        return Err(Error::Length);
    }
    NotionalPosition::from_parts(
        state.opening_notional(),
        state.opening_basis(),
        state.rates(),
        state.kink_bps(),
        state.protocol_bps(),
        state.minimum_rent(),
    )?;
    let mut out = Writer { bytes, offset: 0 };
    out.put(&POSITION_MAGIC)?;
    out.put(&NOTIONAL_RENT_VERSION.to_le_bytes())?;
    out.put(&state.opening_notional().lamports().to_le_bytes())?;
    for value in state.opening_basis().indices() {
        out.put(&value.to_le_bytes())?;
    }
    for value in state.rates().coefficients() {
        out.put(&value.to_le_bytes())?;
    }
    out.put(&state.kink_bps().to_le_bytes())?;
    out.put(&state.protocol_bps().to_le_bytes())?;
    out.put(&state.minimum_rent().to_le_bytes())?;
    Ok(())
}

pub fn decode_position(bytes: &[u8]) -> Result<NotionalPosition, Error> {
    if bytes.len() != POSITION_ENCODED_LEN {
        return Err(Error::Length);
    }
    let mut input = Reader { bytes, offset: 0 };
    input.header(POSITION_MAGIC)?;
    let notional = OpeningNotional::new(input.u64()?)?;
    let basis = input.basis()?;
    let rates = LockedRates::new(input.u32()?, input.u32()?, input.u32()?);
    let kink = input.u32()?;
    let protocol = input.u32()?;
    let minimum_rent = input.u64()?;
    Ok(NotionalPosition::from_parts(
        notional,
        basis,
        rates,
        kink,
        protocol,
        minimum_rent,
    )?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::num::WAD;
    use crate::quote_rent::{LENDER_DENOMINATOR, YEAR_SECONDS, due};

    fn position(basis: &Basis, n: u64, kink: u32) -> NotionalPosition {
        NotionalPosition::lock(
            OpeningNotional::new(n).unwrap(),
            basis,
            LockedRates::new(123, 4567, 98765),
            1500,
            8000,
            kink,
            17,
        )
        .unwrap()
    }
    fn roundtrip_market(state: MarketCheckpoint) -> MarketCheckpoint {
        let mut bytes = [0; MARKET_ENCODED_LEN];
        encode_market(&state, &mut bytes).unwrap();
        let restored = decode_market(&bytes).unwrap();
        assert_eq!(state, restored);
        restored
    }
    fn roundtrip_position(state: NotionalPosition) -> NotionalPosition {
        let mut bytes = [0; POSITION_ENCODED_LEN];
        encode_position(&state, &mut bytes).unwrap();
        let restored = decode_position(&bytes).unwrap();
        assert_eq!(state, restored);
        restored
    }
    #[test]
    fn golden_little_endian_records_are_distinct_and_exact() {
        let mut bytes = [0xcc; MARKET_ENCODED_LEN];
        encode_market(&MarketCheckpoint::empty(0x0807060504030201), &mut bytes).unwrap();
        let mut expected = [0; MARKET_ENCODED_LEN];
        expected[..8].copy_from_slice(b"QNRENTM1");
        expected[8..10].copy_from_slice(&[1, 0]);
        expected[151..].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(bytes, expected);
        let p = NotionalPosition::from_parts(
            OpeningNotional::new(0x0807060504030201).unwrap(),
            Basis::zero(),
            LockedRates::new(0x04030201, 0x08070605, 0x0c0b0a09),
            8000,
            1500,
            0x100f0e0d0c0b0a09,
        )
        .unwrap();
        let mut bytes = [0; POSITION_ENCODED_LEN];
        encode_position(&p, &mut bytes).unwrap();
        let mut expected = [0; POSITION_ENCODED_LEN];
        expected[..8].copy_from_slice(b"QNRENTP1");
        expected[8..10].copy_from_slice(&[1, 0]);
        expected[10..18].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        expected[66..78].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        expected[78..82].copy_from_slice(&8000u32.to_le_bytes());
        expected[82..86].copy_from_slice(&1500u32.to_le_bytes());
        expected[86..94].copy_from_slice(&[9, 10, 11, 12, 13, 14, 15, 16]);
        assert_eq!(bytes, expected);
        assert_eq!(decode_position(&bytes), Ok(p));
    }
    #[test]
    fn rejects_every_truncation_extension_wrong_kind_and_version() {
        let market = MarketCheckpoint::empty(0);
        let mut m = [0; MARKET_ENCODED_LEN];
        encode_market(&market, &mut m).unwrap();
        for end in 0..m.len() {
            assert_eq!(decode_market(&m[..end]), Err(Error::Length));
        }
        let mut long = std::vec::Vec::from(m);
        long.push(0);
        assert_eq!(decode_market(&long), Err(Error::Length));
        let p = position(&Basis::zero(), 1000, 8000);
        let mut b = [0; POSITION_ENCODED_LEN];
        encode_position(&p, &mut b).unwrap();
        for end in 0..b.len() {
            assert_eq!(decode_position(&b[..end]), Err(Error::Length));
        }
        let mut long = std::vec::Vec::from(b);
        long.push(0);
        assert_eq!(decode_position(&long), Err(Error::Length));
        for bytes in [&mut m[..], &mut b[..]] {
            let saved = bytes[0];
            bytes[0] ^= 0xff;
            if bytes.len() == MARKET_ENCODED_LEN {
                assert_eq!(decode_market(bytes), Err(Error::RecordKind));
            } else {
                assert_eq!(decode_position(bytes), Err(Error::RecordKind));
            }
            bytes[0] = saved;
            bytes[8] = 2;
            if bytes.len() == MARKET_ENCODED_LEN {
                assert_eq!(decode_market(bytes), Err(Error::Version));
            } else {
                assert_eq!(decode_position(bytes), Err(Error::Version));
            }
        }
        let mut untouched = [0xaa; 7];
        assert_eq!(encode_market(&market, &mut untouched), Err(Error::Length));
        assert_eq!(untouched, [0xaa; 7]);
        assert_eq!(encode_position(&p, &mut untouched), Err(Error::Length));
        assert_eq!(untouched, [0xaa; 7]);
    }
    #[test]
    fn malformed_checkpoint_rejects_ghost_weights_bad_counts_carry_and_option() {
        let state = MarketCheckpoint::empty(100);
        let mut valid = [0; MARKET_ENCODED_LEN];
        encode_market(&state, &mut valid).unwrap();
        for (offset, value) in [(10, 1), (58, 1), (106, 1), (114, 1), (130, 2), (131, 1)] {
            let mut bytes = valid;
            bytes[offset] = value;
            assert!(decode_market(&bytes).is_err(), "offset {offset}");
        }
        let mut bytes = valid;
        bytes[135..151].copy_from_slice(&LENDER_DENOMINATOR.to_le_bytes());
        assert!(decode_market(&bytes).is_err());
        assert!(NotionalAggregate::from_parts([0; 3], 2, 1, Some(8000)).is_err());
        assert!(
            NotionalAggregate::from_parts([0; 3], 1, u128::from(u64::MAX) + 1, Some(8000)).is_err()
        );
        assert!(NotionalAggregate::from_parts([0; 3], 1, 1, None).is_err());
        assert!(NotionalAggregate::from_parts([0; 3], 1, 1, Some(10001)).is_err());
        assert!(NotionalAggregate::from_parts([u128::MAX; 3], 1, 1, Some(8000)).is_err());
        assert!(NotionalAggregate::from_parts([1, 0, 0], 1, 2, Some(8000)).is_err());
        // A fully protocol-directed or zero-rate cohort may legitimately have no lender weight.
        assert!(NotionalAggregate::from_parts([0; 3], 1, 1, Some(8000)).is_ok());
    }
    #[test]
    fn malformed_position_rejects_zero_notional_impossible_basis_and_out_of_range_terms() {
        let p = position(&Basis::zero(), 1000, 8000);
        let mut valid = [0; POSITION_ENCODED_LEN];
        encode_position(&p, &mut valid).unwrap();
        let mut bytes = valid;
        bytes[10..18].fill(0);
        assert!(decode_position(&bytes).is_err());
        let mut bytes = valid;
        bytes[18] = 1;
        assert!(decode_position(&bytes).is_err());
        let mut bytes = valid;
        bytes[78..82].copy_from_slice(&10001u32.to_le_bytes());
        assert!(decode_position(&bytes).is_err());
        let mut bytes = valid;
        bytes[82..86].copy_from_slice(&10001u32.to_le_bytes());
        assert!(decode_position(&bytes).is_err());
        let open = Basis::from_indices([WAD, 0, 0]).unwrap();
        let p = position(&open, 1000, 8000);
        assert_eq!(
            p.validate_at_basis(&Basis::zero()),
            Err(MathError::Underflow)
        );
        let impossible = Basis::from_indices([WAD, WAD, 0]).unwrap();
        assert_eq!(
            p.validate_at_basis(&impossible),
            Err(MathError::InvalidInput)
        );
    }
    #[test]
    fn persisted_carry_survives_empty_intervals_and_new_kink_cohort() {
        let mut uninterrupted = MarketCheckpoint::empty(100);
        let p = position(&uninterrupted.basis(), 1_000_000_007, 8000);
        let mut aggregate = uninterrupted.aggregate();
        aggregate.add(&p).unwrap();
        uninterrupted.aggregate = aggregate;
        uninterrupted.accrue_through(101, 7919, 8000).unwrap();
        assert_ne!(uninterrupted.carry().numerator(), 0);
        let mut restored = roundtrip_market(uninterrupted);
        aggregate.remove(&p).unwrap();
        uninterrupted.aggregate = aggregate;
        restored.aggregate = roundtrip_market(
            MarketCheckpoint::from_parts(
                restored.basis(),
                aggregate,
                restored.carry(),
                restored.accrued_through(),
            )
            .unwrap(),
        )
        .aggregate();
        let carry = restored.carry();
        assert_eq!(
            restored
                .accrue_through(10_000, 10000, 7000)
                .unwrap()
                .lender_lamports,
            0
        );
        uninterrupted.accrue_through(10_000, 10000, 7000).unwrap();
        assert_eq!(restored.carry(), carry);
        assert_eq!(restored, uninterrupted);
        let p2 = roundtrip_position(position(&restored.basis(), 5_000_000_009, 7000));
        aggregate.add(&p2).unwrap();
        restored.aggregate = aggregate;
        uninterrupted.aggregate = aggregate;
        restored = roundtrip_market(restored);
        let a = restored.accrue_through(20_000, 9876, 7000).unwrap();
        let b = uninterrupted.accrue_through(20_000, 9876, 7000).unwrap();
        assert_eq!(a, b);
        assert_eq!(restored, uninterrupted);
        assert_eq!(
            due(&p2, &restored.basis()),
            due(&p2, &uninterrupted.basis())
        );
    }
    #[test]
    fn seeded_membership_transitions_roundtrip_without_losing_fractional_credit() {
        let mut state = MarketCheckpoint::empty(0);
        let mut active = std::vec::Vec::new();
        let mut restored = state;
        let mut now = 0;
        let mut total_credit = 0;
        let mut restored_credit = 0;
        for step in 0..200u32 {
            now += u64::from(step % 13);
            total_credit += state
                .accrue_through(now, step * 7919 % 10001, 8000)
                .unwrap()
                .lender_lamports;
            restored_credit += restored
                .accrue_through(now, step * 7919 % 10001, 8000)
                .unwrap()
                .lender_lamports;
            if step % 3 == 0 {
                let p = position(
                    &state.basis(),
                    1_000_000_007 + u64::from(step) * 1_000_000,
                    8000,
                );
                state.aggregate.add(&p).unwrap();
                restored.aggregate.add(&roundtrip_position(p)).unwrap();
                active.push(p);
            }
            if step % 2 == 0 && !active.is_empty() {
                let p = active.remove(0);
                state.aggregate.remove(&p).unwrap();
                restored.aggregate.remove(&roundtrip_position(p)).unwrap();
            }
            restored = roundtrip_market(restored);
            assert_eq!(restored, state);
            assert_eq!(restored_credit, total_credit);
            for p in &active {
                assert!(p.validate_at_basis(&restored.basis()).is_ok());
                assert_eq!(due(p, &state.basis()), due(p, &restored.basis()));
            }
        }
    }
    #[test]
    fn timestamp_regression_kink_mismatch_and_overflow_preserve_whole_checkpoint() {
        let mut state = MarketCheckpoint::empty(100);
        let p = position(&state.basis(), u64::MAX, 8000);
        state.aggregate.add(&p).unwrap();
        for (now, util, kink) in [
            (99, 0, 8000),
            (101, 10001, 8000),
            (101, 0, 7000),
            (u64::MAX, 10000, 8000),
        ] {
            let saved = state;
            assert!(state.accrue_through(now, util, kink).is_err());
            assert_eq!(state, saved);
        }
        state.accrue_through(100 + YEAR_SECONDS, 0, 8000).unwrap();
        assert_eq!(roundtrip_market(state), state);
    }
    #[test]
    fn final_removal_cannot_leave_ghost_aggregate_or_mutate_on_refusal() {
        let p = position(&Basis::zero(), 1000, 8000);
        let fake = NotionalPosition::from_parts(
            p.opening_notional(),
            p.opening_basis(),
            LockedRates::new(1, 1, 1),
            p.kink_bps(),
            p.protocol_bps(),
            p.minimum_rent(),
        )
        .unwrap();
        let mut aggregate = NotionalAggregate::empty();
        aggregate.add(&p).unwrap();
        let saved = aggregate;
        assert_eq!(aggregate.remove(&fake), Err(MathError::InvalidInput));
        assert_eq!(aggregate, saved);
        aggregate.remove(&p).unwrap();
        assert_eq!(aggregate, NotionalAggregate::empty());
    }
}
