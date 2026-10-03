//! **complete-curve**: short.win's stateless helper for the straddle bundle (ARCHITECTURE §5.4
//! case 2, A66; R3B-11). One instruction, [`complete_curve::complete_curve`]`(max_sol_cost)`:
//!
//! 1. reads the pump bonding curve **at execution** (the length-tolerant reader of
//!    `shortwin-venue-state`, the one the core program and the keeper use);
//! 2. if the curve is already complete (or holds no real tokens), succeeds as a **no-op** (no CPI;
//!    event [`CurveAlreadyComplete`]): a third party completed it first, and `migrate_v2` in the
//!    bundle's next transaction is idempotent;
//! 3. otherwise CPIs pump `buy_v2(amount = real_token_reserves, max_sol_cost)` with the
//!    **transaction's own signer** as pump's `user` (it pays, it receives the tokens in its
//!    `associated_base_user`), then requires the curve to be complete (`NotCompleted`) and emits
//!    [`CurveCompleted`].
//!
//! v0.3's tx1 named an `rT` read before the bundle, so any curve trade landing between the read and
//! the bundle made pump reject the buy (a buy before it: 6021 `NotEnoughTokensToBuy`) or left the
//! curve incomplete for `migrate_v2` (a sale before it), and the bundle reverted while the price ran
//! (R3B-11). Reading `rT` here removes that race; `max_sol_cost` (pump's own bound, 6002
//! `TooMuchSolRequired`) caps what the signer pays for whatever `rT` is live.
//!
//! **Accounts**: exactly pump `buy_v2`'s 27, in its IDL order and with its flags (R-PUMP §3.5),
//! `user` the only signer; any remaining accounts are appended to the CPI unchanged (an additive
//! pump interface change, like short.win's extras, A41). The helper checks what it relies on
//! itself — the CPI target is pump (executable), `bonding_curve` is pump's
//! `PDA(["bonding-curve", base_mint])` and parses as a `BondingCurve`, `user` signs — and leaves
//! every other account to pump, which checks them (the no-op path reads only the curve, whose key
//! is therefore checked here and not left to pump).
//!
//! **Data**: pump gets exactly 24 bytes (discriminator, amount, bound): never the undocumented
//! trailing cap byte (R-PUMP Update U7), so a buy above `rT` would still revert.
//!
//! **Why a separate program** (ARCH §1 principle 1, A4, A66): it forwards a user's signature to a
//! venue, which the core program never does. It holds no accounts of its own, touches no short.win
//! account (short.win never invokes it, and its `trader` PDA can sign only through short.win), and
//! can spend only what its signer signed for. Upgrade authority: Governance, as the core.
//!
//! **Budget** (ARCH §5.4, §6.1, §6.2): stack helper 1 → pump 2 → System / Token / pump-fees /
//! pump's event self-CPI 3; trace 1 + the completing `buy_v2`'s 8–9 entries; the helper's own
//! logic is the curve PDA check (`find_program_address`), two curve reads and the CPI (measured by
//! `tests/litesvm/tests/straddle_it.rs`, T-STRADDLE).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    unsafe_code
)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke;
use shortwin_venue_state::pump;

declare_id!("4B5mGykX9EWwaYdbTvsp8qXCUTbc7caQourz16Gat6LJ");

// security.txt (neodyme-labs/solana-security-txt =1.1.3; ARCH §8.5, M10 SECTXT-1), read by
// Solana Explorer and `query-security-txt`; `scripts/check-security-txt.py
// --forbid-placeholders` checks it in CI (the release gate). The values are the owner's (M10 Q3,
// 2026-10-02; `SECURITY.md`, the web app's `/security` page and `/.well-known/security.txt` carry
// the same). Not in
// `no-entrypoint` builds (the keeper links both programs as libraries: two `#[no_mangle]
// SECURITY_TXT` symbols would clash) nor in the TEST-ONLY `test-venue` build (A49). No `expiry`,
// `source_code` or `source_revision` while the repository is private, and no `auditors` before
// the first published audit; `source_release` is the package version (`v1.0.0` at the audit
// freeze), never a commit (the hash must not depend on it).
#[cfg(not(any(feature = "no-entrypoint", feature = "test-venue")))]
solana_security_txt::security_txt! {
    name: "short.win complete_curve",
    project_url: "https://short.win",
    contacts: "email:security@short.win,link:https://short.win/security",
    policy: "https://short.win/security",
    preferred_languages: "en",
    source_release: concat!("v", env!("CARGO_PKG_VERSION"))
}

/// The pump program `6EF8rr…` (the only CPI target).
pub const PUMP_PROGRAM_ID: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");

/// `sha256("global:buy_v2")[..8]` (R-PUMP §3.5).
pub const IX_BUY_V2: [u8; 8] = [0xb8, 0x17, 0xee, 0x61, 0x67, 0xc5, 0xd3, 0x3d];

/// `buy_v2`'s data length: discriminator, amount, bound — no trailing cap byte (module docs).
pub const BUY_V2_DATA_LEN: usize = 24;

/// `buy_v2`'s fixed account count (R-PUMP §3.5).
pub const BUY_V2_ACCOUNTS: usize = 27;

/// Index of `user` in `buy_v2`'s (and this instruction's) account list.
pub const USER_INDEX: usize = 13;

/// Index of `bonding_curve`.
pub const BONDING_CURVE_INDEX: usize = 10;

/// `buy_v2(amount, max_sol_cost)`'s data (exactly [`BUY_V2_DATA_LEN`] bytes).
pub fn buy_v2_data(amount: u64, max_sol_cost: u64) -> [u8; BUY_V2_DATA_LEN] {
    let mut data = [0u8; BUY_V2_DATA_LEN];
    let (disc, rest) = data.split_at_mut(8);
    disc.copy_from_slice(&IX_BUY_V2);
    let (a, b) = rest.split_at_mut(8);
    a.copy_from_slice(&amount.to_le_bytes());
    b.copy_from_slice(&max_sol_cost.to_le_bytes());
    data
}

#[program]
pub mod complete_curve {
    use super::*;

    /// Completes the curve of `base_mint` if it is live (module docs): `buy_v2(rT_live,
    /// max_sol_cost)` by the signing `user`; a no-op on a complete curve.
    pub fn complete_curve<'info>(
        ctx: Context<'info, CompleteCurve<'info>>,
        max_sol_cost: u64,
    ) -> Result<()> {
        handler(ctx, max_sol_cost)
    }
}

/// `buy_v2`'s 27 accounts in pump's IDL order and flags (R-PUMP §3.5). Everything the helper
/// does not interpret is `UncheckedAccount`: pump checks it.
#[derive(Accounts)]
pub struct CompleteCurve<'info> {
    /// CHECK: pump `Global` (checked by pump).
    pub global: UncheckedAccount<'info>,
    /// CHECK: the coin's mint; seeds the curve PDA checked below (and by pump).
    pub base_mint: UncheckedAccount<'info>,
    /// CHECK: WSOL for a SOL curve (checked by pump).
    pub quote_mint: UncheckedAccount<'info>,
    /// CHECK: the mint's token program (checked by pump).
    pub base_token_program: UncheckedAccount<'info>,
    /// CHECK: the quote mint's token program (checked by pump).
    pub quote_token_program: UncheckedAccount<'info>,
    /// CHECK: the ATA program (checked by pump).
    pub associated_token_program: UncheckedAccount<'info>,
    /// CHECK: one of `Global`'s fee recipients (checked by pump).
    #[account(mut)]
    pub fee_recipient: UncheckedAccount<'info>,
    /// CHECK: its quote ATA (seed-checked by pump).
    #[account(mut)]
    pub associated_quote_fee_recipient: UncheckedAccount<'info>,
    /// CHECK: one of `Global`'s buyback recipients (checked by pump).
    #[account(mut)]
    pub buyback_fee_recipient: UncheckedAccount<'info>,
    /// CHECK: its quote ATA (seed-checked by pump).
    #[account(mut)]
    pub associated_quote_buyback_fee_recipient: UncheckedAccount<'info>,
    /// CHECK: the curve the helper reads: owned by pump, `PDA(pump, ["bonding-curve", base_mint])`
    /// (the no-op path reads it without any pump CPI, so its key is checked here), parsed by
    /// `shortwin_venue_state::pump::read_curve`.
    #[account(
        mut,
        owner = PUMP_PROGRAM_ID @ CompleteCurveError::BadCurve,
        seeds = [pump::BONDING_CURVE_SEED, base_mint.key().as_ref()],
        bump,
        seeds::program = PUMP_PROGRAM_ID,
    )]
    pub bonding_curve: UncheckedAccount<'info>,
    /// CHECK: the curve's base ATA (checked by pump).
    #[account(mut)]
    pub associated_base_bonding_curve: UncheckedAccount<'info>,
    /// CHECK: the curve's quote ATA (seed-checked by pump).
    #[account(mut)]
    pub associated_quote_bonding_curve: UncheckedAccount<'info>,
    /// The transaction's signer: pump's `user` (pays, receives the tokens).
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: the tokens' destination (must exist; pump does not create it).
    #[account(mut)]
    pub associated_base_user: UncheckedAccount<'info>,
    /// CHECK: the user's quote ATA (seed-checked by pump; untouched for a SOL curve).
    #[account(mut)]
    pub associated_quote_user: UncheckedAccount<'info>,
    /// CHECK: `PDA(pump, ["creator-vault", curve.creator])` (checked by pump).
    #[account(mut)]
    pub creator_vault: UncheckedAccount<'info>,
    /// CHECK: its quote ATA (seed-checked by pump).
    #[account(mut)]
    pub associated_creator_vault: UncheckedAccount<'info>,
    /// CHECK: pump-fees `["sharing-config", mint]` (may be uninitialised; checked by pump).
    pub sharing_config: UncheckedAccount<'info>,
    /// CHECK: pump's global volume accumulator (checked by pump).
    pub global_volume_accumulator: UncheckedAccount<'info>,
    /// CHECK: the user's pump volume accumulator (created by pump if missing, paid by `user`).
    #[account(mut)]
    pub user_volume_accumulator: UncheckedAccount<'info>,
    /// CHECK: its quote ATA (seed-checked by pump).
    #[account(mut)]
    pub associated_user_volume_accumulator: UncheckedAccount<'info>,
    /// CHECK: pump-fees `FeeConfig` of pump (checked by pump).
    pub fee_config: UncheckedAccount<'info>,
    /// CHECK: the pump-fees program (checked by pump).
    pub fee_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: pump's `emit_cpi!` authority (checked by pump).
    pub event_authority: UncheckedAccount<'info>,
    /// CHECK: the CPI target: pump itself, executable.
    #[account(address = PUMP_PROGRAM_ID @ CompleteCurveError::NotPump, executable)]
    pub program: UncheckedAccount<'info>,
}

/// The curve was live: the signer bought its remaining real tokens and completed it.
#[event]
pub struct CurveCompleted {
    pub mint: Pubkey,
    pub bonding_curve: Pubkey,
    pub user: Pubkey,
    /// `real_token_reserves` read at execution (the tokens bought).
    pub tokens: u64,
    /// Lamports the signer's balance fell by (the buy, its fees, and any rent pump charged it).
    pub lamports_spent: u64,
}

/// The curve was already complete (or held no real tokens): nothing was done.
#[event]
pub struct CurveAlreadyComplete {
    pub mint: Pubkey,
    pub bonding_curve: Pubkey,
    /// `virtual_token_reserves` as read (0 once `migrate_v2` ran).
    pub virtual_token_reserves: u64,
}

/// The helper's errors, numbered from **8000** in declaration order (append only): clear of
/// pump's own 6000-range codes (its 6002 `TooMuchSolRequired` surfaces through this instruction
/// too), of PumpSwap's and of short.win's, so a failed bundle names its cause unambiguously.
#[error_code(offset = 8000)]
#[derive(PartialEq, Eq)]
pub enum CompleteCurveError {
    #[msg("The CPI target is not the pump program")]
    NotPump,
    #[msg("The bonding curve is not a pump BondingCurve")]
    BadCurve,
    #[msg("The buy left the curve incomplete")]
    NotCompleted,
}

/// The curve as pump stores it (`BadCurve` if it does not parse).
fn read_curve(info: &AccountInfo) -> Result<pump::BondingCurve> {
    let data = info.try_borrow_data()?;
    pump::read_curve(&info.owner.to_bytes(), &data)
        .map_err(|_| error!(CompleteCurveError::BadCurve))
}

fn meta(info: &AccountInfo, writable: bool, signer: bool) -> AccountMeta {
    AccountMeta {
        pubkey: info.key(),
        is_signer: signer,
        is_writable: writable,
    }
}

fn handler<'info>(ctx: Context<'info, CompleteCurve<'info>>, max_sol_cost: u64) -> Result<()> {
    let a = &ctx.accounts;
    let curve_info = a.bonding_curve.to_account_info();
    let curve = read_curve(&curve_info)?;
    if curve.complete || curve.real_token_reserves == 0 {
        emit!(CurveAlreadyComplete {
            mint: a.base_mint.key(),
            bonding_curve: a.bonding_curve.key(),
            virtual_token_reserves: curve.virtual_token_reserves,
        });
        return Ok(());
    }
    let amount = curve.real_token_reserves;
    let user = a.user.to_account_info();
    let before = user.lamports();

    // `buy_v2`'s accounts in pump's order (R-PUMP §3.5), `user` (index 13) the only signer;
    // built in place (a 27-entry array of `AccountInfo`s would weigh on the 4 KiB SBF frame).
    let extra = ctx.remaining_accounts.len();
    let mut metas: Vec<AccountMeta> = Vec::with_capacity(BUY_V2_ACCOUNTS.saturating_add(extra));
    let mut infos: Vec<AccountInfo<'info>> =
        Vec::with_capacity(BUY_V2_ACCOUNTS.saturating_add(extra));
    let mut push = |info: AccountInfo<'info>, writable: bool| {
        metas.push(meta(&info, writable, metas.len() == USER_INDEX));
        infos.push(info);
    };
    push(a.global.to_account_info(), false);
    push(a.base_mint.to_account_info(), false);
    push(a.quote_mint.to_account_info(), false);
    push(a.base_token_program.to_account_info(), false);
    push(a.quote_token_program.to_account_info(), false);
    push(a.associated_token_program.to_account_info(), false);
    push(a.fee_recipient.to_account_info(), true);
    push(a.associated_quote_fee_recipient.to_account_info(), true);
    push(a.buyback_fee_recipient.to_account_info(), true);
    push(
        a.associated_quote_buyback_fee_recipient.to_account_info(),
        true,
    );
    push(curve_info.clone(), true);
    push(a.associated_base_bonding_curve.to_account_info(), true);
    push(a.associated_quote_bonding_curve.to_account_info(), true);
    push(user.clone(), true);
    push(a.associated_base_user.to_account_info(), true);
    push(a.associated_quote_user.to_account_info(), true);
    push(a.creator_vault.to_account_info(), true);
    push(a.associated_creator_vault.to_account_info(), true);
    push(a.sharing_config.to_account_info(), false);
    push(a.global_volume_accumulator.to_account_info(), false);
    push(a.user_volume_accumulator.to_account_info(), true);
    push(a.associated_user_volume_accumulator.to_account_info(), true);
    push(a.fee_config.to_account_info(), false);
    push(a.fee_program.to_account_info(), false);
    push(a.system_program.to_account_info(), false);
    push(a.event_authority.to_account_info(), false);
    push(a.program.to_account_info(), false);
    debug_assert_eq!(metas.len(), BUY_V2_ACCOUNTS);
    // Trailing accounts pass through with the privileges the caller gave them.
    for info in ctx.remaining_accounts {
        metas.push(meta(info, info.is_writable, info.is_signer));
        infos.push(info.clone());
    }
    let ix = Instruction {
        program_id: PUMP_PROGRAM_ID,
        accounts: metas,
        data: buy_v2_data(amount, max_sol_cost).to_vec(),
    };
    invoke(&ix, &infos)?;

    // The buy of exactly `rT` completes the curve (R-PUMP §7.1); anything else would leave
    // `migrate_v2` failing in the bundle's next transaction, so fail here and revert the bundle.
    let after = read_curve(&curve_info)?;
    require!(
        after.complete && after.real_token_reserves == 0,
        CompleteCurveError::NotCompleted
    );
    emit!(CurveCompleted {
        mint: a.base_mint.key(),
        bonding_curve: a.bonding_curve.key(),
        user: a.user.key(),
        tokens: amount,
        lamports_spent: before.saturating_sub(user.lamports()),
    });
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sha256_8(s: &str) -> [u8; 8] {
        let h = solana_sha256_hasher::hash(s.as_bytes());
        <[u8; 8]>::try_from(&h.to_bytes()[..8]).unwrap()
    }

    /// pump's `buy_v2` discriminator and the 24-byte data (no cap byte, R-PUMP Update U7).
    #[test]
    fn buy_v2_data_is_the_idls_without_the_cap_byte() {
        assert_eq!(sha256_8("global:buy_v2"), IX_BUY_V2);
        let d = buy_v2_data(0x0102_0304_0506_0708, 0x1112_1314_1516_1718);
        assert_eq!(d.len(), 24);
        assert_eq!(d[..8], IX_BUY_V2);
        assert_eq!(
            u64::from_le_bytes(d[8..16].try_into().unwrap()),
            0x0102_0304_0506_0708
        );
        assert_eq!(
            u64::from_le_bytes(d[16..].try_into().unwrap()),
            0x1112_1314_1516_1718
        );
    }

    /// The pump id matches the reader crate's, and `user` / `bonding_curve` sit at pump's
    /// indices in the generated account list (Anchor keeps declaration order).
    #[test]
    fn the_account_list_is_buy_v2s() {
        assert_eq!(PUMP_PROGRAM_ID.to_bytes(), pump::PROGRAM);
        let k: Vec<Pubkey> = (0..BUY_V2_ACCOUNTS).map(|_| Pubkey::new_unique()).collect();
        let accs = crate::accounts::CompleteCurve {
            global: k[0],
            base_mint: k[1],
            quote_mint: k[2],
            base_token_program: k[3],
            quote_token_program: k[4],
            associated_token_program: k[5],
            fee_recipient: k[6],
            associated_quote_fee_recipient: k[7],
            buyback_fee_recipient: k[8],
            associated_quote_buyback_fee_recipient: k[9],
            bonding_curve: k[10],
            associated_base_bonding_curve: k[11],
            associated_quote_bonding_curve: k[12],
            user: k[13],
            associated_base_user: k[14],
            associated_quote_user: k[15],
            creator_vault: k[16],
            associated_creator_vault: k[17],
            sharing_config: k[18],
            global_volume_accumulator: k[19],
            user_volume_accumulator: k[20],
            associated_user_volume_accumulator: k[21],
            fee_config: k[22],
            fee_program: k[23],
            system_program: k[24],
            event_authority: k[25],
            program: k[26],
        };
        let metas = anchor_lang::ToAccountMetas::to_account_metas(&accs, None);
        assert_eq!(metas.len(), BUY_V2_ACCOUNTS);
        for (i, m) in metas.iter().enumerate() {
            assert_eq!(m.pubkey, k[i]);
            assert_eq!(m.is_signer, i == USER_INDEX, "signer flag of {i}");
        }
        // pump's writable set (R-PUMP §3.5 "W"): 6–17, 20, 21.
        let writable: Vec<usize> = metas
            .iter()
            .enumerate()
            .filter(|(_, m)| m.is_writable)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            writable,
            [6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 20, 21]
        );
        assert_eq!(k[BONDING_CURVE_INDEX], accs.bonding_curve);
    }

    /// The error codes sit at 8000.. (clear of pump's, PumpSwap's and short.win's 6000 range).
    #[test]
    fn error_codes_start_at_8000() {
        assert_eq!(u32::from(CompleteCurveError::NotPump), 8_000);
        assert_eq!(u32::from(CompleteCurveError::BadCurve), 8_001);
        assert_eq!(u32::from(CompleteCurveError::NotCompleted), 8_002);
    }
}
