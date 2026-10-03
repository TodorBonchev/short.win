//! `open_short` → `close_short` on the real pump program (ARCH §2.4.1 PumpCurve row, §3.4, §3.5):
//! every lamport and token accounted for on Token-2022 curves of 125, 115 and 151 bytes and a
//! legacy SPL-Token curve of 115 bytes without a creator (creator fee 0).
//!
//! Each round trip asserts: `S` = `quote_sell(q)` on the pre-trade state exactly (the creator
//! vaults of the dumped coins are funded, so pump takes no venue-side rent); the lending vault
//! loses exactly `q`, the curve's base ATA gains it, and the curve's lamports fall by the gross
//! `g`; the borrower pays the position's and the exposure record's rent, `M` and the open fee, and
//! gets its `ert` back (it funds the trader for the leg); `trader` and `trader_base_ata` hold
//! nothing; `N_open` = `B̂` on the post-open state. At close: the venue charges exactly pump's
//! `curve_charge_exact_out(q)` (≤ short.win's quote `B̂_live(q)`, by at most `1 + #fees`), the
//! lending vault receives exactly `q` through the trader's base ATA, the curve gains the pre-fee
//! `n`, the waterfall pays `B + R` and the borrower the rest.

use shortwin::state::WalletExposure;

use crate::common::*;

/// Balances around one instruction.
#[derive(Clone, Copy, Debug)]
struct Snap {
    pool_vault: u64,
    curve_tokens: u64,
    curve: u64,
    borrower: u64,
    market_vault: u64,
}

fn snap(env: &Env, pm: &PumpMarket, borrower: &Pubkey) -> Snap {
    Snap {
        pool_vault: tokens(env, &pm.keys.pool_vault),
        curve_tokens: tokens(env, &pm.curve.curve_base_ata),
        curve: lamports(env, &pm.curve.curve),
        borrower: lamports(env, borrower),
        market_vault: lamports(env, &pm.keys.market_vault),
    }
}

/// One exact open → close round trip of `q` on `pm` (module docs). Returns `(S, B)`.
pub fn round_trip(env: &mut Env, pm: &PumpMarket, q: u64) -> (u64, u64) {
    let mut b = env.borrower();
    let s0 = snap(env, pm, &b.key());
    let sale = pm.curve.quote_sell(&env.svm, q).expect("sale quote");
    let vq0 = pm.curve.state(&env.svm).virtual_quote_reserves;
    let gross = vq0 - u64::try_from(sale.sol_after).unwrap();
    let first_exposure =
        read::account(&env.svm, &pda::exposure(&pm.market(), &b.key()).0).is_none();
    let (open, nonce, _) = open_ok(env, pm, &mut b, q);
    let s1 = snap(env, pm, &b.key());
    assert_eq!(
        open.s, sale.proceeds,
        "S = the sale quote on the pre-trade state"
    );
    assert_eq!(
        s0.pool_vault - s1.pool_vault,
        q,
        "the lending vault spent exactly q"
    );
    assert_eq!(s1.curve_tokens - s0.curve_tokens, q, "the curve received q");
    assert_eq!(s0.curve - s1.curve, gross, "the curve paid the gross g");
    assert_trader_clean(env, pm);
    let n_open = pm
        .curve
        .quote_buy(&env.svm, q)
        .expect("post-open buy quote")
        .cost;
    assert_eq!(open.n_open, n_open, "N_open = B̂ on the post-open state");
    assert_eq!(open.venue, VenueKind::PumpCurve as u8);
    assert_eq!(
        open.tier,
        shortwin_math::tier::Tier::D as u8,
        "the curve is Tier D"
    );
    let pos_key = pda::position(&pm.market(), &b.key(), nonce).0;
    let pos: ShortPosition = read::borsh(&env.svm, &pos_key).expect("position");
    let cfg = env.config().params;
    assert_eq!(
        pos.ert, cfg.exec_rent_tol,
        "a curve position locks ert = exec_rent_tol"
    );
    let pos_rent = env
        .svm
        .minimum_balance_for_rent_exemption(ShortPosition::SPACE);
    assert_eq!(
        lamports(env, &pos_key),
        pos_rent + pos.v,
        "position = rent + V"
    );
    let exp_rent = if first_exposure {
        env.svm
            .minimum_balance_for_rent_exemption(WalletExposure::SPACE)
    } else {
        0
    };
    // The borrower funded ert and got it back: it pays only the accounts, M and the fee.
    assert_eq!(
        s0.borrower - s1.borrower,
        pos_rent + exp_rent + open.m + open.fee_open,
        "the borrower paid the accounts, M and the open fee"
    );
    assert_eq!(
        s1.market_vault - s0.market_vault,
        open.fee_open,
        "the open fee went to the market vault"
    );

    // ---- close ----
    let quote = pm.curve.quote_buy(&env.svm, q).expect("buy quote");
    let charge = pm.curve.charge_buy(&env.svm, q).expect("pump's charge");
    let (close, _) = close_ok(env, pm, &b, nonce);
    let s2 = snap(env, pm, &b.key());
    assert_eq!(close.b_live, quote.cost, "the leg is bounded by B̂_live(q)");
    assert_eq!(close.buyback_cost, charge.cost, "B = pump's charge");
    assert!(quote.cost >= charge.cost && quote.cost - charge.cost <= 3);
    assert_eq!(
        s2.pool_vault - s1.pool_vault,
        q,
        "the lending vault got q back"
    );
    assert_eq!(s1.curve_tokens - s2.curve_tokens, q, "the curve sold q");
    assert_eq!(s2.curve - s1.curve, charge.net, "the curve gained n");
    assert_eq!(close.token_surplus, 0);
    assert_trader_clean(env, pm);
    assert!(
        read::account(&env.svm, &pos_key).is_none(),
        "position closed"
    );
    (open.s, close.buyback_cost)
}

/// A Token-2022 curve of 115 bytes (the first v2 trade — the listing's warm-up already — grows it
/// to 141 bytes since the 2026-10-02 redeploy, 125 before; its rent reserve already covers that,
/// so no top-up).
#[test]
fn round_trip_t22_115() {
    let (mut env, pm, q) = setup(PumpCoin::T22_115);
    round_trip(&mut env, &pm, q);
    assert_eq!(
        pm.curve.data_len(&env.svm),
        fork::CURVE_LEN_2026_10,
        "the curve grew to 141 bytes"
    );
}

/// A legacy SPL-Token curve of 115 bytes without a creator: no creator fee, no creator vault.
#[test]
fn round_trip_legacy_spl_without_creator() {
    let (mut env, pm, q) = setup(PumpCoin::Legacy115);
    assert_eq!(pm.curve.token_program, TokenProgram::Spl);
    let fees = pm.curve.fees(&env.svm).unwrap();
    assert_eq!(fees.creator, 0, "a default creator pays no creator fee");
    round_trip(&mut env, &pm, q);
}

/// A fresh `create_v2` curve (125 bytes, Token-2022) after 5 SOL of buys (which grow it to 141
/// bytes since the 2026-10-02 redeploy).
#[test]
fn round_trip_fresh_125() {
    let (mut env, pm, q) = setup(PumpCoin::Fresh);
    assert_eq!(pm.curve.data_len(&env.svm), fork::CURVE_LEN_2026_10);
    round_trip(&mut env, &pm, q);
}

/// The near-complete 151-byte curve, twice (the second open reuses the exposure record... after
/// the first closed it; each round trip is exact on its own pre-state).
#[test]
fn round_trip_near_151_twice() {
    let (mut env, pm, q) = setup(PumpCoin::Near);
    assert_eq!(pm.curve.data_len(&env.svm), 151);
    round_trip(&mut env, &pm, q);
    env.warp(3);
    round_trip(&mut env, &pm, q / 2);
}
