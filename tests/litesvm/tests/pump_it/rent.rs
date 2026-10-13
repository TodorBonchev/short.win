//! T-RENT, curve part (ARCH §2.4 step 3 "Exact funding", §8.2; C1-4, C1-9; R-PUMP U6, Update
//! 2026-09-29): the venue-side rent pump takes from `user` — the trader PDA — stays within
//! `exec_rent_tol` and is borne as designed, on the real pump ELF:
//!
//! - **sells**: an empty creator vault is topped up to `Rent.minimum_balance(0)` (650,240 under
//!   mainnet's Rent) out of the trader's `ert` **before** the proceeds are credited, so
//!   `S = trader.lamports_after − ert` is the sale's proceeds less the top-up (the rent cannot
//!   be separated from the proceeds on the curve, ARCH §2.4 step 3); the borrower is refunded
//!   exactly `ert`, bears the top-up through a smaller `S`, and a sale small enough that the
//!   top-up exceeds the `exec_tol_bps` tolerance on `S_exp` is refused (`ProceedsBelowMin`);
//! - **buys**: the same top-up comes out of the leg's funding (`max_in + ert`), so `B` = pump's
//!   charge + 650,240, within the bound;
//! - the pump UVA was created at listing (`create_market`), so no leg pays its 1,346,200;
//! - **growth** (pump's 2026-10-08 redeploy; 141 bytes on the 2026-10-02 one): a curve under 166
//!   bytes grows to 166 on short.win's first v2 trade, its rent reserve (`lamports − rQ −` the fee
//!   buckets, `fork::curve_growth_top_up`) topped up to `Rent.minimum_balance(166)` from the
//!   trader the same way — out of `S` on a sale (125 bytes before the 2026-10-02 redeploy, which
//!   no curve of the dump needed: they are over-funded; the 151-byte `near` curve's reserve is
//!   exactly `minimum_balance(151)`, so it pays 76,200).
//!
//! The empty creator vault is planted on pump's side (the vault PDA's lamports removed), as
//! `pump_math_it::an_empty_creator_vault_is_topped_up_by_the_user` does for a normal user.
//! Every instruction goes through `Env::send`: the invariant hooks run after each one.

use solana_account::Account;

use crate::common::*;

/// Mainnet's `Rent.minimum_balance(0)` (the `sysvar_rent` fixture).
const TOP_UP: u64 = 650_240;

fn empty_creator_vault(env: &mut Env, pm: &PumpMarket) {
    env.svm
        .set_account(pm.creator_vault, Account::default())
        .unwrap();
    assert_eq!(lamports(env, &pm.creator_vault), 0);
}

/// A sale into an empty creator vault: `S` = the quote less the top-up, `ert` back to the
/// borrower, the vault rent-exempt afterwards, the trader clean.
#[test]
fn a_sale_pays_the_creator_vault_top_up_out_of_its_proceeds() {
    let (mut env, pm, _) = setup(PumpCoin::Near);
    assert_eq!(env.svm.minimum_balance_for_rent_exemption(0), TOP_UP);
    let vt = pm.curve.state(&env.svm).virtual_token_reserves;
    // Large enough that the top-up fits the 10 bps tolerance on S_exp (S_exp ≥ 650.24 M).
    let q = vt / 100;
    let sale = pm.curve.quote_sell(&env.svm, q).unwrap();
    assert!(sale.proceeds > 700_000_000, "S_exp {}", sale.proceeds);
    let fee = pm.curve.fees(&env.svm).unwrap();
    assert!(fee.creator > 0, "a creator fee: pump tops the vault up");
    empty_creator_vault(&mut env, &pm);
    // The 151-byte curve's growth to 166 (the 2026-10-08 build) comes out of `S` the same way.
    let growth = pm.curve.growth_top_up(&env.svm);
    assert_eq!(growth, 76_200);
    let mut b = env.borrower();
    let b0 = lamports(&env, &b.key());
    let (open, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    assert_eq!(
        open.s,
        sale.proceeds - TOP_UP - growth,
        "S = proceeds − the top-up − the growth"
    );
    assert!(
        lamports(&env, &pm.creator_vault) >= TOP_UP,
        "the vault is rent-exempt"
    );
    assert_trader_clean(&env, &pm);
    // The borrower got ert back: it paid the accounts, M and the fee only.
    let pos_key = pda::position(&pm.market(), &b.key(), nonce).0;
    let pos_rent = env
        .svm
        .minimum_balance_for_rent_exemption(ShortPosition::SPACE);
    let exp_rent = env
        .svm
        .minimum_balance_for_rent_exemption(shortwin::state::WalletExposure::SPACE);
    assert_eq!(
        b0 - lamports(&env, &b.key()),
        pos_rent + exp_rent + open.m + open.fee_open,
        "ert refunded exactly"
    );
    assert_eq!(lamports(&env, &pos_key), pos_rent + open.s + open.m);
}

/// A small sale into an empty creator vault: the top-up exceeds `S_exp·exec_tol_bps/10⁴`, so
/// the net `S` misses `min_out` and the open is refused (`ProceedsBelowMin`) — pump's own
/// check on the gross credit passed (ARCH §2.4 step 3: short.win requires the **net**).
#[test]
fn a_small_sale_into_an_empty_vault_is_refused() {
    let (mut env, pm, q) = setup(PumpCoin::Near);
    let sale = pm.curve.quote_sell(&env.svm, q).unwrap();
    let tol = env.config().params.exec_tol_bps;
    assert!(
        u128::from(TOP_UP) * 10_000 > u128::from(sale.proceeds) * u128::from(tol),
        "the top-up exceeds the tolerance"
    );
    empty_creator_vault(&mut env, &pm);
    let mut b = env.borrower();
    let (res, _) = env.open(&pm, &mut b, q);
    assert_err(&res, E::ProceedsBelowMin);
    assert_eq!(lamports(&env, &pm.creator_vault), 0, "reverted");
}

/// A buyback into an empty creator vault: pump takes the top-up out of the leg's funding, so
/// `B` = pump's charge + 650,240 (≤ `max_in + ert`); the lending vault gets exactly `q`.
#[test]
fn a_buyback_pays_the_creator_vault_top_up() {
    let (mut env, pm, q) = setup(PumpCoin::T22_115);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    env.warp(1);
    empty_creator_vault(&mut env, &pm);
    let charge = pm.curve.charge_buy(&env.svm, q).unwrap();
    let quote = pm.curve.quote_buy(&env.svm, q).unwrap();
    let ert = env.config().params.exec_rent_tol;
    assert!(
        charge.cost + TOP_UP <= quote.cost + ert,
        "within the leg's funding"
    );
    let vault0 = tokens(&env, &pm.keys.pool_vault);
    let (close, _) = close_ok(&mut env, &pm, &b, nonce);
    assert_eq!(
        close.buyback_cost,
        charge.cost + TOP_UP,
        "B = charge + top-up"
    );
    assert_eq!(tokens(&env, &pm.keys.pool_vault) - vault0, q);
    assert!(lamports(&env, &pm.creator_vault) >= TOP_UP);
    assert_trader_clean(&env, &pm);
}

/// The trader's pump UVA exists from the listing: a first open and close on a fresh market
/// leave it as it was (no leg pays UVA rent), and it accumulates for the trader.
#[test]
fn the_uva_created_at_listing_is_never_paid_by_a_leg() {
    let (mut env, pm, q) = setup(PumpCoin::Fresh);
    let uva = fork::pump_user_volume_accumulator(&pm.trader());
    let l0 = lamports(&env, &uva);
    assert!(l0 > 0, "created at listing");
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    env.warp(1);
    close_ok(&mut env, &pm, &b, nonce);
    assert_eq!(lamports(&env, &uva), l0, "no rent moved");
}

/// A sale on a 115-byte curve whose rent reserve is planted down to `minimum_balance(115)` (the
/// dump's is over-funded at the old rent): pump's 2026-10-08 redeploy grows it to 166 bytes (141
/// on the 2026-10-02 build) and tops the reserve up to `minimum_balance(166)` — 259,080 lamports
/// (132,080 to 141) — from the trader before the proceeds, so `S` = the quote less the growth;
/// the curve is 166 bytes after it, the borrower got `ert` back, the trader is clean.
#[test]
fn a_sale_pays_a_short_curves_growth_out_of_its_proceeds() {
    let (mut env, pm, _) = setup(PumpCoin::T22_115);
    assert_eq!(pm.curve.data_len(&env.svm), 115);
    let grows_to = fork::PumpDeploy::CURRENT.curve_len();
    let min115 = env.svm.minimum_balance_for_rent_exemption(115);
    let min_new = env.svm.minimum_balance_for_rent_exemption(grows_to);
    let growth = min_new - min115;
    assert_eq!(growth, 259_080);
    let rq = pm.curve.state(&env.svm).real_quote_reserves;
    let mut acc = env.svm.get_account(&pm.curve.curve).unwrap();
    acc.lamports = rq + min115;
    env.svm.set_account(pm.curve.curve, acc).unwrap();
    let vt = pm.curve.state(&env.svm).virtual_token_reserves;
    // Large enough that the growth fits the 10 bps tolerance on S_exp.
    let q = vt / 100;
    let sale = pm.curve.quote_sell(&env.svm, q).unwrap();
    let tol = u64::from(env.config().params.exec_tol_bps);
    assert!(
        sale.proceeds * tol / 10_000 >= growth,
        "S_exp {}",
        sale.proceeds
    );
    let mut b = env.borrower();
    let (open, _, _) = open_ok(&mut env, &pm, &mut b, q);
    assert_eq!(open.s, sale.proceeds - growth, "S = proceeds − the growth");
    let acc = env.svm.get_account(&pm.curve.curve).unwrap();
    assert_eq!(acc.data.len(), grows_to);
    assert_eq!(
        acc.lamports - pm.curve.state(&env.svm).real_quote_reserves,
        min_new
    );
    assert_trader_clean(&env, &pm);
}
