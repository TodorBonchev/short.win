//! Venue adapters (ARCH §2.1 `venues/`, §3.0 "Remaining-accounts convention", §3.12).
//!
//! Venue accounts arrive in `remaining_accounts` as
//!
//! ```text
//! ReadSet(venue) ++ TradeExtras(venue) [trading instructions] ++ Extras(venue, ix) [A41]
//! ```
//!
//! and are parsed by a strict per-venue parser ([`parse`]): every slot is mandatory and
//! key-checked against a compile-time constant, a `Market` cache entry or an on-chain
//! re-derivation; a missing, extra or wrong account reverts (`BadRemainingAccounts`,
//! `VenueAccountMismatch`) and never maps to "no route" (C0-2). A correctly keyed venue-state
//! account whose contents no longer parse makes the venue **not executable** (A63, V1-8):
//! [`VenueState`] then says so, and the route logic (`engine::route`) decides.
//!
//! **Extras (A41; O6).** After the adapter's own list come exactly the accounts of the extras
//! set the caller selects in the instruction data ([`ExtrasSel`]): `none` (nothing follows),
//! or `current` / `previous` / `stable` (M2-SEC-2) of `VenueExtras[venue][ix]`, in which case
//! the `VenueExtras`
//! account itself comes first and its 0–4 entries follow in order ([`parse_extras`]). A fixed
//! entry must match its key; a template entry is re-derived with `create_program_address` from
//! its constant seeds, its placeholders (`mint`, `pool`, `trader`, `bonding_curve`,
//! `coin_creator`) and the caller's bump, which must be the **canonical** one (M2-SEC-3: every
//! higher bump must fail to derive, so a caller cannot substitute a second, non-canonical PDA of
//! the template's program; ARCH says "no search" — this is a search of the bumps above the
//! caller's only, usually 0–2 extra derivations). Every extra must be a non-signer, not
//! owned by short.win, not on the denylist (ARCH §2.4 rule 4), and writable exactly when its
//! flag says so; the resolved metas are appended to the venue CPI. Because `none` and
//! `previous` stay selectable, a wrong or hostile Emergency write never bricks exits; because
//! `stable` keeps a set that worked for `EXTRAS_STABLE_S` (or that Governance published),
//! neither do several quick ones (M2-SEC-2, `set_venue_extras`).
//! *Where the `VenueExtras` account travels is this module's choice* (ARCH names the sets,
//! not the slot): first in the extras segment, only when a set is selected.
//!
//! Venue kinds with an adapter in this build: [`VenueKind::RaydiumV4`] ([`raydium_v4`]) and
//! [`VenueKind::RaydiumCpmm`] ([`raydium_cpmm`]) since M3, sharing [`raydium`] (the delegate
//! sell out of the lending vault, the direct-to-vault buy, eligibility, adoption), and
//! [`VenueKind::Mock`] (TEST-ONLY, feature `test-venue`, [`mock`]). The pump venues arrive
//! with their milestones (M5 pump curve, M6 PumpSwap): each adds its arm to the `match`es
//! below and its own module; until then their markets refuse trades with `NoSupportedVenue`
//! (no market can be on them yet: `create_market` lists `None`, a verified Raydium pool and,
//! in a test build, the mock). [`detect`] is the part of the pump detection a Raydium listing
//! and a list-only market's `propose_pool` need today (D1 precedence).

use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::AccountMeta;
use shortwin_math::quote::{BuyQuote, Pricing, SellQuote};
use shortwin_math::risk::price_wad;

use crate::constants::VENUE_EXTRAS_VERSION;
use crate::engine::trader::TraderAccounts;
use crate::errors::ShortwinError;
use crate::state::{
    Market, VenueExtras, VenueKind, ENTRY_FIXED, ENTRY_NONE, ENTRY_TEMPLATE, EXTRAS_IX_KINDS,
    EXTRAS_MAX_ENTRIES, EXTRAS_MAX_SEEDS, PLACEHOLDER_BONDING_CURVE, PLACEHOLDER_COIN_CREATOR,
    PLACEHOLDER_MINT, PLACEHOLDER_POOL, PLACEHOLDER_TRADER, SEED_CONST, SEED_NONE,
    SEED_PLACEHOLDER,
};

pub mod detect;
#[cfg(feature = "test-venue")]
pub mod mock;
pub mod raydium;
pub mod raydium_cpmm;
pub mod raydium_v4;

// ---- extras selection ------------------------------------------------------------------

/// `ExtrasSel.set`: no trailing accounts.
pub const EXTRAS_SET_NONE: u8 = 0;
/// `ExtrasSel.set`: the `current` set of `VenueExtras[venue][ix]`.
pub const EXTRAS_SET_CURRENT: u8 = 1;
/// `ExtrasSel.set`: the `previous` set.
pub const EXTRAS_SET_PREVIOUS: u8 = 2;
/// `ExtrasSel.set`: the `stable` set (M2-SEC-2: the newest set that stood as `current` for
/// `EXTRAS_STABLE_S`, or Governance's last publication).
pub const EXTRAS_SET_STABLE: u8 = 3;

/// The extras set a trading instruction selects (ARCH §3.0 "Bounded extras", A41): `set`
/// ([`EXTRAS_SET_NONE`], [`EXTRAS_SET_CURRENT`], [`EXTRAS_SET_PREVIOUS`],
/// [`EXTRAS_SET_STABLE`]) and, for each
/// template entry `i`, the canonical bump `bumps[i]` of the account it derives (fixed entries
/// ignore theirs; a non-canonical bump is `BadExtras`, M2-SEC-3).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExtrasSel {
    pub set: u8,
    pub bumps: [u8; 4],
}

impl ExtrasSel {
    /// No trailing accounts.
    pub const NONE: Self = Self {
        set: EXTRAS_SET_NONE,
        bumps: [0; 4],
    };
}

/// The keys a template's placeholders stand for on this market (ARCH §2.3.2). A placeholder
/// the market has no value for (`None`) makes the entry unusable (`BadExtras`).
#[derive(Clone, Copy, Debug)]
pub struct Placeholders {
    pub mint: Pubkey,
    pub pool: Option<Pubkey>,
    pub trader: Pubkey,
    pub bonding_curve: Pubkey,
    pub coin_creator: Option<Pubkey>,
}

impl Placeholders {
    fn key(&self, id: u8) -> Result<Pubkey> {
        let k = match id {
            PLACEHOLDER_MINT => Some(self.mint),
            PLACEHOLDER_POOL => self.pool,
            PLACEHOLDER_TRADER => Some(self.trader),
            PLACEHOLDER_BONDING_CURVE => Some(self.bonding_curve),
            PLACEHOLDER_COIN_CREATOR => self.coin_creator,
            _ => None,
        };
        k.ok_or_else(|| error!(ShortwinError::BadExtras))
    }
}

/// Resolves the extras segment (ARCH §3.0; module docs): `accounts` is everything after the
/// adapter's own list. Returns the metas to append to the venue CPI (writable exactly as the
/// entries say, never signers) in order, together with their infos.
///
/// Errors: `BadRemainingAccounts` if the segment has the wrong length (a `none` selection with
/// trailing accounts, or fewer / more accounts than the set's entries); `BadExtras` for an
/// unknown selection, a `VenueExtras` account that is not this venue's (PDA with its stored
/// bump, owner, discriminator, version, venue byte), an entry that does not match (fixed key,
/// template re-derivation, writable flag), or a placeholder the market has no value for;
/// `DeniedAccount` for a signer, a short.win-owned account or a denylisted key.
pub fn parse_extras<'info>(
    kind: VenueKind,
    ix: usize,
    sel: &ExtrasSel,
    accounts: &'info [AccountInfo<'info>],
    ph: &Placeholders,
    deny: &[Pubkey],
) -> Result<(Vec<AccountMeta>, &'info [AccountInfo<'info>])> {
    if sel.set == EXTRAS_SET_NONE {
        require!(accounts.is_empty(), ShortwinError::BadRemainingAccounts);
        return Ok((Vec::new(), accounts));
    }
    require!(
        matches!(
            sel.set,
            EXTRAS_SET_CURRENT | EXTRAS_SET_PREVIOUS | EXTRAS_SET_STABLE
        ),
        ShortwinError::BadExtras
    );
    require!(ix < EXTRAS_IX_KINDS, ShortwinError::BadExtras);
    let seed = kind.extras_seed().ok_or(ShortwinError::BadExtras)?;
    let (head, entries_infos) = accounts
        .split_first()
        .ok_or(ShortwinError::BadRemainingAccounts)?;
    let loader = AccountLoader::<VenueExtras>::try_from(head)
        .map_err(|_| error!(ShortwinError::BadExtras))?;
    let ve = loader
        .load()
        .map_err(|_| error!(ShortwinError::BadExtras))?;
    let pda = Pubkey::create_program_address(&[VenueExtras::SEED, &[seed], &[ve.bump]], &crate::ID)
        .map_err(|_| error!(ShortwinError::BadExtras))?;
    require_keys_eq!(pda, head.key(), ShortwinError::BadExtras);
    require!(
        ve.version == VENUE_EXTRAS_VERSION && ve.venue == seed,
        ShortwinError::BadExtras
    );
    let ixe = ve.sets.get(ix).ok_or(ShortwinError::BadExtras)?;
    let set = match sel.set {
        EXTRAS_SET_CURRENT => &ixe.current,
        EXTRAS_SET_PREVIOUS => &ixe.previous,
        _ => &ixe.stable,
    };
    let n = usize::from(set.count);
    require!(n <= EXTRAS_MAX_ENTRIES, ShortwinError::BadExtras);
    require!(
        entries_infos.len() == n,
        ShortwinError::BadRemainingAccounts
    );
    let mut metas = Vec::with_capacity(n);
    for (i, (entry, info)) in set.entries.iter().zip(entries_infos.iter()).enumerate() {
        let expected = match entry.kind {
            ENTRY_FIXED => entry.key,
            ENTRY_TEMPLATE => {
                let bump = *sel.bumps.get(i).ok_or(ShortwinError::BadExtras)?;
                derive_template(entry, ph, bump)?
            }
            _ => return err!(ShortwinError::BadExtras),
        };
        require_keys_eq!(info.key(), expected, ShortwinError::BadExtras);
        require!(!info.is_signer, ShortwinError::DeniedAccount);
        require_keys_neq!(*info.owner, crate::ID, ShortwinError::DeniedAccount);
        require!(!deny.contains(info.key), ShortwinError::DeniedAccount);
        let writable = entry.writable == 1;
        require!(info.is_writable == writable, ShortwinError::BadExtras);
        metas.push(AccountMeta {
            pubkey: info.key(),
            is_signer: false,
            is_writable: writable,
        });
    }
    Ok((metas, entries_infos))
}

/// `create_program_address(seeds ++ [bump], program)`, `None` if that is on the curve.
fn derive_with_bump(seeds: &[&[u8]], bump: u8, program: &Pubkey) -> Option<Pubkey> {
    let bump_seed = [bump];
    let mut all: [&[u8]; EXTRAS_MAX_SEEDS + 1] = [&[]; EXTRAS_MAX_SEEDS + 1];
    let n = seeds.len();
    all.get_mut(..n)?.copy_from_slice(seeds);
    *all.get_mut(n)? = &bump_seed;
    Pubkey::create_program_address(all.get(..=n)?, program).ok()
}

/// `create_program_address(seeds ++ [bump], program)` of a template entry, for the
/// **canonical** bump only (M2-SEC-3): the caller's `bump` must derive an off-curve address and
/// every higher bump must fail to (what `find_program_address` returns). Without this check any
/// off-curve bump was accepted, so a caller — the keeper on `liquidate` / `expire`, whose venue
/// CPI spends the borrower's SOL — could pass a second, non-canonical PDA of the template's
/// program instead of the account the Emergency multisig meant; a venue that consumes such a
/// trailing account without re-deriving it canonically would take it. The bumps above a
/// canonical one are on the curve with probability ½ each, so this costs 0–2 extra derivations
/// in practice.
fn derive_template(
    entry: &crate::state::ExtraEntry,
    ph: &Placeholders,
    bump: u8,
) -> Result<Pubkey> {
    let n = usize::from(entry.n_seeds);
    require!(
        (1..=EXTRAS_MAX_SEEDS).contains(&n),
        ShortwinError::BadExtras
    );
    // Placeholder keys must outlive the seed slices: resolve them first.
    let mut keys = [Pubkey::default(); EXTRAS_MAX_SEEDS];
    for (k, part) in keys.iter_mut().zip(entry.seeds.iter()).take(n) {
        if part.kind == SEED_PLACEHOLDER {
            *k = ph.key(part.len)?;
        }
    }
    let mut seeds: [&[u8]; EXTRAS_MAX_SEEDS] = [&[]; EXTRAS_MAX_SEEDS];
    for ((slot, part), key) in seeds
        .iter_mut()
        .zip(entry.seeds.iter())
        .zip(keys.iter())
        .take(n)
    {
        *slot = match part.kind {
            SEED_CONST => {
                let len = usize::from(part.len);
                part.bytes.get(..len).ok_or(ShortwinError::BadExtras)?
            }
            SEED_PLACEHOLDER => key.as_ref(),
            _ => return err!(ShortwinError::BadExtras),
        };
    }
    let seeds = seeds.get(..n).ok_or(ShortwinError::BadExtras)?;
    let key = derive_with_bump(seeds, bump, &entry.key).ok_or(ShortwinError::BadExtras)?;
    let mut higher = bump;
    while let Some(b) = higher.checked_add(1) {
        require!(
            derive_with_bump(seeds, b, &entry.key).is_none(),
            ShortwinError::BadExtras
        );
        higher = b;
    }
    Ok(key)
}

/// Checks one entry of a `set_venue_extras` argument (ARCH §3.1): a fixed key or a template
/// of 1–4 seeds, never short.win itself nor the default key; constant seeds ≤ 32 bytes (zero
/// past their length), placeholders among the five known ones, unused seed slots empty.
pub fn check_entry(entry: &crate::state::ExtraEntry) -> Result<()> {
    require_keys_neq!(entry.key, crate::ID, ShortwinError::BadExtras);
    require_keys_neq!(entry.key, Pubkey::default(), ShortwinError::BadExtras);
    require!(entry.writable <= 1, ShortwinError::BadExtras);
    let n = usize::from(entry.n_seeds);
    match entry.kind {
        ENTRY_FIXED => require!(n == 0, ShortwinError::BadExtras),
        ENTRY_TEMPLATE => require!(
            (1..=EXTRAS_MAX_SEEDS).contains(&n),
            ShortwinError::BadExtras
        ),
        _ => return err!(ShortwinError::BadExtras),
    }
    for (i, part) in entry.seeds.iter().enumerate() {
        if i >= n {
            require!(
                part.kind == SEED_NONE && part.len == 0 && part.bytes == [0; 32],
                ShortwinError::BadExtras
            );
            continue;
        }
        match part.kind {
            SEED_CONST => {
                let len = usize::from(part.len);
                require!(len <= 32, ShortwinError::BadExtras);
                let tail = part.bytes.get(len..).ok_or(ShortwinError::BadExtras)?;
                require!(tail.iter().all(|b| *b == 0), ShortwinError::BadExtras);
            }
            SEED_PLACEHOLDER => {
                require!(
                    part.len <= PLACEHOLDER_COIN_CREATOR && part.bytes == [0; 32],
                    ShortwinError::BadExtras
                );
            }
            _ => return err!(ShortwinError::BadExtras),
        }
    }
    Ok(())
}

/// `true` for an empty entry slot (`ENTRY_NONE`, everything zero).
pub fn is_empty_entry(entry: &crate::state::ExtraEntry) -> bool {
    entry.kind == ENTRY_NONE
        && entry.writable == 0
        && entry.n_seeds == 0
        && entry.key == Pubkey::default()
}

// ---- venue state -------------------------------------------------------------------------

/// The live state of a market's venue (ARCH §4.1 `VenueState { X, Y, R, P, L, fee }`), read
/// from its ReadSet in the prologue (step 6).
#[derive(Clone, Copy, Debug)]
pub struct VenueState {
    /// The market's venue.
    pub kind: VenueKind,
    /// `venue_executable` (ARCH §5.3): the venue predicate holds and every venue-state
    /// account parses (A63). Only this drives `B̂_live`, closes, observation recording and the
    /// "no route ⇒ spot leg true" rule; the route-broken flag never enters it (V2-1).
    pub executable: bool,
    /// Pricing reserves `X` (token) and `Y` (SOL); 0 when the venue is not executable.
    pub x: u64,
    pub y: u64,
    /// Real SOL `R` (tiers, pool eligibility); `Y` on the mock and Raydium.
    pub r: u64,
    /// The venue's fees resolved on the live state (execution quotes); `None` when not
    /// executable.
    pub pricing: Option<Pricing>,
    /// One-way live fee φ in ppm (ARCH §4.2), 0 when unknown.
    pub fee_ppm: u32,
    /// Raydium: the pool's locked constant-product depth `D = ⌊λ_bps·isqrt(X·Y)/10⁴⌋` on the
    /// pricing reserves (ARCH §2.6; `pool_select::locked_depth`); 0 on other venues and when
    /// not executable.
    pub locked_depth: u128,
}

impl VenueState {
    /// A venue that cannot execute (no accounts, a reader failure, a halt): no price, no
    /// quote.
    pub const fn not_executable(kind: VenueKind) -> Self {
        Self {
            kind,
            executable: false,
            x: 0,
            y: 0,
            r: 0,
            pricing: None,
            fee_ppm: 0,
            locked_depth: 0,
        }
    }

    /// Spot `P = ⌊Y·WAD/X⌋` of an executable venue with both reserves non-zero.
    pub fn price(&self) -> Option<u128> {
        if !self.executable || self.x == 0 || self.y == 0 {
            return None;
        }
        price_wad(u128::from(self.x), u128::from(self.y))
            .ok()
            .filter(|p| *p > 0)
    }

    /// `L = √(X·Y)` of an executable venue.
    pub fn liquidity(&self) -> Option<u128> {
        if !self.executable {
            return None;
        }
        shortwin_math::quote::liquidity(u128::from(self.x), u128::from(self.y))
            .ok()
            .filter(|l| *l > 0)
    }

    /// Execution quote `B̂_live(q)` at the live fee (ARCH §4.3 "two fee views"); `None` (`+∞`)
    /// when undefined or the venue is not executable.
    pub fn buy(&self, q: u64) -> Option<BuyQuote> {
        self.pricing?.buy(q, u128::from(self.x), u128::from(self.y))
    }

    /// Risk quote at `min(φ_live, cap_bps)` (V2-1).
    pub fn buy_risk(&self, q: u64, cap_bps: u64) -> Option<BuyQuote> {
        self.pricing?
            .capped(cap_bps)
            .buy(q, u128::from(self.x), u128::from(self.y))
    }

    /// Exact-in sale of `q` tokens at the live fee (`S_exp`, ARCH §3.4 step 7).
    pub fn sell(&self, q: u64) -> Option<SellQuote> {
        self.pricing?
            .sell(q, u128::from(self.x), u128::from(self.y), self.r)
    }

    /// A quote at the live fee on a hypothetical state `(x, y)` (a ring entry's cached
    /// reprojection: the D4 margin's EMA leg, ARCH §3.4 step 9).
    pub fn buy_at(&self, q: u64, x: u128, y: u128) -> Option<BuyQuote> {
        if x == 0 {
            return None;
        }
        self.pricing?.buy(q, x, y)
    }
}

// ---- per-venue dispatch ------------------------------------------------------------------

/// Length of the venue's ReadSet (ARCH §3.12). `NoSupportedVenue` for a venue without an
/// adapter in this build.
pub fn read_len(kind: VenueKind) -> Result<usize> {
    match kind {
        VenueKind::RaydiumV4 => Ok(raydium_v4::READ_LEN),
        VenueKind::RaydiumCpmm => Ok(raydium_cpmm::READ_LEN),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => Ok(mock::READ_LEN),
        _ => err!(ShortwinError::NoSupportedVenue),
    }
}

/// Length of the venue's TradeExtras (ARCH §3.12).
pub fn trade_len(kind: VenueKind) -> Result<usize> {
    match kind {
        VenueKind::RaydiumV4 => Ok(raydium_v4::TRADE_LEN),
        VenueKind::RaydiumCpmm => Ok(raydium_cpmm::TRADE_LEN),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => Ok(mock::TRADE_LEN),
        _ => err!(ShortwinError::NoSupportedVenue),
    }
}

/// Key-checks the ReadSet against the market's cache (`VenueAccountMismatch`).
pub fn check_read(market: &Market, kind: VenueKind, read: &[AccountInfo]) -> Result<()> {
    match kind {
        VenueKind::RaydiumV4 => raydium_v4::check_read(market, read),
        VenueKind::RaydiumCpmm => raydium_cpmm::check_read(market, read),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => mock::check_read(market, read),
        _ => {
            let _ = (market, read);
            err!(ShortwinError::NoSupportedVenue)
        }
    }
}

/// Key-checks the TradeExtras (`VenueAccountMismatch`).
pub fn check_trade(market: &Market, kind: VenueKind, trade: &[AccountInfo]) -> Result<()> {
    match kind {
        VenueKind::RaydiumV4 => raydium_v4::check_trade(market, trade),
        VenueKind::RaydiumCpmm => raydium_cpmm::check_trade(market, trade),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => mock::check_trade(market, trade),
        _ => {
            let _ = (market, trade);
            err!(ShortwinError::NoSupportedVenue)
        }
    }
}

/// Reads the venue state from `market`'s key-checked ReadSet at clock time `now` (the
/// cluster's `unix_timestamp`: the venues' own time gates — AMM v4 `pool_open_time` while
/// `status == 7`, CPMM `open_time` — compare against it). Never fails: a reader failure is
/// "not executable" (A63).
pub fn read_state(market: &Market, kind: VenueKind, read: &[AccountInfo], now: i64) -> VenueState {
    match kind {
        VenueKind::RaydiumV4 => raydium_v4::read_state(market, read, now),
        VenueKind::RaydiumCpmm => raydium_cpmm::read_state(market, read, now),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => mock::read_state(read),
        _ => {
            let _ = (market, read, now);
            VenueState::not_executable(kind)
        }
    }
}

/// The fee view of the **risk** quotes on hypothetical states — the EMA leg on the ring's
/// cached `(x, y)`, the settlement-box corners, the refutation, FV (ARCH §4.3 "two fee
/// views", §4.5) — which `sol_settle` evaluates whether or not the venue can execute:
/// `min(φ_live, cap_bps)`, the position's locked hard cap (V2-1, A43).
///
/// - While the venue executes: its live pricing, capped.
/// - A halted venue whose fee-bearing accounts still parse (a disabled pool) keeps the fee it
///   carries: the venue's own adapter reads it (the mock's `fee_view`).
/// - A venue whose fee-bearing accounts no longer parse (a layout change, A63) has no readable
///   `φ_live`, so the cap itself is used (`min(unknown, cap) = cap`, FV's ceiling): an S3 on
///   such a venue still has a fair value (ARCH §5.3 "FV is priced at the last pre-halt
///   `P_ema`") instead of an uncapped take.
///
/// `None` for a venue without an adapter in this build (M5–M6 add their arms).
pub fn risk_pricing(
    market: &Market,
    state: &VenueState,
    read: &[AccountInfo],
    cap_bps: u64,
) -> Option<Pricing> {
    if let Some(p) = state.pricing {
        return Some(p.capped(cap_bps));
    }
    match state.kind {
        VenueKind::RaydiumV4 => Some(raydium_v4::fee_view(read, cap_bps)),
        VenueKind::RaydiumCpmm => Some(raydium_cpmm::fee_view(market, read, cap_bps)),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => Some(mock::fee_view(read, cap_bps)),
        _ => {
            let _ = (market, read);
            None
        }
    }
}

/// The market's pool for the `pool` placeholder of extras templates.
pub fn pool_key(market: &Market, kind: VenueKind) -> Option<Pubkey> {
    match kind {
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => Some(mock::pool(market)),
        VenueKind::PumpSwap => Some(market.canonical_pool),
        VenueKind::RaydiumV4 | VenueKind::RaydiumCpmm => market
            .pool_cache
            .get(crate::state::raydium_cache::RAY_POOL)
            .copied(),
        _ => None,
    }
}

/// The remaining accounts of a trading or cranking instruction, split and checked.
pub struct Remaining<'info> {
    /// The ReadSet (key-checked).
    pub read: &'info [AccountInfo<'info>],
    /// The TradeExtras (key-checked; empty when the instruction carries none).
    pub trade: &'info [AccountInfo<'info>],
    /// The resolved extras metas and their infos.
    pub extra_metas: Vec<AccountMeta>,
    pub extra_infos: &'info [AccountInfo<'info>],
}

/// What an instruction carries after its ReadSet.
#[derive(Clone, Copy, Debug)]
pub enum Carry {
    /// `ReadSet` only (`crank_price`, `sol_settle`).
    ReadSet,
    /// `ReadSet ++ TradeExtras ++ Extras(ix)` (trading instructions), with the caller's
    /// selection.
    Trade { ix: usize, sel: ExtrasSel },
}

/// Splits and checks `remaining` (module docs): the ReadSet and, for a trading instruction,
/// the TradeExtras and the extras segment. `deny` is the instruction's denylist (ARCH §2.4
/// rule 4) for the extras.
pub fn parse<'info>(
    market: &Market,
    kind: VenueKind,
    remaining: &'info [AccountInfo<'info>],
    carry: Carry,
    deny: &[Pubkey],
) -> Result<Remaining<'info>> {
    let rl = read_len(kind)?;
    require!(remaining.len() >= rl, ShortwinError::BadRemainingAccounts);
    let (read, rest) = remaining.split_at(rl);
    check_read(market, kind, read)?;
    match carry {
        Carry::ReadSet => {
            require!(rest.is_empty(), ShortwinError::BadRemainingAccounts);
            Ok(Remaining {
                read,
                trade: &[],
                extra_metas: Vec::new(),
                extra_infos: &[],
            })
        }
        Carry::Trade { ix, sel } => {
            let tl = trade_len(kind)?;
            require!(rest.len() >= tl, ShortwinError::BadRemainingAccounts);
            let (trade, extras) = rest.split_at(tl);
            check_trade(market, kind, trade)?;
            let ph = Placeholders {
                mint: market.mint,
                pool: pool_key(market, kind),
                trader: market.trader,
                bonding_curve: market.bonding_curve,
                coin_creator: None,
            };
            let (extra_metas, extra_infos) = parse_extras(kind, ix, &sel, extras, &ph, deny)?;
            Ok(Remaining {
                read,
                trade,
                extra_metas,
                extra_infos,
            })
        }
    }
}

// ---- legs --------------------------------------------------------------------------------

/// Everything a venue leg needs (ARCH §2.4.1): the market (it signs the pool vault's
/// `transfer_checked` / `approve_checked`), the pool vault, the base mint and its token
/// program, the trader accounts and the parsed remaining accounts.
pub struct Leg<'info> {
    pub kind: VenueKind,
    pub market: AccountInfo<'info>,
    pub market_bump: u8,
    pub pool_vault: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub decimals: u8,
    pub token_program: AccountInfo<'info>,
    /// Raydium: WSOL is the AMM v4 coin / the CPMM token 0 (`Market.flags` bit 3; the CPMM
    /// swap names its input and output vaults by side). Unused by the mock.
    pub sol_first: bool,
    pub trader: TraderAccounts<'info>,
    pub read: &'info [AccountInfo<'info>],
    pub trade: &'info [AccountInfo<'info>],
    pub extra_metas: Vec<AccountMeta>,
    pub extra_infos: &'info [AccountInfo<'info>],
}

/// What a buy leg measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuyOut {
    /// `B`: lamports the position spent (incl. venue-side rent on pump venues).
    pub spent: u64,
    /// Tokens the pool vault received (`≥ q`; the surplus is a donation).
    pub received: u64,
}

/// The trader-side accounts of a trading instruction on `kind`: the named `trader` and
/// `market_vault`, the WSOL mint, SPL Token and `trader_wsol` from the venue's TradeExtras.
pub fn trader_accounts<'info>(
    kind: VenueKind,
    market_key: Pubkey,
    market: &Market,
    trader: AccountInfo<'info>,
    market_vault: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
    trade: &'info [AccountInfo<'info>],
) -> Result<TraderAccounts<'info>> {
    let at = |i: usize| -> Result<AccountInfo<'info>> {
        trade
            .get(i)
            .cloned()
            .ok_or_else(|| error!(ShortwinError::BadRemainingAccounts))
    };
    let (wsol, wsol_mint, wsol_program) = match kind {
        VenueKind::RaydiumV4 => (
            raydium_v4::X_TRADER_WSOL,
            raydium_v4::X_WSOL_MINT,
            raydium_v4::X_TOKEN_PROGRAM,
        ),
        VenueKind::RaydiumCpmm => (
            raydium_cpmm::X_TRADER_WSOL,
            raydium_cpmm::X_WSOL_MINT,
            raydium_cpmm::X_WSOL_PROGRAM,
        ),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => (mock::T_TRADER_WSOL, mock::T_WSOL_MINT, mock::T_WSOL_PROGRAM),
        _ => return err!(ShortwinError::NoSupportedVenue),
    };
    Ok(TraderAccounts {
        market: market_key,
        trader,
        trader_bump: market.trader_bump,
        trader_wsol: at(wsol)?,
        trader_wsol_bump: market.trader_wsol_bump,
        market_vault,
        system_program,
        wsol_mint: at(wsol_mint)?,
        wsol_token_program: at(wsol_program)?,
    })
}

/// What the position refunds the borrower after a sell leg: the ephemeral WSOL account's rent
/// on the WSOL venues (PumpSwap, Raydium, the mock), 0 on the curve.
pub fn sell_refund(kind: VenueKind) -> Result<u64> {
    match kind {
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => TraderAccounts::wsol_rent(),
        VenueKind::PumpSwap | VenueKind::RaydiumV4 | VenueKind::RaydiumCpmm => {
            TraderAccounts::wsol_rent()
        }
        _ => Ok(0),
    }
}

/// The venue-side rent tolerance of a venue (`ert`, ARCH §2.4 rule 3): `exec_rent_tol` on the
/// pump venues, 0 on Raydium and the mock.
pub fn effective_ert(kind: VenueKind, exec_rent_tol: u64) -> u64 {
    match kind {
        VenueKind::PumpCurve | VenueKind::PumpSwap => exec_rent_tol,
        _ => 0,
    }
}

/// The sell leg of an open (exact in `q`, venue slippage bound `min_out`). Returns `S`, the
/// lamports of proceeds credited to the position. On the WSOL venues the position also holds
/// the ephemeral account's rent afterwards (`TraderAccounts::wsol_rent`), which the borrower
/// paid: the caller refunds it by a direct lamport move **after its last CPI that names the
/// position or the borrower** (a direct move is visible to the runtime's per-CPI balance check
/// only when both accounts are in that CPI).
pub fn sell<'info>(
    leg: &Leg<'info>,
    borrower: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    q: u64,
    min_out: u64,
) -> Result<u64> {
    match leg.kind {
        VenueKind::RaydiumV4 => raydium_v4::sell(leg, borrower, position, q, min_out),
        VenueKind::RaydiumCpmm => raydium_cpmm::sell(leg, borrower, position, q, min_out),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => mock::sell(leg, borrower, position, q, min_out),
        _ => {
            let _ = (borrower, position, q, min_out);
            err!(ShortwinError::NoSupportedVenue)
        }
    }
}

/// The buy leg of an exit (exact out `q`, venue slippage bound `max_in`), paid from the
/// position.
pub fn buy<'info>(
    leg: &Leg<'info>,
    position: &AccountInfo<'info>,
    q: u64,
    max_in: u64,
) -> Result<BuyOut> {
    match leg.kind {
        VenueKind::RaydiumV4 => raydium_v4::buy(leg, position, q, max_in),
        VenueKind::RaydiumCpmm => raydium_cpmm::buy(leg, position, q, max_in),
        #[cfg(feature = "test-venue")]
        VenueKind::Mock => mock::buy(leg, position, q, max_in),
        _ => {
            let _ = (position, q, max_in);
            err!(ShortwinError::NoSupportedVenue)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::state::{ExtraEntry, SeedPart};

    fn seed(kind: u8, len: u8, bytes: &[u8]) -> SeedPart {
        let mut b = [0u8; 32];
        b[..bytes.len()].copy_from_slice(bytes);
        SeedPart {
            kind,
            len,
            bytes: b,
        }
    }

    /// `["extra", mint]` under `program`.
    fn per_mint(program: Pubkey) -> ExtraEntry {
        let none = seed(SEED_NONE, 0, &[]);
        ExtraEntry {
            kind: ENTRY_TEMPLATE,
            writable: 0,
            n_seeds: 2,
            _pad: [0; 5],
            key: program,
            seeds: [
                seed(SEED_CONST, 5, b"extra"),
                seed(SEED_PLACEHOLDER, PLACEHOLDER_MINT, &[]),
                none,
                none,
            ],
        }
    }

    /// M2-SEC-3: a template derives with the canonical bump (what `find_program_address`
    /// returns) and refuses every other one — the lower off-curve bumps a caller could pick,
    /// and the higher ones, which are on the curve.
    #[test]
    fn templates_take_the_canonical_bump_only() {
        let mint = Pubkey::new_unique();
        let ph = Placeholders {
            mint,
            pool: None,
            trader: Pubkey::new_unique(),
            bonding_curve: Pubkey::new_unique(),
            coin_creator: None,
        };
        let mut non_canonical = 0;
        for _ in 0..32 {
            let program = Pubkey::new_unique();
            let entry = per_mint(program);
            let (canon, bump) = Pubkey::find_program_address(&[b"extra", mint.as_ref()], &program);
            assert_eq!(derive_template(&entry, &ph, bump).unwrap(), canon);
            for b in 0..=u8::MAX {
                if b == bump {
                    continue;
                }
                let off_curve =
                    Pubkey::create_program_address(&[b"extra", mint.as_ref(), &[b]], &program)
                        .is_ok();
                non_canonical += usize::from(off_curve);
                assert!(
                    derive_template(&entry, &ph, b).is_err(),
                    "bump {b} (canonical {bump}, off curve {off_curve})"
                );
            }
        }
        assert!(
            non_canonical > 0,
            "the sample must contain non-canonical PDAs"
        );
    }
}
