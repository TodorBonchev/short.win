//! `VenueGuard` (zero-copy, seeds `["venue_guard"]`, 224 B): the event-driven venue-upgrade
//! latch (ARCH §2.3.2, §3.1, A7; decision 4; hardened in v0.4.1, R4B-7).
//!
//! The permissionless `flag_venue_upgraded` sets bit `venue` of `upgraded` when a venue
//! ProgramData's deploy slot differs from `guard_slot[venue]`, and while the bit is set
//! refreshes `flagged_slot` / `flagged_at` whenever the deploy slot changes again. The
//! Emergency `revalidate_venue(venue, expected_slot)` requires the current deploy slot to equal
//! `expected_slot`, records it as `guard_slot[venue]` and clears the bit. `open_short` refuses
//! while a bit of its venue's mask is set (`GUARD_MASK_*`). Kept out of `GlobalConfig` so a
//! permissionless writer never write-locks the account every instruction reads (V1-11).
//!
//! | Offset | Bytes | Fields |
//! |---|---|---|
//! | 8 | 8 | `version` u16, `bump` u8, `_pad0` u8, `upgraded` u32 |
//! | 16 | 48 | `guard_slot` [u64; 6] |
//! | 64 | 48 | `flagged_slot` [u64; 6] |
//! | 112 | 48 | `flagged_at` [i64; 6] |
//! | 160 | 64 | `_reserved` |
//!
//! Index order (`GUARD_*` in `constants.rs`): pump, pAMM, pfee, AMM v4, CPMM, Jupiter.

use anchor_lang::prelude::*;

use crate::constants::VENUE_PROGRAMS;

/// The venue-upgrade latch.
#[account(zero_copy)]
pub struct VenueGuard {
    /// Layout version ([`crate::constants::VENUE_GUARD_VERSION`]).
    pub version: u16,
    /// PDA bump of `["venue_guard"]`.
    pub bump: u8,
    pub _pad0: u8,
    /// One bit per venue program: upgraded and not yet re-validated.
    pub upgraded: u32,
    /// Last validated ProgramData deploy slot per venue program.
    pub guard_slot: [u64; 6],
    /// The latest deploy slot seen while flagged.
    pub flagged_slot: [u64; 6],
    /// When `flagged_slot` was last recorded (unix seconds).
    pub flagged_at: [i64; 6],
    pub _reserved: [u8; 64],
}

impl VenueGuard {
    /// PDA seed.
    pub const SEED: &'static [u8] = b"venue_guard";
    /// Account size including the discriminator.
    pub const SPACE: usize = 8 + core::mem::size_of::<Self>();

    /// `true` if any bit of `mask` is set (an open on a venue with that mask is refused, I17).
    pub fn blocks(&self, mask: u32) -> bool {
        self.upgraded & mask != 0
    }

    /// `true` if venue program `venue` is flagged.
    pub fn is_flagged(&self, venue: u8) -> bool {
        (venue as usize) < VENUE_PROGRAMS && self.upgraded & (1u32 << venue) != 0
    }
}
