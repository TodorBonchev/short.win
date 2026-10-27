//! Jupiter V2 scalar headers and fixed-account bindings; no route or CPI authorization.
//!
//! The deployed IDL identity and the deliberately incomplete validation contract are in
//! the crate README. A ValidatedHeader is not a validated route, risk price, signature,
//! token ownership proof, spend/output guarantee, or permission to execute a CPI.
//! The route plan and remaining accounts stay opaque and need a separate authorization
//! boundary plus independent pre/post balance checks before production integration.
#![no_std]
#![forbid(unsafe_code)]

/// Additional conservative checks for the isolated test executor; not production authorization.
pub mod isolation;

/// A Solana address, without a runtime or SDK dependency.
pub type Address = [u8; 32];

/// Current pinned Jupiter program; the caller must separately enforce executable identity.
pub const JUPITER_PROGRAM: Address = [
    4, 121, 213, 91, 242, 49, 192, 110, 238, 116, 197, 110, 206, 104, 21, 7, 253, 177, 178, 222,
    163, 244, 142, 81, 2, 177, 205, 162, 86, 188, 19, 143,
];
/// Fixed event authority in the pinned on-chain IDL.
pub const EVENT_AUTHORITY: Address = [
    180, 63, 250, 39, 245, 215, 246, 74, 116, 192, 155, 31, 41, 88, 121, 222, 75, 9, 171, 54, 223,
    201, 221, 81, 75, 50, 26, 167, 179, 140, 229, 232,
];
pub const SPL_TOKEN_PROGRAM: Address = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
];
pub const TOKEN_2022_PROGRAM: Address = [
    6, 221, 246, 225, 238, 117, 143, 222, 24, 66, 93, 188, 228, 108, 205, 218, 182, 26, 252, 77,
    131, 185, 13, 39, 254, 189, 249, 40, 216, 161, 139, 252,
];

pub const ROUTE_V2: [u8; 8] = [187, 100, 250, 204, 49, 196, 175, 20];
pub const SHARED_ACCOUNTS_ROUTE_V2: [u8; 8] = [209, 152, 83, 147, 124, 254, 216, 233];
pub const EXACT_OUT_ROUTE_V2: [u8; 8] = [157, 138, 184, 82, 21, 244, 243, 36];
pub const SHARED_ACCOUNTS_EXACT_OUT_ROUTE_V2: [u8; 8] = [53, 96, 229, 202, 216, 187, 250, 24];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteKind {
    ExactIn,
    SharedExactIn,
    ExactOut,
    SharedExactOut,
}

impl RouteKind {
    pub const fn is_shared(self) -> bool {
        matches!(self, Self::SharedExactIn | Self::SharedExactOut)
    }

    pub const fn is_exact_out(self) -> bool {
        matches!(self, Self::ExactOut | Self::SharedExactOut)
    }

    pub const fn fixed_account_count(self) -> usize {
        if self.is_shared() { 12 } else { 10 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TruncatedHeader,
    UnknownDiscriminator,
    EmptyRoutePlan,
    TruncatedRoutePlan,
    LengthOverflow,
    InvalidLimits,
    DataLimit,
    RouteStepLimit,
    AccountCount,
    WrongProgram,
    InvalidExpectations,
    UnsupportedTokenProgram,
    WrongAmountMode,
    AmountOutOfRange,
    QuotedAmountOutOfRange,
    SlippageLimit,
    NonzeroFee,
    SharedModeMismatch,
    SharedIdMismatch,
    WrongAccount { index: usize },
    MissingSigner { index: usize },
    UnexpectedSigner { index: usize },
    MissingWritable { index: usize },
}

/// Parsed scalar fields with a borrowed, undecoded Borsh route tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedHeader<'a> {
    kind: RouteKind,
    shared_id: Option<u8>,
    amount: u64,
    quoted_amount: u64,
    slippage_bps: u16,
    platform_fee_bps: u16,
    positive_slippage_bps: u16,
    declared_route_steps: u32,
    opaque_route_plan: &'a [u8],
}

impl<'a> ParsedHeader<'a> {
    /// Parses only a pinned V2 header and checks a necessary minimum tail length.
    ///
    /// Each RoutePlanStepV2 has at least one Swap enum tag, u16 bps, and two u8
    /// indices (five bytes). Variable payloads, enum tags, step count consistency,
    /// sums, indices and trailing bytes are NOT decoded or authorized.
    pub fn parse(data: &'a [u8]) -> Result<Self, Error> {
        let discriminator = data.get(..8).ok_or(Error::TruncatedHeader)?;
        let kind = match discriminator {
            x if x == ROUTE_V2 => RouteKind::ExactIn,
            x if x == SHARED_ACCOUNTS_ROUTE_V2 => RouteKind::SharedExactIn,
            x if x == EXACT_OUT_ROUTE_V2 => RouteKind::ExactOut,
            x if x == SHARED_ACCOUNTS_EXACT_OUT_ROUTE_V2 => RouteKind::SharedExactOut,
            _ => return Err(Error::UnknownDiscriminator),
        };
        let scalar = if kind.is_shared() { 9 } else { 8 };
        let header_len = scalar + 26;
        if data.len() < header_len {
            return Err(Error::TruncatedHeader);
        }
        let amount = read_u64(data, scalar);
        let quoted_amount = read_u64(data, scalar + 8);
        let slippage_bps = read_u16(data, scalar + 16);
        let platform_fee_bps = read_u16(data, scalar + 18);
        let positive_slippage_bps = read_u16(data, scalar + 20);
        let declared_route_steps = u32::from_le_bytes([
            data[scalar + 22],
            data[scalar + 23],
            data[scalar + 24],
            data[scalar + 25],
        ]);
        if declared_route_steps == 0 {
            return Err(Error::EmptyRoutePlan);
        }
        let minimum_tail = usize::try_from(declared_route_steps)
            .map_err(|_| Error::LengthOverflow)?
            .checked_mul(5)
            .ok_or(Error::LengthOverflow)?;
        let opaque_route_plan = &data[header_len..];
        if opaque_route_plan.len() < minimum_tail {
            return Err(Error::TruncatedRoutePlan);
        }
        Ok(Self {
            kind,
            shared_id: if kind.is_shared() {
                Some(data[8])
            } else {
                None
            },
            amount,
            quoted_amount,
            slippage_bps,
            platform_fee_bps,
            positive_slippage_bps,
            declared_route_steps,
            opaque_route_plan,
        })
    }

    pub const fn kind(&self) -> RouteKind {
        self.kind
    }
    pub const fn shared_id(&self) -> Option<u8> {
        self.shared_id
    }
    /// in_amount for ExactIn; out_amount for ExactOut.
    pub const fn amount(&self) -> u64 {
        self.amount
    }
    /// quoted_out_amount for ExactIn; quoted_in_amount for ExactOut.
    pub const fn quoted_amount(&self) -> u64 {
        self.quoted_amount
    }
    pub const fn slippage_bps(&self) -> u16 {
        self.slippage_bps
    }
    pub const fn platform_fee_bps(&self) -> u16 {
        self.platform_fee_bps
    }
    pub const fn positive_slippage_bps(&self) -> u16 {
        self.positive_slippage_bps
    }
    pub const fn declared_route_steps(&self) -> u32 {
        self.declared_route_steps
    }
    pub const fn opaque_route_plan(&self) -> &'a [u8] {
        self.opaque_route_plan
    }
}

fn read_u16(data: &[u8], start: usize) -> u16 {
    u16::from_le_bytes([data[start], data[start + 1]])
}

fn read_u64(data: &[u8], start: usize) -> u64 {
    u64::from_le_bytes([
        data[start],
        data[start + 1],
        data[start + 2],
        data[start + 3],
        data[start + 4],
        data[start + 5],
        data[start + 6],
        data[start + 7],
    ])
}

/// Proposed instruction metas, not live AccountInfo privileges, owners or signature proofs.
///
/// Required writable and signer roles follow the IDL. Extra writability is not certified
/// safe here; the future full remaining-account policy must authorize it. Signers other
/// than the expected transfer authority are rejected, including in the opaque tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountMeta {
    pub key: Address,
    pub is_signer: bool,
    pub is_writable: bool,
}

/// Shared account identities must already be derived and checked by the trusted caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpectedSharedAccounts {
    pub id: u8,
    pub program_authority: Address,
    pub program_source_token_account: Address,
    pub program_destination_token_account: Address,
}

/// Trusted bindings, never copied unchecked from a route API response.
///
/// This crate compares keys only. The caller still needs mint/owner/extension checks,
/// correct authority derivation, shared PDA validation, and explicit alias/write policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpectedAccounts {
    pub transfer_authority: Address,
    pub source_token_account: Address,
    pub destination_token_account: Address,
    pub source_mint: Address,
    pub destination_mint: Address,
    pub source_token_program: Address,
    pub destination_token_program: Address,
    pub shared: Option<ExpectedSharedAccounts>,
}

/// Inclusive bounds on a scalar header field; both ends must be positive and ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmountRange {
    pub minimum: u64,
    pub maximum: u64,
}

impl AmountRange {
    fn valid(self) -> bool {
        self.minimum > 0 && self.minimum <= self.maximum
    }
    fn contains(self, value: u64) -> bool {
        value >= self.minimum && value <= self.maximum
    }
}

/// Scalar quote constraints, not execution thresholds or oracle prices.
///
/// In particular, max_quoted_input does not bound an ExactOut fill after slippage.
/// No private Jupiter rounding formula is assumed. Independent absolute spend/output
/// enforcement and balance accounting are required before any eventual CPI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmountExpectation {
    ExactIn {
        input: AmountRange,
        minimum_quoted_output: u64,
    },
    ExactOut {
        output: AmountRange,
        maximum_quoted_input: u64,
    },
}

/// Resource and slippage policy supplied by a future trusted integration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderLimits {
    pub maximum_data_length: usize,
    pub maximum_accounts: usize,
    pub maximum_route_steps: u32,
    /// Must be below 10000; an unconstrained zero-output slippage policy is rejected.
    pub maximum_slippage_bps: u16,
}

/// Certifies ONLY the scalar bounds and fixed key/meta checks in validate_header.
///
/// Opaque route bytes and remaining non-signer account privileges are not validated.
/// The type is intentionally not named AuthorizedRoute and cannot authorize a risk price.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedHeader<'a> {
    parsed: ParsedHeader<'a>,
}

impl<'a> ValidatedHeader<'a> {
    pub const fn parsed(&self) -> &ParsedHeader<'a> {
        &self.parsed
    }
}

/// Validates a pinned header and fixed-account bindings, with zero ancillary fee fields.
///
/// program_id is the proposed instruction target, not a check of deployed ELF identity.
/// All amounts and bindings must originate in a trusted policy independent of API JSON.
pub fn validate_header<'a>(
    program_id: &Address,
    data: &'a [u8],
    accounts: &[AccountMeta],
    expected: &ExpectedAccounts,
    amounts: AmountExpectation,
    limits: HeaderLimits,
) -> Result<ValidatedHeader<'a>, Error> {
    if *program_id != JUPITER_PROGRAM {
        return Err(Error::WrongProgram);
    }
    if limits.maximum_data_length < 39
        || limits.maximum_accounts < 10
        || limits.maximum_route_steps == 0
        || limits.maximum_slippage_bps >= 10_000
    {
        return Err(Error::InvalidLimits);
    }
    if data.len() > limits.maximum_data_length {
        return Err(Error::DataLimit);
    }
    let parsed = ParsedHeader::parse(data)?;
    if parsed.declared_route_steps > limits.maximum_route_steps {
        return Err(Error::RouteStepLimit);
    }
    if accounts.len() < parsed.kind.fixed_account_count()
        || accounts.len() > limits.maximum_accounts
    {
        return Err(Error::AccountCount);
    }
    if parsed.slippage_bps > limits.maximum_slippage_bps {
        return Err(Error::SlippageLimit);
    }
    if parsed.platform_fee_bps != 0 || parsed.positive_slippage_bps != 0 {
        return Err(Error::NonzeroFee);
    }
    let (range, quote_bound, exact_out) = match amounts {
        AmountExpectation::ExactIn {
            input,
            minimum_quoted_output,
        } => (input, minimum_quoted_output, false),
        AmountExpectation::ExactOut {
            output,
            maximum_quoted_input,
        } => (output, maximum_quoted_input, true),
    };
    if !range.valid() || quote_bound == 0 {
        return Err(Error::InvalidExpectations);
    }
    if parsed.kind.is_exact_out() != exact_out {
        return Err(Error::WrongAmountMode);
    }
    if !range.contains(parsed.amount) {
        return Err(Error::AmountOutOfRange);
    }
    if parsed.quoted_amount == 0
        || (exact_out && parsed.quoted_amount > quote_bound)
        || (!exact_out && parsed.quoted_amount < quote_bound)
    {
        return Err(Error::QuotedAmountOutOfRange);
    }
    if expected.source_mint == expected.destination_mint
        || expected.source_token_account == expected.destination_token_account
    {
        return Err(Error::InvalidExpectations);
    }
    for program in [
        expected.source_token_program,
        expected.destination_token_program,
    ] {
        if program != SPL_TOKEN_PROGRAM && program != TOKEN_2022_PROGRAM {
            return Err(Error::UnsupportedTokenProgram);
        }
    }
    if parsed.kind.is_shared() != expected.shared.is_some() {
        return Err(Error::SharedModeMismatch);
    }
    let authority_index = if let Some(shared) = expected.shared {
        if parsed.shared_id != Some(shared.id) {
            return Err(Error::SharedIdMismatch);
        }
        let keys = [
            shared.program_authority,
            expected.transfer_authority,
            expected.source_token_account,
            shared.program_source_token_account,
            shared.program_destination_token_account,
            expected.destination_token_account,
            expected.source_mint,
            expected.destination_mint,
            expected.source_token_program,
            expected.destination_token_program,
            EVENT_AUTHORITY,
            JUPITER_PROGRAM,
        ];
        for (index, key) in keys.iter().enumerate() {
            require_key(accounts, index, key)?;
        }
        for index in [2, 3, 4, 5] {
            require_writable(accounts, index)?;
        }
        1
    } else {
        let keys = [
            expected.transfer_authority,
            expected.source_token_account,
            expected.destination_token_account,
            expected.source_mint,
            expected.destination_mint,
            expected.source_token_program,
            expected.destination_token_program,
        ];
        for (index, key) in keys.iter().enumerate() {
            require_key(accounts, index, key)?;
        }
        // Anchor's optional None sentinel is the invoked program's key. Any actual
        // optional recipient is restricted to the same expected destination account.
        if accounts[7].key != JUPITER_PROGRAM {
            require_key(accounts, 7, &expected.destination_token_account)?;
            require_writable(accounts, 7)?;
        }
        require_key(accounts, 8, &EVENT_AUTHORITY)?;
        require_key(accounts, 9, &JUPITER_PROGRAM)?;
        for index in [1, 2] {
            require_writable(accounts, index)?;
        }
        0
    };
    if !accounts[authority_index].is_signer {
        return Err(Error::MissingSigner {
            index: authority_index,
        });
    }
    for (index, account) in accounts.iter().enumerate() {
        if account.is_signer
            && (account.key != expected.transfer_authority
                || (index < parsed.kind.fixed_account_count() && index != authority_index))
        {
            return Err(Error::UnexpectedSigner { index });
        }
    }
    Ok(ValidatedHeader { parsed })
}

fn require_key(accounts: &[AccountMeta], index: usize, key: &Address) -> Result<(), Error> {
    if accounts[index].key != *key {
        return Err(Error::WrongAccount { index });
    }
    Ok(())
}

fn require_writable(accounts: &[AccountMeta], index: usize) -> Result<(), Error> {
    if !accounts[index].is_writable {
        return Err(Error::MissingWritable { index });
    }
    Ok(())
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{vec, vec::Vec};

    fn expected(shared: bool) -> ExpectedAccounts {
        ExpectedAccounts {
            transfer_authority: [1; 32],
            source_token_account: [2; 32],
            destination_token_account: [3; 32],
            source_mint: [4; 32],
            destination_mint: [5; 32],
            source_token_program: SPL_TOKEN_PROGRAM,
            destination_token_program: TOKEN_2022_PROGRAM,
            shared: if shared {
                Some(ExpectedSharedAccounts {
                    id: 7,
                    program_authority: [6; 32],
                    program_source_token_account: [7; 32],
                    program_destination_token_account: [8; 32],
                })
            } else {
                None
            },
        }
    }

    fn limits() -> HeaderLimits {
        HeaderLimits {
            maximum_data_length: 1232,
            maximum_accounts: 64,
            maximum_route_steps: 8,
            maximum_slippage_bps: 100,
        }
    }

    fn amounts(exact_out: bool) -> AmountExpectation {
        let range = AmountRange {
            minimum: 100,
            maximum: 100,
        };
        if exact_out {
            AmountExpectation::ExactOut {
                output: range,
                maximum_quoted_input: 200,
            }
        } else {
            AmountExpectation::ExactIn {
                input: range,
                minimum_quoted_output: 200,
            }
        }
    }

    fn wire(kind: RouteKind) -> Vec<u8> {
        // Independent literal wire vectors from the decoded IDL, not a production encoder.
        let mut bytes = match kind {
            RouteKind::ExactIn => vec![187, 100, 250, 204, 49, 196, 175, 20],
            RouteKind::SharedExactIn => vec![209, 152, 83, 147, 124, 254, 216, 233, 7],
            RouteKind::ExactOut => vec![157, 138, 184, 82, 21, 244, 243, 36],
            RouteKind::SharedExactOut => vec![53, 96, 229, 202, 216, 187, 250, 24, 7],
        };
        bytes.extend_from_slice(&[100, 0, 0, 0, 0, 0, 0, 0]);
        bytes.extend_from_slice(&[200, 0, 0, 0, 0, 0, 0, 0]);
        // slippage 100; fee 0; positive-slippage fee 0; vector count 1;
        // Saber enum tag 0, bps 10000 LE, input index 0, output index 1.
        bytes.extend_from_slice(&[100, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 16, 39, 0, 1]);
        bytes
    }

    fn metas(shared: bool) -> Vec<AccountMeta> {
        let e = expected(shared);
        let keys = if let Some(s) = e.shared {
            vec![
                s.program_authority,
                e.transfer_authority,
                e.source_token_account,
                s.program_source_token_account,
                s.program_destination_token_account,
                e.destination_token_account,
                e.source_mint,
                e.destination_mint,
                e.source_token_program,
                e.destination_token_program,
                EVENT_AUTHORITY,
                JUPITER_PROGRAM,
            ]
        } else {
            vec![
                e.transfer_authority,
                e.source_token_account,
                e.destination_token_account,
                e.source_mint,
                e.destination_mint,
                e.source_token_program,
                e.destination_token_program,
                JUPITER_PROGRAM,
                EVENT_AUTHORITY,
                JUPITER_PROGRAM,
            ]
        };
        keys.into_iter()
            .enumerate()
            .map(|(i, key)| AccountMeta {
                key,
                is_signer: i == usize::from(shared),
                is_writable: if shared {
                    (2..=5).contains(&i)
                } else {
                    i == 1 || i == 2
                },
            })
            .collect()
    }

    fn check<'a>(
        kind: RouteKind,
        bytes: &'a [u8],
        accounts: &[AccountMeta],
    ) -> Result<ValidatedHeader<'a>, Error> {
        validate_header(
            &JUPITER_PROGRAM,
            bytes,
            accounts,
            &expected(kind.is_shared()),
            amounts(kind.is_exact_out()),
            limits(),
        )
    }

    #[test]
    fn four_current_idl_headers_and_exact_u64_fields() {
        for kind in [
            RouteKind::ExactIn,
            RouteKind::SharedExactIn,
            RouteKind::ExactOut,
            RouteKind::SharedExactOut,
        ] {
            let bytes = wire(kind);
            let parsed = check(kind, &bytes, &metas(kind.is_shared())).unwrap();
            assert_eq!(parsed.parsed().kind(), kind);
            assert_eq!(parsed.parsed().amount(), 100);
            assert_eq!(parsed.parsed().quoted_amount(), 200);
            assert_eq!(parsed.parsed().slippage_bps(), 100);
            assert_eq!(parsed.parsed().declared_route_steps(), 1);
            assert_eq!(parsed.parsed().opaque_route_plan(), [0, 16, 39, 0, 1]);
        }
    }

    #[test]
    fn every_truncated_header_and_tail_is_rejected_without_panics() {
        for kind in [
            RouteKind::ExactIn,
            RouteKind::SharedExactIn,
            RouteKind::ExactOut,
            RouteKind::SharedExactOut,
        ] {
            let bytes = wire(kind);
            for end in 0..bytes.len() {
                assert!(
                    ParsedHeader::parse(&bytes[..end]).is_err(),
                    "{kind:?} cut at {end}"
                );
            }
        }
        let mut bytes = wire(RouteKind::ExactIn);
        bytes[0] ^= 1;
        assert_eq!(
            ParsedHeader::parse(&bytes),
            Err(Error::UnknownDiscriminator)
        );
    }

    #[test]
    fn oversized_counts_lengths_and_u64_maximum_are_bounded() {
        let mut bytes = wire(RouteKind::ExactIn);
        bytes[30..34].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            ParsedHeader::parse(&bytes),
            Err(Error::TruncatedRoutePlan | Error::LengthOverflow)
        ));
        bytes[30..34].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(ParsedHeader::parse(&bytes), Err(Error::EmptyRoutePlan));
        let mut bytes = wire(RouteKind::ExactIn);
        bytes[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(ParsedHeader::parse(&bytes).unwrap().amount(), u64::MAX);
        assert_eq!(
            check(RouteKind::ExactIn, &bytes, &metas(false)),
            Err(Error::AmountOutOfRange)
        );
        let mut bytes = wire(RouteKind::ExactOut);
        bytes[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            check(RouteKind::ExactOut, &bytes, &metas(false)),
            Err(Error::QuotedAmountOutOfRange)
        );
        let mut bytes = wire(RouteKind::ExactIn);
        bytes.resize(1233, 0);
        assert_eq!(
            check(RouteKind::ExactIn, &bytes, &metas(false)),
            Err(Error::DataLimit)
        );
        let mut bytes = wire(RouteKind::ExactIn);
        bytes[30..34].copy_from_slice(&9u32.to_le_bytes());
        bytes.resize(34 + 9 * 5, 0);
        assert_eq!(
            check(RouteKind::ExactIn, &bytes, &metas(false)),
            Err(Error::RouteStepLimit)
        );
    }

    #[test]
    fn amount_mode_ranges_slippage_and_both_fees_are_checked() {
        let bytes = wire(RouteKind::ExactIn);
        let accounts = metas(false);
        assert_eq!(
            validate_header(
                &JUPITER_PROGRAM,
                &bytes,
                &accounts,
                &expected(false),
                amounts(true),
                limits()
            ),
            Err(Error::WrongAmountMode)
        );
        for (offset, error) in [
            (24, Error::SlippageLimit),
            (26, Error::NonzeroFee),
            (28, Error::NonzeroFee),
        ] {
            let mut bad = bytes.clone();
            bad[offset..offset + 2].copy_from_slice(&101u16.to_le_bytes());
            assert_eq!(check(RouteKind::ExactIn, &bad, &accounts), Err(error));
        }
        for value in [0u64, 199] {
            let mut bad = bytes.clone();
            bad[16..24].copy_from_slice(&value.to_le_bytes());
            assert_eq!(
                check(RouteKind::ExactIn, &bad, &accounts),
                Err(Error::QuotedAmountOutOfRange)
            );
        }
        let mut bad_limits = limits();
        bad_limits.maximum_slippage_bps = 10_000;
        assert_eq!(
            validate_header(
                &JUPITER_PROGRAM,
                &bytes,
                &accounts,
                &expected(false),
                amounts(false),
                bad_limits
            ),
            Err(Error::InvalidLimits)
        );
        let bad = AmountExpectation::ExactIn {
            input: AmountRange {
                minimum: 101,
                maximum: 100,
            },
            minimum_quoted_output: 200,
        };
        assert_eq!(
            validate_header(
                &JUPITER_PROGRAM,
                &bytes,
                &accounts,
                &expected(false),
                bad,
                limits()
            ),
            Err(Error::InvalidExpectations)
        );
    }

    #[test]
    fn every_fixed_key_is_bound_and_minimum_account_counts_hold() {
        for kind in [RouteKind::ExactIn, RouteKind::SharedExactOut] {
            let bytes = wire(kind);
            let accounts = metas(kind.is_shared());
            for index in 0..accounts.len() {
                let mut bad = accounts.clone();
                bad[index].key = [99; 32];
                assert_eq!(
                    check(kind, &bytes, &bad),
                    Err(Error::WrongAccount { index })
                );
            }
            for count in 0..accounts.len() {
                assert_eq!(
                    check(kind, &bytes, &accounts[..count]),
                    Err(Error::AccountCount)
                );
            }
            let mut excess = accounts.clone();
            excess.resize(
                65,
                AccountMeta {
                    key: [99; 32],
                    is_signer: false,
                    is_writable: false,
                },
            );
            assert_eq!(check(kind, &bytes, &excess), Err(Error::AccountCount));
            assert_eq!(
                validate_header(
                    &[99; 32],
                    &bytes,
                    &accounts,
                    &expected(kind.is_shared()),
                    amounts(kind.is_exact_out()),
                    limits()
                ),
                Err(Error::WrongProgram)
            );
        }
    }

    #[test]
    fn recipient_signer_and_writable_substitution_is_rejected() {
        let kind = RouteKind::ExactIn;
        let bytes = wire(kind);
        let mut accounts = metas(false);
        accounts[7].key = expected(false).destination_token_account;
        assert_eq!(
            check(kind, &bytes, &accounts),
            Err(Error::MissingWritable { index: 7 })
        );
        accounts[7].is_writable = true;
        assert!(check(kind, &bytes, &accounts).is_ok());
        accounts[7].key = [99; 32];
        assert_eq!(
            check(kind, &bytes, &accounts),
            Err(Error::WrongAccount { index: 7 })
        );
        for kind in [RouteKind::ExactIn, RouteKind::SharedExactOut] {
            let bytes = wire(kind);
            let authority = usize::from(kind.is_shared());
            let mut accounts = metas(kind.is_shared());
            accounts[authority].is_signer = false;
            assert_eq!(
                check(kind, &bytes, &accounts),
                Err(Error::MissingSigner { index: authority })
            );
            let mut accounts = metas(kind.is_shared());
            accounts[3].is_signer = true;
            assert_eq!(
                check(kind, &bytes, &accounts),
                Err(Error::UnexpectedSigner { index: 3 })
            );
            let mut accounts = metas(kind.is_shared());
            let index = if kind.is_shared() { 2 } else { 1 };
            accounts[index].is_writable = false;
            assert_eq!(
                check(kind, &bytes, &accounts),
                Err(Error::MissingWritable { index })
            );
            let mut accounts = metas(kind.is_shared());
            let tail = accounts.len();
            accounts.push(AccountMeta {
                key: [99; 32],
                is_signer: true,
                is_writable: true,
            });
            assert_eq!(
                check(kind, &bytes, &accounts),
                Err(Error::UnexpectedSigner { index: tail })
            );
        }
    }

    #[test]
    fn shared_id_and_trusted_binding_sanity_are_checked() {
        let mut bytes = wire(RouteKind::SharedExactIn);
        bytes[8] = 8;
        assert_eq!(
            check(RouteKind::SharedExactIn, &bytes, &metas(true)),
            Err(Error::SharedIdMismatch)
        );
        let bytes = wire(RouteKind::ExactIn);
        let accounts = metas(false);
        let mut e = expected(false);
        e.source_token_program = [99; 32];
        assert_eq!(
            validate_header(
                &JUPITER_PROGRAM,
                &bytes,
                &accounts,
                &e,
                amounts(false),
                limits()
            ),
            Err(Error::UnsupportedTokenProgram)
        );
        let mut e = expected(false);
        e.destination_mint = e.source_mint;
        assert_eq!(
            validate_header(
                &JUPITER_PROGRAM,
                &bytes,
                &accounts,
                &e,
                amounts(false),
                limits()
            ),
            Err(Error::InvalidExpectations)
        );
        let mut e = expected(false);
        e.shared = expected(true).shared;
        assert_eq!(
            validate_header(
                &JUPITER_PROGRAM,
                &bytes,
                &accounts,
                &e,
                amounts(false),
                limits()
            ),
            Err(Error::SharedModeMismatch)
        );
    }

    #[test]
    fn opaque_route_data_is_deliberately_not_full_authorization() {
        let mut bytes = wire(RouteKind::ExactIn);
        // Invalid Swap tag and uninterpreted trailing bytes still pass HEADER validation.
        // This executable boundary test prevents a future integration claiming a full decoder.
        bytes[34] = 255;
        bytes.extend_from_slice(&[255; 16]);
        let mut accounts = metas(false);
        accounts.push(AccountMeta {
            key: [99; 32],
            is_signer: false,
            is_writable: true,
        });
        accounts.push(AccountMeta {
            key: expected(false).transfer_authority,
            is_signer: true,
            is_writable: true,
        });
        assert!(check(RouteKind::ExactIn, &bytes, &accounts).is_ok());
    }
}
