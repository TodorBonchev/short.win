//! S3 on the mock venue (ARCH §3.8 step 4, §5.3; ECON §10.3; SPEC §5.3, §11 O6): T-S3-BROKEN
//! (the Emergency route-broken flag starts the S3 chain and changes nothing else), a disabled
//! venue and a venue-state layout change (not executable, A63: the chain starts by itself), the
//! S3 timer and its continuity, and the S3 amount (FV at the frozen EMA point, O15's expiry
//! fee for an expired position).

#![allow(clippy::result_large_err)]

#[path = "settle_support.rs"]
mod support;

use shortwin::errors::ShortwinError as E;
use shortwin_math::fees::CpmmFees;
use shortwin_math::quote::{cpmm, reprice};
use shortwin_math::rent;
use support::*;

/// `FV(P, L) = B̂(Q; reprice(L, P)) + ⌈p'·B̂/10⁴⌉ + R_due` at a one-way fee of `fee_ppm`
/// (ARCH §4.5), recomputed off-chain from the math crate.
fn fair_value(q: u64, p: u128, l: u128, fee_ppm: u64, pprime_bps: u16, r_due: u64) -> u64 {
    let (x, y) = reprice(l, p).expect("reprice");
    let fees = CpmmFees {
        trade: fee_ppm,
        ..mock::cpmm_fees(0)
    };
    let b = cpmm::buy_exact_out(q, y, x, &fees).expect("quote").cost;
    b + (u128::from(b) * u128::from(pprime_bps)).div_ceil(10_000) as u64 + r_due
}

/// Ages the market past `graduated_at + grad_grace_s` (the mock market's age counts from its
/// listing: no S3 can start earlier, ARCH §5.3), then moves the pool to `frac` of the S1
/// boundary (from the spot before the move) and lets the EMA follow: the position is then
/// CR-liquidatable on both legs but outside S1 (and, on a deep pool, S2): it would be
/// liquidated normally.
fn make_liquidatable(sc: &mut Sc, frac: f64) {
    let grace = i64::from(sc.pos().grad_grace_s);
    let age = sc.env.now() - sc.market().graduated_at;
    if age < grace {
        sc.env.warp(grace - age);
        sc.env.tick(&sc.mm, 5);
    }
    let p0 = sc.spot();
    let kb = sc.s1_boundary();
    sc.move_to(scale(p0, kb * frac));
    for _ in 0..20 {
        let out = sc.second();
        assert!(out.settled.is_none() && out.armed.is_none());
    }
}

mod t_s3_broken {
    use super::*;

    /// T-S3-BROKEN (ARCH §5.3, §8.2; V2-1, V1-7): an expired healthy position under the
    /// Emergency flag — market scope or the venue's bit — settles S3 once the continuous chain
    /// has run `s3_wait_s`; before its deadline the same position never settles (the flag never
    /// makes the spot leg true). The venue still executes, so the box is the ring's, and the
    /// amount is FV at the EMA with the **expiry fee** at both corners (O15), below `V − K_s`.
    #[test]
    fn a_flagged_route_settles_expired_positions_after_the_timer() {
        for venue_scope in [false, true] {
            let mut sc = Sc::default_market();
            sc.env.tick(&sc.mm, 5);
            // 700 s before the deadline the Emergency multisig flags the route.
            sc.warp_to_deadline(-700);
            if venue_scope {
                sc.set_venue_route_broken(true);
            } else {
                sc.set_route_broken(true);
            }
            // The keeper observes every 30 s (duty 5) and calls sol_settle: a healthy position
            // with the flag set is a no-op, however long the chain.
            for _ in 0..23 {
                sc.env.warp(30);
                sc.crank();
                let out = sc.settle_ok();
                assert!(out.settled.is_none() && out.armed.is_none());
            }
            let m = sc.market();
            assert!(m.no_route_since > 0 && sc.env.now() - m.no_route_since >= 600);
            assert_eq!(m.pool_fail_since, 0, "pool_fail never reads the flag");
            assert!(sc.env.now() < sc.pos().deadline);
            let pos = sc.pos();
            // At the deadline, per-second keeper calls: S3.
            sc.warp_to_deadline(0);
            let (_s, ev) = sc.run_until_settled(10).expect("S3");
            assert_eq!(ev.condition, 3);
            assert_eq!(
                (ev.pprime_lo, ev.pprime_hi),
                (pos.expiry_fee_bps, pos.expiry_fee_bps),
                "an expired position that is not CR-liquidatable pays the expiry fee (O15)"
            );
            let take = pos.v - k_s(&pos);
            assert!(ev.fv > 0 && ev.lender_amount == ev.fv.min(take) && ev.lender_amount < take);
            assert_eq!(ev.borrower_amount, take - ev.lender_amount);
            assert!(ev.ema_age_s <= 1, "the venue is observed: a fresh EMA");
            assert!(ev.p_lo <= ev.p_hi && ev.l_lo <= ev.l_hi);
        }
    }

    /// T-S3-BROKEN: with the flag set, the arming clock, S1's timing and amount, every close
    /// outcome and the `pool_fail` chain equal those of the unflagged run from the same state;
    /// the flag only adds the S3 chain.
    #[test]
    fn the_flag_changes_nothing_but_the_s3_chain() {
        let mut sc = Sc::default_market();
        sc.env.tick(&sc.mm, 5);
        let saved = sc.env.svm.clone();
        let mut runs = Vec::new();
        for flag in [false, true] {
            sc.env.svm = saved.clone();
            if flag {
                sc.set_route_broken(true);
            }
            let kb = sc.s1_boundary();
            sc.move_by(kb * 1.05);
            let mut trace = Vec::new();
            let mut settled = None;
            for s in 1..=80u32 {
                let out = sc.second();
                let m = sc.market();
                trace.push((
                    out.armed.map(|a| a.s_count),
                    sc.pos_opt().map(|p| p.arm_state()),
                    m.pool_fail_since,
                    m.ema_count,
                ));
                if let Some(ev) = out.settled {
                    settled = Some((s, ev.condition, ev.lender_amount, ev.borrower_amount));
                    break;
                }
            }
            let m = sc.market();
            runs.push((trace, settled, m.no_route_since));
        }
        let (a, b) = (&runs[0], &runs[1]);
        assert_eq!(a.0, b.0, "arming, clock and pool_fail chain are identical");
        assert_eq!(a.1, b.1, "S1 at the same second with the same amount");
        assert_eq!(a.1.map(|s| s.1), Some(1));
        assert_eq!(a.2, 0);
        assert!(b.2 > 0, "only the flagged run has an S3 chain");
    }

    /// T-S3-BROKEN (the close side): under the flag a liquidatable position still closes, with
    /// the same buyback, rent and payout as without it (I10, V2-1); and a liquidatable position
    /// that nobody liquidates settles S3 at the timer under the flag only.
    #[test]
    fn closes_are_unchanged_and_liquidatable_positions_reach_s3() {
        let mut sc = Sc::default_market();
        sc.env.tick(&sc.mm, 5);
        make_liquidatable(&mut sc, 0.8);
        let saved = sc.env.svm.clone();
        let (honest, _) = close_ok(&mut sc.env, &sc.mm, &sc.b, sc.nonce);
        sc.env.svm = saved.clone();
        sc.set_route_broken(true);
        let (flagged, _) = close_ok(&mut sc.env, &sc.mm, &sc.b, sc.nonce);
        assert_eq!(
            (flagged.buyback_cost, flagged.rent, flagged.borrower_payout),
            (honest.buyback_cost, honest.rent, honest.borrower_payout)
        );
        // Unflagged: the liquidatable position just waits (the liquidation track's exit).
        sc.env.svm = saved.clone();
        sc.crank_every(30, 21);
        assert!(sc.settle_ok().settled.is_none());
        // Flagged: S3 once the chain has run s3_wait_s, capped at FV with the penalty.
        sc.env.svm = saved;
        sc.set_route_broken(true);
        let pos = sc.pos();
        sc.crank_every(30, 20);
        assert!(
            sc.settle_ok().settled.is_none(),
            "the S3 timer is still running"
        );
        let (_s, ev) = sc.run_until_settled(40).expect("S3");
        assert_eq!(ev.condition, 3);
        assert_eq!(
            (ev.pprime_lo, ev.pprime_hi),
            (pos.penalty_bps, pos.penalty_bps)
        );
        let m = sc.market();
        assert!(sc.env.now() >= m.no_route_since + i64::from(pos.s3_wait_s));
    }
}

/// The keeper's exits of a liquidatable position under a price path, second by second: every
/// second the keeper cranks and tries `liquidate`; the refusal codes until it executes, and the
/// executed event's amounts.
type LiqRun = (Vec<Option<u32>>, Option<(u32, [u64; 10])>, i64, i64);

fn liquidation_run(sc: &mut Sc, flag: bool, keeper: &solana_keypair::Keypair) -> LiqRun {
    use solana_signer::Signer;
    if flag {
        sc.set_route_broken(true);
    }
    let mut refusals = Vec::new();
    let mut done = None;
    for s in 1..=90u32 {
        sc.move_by(1.01);
        sc.env.warp(1);
        sc.crank();
        let ix = shortwin_litesvm_tests::engine::liquidate_ix(
            &sc.mm,
            &keeper.pubkey(),
            &sc.b.key(),
            sc.nonce,
        );
        let res = sc.env.send(&[ix], &[keeper]);
        match res {
            Ok(meta) => {
                let e: shortwin::events::Liquidate = events::one(&meta);
                done = Some((
                    s,
                    [
                        e.buyback_cost,
                        e.b_live,
                        e.b_ref,
                        e.max_in,
                        e.rent,
                        e.penalty,
                        e.penalty_keeper,
                        e.keeper_topup,
                        e.borrower_payout,
                        e.token_surplus,
                    ],
                ));
                break;
            }
            Err(_) => refusals.push(custom_code(&res)),
        }
    }
    let m = sc.market();
    (refusals, done, m.pool_fail_since, m.no_route_since)
}

mod t_s3_broken_exits {
    use super::*;
    use solana_signer::Signer;

    /// T-S3-BROKEN (the liquidation side; V2-1, I10): along the same rising path, with and
    /// without the route-broken flag, the keeper's `liquidate` is refused with the same codes
    /// in the same seconds, executes in the same second with identical amounts (buyback,
    /// quotes, bound, rent, penalty and its split, keeper top-up, borrower payout), and the
    /// `pool_fail` chain stays empty in both runs. The flag's only trace is its S3 chain.
    #[test]
    fn the_liquidatable_set_and_liquidations_equal_the_unflagged_run() {
        let mut sc = Sc::default_market();
        sc.env.tick(&sc.mm, 5);
        let keeper = sc.env.funded_keypair();
        let saved = sc.env.svm.clone();
        let plain = liquidation_run(&mut sc, false, &keeper);
        sc.env.svm = saved;
        let flagged = liquidation_run(&mut sc, true, &keeper);
        assert!(
            plain.1.is_some(),
            "the path makes the position liquidatable"
        );
        assert!(!plain.0.is_empty(), "it was healthy first");
        assert_eq!(plain.0, flagged.0, "the same refusals, second by second");
        assert_eq!(plain.1, flagged.1, "the same liquidation, to the lamport");
        assert_eq!(
            (plain.2, flagged.2),
            (0, 0),
            "pool_fail never reads the flag"
        );
        assert_eq!(plain.3, 0);
        assert!(flagged.3 > 0, "only the flagged run has an S3 chain");
    }

    /// ECON §8.6's race under the Emergency flag (O15; ARCH §8.2 round 3 "Emergency-flag S3
    /// raced against an honest expiry"): at the deadline of a flagged, expired, healthy
    /// position whose S3 chain has run `s3_wait_s`, both the honest keeper's `expire()` and a
    /// lender-keeper's `sol_settle(S3)` are valid, and whichever lands first closes the
    /// position (the other then finds no position). The S3 amount is FV with the **expiry fee**
    /// (`p'` on both corners): the lenders get the tokens' executable value in SOL plus the fee
    /// and the rent — what the expiry pays them in tokens and SOL, within the EMA-to-spot
    /// difference — and the borrower loses about `K_s − K_e` more.
    #[test]
    fn an_emergency_flag_s3_races_an_honest_expiry() {
        let mut sc = Sc::default_market();
        sc.env.tick(&sc.mm, 5);
        sc.warp_to_deadline(-700);
        sc.set_route_broken(true);
        for _ in 0..23 {
            sc.env.warp(30);
            sc.crank();
            assert!(sc.settle_ok().settled.is_none());
        }
        // The keeper covers the last seconds before a deadline every second (duty 1), so the
        // expiry has its fresh completed bucket.
        sc.warp_to_deadline(-5);
        sc.env.tick(&sc.mm, 5);
        assert_eq!(sc.env.now(), sc.pos().deadline);
        let pos = sc.pos();
        let honest = sc.env.funded_keypair();
        let saved = sc.env.svm.clone();

        // The honest keeper first: an ordinary expiry, the flag ignored (I10).
        let ix = shortwin_litesvm_tests::engine::expire_ix(
            &sc.mm,
            &honest.pubkey(),
            &sc.b.key(),
            sc.nonce,
        );
        let exp: shortwin::events::Expire = events::one(&ok(sc.env.send(&[ix], &[&honest])));
        assert!(sc.pos_opt().is_none(), "the expiry closed the position");
        assert!(
            sc.settle().is_err(),
            "the lender-keeper's S3 finds no position"
        );

        // The lender-keeper first: S3 at FV with the expiry fee.
        sc.env.svm = saved;
        let ev = sc.settle_ok().settled.expect("S3 at the deadline");
        assert_eq!(ev.condition, 3);
        assert_eq!(
            (ev.pprime_lo, ev.pprime_hi),
            (pos.expiry_fee_bps, pos.expiry_fee_bps),
            "O15: the expiry fee at both corners"
        );
        let ix = shortwin_litesvm_tests::engine::expire_ix(
            &sc.mm,
            &honest.pubkey(),
            &sc.b.key(),
            sc.nonce,
        );
        assert!(
            sc.env.send(&[ix], &[&honest]).is_err(),
            "the honest expiry finds no position"
        );

        // The two outcomes (ECON §8.6's table): the lenders' S3 take is the expiry's buyback,
        // fee and rent in SOL, within the EMA-to-spot difference of the quote ...
        let expiry_value = exp.buyback_cost + exp.expiry_fee + exp.rent;
        let diff = ev.lender_amount.abs_diff(expiry_value);
        assert!(
            diff * 1_000 <= expiry_value,
            "S3 take {} vs the expiry's B + fee + rent {expiry_value}",
            ev.lender_amount
        );
        assert_eq!(ev.lender_amount, ev.fv.min(pos.v - ev.k_s));
        // ... and the borrower loses K_s − K_e more (plus that same difference).
        let extra = i128::from(exp.borrower_payout) - i128::from(ev.borrower_amount);
        let keeper_gap = i128::from(ev.k_s) - i128::from(exp.keeper_reward);
        assert!(keeper_gap > 0, "K_s above K_e");
        assert!(
            (extra - keeper_gap).unsigned_abs() <= u128::from(diff),
            "borrower: expiry {} vs S3 {} (K_s {} / K_e {})",
            exp.borrower_payout,
            ev.borrower_amount,
            ev.k_s,
            exp.keeper_reward
        );
    }
}

mod fee_hike {
    use super::*;

    /// A venue fee above the global hard cap (ARCH §4.3 "two fee views", §5.3; A43, V2-1,
    /// I16): the pool still trades, so it is still observed, but it is `s3_no_route` evidence;
    /// an expired position settles S3 at the timer with FV at the position's locked cap, never
    /// at the live fee (an admin fee hike cannot inflate FV). A healthy position under the hike
    /// never settles.
    #[test]
    fn a_fee_above_the_hard_cap_is_s3_evidence_and_fv_stays_at_the_cap() {
        let mut sc = Sc::default_market();
        sc.env.tick(&sc.mm, 5);
        sc.warp_to_deadline(-700);
        sc.knobs(|k| k.fee_ppm = 60_000);
        for _ in 0..23 {
            sc.env.warp(30);
            sc.crank();
            assert!(
                sc.settle_ok().settled.is_none(),
                "not expired, not liquidatable"
            );
        }
        let m = sc.market();
        assert!(m.no_route_since > 0 && m.pool_fail_since > 0 && m.b_lo.get() > 0);
        let pos = sc.pos();
        sc.warp_to_deadline(0);
        let (_s, ev) = sc.run_until_settled(10).expect("S3");
        assert_eq!(ev.condition, 3);
        let m = sc.market();
        let (p, l) = (m.ema_price.get(), m.ema_liq.get());
        let r_due = rent::due(&pos.pos_rent(), &m.basis()).unwrap();
        let cap_ppm = u64::from(pos.venue_fee_hard_cap_bps) * 100;
        assert_eq!(
            ev.fv,
            fair_value(pos.q, p, l, cap_ppm, ev.pprime_lo, r_due),
            "FV at (P_ema, L_ema) and the locked cap"
        );
        assert_eq!(ev.pprime_lo, pos.expiry_fee_bps);
    }
}

mod not_executable {
    use super::*;

    /// A disabled venue (an admin halt; A64, V2-5): nothing is observed, the S3 chain starts
    /// by itself, closes fail with `NoRoute`, and a liquidatable position settles S3 at the
    /// timer with FV at the **frozen** EMA point — the box collapses to it after `RING_LEN`
    /// seconds without an integration — priced at the halted pool's own fee.
    #[test]
    fn a_disabled_venue_reaches_s3_after_the_timer() {
        let mut sc = Sc::default_market();
        sc.env.tick(&sc.mm, 5);
        make_liquidatable(&mut sc, 0.8);
        let pos = sc.pos();
        sc.knobs(|k| k.disabled = true);
        sc.env.warp(1);
        let out = sc.settle_ok();
        assert!(out.settled.is_none());
        let m0 = sc.market();
        assert_eq!(
            m0.no_route_since,
            sc.env.now(),
            "the chain starts at the first call"
        );
        assert_eq!(m0.b_lo.get(), 0, "nothing recorded");
        assert_err(&sc.env.close(&sc.mm, &sc.b, sc.nonce), E::NoRoute);
        // A gap longer than S3_OBS_GAP_S (60 s) restarts the chain (C0-16).
        sc.env.warp(61);
        sc.crank();
        let since = sc.market().no_route_since;
        assert_eq!(since, sc.env.now());
        sc.crank_every(30, 19);
        assert!(sc.settle_ok().settled.is_none(), "not yet: 570 s of chain");
        let (_s, out) = sc.run_until_settled_out(40).expect("S3");
        eprintln!("sol_settle S3: {} trace entries, {} CU", out.trace, out.cu);
        assert!(out.trace == 2 && out.cu <= 150_000);
        let ev = out.settled.unwrap();
        assert_eq!(ev.condition, 3);
        assert!(sc.env.now() >= since + i64::from(pos.s3_wait_s));
        let m = sc.market();
        let (p, l) = (m.ema_price.get(), m.ema_liq.get());
        assert_eq!(
            (p, l, m.ema_count),
            (m0.ema_price.get(), m0.ema_liq.get(), m0.ema_count),
            "the EMA is frozen while the venue cannot execute"
        );
        assert_eq!(
            (ev.p_lo, ev.p_hi, ev.l_lo, ev.l_hi),
            (p, p, l, l),
            "the EMA point"
        );
        assert!(ev.ema_age_s > 600, "the EMA is as old as the halt");
        let r_due = rent::due(&pos.pos_rent(), &m.basis()).unwrap();
        let fv = fair_value(pos.q, p, l, u64::from(FEE_PPM), pos.penalty_bps, r_due);
        assert_eq!(ev.fv, fv, "FV at the pool's own fee");
        assert_eq!(ev.lender_amount, fv.min(pos.v - k_s(&pos)));
    }

    /// A venue-state layout change (A63, V1-8: `layout_version = 1` moves the reserve counters,
    /// so the pool no longer parses): not executable — the crank and `sol_settle` succeed,
    /// record nothing, and start the chain; S3 settles at the timer, and with no readable fee
    /// FV is priced at the position's hard cap (`min(φ_live, cap)` with `φ_live` unknown).
    #[test]
    fn a_layout_change_is_not_executable_and_settles_s3_at_the_cap_fee() {
        let mut sc = Sc::default_market();
        sc.env.tick(&sc.mm, 5);
        make_liquidatable(&mut sc, 0.8);
        let pos = sc.pos();
        sc.knobs(|k| k.layout_version = 1);
        sc.env.warp(1);
        sc.crank();
        let m = sc.market();
        assert_eq!((m.no_route_since, m.b_lo.get()), (sc.env.now(), 0));
        assert!(sc.settle_ok().settled.is_none());
        sc.crank_every(30, 19);
        let (_s, ev) = sc.run_until_settled(40).expect("S3");
        assert_eq!(ev.condition, 3);
        let m = sc.market();
        let (p, l) = (m.ema_price.get(), m.ema_liq.get());
        let r_due = rent::due(&pos.pos_rent(), &m.basis()).unwrap();
        let cap_ppm = u64::from(pos.venue_fee_hard_cap_bps) * 100;
        let fv = fair_value(pos.q, p, l, cap_ppm, pos.penalty_bps, r_due);
        assert_eq!(ev.fv, fv, "FV at the hard cap");
        assert_eq!(ev.lender_amount, fv.min(pos.v - k_s(&pos)));
    }

    /// V2-5 (disclosed): a healthy position on a halted venue stays frozen — no S3 (it is
    /// neither liquidatable nor expired; the frozen EMA cannot move), no close — until the venue
    /// returns; then the chain clears and everything works again.
    #[test]
    fn a_healthy_position_on_a_halted_venue_stays_frozen() {
        let mut sc = Sc::default_market();
        sc.env.tick(&sc.mm, 5);
        sc.knobs(|k| k.disabled = true);
        sc.crank_every(30, 25);
        let out = sc.settle_ok();
        assert!(out.settled.is_none() && out.armed.is_none());
        assert!(sc.env.now() - sc.market().no_route_since >= 700);
        assert_err(&sc.env.close(&sc.mm, &sc.b, sc.nonce), E::NoRoute);
        sc.knobs(|k| k.disabled = false);
        sc.env.warp(1);
        sc.crank();
        assert_eq!(
            sc.market().no_route_since,
            0,
            "an observation with a route clears it"
        );
        sc.env.warp(1);
        close_ok(&mut sc.env, &sc.mm, &sc.b, sc.nonce);
    }
}
