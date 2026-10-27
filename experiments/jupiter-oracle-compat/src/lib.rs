//! Standalone native compatibility experiment, not production or an oracle policy.
//!
//! No client feature, keys, signing, RPC, feed creation or transaction submission.
//! A successful compile does not qualify SBF execution or any live attestation.

use anchor_lang::{AnchorSerialize, Discriminator, InstructionData};
use solana_account_info::AccountInfo;
use solana_clock::Clock;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::str::FromStr;
use switchboard_on_demand as sb;

pub const SCALE: i128 = 1_000_000_000_000_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedBinding {
    pub queue: [u8; 32],
    pub queue_owner: [u8; 32],
    pub feed_id: [u8; 32],
    pub input_mint: [u8; 32],
    pub output_mint: [u8; 32],
    pub input_amount_raw: u64,
    pub min_oracle_samples: u8,
    pub max_age_slots: u64,
    pub quote_version: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedRequest {
    pub input_mint: [u8; 32],
    pub output_mint: [u8; 32],
    pub input_amount_raw: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProbeError {
    RequestBinding,
    QueueKey,
    QueueOwner,
    SysvarIdentity,
    InstructionIndex,
    Verification(String),
    Feed,
    Version,
    Freshness,
    SlotRegression,
    Quorum,
    Value,
}

/// A compile-time identity check against the real published SDK alias.
/// It deliberately has no transmute or unchecked byte bridge.
pub fn anchor_account_as_sdk<'info, 'borrow>(
    account: &'borrow anchor_lang::prelude::AccountInfo<'info>,
) -> &'borrow sb::AccountInfo<'info> {
    account
}

pub fn pinned_account_as_sdk<'info, 'borrow>(
    account: &'borrow AccountInfo<'info>,
) -> &'borrow sb::AccountInfo<'info> {
    account
}

fn validate_request(binding: &FeedBinding, request: FixedRequest) -> Result<(), ProbeError> {
    if request.input_mint != binding.input_mint
        || request.output_mint != binding.output_mint
        || request.input_amount_raw == 0
        || request.input_amount_raw != binding.input_amount_raw
    {
        return Err(ProbeError::RequestBinding);
    }
    Ok(())
}

fn sysvar_key(name: &str) -> [u8; 32] {
    Pubkey::from_str(name)
        .expect("fixed public sysvar address")
        .to_bytes()
}

fn validate_accounts(
    binding: &FeedBinding,
    queue: &AccountInfo<'_>,
    slothashes: &AccountInfo<'_>,
    instructions: &AccountInfo<'_>,
) -> Result<(), ProbeError> {
    if queue.key.to_bytes() != binding.queue {
        return Err(ProbeError::QueueKey);
    }
    if queue.owner.to_bytes() != binding.queue_owner {
        return Err(ProbeError::QueueOwner);
    }
    let sysvar_owner = sysvar_key("Sysvar1111111111111111111111111111111111111");
    if slothashes.key.to_bytes() != sysvar_key("SysvarS1otHashes111111111111111111111111111")
        || instructions.key.to_bytes() != sysvar_key("Sysvar1nstructions1111111111111111111111111")
        || slothashes.owner.to_bytes() != sysvar_owner
        || instructions.owner.to_bytes() != sysvar_owner
    {
        return Err(ProbeError::SysvarIdentity);
    }
    Ok(())
}

struct PostVerificationView<'a> {
    feed_id: [u8; 32],
    version: u8,
    recent_slot: u64,
    oracle_count: u8,
    oracle_indices: &'a [u8],
    signed_min_samples: u8,
    scaled_value: i128,
}

/// Business-policy checks only. Synthetic unit tests of this helper do not verify
/// signatures and must never be described as successful oracle-attestation tests.
fn check_post_verification_policy(
    binding: &FeedBinding,
    view: PostVerificationView<'_>,
    now_slot: u64,
    previous_slot: u64,
) -> Result<u64, ProbeError> {
    if view.feed_id != binding.feed_id {
        return Err(ProbeError::Feed);
    }
    if view.version != binding.quote_version {
        return Err(ProbeError::Version);
    }
    if view.recent_slot > now_slot || now_slot - view.recent_slot > binding.max_age_slots {
        return Err(ProbeError::Freshness);
    }
    if view.recent_slot < previous_slot {
        return Err(ProbeError::SlotRegression);
    }
    let mut seen = 0u32;
    if view.oracle_indices.len() != usize::from(view.oracle_count)
        || view.oracle_count < binding.min_oracle_samples
        || view.signed_min_samples < binding.min_oracle_samples
        || view.oracle_count < view.signed_min_samples
    {
        return Err(ProbeError::Quorum);
    }
    for &index in view.oracle_indices {
        if index >= 30 || seen & (1u32 << index) != 0 {
            return Err(ProbeError::Quorum);
        }
        seen |= 1u32 << index;
    }
    if view.scaled_value <= 0 || view.scaled_value % SCALE != 0 {
        return Err(ProbeError::Value);
    }
    u64::try_from(view.scaled_value / SCALE).map_err(|_| ProbeError::Value)
}

/// Real SDK reader call path, compiled but with ZERO successful live-attestation
/// tests in this experiment. The caller must bind the feed ID to the independently
/// reviewed immutable job and choose the queue/version/quorum/freshness policy.
/// This uses the actual instruction sysvar path, never parse_unverified.
#[allow(clippy::too_many_arguments)]
pub fn read_verified_raw_output<'info>(
    binding: &FeedBinding,
    request: FixedRequest,
    queue: &anchor_lang::prelude::AccountInfo<'info>,
    slothashes: &anchor_lang::prelude::AccountInfo<'info>,
    instructions: &anchor_lang::prelude::AccountInfo<'info>,
    clock: &Clock,
    ed25519_instruction_index: i64,
    previous_slot: u64,
) -> Result<u64, ProbeError> {
    validate_request(binding, request)?;
    validate_accounts(binding, queue, slothashes, instructions)?;
    if ed25519_instruction_index < 0 {
        return Err(ProbeError::InstructionIndex);
    }
    let mut verifier = sb::QuoteVerifier::new();
    verifier
        .queue(anchor_account_as_sdk(queue))
        .slothash_sysvar(anchor_account_as_sdk(slothashes))
        .ix_sysvar(anchor_account_as_sdk(instructions))
        .clock_slot(clock.slot)
        .max_age(binding.max_age_slots);
    let quote = verifier
        .verify_instruction_at(ed25519_instruction_index)
        .map_err(|error| ProbeError::Verification(error.to_string()))?;
    let feed = quote.feed(&binding.feed_id).map_err(|_| ProbeError::Feed)?;
    check_post_verification_policy(
        binding,
        PostVerificationView {
            feed_id: *feed.feed_id(),
            version: quote.version(),
            recent_slot: quote.slot(),
            oracle_count: quote.oracle_count,
            oracle_indices: quote.oracle_idxs,
            signed_min_samples: feed.min_oracle_samples(),
            scaled_value: feed.feed_value(),
        },
        clock.slot,
        previous_slot,
    )
}

/// Byte-based bridge preserves instruction data and all account privileges. It
/// does not sign, execute or endorse an arbitrary SDK instruction.
pub fn sdk_instruction_to_pinned(source: sb::prelude::Instruction) -> Instruction {
    Instruction {
        program_id: Pubkey::new_from_array(source.program_id.to_bytes()),
        accounts: source
            .accounts
            .into_iter()
            .map(|account| AccountMeta {
                pubkey: Pubkey::new_from_array(account.pubkey.to_bytes()),
                is_signer: account.is_signer,
                is_writable: account.is_writable,
            })
            .collect(),
        data: source.data,
    }
}

/// Experimental consumer ABI only. This is not a production short.win instruction.
#[derive(AnchorSerialize)]
pub struct ProbeReadArgs {
    pub feed_id: [u8; 32],
    pub input_mint: [u8; 32],
    pub output_mint: [u8; 32],
    pub input_amount_raw: u64,
    pub ed25519_instruction_index: u16,
}

impl Discriminator for ProbeReadArgs {
    const DISCRIMINATOR: &'static [u8] = b"JUPPROBE";
}
impl InstructionData for ProbeReadArgs {}

pub fn build_probe_consumer_instruction(
    probe_program: Pubkey,
    queue: Pubkey,
    slothashes: Pubkey,
    instructions: Pubkey,
    clock: Pubkey,
    args: &ProbeReadArgs,
) -> Instruction {
    Instruction {
        program_id: probe_program,
        accounts: [queue, slothashes, instructions, clock]
            .into_iter()
            .map(|key| AccountMeta::new_readonly(key, false))
            .collect(),
        data: args.data(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sb::on_demand::oracle_quote::feed_info::{PackedFeedInfo, PackedQuoteHeader};
    use sb::on_demand::oracle_quote::instruction_parser::ParsedEd25519Instruction;

    fn binding() -> FeedBinding {
        FeedBinding {
            queue: [7; 32],
            queue_owner: [8; 32],
            feed_id: [9; 32],
            input_mint: [1; 32],
            output_mint: [2; 32],
            input_amount_raw: 100_000_000,
            min_oracle_samples: 3,
            max_age_slots: 20,
            quote_version: 1,
        }
    }
    fn request() -> FixedRequest {
        FixedRequest {
            input_mint: [1; 32],
            output_mint: [2; 32],
            input_amount_raw: 100_000_000,
        }
    }
    fn observation<'a>(indices: &'a [u8]) -> PostVerificationView<'a> {
        PostVerificationView {
            feed_id: [9; 32],
            version: 1,
            recent_slot: 1000,
            oracle_count: 3,
            oracle_indices: indices,
            signed_min_samples: 3,
            scaled_value: ((1i128 << 53) + 1) * SCALE,
        }
    }

    #[test]
    fn actual_anchor_account_info_alias_matches_published_sdk() {
        let key = Pubkey::new_from_array([1; 32]);
        let owner = Pubkey::new_from_array([2; 32]);
        let mut lamports = 0;
        let mut data = [];
        let account = AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        assert_eq!(
            anchor_account_as_sdk(&account).key.to_bytes(),
            key.to_bytes()
        );
        assert_eq!(
            pinned_account_as_sdk(&account).owner.to_bytes(),
            owner.to_bytes()
        );
    }

    #[test]
    fn compiled_reader_rejects_wrong_queue_owner_before_sdk_parsing() {
        let queue_key = Pubkey::new_from_array([7; 32]);
        let wrong_owner = Pubkey::new_from_array([0; 32]);
        let mut lamports = 0;
        let mut data = [];
        let account = AccountInfo::new(
            &queue_key,
            false,
            false,
            &mut lamports,
            &mut data,
            &wrong_owner,
            false,
        );
        let clock = Clock {
            slot: 1005,
            ..Clock::default()
        };
        assert_eq!(
            read_verified_raw_output(
                &binding(),
                request(),
                &account,
                &account,
                &account,
                &clock,
                0,
                0
            ),
            Err(ProbeError::QueueOwner)
        );
    }

    #[test]
    fn compiled_reader_rejects_mint_and_quantity_mapping() {
        for bad in [
            FixedRequest {
                input_mint: [3; 32],
                ..request()
            },
            FixedRequest {
                output_mint: [3; 32],
                ..request()
            },
            FixedRequest {
                input_amount_raw: 1,
                ..request()
            },
            FixedRequest {
                input_amount_raw: 0,
                ..request()
            },
        ] {
            assert_eq!(
                validate_request(&binding(), bad),
                Err(ProbeError::RequestBinding)
            );
        }
    }

    #[test]
    fn synthetic_post_verification_gates_preserve_large_integer_units() {
        let raw = check_post_verification_policy(&binding(), observation(&[0, 1, 2]), 1005, 999);
        assert_eq!(raw, Ok((1u64 << 53) + 1));
    }

    #[test]
    fn synthetic_post_verification_rejects_feed_freshness_and_slot_regression() {
        let mut view = observation(&[0, 1, 2]);
        view.feed_id = [0; 32];
        assert_eq!(
            check_post_verification_policy(&binding(), view, 1005, 0),
            Err(ProbeError::Feed)
        );
        for slot in [979, 1006] {
            let mut view = observation(&[0, 1, 2]);
            view.recent_slot = slot;
            assert_eq!(
                check_post_verification_policy(&binding(), view, 1005, 0),
                Err(ProbeError::Freshness)
            );
        }
        assert_eq!(
            check_post_verification_policy(&binding(), observation(&[0, 1, 2]), 1005, 1001),
            Err(ProbeError::SlotRegression)
        );
    }

    #[test]
    fn synthetic_post_verification_rejects_quorum_and_numeric_errors() {
        for indices in [&[0, 0, 1][..], &[0, 1, 30][..], &[0, 1][..]] {
            assert_eq!(
                check_post_verification_policy(&binding(), observation(indices), 1005, 0),
                Err(ProbeError::Quorum)
            );
        }
        for scaled in [-1, 0, SCALE + 1, (i128::from(u64::MAX) + 1) * SCALE] {
            let mut view = observation(&[0, 1, 2]);
            view.scaled_value = scaled;
            assert_eq!(
                check_post_verification_policy(&binding(), view, 1005, 0),
                Err(ProbeError::Value)
            );
        }
        let mut view = observation(&[0, 1, 2]);
        view.signed_min_samples = 1;
        assert_eq!(
            check_post_verification_policy(&binding(), view, 1005, 0),
            Err(ProbeError::Quorum)
        );
        let mut view = observation(&[0, 1, 2]);
        view.signed_min_samples = 5;
        assert_eq!(
            check_post_verification_policy(&binding(), view, 1005, 0),
            Err(ProbeError::Quorum)
        );
    }

    #[test]
    fn published_sdk_feed_accessor_has_expected_packed_abi() {
        assert_eq!(std::mem::size_of::<PackedFeedInfo>(), 49);
        assert_eq!(std::mem::size_of::<PackedQuoteHeader>(), 32);
        let feed = PackedFeedInfo {
            feed_id: [9; 32],
            feed_value: ((1i128 << 53) + 1) * SCALE,
            min_oracle_samples: 3,
        };
        assert_eq!(*feed.feed_id(), [9; 32]);
        assert_eq!(feed.feed_value() / SCALE, (1i128 << 53) + 1);
        assert_eq!(feed.min_oracle_samples(), 3);
    }

    #[test]
    fn raw_u64_max_bypasses_sdk_decimal_mantissa_limit_without_truncation() {
        let scaled = i128::from(u64::MAX).checked_mul(SCALE).unwrap();
        let feed = PackedFeedInfo {
            feed_id: [9; 32],
            feed_value: scaled,
            min_oracle_samples: 3,
        };
        assert_eq!(feed.feed_value(), scaled);
        let mut view = observation(&[0, 1, 2]);
        view.scaled_value = feed.feed_value();
        assert_eq!(
            check_post_verification_policy(&binding(), view, 1005, 0),
            Ok(u64::MAX)
        );
        // SDK .value() calls Decimal::from_i128_with_scale before normalize().
        // Decimal's mantissa is limited to96bits, even though the raw
        // Switchboard i128 can hold all u64 raw units multiplied by10^18.
        let decimal_max_mantissa = (1i128 << 96) - 1;
        assert!(scaled > decimal_max_mantissa);
        assert!(sb::prelude::rust_decimal::Decimal::try_from_i128_with_scale(scaled, 18).is_err());
        assert!(std::panic::catch_unwind(|| feed.value()).is_err());
        let cost_25_sol_lamports_scaled = 25_000_000_000i128 * SCALE;
        assert!(
            sb::prelude::rust_decimal::Decimal::try_from_i128_with_scale(
                cost_25_sol_lamports_scaled,
                18
            )
            .is_ok()
        );
    }

    #[test]
    fn actual_sdk_parser_rejects_empty_unverified_payload() {
        assert!(ParsedEd25519Instruction::parse(&[]).is_err());
    }

    #[test]
    #[should_panic(expected = "Reconstructing offset-based format not yet implemented")]
    fn published_sdk_reverse_serializer_is_explicitly_unimplemented() {
        // No signature is produced: this fixture only exposes the upstream stub.
        let parsed = ParsedEd25519Instruction {
            num_signatures: 0,
            padding: 0,
            signatures: vec![],
            quote_header: PackedQuoteHeader {
                signed_slothash: [0; 32],
            },
            feeds: vec![],
            oracle_idxs: vec![],
            slot: 0,
            version: 1,
            discriminator: *b"SBOD",
        };
        let _ = parsed.to_instruction_bytes();
    }

    #[test]
    fn actual_anchor_instruction_builder_and_sdk_bridge_preserve_roles() {
        let args = ProbeReadArgs {
            feed_id: [9; 32],
            input_mint: [1; 32],
            output_mint: [2; 32],
            input_amount_raw: 100_000_000,
            ed25519_instruction_index: 2,
        };
        let key = Pubkey::new_from_array([7; 32]);
        let ix = build_probe_consumer_instruction(key, key, key, key, key, &args);
        assert_eq!(ix.data.len(), 114);
        assert_eq!(&ix.data[..8], b"JUPPROBE");
        assert!(ix.accounts.iter().all(|a| !a.is_signer && !a.is_writable));
        let sdk_ix = sb::prelude::Instruction {
            program_id: sb::prelude::Pubkey::new_from_array([4; 32]),
            accounts: vec![
                sb::prelude::AccountMeta::new(sb::prelude::Pubkey::new_from_array([5; 32]), true),
                sb::prelude::AccountMeta::new_readonly(
                    sb::prelude::Pubkey::new_from_array([6; 32]),
                    false,
                ),
            ],
            data: ix.data.clone(),
        };
        let converted = sdk_instruction_to_pinned(sdk_ix);
        assert_eq!(converted.program_id.to_bytes(), [4; 32]);
        assert_eq!(converted.accounts[0].pubkey.to_bytes(), [5; 32]);
        assert!(converted.accounts[0].is_signer && converted.accounts[0].is_writable);
        assert!(!converted.accounts[1].is_signer && !converted.accounts[1].is_writable);
        assert_eq!(converted.data, ix.data);
    }
}
