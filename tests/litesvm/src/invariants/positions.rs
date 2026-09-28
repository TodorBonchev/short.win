//! The position engine's invariants (ARCH §8.3; M2 exit criteria: I3, I5–I7, I9–I14, I16),
//! checked by the same two hooks as M1's ([`super::check_all`], [`super::check_ix`]).
//!
//! | # | Where | What is checked |
//! |---|---|---|
//! | I3 | `check_ix` | a position's lamports fall (or it closes) only in an instruction that emitted an exit event for it (`Close`, `Liquidate`, `Expire`, `SolSettle`); a position an exit event names is gone afterwards; a surviving position's tracked `v` never falls, rises only with `m` by the same amount (`add_margin`) and never by more than its lamports; a new position appears only with this instruction's `Open` event, whose figures it stores |
//! | A26 | `check_ix` | the snapshot rules: every field of a surviving position except `v`, `m` and the settlement arming state (`s_armed`, `s_count`, `s_since`, `s_last`) is unchanged — the locked terms, `deadline`, `r_min`, `j_open`, `q`, `s`, `n_open`, `fee_open`, `entry_price`, tier, venue, nonce, bump, status |
//! | I5 | `check_ix` | after every `Open`: `ΣB_Q·10⁴ ≤ util_cap_bps·(T + ΣB_Q)` |
//! | I6 | `check_all` | `ΣB_Q` = Σ open positions' `q` (and by tier); `open_positions`, `total_notional_open`, `rent_aggr` (`Σ q·coef_j·(10⁴ − p_pos)`) match the positions; `max_release ≥` every position's `release_at`; every `WalletExposure` of the market sits at its PDA with `q_open` = Σ `q` of its wallet's positions and `q_open > 0` (closed at 0), and every borrower with a position has one |
//! | I7 | `check_ix` + `check_all` | after every instruction that ran a leg (`Open`, `Close`, `Liquidate`, `Expire`): `trader` 0 lamports, System-owned, no data; `trader_wsol` holds nothing; `pool_vault` has no delegate — the last also after **every** instruction (`check_all`: the delegate exists only inside a Raydium sell leg); a pump market's `trader_base_ata` holds 0 tokens, is owned by `trader`, with no delegate and no close authority |
//! | I9 | `check_ix` | every waterfall conserves `V` (ECON I-E4), read off its event against the position's `V` just before: `Close` (`B + R + borrower`, and `rent_protocol = ⌊R·rent_protocol_bps_pos/10⁴⌋`), `Liquidate` (`B + r + π + top-up + borrower`), `Expire` (`B + r + e + K_e + borrower`), `SolSettle` (`K_s + lenders + borrower`) |
//! | I10 | `check_ix` | an exit (`Close`, `Liquidate`, `Expire`, `SolSettle`) names neither `oracle_cache` nor `venue_guard` among its accounts |
//! | I11 | `check_ix` (event part) | a liquidation's spend `≤ ⌊B̂_ref·(10⁴ + δ_liq)/10⁴⌋` with the position's locked δ, an expiry's `≤ ⌊B̂_ref·(10⁴ + δ_exp)/10⁴⌋`; `B ≤ max_in + ert` and `max_in ≤` that fixed bound; penalty / expiry fee `≤ ⌈rate·min(B̂_live,risk, B̂_ref)/10⁴⌉`; an expiry's `π_ref = ⌈fee·B̂_ref/10⁴⌉`, its `K_e = min(fixed + ⌊bps·B̂_ref/10⁴⌋, cap, V − B − r − e)`, and its rent (`R_due` on the post-accrual basis) and fee paid in full whenever `V − R − π_ref − ert ≥ ⌊B̂_ref·(10⁴ + ε)/10⁴⌋`. The liveness clause (I-E11) is a scenario test (`tests/liquidate_it.rs`, `tests/attacks_exit_it.rs`) |
//! | I12 | `check_ix` (event part and the state around it) | S1 take `= V − K_s`; an S2/S3 take inside `[x_lo, x_hi]` (±2) with the corners within `fv_dev_bps`; `K_s` = the locked terms' reward; a liquidation that leaves the borrower equity pays the keeper `≥ K_s`; S1/S2 only on a live clock (armed, last evaluation within `settle_arm_gap_s`) that has run `settle_persist_n` EMA updates (the position's `s_count` before, the market's `ema_count` after); S3 only on a continuous no-route chain observed now and at least `s3_wait_s` (and the graduation / migration timers) long, with the position's locked terms; `p'` per corner: the penalty on both corners of a position not expired, and on an expired one the expiry fee at the dearer corner `(P_hi, L_lo)` only if the cheaper corner has it too |
//! | I13 | `check_all` + `check_ix` | every ring entry's `(x, y) = reprice(l, p)`; the newest ring entry is the EMA state (`ts = ema_ts`, `p = P_ema`, `l = L_ema`); no decision state holds the current second (`lb_sec <` now, `b_sec ≤` now, `ema_ts ≤` now, with now = `max(clock, last_accrual_ts)`: the one-second lag); a venue charge ∈ `[B̂ − 5, B̂]` for `Close`, `Liquidate`, `Expire`; on the mock, an `Open`'s proceeds ∈ `[S_exp, S_exp + 5]` and a `Close`'s `b_live` = `B̂_live(q)`, both re-quoted with `quote::cpmm` on the pool just before the instruction; one EMA integration moves `P_ema` and `L_ema` within their single-step bounds |
//! | I14 | structural tests (`tests/engine_it/structure.rs`; `tests/add_margin_it.rs`; `tests/liquidate_it.rs` `remaining_accounts_are_strict` / `named_accounts_are_checked`; `tests/settle_it.rs` account checks) | every ReadSet / TradeExtras slot is mandatory and key-checked on `open_short`, `close_short`, `crank_price`, `liquidate`, `expire` and `sol_settle` (omitted, replaced, swapped, appended); the named accounts of every M2 instruction |
//! | I16 | `check_ix` | no observation is recorded while the (mock) venue is not executable; the route chains are sound (A40, V2-1, R3A-4): a hit of the S3 chain only when `s3_no_route` held on the state just before the instruction, a clear only when it did not; a hit of the `pool_fail` chain only when `pool_fail` held (not executable or fee above the **global** hard cap — never because of the route-broken flag), a clear only when it did not; the risk-quote part (every CR leg, box corner, `B̂_ref` and FV at `min(φ_live, venue_fee_hard_cap_bps_pos)`) is checked by scenario: `tests/route_it.rs` `fee_hike` (FV exactly at the locked cap under a 6 % live fee), `tests/expire_it.rs` `a_fee_far_above_the_hard_cap_holds_expiries_for_s3`, `tests/liquidate_it.rs` `liquidations_ignore_…_fee_cap_…` (the oracle prices at the capped fee) |
//!
//! Every check reads the state the program sees, before and after the instruction.

use std::cell::Cell;

use anchor_lang::AccountDeserialize;
use anchor_lang::prelude::Pubkey;
use litesvm::LiteSVM;
use shortwin::events::{Close, Expire, Liquidate, Open, SolSettle};
use shortwin::state::{
    GlobalConfig, MARKET_FLAG_ROUTE_BROKEN, Market, ShortPosition, VenueKind, WalletExposure,
};
use shortwin_math::observe::RingEntry as MathRingEntry;
use shortwin_math::quote::{cpmm, reprice};
use shortwin_math::rent::{self, Aggr};
use shortwin_math::settle;
use solana_clock::Clock;

use super::{Violation, ensure, fail, positions_of};
use crate::{pda, read};

thread_local! {
    static INJECTED_BORROWER_SIDE: Cell<bool> = const { Cell::new(false) };
}

/// Relaxes I6's position accounting for this test thread: only M1's lender lab
/// (`tests/lender_it`), which injects the borrower side (`ΣB_Q`, `A_j`, `open_positions`,
/// `max_release`) straight into a list-only market's stored state with `shortwin_math::pool`
/// and has no `ShortPosition` accounts to match. Every other I6 part and every other
/// invariant still run.
pub fn allow_injected_borrower_side(on: bool) {
    INJECTED_BORROWER_SIDE.with(|c| c.set(on));
}

fn injected_borrower_side() -> bool {
    INJECTED_BORROWER_SIDE.with(Cell::get)
}

/// One market's position-side state before an instruction.
#[derive(Clone)]
pub struct MarketPositions {
    pub market: Pubkey,
    pub state: Market,
    pub positions: Vec<(Pubkey, ShortPosition, u64)>,
    /// `Some(executable)` for a mock market (I16), `None` otherwise.
    pub mock_executable: Option<bool>,
    /// The mock pool as short.win's reader sees it (I13 re-quotes, I16 route predicates);
    /// `None` for other venues.
    pub mock: Option<MockView>,
}

/// A mock pool as short.win's adapter sees it (INTERFACE.md "Reader contract"): executable,
/// pricing reserves `(X, Y)` and the fee on the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MockView {
    /// The venue predicate (the pool parses, is not disabled, both reserves positive).
    pub executable: bool,
    /// Pricing reserves `(X, Y)` when executable.
    pub reserves: Option<(u64, u64)>,
    /// `fee_ppm` (0 when the pool does not parse).
    pub fee_ppm: u32,
}

impl MockView {
    /// `B̂_live(q)`: short.win's `quote::cpmm` buy of `q` tokens on this state.
    pub fn buy(&self, q: u64) -> Option<u64> {
        let (x, y) = self.reserves?;
        cpmm::buy_exact_out(
            q,
            u128::from(y),
            u128::from(x),
            &crate::mock::cpmm_fees(self.fee_ppm),
        )
        .map(|b| b.cost)
    }

    /// `S_exp(q)`: short.win's `quote::cpmm` sale of `q` tokens on this state.
    pub fn sell(&self, q: u64) -> Option<u64> {
        let (x, y) = self.reserves?;
        cpmm::sell_exact_in(
            q,
            u128::from(x),
            u128::from(y),
            &crate::mock::cpmm_fees(self.fee_ppm),
        )
        .map(|s| s.proceeds)
    }
}

/// The position-side state of the tracked markets before an instruction.
#[derive(Clone, Default)]
pub struct PositionsSnapshot {
    pub markets: Vec<MarketPositions>,
}

/// Snapshots `markets`.
pub fn snapshot(svm: &LiteSVM, markets: &[Pubkey]) -> PositionsSnapshot {
    PositionsSnapshot {
        markets: markets
            .iter()
            .filter_map(|k| {
                let m = read::zc::<Market>(svm, k)?;
                Some(MarketPositions {
                    market: *k,
                    state: m,
                    positions: positions_of(svm, k),
                    mock_executable: mock_executable(svm, &m),
                    mock: mock_view(svm, &m),
                })
            })
            .collect(),
    }
}

/// Whether a mock market's venue is executable now (the adapter's venue predicate, INTERFACE.md
/// "Reader contract"); `None` for other venues.
pub fn mock_executable(svm: &LiteSVM, m: &Market) -> Option<bool> {
    mock_view(svm, m).map(|v| v.executable)
}

/// A mock market's pool as short.win's reader sees it now ([`MockView`]); `None` for other
/// venues.
pub fn mock_view(svm: &LiteSVM, m: &Market) -> Option<MockView> {
    if m.venue_kind() != Some(VenueKind::Mock) {
        return None;
    }
    use shortwin_venue_state::mock_cpamm;
    let dead = MockView {
        executable: false,
        reserves: None,
        fee_ppm: 0,
    };
    let pool = m.pool_cache[shortwin::state::raydium_cache::RAY_POOL];
    let Some(acc) = svm.get_account(&pool) else {
        return Some(dead);
    };
    let Ok(p) = mock_cpamm::read(&acc.owner.to_bytes(), &acc.data) else {
        return Some(dead);
    };
    let base = read::token_amount(svm, &Pubkey::new_from_array(p.base_vault)).unwrap_or(0);
    let quote = read::token_amount(svm, &Pubkey::new_from_array(p.quote_vault)).unwrap_or(0);
    let reserves = mock_cpamm::reserves(&p, base, quote);
    Some(MockView {
        executable: reserves.is_some(),
        reserves,
        fee_ppm: p.fee_ppm,
    })
}

/// The per-market position invariants (I6, I7's `pool_vault` delegate and `trader_base_ata`,
/// I13's ring and time parts).
pub fn check_market(svm: &LiteSVM, key: &Pubkey, m: &Market) -> Result<(), Violation> {
    ring_reprojection(key, m)?;
    ema_is_newest_entry(key, m)?;
    one_second_lag(svm, key, m)?;
    no_delegate_outside_legs(svm, key, m)?;
    let positions = positions_of(svm, key);
    exposures_match(svm, key, &positions)?;
    if injected_borrower_side() && positions.is_empty() {
        return Ok(());
    }
    let mut sum_q: u128 = 0;
    let mut by_tier = [0u128; 4];
    let mut notional: u128 = 0;
    let mut aggr = Aggr::default();
    for (pk, p, _) in &positions {
        sum_q += u128::from(p.q);
        if let Some(t) = by_tier.get_mut(usize::from(p.tier)) {
            *t += u128::from(p.q);
        }
        notional += u128::from(p.n_open);
        if aggr
            .add(p.q, &p.rent_terms(), u32::from(p.rent_protocol_bps))
            .is_err()
        {
            return fail("I6", format!("position {pk}: rent aggregate overflows"));
        }
        let release = p.release_at().unwrap_or(i64::MAX);
        ensure(m.max_release >= release, "I6", || {
            format!(
                "market {key}: max_release {} < position {pk}'s release_at {release}",
                m.max_release
            )
        })?;
    }
    ensure(sum_q == u128::from(m.borrowed_q), "I6", || {
        format!(
            "market {key}: ΣB_Q {} != Σ open positions' q {sum_q}",
            m.borrowed_q
        )
    })?;
    for (tier, (want, have)) in by_tier.iter().zip(m.borrowed_q_by_tier.iter()).enumerate() {
        ensure(*want == u128::from(*have), "I6", || {
            format!("market {key}: borrowed_q_by_tier[{tier}] {have} != Σ q of tier {tier} {want}")
        })?;
    }
    ensure(positions.len() == m.open_positions as usize, "I6", || {
        format!(
            "market {key}: open_positions {} != {} ShortPosition accounts",
            m.open_positions,
            positions.len()
        )
    })?;
    ensure(notional == u128::from(m.total_notional_open), "I6", || {
        format!(
            "market {key}: total_notional_open {} != Σ n_open {notional}",
            m.total_notional_open
        )
    })?;
    let stored = m.aggr();
    ensure(stored == aggr, "I6", || {
        format!(
            "market {key}: rent_aggr {:?} != Σ q·coef·(10⁴ − p) {:?}",
            stored.a, aggr.a
        )
    })?;
    Ok(())
}

/// I13 (state part): the newest ring entry is the EMA state — `ts = ema_ts`, `p = P_ema`,
/// `l = L_ema` — after every integration and every liquidity rewrite (`observe::integrate`,
/// `rewrite_liquidity`). A market without a ring (list-only) has nothing to compare.
fn ema_is_newest_entry(key: &Pubkey, m: &Market) -> Result<(), Violation> {
    let ring = m.ring_math();
    let Some(newest) = ring.newest() else {
        return Ok(());
    };
    let ema = (m.ema_ts, m.ema_price.get(), m.ema_liq.get());
    ensure(ema == (newest.ts, newest.p, newest.l), "I13", || {
        format!(
            "market {key}: the EMA (ts, P, L) {ema:?} is not the newest ring entry ({}, {}, {})",
            newest.ts, newest.p, newest.l
        )
    })
}

/// I13 (the one-second lag, ECON §5.2, state part): no decision state holds the current second.
/// With now `= max(clock, last_accrual_ts)` (`prologue::market_now`): the last completed
/// bucket is from an earlier second (`lb_sec < now`), the open bucket is not from the future
/// (`b_sec ≤ now`), and neither is the EMA (`ema_ts ≤ now`). A bucket of second `now` is
/// integrated only at a later second, so an instruction never reads its own observation.
fn one_second_lag(svm: &LiteSVM, key: &Pubkey, m: &Market) -> Result<(), Violation> {
    let now = svm
        .get_sysvar::<Clock>()
        .unix_timestamp
        .max(m.last_accrual_ts);
    if m.lb_lo.get() != 0 {
        ensure(m.lb_sec < now, "I13", || {
            format!(
                "market {key}: the last completed bucket is second {} at now {now}",
                m.lb_sec
            )
        })?;
    }
    if m.b_lo.get() != 0 {
        ensure(m.b_sec <= now, "I13", || {
            format!(
                "market {key}: the open bucket is second {} at now {now}",
                m.b_sec
            )
        })?;
    }
    ensure(m.ema_ts <= now, "I13", || {
        format!("market {key}: ema_ts {} after now {now}", m.ema_ts)
    })
}

/// I7 (every instruction): `pool_vault` has no delegate — a delegate exists only inside a
/// Raydium sell leg (ARCH §2.4 rule 5), and only short.win (the vault's authority `market`)
/// can set one.
fn no_delegate_outside_legs(svm: &LiteSVM, key: &Pubkey, m: &Market) -> Result<(), Violation> {
    ensure(
        read::token_delegate(svm, &m.pool_vault).is_none(),
        "I7",
        || format!("market {key}: pool_vault has a delegate"),
    )
}

/// I7 (after a leg, pump venues): the `trader_base_ata` (when it exists) holds nothing, is
/// owned by `trader`, with no delegate and no close authority (ARCH §2.4 rule 5). Other venues
/// never use it (a third party may create and fill `ATA(trader, mint)` of any market).
fn trader_base_ata_clean(svm: &LiteSVM, key: &Pubkey, m: &Market) -> Result<(), Violation> {
    let pump = matches!(
        m.venue_kind(),
        Some(VenueKind::PumpCurve | VenueKind::PumpSwap | VenueKind::PumpMigrating)
    );
    if !pump || m.trader_base_ata == Pubkey::default() {
        return Ok(());
    }
    let Some(acc) = read::account(svm, &m.trader_base_ata) else {
        return Ok(());
    };
    if acc.lamports == 0 {
        return Ok(());
    }
    let amount = read::token_amount(svm, &m.trader_base_ata).unwrap_or(0);
    let owner = acc
        .data
        .get(32..64)
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .map(Pubkey::new_from_array);
    // `close_authority` is a `COption<Pubkey>` at 129 (tag u32).
    let close_auth = acc
        .data
        .get(129..133)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(u32::from_le_bytes);
    ensure(
        amount == 0
            && owner == Some(m.trader)
            && read::token_delegate(svm, &m.trader_base_ata).is_none()
            && close_auth == Some(0),
        "I7",
        || {
            format!(
                "market {key}: after a leg trader_base_ata holds {amount} (owner {owner:?}, \
                 close authority tag {close_auth:?}) or has a delegate"
            )
        },
    )
}

/// I6 (wallet exposure): every `WalletExposure` of the market sits at its PDA `["exposure",
/// market, wallet]` with `q_open` = Σ `q` of that wallet's open positions and `q_open > 0`
/// (the record is closed to the borrower at 0, ARCH §3.5 step 7), and every borrower with an
/// open position has one.
fn exposures_match(
    svm: &LiteSVM,
    key: &Pubkey,
    positions: &[(Pubkey, ShortPosition, u64)],
) -> Result<(), Violation> {
    let mut by_wallet: Vec<(Pubkey, u128)> = Vec::new();
    for (_, p, _) in positions {
        match by_wallet.iter_mut().find(|(w, _)| *w == p.borrower) {
            Some((_, q)) => *q += u128::from(p.q),
            None => by_wallet.push((p.borrower, u128::from(p.q))),
        }
    }
    let exposures: Vec<(Pubkey, WalletExposure)> = read::all_of::<WalletExposure>(svm)
        .into_iter()
        .filter_map(|(k, a)| {
            WalletExposure::try_deserialize(&mut a.data.as_slice())
                .ok()
                .filter(|e| e.market == *key)
                .map(|e| (k, e))
        })
        .collect();
    for (k, e) in &exposures {
        ensure(*k == pda::exposure(key, &e.wallet).0, "I6", || {
            format!("market {key}: exposure of {} at {k}, not its PDA", e.wallet)
        })?;
        let want = by_wallet
            .iter()
            .find(|(w, _)| *w == e.wallet)
            .map_or(0, |(_, q)| *q);
        ensure(u128::from(e.q_open) == want && e.q_open > 0, "I6", || {
            format!(
                "market {key}: exposure of {} has q_open {}, its positions owe {want}",
                e.wallet, e.q_open
            )
        })?;
    }
    for (w, q) in &by_wallet {
        ensure(exposures.iter().any(|(_, e)| e.wallet == *w), "I6", || {
            format!("market {key}: borrower {w} owes {q} with no exposure record")
        })?;
    }
    Ok(())
}

/// I13 (state part): every valid ring entry caches its reprojection `(x, y) = reprice(l, p)`.
fn ring_reprojection(key: &Pubkey, m: &Market) -> Result<(), Violation> {
    let ring = m.ring_math();
    for (k, e) in ring.iter().enumerate() {
        let want = MathRingEntry::reprojected(e.ts, e.p, e.l, e.lo, e.hi, e.llo, e.lhi);
        ensure(want.x == e.x && want.y == e.y, "I13", || {
            format!(
                "market {key}: ring entry {k} caches ({}, {}), reprice(l, p) = {:?}",
                e.x,
                e.y,
                reprice(e.l, e.p)
            )
        })?;
    }
    Ok(())
}

/// The events and identity of one instruction that the position hooks read.
pub struct IxFacts<'a> {
    pub opens: &'a [Open],
    pub closes: &'a [Close],
    pub liquidations: &'a [Liquidate],
    pub expiries: &'a [Expire],
    pub settlements: &'a [SolSettle],
    /// The instruction's account keys (I10).
    pub accounts: &'a [Pubkey],
}

impl IxFacts<'_> {
    fn exits_position(&self, position: &Pubkey) -> bool {
        self.closes.iter().any(|e| e.position == *position)
            || self.liquidations.iter().any(|e| e.position == *position)
            || self.expiries.iter().any(|e| e.position == *position)
            || self.settlements.iter().any(|e| e.position == *position)
    }

    fn ran_leg(&self, market: &Pubkey) -> bool {
        self.opens.iter().any(|e| e.market == *market)
            || self.closes.iter().any(|e| e.market == *market)
            || self.liquidations.iter().any(|e| e.market == *market)
            || self.expiries.iter().any(|e| e.market == *market)
    }

    fn is_exit(&self) -> bool {
        !(self.closes.is_empty()
            && self.liquidations.is_empty()
            && self.expiries.is_empty()
            && self.settlements.is_empty())
    }
}

/// The instruction-level position invariants (I3, A26, I5, I7, I9, I10, I13, I16) for one
/// instruction: `pre` just before it, `svm` just after it.
pub fn check_ix(svm: &LiteSVM, pre: &PositionsSnapshot, ix: &IxFacts<'_>) -> Result<(), Violation> {
    // I10: exits never read the oracle or the latch (structural: not even passed).
    if ix.is_exit() {
        let oracle = crate::pda::oracle_cache();
        let guard = crate::pda::venue_guard();
        ensure(
            !ix.accounts.contains(&oracle) && !ix.accounts.contains(&guard),
            "I10",
            || "an exit instruction names oracle_cache or venue_guard".to_string(),
        )?;
    }
    let config = read::zc::<GlobalConfig>(svm, &crate::pda::config());
    for mp in &pre.markets {
        let market = mp.market;
        let post = read::zc::<Market>(svm, &market);
        for (pk, p, before) in &mp.positions {
            let after = read::lamports(svm, pk);
            // I3: position SOL leaves only through an exit.
            if after < *before {
                ensure(ix.exits_position(pk), "I3", || {
                    format!(
                        "position {pk} lost {} lamports in an instruction that did not exit it",
                        before - after
                    )
                })?;
            }
            // I3 and A26 on a surviving position: `v` tracks SOL added by `add_margin` only,
            // and nothing snapshot at open changes.
            if let Some(now_p) = read::borsh::<ShortPosition>(svm, pk) {
                surviving_position(pk, p, *before, &now_p, after)?;
            }
            // I9 (close): the waterfall conserves V and splits the rent by the locked share.
            for c in ix.closes.iter().filter(|c| c.position == *pk) {
                let total =
                    u128::from(c.buyback_cost) + u128::from(c.rent) + u128::from(c.borrower_payout);
                ensure(total == u128::from(p.v), "I9", || {
                    format!(
                        "close of {pk}: buyback {} + rent {} + borrower {} != V {}",
                        c.buyback_cost, c.rent, c.borrower_payout, p.v
                    )
                })?;
                let protocol = u128::from(c.rent) * u128::from(p.rent_protocol_bps) / 10_000;
                ensure(u128::from(c.rent_protocol) == protocol, "I9", || {
                    format!(
                        "close of {pk}: rent_protocol {} != ⌊rent {}·{}/10⁴⌋ = {protocol}",
                        c.rent_protocol, c.rent, p.rent_protocol_bps
                    )
                })?;
                // I13: the venue charged within [B̂ − 5, B̂] (quotes are upper bounds).
                ensure(
                    c.buyback_cost <= c.b_live && c.b_live - c.buyback_cost <= 5,
                    "I13",
                    || {
                        format!(
                            "close of {pk}: venue charged {} against the quote {}",
                            c.buyback_cost, c.b_live
                        )
                    },
                )?;
                // I13 (mock): the event's quote is B̂_live(q) on the pool just before.
                if let Some(want) = mp.mock.and_then(|v| v.buy(p.q)) {
                    ensure(c.b_live == want, "I13", || {
                        format!(
                            "close of {pk}: b_live {} but quote::cpmm on the pre-state gives {want}",
                            c.b_live
                        )
                    })?;
                }
            }
        }
        exit_waterfalls(mp, post.as_ref(), ix)?;
        // I13: liquidation and expiry charges within [B̂ − 5, B̂] too (after the I9 / I11 /
        // I12 rows, so a mutant event fails its own invariant first).
        for (pk, cost, quote) in ix
            .liquidations
            .iter()
            .filter(|e| e.market == market)
            .map(|e| (e.position, e.buyback_cost, e.b_live))
            .chain(
                ix.expiries
                    .iter()
                    .filter(|e| e.market == market)
                    .map(|e| (e.position, e.buyback_cost, e.b_live)),
            )
        {
            ensure(cost <= quote && quote - cost <= 5, "I13", || {
                format!("exit of {pk}: venue charged {cost} against the quote {quote}")
            })?;
        }
        // I3: an exit closes the position it names (its V is paid out in full, I9). Checked
        // after the waterfall rows, so a mutant event fails its own invariant first.
        // (`SolSettle` is emitted only by a settlement that executes; ARCH §3.8, §3.10.)
        for (pk, _, _) in mp
            .positions
            .iter()
            .filter(|(pk, _, _)| ix.exits_position(pk))
        {
            ensure(
                read::borsh::<ShortPosition>(svm, pk).is_none(),
                "I3",
                || {
                    format!(
                        "position {pk} survived the exit that named it ({} lamports)",
                        read::lamports(svm, pk)
                    )
                },
            )?;
        }
        // I3: a new position appears only with this instruction's `Open`, whose figures it
        // stores.
        let known: Vec<Pubkey> = mp.positions.iter().map(|(k, _, _)| *k).collect();
        for (pk, p, _) in positions_of(svm, &market) {
            if known.contains(&pk) {
                continue;
            }
            let Some(o) = ix.opens.iter().find(|o| o.position == pk) else {
                return fail(
                    "I3",
                    format!("position {pk} appeared without an Open event"),
                );
            };
            let stored = (
                p.borrower,
                p.nonce,
                p.q,
                p.s,
                p.m,
                p.v,
                p.n_open,
                p.fee_open,
                p.tier,
                p.entry_price,
                p.deadline,
            );
            let event = (
                o.borrower,
                o.nonce,
                o.q,
                o.s,
                o.m,
                o.v,
                o.n_open,
                o.fee_open,
                o.tier,
                o.entry_price,
                o.deadline,
            );
            ensure(
                stored == event && p.v == p.s.saturating_add(p.m),
                "I3",
                || format!("position {pk} stores {stored:?}, its Open event says {event:?}"),
            )?;
        }
        let Some(post) = post else { continue };
        // I7: the leg left the trader state clean.
        if ix.ran_leg(&market) {
            let trader = read::account(svm, &post.trader);
            if let Some(t) = trader {
                ensure(
                    t.lamports == 0
                        && t.data.is_empty()
                        && t.owner == anchor_lang::system_program::ID,
                    "I7",
                    || {
                        format!(
                            "market {market}: trader holds {} lamports / {} bytes after a leg",
                            t.lamports,
                            t.data.len()
                        )
                    },
                )?;
            }
            let wsol = read::lamports(svm, &post.trader_wsol);
            ensure(wsol == 0, "I7", || {
                format!("market {market}: trader_wsol holds {wsol} lamports after a leg")
            })?;
            ensure(
                read::token_delegate(svm, &post.pool_vault).is_none(),
                "I7",
                || format!("market {market}: pool_vault has a delegate after a leg"),
            )?;
            trader_base_ata_clean(svm, &market, &post)?;
        }
        for o in ix.opens.iter().filter(|o| o.market == market) {
            // I5: utilisation within the cap after every open.
            let cap = config.map_or(0, |c| c.params.util_cap_bps);
            let b = u128::from(post.borrowed_q);
            let total = u128::from(post.tokens_free) + b;
            ensure(b * 10_000 <= u128::from(cap) * total, "I5", || {
                format!(
                    "open {}: utilisation ΣB_Q {b} / (T + ΣB_Q) {total} above the cap {cap} bps",
                    o.position
                )
            })?;
            // I13 (mock): the sale's proceeds within [S_exp, S_exp + 5] of the quote on the
            // pool just before (quotes are lower bounds on proceeds).
            if let Some(want) = mp.mock.and_then(|v| v.sell(o.q)) {
                ensure(o.s >= want && o.s - want <= 5, "I13", || {
                    format!(
                        "open {}: proceeds {} against quote::cpmm {want} on the pre-state",
                        o.position, o.s
                    )
                })?;
            }
        }
        // I13: one EMA integration moves P_ema and L_ema within their single-step bounds
        // (ECON §5.2).
        if post.ema_count == mp.state.ema_count + 1 {
            let tau = u128::from(shortwin_math::consts::EMA_TAU_S);
            let c = u128::from(post.ema_clamp_bps);
            let dt = u128::from(post.ema_dtmax_s.max(1));
            for (what, old, new) in [
                ("P_ema", mp.state.ema_price.get(), post.ema_price.get()),
                ("L_ema", mp.state.ema_liq.get(), post.ema_liq.get()),
            ] {
                if old == 0 {
                    continue;
                }
                // new ≤ (E·τ + E·(1 + C)·dt)/(τ + dt) and ≥ (E·τ + E/(1 + C)·dt)/(τ + dt).
                let hi = (old * tau * 10_000 + old * (10_000 + c) * dt) / ((tau + dt) * 10_000) + 1;
                let lo =
                    (old * tau * (10_000 + c) + old * 10_000 * dt) / ((tau + dt) * (10_000 + c));
                ensure(new <= hi && new + 1 >= lo, "I13", || {
                    format!(
                        "market {market}: one EMA step moved {what} {old} -> {new} (bounds [{lo}, {hi}])"
                    )
                })?;
            }
        }
        // I16: no observation is recorded while the venue cannot execute.
        if let (Some(false), Some(false)) = (mp.mock_executable, mock_executable(svm, &post)) {
            let bucket = |m: &Market| {
                (
                    m.b_sec,
                    m.b_lo.get(),
                    m.b_hi.get(),
                    m.b_llo.get(),
                    m.b_lhi.get(),
                )
            };
            let (b0, b1) = (bucket(&mp.state), bucket(&post));
            ensure(b1 == b0 || b1.1 == 0, "I16", || {
                format!(
                    "market {market}: an observation was recorded on a venue that cannot execute ({b0:?} -> {b1:?})"
                )
            })?;
        }
        // I16: the route chains are sound; `pool_fail` never reads the route-broken flag.
        if let Some(config) = config.as_ref() {
            route_chains(config, mp, &post)?;
        }
    }
    Ok(())
}

/// I3 and A26 for a position that exists before and after an instruction (module docs).
fn surviving_position(
    pk: &Pubkey,
    p: &ShortPosition,
    lamports_before: u64,
    now_p: &ShortPosition,
    lamports_after: u64,
) -> Result<(), Violation> {
    let dv = i128::from(now_p.v) - i128::from(p.v);
    let dm = i128::from(now_p.m) - i128::from(p.m);
    let dl = i128::from(lamports_after) - i128::from(lamports_before);
    ensure(dv >= 0 && dv == dm && dl >= dv, "I3", || {
        format!(
            "position {pk}: v {} -> {}, m {} -> {}, lamports {lamports_before} -> \
             {lamports_after} (v may only rise with m, backed by lamports: add_margin)",
            p.v, now_p.v, p.m, now_p.m
        )
    })?;
    let mut expect = now_p.clone();
    expect.v = p.v;
    expect.m = p.m;
    expect.s_armed = p.s_armed;
    expect.s_count = p.s_count;
    expect.s_since = p.s_since;
    expect.s_last = p.s_last;
    ensure(expect == *p, "A26", || {
        format!("position {pk}: a snapshot field changed: {p:?} -> {now_p:?}")
    })
}

/// I16 (route rules, ARCH §5.3; A40, V2-1, R3A-4): on a market whose venue view the harness has
/// (the mock; a list-only market is never executable), a route chain that was stepped by this
/// instruction with a hit (`last_*_obs` moved to a non-zero second) had its predicate true on
/// the state just before, and one that was cleared had it false. `s3_no_route = ¬executable ∨
/// route_broken(market or venue bit) ∨ φ > global hard cap`; `pool_fail = ¬executable ∨ φ >
/// global hard cap` — the flag is not an input.
fn route_chains(
    config: &GlobalConfig,
    mp: &MarketPositions,
    post: &Market,
) -> Result<(), Violation> {
    let pre = &mp.state;
    let kind = pre.venue_kind();
    let view = match (kind, mp.mock) {
        (Some(VenueKind::Mock), Some(v)) => v,
        (Some(VenueKind::None), _) => MockView {
            executable: false,
            reserves: None,
            fee_ppm: 0,
        },
        _ => return Ok(()),
    };
    let cap = u64::from(config.params.venue_fee_hard_cap_bps);
    let flag = pre.has_flag(MARKET_FLAG_ROUTE_BROKEN)
        || kind
            .is_some_and(|k| k.venue_bit() != 0 && config.venue_route_broken & k.venue_bit() != 0);
    let s3 = settle::s3_no_route(view.executable, flag, view.fee_ppm, cap);
    let pf = settle::pool_fail(view.executable, view.fee_ppm, cap);
    let market = mp.market;
    for (what, holds, (s0, l0), (s1, l1)) in [
        (
            "S3 (s3_no_route)",
            s3,
            (pre.no_route_since, pre.last_no_route_obs),
            (post.no_route_since, post.last_no_route_obs),
        ),
        (
            "pool_fail",
            pf,
            (pre.pool_fail_since, pre.last_pool_fail_obs),
            (post.pool_fail_since, post.last_pool_fail_obs),
        ),
    ] {
        if (s0, l0) == (s1, l1) {
            continue;
        }
        if l1 != 0 {
            ensure(holds, "I16", || {
                format!(
                    "market {market}: the {what} chain was hit ({s0}, {l0}) -> ({s1}, {l1}) \
                     while its predicate was false (executable {}, fee {} ppm, flag {flag})",
                    view.executable, view.fee_ppm
                )
            })?;
        } else {
            ensure(!holds, "I16", || {
                format!(
                    "market {market}: the {what} chain was cleared while its predicate held \
                     (executable {}, fee {} ppm, flag {flag})",
                    view.executable, view.fee_ppm
                )
            })?;
        }
    }
    Ok(())
}

/// `⌊b·(10⁴ + bps)/10⁴⌋`.
fn widen(b: u64, bps: u16) -> u128 {
    u128::from(b) * (10_000 + u128::from(bps)) / 10_000
}

/// `⌈x·bps/10⁴⌉`.
fn rate_of(x: u64, bps: u16) -> u128 {
    (u128::from(x) * u128::from(bps)).div_ceil(10_000)
}

/// `K_s = min(fixed + ⌊V·bps/10⁴⌋, cap, V)` with the position's locked terms.
fn k_s(p: &ShortPosition) -> u128 {
    (u128::from(p.settle_keeper_fixed) + u128::from(p.v) * u128::from(p.settle_keeper_bps) / 10_000)
        .min(u128::from(p.settle_keeper_cap))
        .min(u128::from(p.v))
}

/// I9, I11 and I12 read off the liquidation, expiry and settlement events of positions of this
/// market, against each position's `V`, locked terms and arming clock just before the
/// instruction and the market just after it (`post`: its rent basis is the instruction's
/// post-accrual one; its `ema_count` and S3 chain). I13's exit charges follow in `check_ix`.
fn exit_waterfalls(
    mp: &MarketPositions,
    post: Option<&Market>,
    ix: &IxFacts<'_>,
) -> Result<(), Violation> {
    for (pk, p, _) in &mp.positions {
        let v = u128::from(p.v);
        for e in ix.liquidations.iter().filter(|e| e.position == *pk) {
            let total = u128::from(e.buyback_cost)
                + u128::from(e.rent)
                + u128::from(e.penalty)
                + u128::from(e.keeper_topup)
                + u128::from(e.borrower_payout);
            ensure(total == v, "I9", || {
                format!("liquidation of {pk}: legs {total} != V {v}")
            })?;
            ensure(
                e.penalty_lender + e.penalty_protocol + e.penalty_keeper == e.penalty,
                "I9",
                || {
                    format!(
                        "liquidation of {pk}: penalty split does not sum to {}",
                        e.penalty
                    )
                },
            )?;
            ensure(
                u128::from(e.buyback_cost) <= widen(e.b_ref, p.delta_liq_bps),
                "I11",
                || {
                    format!(
                        "liquidation of {pk}: spend {} above B̂_ref {} + δ",
                        e.buyback_cost, e.b_ref
                    )
                },
            )?;
            ensure(
                u128::from(e.buyback_cost) <= u128::from(e.max_in) + u128::from(p.ert)
                    && u128::from(e.max_in) <= widen(e.b_ref, p.delta_liq_bps),
                "I11",
                || {
                    format!(
                        "liquidation of {pk}: spend {} / max_in {} against B̂_ref {} + δ and ert {}",
                        e.buyback_cost, e.max_in, e.b_ref, p.ert
                    )
                },
            )?;
            ensure(
                u128::from(e.penalty) <= rate_of(e.b_live_risk.min(e.b_ref), p.penalty_bps),
                "I11",
                || {
                    format!(
                        "liquidation of {pk}: penalty {} above the rate on min(B̂_live,risk, B̂_ref)",
                        e.penalty
                    )
                },
            )?;
            if e.borrower_payout > 0 {
                ensure(
                    u128::from(e.penalty_keeper) + u128::from(e.keeper_topup) >= k_s(p),
                    "I12",
                    || {
                        format!(
                            "liquidation of {pk}: keeper paid below K_s with borrower equity left"
                        )
                    },
                )?;
            }
        }
        for e in ix.expiries.iter().filter(|e| e.position == *pk) {
            let total = u128::from(e.buyback_cost)
                + u128::from(e.rent)
                + u128::from(e.expiry_fee)
                + u128::from(e.keeper_reward)
                + u128::from(e.borrower_payout);
            ensure(total == v, "I9", || {
                format!("expiry of {pk}: legs {total} != V {v}")
            })?;
            ensure(
                u128::from(e.buyback_cost) <= widen(e.b_ref, p.delta_exp_bps),
                "I11",
                || {
                    format!(
                        "expiry of {pk}: spend {} above B̂_ref {} + δ_exp",
                        e.buyback_cost, e.b_ref
                    )
                },
            )?;
            ensure(
                u128::from(e.buyback_cost) <= u128::from(e.max_in) + u128::from(p.ert)
                    && u128::from(e.max_in) <= widen(e.b_ref, p.delta_exp_bps),
                "I11",
                || {
                    format!(
                        "expiry of {pk}: spend {} / max_in {} against B̂_ref {} + δ_exp and ert {}",
                        e.buyback_cost, e.max_in, e.b_ref, p.ert
                    )
                },
            )?;
            let base = e.b_live_risk.min(e.b_ref);
            ensure(
                u128::from(e.expiry_fee) <= rate_of(base, p.expiry_fee_bps),
                "I11",
                || format!("expiry of {pk}: fee {} above the rate", e.expiry_fee),
            )?;
            ensure(
                u128::from(e.pi_ref) == rate_of(e.b_ref, p.expiry_fee_bps),
                "I11",
                || format!("expiry of {pk}: π_ref {} is not the fee on B̂_ref", e.pi_ref),
            )?;
            // K_e from the borrower's remainder (ECON §8.4).
            let k_e = (u128::from(p.expiry_keeper_fixed)
                + u128::from(e.b_ref) * u128::from(p.expiry_keeper_bps) / 10_000)
                .min(u128::from(p.expiry_keeper_cap));
            let w = v.saturating_sub(
                u128::from(e.buyback_cost) + u128::from(e.rent) + u128::from(e.expiry_fee),
            );
            ensure(u128::from(e.keeper_reward) == k_e.min(w), "I11", || {
                format!(
                    "expiry of {pk}: keeper reward {} != min(K_e {k_e}, remainder {w})",
                    e.keeper_reward
                )
            })?;
            // Rent and fee senior wherever the fair cost leaves room (ARCH §3.7, I-E2).
            if let Some(m) = post {
                let r_due = rent::due(&p.pos_rent(), &m.basis()).unwrap_or(u64::MAX);
                let room = i128::try_from(v).unwrap_or(i128::MAX)
                    - i128::from(r_due)
                    - i128::from(e.pi_ref)
                    - i128::from(p.ert);
                let fair = i128::try_from(widen(e.b_ref, p.exec_tol_bps)).unwrap_or(i128::MAX);
                if room >= fair {
                    ensure(
                        e.rent == r_due
                            && u128::from(e.expiry_fee) == rate_of(base, p.expiry_fee_bps),
                        "I11",
                        || {
                            format!(
                                "expiry of {pk}: rent {} / fee {} truncated although V − R − π_ref − ert \
                                 ({room}) ≥ B̂_ref·(1 + ε) ({fair}); R_due {r_due}",
                                e.rent, e.expiry_fee
                            )
                        },
                    )?;
                }
            }
        }
        for e in ix.settlements.iter().filter(|e| e.position == *pk) {
            let total =
                u128::from(e.k_s) + u128::from(e.lender_amount) + u128::from(e.borrower_amount);
            ensure(total == v, "I9", || {
                format!("settlement of {pk}: legs {total} != V {v}")
            })?;
            ensure(u128::from(e.k_s) == k_s(p), "I12", || {
                format!(
                    "settlement of {pk}: K_s {} != the locked terms' {}",
                    e.k_s,
                    k_s(p)
                )
            })?;
            match e.condition {
                1 => ensure(e.borrower_amount == 0, "I12", || {
                    format!(
                        "S1 of {pk}: the lenders take V − K_s, the borrower {}",
                        e.borrower_amount
                    )
                })?,
                2 | 3 => {
                    let (lo, hi) = (e.x_lo.min(e.x_hi), e.x_lo.max(e.x_hi));
                    let x = u128::from(e.lender_amount);
                    ensure(
                        x + 2 >= u128::from(lo) && x <= u128::from(hi) + 2,
                        "I12",
                        || {
                            format!(
                                "S{} of {pk}: take {} outside the box [{lo}, {hi}]",
                                e.condition, e.lender_amount
                            )
                        },
                    )?;
                    ensure(
                        u128::from(hi - lo) * 10_000 <= u128::from(p.fv_dev_bps) * u128::from(lo),
                        "I12",
                        || {
                            format!(
                                "S{} of {pk}: box corners {lo}..{hi} wider than fv_dev_bps",
                                e.condition
                            )
                        },
                    )?;
                }
                c => return fail("I12", format!("settlement of {pk}: unknown condition {c}")),
            }
            settlement_timing(pk, p, post, e)?;
            settlement_pprime(pk, p, e)?;
        }
    }
    Ok(())
}

/// I12 (ECON I-E8, I-E9): S1 and S2 execute only on a live clock (armed, its last evaluation
/// at most `settle_arm_gap_s` before) that has run `settle_persist_n` EMA updates — `s_count`
/// of the position before the instruction against the market's `ema_count` after it (a clock
/// armed in this very call has run none); S3 only on the market's continuous no-route chain,
/// observed in this call, at least `s3_wait_s` after `max(no_route_since, graduated_at +
/// grad_grace_s, migration_seen_at + max_migration_pause_s [migrating])`, the position's
/// locked timers.
fn settlement_timing(
    pk: &Pubkey,
    p: &ShortPosition,
    post: Option<&Market>,
    e: &SolSettle,
) -> Result<(), Violation> {
    let Some(m) = post else {
        return fail("I12", format!("settlement of {pk}: the market is gone"));
    };
    // The instruction's time: its prologue's `market_now` (never before the last accrual).
    let now = m.last_accrual_ts.max(e.ts);
    match e.condition {
        1 | 2 => {
            let live =
                p.s_armed == 1 && now.saturating_sub(p.s_last) <= i64::from(p.settle_arm_gap_s);
            let updates = m.ema_count.saturating_sub(p.s_count);
            ensure(
                live && p.s_count <= m.ema_count && updates >= u64::from(p.settle_persist_n),
                "I12",
                || {
                    format!(
                        "S{} of {pk} without persistence: armed {}, s_count {}, s_last {}, ema_count {}, now {now}",
                        e.condition, p.s_armed, p.s_count, p.s_last, m.ema_count
                    )
                },
            )
        }
        3 => {
            let at = |t: i64, add: u32| (t != 0).then(|| t.saturating_add(i64::from(add)));
            let mut start = m.no_route_since;
            if let Some(g) = at(m.graduated_at, p.grad_grace_s) {
                start = start.max(g);
            }
            if m.venue_kind() == Some(VenueKind::PumpMigrating) {
                let mig = at(m.migration_seen_at, p.max_migration_pause_s).unwrap_or(i64::MAX);
                start = start.max(mig);
            }
            ensure(
                m.no_route_since != 0
                    && m.last_no_route_obs == now
                    && now >= start.saturating_add(i64::from(p.s3_wait_s)),
                "I12",
                || {
                    format!(
                        "S3 of {pk} before its timer: no_route_since {}, last obs {}, start {start}, now {now}",
                        m.no_route_since, m.last_no_route_obs
                    )
                },
            )
        }
        _ => Ok(()),
    }
}

/// I12 (ECON I-E5, O15, R4A-3): FV's `p'` is decided at each corner. A position not expired
/// pays the penalty at both corners; an expired one pays the expiry fee at a corner only if its
/// CR there is at least `L` — the dearer corner `(P_hi, L_lo)` has the lower CR, so it cannot
/// have the expiry fee while the cheaper corner `(P_lo, L_hi)` has the penalty.
fn settlement_pprime(pk: &Pubkey, p: &ShortPosition, e: &SolSettle) -> Result<(), Violation> {
    if e.condition == 1 {
        return Ok(());
    }
    let (pen, fee) = (p.penalty_bps, p.expiry_fee_bps);
    let valid = |x: u16| x == pen || x == fee || x == 0;
    let expired = e.ts >= p.deadline;
    let ok = valid(e.pprime_lo)
        && valid(e.pprime_hi)
        && if expired {
            !(e.pprime_lo == pen && e.pprime_hi == fee && pen != fee)
        } else {
            (e.pprime_lo == pen || e.pprime_lo == 0) && (e.pprime_hi == pen || e.pprime_hi == 0)
        };
    ensure(ok, "I12", || {
        format!(
            "S{} of {pk}: p' {} / {} (penalty {pen}, expiry fee {fee}, expired {expired})",
            e.condition, e.pprime_lo, e.pprime_hi
        )
    })
}
