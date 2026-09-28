//! I14 on Raydium (ARCH §3.0, §3.12, §8.3; C0-2): every ReadSet and TradeExtras slot of both
//! adapters is mandatory and key-checked on `open_short`, `close_short`, `liquidate`, `expire`,
//! `crank_price` and `sol_settle` — each slot omitted, replaced by a foreign key, the first two
//! swapped, or a trailing account appended makes the instruction fail with
//! `BadRemainingAccounts` / `VenueAccountMismatch`, never "no route" or a silent default.
//! Also: the venue ignores trailing extras (R-RAY §9.6), so a selected extras set with a fixed
//! read-only entry trades normally on both programs.

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
