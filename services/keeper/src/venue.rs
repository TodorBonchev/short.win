//! Per-venue account lists (ARCHITECTURE §3.0 "Remaining-accounts convention", §3.12): the
//! keys of a market's `ReadSet` and `TradeExtras`, and the extras sets of A41 with the
//! placeholders resolved (ARCH §7.1 duty 10: "always select the extras set that simulates
//! successfully").
//!
//! The program key-checks every slot; this module only produces the same lists from the
//! market's caches. Venues without an adapter in this build (the MVP venues arrive with M3–M6)
//! are skipped with a reason. `VenueKind::Mock` exists only with the TEST-ONLY `test-venue`
//! feature (A49).

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::AccountMeta;
use anyhow::bail;
use shortwin::state::{
    ENTRY_FIXED, ENTRY_TEMPLATE, EXTRAS_MAX_SEEDS, ExtraEntry, ExtrasSet, Market,
    PLACEHOLDER_BONDING_CURVE, PLACEHOLDER_MINT, PLACEHOLDER_POOL, PLACEHOLDER_TRADER, SEED_CONST,
    SEED_PLACEHOLDER, VenueExtras, VenueKind,
};
use shortwin::venues::{EXTRAS_SET_CURRENT, EXTRAS_SET_PREVIOUS, EXTRAS_SET_STABLE, ExtrasSel};

/// The ReadSet keys of a market on `kind`, in the parser's order.
pub fn read_set(kind: VenueKind, market: &Market) -> anyhow::Result<Vec<Pubkey>> {
    match kind {
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
            bail!("venue {other:?} has no adapter in this build (M3–M6)")
        }
    }
}

/// The TradeExtras metas of a market on `kind` (after the ReadSet in `liquidate` / `expire`).
pub fn trade_extras(kind: VenueKind, market: &Market) -> anyhow::Result<Vec<AccountMeta>> {
    match kind {
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
            bail!("venue {other:?} has no adapter in this build (M3–M6)")
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
