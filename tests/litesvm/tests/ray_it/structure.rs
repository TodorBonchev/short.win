//! I14 on Raydium (ARCH §3.0, §3.12, §8.3; C0-2): every ReadSet and TradeExtras slot of both
//! adapters is mandatory and key-checked on `open_short`, `close_short`, `liquidate`, `expire`,
//! `crank_price` and `sol_settle` — each slot omitted, replaced by a foreign key, the first two
//! swapped, or a trailing account appended makes the instruction fail with
//! `BadRemainingAccounts` / `VenueAccountMismatch`, never "no route" or a silent default.
//! Also: the venue ignores trailing extras (R-RAY §9.6), so a selected extras set with a fixed
//! read-only entry trades normally on both programs. M10 on AMM v4: a thin pool (the reversed
//! Fartcoin pool) in place of the adopted one on every instruction (G-A6's AMM v4 part), and a
//! denylisted key in an extras set refused and bypassed with `previous` (G-A5's fork run).

use shortwin::state::EXTRAS_IX_SELL;
use shortwin::venues::EXTRAS_SET_CURRENT;
use shortwin::{ExtraEntryArg, SeedPartArg};
use shortwin_litesvm_tests::engine::{expire_ix, liquidate_ix, set_venue_extras_ix, sol_settle_ix};

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
    let mut order = ix.clone();
    order.accounts.swap(start + 1, start + 2);
    out.push(("slots 1 and 2 swapped".to_string(), order));
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
        let c = custom_code(&res);
        assert!(
            c == Some(code(E::BadRemainingAccounts)) || c == Some(code(E::VenueAccountMismatch)),
            "{what}: expected BadRemainingAccounts / VenueAccountMismatch, got {c:?}"
        );
    }
}

#[test]
fn every_venue_slot_is_mandatory_and_key_checked() {
    for fixture in [FART, USELESS, T22] {
        let (mut env, rm, q) = setup(fixture);
        let (read_len, trade_len) = (rm.read_set(false).len(), rm.trade_extras().len());
        let all = read_len + trade_len;
        let b = env.borrower();
        let open = open_short_ix(
            &rm,
            &b.key(),
            0,
            q,
            0,
            u64::MAX,
            u64::MAX,
            ExtrasSel::NONE,
            &[],
        );
        assert_all_fail(&mut env, &open, all, &[&b.kp]);
        ok(env.send(&[open], &[&b.kp]));
        env.tick(&rm, 1);
        let crank = crank_price_ix(&rm);
        assert_all_fail(&mut env, &crank, read_len, &[]);
        let keeper = env.funded_keypair();
        let settle = sol_settle_ix(&env, &rm, &keeper.pubkey(), &b.key(), 0);
        assert_all_fail(&mut env, &settle, read_len, &[&keeper]);
        let liq = liquidate_ix(&rm, &keeper.pubkey(), &b.key(), 0);
        assert_all_fail(&mut env, &liq, all, &[&keeper]);
        let exp = expire_ix(&rm, &keeper.pubkey(), &b.key(), 0);
        assert_all_fail(&mut env, &exp, all, &[&keeper]);
        let close = close_short_ix(&rm, &b.key(), 0, u64::MAX, ExtrasSel::NONE, &[]);
        assert_all_fail(&mut env, &close, all, &[&b.kp]);
        ok(env.send(&[crank], &[]));
        ok(env.send(&[close], &[&b.kp]));
    }
}

/// The ReadSet and TradeExtras are the ones the market recorded at listing: another pool of
/// the same mint (the reversed Fartcoin pool) in place of the adopted one is refused.
#[test]
fn a_thin_pool_in_place_of_the_adopted_one() {
    let (mut env, rm, q) = setup(FART);
    let rev = env.load_ray_v4(V4Fixture::FartcoinReversed);
    let b = env.borrower();
    let mut open = open_short_ix(
        &rm,
        &b.key(),
        0,
        q,
        0,
        u64::MAX,
        u64::MAX,
        ExtrasSel::NONE,
        &[],
    );
    let start = open.accounts.len() - 10;
    for (i, key) in [rev.pool, rev.coin_vault, rev.pc_vault, rev.lp_mint]
        .into_iter()
        .enumerate()
    {
        open.accounts[start + i].pubkey = key;
    }
    assert_err(&env.send(&[open], &[&b.kp]), E::VenueAccountMismatch);
}

/// A selected extras set (A41) is appended to the venue CPI; AMM v4 v2 and CPMM ignore
/// trailing accounts (R-RAY §9.6), so a harmless fixed entry changes nothing.
#[test]
fn extras_ride_along_on_both_programs() {
    for fixture in [FART, USELESS] {
        let (mut env, rm, q) = setup(fixture);
        let venue = rm.kind() as u8;
        let extra = env.funded_keypair().pubkey();
        let ea = env.pause_authority.insecure_clone();
        let entry = ExtraEntryArg {
            kind: shortwin::state::ENTRY_FIXED,
            writable: false,
            key: extra,
            seeds: Vec::<SeedPartArg>::new(),
        };
        ok(env.send(
            &[set_venue_extras_ix(
                &ea.pubkey(),
                venue,
                EXTRAS_IX_SELL as u8,
                vec![entry],
            )],
            &[&ea],
        ));
        let b = env.borrower();
        let sel = ExtrasSel {
            set: EXTRAS_SET_CURRENT,
            bumps: [0; 4],
        };
        let extras = [
            AccountMeta::new_readonly(pda::venue_extras_seed(venue), false),
            AccountMeta::new_readonly(extra, false),
        ];
        let s_exp = rm.quote_sell(&env.svm, q).unwrap().proceeds;
        let open = open_short_ix(&rm, &b.key(), 0, q, 0, u64::MAX, u64::MAX, sel, &extras);
        let meta = ok(env.send(&[open], &[&b.kp]));
        let ev: Open = events::one(&meta);
        assert_eq!(ev.s, s_exp, "{fixture:?}");
    }
}

/// The invariant hooks see Raydium markets (`invariants::venue_view`, computed from the readers
/// and the math crate, not from the adapter): the same executability, reserves and quotes as
/// the fork's pool helpers — so I13's re-quotes and I16's route rules run on every Raydium
/// instruction of this binary — and a halted pool is not executable to them either.
#[test]
fn the_hooks_see_raydium_markets() {
    use shortwin_litesvm_tests::invariants::venue_view;
    for fixture in ELIGIBLE {
        let (mut env, rm, q) = setup(fixture);
        let m = env.market(&rm.market());
        let v = venue_view::view(&env.svm, &m).expect("a Raydium view");
        assert!(v.executable, "{fixture:?}");
        assert_eq!(v.reserves, rm.reserves(&env.svm));
        assert_eq!(v.buy(q), rm.quote_buy(&env.svm, q).map(|b| b.cost));
        assert_eq!(v.sell(q), rm.quote_sell(&env.svm, q).map(|s| s.proceeds));
        assert_eq!(
            u64::from(v.fee_ppm),
            match &rm.pool {
                AnyPool::V4(p) => u64::from(p.info(&env.svm).fee_ppm().unwrap()),
                AnyPool::Cpmm(p) =>
                    u64::from(p.state(&env.svm).fee_ppm(&p.config(&env.svm)).unwrap()),
            }
        );
        match &rm.pool {
            AnyPool::V4(p) => fork::set_v4_status(&mut env.svm, p, 2),
            AnyPool::Cpmm(p) => fork::set_cpmm_status(&mut env.svm, p, 4),
        }
        let v = venue_view::view(&env.svm, &m).unwrap();
        assert_eq!((v.executable, v.reserves, v.fee_ppm), (false, None, 0));
    }
}

/// The reversed Fartcoin pool's keys (pool, coin vault, pc vault, LP mint) in place of the adopted
/// pool's in the first four ReadSet slots of `ix`, whose last `remaining` metas are the venue's.
fn thin(ix: &Instruction, remaining: usize, rev: &fork::V4Pool) -> Instruction {
    let mut ix = ix.clone();
    let start = ix.accounts.len() - remaining;
    for (i, key) in [rev.pool, rev.coin_vault, rev.pc_vault, rev.lp_mint]
        .into_iter()
        .enumerate()
    {
        ix.accounts[start + i].pubkey = key;
    }
    ix
}

/// M10 G-A6's AMM v4 part beyond the open: the reversed Fartcoin pool in place of the adopted one
/// is refused (`VenueAccountMismatch`) on `crank_price`, `sol_settle`, `close_short`, `liquidate`
/// and `expire` as on `open_short`, the position unchanged; each honest instruction (the control)
/// is accepted — the crank, the `sol_settle` no-op, the close, the healthy position's liquidation
/// refused by its body (`NotLiquidatable`), the expiry after the deadline.
#[test]
fn a_thin_pool_in_place_of_the_adopted_one_on_every_instruction() {
    let (mut env, rm, q) = setup(FART);
    let rev = env.load_ray_v4(V4Fixture::FartcoinReversed);
    let (read_len, all) = (
        rm.read_set(false).len(),
        rm.read_set(false).len() + rm.trade_extras().len(),
    );
    let mut b = env.borrower();
    let (_, n0, _) = open_ok(&mut env, &rm, &mut b, q);
    let mut b2 = env.borrower();
    let (_, n1, _) = open_ok(&mut env, &rm, &mut b2, q);
    env.tick(&rm, 1);
    let keeper = env.funded_keypair();
    let pos1 = pda::position(&rm.market(), &b2.key(), n1).0;
    let pos_bytes = |env: &Env| env.svm.get_account(&pos1).map(|a| a.data);
    let before = pos_bytes(&env);

    let crank = crank_price_ix(&rm);
    assert_err(
        &env.send(&[thin(&crank, read_len, &rev)], &[]),
        E::VenueAccountMismatch,
    );
    ok(env.send(&[crank], &[]));
    let settle = sol_settle_ix(&env, &rm, &keeper.pubkey(), &b2.key(), n1);
    assert_err(
        &env.send(&[thin(&settle, read_len, &rev)], &[&keeper]),
        E::VenueAccountMismatch,
    );
    let liq = liquidate_ix(&rm, &keeper.pubkey(), &b2.key(), n1);
    assert_err(
        &env.send(&[thin(&liq, all, &rev)], &[&keeper]),
        E::VenueAccountMismatch,
    );
    assert_eq!(pos_bytes(&env), before, "the position unchanged");
    ok(env.send(&[settle], &[&keeper]));
    assert_err(&env.send(&[liq], &[&keeper]), E::NotLiquidatable);

    let close = close_short_ix(&rm, &b.key(), n0, u64::MAX, ExtrasSel::NONE, &[]);
    assert_err(
        &env.send(&[thin(&close, all, &rev)], &[&b.kp]),
        E::VenueAccountMismatch,
    );
    ok(env.send(&[close], &[&b.kp]));

    let deadline = env.position(&rm, &b2.key(), n1).deadline;
    let now = env.now();
    env.warp(deadline - now + 1);
    env.tick(&rm, 3);
    let exp = expire_ix(&rm, &keeper.pubkey(), &b2.key(), n1);
    let before = pos_bytes(&env);
    assert_err(
        &env.send(&[thin(&exp, all, &rev)], &[&keeper]),
        E::VenueAccountMismatch,
    );
    assert_eq!(pos_bytes(&env), before, "the position unchanged");
    let meta = ok(env.send(&[exp], &[&keeper]));
    let _: shortwin::events::Expire = events::one(&meta);
}

/// A denylisted key in an AMM v4 extras set (M10 G-A5's fork run; ARCH §2.4 rule 4, A41): the
/// Emergency publishes a harmless fixed extra, then a hostile set naming the market's pool vault —
/// on the sell leg and on the buy leg. Selecting the hostile `current` set fails `DeniedAccount`
/// on the open and on the close; selecting `previous` (the harmless set) bypasses it and trades
/// exactly as with no extras.
#[test]
fn a_denylisted_extra_on_amm_v4_is_bypassed_with_previous() {
    use shortwin::state::EXTRAS_IX_BUY;
    use shortwin::venues::EXTRAS_SET_PREVIOUS;
    let (mut env, rm, q) = setup(FART);
    let venue = rm.kind() as u8;
    let ea = env.pause_authority.insecure_clone();
    let harmless = env.funded_keypair().pubkey();
    let entry = |key: Pubkey| ExtraEntryArg {
        kind: shortwin::state::ENTRY_FIXED,
        writable: false,
        key,
        seeds: Vec::<SeedPartArg>::new(),
    };
    for ix_kind in [EXTRAS_IX_SELL as u8, EXTRAS_IX_BUY as u8] {
        for key in [harmless, rm.keys.pool_vault] {
            ok(env.send(
                &[set_venue_extras_ix(
                    &ea.pubkey(),
                    venue,
                    ix_kind,
                    vec![entry(key)],
                )],
                &[&ea],
            ));
        }
    }
    let ve = AccountMeta::new_readonly(pda::venue_extras_seed(venue), false);
    let hostile = [
        ve.clone(),
        AccountMeta::new_readonly(rm.keys.pool_vault, false),
    ];
    let good = [ve, AccountMeta::new_readonly(harmless, false)];
    let current = ExtrasSel {
        set: EXTRAS_SET_CURRENT,
        bumps: [0; 4],
    };
    let previous = ExtrasSel {
        set: EXTRAS_SET_PREVIOUS,
        bumps: [0; 4],
    };
    let b = env.borrower();
    let open = |sel, extras: &[AccountMeta]| {
        open_short_ix(&rm, &b.key(), 0, q, 0, u64::MAX, u64::MAX, sel, extras)
    };
    assert_err(
        &env.send(&[open(current, &hostile)], &[&b.kp]),
        E::DeniedAccount,
    );
    let s_exp = rm.quote_sell(&env.svm, q).unwrap().proceeds;
    let meta = ok(env.send(&[open(previous, &good)], &[&b.kp]));
    let ev: Open = events::one(&meta);
    assert_eq!(ev.s, s_exp, "as with no extras");
    env.tick(&rm, 1);
    let close =
        |sel, extras: &[AccountMeta]| close_short_ix(&rm, &b.key(), 0, u64::MAX, sel, extras);
    assert_err(
        &env.send(&[close(current, &hostile)], &[&b.kp]),
        E::DeniedAccount,
    );
    let b_exp = rm.quote_buy(&env.svm, q).unwrap().cost;
    let meta = ok(env.send(&[close(previous, &good)], &[&b.kp]));
    let ev: Close = events::one(&meta);
    assert_eq!(ev.buyback_cost, b_exp, "as with no extras");
}
