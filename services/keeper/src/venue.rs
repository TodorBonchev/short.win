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
//! Venues without an adapter in this build (the pump venues arrive with M5–M6) are skipped with
//! a reason. `VenueKind::Mock` exists only with the TEST-ONLY `test-venue` feature (A49).

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::AccountMeta;
use anyhow::bail;
use shortwin::state::{
    ENTRY_FIXED, ENTRY_TEMPLATE, EXTRAS_MAX_SEEDS, ExtraEntry, ExtrasSet, Market,
    PLACEHOLDER_BONDING_CURVE, PLACEHOLDER_MINT, PLACEHOLDER_POOL, PLACEHOLDER_TRADER, SEED_CONST,
    SEED_PLACEHOLDER, VenueExtras, VenueKind,
};
use shortwin::venues::{
    EXTRAS_SET_CURRENT, EXTRAS_SET_PREVIOUS, EXTRAS_SET_STABLE, ExtrasSel, raydium_cpmm, raydium_v4,
};

/// Which ReadSet slots are writable in a trade (ARCH §3.12), in the parser's order:
/// AMM v4 `[pool W, coin_vault W, pc_vault W, lp_mint]`, CPMM `[pool W, amm_config,
/// token_0_vault W, token_1_vault W, lp_mint]`, the mock `[pool W, base vault W, WSOL vault W]`.
/// `crank_price` and `sol_settle` pass every slot read-only.
pub fn read_writable(kind: VenueKind) -> anyhow::Result<&'static [bool]> {
    const V4: [bool; raydium_v4::READ_LEN] = [true, true, true, false];
    const CPMM: [bool; raydium_cpmm::READ_LEN] = [true, false, true, true, false];
    match kind {
        VenueKind::RaydiumV4 => Ok(&V4),
        VenueKind::RaydiumCpmm => Ok(&CPMM),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => Ok(&[true, true, true]),
        VenueKind::None => bail!("list-only market (no venue)"),
        other => bail!("venue {other:?} has no adapter in this build (M5–M6)"),
    }
}

/// The ReadSet keys of a market on `kind`, in the parser's order.
pub fn read_set(kind: VenueKind, market: &Market) -> anyhow::Result<Vec<Pubkey>> {
    match kind {
        VenueKind::RaydiumV4 => Ok(raydium_v4::read_keys(market).to_vec()),
        VenueKind::RaydiumCpmm => Ok(raydium_cpmm::read_keys(market).to_vec()),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => {
            use shortwin::state::raydium_cache::{RAY_POOL, RAY_VAULT_SOL, RAY_VAULT_TOK};
            Ok(vec![
                market.pool_cache[RAY_POOL],
                market.pool_cache[RAY_VAULT_TOK],
                market.pool_cache[RAY_VAULT_SOL],
            ])
        }
        VenueKind::None => bail!("list-only market (no venue)"),
        other => {
            let _ = market;
            bail!("venue {other:?} has no adapter in this build (M5–M6)")
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

/// The TradeExtras metas of a market on `kind` (after the ReadSet in `liquidate` / `expire`).
pub fn trade_extras(kind: VenueKind, market: &Market) -> anyhow::Result<Vec<AccountMeta>> {
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
        other => {
            let _ = market;
            bail!("venue {other:?} has no adapter in this build (M5–M6)")
        }
    }
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
                let x = trade_extras(VenueKind::RaydiumV4, &m).unwrap();
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
                let x = trade_extras(VenueKind::RaydiumCpmm, &m).unwrap();
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
        for kind in [
            VenueKind::None,
            VenueKind::PumpCurve,
            VenueKind::PumpSwap,
            VenueKind::PumpMigrating,
        ] {
            assert!(read_set(kind, &m).is_err(), "{kind:?}");
            assert!(read_writable(kind).is_err(), "{kind:?}");
        }
        assert!(trade_extras(VenueKind::PumpSwap, &m).is_err());
        // A ReadSet of the wrong length is refused, never padded.
        assert!(read_metas(VenueKind::RaydiumV4, &[Pubkey::new_unique()], true).is_err());
    }
}
