//! Graduation duties on the pump curve (ARCHITECTURE §5.1, §5.4, §7.1 duty 6; ADOPTED Q14, Q16;
//! A14, A15, A66; M5 part): what the keeper sends when a curve completes, and the straddle
//! bundle for a position whose `Q` exceeds the curve's remaining real tokens `rT`.
//!
//! **`migrate_v2` as soon as a curve completes** ([`migration_due`], [`migrate_v2_ix`]): a
//! covered `PumpCurve` or `PumpMigrating` market whose curve is complete with `vT > 0` and whose
//! canonical PumpSwap pool does not exist yet. pump's `migrate_v2` is permissionless and
//! idempotent (R-PUMP §7.4); it is sent **top-level, alone with its compute budget** (A15: a CPI
//! from short.win would reach stack height 5; its 51 trace entries leave 13 of the 64, ARCH §5.4 —
//! `straddle_it` measures the completion and the migration merged at exactly 64 with a tip, one
//! over the limit when pump also has to create the completer's UVA), with its 27 IDL accounts plus
//! the two boost accounts mainnet passes (R-PUMP §3.6).
//! The keeper then observes the market in the same second, so the prologue records the
//! transition (M5: `→ PumpMigrating`; M6 adds `→ PumpSwap` with the ring rewrite and the pool's
//! first-trade duties — `trader_base_ata`, the PumpSwap UVA, `extend_account` of the new 271-byte
//! pool to 301 bytes).
//!
//! **A list-only market back on a pump curve, and the trader's curve accounts** ([`redetection_due`],
//! [`trader_account_ixs`]; ARCH §2.5 `None` row): a list-only market whose detection set shows a
//! live or migrating curve (the program's own `classify_pump`) is cranked, which moves it onto the
//! curve; `crank_price` has no payer and creates no account, so the keeper then creates the
//! trader's base ATA and pump UVA the curve legs require (T16, T18: an open refuses
//! `VenueAccountMismatch` / `TraderStateInvalid` without them) — checked once per curve market
//! (`create_market` creates both, so a market listed on its curve needs nothing).
//!
//! **The straddle bundle** ([`plan_straddle`]; ARCH §5.4 case 2): pump rejects any buy above `rT`
//! (6021) and never fills partially, so a liquidatable position with `Q > rT` on a live curve
//! (`decide::Plan::Straddle`) exits through a Jito bundle of at least three transactions
//! (sequential, all-or-nothing, a tip; U22):
//!
//! | tx | instructions | M5 |
//! |---|---|---|
//! | tx1 | ComputeBudget ×2 + the keeper's base ATA (`create_idempotent`) + `complete_curve(max_sol_cost)` of the `complete-curve` helper (A66: it reads `rT` **at execution** and buys it with the keeper as pump's `user`; a no-op on a complete curve) | built ([`complete_curve_ix`]) |
//! | tx2 | ComputeBudget ×2 + `migrate_v2` top-level (+ the tip when last) | built ([`migrate_v2_ix`]) |
//! | tx3 | ComputeBudget ×2 + PumpSwap UVA / `extend_account` if needed + `crank_price` (→ PumpSwap) + `liquidate` / `expire` on PumpSwap (+ the tip) | **M6** ([`exit_tx`] returns `None`: the PumpSwap adapter, its ReadSet and TradeExtras and the `→ PumpSwap` transition are M6's) |
//!
//! `max_sol_cost` is the keeper's own quote of buying `rT` on the live curve (`sol_leg::
//! buy_exact_out` at the live curve fees) plus `complete_slippage_bps` and a rent allowance (the
//! creator-vault top-up pump may take first, R-PUMP Update U6): a curve trade that lands between
//! the keeper's read and the bundle either lowers the cost (a buy) or raises it within the bound
//! (a sale); beyond it pump reverts tx1 (6002) and the whole bundle, and the keeper rebuilds it the
//! next second (T-STRADDLE, `tests/litesvm/tests/straddle_it.rs`).
//!
//! **Keeper fallback** (V1-9): when tx3 would fail (its simulation returns `UseSolSettle`,
//! `ExitBoundNotMet` or `InsufficientVault`: [`FALLBACK_CODES`]) the keeper sends tx1 + tx2 only
//! and treats the position as an ordinary PumpSwap position in the following seconds. **In M5
//! there is no tx3 and no PumpSwap position**: a migrated curve stays `PumpMigrating` (exits
//! pause, S3 after the migration pause), so sending tx1 + tx2 would freeze the market's positions
//! instead of exiting them. The M5 keeper therefore sends the two-transaction bundle only when
//! `graduation.send_without_exit` is set (off by default; the scenario tests set it); otherwise
//! it reports the straddle as waiting and calls `sol_settle` for the position (the decision
//! tree's last branch: it arms, keeps the clock, or settles S1 / S2).

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::{InstructionData, ToAccountMetas};
use anyhow::{Context, anyhow, bail};
use shortwin::constants::{
    ASSOCIATED_TOKEN_PROGRAM_ID, PUMP_AMM_EVENT_AUTHORITY, PUMP_AMM_GLOBAL_CONFIG,
    PUMP_AMM_PROGRAM_ID, PUMP_EVENT_AUTHORITY, PUMP_FEE_CONFIG, PUMP_FEES_PROGRAM_ID, PUMP_GLOBAL,
    PUMP_GLOBAL_VOLUME_ACCUMULATOR, PUMP_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
    WSOL_MINT,
};
use shortwin::errors::ShortwinError;
use shortwin::state::{Market, VenueKind};
use shortwin::venues::detect::PumpClass;
use shortwin::venues::pump_curve;
use shortwin_math::quote::sol_leg;
use shortwin_services_common::{AccountData, BundleOutcome, TxOutcome};
use shortwin_venue_state::{pump, pump_amm};

use crate::venue;
use crate::view::{MarketView, PositionView};

/// pump `migrate_v2`'s discriminator (`idl/pump.json`, R-PUMP §3.6).
pub const PUMP_MIGRATE_V2: [u8; 8] = [0xbb, 0xcb, 0x12, 0x1f, 0xce, 0xed, 0xfe, 0x29];

/// The Rent sysvar `migrate_v2` names.
pub const SYSVAR_RENT: Pubkey =
    anchor_lang::prelude::pubkey!("SysvarRent111111111111111111111111111111111");

/// The compute-unit limit of the bundle's `migrate_v2` transaction: it cannot be simulated before
/// tx1 lands on a plain JSON-RPC node (the curve is not complete yet). Measured 329,758 CU on the
/// fork (`straddle_it`), 322,371–358,072 on mainnet (ARCH §6.1, ECON T7/T8).
pub const MIGRATE_CU: u32 = 400_000;

/// The compute-unit limit of a bundle transaction nothing could simulate (M6's tx3 on a node
/// without bundle simulation): ARCH §6.1's PROPOSED keeper-exit limit.
pub const UNSIMULATED_CU: u32 = 300_000;

/// Lamports `max_sol_cost` carries on top of the quote for rent pump may charge the completer
/// before the curve transfer (the creator-vault top-up to `Rent.minimum_balance(0)`, 650,240 at
/// mainnet's rent; R-PUMP Update U6).
pub const RENT_ALLOWANCE: u64 = 1_000_000;

/// The Jito mainnet tip accounts (`getTipAccounts` on the block engine, read 2026-09-29).
pub const JITO_TIP_ACCOUNTS: [&str; 8] = [
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
];

/// The tx3 simulation failures after which the keeper sends tx1 + tx2 only (V1-9, ARCH §5.4).
pub const FALLBACK_CODES: [ShortwinError; 3] = [
    ShortwinError::UseSolSettle,
    ShortwinError::ExitBoundNotMet,
    ShortwinError::InsufficientVault,
];

/// Whether a tx3 simulation failure with custom error `code` is one the fallback covers.
pub fn falls_back(code: Option<u32>) -> bool {
    code.is_some_and(|c| FALLBACK_CODES.iter().any(|e| u32::from(*e) == c))
}

// ---- keys -----------------------------------------------------------------------------------

/// `ATA(owner, mint)` under `token_program`.
pub fn ata(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}

/// The canonical PumpSwap pool `migrate_v2` creates for a SOL curve's mint and its accounts
/// (R-VEN §1.3, R-PUMP §3.6; the harness's `pump::NewPool` derives the same).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigrationKeys {
    /// `PDA(pump, ["pool-authority", mint])`: the pool's creator.
    pub pool_authority: Pubkey,
    /// `PDA(pAMM, ["pool", 0u16, pool_authority, mint, WSOL])` (the market's `canonical_pool`).
    pub pool: Pubkey,
    /// `PDA(pAMM, ["pool_lp_mint", pool])`.
    pub lp_mint: Pubkey,
    /// `ATA(pool, mint)` and `ATA(pool, WSOL)`: the pool's vaults.
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    /// `PDA(pAMM, ["boost_vault", pool])` and its WSOL ATA (the two remaining accounts).
    pub boost_vault: Pubkey,
    pub boost_vault_ata: Pubkey,
}

impl MigrationKeys {
    /// The keys of `mint`'s migration (base token program `base_program`).
    pub fn of(mint: &Pubkey, base_program: &Pubkey) -> Self {
        let pool_authority = Pubkey::find_program_address(
            &[pump::POOL_AUTHORITY_SEED, mint.as_ref()],
            &PUMP_PROGRAM_ID,
        )
        .0;
        let pool = Pubkey::find_program_address(
            &[
                b"pool",
                &0u16.to_le_bytes(),
                pool_authority.as_ref(),
                mint.as_ref(),
                WSOL_MINT.as_ref(),
            ],
            &PUMP_AMM_PROGRAM_ID,
        )
        .0;
        let lp_mint =
            Pubkey::find_program_address(&[b"pool_lp_mint", pool.as_ref()], &PUMP_AMM_PROGRAM_ID).0;
        let boost_vault = Pubkey::find_program_address(
            &[pump_amm::BOOST_VAULT_SEED, pool.as_ref()],
            &PUMP_AMM_PROGRAM_ID,
        )
        .0;
        Self {
            pool_authority,
            pool,
            lp_mint,
            base_vault: ata(&pool, mint, base_program),
            quote_vault: ata(&pool, &WSOL_MINT, &TOKEN_PROGRAM_ID),
            boost_vault,
            boost_vault_ata: ata(&boost_vault, &WSOL_MINT, &TOKEN_PROGRAM_ID),
        }
    }
}

fn w(k: Pubkey) -> AccountMeta {
    AccountMeta::new(k, false)
}

fn r(k: Pubkey) -> AccountMeta {
    AccountMeta::new_readonly(k, false)
}

/// The pump `Global` of a curve market's ReadSet accounts, parsed.
fn global_of(read: &[Option<AccountData>]) -> anyhow::Result<&AccountData> {
    read.get(pump_curve::C_GLOBAL)
        .and_then(Option::as_ref)
        .context("pump Global not read")
}

// ---- instructions ---------------------------------------------------------------------------

/// pump `migrate_v2` of `market`'s curve by `user` (anyone, R-PUMP §7.4): the 27 IDL accounts
/// (`idl/pump.json`) — `withdraw_authority` from the live `Global` (not a signer) — plus the boost
/// vault and its WSOL ATA; `read` is the market's ReadSet accounts.
pub fn migrate_v2_ix(
    user: &Pubkey,
    market: &Market,
    read: &[Option<AccountData>],
) -> anyhow::Result<Instruction> {
    let g = global_of(read)?;
    let global =
        pump::read_global(&g.owner.to_bytes(), &g.data).map_err(|e| anyhow!("Global: {e:?}"))?;
    let withdraw_authority = Pubkey::new_from_array(global.withdraw_authority());
    let mint = market.mint;
    let base = market.token_program;
    let k = MigrationKeys::of(&mint, &base);
    if k.pool != market.canonical_pool {
        bail!(
            "the derived canonical pool {} is not the market's {}",
            k.pool,
            market.canonical_pool
        );
    }
    let accounts = vec![
        r(PUMP_GLOBAL),
        w(withdraw_authority),
        r(mint),
        r(WSOL_MINT),
        w(market.bonding_curve),
        w(market.curve_base_ata),
        w(market.curve_quote_ata),
        AccountMeta::new(*user, true),
        r(anchor_lang::system_program::ID),
        r(PUMP_AMM_PROGRAM_ID),
        w(k.pool),
        w(k.pool_authority),
        w(ata(&k.pool_authority, &mint, &base)),
        w(ata(&k.pool_authority, &WSOL_MINT, &TOKEN_PROGRAM_ID)),
        r(PUMP_AMM_GLOBAL_CONFIG),
        w(k.lp_mint),
        w(ata(&k.pool_authority, &k.lp_mint, &TOKEN_2022_PROGRAM_ID)),
        w(k.base_vault),
        w(k.quote_vault),
        r(base),
        r(TOKEN_PROGRAM_ID),
        r(TOKEN_2022_PROGRAM_ID),
        r(ASSOCIATED_TOKEN_PROGRAM_ID),
        r(PUMP_AMM_EVENT_AUTHORITY),
        r(SYSVAR_RENT),
        r(PUMP_EVENT_AUTHORITY),
        r(PUMP_PROGRAM_ID),
        r(k.boost_vault),
        w(k.boost_vault_ata),
    ];
    Ok(Instruction {
        program_id: PUMP_PROGRAM_ID,
        accounts,
        data: PUMP_MIGRATE_V2.to_vec(),
    })
}

/// `create_associated_token_account_idempotent(payer, owner, mint)` under `token_program`.
pub fn create_ata_idempotent_ix(
    payer: &Pubkey,
    owner: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Instruction {
    Instruction {
        program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            w(ata(owner, mint, token_program)),
            r(*owner),
            r(*mint),
            r(anchor_lang::system_program::ID),
            r(*token_program),
        ],
        data: vec![1],
    }
}

/// pump `init_user_volume_accumulator(payer, user)` (R-PUMP §3.6; ARCH §3.2 "Effects"):
/// `[payer S W, user, user_volume_accumulator W, system, event_authority, program]`. Anyone may
/// create the UVA of any user (payer ≠ user); pump refuses a second creation.
pub fn pump_init_uva_ix(payer: &Pubkey, user: &Pubkey) -> Instruction {
    Instruction {
        program_id: PUMP_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            r(*user),
            w(pump_curve::user_volume_accumulator(user)),
            r(anchor_lang::system_program::ID),
            r(PUMP_EVENT_AUTHORITY),
            r(PUMP_PROGRAM_ID),
        ],
        data: pump_curve::IX_INIT_USER_VOLUME_ACCUMULATOR.to_vec(),
    }
}

/// The trader accounts a curve market's legs require (ARCH §3.12 PumpCurve T16, T18): the
/// trader's base ATA and its pump UVA, in that order. `create_market` creates both; `crank_price`'s
/// `None →` pump re-detection creates neither (it has no payer), so the keeper does
/// ([`trader_account_ixs`]).
pub fn trader_account_keys(market: &Market) -> [Pubkey; 2] {
    [market.trader_base_ata, market.pump_uva]
}

/// The instructions that create the trader accounts of [`trader_account_keys`] `missing` marks,
/// paid by `payer`: `create_idempotent(trader, mint)` and pump `init_user_volume_accumulator(payer,
/// user = trader)`.
pub fn trader_account_ixs(payer: &Pubkey, market: &Market, missing: [bool; 2]) -> Vec<Instruction> {
    let mut v = Vec::new();
    if missing[0] {
        v.push(create_ata_idempotent_ix(
            payer,
            &market.trader,
            &market.mint,
            &market.token_program,
        ));
    }
    if missing[1] {
        v.push(pump_init_uva_ix(payer, &market.trader));
    }
    v
}

/// `complete_curve(max_sol_cost)` of the `complete-curve` helper (A66) by `user` on `market`'s
/// curve: pump `buy_v2`'s 27 accounts (R-PUMP §3.5) with `user` the signer, receiving into its
/// base ATA, a fee and a buyback recipient of the live `Global` and the creator vault of the live
/// curve (the same three live slots as a curve exit's TradeExtras, [`venue::trade_extras`]).
pub fn complete_curve_ix(
    user: &Pubkey,
    market: &Market,
    read: &[Option<AccountData>],
    max_sol_cost: u64,
) -> anyhow::Result<Instruction> {
    let (fee_recipient, buyback) = venue::pump_recipients(market, read)?;
    let curve_acc = read
        .get(pump_curve::C_CURVE)
        .and_then(Option::as_ref)
        .context("bonding curve not read")?;
    let curve = pump::read_curve(&curve_acc.owner.to_bytes(), &curve_acc.data)
        .map_err(|e| anyhow!("bonding curve: {e:?}"))?;
    let creator_vault = pump_curve::creator_vault(&Pubkey::new_from_array(curve.creator));
    let uva = pump_curve::user_volume_accumulator(user);
    let accounts = complete_curve::accounts::CompleteCurve {
        global: PUMP_GLOBAL,
        base_mint: market.mint,
        quote_mint: WSOL_MINT,
        base_token_program: market.token_program,
        quote_token_program: TOKEN_PROGRAM_ID,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        fee_recipient,
        associated_quote_fee_recipient: pump_curve::wsol_ata(&fee_recipient),
        buyback_fee_recipient: buyback,
        associated_quote_buyback_fee_recipient: pump_curve::wsol_ata(&buyback),
        bonding_curve: market.bonding_curve,
        associated_base_bonding_curve: market.curve_base_ata,
        associated_quote_bonding_curve: market.curve_quote_ata,
        user: *user,
        associated_base_user: ata(user, &market.mint, &market.token_program),
        associated_quote_user: pump_curve::wsol_ata(user),
        creator_vault,
        associated_creator_vault: pump_curve::wsol_ata(&creator_vault),
        sharing_config: market.sharing_config,
        global_volume_accumulator: PUMP_GLOBAL_VOLUME_ACCUMULATOR,
        user_volume_accumulator: uva,
        associated_user_volume_accumulator: pump_curve::wsol_ata(&uva),
        fee_config: PUMP_FEE_CONFIG,
        fee_program: PUMP_FEES_PROGRAM_ID,
        system_program: anchor_lang::system_program::ID,
        event_authority: PUMP_EVENT_AUTHORITY,
        program: PUMP_PROGRAM_ID,
    };
    Ok(Instruction {
        program_id: complete_curve::ID,
        accounts: accounts.to_account_metas(None),
        data: complete_curve::instruction::CompleteCurve { max_sol_cost }.data(),
    })
}

/// A System transfer of `lamports` from `from` to the Jito tip account `to`.
pub fn tip_ix(from: &Pubkey, to: &Pubkey, lamports: u64) -> Instruction {
    let mut data = vec![2u8, 0, 0, 0];
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: anchor_lang::system_program::ID,
        accounts: vec![AccountMeta::new(*from, true), w(*to)],
        data,
    }
}

// ---- decisions ------------------------------------------------------------------------------

/// `migrate_v2` is due on `mv` (module docs): a curve market whose curve is complete with
/// `vT > 0` (not migrated) and whose canonical pool is not valid yet.
pub fn migration_due(mv: &MarketView) -> bool {
    matches!(mv.kind, VenueKind::PumpCurve | VenueKind::PumpMigrating)
        && mv
            .state
            .curve
            .as_ref()
            .is_some_and(|c| c.migrating() && !c.canonical_pool_valid())
}

/// A list-only market whose detection set shows a live or migrating pump curve (the program's
/// `classify_pump` on this tick's accounts, [`crate::pool_monitor::classify`]): `crank_price`
/// moves it onto that venue (ARCH §2.5 `None` row; M5), so the keeper cranks it.
pub fn redetection_due(mv: &MarketView) -> bool {
    mv.kind == VenueKind::None
        && matches!(
            crate::pool_monitor::classify(&mv.market, &mv.read_accounts),
            Ok(PumpClass::PumpCurve | PumpClass::PumpMigrating)
        )
}

/// The keeper's quote of completing `mv`'s live curve: `rT` and `buy_exact_out(rT)` on the
/// live `(vT, vQ)` at the live curve fees (ε₁ = 1, ≥ pump's own charge; ECON §3.1). `None` off
/// a live curve or when a fee input does not parse.
pub fn completion_quote(mv: &MarketView) -> Option<(u64, u64)> {
    let c = mv.state.curve.as_ref()?;
    if c.live.complete || c.live.rt == 0 {
        return None;
    }
    let fees = c.pricing.as_ref()?.fees;
    let rt = u64::try_from(c.live.rt).ok()?;
    let q = sol_leg::buy_exact_out(rt, c.live.vt, c.live.vq, true, &fees)?;
    Some((rt, q.cost))
}

/// `max_sol_cost` for a completion quoted at `quote` lamports: `⌈quote·(10⁴ + slippage)/10⁴⌉ +`
/// [`RENT_ALLOWANCE`].
pub fn max_sol_cost(quote: u64, slippage_bps: u32) -> u64 {
    let scaled = u128::from(quote)
        .saturating_mul(10_000u128.saturating_add(u128::from(slippage_bps)))
        .div_ceil(10_000);
    u64::try_from(scaled)
        .unwrap_or(u64::MAX)
        .saturating_add(RENT_ALLOWANCE)
}

/// tx3 of the straddle bundle for `pv` (ARCH §5.4): `crank_price` (→ PumpSwap) and `liquidate`
/// / `expire` on the new pool, with the PumpSwap UVA and `extend_account` when needed. **M5:
/// `None`** — the PumpSwap adapter (its ReadSet and TradeExtras), the `→ PumpSwap` transition
/// that tx3's crank must make, and therefore any short.win instruction on the pool the bundle
/// creates, are M6's. M6 builds it here from the post-migration pool's derived keys
/// ([`MigrationKeys`]) and the program's `pump_swap` key functions.
pub fn exit_tx(_mv: &MarketView, _pv: &PositionView) -> Option<Vec<Instruction>> {
    None
}

/// A straddle bundle for one position (module docs).
#[derive(Clone, Debug)]
pub struct StraddlePlan {
    /// `rT` as the keeper read it (tx1 buys whatever is live when it runs).
    pub rt_read: u64,
    /// The keeper's quote of buying `rt_read` now.
    pub quote: u64,
    /// tx1's bound.
    pub max_sol_cost: u64,
    /// tx1: the keeper's base ATA (idempotent) and `complete_curve`.
    pub tx1: Vec<Instruction>,
    /// tx2: `migrate_v2`.
    pub tx2: Vec<Instruction>,
    /// tx3: the exit on the new pool (M6; [`exit_tx`]).
    pub tx3: Option<Vec<Instruction>>,
}

/// The straddle bundle of `pv` on `mv` with `keeper` as the completer (tips not included: the
/// sender appends the tip to the bundle's last transaction). Errors when the curve is not live
/// or an input does not parse.
pub fn plan_straddle(
    keeper: &Pubkey,
    mv: &MarketView,
    pv: &PositionView,
    slippage_bps: u32,
) -> anyhow::Result<StraddlePlan> {
    let (rt_read, quote) = completion_quote(mv).context("no live curve to complete")?;
    let max = max_sol_cost(quote, slippage_bps);
    let m = &mv.market;
    let tx1 = vec![
        create_ata_idempotent_ix(keeper, keeper, &m.mint, &m.token_program),
        complete_curve_ix(keeper, m, &mv.read_accounts, max)?,
    ];
    let tx2 = vec![migrate_v2_ix(keeper, m, &mv.read_accounts)?];
    Ok(StraddlePlan {
        rt_read,
        quote,
        max_sol_cost: max,
        tx1,
        tx2,
        tx3: exit_tx(mv, pv),
    })
}

// ---- records --------------------------------------------------------------------------------

/// What became of a straddle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StraddleResult {
    /// The bundle landed; `with_exit`: it carried tx3 (M6).
    Landed {
        outcome: BundleOutcome,
        with_exit: bool,
    },
    /// Sent, and the bundle did not land (dropped, refused by the engine, or a transaction failed).
    Failed { outcome: BundleOutcome },
    /// Its pre-flight failed: transaction `tx` (0-based) with `outcome` (e.g. tx1 with pump's 6002
    /// when the completion now costs more than `max_sol_cost`).
    Refused { tx: usize, outcome: TxOutcome },
    /// Dry run: simulated only.
    DryRun { outcomes: Vec<TxOutcome> },
    /// Not sent (the keeper calls `sol_settle` for the position instead): no exit transaction
    /// before M6 and `send_without_exit` is off, no block engine, or the completion costs more
    /// than `max_complete_lamports`.
    Waiting { reason: String },
    /// A transport or building error.
    Error(String),
}

impl StraddleResult {
    /// Whether the bundle landed.
    pub fn landed(&self) -> bool {
        matches!(self, Self::Landed { .. })
    }
}

/// One straddle attempt.
#[derive(Clone, Debug)]
pub struct StraddleRecord {
    pub market: Pubkey,
    pub position: Pubkey,
    /// `rT` read, the quote, the bound, and the transactions of the bundle (2 or 3).
    pub rt_read: u64,
    pub quote: u64,
    pub max_sol_cost: u64,
    pub txs: usize,
    pub result: StraddleResult,
}

/// One `migrate_v2` attempt of the keeper.
#[derive(Clone, Debug)]
pub struct MigrateRecord {
    pub market: Pubkey,
    pub result: crate::keeper::ExitResult,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_sol_cost_adds_the_slippage_and_the_rent_allowance() {
        assert_eq!(
            max_sol_cost(1_000_000_000, 500),
            1_050_000_000 + RENT_ALLOWANCE
        );
        assert_eq!(max_sol_cost(3, 1), 4 + RENT_ALLOWANCE, "rounded up");
        assert_eq!(max_sol_cost(u64::MAX, 10_000), u64::MAX, "saturates");
    }

    #[test]
    fn the_fallback_covers_archs_three_codes_only() {
        for e in FALLBACK_CODES {
            assert!(falls_back(Some(u32::from(e))));
        }
        assert!(!falls_back(Some(u32::from(ShortwinError::NotLiquidatable))));
        assert!(!falls_back(None));
    }

    /// `migrate_v2` names 29 accounts, `user` the only signer, `withdraw_authority` from the live
    /// `Global`, the canonical pool the market caches, and the discriminator
    /// `sha256("global:migrate_v2")[..8]` (the fork scenario checks the whole list against the
    /// harness's own derivation, `graduation::keeper_migrates_a_completed_curve`).
    #[test]
    fn migrate_v2_names_the_canonical_pool() {
        let mint = Pubkey::new_unique();
        let k = MigrationKeys::of(&mint, &TOKEN_2022_PROGRAM_ID);
        let mut m: Market = bytemuck::Zeroable::zeroed();
        m.mint = mint;
        m.token_program = TOKEN_2022_PROGRAM_ID;
        m.canonical_pool = k.pool;
        m.bonding_curve = Pubkey::new_unique();
        let mut g = vec![0u8; 1_087];
        g[..8].copy_from_slice(&[0xa7, 0xe8, 0xe8, 0xb1, 0xc8, 0x6c, 0x72, 0x7f]);
        let wa = Pubkey::new_unique();
        g[113..145].copy_from_slice(wa.as_ref());
        let mut read: Vec<Option<AccountData>> = vec![None; pump_curve::READ_LEN];
        read[pump_curve::C_GLOBAL] = Some(AccountData {
            lamports: 1,
            owner: PUMP_PROGRAM_ID,
            executable: false,
            data: g,
        });
        let user = Pubkey::new_unique();
        let ix = migrate_v2_ix(&user, &m, &read).unwrap();
        assert_eq!(ix.accounts.len(), 29);
        assert_eq!(ix.accounts[1].pubkey, wa);
        assert!(!ix.accounts[1].is_signer);
        assert_eq!(ix.accounts[7].pubkey, user);
        let signers: Vec<usize> = ix
            .accounts
            .iter()
            .enumerate()
            .filter(|(_, a)| a.is_signer)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(signers, [7]);
        assert_eq!(ix.accounts[10].pubkey, k.pool);
        assert_eq!(ix.accounts[27].pubkey, k.boost_vault);
        let h = solana_sha256_hasher::hash(b"global:migrate_v2");
        assert_eq!(PUMP_MIGRATE_V2, h.to_bytes()[..8]);
        // A market whose cached pool is not the derived one is refused.
        m.canonical_pool = Pubkey::new_unique();
        assert!(migrate_v2_ix(&user, &m, &read).is_err());
    }
}
