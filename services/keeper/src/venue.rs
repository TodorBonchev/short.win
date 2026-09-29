//! Per-venue account lists (ARCHITECTURE §3.0 "Remaining-accounts convention", §3.12): the
//! keys of a market's `ReadSet` and `TradeExtras`, and the extras sets of A41 with the
//! placeholders resolved (ARCH §7.1 duty 10: "always select the extras set that simulates
//! successfully").
//!
//! The program key-checks every slot; this module only produces the same lists from the
//! market's caches — for Raydium through the adapters' own key functions
//! (`shortwin::venues::raydium_v4::{read_keys, trade_keys}`, the same in `raydium_cpmm`), so the
//! keeper's lists are the program's by construction. What the program does not state as data
//! is the **writability** of each slot in a trade (ARCH §3.12's "W" marks), which this module
//! adds: a slot the venue writes must be writable in the transaction (the CPI cannot escalate
//! it), and a slot it only reads stays read-only — a CPMM `AmmConfig` is shared by every pool
//! on that config, so write-locking it would contend with all of them (ARCH §6.6).
//!
//! **Pump curve** (M5): a `PumpCurve` or `PumpMigrating` market's ReadSet is the curve adapter's
//! own (`shortwin::venues::pump_curve::read_keys`: C0 curve W, C1 `Global`, C2 pump `FeeConfig`,
//! C3 the canonical pool, C4 the curve's base ATA W, C5 / C6 PumpSwap's fee and global configs),
//! and a `PumpCurve` trade's TradeExtras T0–T19 ([`trade_extras`]) come from the adapter's key
//! functions and the market's curve cache, with the three slots the program checks against the
//! **live** state read from this tick's ReadSet accounts: T6 / T8 a fee and a buyback recipient of
//! the live `Global` (one per market, spread by the mint's first byte so the keeper's exits do not
//! all write-lock the same recipient), T12 / T13 the creator vault of the live `curve.creator`
//! (R-PUMP §8). A `PumpMigrating` market has no trade (exits pause, `VenueMigrating`).
//!
//! **List-only** (`None`, M5): the program's "ReadSet" of a list-only market is its detection set
//! `[bonding_curve, canonical_pool]` (`crank_price`, which re-runs `classify_pump` on it, and
//! `sol_settle`, which runs the S3 of a curve that graduated elsewhere with positions open on it:
//! M2-SEC-5), read-only; a list-only market has no trade.
//!
//! Venues without an adapter in this build (PumpSwap arrives with M6) are skipped with a reason.
//! `VenueKind::Mock` exists only with the TEST-ONLY `test-venue` feature (A49).

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::AccountMeta;
use anyhow::{Context, anyhow, bail};
use shortwin::state::{
    ENTRY_FIXED, ENTRY_TEMPLATE, EXTRAS_MAX_SEEDS, ExtraEntry, ExtrasSet, Market,
    PLACEHOLDER_BONDING_CURVE, PLACEHOLDER_MINT, PLACEHOLDER_POOL, PLACEHOLDER_TRADER, SEED_CONST,
    SEED_PLACEHOLDER, VenueExtras, VenueKind,
};
use shortwin::venues::{
    EXTRAS_SET_CURRENT, EXTRAS_SET_PREVIOUS, EXTRAS_SET_STABLE, ExtrasSel, pump_curve,
    raydium_cpmm, raydium_v4,
};
use shortwin_services_common::AccountData;
use shortwin_venue_state::pump;

/// Which ReadSet slots are writable in a trade (ARCH §3.12), in the parser's order:
/// AMM v4 `[pool W, coin_vault W, pc_vault W, lp_mint]`, CPMM `[pool W, amm_config,
/// token_0_vault W, token_1_vault W, lp_mint]`, the pump curve `[curve W, Global, FeeConfig,
/// canonical pool, curve_base_ata W, PumpSwap FeeConfig, PumpSwap GlobalConfig]`, the mock
/// `[pool W, base vault W, WSOL vault W]`. `crank_price` and `sol_settle` pass every slot
/// read-only.
pub fn read_writable(kind: VenueKind) -> anyhow::Result<&'static [bool]> {
    const V4: [bool; raydium_v4::READ_LEN] = [true, true, true, false];
    const CPMM: [bool; raydium_cpmm::READ_LEN] = [true, false, true, true, false];
    const CURVE: [bool; pump_curve::READ_LEN] = [true, false, false, false, true, false, false];
    match kind {
        VenueKind::RaydiumV4 => Ok(&V4),
        VenueKind::RaydiumCpmm => Ok(&CPMM),
        VenueKind::PumpCurve | VenueKind::PumpMigrating => Ok(&CURVE),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => Ok(&[true, true, true]),
        // The detection set: read only (a list-only market never trades).
        VenueKind::None => Ok(&[false, false]),
        other => bail!("venue {other:?} has no adapter in this build (M6)"),
    }
}

/// The ReadSet keys of a market on `kind`, in the parser's order.
pub fn read_set(kind: VenueKind, market: &Market) -> anyhow::Result<Vec<Pubkey>> {
    match kind {
        VenueKind::RaydiumV4 => Ok(raydium_v4::read_keys(market).to_vec()),
        VenueKind::RaydiumCpmm => Ok(raydium_cpmm::read_keys(market).to_vec()),
        // A migrating market reads its curve's ReadSet (ARCH §3.12).
        VenueKind::PumpCurve | VenueKind::PumpMigrating => {
            Ok(pump_curve::read_keys(market).to_vec())
        }
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => {
            use shortwin::state::raydium_cache::{RAY_POOL, RAY_VAULT_SOL, RAY_VAULT_TOK};
            Ok(vec![
                market.pool_cache[RAY_POOL],
                market.pool_cache[RAY_VAULT_TOK],
                market.pool_cache[RAY_VAULT_SOL],
            ])
        }
        // The detection set (`crank_price`, `sol_settle`; module docs).
        VenueKind::None => Ok(vec![market.bonding_curve, market.canonical_pool]),
        other => {
            let _ = market;
            bail!("venue {other:?} has no adapter in this build (M6)")
        }
    }
}

/// The ReadSet metas of `keys` (a market's [`read_set`]) on `kind`: writable per
/// [`read_writable`] when `trading`, all read-only otherwise.
pub fn read_metas(
    kind: VenueKind,
    keys: &[Pubkey],
    trading: bool,
) -> anyhow::Result<Vec<AccountMeta>> {
    let writable = read_writable(kind)?;
    if writable.len() != keys.len() {
        bail!(
            "venue {kind:?}: {} ReadSet keys, the adapter takes {}",
            keys.len(),
            writable.len()
        );
    }
    Ok(keys
        .iter()
        .zip(writable)
        .map(|(k, w)| {
            if trading && *w {
                AccountMeta::new(*k, false)
            } else {
                AccountMeta::new_readonly(*k, false)
            }
        })
        .collect())
}

/// The TradeExtras metas of a market on `kind` (after the ReadSet in `liquidate` / `expire`);
/// `read` is this tick's ReadSet accounts (in [`read_set`]'s order), which the pump curve's
/// live-checked slots come from.
pub fn trade_extras(
    kind: VenueKind,
    market: &Market,
    read: &[Option<AccountData>],
) -> anyhow::Result<Vec<AccountMeta>> {
    let metas = |keys: &[Pubkey], writable: &[usize]| {
        keys.iter()
            .enumerate()
            .map(|(i, k)| {
                if writable.contains(&i) {
                    AccountMeta::new(*k, false)
                } else {
                    AccountMeta::new_readonly(*k, false)
                }
            })
            .collect::<Vec<_>>()
    };
    match kind {
        // X5 `trader_wsol` (W).
        VenueKind::RaydiumV4 => Ok(metas(
            &raydium_v4::trade_keys(market),
            &[raydium_v4::X_TRADER_WSOL],
        )),
        // X2 `observation` (W), X7 `trader_wsol` (W).
        VenueKind::RaydiumCpmm => Ok(metas(
            &raydium_cpmm::trade_keys(market),
            &[raydium_cpmm::X_OBSERVATION, raydium_cpmm::X_TRADER_WSOL],
        )),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => {
            use shortwin::constants::{
                MOCK_CPAMM_AUTHORITY, MOCK_CPAMM_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT,
            };
            Ok(vec![
                AccountMeta::new_readonly(MOCK_CPAMM_PROGRAM_ID, false),
                AccountMeta::new_readonly(MOCK_CPAMM_AUTHORITY, false),
                AccountMeta::new_readonly(WSOL_MINT, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new(market.trader_wsol, false),
            ])
        }
        VenueKind::PumpCurve => pump_trade_extras(market, read),
        VenueKind::PumpMigrating => bail!("a migrating curve has no trade (VenueMigrating)"),
        VenueKind::None => bail!("a list-only market has no trade (NoRoute)"),
        other => {
            let _ = (market, read);
            bail!("venue {other:?} has no adapter in this build (M6)")
        }
    }
}

/// The live pump `Global` and curve of a curve market's ReadSet accounts.
fn live_curve_accounts(
    read: &[Option<AccountData>],
) -> anyhow::Result<(&AccountData, pump::BondingCurve)> {
    let acc = |i: usize| read.get(i).and_then(Option::as_ref);
    let global = acc(pump_curve::C_GLOBAL).context("pump Global not read")?;
    let curve_acc = acc(pump_curve::C_CURVE).context("bonding curve not read")?;
    let curve = pump::read_curve(&curve_acc.owner.to_bytes(), &curve_acc.data)
        .map_err(|e| anyhow!("bonding curve: {e:?}"))?;
    Ok((global, curve))
}

/// A recipient of a live `Global` list for `market`: the first non-default entry from the
/// mint's first byte on (every entry is one pump accepts, C0-14).
fn recipient(list: [[u8; 32]; pump::RECIPIENTS], market: &Market) -> anyhow::Result<Pubkey> {
    let start = usize::from(market.mint.to_bytes()[0]);
    (0..pump::RECIPIENTS)
        .map(|k| list[(start + k) % pump::RECIPIENTS])
        .find(|r| *r != [0u8; 32])
        .map(Pubkey::new_from_array)
        .context("the live Global lists no recipient")
}

/// A live fee recipient and buyback recipient of the curve market's `Global`.
pub fn pump_recipients(
    market: &Market,
    read: &[Option<AccountData>],
) -> anyhow::Result<(Pubkey, Pubkey)> {
    let (global, _) = live_curve_accounts(read)?;
    let g = pump::read_global(&global.owner.to_bytes(), &global.data)
        .map_err(|e| anyhow!("pump Global: {e:?}"))?;
    Ok((
        recipient(g.fee_recipients(), market)?,
        recipient(g.buyback_fee_recipients(), market)?,
    ))
}

/// A `PumpCurve` trade's TradeExtras T0–T19 (ARCH §3.12; module docs): W at T6–T13, T16–T18.
fn pump_trade_extras(
    market: &Market,
    read: &[Option<AccountData>],
) -> anyhow::Result<Vec<AccountMeta>> {
    use shortwin::constants::{
        ASSOCIATED_TOKEN_PROGRAM_ID, PUMP_EVENT_AUTHORITY, PUMP_FEES_PROGRAM_ID,
        PUMP_GLOBAL_VOLUME_ACCUMULATOR, PUMP_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT,
    };
    let (fee_recipient, buyback) = pump_recipients(market, read)?;
    let (_, curve) = live_curve_accounts(read)?;
    let creator = Pubkey::new_from_array(curve.creator);
    let vault = if creator == market.cached_creator {
        market.cached_creator_vault
    } else {
        pump_curve::creator_vault(&creator)
    };
    let keys: [(Pubkey, bool); pump_curve::TRADE_LEN] = [
        (PUMP_PROGRAM_ID, false),
        (PUMP_EVENT_AUTHORITY, false),
        (PUMP_FEES_PROGRAM_ID, false),
        (WSOL_MINT, false),
        (TOKEN_PROGRAM_ID, false),
        (ASSOCIATED_TOKEN_PROGRAM_ID, false),
        (fee_recipient, true),
        (pump_curve::wsol_ata(&fee_recipient), true),
        (buyback, true),
        (pump_curve::wsol_ata(&buyback), true),
        (market.curve_quote_ata, true),
        (pump_curve::wsol_ata(&market.trader), true),
        (vault, true),
        (pump_curve::wsol_ata(&vault), true),
        (market.sharing_config, false),
        (PUMP_GLOBAL_VOLUME_ACCUMULATOR, false),
        (market.pump_uva, true),
        (market.pump_uva_quote_ata, true),
        (market.trader_base_ata, true),
        (anchor_lang::system_program::ID, false),
    ];
    Ok(keys
        .into_iter()
        .map(|(k, w)| {
            if w {
                AccountMeta::new(k, false)
            } else {
                AccountMeta::new_readonly(k, false)
            }
        })
        .collect())
}

/// The pool a template's `pool` placeholder stands for (`venues::pool_key`).
fn pool_placeholder(kind: VenueKind, market: &Market) -> Option<Pubkey> {
    shortwin::venues::pool_key(market, kind)
}

/// One selectable extras set of a trading instruction: the selection and the trailing metas
/// (the `VenueExtras` account first, then its entries), empty for `none`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtrasChoice {
    pub sel: ExtrasSel,
    pub metas: Vec<AccountMeta>,
}

impl ExtrasChoice {
    /// `none`: nothing follows the adapter's list.
    pub fn none() -> Self {
        Self {
            sel: ExtrasSel::NONE,
            metas: Vec::new(),
        }
    }
}

fn resolve_entry(entry: &ExtraEntry, kind: VenueKind, market: &Market) -> Option<(Pubkey, u8)> {
    match entry.kind {
        ENTRY_FIXED => Some((entry.key, 0)),
        ENTRY_TEMPLATE => {
            let n = usize::from(entry.n_seeds);
            if n == 0 || n > EXTRAS_MAX_SEEDS {
                return None;
            }
            let mut owned: Vec<Vec<u8>> = Vec::with_capacity(n);
            for part in entry.seeds.iter().take(n) {
                match part.kind {
                    SEED_CONST => owned.push(part.bytes.get(..usize::from(part.len))?.to_vec()),
                    SEED_PLACEHOLDER => {
                        let key = match part.len {
                            PLACEHOLDER_MINT => Some(market.mint),
                            PLACEHOLDER_POOL => pool_placeholder(kind, market),
                            PLACEHOLDER_TRADER => Some(market.trader),
                            PLACEHOLDER_BONDING_CURVE => Some(market.bonding_curve),
                            // The coin creator is not resolvable from the market cache.
                            _ => None,
                        }?;
                        owned.push(key.to_bytes().to_vec());
                    }
                    _ => return None,
                }
            }
            let seeds: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
            Some(Pubkey::find_program_address(&seeds, &entry.key))
        }
        _ => None,
    }
}

fn choice_of(
    set: &ExtrasSet,
    which: u8,
    extras_key: Pubkey,
    kind: VenueKind,
    market: &Market,
) -> Option<ExtrasChoice> {
    let n = usize::from(set.count);
    if n == 0 {
        return None;
    }
    let mut metas = vec![AccountMeta::new_readonly(extras_key, false)];
    let mut bumps = [0u8; 4];
    for (i, entry) in set.entries.iter().take(n).enumerate() {
        let (key, bump) = resolve_entry(entry, kind, market)?;
        if let Some(b) = bumps.get_mut(i) {
            *b = bump;
        }
        metas.push(AccountMeta {
            pubkey: key,
            is_signer: false,
            is_writable: entry.writable == 1,
        });
    }
    Some(ExtrasChoice {
        sel: ExtrasSel { set: which, bumps },
        metas,
    })
}

/// The extras sets to try for instruction kind `ix` (`EXTRAS_IX_*`), in order: `current`
/// (the Emergency's latest), `none`, `previous`, `stable` (M2-SEC-2: the set that stood for a
/// week, or Governance's last) — a set with no entries is only ever `none`. A set whose
/// template cannot be resolved from the market is left out.
pub fn extras_candidates(
    kind: VenueKind,
    ix: usize,
    market: &Market,
    extras: Option<&VenueExtras>,
) -> Vec<ExtrasChoice> {
    let mut out = Vec::with_capacity(4);
    let (Some(ve), Some(seed)) = (extras, kind.extras_seed()) else {
        return vec![ExtrasChoice::none()];
    };
    let key = crate::pda::venue_extras(seed);
    let Some(ixe) = ve.sets.get(ix) else {
        return vec![ExtrasChoice::none()];
    };
    if let Some(c) = choice_of(&ixe.current, EXTRAS_SET_CURRENT, key, kind, market) {
        out.push(c);
    }
    out.push(ExtrasChoice::none());
    if let Some(c) = choice_of(&ixe.previous, EXTRAS_SET_PREVIOUS, key, kind, market) {
        out.push(c);
    }
    if let Some(c) = choice_of(&ixe.stable, EXTRAS_SET_STABLE, key, kind, market) {
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use shortwin::state::raydium_cache::{
        RAY_AMM_CONFIG, RAY_LP_MINT, RAY_OBSERVATION, RAY_POOL, RAY_VAULT_SOL, RAY_VAULT_TOK,
    };

    fn market(sol_first: bool, token_program: Pubkey) -> Market {
        let mut m: Market = bytemuck::Zeroable::zeroed();
        for i in [
            RAY_POOL,
            RAY_VAULT_SOL,
            RAY_VAULT_TOK,
            RAY_AMM_CONFIG,
            RAY_OBSERVATION,
            RAY_LP_MINT,
        ] {
            m.pool_cache[i] = Pubkey::new_unique();
        }
        m.trader_wsol = Pubkey::new_unique();
        m.token_program = token_program;
        if sol_first {
            m.flags |= shortwin::state::MARKET_FLAG_SOL_IS_TOKEN0_OR_COIN;
        }
        m
    }

    fn flags(metas: &[AccountMeta]) -> Vec<bool> {
        metas.iter().map(|m| m.is_writable).collect()
    }

    /// ARCH §3.12: the keeper's lists are the adapters' own keys, with the "W" marks of the
    /// table, and no signer anywhere.
    #[test]
    fn raydium_lists_are_the_adapters_with_archs_writability() {
        use shortwin::constants::{TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID};
        for sol_first in [false, true] {
            for program in [TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID] {
                let m = market(sol_first, program);
                // AMM v4: pool, coin vault, pc vault (WSOL side per the flag), LP mint.
                let keys = read_set(VenueKind::RaydiumV4, &m).unwrap();
                assert_eq!(keys, raydium_v4::read_keys(&m).to_vec());
                let (sol, tok) = (m.pool_cache[RAY_VAULT_SOL], m.pool_cache[RAY_VAULT_TOK]);
                let (coin, pc) = if sol_first { (sol, tok) } else { (tok, sol) };
                assert_eq!(
                    keys,
                    vec![m.pool_cache[RAY_POOL], coin, pc, m.pool_cache[RAY_LP_MINT]]
                );
                let trade = read_metas(VenueKind::RaydiumV4, &keys, true).unwrap();
                assert_eq!(flags(&trade), [true, true, true, false]);
                let crank = read_metas(VenueKind::RaydiumV4, &keys, false).unwrap();
                assert!(crank.iter().all(|m| !m.is_writable));
                let x = trade_extras(VenueKind::RaydiumV4, &m, &[]).unwrap();
                assert_eq!(
                    x.iter().map(|m| m.pubkey).collect::<Vec<_>>(),
                    raydium_v4::trade_keys(&m).to_vec()
                );
                assert_eq!(flags(&x), [false, false, false, false, false, true]);
                assert_eq!(x[raydium_v4::X_TRADER_WSOL].pubkey, m.trader_wsol);

                // CPMM: pool, config, token 0 vault, token 1 vault, LP mint.
                let keys = read_set(VenueKind::RaydiumCpmm, &m).unwrap();
                assert_eq!(keys, raydium_cpmm::read_keys(&m).to_vec());
                let (v0, v1) = if sol_first { (sol, tok) } else { (tok, sol) };
                assert_eq!(
                    keys,
                    vec![
                        m.pool_cache[RAY_POOL],
                        m.pool_cache[RAY_AMM_CONFIG],
                        v0,
                        v1,
                        m.pool_cache[RAY_LP_MINT]
                    ]
                );
                let trade = read_metas(VenueKind::RaydiumCpmm, &keys, true).unwrap();
                assert_eq!(flags(&trade), [true, false, true, true, false]);
                let x = trade_extras(VenueKind::RaydiumCpmm, &m, &[]).unwrap();
                assert_eq!(
                    x.iter().map(|m| m.pubkey).collect::<Vec<_>>(),
                    raydium_cpmm::trade_keys(&m).to_vec()
                );
                assert_eq!(
                    flags(&x),
                    [false, false, true, false, false, false, false, true]
                );
                assert_eq!(
                    x[raydium_cpmm::X_OBSERVATION].pubkey,
                    m.pool_cache[RAY_OBSERVATION]
                );
                assert_eq!(x[raydium_cpmm::X_TOKEN_SIDE_PROGRAM].pubkey, program);
                for meta in trade.iter().chain(&x) {
                    assert!(!meta.is_signer);
                }
            }
        }
    }

    #[test]
    fn venues_without_an_adapter_are_skipped_with_a_reason() {
        let m = market(false, Pubkey::new_unique());
        assert!(read_set(VenueKind::PumpSwap, &m).is_err());
        assert!(read_writable(VenueKind::PumpSwap).is_err());
        assert!(trade_extras(VenueKind::PumpSwap, &m, &[]).is_err());
        assert!(trade_extras(VenueKind::PumpMigrating, &m, &[]).is_err());
        assert!(trade_extras(VenueKind::None, &m, &[]).is_err());
        // A ReadSet of the wrong length is refused, never padded.
        assert!(read_metas(VenueKind::RaydiumV4, &[Pubkey::new_unique()], true).is_err());
    }

    /// A list-only market's ReadSet is its detection set, read-only even in a trade's position
    /// (the program's `crank_price` / `sol_settle` on a `None` market, M2-SEC-5).
    #[test]
    fn a_list_only_market_reads_its_detection_set() {
        let mut m = market(false, Pubkey::new_unique());
        m.bonding_curve = Pubkey::new_unique();
        m.canonical_pool = Pubkey::new_unique();
        let keys = read_set(VenueKind::None, &m).unwrap();
        assert_eq!(keys, vec![m.bonding_curve, m.canonical_pool]);
        assert_eq!(keys.len(), shortwin::venues::DETECTION_LEN);
        for trading in [false, true] {
            let metas = read_metas(VenueKind::None, &keys, trading).unwrap();
            assert!(metas.iter().all(|a| !a.is_writable && !a.is_signer));
        }
    }

    /// A pump `Global` of 1,087 bytes with `fee` / `buyback` as its recipient lists (the
    /// reader's offsets: `fee_recipient` @41, `fee_recipients[7]` @162, buyback @741).
    fn global(fee: [Pubkey; 8], buyback: [Pubkey; 8]) -> AccountData {
        let mut d = vec![0u8; 1_087];
        d[..8].copy_from_slice(&[0xa7, 0xe8, 0xe8, 0xb1, 0xc8, 0x6c, 0x72, 0x7f]);
        d[41..73].copy_from_slice(fee[0].as_ref());
        for (i, k) in fee[1..].iter().enumerate() {
            d[162 + 32 * i..194 + 32 * i].copy_from_slice(k.as_ref());
        }
        for (i, k) in buyback.iter().enumerate() {
            d[741 + 32 * i..773 + 32 * i].copy_from_slice(k.as_ref());
        }
        AccountData {
            lamports: 1,
            owner: shortwin::constants::PUMP_PROGRAM_ID,
            executable: false,
            data: d,
        }
    }

    /// A live 151-byte curve with `creator`.
    fn curve(creator: Pubkey) -> AccountData {
        let mut d = vec![0u8; 151];
        d[..8].copy_from_slice(&pump::BONDING_CURVE_DISCRIMINATOR);
        d[8..16].copy_from_slice(&1_000_000u64.to_le_bytes());
        d[16..24].copy_from_slice(&1_000u64.to_le_bytes());
        d[24..32].copy_from_slice(&500_000u64.to_le_bytes());
        d[49..81].copy_from_slice(creator.as_ref());
        AccountData {
            lamports: 1,
            owner: shortwin::constants::PUMP_PROGRAM_ID,
            executable: false,
            data: d,
        }
    }

    /// ARCH §3.12 PumpCurve: the ReadSet is the adapter's, C0 and C4 writable in trades; the
    /// TradeExtras are T0–T19 with the live `Global`'s recipients, the live creator's vault
    /// (the cache's when it is the cached creator) and the cached curve keys, writable at
    /// T6–T13 and T16–T18.
    #[test]
    fn pump_curve_lists_are_the_adapters_with_the_live_recipients() {
        use shortwin::constants::PUMP_PROGRAM_ID;
        let mut m = market(false, Pubkey::new_unique());
        m.bonding_curve = Pubkey::new_unique();
        m.canonical_pool = Pubkey::new_unique();
        m.trader = Pubkey::new_unique();
        m.trader_base_ata = Pubkey::new_unique();
        m.mint = Pubkey::new_from_array([3u8; 32]);
        let cached = Pubkey::new_unique();
        pump_curve::write_cache(&mut m, &cached);
        for kind in [VenueKind::PumpCurve, VenueKind::PumpMigrating] {
            assert_eq!(
                read_set(kind, &m).unwrap(),
                pump_curve::read_keys(&m).to_vec()
            );
            let metas = read_metas(kind, &read_set(kind, &m).unwrap(), true).unwrap();
            assert_eq!(
                flags(&metas),
                [true, false, false, false, true, false, false]
            );
        }
        let fee: [Pubkey; 8] = core::array::from_fn(|_| Pubkey::new_unique());
        let bb: [Pubkey; 8] = core::array::from_fn(|_| Pubkey::new_unique());
        for (creator, vault) in [
            (cached, m.cached_creator_vault),
            (Pubkey::new_unique(), Pubkey::default()),
        ] {
            let mut read: Vec<Option<AccountData>> = vec![None; pump_curve::READ_LEN];
            read[pump_curve::C_CURVE] = Some(curve(creator));
            read[pump_curve::C_GLOBAL] = Some(global(fee, bb));
            let x = trade_extras(VenueKind::PumpCurve, &m, &read).unwrap();
            assert_eq!(x.len(), pump_curve::TRADE_LEN);
            assert_eq!(x[pump_curve::T_PROGRAM].pubkey, PUMP_PROGRAM_ID);
            // The mint's first byte (3) picks entry 3 of each list.
            assert_eq!(x[pump_curve::T_FEE_RECIPIENT].pubkey, fee[3]);
            assert_eq!(x[pump_curve::T_BUYBACK_RECIPIENT].pubkey, bb[3]);
            assert_eq!(
                x[pump_curve::T_FEE_RECIPIENT_ATA].pubkey,
                pump_curve::wsol_ata(&fee[3])
            );
            let want = if vault == Pubkey::default() {
                pump_curve::creator_vault(&creator)
            } else {
                vault
            };
            assert_eq!(x[pump_curve::T_CREATOR_VAULT].pubkey, want);
            assert_eq!(
                x[pump_curve::T_CREATOR_VAULT_ATA].pubkey,
                pump_curve::wsol_ata(&want)
            );
            assert_eq!(x[pump_curve::T_UVA].pubkey, m.pump_uva);
            assert_eq!(x[pump_curve::T_TRADER_BASE_ATA].pubkey, m.trader_base_ata);
            assert_eq!(
                x[pump_curve::T_TRADER_WSOL_ATA].pubkey,
                pump_curve::wsol_ata(&m.trader)
            );
            let w: Vec<usize> = x
                .iter()
                .enumerate()
                .filter(|(_, a)| a.is_writable)
                .map(|(i, _)| i)
                .collect();
            assert_eq!(w, [6, 7, 8, 9, 10, 11, 12, 13, 16, 17, 18]);
            assert!(x.iter().all(|a| !a.is_signer));
        }
        // No Global read: no trade.
        assert!(trade_extras(VenueKind::PumpCurve, &m, &[]).is_err());
    }
}
