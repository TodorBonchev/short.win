//! Scenario contributed by Defriza Crypto (defrizacrypto@gmail.com), 2026-10-29, kept as a
//! regression (SW-H2).
//!
//! SW-H2 measurement (external pre-beta audit, HEAD 3776f9e; scenario authored
//! 2026-10-29 for this audit, not part of the repo's suite).
//!
//! Hypothesis: a `None` market holding open positions is re-detected by a
//! permissionless `crank_price` onto PumpCurve/PumpSwap with NO carried-fit
//! check (unlike `propose_pool`/`execute_pool_switch`, which refuse a switch
//! the carried positions do not fit). The re-detection rewrites the market's
//! liquidity, restarts its observation and clears the route chains and the
//! collection window. Question measured here: does the rewrite itself harm
//! the carried position (e.g. make it liquidatable at a price it was healthy
//! at, or change its locked terms)?
//!
//! Construction (the only one the program admits): a curve market with a
//! position open completes and migrates ELSEWHERE, so a crank moves it to
//! `None` with `graduated_elsewhere` and the position still open. The
//! re-detection branch is then reached by restoring the completed curve as a
//! live curve — the same planted state the repo's own
//! `a_none_to_pump_redetection_clears_the_collection_window` uses, whose
//! docstring states this is "a state mainnet cannot reach — a migration is
//! final". The PumpSwap branch is not separately planted: a canonical pool
//! (creator = pump's `pool-authority` PDA, index 0) can only be created by
//! pump's own migration consuming the curve's reserves, which is exactly the
//! migration that already happened elsewhere; there is no second migration.

use shortwin::state::MARKET_FLAG_GRADUATED_ELSEWHERE;
use shortwin_litesvm_tests::engine::liquidate_ix;

use crate::common::*;
use crate::graduation::{graduated_elsewhere_with_a_position, migrate_elsewhere};

#[test]
fn sw_h2_redetect_with_a_carried_position_measures_the_rewrite() {
    // None with one position open (graduated elsewhere).
    let (mut env, pm, b, nonce, q) = graduated_elsewhere_with_a_position();
    // Save the completed curve (reserves intact) before the elsewhere
    // migration zeroes it, for the planted resurrection below.
    let saved_curve = env.svm.get_account(&pm.curve.curve).unwrap();
    migrate_elsewhere(&mut env, &pm);
    let m_none = market(&env, &pm);
    assert_eq!(m_none.venue_kind(), Some(VenueKind::None));
    assert!(m_none.has_flag(MARKET_FLAG_GRADUATED_ELSEWHERE));
    assert_eq!(m_none.open_positions, 1);
    let pos_key = pda::position(&pm.market(), &b.key(), nonce).0;
    let pos_bytes_none = env.svm.get_account(&pos_key).unwrap().data.clone();
    let pos_none: ShortPosition = env.position(&pm, &b.key(), nonce);
    println!(
        "SW-H2 None state: q {q}, pos.v {}, ema_price {}, ema_liq {}, ring {}, no_route_since {}",
        pos_none.v,
        m_none.ema_price.get(),
        m_none.ema_liq.get(),
        m_none.ring_count,
        m_none.no_route_since,
    );

    // Plant the completed curve live again (mainnet-unreachable; see header).
    env.svm.set_account(pm.curve.curve, saved_curve).unwrap();
    fork::set_curve_complete(&mut env.svm, &pm.curve.curve, false);
    let s = pm.curve.state(&env.svm);
    fork::set_curve_reserves(
        &mut env.svm,
        &pm.curve.curve,
        s.virtual_token_reserves + 3 * q,
        s.virtual_quote_reserves,
        3 * q,
        s.real_quote_reserves,
    );
    let base_ata = pm.curve.curve_base_ata;
    let held = tokens(&env, &base_ata);
    fork::set_token_amount(&mut env.svm, &base_ata, held + 3 * q);
    env.warp(1);

    // The permissionless re-detection itself.
    let meta = crank(&mut env, &AsNone(&pm));
    venue_changed(&meta, VenueKind::None, VenueKind::PumpCurve);
    let m_re = market(&env, &pm);
    println!(
        "SW-H2 after re-detection: ema_price {} (was {}), ema_liq {} (was {}), ring {} (was {}), \
         ema_warm {}, no_route_since {} (was {}), open_positions {}",
        m_re.ema_price.get(),
        m_none.ema_price.get(),
        m_re.ema_liq.get(),
        m_none.ema_liq.get(),
        m_re.ring_count,
        m_none.ring_count,
        m_re.ema_warm,
        m_re.no_route_since,
        m_none.no_route_since,
        m_re.open_positions,
    );
    // The rewrite does not touch the carried position's locked terms.
    assert_eq!(
        env.svm.get_account(&pos_key).unwrap().data,
        pos_bytes_none,
        "the position is carried byte for byte across the re-detection"
    );
    assert_eq!(m_re.open_positions, 1);

    // Warm the EMA back up at the restored curve's own price.
    for _ in 0..40 {
        if market(&env, &pm).ema_warm == 0 {
            break;
        }
        env.warp(1);
        crank(&mut env, &pm);
    }
    let m_warm = market(&env, &pm);
    assert_eq!(m_warm.ema_warm, 0, "the warm-up completes");
    println!(
        "SW-H2 after warm-up: ema_price {}, ema_liq {}",
        m_warm.ema_price.get(),
        m_warm.ema_liq.get()
    );

    // Liquidation probe at the restored price: the position was healthy when
    // the market left the curve; the rewrite alone must not liquidate it.
    let kp = keeper(&mut env);
    let liq = liquidate_ix(&pm, &kp.pubkey(), &b.key(), nonce);
    let r = env.send(&[liq], &[&kp]);
    match &r {
        Ok(_) => println!("SW-H2 liquidate after re-detection: SUCCEEDED (harm signal)"),
        Err(e) => println!("SW-H2 liquidate after re-detection: refused, {:?}", e.err),
    }
    assert!(
        r.is_err(),
        "the rewrite alone must not make the carried position liquidatable"
    );
    assert_eq!(
        env.svm.get_account(&pos_key).unwrap().data,
        pos_bytes_none,
        "the position survives the liquidation probe untouched"
    );

    // The borrower exit works on the re-detected venue.
    env.warp(1);
    crank(&mut env, &pm);
    let (c, _) = close_ok(&mut env, &pm, &b, nonce);
    assert_eq!(c.q, q);
    println!(
        "SW-H2 close after re-detection: q {}, buyback {} lamports; the carried position exits whole",
        c.q, c.buyback_cost
    );
}
