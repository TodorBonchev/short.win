//! BPF upgradeable loader `ProgramData` header (ARCH §4.1; §3.1 venue-upgrade latch, A7; C1-15).
//!
//! `UpgradeableLoaderState::ProgramData` in bincode: `tag u32 @0 (== 3)`, `slot u64 @4` (the
//! last deploy slot), `upgrade_authority_address Option<Pubkey>` as `u8 @12` (0 = None, 1 =
//! Some) and the key `@13`; the ELF follows at 45 [R-PUMP §1; V9]. Only
//! `flag_venue_upgraded` and `revalidate_venue` read a venue's ProgramData (ARCH §6.5), and
//! they need only the deploy slot; the account can be 10 MiB, so this reader touches nothing
//! past the header.

use crate::bytes::{key_at, u8_at, u32_at, u64_at};
use crate::{Key, ReadError};

/// `BPFLoaderUpgradeab1e11111111111111111111111`, the owner of every `ProgramData` account.
pub const LOADER_UPGRADEABLE: Key = [
    2, 168, 246, 145, 78, 136, 161, 176, 226, 16, 21, 62, 247, 99, 174, 43, 0, 194, 185, 61, 22,
    193, 36, 210, 192, 83, 122, 16, 4, 128, 0, 0,
];

/// bincode enum tag of `UpgradeableLoaderState::ProgramData`.
pub const TAG_PROGRAM_DATA: u32 = 3;

/// Offset of the deploy slot.
pub const SLOT_OFFSET: usize = 4;

/// Offset of the upgrade authority's `Option` tag.
pub const AUTHORITY_OFFSET: usize = 12;

/// Length of the header (`tag + slot + Option<Pubkey>`); the ELF starts here.
pub const HEADER_LEN: usize = 45;

/// Smallest account this reader accepts: the tag and the deploy slot.
pub const MIN_LEN: usize = 12;

/// The `ProgramData` header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProgramDataHeader {
    /// Slot of the last deployment (upgrade) of the program.
    pub slot: u64,
    /// Upgrade authority, `None` once the program is immutable (or the field is absent in a
    /// truncated account: zero padding reads `None`).
    pub upgrade_authority: Option<Key>,
}

/// Reads the header of a `ProgramData` account: owner == the upgradeable loader, at least
/// [`MIN_LEN`] bytes, tag == 3, a valid `Option` tag.
pub fn read(owner: &Key, data: &[u8]) -> Result<ProgramDataHeader, ReadError> {
    if *owner != LOADER_UPGRADEABLE {
        return Err(ReadError::WrongOwner);
    }
    if data.len() < MIN_LEN {
        return Err(ReadError::TooShort);
    }
    if u32_at(data, 0) != TAG_PROGRAM_DATA {
        return Err(ReadError::WrongTag);
    }
    let upgrade_authority = match u8_at(data, AUTHORITY_OFFSET) {
        0 => None,
        1 => Some(key_at(data, AUTHORITY_OFFSET.saturating_add(1))),
        _ => return Err(ReadError::Malformed),
    };
    Ok(ProgramDataHeader {
        slot: u64_at(data, SLOT_OFFSET),
        upgrade_authority,
    })
}

/// The deploy slot of a `ProgramData` account (the only field the venue-upgrade latch reads).
pub fn deploy_slot(owner: &Key, data: &[u8]) -> Result<u64, ReadError> {
    read(owner, data).map(|h| h.slot)
}

/// Encodes a `ProgramData` header (tests and fixtures; the inverse of [`read`]).
pub fn encode_header(slot: u64, upgrade_authority: Option<&Key>) -> [u8; HEADER_LEN] {
    let mut out = [0u8; HEADER_LEN];
    let (tag, rest) = out.split_at_mut(SLOT_OFFSET);
    tag.copy_from_slice(&TAG_PROGRAM_DATA.to_le_bytes());
    let (slot_bytes, rest) = rest.split_at_mut(8);
    slot_bytes.copy_from_slice(&slot.to_le_bytes());
    if let Some(key) = upgrade_authority {
        let (flag, key_bytes) = rest.split_at_mut(1);
        flag.copy_from_slice(&[1]);
        key_bytes.copy_from_slice(key);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTH: Key = [7u8; 32];

    #[test]
    fn round_trip() {
        let h = encode_header(449_734_335, Some(&AUTH));
        assert_eq!(
            read(&LOADER_UPGRADEABLE, &h),
            Ok(ProgramDataHeader {
                slot: 449_734_335,
                upgrade_authority: Some(AUTH)
            })
        );
        let h = encode_header(1, None);
        assert_eq!(deploy_slot(&LOADER_UPGRADEABLE, &h), Ok(1));
        assert_eq!(
            read(&LOADER_UPGRADEABLE, &h).map(|h| h.upgrade_authority),
            Ok(None)
        );
    }

    #[test]
    fn ignores_the_elf_after_the_header() {
        let mut data = [0xabu8; 200];
        data[..HEADER_LEN].copy_from_slice(&encode_header(42, Some(&AUTH)));
        assert_eq!(deploy_slot(&LOADER_UPGRADEABLE, &data), Ok(42));
    }

    #[test]
    fn refusals() {
        let h = encode_header(5, None);
        assert_eq!(deploy_slot(&[0u8; 32], &h), Err(ReadError::WrongOwner));
        assert_eq!(
            deploy_slot(&LOADER_UPGRADEABLE, &h[..11]),
            Err(ReadError::TooShort)
        );
        let mut program = h;
        program[0] = 2; // UpgradeableLoaderState::Program
        assert_eq!(
            deploy_slot(&LOADER_UPGRADEABLE, &program),
            Err(ReadError::WrongTag)
        );
        let mut bad_option = h;
        bad_option[AUTHORITY_OFFSET] = 2;
        assert_eq!(
            read(&LOADER_UPGRADEABLE, &bad_option),
            Err(ReadError::Malformed)
        );
    }

    #[test]
    fn truncated_header_reads_no_authority() {
        let h = encode_header(9, Some(&AUTH));
        assert_eq!(
            read(&LOADER_UPGRADEABLE, &h[..MIN_LEN]),
            Ok(ProgramDataHeader {
                slot: 9,
                upgrade_authority: None
            })
        );
    }
}
