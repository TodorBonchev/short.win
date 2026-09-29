//! I18 (ARCH §8.3, §4.4; ECON §5.2; M4R post-change confirmation, **PROPOSED**, owner item
//! O18), checked by `check_ix` against the state just before each instruction
//! (`super::positions::PositionsSnapshot`) and the instruction's own events.
//!
//! | Part | What is checked |
//! |---|---|
//! | (a) restart | only an instruction that emits `PoolSwitched` for the market, or a Raydium `create_market`, raises `ema_warm` (to `EMA_WARMUP_BUCKETS`) or sets `ema_guard`; after such an instruction both are `(8, 1)`; a market created on another venue starts `(0, 0)` |
//! | (b) countdown and seed | across any other instruction `ema_warm` falls by the number of integrations (0 or 1) while it is positive and stays 0 otherwise; a warm-up integration leaves the newest ring entry and the EMA at the §4.1 seed over the newest `8 − ema_warm_after` entries, `p = min(min lo, the largest v0.4.1 step from the pre-instruction P_ema)` (M4X-1: `observe::price_up_bound`, `dt = min(ts − ema_ts, ema_dtmax_s)`), `l = min(max lhi, ⌊lomed(lhi)·(10⁴ + C)/10⁴⌋)` (recomputed here), with `L_slow` and `slow_ts` unchanged |
//! | (c) gate | no `Liquidate` or `Expire` event, no `SolSettle` of S1/S2 and no `SettleArmed` / `SettleDisarmed` on a market whose post-instruction `ema_warm ≠ 0` while its venue is executable; no Raydium `Open` with `ema_warm ≠ 0` |
//! | (d) guard | `ema_guard` goes 1 → 0 only at an integration with `ema_warm = 0` after it and `L_ema·10⁴ ≤ L_mlo·10,100` (and does so at every such integration); an integration after the warm-up with the guard set never lifts `L_slow` above `max(L_slow before, L_mlo after)` (M4X-2: `L_slow` integrates `min(lobs, L_mlo)`); on a Raydium market, and on any market while the guard is set, `Liquidate.b_ref` / `Expire.b_ref` are the risk quote at `reprice(max(L_live, min(max(L_ema', L_slow), cap)), P_ref)`, `L_ema' = min(L_ema, L_mlo)` while the guard is set, else `L_ema` (V2-14), `cap = ⌊pool_d·10⁴/λ_min⌋` on Raydium, none elsewhere (M4F-1). On the mock, which never restarts, the guard-clear case is the exit predictors' (`liquidate_support`), and the hook tests' synthetic exits there carry arbitrary references |
//!
//! | (e) spot leg | FITS-2 (a): every `Liquidate` on an executable Raydium venue is CR-liquidatable, and its spot leg is judged at `max(L_live, L_ema')` — `Liquidate.cr_spot_bps` is `⌊V_eff·10⁴/B̂⌋` of the risk quote `B̂ = B̂(Q; reprice(max(L_live, L_ema'), P_rs))` (recomputed here: `L_live` the pre-trade venue's `√(X·Y)`, `L_ema' = min(L_ema, L_mlo)` while the guard is set, else `L_ema`, `P_rs = min(P_live, lb_lo)` fresh within `ref_max_age_s`, `V_eff = V − R_due`) and it is below the position's `L` — so no liquidation lands on a spot leg that only a drained `L_live` makes true. On the mock the exit predictors (`liquidate_support`, the same rule) check it, and the hook tests' synthetic exits there carry arbitrary figures |
//!
//! `L_mlo` is the lower median of the ring's bucket liquidity lows. A market that never saw a
//! restart has `ema_warm = ema_guard = 0` throughout and integrates exactly as before M4R.

use litesvm::LiteSVM;
use shortwin::events::{
    Expire, Liquidate, MarketCreated, Open, PoolSwitched, SettleArmed, SettleDisarmed, SolSettle,
};
use shortwin::state::{GlobalConfig, Market, ShortPosition, VenueKind};
use shortwin_math::bounds;
use shortwin_math::consts::{EMA_WARMUP_BUCKETS, REF_GUARD_TOL_BPS};
use shortwin_math::num::{Rounding, mul_div};
use shortwin_math::observe::{self, Observation, price_up_bound, ring_medians};
use shortwin_math::quote::liquidity;
use shortwin_math::rent;
use solana_clock::Clock;

use super::positions::{MarketPositions, PositionsSnapshot};
use super::venue_view::VenueView;
use super::{Violation, ensure, fail};
use crate::read;

/// The events of one instruction that I18 reads.
pub struct ConfirmFacts<'a> {
    pub created: &'a [MarketCreated],
    pub switched: &'a [PoolSwitched],
    pub opens: &'a [Open],
    pub liquidations: &'a [Liquidate],
    pub expiries: &'a [Expire],
    pub settlements: &'a [SolSettle],
    pub armed: &'a [SettleArmed],
    pub disarmed: &'a [SettleDisarmed],
}

/// The instruction-level I18 checks for one instruction (module docs).
pub fn check_ix(
    svm: &LiteSVM,
    pre: &PositionsSnapshot,
    ix: &ConfirmFacts<'_>,
) -> Result<(), Violation> {
    for c in ix.created {
        created(svm, c)?;
    }
    for mp in &pre.markets {
        let Some(post) = read::zc::<Market>(svm, &mp.market) else {
            continue;
        };
        let switched = ix.switched.iter().any(|e| e.market == mp.market);
        if switched {
            ensure(
                (post.ema_warm, post.ema_guard) == (EMA_WARMUP_BUCKETS, 1),
                "I18",
                || {
                    format!(
                        "market {}: ema_warm {} / ema_guard {} after PoolSwitched, want {} / 1",
                        mp.market, post.ema_warm, post.ema_guard, EMA_WARMUP_BUCKETS
                    )
                },
            )?;
        } else {
            countdown_and_seed(mp, &post)?;
            guard_transition(mp, &post)?;
        }
        gate(mp, &post, ix)?;
        references(svm, mp, &post, ix)?;
        spot_legs(svm, mp, &post, ix)?;
    }
    Ok(())
}

/// (a) at creation: a Raydium listing is marked, any other venue is not.
fn created(svm: &LiteSVM, c: &MarketCreated) -> Result<(), Violation> {
    let Some(m) = read::zc::<Market>(svm, &c.market) else {
        return Ok(());
    };
    let raydium = VenueKind::from_u8(c.venue).is_some_and(VenueKind::is_raydium);
    let want = if raydium {
        (EMA_WARMUP_BUCKETS, 1)
    } else {
        (0, 0)
    };
    ensure((m.ema_warm, m.ema_guard) == want, "I18", || {
        format!(
            "market {} created on venue {}: ema_warm {} / ema_guard {}, want {want:?}",
            c.market, c.venue, m.ema_warm, m.ema_guard
        )
    })
}

/// The lower median of `vals` (index `⌊(n − 1)/2⌋` of the sorted values).
fn lomed(mut vals: Vec<u128>) -> u128 {
    vals.sort_unstable();
    vals[(vals.len() - 1) / 2]
}

/// (a) and (b) on an instruction without `PoolSwitched`.
fn countdown_and_seed(mp: &MarketPositions, post: &Market) -> Result<(), Violation> {
    let pre = &mp.state;
    let market = mp.market;
    let steps = post.ema_count.wrapping_sub(pre.ema_count);
    ensure(steps <= 1, "I18", || {
        format!(
            "market {market}: {steps} integrations in one instruction (ema_count {} -> {})",
            pre.ema_count, post.ema_count
        )
    })?;
    let want_warm = if pre.ema_warm > 0 {
        pre.ema_warm - u8::from(steps == 1)
    } else {
        0
    };
    ensure(post.ema_warm == want_warm, "I18", || {
        format!(
            "market {market}: ema_warm {} -> {} across {steps} integration(s) without a restart",
            pre.ema_warm, post.ema_warm
        )
    })?;
    ensure(pre.ema_guard == 1 || post.ema_guard == 0, "I18", || {
        format!("market {market}: ema_guard set without a restart")
    })?;
    if pre.ema_warm == 0 || steps == 0 {
        return Ok(());
    }
    // A warm-up integration: the seed over the post-change entries.
    let obs = post.observation();
    let k = usize::from(EMA_WARMUP_BUCKETS - post.ema_warm);
    let ents: Vec<_> = obs.ring.iter().take(k).copied().collect();
    let Some(newest) = ents.first().copied() else {
        return fail(
            "I18",
            format!("market {market}: a warm-up step left no ring entry"),
        );
    };
    // M4X-1: the price seed rises no faster than the largest v0.4.1 step from the
    // pre-instruction P_ema.
    let dt = u128::try_from(
        newest
            .ts
            .saturating_sub(pre.ema_ts)
            .clamp(0, i64::from(post.ema_dtmax_s)),
    )
    .unwrap_or(0);
    let bound = price_up_bound(pre.ema_price.get(), dt, post.ema_clamp_bps).unwrap_or(u128::MAX);
    let p = ents.iter().map(|e| e.lo).min().unwrap_or(0).min(bound);
    let highs: Vec<u128> = ents.iter().map(|e| e.lhi).collect();
    let max = highs.iter().copied().max().unwrap_or(0);
    let up = mul_div(
        lomed(highs),
        10_000 + u128::from(post.ema_clamp_bps),
        10_000,
        Rounding::Floor,
    )
    .unwrap_or(u128::MAX);
    let l = max.min(up);
    ensure(
        ents.len() == k
            && (newest.p, newest.l) == (p, l)
            && (obs.ema.price, obs.ema.liq) == (p, l)
            && (post.liq_slow.get(), post.slow_ts) == (pre.liq_slow.get(), pre.slow_ts),
        "I18",
        || {
            format!(
                "market {market}: warm-up step {k}: newest entry (p, l) = ({}, {}), EMA ({}, {}), \
                 want the seed ({p}, {l}) over {} entries; L_slow {} -> {}, slow_ts {} -> {}",
                newest.p,
                newest.l,
                obs.ema.price,
                obs.ema.liq,
                ents.len(),
                pre.liq_slow.get(),
                post.liq_slow.get(),
                pre.slow_ts,
                post.slow_ts
            )
        },
    )
}

/// (d) the guard clears exactly at the integrations that find `L_ema` within
/// `REF_GUARD_TOL_BPS` of `L_mlo` after the warm-up.
fn guard_transition(mp: &MarketPositions, post: &Market) -> Result<(), Violation> {
    let pre = &mp.state;
    if pre.ema_guard == 0 {
        return Ok(());
    }
    let market = mp.market;
    let integrated = post.ema_count != pre.ema_count;
    let l_mlo = ring_medians(&post.ring_math()).map_or(0, |m| m.2);
    let within = post.ema_liq.get().saturating_mul(10_000)
        <= l_mlo.saturating_mul(10_000 + u128::from(REF_GUARD_TOL_BPS));
    let want = if integrated && post.ema_warm == 0 && within {
        0
    } else {
        1
    };
    ensure(post.ema_guard == want, "I18", || {
        format!(
            "market {market}: ema_guard 1 -> {} (integrated {integrated}, ema_warm {}, L_ema {}, \
             L_mlo {l_mlo}), want {want}",
            post.ema_guard,
            post.ema_warm,
            post.ema_liq.get()
        )
    })?;
    // M4X-2: a guarded steady integration feeds L_slow at most the ring's lows.
    let steady = integrated && pre.ema_warm == 0;
    let cap = pre.liq_slow.get().max(l_mlo);
    ensure(!steady || post.liq_slow.get() <= cap, "I18", || {
        format!(
            "market {market}: L_slow {} -> {} while the guard was set, above max(L_slow, L_mlo \
             {l_mlo}) (L_ema {})",
            pre.liq_slow.get(),
            post.liq_slow.get(),
            post.ema_liq.get()
        )
    })
}

/// (c) the gate.
fn gate(mp: &MarketPositions, post: &Market, ix: &ConfirmFacts<'_>) -> Result<(), Violation> {
    if post.ema_warm == 0 {
        return Ok(());
    }
    let market = mp.market;
    let executable = mp.venue.is_some_and(|v| v.executable);
    let raydium = post.venue_kind().is_some_and(VenueKind::is_raydium);
    let opens = ix.opens.iter().filter(|e| e.market == market).count();
    ensure(!raydium || opens == 0, "I18", || {
        format!(
            "market {market}: {opens} Raydium open(s) with ema_warm {}",
            post.ema_warm
        )
    })?;
    if !executable {
        return Ok(());
    }
    let exits = ix
        .liquidations
        .iter()
        .filter(|e| e.market == market)
        .count()
        + ix.expiries.iter().filter(|e| e.market == market).count();
    let s12 = ix
        .settlements
        .iter()
        .filter(|e| e.market == market && e.condition != 3)
        .count();
    let clock = ix.armed.iter().filter(|e| e.market == market).count()
        + ix.disarmed.iter().filter(|e| e.market == market).count();
    ensure(exits + s12 + clock == 0, "I18", || {
        format!(
            "market {market}: {exits} keeper exit(s), {s12} S1/S2 settlement(s) and {clock} arm \
             change(s) while warming up on an executable venue (ema_warm {})",
            post.ema_warm
        )
    })
}

/// (e) FITS-2 (a): a liquidation's spot leg holds at `max(L_live, L_ema')` (module docs).
fn spot_legs(
    svm: &LiteSVM,
    mp: &MarketPositions,
    post: &Market,
    ix: &ConfirmFacts<'_>,
) -> Result<(), Violation> {
    let market = mp.market;
    // On the mock the exit predictors (`liquidate_support`) check the spot leg, and the hook
    // tests' synthetic exits there carry arbitrary figures.
    if !post.venue_kind().is_some_and(VenueKind::is_raydium) {
        return Ok(());
    }
    let Some(v) = mp.venue.filter(|v| v.executable) else {
        return Ok(());
    };
    let (Some((x, y)), Some(pricing)) = (v.reserves, v.pricing) else {
        return Ok(());
    };
    let obs = post.observation();
    let now = svm
        .get_sysvar::<Clock>()
        .unix_timestamp
        .max(mp.state.last_accrual_ts);
    let (x, y) = (u128::from(x), u128::from(y));
    let l_live = liquidity(x, y).ok();
    let p_live = shortwin_math::risk::price_wad(x, y).ok();
    let l_ema = if obs.ema.guard != 0 {
        obs.ema
            .liq
            .min(lomed(obs.ring.iter().map(|e| e.llo).collect()))
    } else {
        obs.ema.liq
    };
    for e in ix.liquidations.iter().filter(|e| e.market == market) {
        let Some((_, pos, _)) = mp.positions.iter().find(|(pk, _, _)| *pk == e.position) else {
            continue;
        };
        let l_spot = l_live.map(|l| l.max(l_ema));
        let p_rs = p_live.and_then(|p| observe::robust_spot(p, &obs.last, now, pos.ref_max_age_s));
        let b = match (l_spot, p_rs) {
            (Some(l), Some(p)) => pricing
                .capped(u64::from(pos.venue_fee_hard_cap_bps))
                .buy_repriced(pos.q, l, p)
                .map(|b| b.cost),
            _ => None,
        };
        let r_due = rent::due(&pos.pos_rent(), &post.basis()).unwrap_or(u64::MAX);
        let v_eff = shortwin_math::risk::v_eff(pos.v, r_due);
        let want = shortwin_math::risk::cr_bps(v_eff, b).unwrap_or(u64::MAX);
        ensure(
            e.cr_spot_bps == want && u128::from(want) < u128::from(pos.cr_liq_bps),
            "I18",
            || {
                format!(
                    "market {market}: Liquidate {} cr_spot_bps {}, want {want} < L {} at                      max(L_live {l_live:?}, L_ema' {l_ema}) and P_rs {p_rs:?} (FITS-2)",
                    e.position, e.cr_spot_bps, pos.cr_liq_bps
                )
            },
        )?;
    }
    Ok(())
}

/// `B̂_ref` of `pos` at `p_ref` with the (guarded, on Raydium capped) reference liquidity of
/// `post` and the pre-trade venue `v` (V2-14, M4R, M4F-1). `cap` is `⌊pool_d·10⁴/λ_min⌋` on a
/// Raydium market (recomputed here from the market's `pool_d` and the config), `None` elsewhere.
fn b_ref_of(
    pos: &ShortPosition,
    obs: &Observation,
    v: &VenueView,
    p_ref: u128,
    cap: Option<u128>,
) -> Option<u64> {
    let (x, y) = v.reserves?;
    let l_live = liquidity(u128::from(x), u128::from(y)).ok()?;
    let l_ema = if obs.ema.guard != 0 {
        obs.ema.liq.min(ring_medians(&obs.ring).ok()?.2)
    } else {
        obs.ema.liq
    };
    let credited = l_ema.max(obs.ema.liq_slow);
    let l_b = l_live.max(cap.map_or(credited, |c| credited.min(c)));
    // The program's own function agrees with the recomputation above.
    if bounds::ref_liquidity(l_live, &obs.ema, &obs.ring, cap).ok()? != l_b {
        return None;
    }
    v.pricing?
        .capped(u64::from(pos.venue_fee_hard_cap_bps))
        .buy_repriced(pos.q, l_b, p_ref)
        .map(|b| b.cost)
}

/// (d) the keeper exits' `B̂_ref` reads the guarded reference liquidity.
fn references(
    svm: &LiteSVM,
    mp: &MarketPositions,
    post: &Market,
    ix: &ConfirmFacts<'_>,
) -> Result<(), Violation> {
    let market = mp.market;
    let Some(v) = mp.venue.filter(|v| v.executable) else {
        return Ok(());
    };
    if post.ema_guard == 0 && !post.venue_kind().is_some_and(VenueKind::is_raydium) {
        return Ok(());
    }
    let obs = post.observation();
    let now = svm
        .get_sysvar::<Clock>()
        .unix_timestamp
        .max(mp.state.last_accrual_ts);
    // M4F-1: on Raydium the EMA terms are capped at ⌊D·10⁴/λ_min⌋, D the locked depth the
    // exit's prologue refreshed (`pool_d`).
    let cap = if post.venue_kind().is_some_and(VenueKind::is_raydium) {
        let Some(config) = read::zc::<GlobalConfig>(svm, &crate::pda::config()) else {
            return fail("I18", format!("market {market}: no config"));
        };
        let lam = u128::from(config.params.pool_lock_min_bps);
        Some(if lam == 0 {
            0
        } else {
            mul_div(post.pool_d.get(), 10_000, lam, Rounding::Floor).unwrap_or(u128::MAX)
        })
    } else {
        None
    };
    let pos_of = |k: &anchor_lang::prelude::Pubkey| {
        mp.positions
            .iter()
            .find(|(pk, _, _)| pk == k)
            .map(|(_, p, _)| p.clone())
    };
    for e in ix.liquidations.iter().filter(|e| e.market == market) {
        let Some(pos) = pos_of(&e.position) else {
            continue;
        };
        let want = b_ref_of(&pos, &obs, &v, e.p_ref, cap);
        ensure(want == Some(e.b_ref), "I18", || {
            format!(
                "market {market}: Liquidate {} b_ref {} at p_ref {}, want {want:?} (guard {}, \
                 L_ema {}, L_slow {})",
                e.position, e.b_ref, e.p_ref, obs.ema.guard, obs.ema.liq, obs.ema.liq_slow
            )
        })?;
    }
    for e in ix.expiries.iter().filter(|e| e.market == market) {
        let Some(pos) = pos_of(&e.position) else {
            continue;
        };
        let p_ref =
            bounds::expiry_reference(&obs.ema, &obs.ring, &obs.last, now, &pos.exit_params())
                .ok()
                .flatten();
        let want = p_ref.and_then(|p| b_ref_of(&pos, &obs, &v, p, cap));
        ensure(want == Some(e.b_ref), "I18", || {
            format!(
                "market {market}: Expire {} b_ref {}, want {want:?} at P_ref {p_ref:?} (guard {})",
                e.position, e.b_ref, obs.ema.guard
            )
        })?;
    }
    Ok(())
}
