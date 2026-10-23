//! What the keeper reads each tick (ARCHITECTURE §7.1 "Inputs"): the config, every covered
//! `Market` with its `MarketVault`, the venue accounts of its ReadSet, and its open
//! `ShortPosition`s — decoded with the **program's own types** (the `shortwin` crate), and the
//! venue state computed by the **program's own reader** (`shortwin::venues::read_state` on
//! host-side `AccountInfo`s, [`with_account_infos`]), so the keeper's view of a venue is the
//! program's bit for bit (R-TOOL §4.3: "the same crates as the program").
//!
//! List-only (`None`) markets have no venue; their "ReadSet" is the detection set
//! `[bonding_curve, canonical_pool]` (the program's `crank_price` re-runs `classify_pump` on it,
//! and `sol_settle` runs a graduated-elsewhere market's S3 on it: M5, M2-SEC-5), so they are
//! covered markets too — a list-only market with positions open (a curve that graduated elsewhere)
//! is kept on the no-route cadence and its positions are settled in S3 when due (duty 5), and one
//! whose detection set shows a pump venue again is cranked onto it ([`crate::keeper`]).
//! [`ChainView::list_only`] also hands them to the pool monitor (duty 12, [`crate::pool_monitor`]),
//! which moves them onto an eligible Raydium pool with `propose_pool` (ARCH §2.5 `None`).
//!
//! **Which markets a tick reads** (M7-SEC-8): the open positions first (one `getProgramAccounts`),
//! then in full — `Market`, `MarketVault`, ReadSet, extras — every market with positions or with
//! something time-critical pending, and the other ("cold") markets every `cold_interval_s`,
//! staggered ([`crate::keeper`]); as built at M7 every discovered market was read every tick,
//! so permissionless junk listings (≈ 0.035–0.038 SOL each at ARCH §3.2's rent) added ≈ 0.05 sequential RPC calls to every
//! tick each. `RpcChain` issues a read's `getMultipleAccounts` chunks in parallel.
//!
//! **PumpSwap** (M6): the fee tier is chosen on the market cap `effective_quote·supply/base`
//! (ARCH §4.2), so the keeper also reads the market's **mint** and hands its live supply to the
//! program's reader (`venues::read_state_boxed_for_exit`, as the program's exits do with the named
//! mint); without it a PumpSwap market would read as not executable. That reader also resolves
//! the fee at the stored `P_ema`'s market cap, which caps the exits' risk view (AIR-10;
//! [`venue_state`]). **Pump curve** (pump-final §5.2): the mint's supply resolves the fee pump
//! charges on a buy past `rT` by synthetic migration (`CurvePricingInputs::synthetic_fees`), so a
//! `PumpCurve` market's mint is read too.

use std::collections::HashMap;

use anchor_lang::prelude::{AccountInfo, Pubkey};
use anchor_lang::{AccountDeserialize, Discriminator, ZeroCopy};
use shortwin::constants::{CONFIG_VERSION, MARKET_VERSION, VENUE_EXTRAS_VERSION};
use shortwin::state::{
    GlobalConfig, Market, MarketVault, OracleCache, POSITION_OPEN, ShortPosition, VenueExtras,
    VenueKind,
};
use shortwin::venues::VenueState;
use shortwin_math::pool::Vault;
use shortwin_services_common::{AccountData, Chain, ChainClock, Memcmp};

use crate::{pda, venue};

/// Decodes a zero-copy short.win account (owner, discriminator and size checked). Typed
/// callers must check the layout version before interpreting accounts that have one.
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
    /// The ReadSet accounts as fetched (`None`: missing), for the admin monitor
    /// ([`crate::monitor`]), which reads the venue's status and fee fields from them.
    pub read_accounts: Vec<Option<AccountData>>,
    /// The venue state the program's reader computes from them.
    pub state: VenueState,
    /// Open positions, oldest nonce first.
    pub positions: Vec<PositionView>,
    /// The venue's `VenueExtras` account (extras sets for trading exits), if any.
    pub extras: Option<Box<VenueExtras>>,
    /// The mint's live supply (read for PumpSwap markets — the fee tier's market cap — and pump
    /// curve markets — the synthetic exit's fee tier, pump-final §5.2).
    pub supply: Option<u64>,
}

/// A covered list-only (`None`) market as the pool monitor sees it (module docs; the same
/// market is also in [`ChainView::markets`] with its detection set as ReadSet).
#[derive(Clone)]
pub struct ListOnlyView {
    pub key: Pubkey,
    pub market: Box<Market>,
}

/// Everything read in one tick.
#[derive(Clone)]
pub struct ChainView {
    pub clock: ChainClock,
    pub config: Box<GlobalConfig>,
    /// The cached SOL/USD price (`None`: not initialized): the tier a market would have on a
    /// window candidate, whose opening caps its open positions must fit ("switch only if all
    /// fit", [`crate::pool_monitor::evaluate`]).
    pub oracle: Option<Box<OracleCache>>,
    pub markets: Vec<MarketView>,
    /// The covered list-only markets, for the pool monitor (they are also in `markets`, with
    /// their detection set as ReadSet, for the coverage duties).
    pub list_only: Vec<ListOnlyView>,
    /// Markets skipped this tick, with why (unsupported venue, unreadable account).
    pub skipped: Vec<(Pubkey, String)>,
}

impl ChainView {
    /// The market `key`, if loaded.
    pub fn market(&self, key: &Pubkey) -> Option<&MarketView> {
        self.markets.iter().find(|m| m.key == *key)
    }
}

/// Runs `f` on host-side `AccountInfo`s of `keys` built from the fetched `accounts` (in
/// order; read-only, not signers): the program's own readers and verifiers then run on the
/// keeper's copy of the chain exactly as on-chain. A missing account (or one past the end of
/// `accounts`) reads as an empty System-owned one, which no reader accepts.
pub fn with_account_infos<R>(
    keys: &[Pubkey],
    accounts: &[Option<AccountData>],
    f: impl FnOnce(&[AccountInfo]) -> R,
) -> R {
    let system = anchor_lang::system_program::ID;
    let acc = |i: usize| accounts.get(i).and_then(Option::as_ref);
    let mut lamports: Vec<u64> = (0..keys.len())
        .map(|i| acc(i).map_or(0, |a| a.lamports))
        .collect();
    let mut datas: Vec<Vec<u8>> = (0..keys.len())
        .map(|i| acc(i).map_or_else(Vec::new, |a| a.data.clone()))
        .collect();
    let owners: Vec<Pubkey> = (0..keys.len())
        .map(|i| acc(i).map_or(system, |a| a.owner))
        .collect();
    let execs: Vec<bool> = (0..keys.len())
        .map(|i| acc(i).is_some_and(|a| a.executable))
        .collect();
    let infos: Vec<AccountInfo> = keys
        .iter()
        .zip(lamports.iter_mut())
        .zip(datas.iter_mut())
        .zip(owners.iter())
        .zip(execs.iter())
        .map(|((((k, l), d), o), e)| AccountInfo::new(k, false, false, l, d.as_mut_slice(), o, *e))
        .collect();
    f(&infos)
}

/// The venue state the program's keeper exits and `sol_settle` compute from a ReadSet
/// (`venues::read_state_boxed_for_exit` on host-side `AccountInfo`s built from the fetched
/// accounts, with the mint's `supply` — PumpSwap's fee tier; a missing account reads as an empty
/// System-owned one, which no reader accepts: "not executable", A63). It is
/// `venues::read_state_with`'s state except that on PumpSwap the fee at the stored `P_ema`'s
/// market cap is resolved too: the exits cap their risk view there (AIR-10,
/// `VenueState::exit_risk_cap_bps`), so the keeper's projection judges them as the program does
/// ([`crate::decide`]). Nothing else reads that fee: the prologue, the route predicates and the
/// monitor's fee take the live one.
pub fn venue_state(
    market: &Market,
    kind: VenueKind,
    keys: &[Pubkey],
    accounts: &[Option<AccountData>],
    now: i64,
    supply: Option<u64>,
) -> VenueState {
    with_account_infos(keys, accounts, |infos| {
        *shortwin::venues::read_state_boxed_for_exit(market, kind, infos, now, supply)
    })
}

/// The supply of a fetched mint account (`supply` @36; SPL Token or Token-2022; the program's
/// `venues::mint_supply`), `None` if it does not parse.
pub fn mint_supply(acc: Option<&AccountData>) -> Option<u64> {
    let a = acc?;
    shortwin_venue_state::mint::read_base(&a.owner.to_bytes(), &a.data)
        .ok()
        .map(|m| m.supply)
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

/// Discover an explicit market allowlist with one batched account read instead of scanning every
/// program account. Match `discover_markets`' owner and discriminator filter here; the full
/// market load still checks the layout version and reports unsupported accounts as skipped.
/// Missing keys are silently excluded, as they are when intersecting the global discovery.
pub async fn discover_selected_markets<C: Chain>(
    chain: &C,
    selected: &[Pubkey],
) -> anyhow::Result<Vec<Pubkey>> {
    let mut keys = selected.to_vec();
    keys.sort_unstable();
    keys.dedup();
    if keys.is_empty() {
        return Ok(keys);
    }
    let accounts = chain.accounts(&keys).await?;
    anyhow::ensure!(
        accounts.len() == keys.len(),
        "incomplete selected-market discovery: {} accounts for {} keys",
        accounts.len(),
        keys.len()
    );
    Ok(keys
        .into_iter()
        .zip(accounts)
        .filter_map(|(key, account)| {
            account
                .filter(|a| a.owner == shortwin::ID && a.data.starts_with(Market::DISCRIMINATOR))
                .map(|_| key)
        })
        .collect())
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

/// Loads the config, the oracle cache and `markets` (with their vaults, ReadSets, extras and open
/// positions).
pub async fn load<C: Chain>(chain: &C, markets: &[Pubkey]) -> anyhow::Result<ChainView> {
    let positions = open_positions(chain).await?;
    load_with_positions(chain, markets, &positions).await
}

/// [`load`] with the open positions already read ([`open_positions`]): the tick reads them first
/// to choose the markets it reads in full (M7-SEC-8, [`crate::keeper`]).
pub async fn load_with_positions<C: Chain>(
    chain: &C,
    markets: &[Pubkey],
    positions: &HashMap<Pubkey, Vec<PositionView>>,
) -> anyhow::Result<ChainView> {
    let clock = chain.clock().await?;
    let mut keys = vec![pda::config()];
    keys.extend(markets.iter().copied());
    keys.extend(markets.iter().map(pda::market_vault));
    keys.push(pda::oracle_cache());
    let accs = chain.accounts(&keys).await?;
    let config: GlobalConfig = accs
        .first()
        .and_then(|a| a.as_ref())
        .and_then(decode_zc)
        .ok_or_else(|| anyhow::anyhow!("GlobalConfig not found"))?;
    anyhow::ensure!(
        config.version == CONFIG_VERSION,
        "unsupported GlobalConfig layout version {} (expected {CONFIG_VERSION})",
        config.version,
    );
    let n = markets.len();
    let oracle = accs
        .get(1 + 2 * n)
        .and_then(|a| a.as_ref())
        .and_then(decode_borsh::<OracleCache>)
        .map(Box::new);
    let mut skipped = Vec::new();
    let mut list_only = Vec::new();
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
        if m.version != MARKET_VERSION {
            skipped.push((
                *key,
                format!(
                    "unsupported Market layout version {} (expected {MARKET_VERSION})",
                    m.version,
                ),
            ));
            continue;
        }
        let Some(kind) = m.venue_kind() else {
            skipped.push((*key, format!("unknown venue byte {}", m.venue)));
            continue;
        };
        if kind == VenueKind::None {
            list_only.push(ListOnlyView {
                key: *key,
                market: Box::new(m),
            });
        }
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
    // Second pass: the venue accounts (a PumpSwap or live-curve market's mint after its ReadSet:
    // the supply) and the extras accounts.
    let with_mint = |p: &Partial| matches!(p.kind, VenueKind::PumpSwap | VenueKind::PumpCurve);
    let mut venue_keys: Vec<Pubkey> = partial
        .iter()
        .flat_map(|p| {
            p.read_keys
                .iter()
                .copied()
                .chain(with_mint(p).then_some(p.market.mint))
        })
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
    let mut out = Vec::with_capacity(partial.len());
    let mut at = 0usize;
    for (i, p) in partial.into_iter().enumerate() {
        let len = p.read_keys.len();
        let slice = venue_accs.get(at..at + len).unwrap_or(&[]);
        at += len;
        let supply = if with_mint(&p) {
            let s = mint_supply(venue_accs.get(at).and_then(Option::as_ref));
            at += 1;
            s
        } else {
            None
        };
        let state = venue_state(
            &p.market,
            p.kind,
            &p.read_keys,
            slice,
            clock.unix_timestamp,
            supply,
        );
        let extras = venue_accs
            .get(n_venue + i)
            .and_then(|a| a.as_ref())
            .and_then(decode_zc::<VenueExtras>)
            .filter(|extras| extras.version == VENUE_EXTRAS_VERSION)
            .map(Box::new);
        out.push(MarketView {
            positions: positions.get(&p.key).cloned().unwrap_or_default(),
            key: p.key,
            market: p.market,
            vault: p.vault,
            kind: p.kind,
            read_keys: p.read_keys,
            read_accounts: slice.to_vec(),
            state,
            extras,
            supply,
        });
    }
    Ok(ChainView {
        clock,
        config: Box::new(config),
        oracle,
        markets: out,
        list_only,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use shortwin_services_common::TxOutcome;
    use solana_hash::Hash;
    use solana_transaction::versioned::VersionedTransaction;

    struct DiscoveryChain {
        accounts: HashMap<Pubkey, AccountData>,
        reads: RefCell<Vec<Vec<Pubkey>>>,
    }

    impl Chain for DiscoveryChain {
        async fn clock(&self) -> anyhow::Result<ChainClock> {
            unreachable!("selected discovery needs no clock")
        }

        async fn accounts(&self, keys: &[Pubkey]) -> anyhow::Result<Vec<Option<AccountData>>> {
            self.reads.borrow_mut().push(keys.to_vec());
            Ok(keys.iter().map(|k| self.accounts.get(k).cloned()).collect())
        }

        async fn program_accounts(
            &self,
            _program: &Pubkey,
            _filters: &[Memcmp],
        ) -> anyhow::Result<Vec<(Pubkey, AccountData)>> {
            panic!("explicit selection must not scan all program accounts")
        }

        async fn latest_blockhash(&self) -> anyhow::Result<Hash> {
            unreachable!("selected discovery needs no blockhash")
        }

        async fn simulate(&self, _tx: &VersionedTransaction) -> anyhow::Result<TxOutcome> {
            unreachable!("selected discovery needs no simulation")
        }

        async fn send(&self, _tx: &VersionedTransaction) -> anyhow::Result<TxOutcome> {
            unreachable!("selected discovery sends no transaction")
        }

        async fn recent_priority_fees(&self, _writable: &[Pubkey]) -> anyhow::Result<Vec<u64>> {
            unreachable!("selected discovery needs no fee quote")
        }
    }

    fn market_account(owner: Pubkey, discriminator: bool, version: u16) -> AccountData {
        let mut data = vec![0u8; 8 + std::mem::size_of::<Market>()];
        if discriminator {
            data[..8].copy_from_slice(Market::DISCRIMINATOR);
        }
        data[8..10].copy_from_slice(&version.to_le_bytes());
        AccountData {
            lamports: 1,
            owner,
            executable: false,
            data,
        }
    }

    #[tokio::test]
    async fn selected_discovery_reads_only_allowlisted_market_accounts() {
        let valid = Pubkey::new_unique();
        let old_version = Pubkey::new_unique();
        let wrong_owner = Pubkey::new_unique();
        let wrong_discriminator = Pubkey::new_unique();
        let missing = Pubkey::new_unique();
        let chain = DiscoveryChain {
            accounts: HashMap::from([
                (valid, market_account(shortwin::ID, true, MARKET_VERSION)),
                (old_version, market_account(shortwin::ID, true, 0)),
                (
                    wrong_owner,
                    market_account(Pubkey::new_unique(), true, MARKET_VERSION),
                ),
                (
                    wrong_discriminator,
                    market_account(shortwin::ID, false, MARKET_VERSION),
                ),
            ]),
            reads: RefCell::new(Vec::new()),
        };
        let chosen = discover_selected_markets(
            &chain,
            &[
                missing,
                valid,
                wrong_discriminator,
                old_version,
                wrong_owner,
                valid,
            ],
        )
        .await
        .unwrap();
        let mut expected = vec![valid, old_version];
        expected.sort();
        assert_eq!(chosen, expected);
        let reads = chain.reads.borrow();
        assert_eq!(reads.len(), 1, "one batched account read");
        assert_eq!(reads[0].len(), 5, "duplicate allowlist key read once");
        assert!(reads[0].windows(2).all(|w| w[0] < w[1]));
        // The old layout remains discoverable so the full loader can report its version error.
        assert_ne!(
            decode_zc::<Market>(chain.accounts.get(&old_version).unwrap())
                .unwrap()
                .version,
            MARKET_VERSION
        );
    }
}
