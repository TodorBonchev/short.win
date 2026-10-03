//! I14 on the curve (ARCH §3.0 A6, §3.12 PumpCurve, §8.3; C0-2, C0-14): every account of the
//! curve's ReadSet (C0–C6) and TradeExtras (T0–T19) is present and key-checked in `open_short`,
//! `close_short`, `crank_price` and `sol_settle` — each slot omitted, replaced by a foreign key,
//! a trailing extra and the first two slots swapped make the instruction fail with
//! `BadRemainingAccounts` / `VenueAccountMismatch`, never "no route" or a silent default; then
//! the recipient slots take any key of the live `Global` lists, and the creator-vault slots
//! follow the **live** curve creator (a creator change after the listing: the stale cached vault
//! is refused, the re-derived one accepted). M10 batch 3: the bonding curve and its base / quote
//! ATAs on every extras site and the position's borrower on the keeper exits are `DeniedAccount`
//! extras, with a fresh-key control.
//!
//! Every instruction goes through `Env::send`: the invariant hooks run after each one.

use shortwin_litesvm_tests::engine::with_cu_limit;

use crate::common::*;

fn variants(ix: &Instruction, remaining: usize) -> Vec<(String, Instruction)> {
    let start = ix.accounts.len() - remaining;
    let mut out = Vec::new();
    for i in 0..remaining {
        let mut omit = ix.clone();
        omit.accounts.remove(start + i);
        out.push((format!("omit slot {i}"), omit));
        let mut swap = ix.clone();
        let m = &mut swap.accounts[start + i];
        *m = AccountMeta {
            pubkey: Pubkey::new_unique(),
            is_signer: false,
            is_writable: m.is_writable,
        };
        out.push((format!("replace slot {i}"), swap));
    }
    let mut extra = ix.clone();
    extra
        .accounts
        .push(AccountMeta::new_readonly(Pubkey::new_unique(), false));
    out.push(("extra trailing account".to_string(), extra));
    let mut order = ix.clone();
    order.accounts.swap(start, start + 1);
    out.push(("slots 0 and 1 swapped".to_string(), order));
    out
}

fn assert_all_fail(
    env: &mut Env,
    ix: &Instruction,
    remaining: usize,
    signers: &[&solana_keypair::Keypair],
) {
    for (what, bad) in variants(ix, remaining) {
        let res = env.send(&[bad], signers);
        let got = custom_code(&res);
        assert!(
            matches!(
                got,
                Some(c) if c == code(E::BadRemainingAccounts) || c == code(E::VenueAccountMismatch)
            ),
            "{what}: expected BadRemainingAccounts / VenueAccountMismatch, got {got:?}"
        );
    }
}

/// Every slot of the four instructions (module docs).
#[test]
fn open_close_crank_and_settle_check_every_curve_slot() {
    let (mut env, pm, q) = setup(PumpCoin::T22_115);
    let b = env.borrower();
    let open = open_short_ix(
        &pm,
        &b.key(),
        0,
        q,
        0,
        u64::MAX,
        u64::MAX,
        ExtrasSel::NONE,
        &[],
    );
    assert_all_fail(&mut env, &open, 27, &[&b.kp]);
    ok(env.send(&with_cu_limit(&pm, open), &[&b.kp]));
    env.warp(1);
    let crank_ix = crank_price_ix(&pm);
    assert_all_fail(&mut env, &crank_ix, 7, &[]);
    let kp = keeper(&mut env);
    let settle_ix = sol_settle_ix(&env, &pm, &kp.pubkey(), &b.key(), 0);
    assert_all_fail(&mut env, &settle_ix, 7, &[&kp]);
    let close = close_short_ix(&pm, &b.key(), 0, u64::MAX, ExtrasSel::NONE, &[]);
    assert_all_fail(&mut env, &close, 27, &[&b.kp]);
    ok(env.send(&[crank_ix], &[]));
    ok(env.send(&[settle_ix], &[&kp]));
    ok(env.send(&with_cu_limit(&pm, close), &[&b.kp]));
}

/// The recipient slots (T6, T8 and their derived WSOL ATAs T7, T9) take any key of the live
/// `Global` lists: an open with the last fee recipient and the third buyback recipient, a close
/// with others, both exact.
#[test]
fn any_listed_recipient_is_accepted() {
    let (mut env, mut pm, q) = setup(PumpCoin::Legacy115);
    pm.use_recipients(&env.svm, 7, 2);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    pm.use_recipients(&env.svm, 4, 6);
    env.warp(1);
    close_ok(&mut env, &pm, &b, nonce);
    assert_trader_clean(&env, &pm);
}

/// A curve creator change after the listing (the creator vault moves): the market's cached
/// vault (T12, T13) is refused, the vault re-derived from the live creator is accepted — on
/// the open and on the close (pump itself checks the vault against the live creator).
#[test]
fn the_creator_vault_follows_the_live_creator() {
    let (mut env, mut pm, q) = setup(PumpCoin::T22_115);
    let stale = pm.creator_vault;
    let new_creator = Pubkey::new_unique();
    fork::set_curve_creator(&mut env.svm, &pm.curve.curve, &new_creator);
    let mut b = env.borrower();
    let (res, _) = env.open(&pm, &mut b, q);
    assert_err(&res, E::VenueAccountMismatch);
    pm.refresh_creator(&env.svm);
    assert_ne!(pm.creator_vault, stale);
    assert_eq!(pm.creator_vault, fork::pump_creator_vault(&new_creator));
    // The new creator's vault holds its rent-exempt minimum (an empty one costs the sale pump's
    // top-up: `rent`).
    let min0 = env.svm.minimum_balance_for_rent_exemption(0);
    env.fund(&pm.creator_vault, min0);
    // Re-deriving both vault keys lifts the open just above the 200 k default: within ARCH
    // §6.1's PROPOSED 250 k limit for curve opens and closes.
    let open = open_short_ix(
        &pm,
        &b.key(),
        0,
        q,
        0,
        u64::MAX,
        u64::MAX,
        ExtrasSel::NONE,
        &[],
    );
    let meta = ok(env.send(&[pump::cu_limit_ix(pump::CURVE_TRADE_CU), open], &[&b.kp]));
    eprintln!(
        "open after a creator change: {} CU",
        meta.compute_units_consumed
    );
    env.warp(1);
    let close = close_short_ix(&pm, &b.key(), 0, u64::MAX, ExtrasSel::NONE, &[]);
    let meta = ok(env.send(&[pump::cu_limit_ix(pump::CURVE_TRADE_CU), close], &[&b.kp]));
    eprintln!(
        "close after a creator change: {} CU",
        meta.compute_units_consumed
    );
    assert_trader_clean(&env, &pm);
}

/// I14 on the curve's keeper exits (M10 G-A1b): every ReadSet (C0–C6) and TradeExtras (T0–T19)
/// slot of `liquidate` and `expire` omitted, replaced by a foreign key, a trailing extra and the
/// first two slots swapped fail with `BadRemainingAccounts` / `VenueAccountMismatch` and leave the
/// position as it was; the controls: the honest `liquidate` of the healthy position gets past the
/// accounts and is refused by its body (`NotLiquidatable`), and the honest `expire` after the
/// deadline lands (pump's charge, `q` back in the lending vault, the trader clean).
#[test]
fn liquidate_and_expire_check_every_curve_slot() {
    use shortwin_litesvm_tests::engine::{expire_ix, liquidate_ix};
    let (mut env, pm, q) = setup(PumpCoin::T22_115);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    env.warp(1);
    crank(&mut env, &pm);
    let kp = keeper(&mut env);
    let position = pda::position(&pm.market(), &b.key(), nonce).0;
    let before = env.svm.get_account(&position).map(|a| a.data);
    let liq = liquidate_ix(&pm, &kp.pubkey(), &b.key(), nonce);
    assert_all_fail(&mut env, &liq, 27, &[&kp]);
    assert_eq!(env.svm.get_account(&position).map(|a| a.data), before);
    let cu = pump::cu_limit_ix(pump::CURVE_KEEPER_EXIT_CU);
    assert_err(&env.send(&[cu.clone(), liq], &[&kp]), E::NotLiquidatable);

    warp_to_deadline(&mut env, &pm, &b.key(), nonce, 1);
    crank(&mut env, &pm);
    env.warp(1);
    let exp = expire_ix(&pm, &kp.pubkey(), &b.key(), nonce);
    let before = env.svm.get_account(&position).map(|a| a.data);
    assert_all_fail(&mut env, &exp, 27, &[&kp]);
    assert_eq!(env.svm.get_account(&position).map(|a| a.data), before);
    let charge = pm.curve.charge_buy(&env.svm, q).unwrap();
    let vault0 = tokens(&env, &pm.keys.pool_vault);
    let meta = ok(env.send(&[cu, exp], &[&kp]));
    let ev: shortwin::events::Expire = events::one(&meta);
    assert_eq!(ev.buyback_cost, charge.cost);
    assert_eq!(tokens(&env, &pm.keys.pool_vault) - vault0, q);
    assert_trader_clean(&env, &pm);
}

// ---- M10 WP-8a: extras on the curve's sell leg (G-H2) -------------------------------------------

/// The Emergency multisig publishes `entries` as the curve's sell-leg `current` extras set.
fn publish_curve_sell_extras(env: &mut Env, entries: Vec<shortwin::ExtraEntryArg>) {
    let ea = env.pause_authority.insecure_clone();
    let ix = shortwin_litesvm_tests::engine::set_venue_extras_ix(
        &ea.pubkey(),
        VenueKind::PumpCurve as u8,
        shortwin::state::EXTRAS_IX_SELL as u8,
        entries,
    );
    ok(env.send(&[ix], &[&ea]));
}

/// A fixed read-only extras entry.
fn fixed_extra(key: Pubkey) -> shortwin::ExtraEntryArg {
    shortwin::ExtraEntryArg {
        kind: shortwin::state::ENTRY_FIXED,
        writable: false,
        key,
        seeds: Vec::<shortwin::SeedPartArg>::new(),
    }
}

/// The transaction of an open of `q` by `b` (nonce 0) with extras selection `set` and `extras`
/// after the curve's accounts; the open is its last instruction.
fn open_with_extras_ixs(
    pm: &PumpMarket,
    b: &Borrower,
    q: u64,
    set: u8,
    extras: &[AccountMeta],
) -> Vec<Instruction> {
    let sel = ExtrasSel { set, bumps: [0; 4] };
    let ix = open_short_ix(pm, &b.key(), 0, q, 0, u64::MAX, u64::MAX, sel, extras);
    with_cu_limit(pm, ix)
}

/// An open of `q` by `b` (nonce 0) with extras selection `set` and `extras` after the curve's
/// accounts.
fn open_with_extras(
    env: &mut Env,
    pm: &PumpMarket,
    b: &Borrower,
    q: u64,
    set: u8,
    extras: &[AccountMeta],
) -> TxResult {
    env.send(&open_with_extras_ixs(pm, b, q, set, extras), &[&b.kp])
}

/// The account list of the one pump CPI at height 2 (short.win's `sell_v2` / `buy_v2`) inside
/// the last instruction of `ixs`, a transaction the harness payer sent.
fn curve_leg_cpi_accounts(
    env: &Env,
    ixs: &[Instruction],
    meta: &TransactionMetadata,
) -> Vec<Pubkey> {
    use shortwin::constants::PUMP_PROGRAM_ID;
    let keys = solana_message::Message::new(ixs, Some(&env.payer.pubkey())).account_keys;
    let calls: Vec<Vec<Pubkey>> = meta.inner_instructions[ixs.len() - 1]
        .iter()
        .filter(|i| {
            i.stack_height == 2
                && keys[usize::from(i.instruction.program_id_index)] == PUMP_PROGRAM_ID
        })
        .map(|i| {
            i.instruction
                .accounts
                .iter()
                .map(|a| keys[usize::from(*a)])
                .collect()
        })
        .collect();
    assert_eq!(calls.len(), 1, "one pump call at height 2");
    calls.into_iter().next().unwrap()
}

/// G-H2 on the curve (A41, V2-6; C0-4 a): the Emergency publishes a harmless fixed extra on the
/// curve's sell leg and an open selects `current`: short.win appends it to the `sell_v2` CPI
/// (read from the transaction's inner instructions: the plain open's accounts plus the extra
/// last), and pump ignores the trailing account, so the proceeds equal those of the same open
/// without extras (from the same state).
/// A second, hostile write names the lending vault: the open selecting `current` is refused
/// `DeniedAccount` (the open's denylist), and the same open selecting `previous` — the harmless
/// set — lands with the same proceeds.
#[test]
fn extras_on_the_sell_leg_ride_along_and_a_hostile_set_is_bypassed() {
    use shortwin::venues::{EXTRAS_SET_CURRENT, EXTRAS_SET_PREVIOUS};
    let (mut env, pm, q) = setup(PumpCoin::T22_115);
    let b = env.borrower();
    let extra = env.funded_keypair().pubkey();
    publish_curve_sell_extras(&mut env, vec![fixed_extra(extra)]);
    let seed = AccountMeta::new_readonly(pda::venue_extras_seed(VenueKind::PumpCurve as u8), false);
    let harmless = [seed.clone(), AccountMeta::new_readonly(extra, false)];
    let saved = env.svm.clone();
    let ixs = open_with_extras_ixs(&pm, &b, q, ExtrasSel::NONE.set, &[]);
    let meta = ok(env.send(&ixs, &[&b.kp]));
    let plain: Open = events::one(&meta);
    let plain_cpi = curve_leg_cpi_accounts(&env, &ixs, &meta);
    env.svm = saved.clone();
    let ixs = open_with_extras_ixs(&pm, &b, q, EXTRAS_SET_CURRENT, &harmless);
    let meta = ok(env.send(&ixs, &[&b.kp]));
    let with: Open = events::one(&meta);
    // The extra reaches pump: the `sell_v2` CPI is the plain open's account list plus the
    // published key at its end (forwarded, not dropped), and pump runs it.
    let with_cpi = curve_leg_cpi_accounts(&env, &ixs, &meta);
    assert_eq!(
        with_cpi.split_last(),
        Some((&extra, plain_cpi.as_slice())),
        "sell_v2 carries the extra last"
    );
    assert_eq!(
        (with.s, with.n_open),
        (plain.s, plain.n_open),
        "proceeds unchanged"
    );
    env.svm = saved;
    publish_curve_sell_extras(&mut env, vec![fixed_extra(pm.keys.pool_vault)]);
    let hostile = [seed, AccountMeta::new_readonly(pm.keys.pool_vault, false)];
    assert_err(
        &open_with_extras(&mut env, &pm, &b, q, EXTRAS_SET_CURRENT, &hostile),
        E::DeniedAccount,
    );
    let prev: Open = events::one(&ok(open_with_extras(
        &mut env,
        &pm,
        &b,
        q,
        EXTRAS_SET_PREVIOUS,
        &harmless,
    )));
    assert_eq!(prev.s, plain.s, "previous: the harmless set");
}

/// M10 batch 3 on the curve (ARCH §2.4 rule 4; `engine::pool_and_borrower_denied_extras`): the
/// bonding curve (its SOL reserve), its base ATA and its quote ATA — and, on the keeper exits, the
/// position's borrower — each published as a fixed extra, are `DeniedAccount` on `open_short`,
/// `close_short`, `liquidate` and `expire`, the position unchanged; the same set shape with a fresh
/// key rides along (pump ignores the trailing account): the open, the close and the expiry land,
/// the healthy liquidation reaches `NotLiquidatable`.
#[test]
fn the_curve_and_the_borrower_are_denied_extras() {
    shortwin_litesvm_tests::engine::pool_and_borrower_denied_extras(
        || setup(PumpCoin::T22_115),
        "curve",
    );
}

/// M10 batch 3 (the security review) on the curve: its base and quote ATAs are ATA-program PDAs
/// of the `bonding_curve` placeholder, so an Emergency template derives them; the denylist runs
/// on the derived key, so each is `DeniedAccount` on `open_short` through `current` and through
/// `previous` (`engine::template_aliases_denied`).
#[test]
fn template_aliases_of_the_curve_atas_are_denied_extras() {
    use shortwin::constants::{ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT};
    use shortwin::state::{PLACEHOLDER_BONDING_CURVE, PLACEHOLDER_MINT};
    use shortwin_litesvm_tests::engine::{PoolAlias, VenueMarket, seed_const, seed_placeholder};
    shortwin_litesvm_tests::engine::template_aliases_denied(
        || setup(PumpCoin::T22_115),
        "curve",
        |env, pm| {
            let bc = pm.read_set(false)[shortwin::venues::pump_curve::C_CURVE].pubkey;
            let mint = pm.mint();
            let tp = env.svm.get_account(&mint).unwrap().owner;
            let ata = |what: &str, prog: Pubkey, m: shortwin::SeedPartArg, mk: Pubkey| PoolAlias {
                what: what.into(),
                program: ASSOCIATED_TOKEN_PROGRAM_ID,
                seeds: vec![
                    seed_placeholder(PLACEHOLDER_BONDING_CURVE),
                    seed_const(prog.as_ref()),
                    m,
                ],
                resolved: vec![
                    bc.to_bytes().to_vec(),
                    prog.to_bytes().to_vec(),
                    mk.to_bytes().to_vec(),
                ],
            };
            vec![
                ata("the base ATA", tp, seed_placeholder(PLACEHOLDER_MINT), mint),
                ata(
                    "the quote ATA",
                    TOKEN_PROGRAM_ID,
                    seed_const(WSOL_MINT.as_ref()),
                    WSOL_MINT,
                ),
            ]
        },
    );
}
