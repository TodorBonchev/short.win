//! `VenueExtras` (zero-copy, seeds `["venue_extras", venue u8]`, 4,352 B): bounded trailing
//! accounts for a venue's CPIs (ARCH §2.3.2, §3.0, A41, A55; V2-6; **gated by O6**).
//!
//! `initialize_config` creates one per CPI venue kind (`PumpCurve`, `PumpSwap`, `RaydiumV4`,
//! `RaydiumCpmm`; the seed byte is the `VenueKind` value) with empty sets. The setter
//! (`set_venue_extras`) and the trading-side parser belong to M2 (O6 is recorded in SPEC §14
//! but the powers are built with the position engine); nothing reads these accounts in M1.
//!
//! Per venue: for each instruction kind (sell, buy, claim) a `current` and a `previous` set of
//! up to 4 entries with a `version` each. An entry is a fixed key or a derivation template
//! `{program, seeds: [SeedPart; ≤ 4], writable}`; a `SeedPart` is constant bytes (≤ 32) or a
//! placeholder among `{mint, pool, trader, bonding_curve, coin_creator}`. Callers select `none`,
//! `current` or `previous`, so one hostile or wrong Emergency write can never brick exits.
//!
//! **Layout note (ARCH §2.3.2 says ≈ 3,500 B).** The exact encoding below is this module's
//! (the ARCH table gives the content, not the bytes): `SeedPart` 34 B, `ExtraEntry` 176 B,
//! `ExtrasSet` 712 B, `IxExtras` 1,424 B, the account 4,352 B (rent ≈ 0.0228 SOL × 4). M2 may
//! re-cut it before the first deployment. M1 review (M1-FID-13): the deviation is recorded for
//! the ARCH sync — either M2 shrinks `SeedPart` / `ExtraEntry` (the 32-byte constant part and
//! the 4-seed template dominate) or ARCH §2.3.1 adopts 4,352 B; no M1 path reads the account.

use anchor_lang::prelude::*;

/// Instruction kinds with extras sets, in `VenueExtras.sets` order.
pub const EXTRAS_IX_SELL: usize = 0;
/// Buy (close, liquidate, expire).
pub const EXTRAS_IX_BUY: usize = 1;
/// Venue reward claim.
pub const EXTRAS_IX_CLAIM: usize = 2;
/// Number of instruction kinds.
pub const EXTRAS_IX_KINDS: usize = 3;
/// Entries per set.
pub const EXTRAS_MAX_ENTRIES: usize = 4;
/// Seed parts per template.
pub const EXTRAS_MAX_SEEDS: usize = 4;

/// `SeedPart.kind`: unused.
pub const SEED_NONE: u8 = 0;
/// `SeedPart.kind`: constant bytes (`len` bytes of `bytes`).
pub const SEED_CONST: u8 = 1;
/// `SeedPart.kind`: a placeholder (`len` = `PLACEHOLDER_*`).
pub const SEED_PLACEHOLDER: u8 = 2;
/// Placeholder: the market's mint.
pub const PLACEHOLDER_MINT: u8 = 0;
/// Placeholder: the market's pool.
pub const PLACEHOLDER_POOL: u8 = 1;
/// Placeholder: the market's trader PDA.
pub const PLACEHOLDER_TRADER: u8 = 2;
/// Placeholder: the pump bonding curve.
pub const PLACEHOLDER_BONDING_CURVE: u8 = 3;
/// Placeholder: the coin creator.
pub const PLACEHOLDER_COIN_CREATOR: u8 = 4;

/// `ExtraEntry.kind`: empty slot.
pub const ENTRY_NONE: u8 = 0;
/// `ExtraEntry.kind`: a fixed key (`key`).
pub const ENTRY_FIXED: u8 = 1;
/// `ExtraEntry.kind`: a template derived under program `key` from `seeds`.
pub const ENTRY_TEMPLATE: u8 = 2;

/// One seed of a derivation template (34 B).
#[zero_copy]
#[derive(PartialEq, Eq)]
pub struct SeedPart {
    /// `SEED_NONE`, `SEED_CONST` or `SEED_PLACEHOLDER`.
    pub kind: u8,
    /// Constant: number of bytes used; placeholder: `PLACEHOLDER_*`.
    pub len: u8,
    pub bytes: [u8; 32],
}

/// One trailing account (176 B).
#[zero_copy]
#[derive(PartialEq, Eq)]
pub struct ExtraEntry {
    /// `ENTRY_NONE`, `ENTRY_FIXED` or `ENTRY_TEMPLATE`.
    pub kind: u8,
    /// 1 if the account is passed writable.
    pub writable: u8,
    /// Seed parts used (template).
    pub n_seeds: u8,
    pub _pad: [u8; 5],
    /// The fixed key, or the template's program.
    pub key: Pubkey,
    pub seeds: [SeedPart; 4],
}

/// A set of up to 4 entries with its version (712 B).
#[zero_copy]
#[derive(PartialEq, Eq)]
pub struct ExtrasSet {
    pub version: u32,
    /// Entries used.
    pub count: u8,
    pub _pad: [u8; 3],
    pub entries: [ExtraEntry; 4],
}

/// The two selectable sets of one instruction kind (1,424 B).
#[zero_copy]
#[derive(PartialEq, Eq)]
pub struct IxExtras {
    pub current: ExtrasSet,
    pub previous: ExtrasSet,
}

/// Bounded venue extras of one venue kind.
#[account(zero_copy)]
pub struct VenueExtras {
    /// Layout version ([`crate::constants::VENUE_EXTRAS_VERSION`]).
    pub version: u16,
    /// PDA bump of `["venue_extras", venue]`.
    pub bump: u8,
    /// The `VenueKind` these sets apply to.
    pub venue: u8,
    pub _pad0: [u8; 4],
    /// Sell, buy, claim.
    pub sets: [IxExtras; 3],
    pub _reserved: [u8; 64],
}

impl VenueExtras {
    /// PDA seed prefix (`["venue_extras", venue u8]`).
    pub const SEED: &'static [u8] = b"venue_extras";
    /// Account size including the discriminator.
    pub const SPACE: usize = 8 + core::mem::size_of::<Self>();
}
