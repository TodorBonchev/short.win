//! What the keeper reads each tick (ARCHITECTURE §7.1 "Inputs"): the config, every covered
//! `Market` with its `MarketVault`, the venue accounts of its ReadSet, and its open
//! `ShortPosition`s — decoded with the **program's own types** (the `shortwin` crate), and the
//! venue state computed by the **program's own reader** (`shortwin::venues::read_state` on
//! host-side `AccountInfo`s), so the keeper's view of a venue is the program's bit for bit
//! (R-TOOL §4.3: "the same crates as the program").

use std::collections::HashMap;

use anchor_lang::prelude::{AccountInfo, Pubkey};
use anchor_lang::{AccountDeserialize, Discriminator, ZeroCopy};
use shortwin::state::{
    GlobalConfig, Market, MarketVault, POSITION_OPEN, ShortPosition, VenueExtras, VenueKind,
};
use shortwin::venues::VenueState;
use shortwin_math::pool::Vault;
use shortwin_services_common::{AccountData, Chain, ChainClock, Memcmp};

use crate::{pda, venue};

/// Decodes a zero-copy short.win account (owner and discriminator checked).
pub fn decode_zc<T: ZeroCopy>(acc: &AccountData) -> Option<T> {
    if acc.owner != shortwin::ID || !acc.data.starts_with(T::DISCRIMINATOR) {
        return None;
    }
    let len = core::mem::size_of::<T>();
    let body = acc.data.get(8..8usize.checked_add(len)?)?;
    Some(bytemuck::pod_read_unaligned(body))
}

/// Decodes a Borsh short.win account (owner and discriminator checked).
pub fn decode_borsh<T: AccountDeserialize>(acc: &AccountData) -> Option<T> {
    if acc.owner != shortwin::ID {
        return None;
    }
    T::try_deserialize(&mut acc.data.as_slice()).ok()
}

/// An open position.
#[derive(Clone, Debug)]
pub struct PositionView {
    pub key: Pubkey,
    pub pos: ShortPosition,
    pub lamports: u64,
}

/// A covered market as the keeper sees it this tick.
#[derive(Clone)]
pub struct MarketView {
    pub key: Pubkey,
    pub market: Box<Market>,
    /// The lender-pool vault state (`MarketVault::vault`).
    pub vault: Box<Vault>,
    pub kind: VenueKind,
    /// The ReadSet keys, in order.
    pub read_keys: Vec<Pubkey>,
    /// The venue state the program's reader computes from them.
    pub state: VenueState,
    /// Open positions, oldest nonce first.
    pub positions: Vec<PositionView>,
    /// The venue's `VenueExtras` account (extras sets for trading exits), if any.
    pub extras: Option<Box<VenueExtras>>,
}

/// Everything read in one tick.
#[derive(Clone)]
pub struct ChainView {
    pub clock: ChainClock,
    pub config: Box<GlobalConfig>,
    pub markets: Vec<MarketView>,
    /// Markets skipped this tick, with why (unsupported venue, unreadable account).
    pub skipped: Vec<(Pubkey, String)>,
}

impl ChainView {
    /// The market `key`, if loaded.
    pub fn market(&self, key: &Pubkey) -> Option<&MarketView> {
        self.markets.iter().find(|m| m.key == *key)
    }
}

/// The venue state the program computes from a ReadSet (`venues::read_state` on host-side
/// `AccountInfo`s built from the fetched accounts; a missing account reads as an empty
/// System-owned one, which no reader accepts: "not executable", A63).
pub fn venue_state(
    kind: VenueKind,
    keys: &[Pubkey],
    accounts: &[Option<AccountData>],
) -> VenueState {
    let system = anchor_lang::system_program::ID;
    let mut lamports: Vec<u64> = accounts
        .iter()
        .map(|a| a.as_ref().map_or(0, |a| a.lamports))
        .collect();
    let mut datas: Vec<Vec<u8>> = accounts
        .iter()
        .map(|a| a.as_ref().map_or_else(Vec::new, |a| a.data.clone()))
        .collect();
    let owners: Vec<Pubkey> = accounts
        .iter()
        .map(|a| a.as_ref().map_or(system, |a| a.owner))
        .collect();
    let execs: Vec<bool> = accounts
        .iter()
        .map(|a| a.as_ref().is_some_and(|a| a.executable))
        .collect();
    let infos: Vec<AccountInfo> = keys
        .iter()
        .zip(lamports.iter_mut())
        .zip(datas.iter_mut())
        .zip(owners.iter())
        .zip(execs.iter())
        .map(|((((k, l), d), o), e)| AccountInfo::new(k, false, false, l, d.as_mut_slice(), o, *e))
        .collect();
    shortwin::venues::read_state(kind, &infos)
}

/// Every `Market` of the program (`getProgramAccounts` on its discriminator).
pub async fn discover_markets<C: Chain>(chain: &C) -> anyhow::Result<Vec<Pubkey>> {
    let found = chain
        .program_accounts(
            &shortwin::ID,
            &[Memcmp {
                offset: 0,
                bytes: Market::DISCRIMINATOR.to_vec(),
            }],
        )
        .await?;
    let mut keys: Vec<Pubkey> = found.into_iter().map(|(k, _)| k).collect();
    keys.sort();
    Ok(keys)
}

/// Every open `ShortPosition`, grouped by market (`getProgramAccounts` on its discriminator).
pub async fn open_positions<C: Chain>(
    chain: &C,
) -> anyhow::Result<HashMap<Pubkey, Vec<PositionView>>> {
    let found = chain
        .program_accounts(
            &shortwin::ID,
            &[Memcmp {
                offset: 0,
                bytes: ShortPosition::DISCRIMINATOR.to_vec(),
            }],
        )
        .await?;
    let mut out: HashMap<Pubkey, Vec<PositionView>> = HashMap::new();
    for (key, acc) in found {
        let Some(pos) = decode_borsh::<ShortPosition>(&acc) else {
            continue;
        };
        if pos.status != POSITION_OPEN {
            continue;
        }
        out.entry(pos.market).or_default().push(PositionView {
            key,
            pos,
            lamports: acc.lamports,
        });
    }
    for v in out.values_mut() {
        v.sort_by_key(|p| (p.pos.t_open, p.key));
    }
    Ok(out)
}

/// Loads the config and `markets` (with their vaults, ReadSets, extras and open positions).
pub async fn load<C: Chain>(chain: &C, markets: &[Pubkey]) -> anyhow::Result<ChainView> {
    let clock = chain.clock().await?;
    let mut keys = vec![pda::config()];
    keys.extend(markets.iter().copied());
    keys.extend(markets.iter().map(pda::market_vault));
    let accs = chain.accounts(&keys).await?;
    let config: GlobalConfig = accs
        .first()
        .and_then(|a| a.as_ref())
        .and_then(decode_zc)
        .ok_or_else(|| anyhow::anyhow!("GlobalConfig not found"))?;
    let n = markets.len();
    let mut skipped = Vec::new();
    // First pass: the markets and their ReadSet keys.
    struct Partial {
        key: Pubkey,
        market: Box<Market>,
        vault: Box<Vault>,
        kind: VenueKind,
        read_keys: Vec<Pubkey>,
    }
    let mut partial = Vec::with_capacity(n);
    for (i, key) in markets.iter().enumerate() {
        let m = accs
            .get(1 + i)
            .and_then(|a| a.as_ref())
            .and_then(decode_zc::<Market>);
        let v = accs
            .get(1 + n + i)
            .and_then(|a| a.as_ref())
            .and_then(decode_zc::<MarketVault>);
        let (Some(m), Some(v)) = (m, v) else {
            skipped.push((*key, "market or vault unreadable".to_string()));
            continue;
        };
        let Some(kind) = m.venue_kind() else {
            skipped.push((*key, format!("unknown venue byte {}", m.venue)));
            continue;
        };
        match venue::read_set(kind, &m) {
            Ok(read_keys) => partial.push(Partial {
                key: *key,
                vault: v.vault(),
                market: Box::new(m),
                kind,
                read_keys,
            }),
            Err(e) => skipped.push((*key, e.to_string())),
        }
    }
    // Second pass: the venue accounts and the extras accounts.
    let mut venue_keys: Vec<Pubkey> = partial
        .iter()
        .flat_map(|p| p.read_keys.iter().copied())
        .collect();
    let extras_keys: Vec<Pubkey> = partial
        .iter()
        .map(|p| {
            p.kind
                .extras_seed()
                .map_or(Pubkey::default(), pda::venue_extras)
        })
        .collect();
    let n_venue = venue_keys.len();
    venue_keys.extend(extras_keys.iter().copied());
    let venue_accs = chain.accounts(&venue_keys).await?;
    let positions = open_positions(chain).await?;
    let mut out = Vec::with_capacity(partial.len());
    let mut at = 0usize;
    for (i, p) in partial.into_iter().enumerate() {
        let len = p.read_keys.len();
        let slice = venue_accs.get(at..at + len).unwrap_or(&[]);
        at += len;
        let state = venue_state(p.kind, &p.read_keys, slice);
        let extras = venue_accs
            .get(n_venue + i)
            .and_then(|a| a.as_ref())
            .and_then(decode_zc::<VenueExtras>)
            .map(Box::new);
        out.push(MarketView {
            positions: positions.get(&p.key).cloned().unwrap_or_default(),
            key: p.key,
            market: p.market,
            vault: p.vault,
            kind: p.kind,
            read_keys: p.read_keys,
            state,
            extras,
        });
    }
    Ok(ChainView {
        clock,
        config: Box::new(config),
        markets: out,
        skipped,
    })
}
