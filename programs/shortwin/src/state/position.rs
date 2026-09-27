//! `ShortPosition` (Borsh `Account<T>`, seeds `["position", market, borrower, nonce u64 LE]`,
//! 384 B; ARCH §2.3.5). Written by `open_short` (M2); M1 only defines the layout.
//!
//! Position SOL is extra lamports on this account: `lamports ≥ rent_min(384) + v` (I2, A4).
//! Everything in the four **locked** groups plus `deadline` and `r_min` never changes for the
//! life of the position (A26).

use anchor_lang::prelude::*;

/// `ShortPosition.status`: open.
pub const POSITION_OPEN: u8 = 0;
/// `ShortPosition.status`: exit in progress (reserved for v1.1).
pub const POSITION_EXIT_IN_PROGRESS: u8 = 1;

/// A short position (ARCH §2.3.5).
#[account]
#[derive(InitSpace, Debug, PartialEq, Eq)]
pub struct ShortPosition {
    pub borrower: Pubkey,
    pub market: Pubkey,
    /// Chosen by the client; `init` fails if reused.
    pub nonce: u64,
    pub bump: u8,
    /// [`POSITION_OPEN`] (1 = ExitInProgress reserved for v1.1).
    pub status: u8,
    pub tier: u8,
    pub venue_at_open: u8,
    /// Tokens owed (constant: no partial close in the MVP).
    pub q: u64,
    /// Sale proceeds.
    pub s: u64,
    /// Margin.
    pub m: u64,
    /// Tracked vault lamports (changes only through `add_margin`).
    pub v: u64,
    pub n_open: u64,
    pub fee_open: u64,
    /// `S·WAD/Q`.
    pub entry_price: u128,
    /// Rent basis indices at open.
    pub j_open: [u128; 3],
    /// Minimum rent, fixed at open.
    pub r_min: u64,
    pub t_open: i64,
    /// `t_open + max_duration_s[tier]`.
    pub deadline: i64,
    // ---- locked tier terms (22) ----
    pub cr_liq_bps: u32,
    pub penalty_bps: u16,
    pub max_liq_slip_bps: u16,
    pub expiry_fee_bps: u16,
    pub rate_base_bps: u32,
    pub rate_slope1_bps: u32,
    pub rate_slope2_bps: u32,
    // ---- locked split, rent share and keeper terms (44) ----
    /// Lender / protocol / keeper.
    pub penalty_split: [u16; 3],
    pub rent_protocol_bps: u16,
    pub settle_keeper_fixed: u64,
    pub settle_keeper_bps: u16,
    pub settle_keeper_cap: u64,
    pub expiry_keeper_fixed: u64,
    pub expiry_keeper_bps: u16,
    pub expiry_keeper_cap: u64,
    // ---- locked exit-bound and persistence terms (14) ----
    pub delta_liq_bps: u16,
    pub delta_exp_bps: u16,
    pub kappa_liq_bps: u16,
    pub kappa_base_bps: u16,
    pub kappa_growth_n: u8,
    pub liq_persist_n: u8,
    pub settle_persist_n: u16,
    pub settle_arm_gap_s: u16,
    // ---- locked execution and settlement terms (27) ----
    /// ε of the expiry bound.
    pub exec_tol_bps: u16,
    /// Effective `exec_rent_tol` (0 on Raydium).
    pub ert: u64,
    pub ref_max_age_s: u8,
    pub fv_dev_bps: u16,
    pub venue_fee_hard_cap_bps: u16,
    pub s3_wait_s: u32,
    pub grad_grace_s: u32,
    pub max_migration_pause_s: u32,
    // ---- settlement arming state (25) ----
    pub s_armed: u8,
    /// `ema_count` at arm.
    pub s_count: u64,
    pub s_since: i64,
    pub s_last: i64,
    /// Room for the O1/O2 terms if the owner ever signs them off (not built, R3A-8 c).
    pub _reserved: [u8; 32],
}

impl ShortPosition {
    /// PDA seed prefix (`["position", market, borrower, nonce u64 LE]`).
    pub const SEED: &'static [u8] = b"position";
    /// Account size including the discriminator.
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}
