//! The O6 Emergency powers (SPEC §11; ARCH §3.1, §3.0 "Bounded extras"; A40, A41; V2-6):
//! `set_route_broken` and `set_venue_extras`, their authorization and validation, and
//! T-REPLAY-PDA on the mock — a venue interface change that adds a per-mint PDA trailing
//! account is followed with a derivation template, and a hostile `current` set is bypassed
//! with `previous` or `none`.

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::AccountMeta;
use shortwin::PauseScope;
use shortwin::errors::ShortwinError as E;
use shortwin::events::{RouteBrokenSet, VenueExtrasSet};
use shortwin::state::{
    MARKET_FLAG_ROUTE_BROKEN, PLACEHOLDER_MINT, SEED_CONST, SEED_PLACEHOLDER, VenueExtras,
};
use shortwin::venues::{EXTRAS_SET_CURRENT, EXTRAS_SET_NONE, EXTRAS_SET_PREVIOUS, ExtrasSel};
use shortwin::{ExtraEntryArg, SeedPartArg};
use shortwin_litesvm_tests::engine::{
    VENUE_HINT_MOCK, mock_extras_key, set_route_broken_ix, set_venue_extras_ix,
};
use shortwin_litesvm_tests::mock::{Knobs, update_knobs};
use solana_keypair::Keypair;
use solana_signer::Signer;

use crate::common::*;

const SELL: u8 = 0;
const BUY: u8 = 1;

fn seed_const(b: &[u8]) -> SeedPartArg {
    let mut bytes = [0u8; 32];
    bytes[..b.len()].copy_from_slice(b);
    SeedPartArg {
        kind: SEED_CONST,
        len: b.len() as u8,
        bytes,
    }
}

fn seed_mint() -> SeedPartArg {
    SeedPartArg {
        kind: SEED_PLACEHOLDER,
        len: PLACEHOLDER_MINT,
        bytes: [0; 32],
    }
}

fn template(program: Pubkey, seeds: Vec<SeedPartArg>) -> ExtraEntryArg {
    ExtraEntryArg {
        kind: 2,
        writable: false,
        key: program,
        seeds,
    }
}

fn fixed(key: Pubkey, writable: bool) -> ExtraEntryArg {
    ExtraEntryArg {
        kind: 1,
        writable,
        key,
        seeds: vec![],
    }
}

fn set_extras(env: &mut Env, ix_kind: u8, entries: Vec<ExtraEntryArg>) -> TxResult {
    let ea = env.pause_authority.insecure_clone();
    let ix = set_venue_extras_ix(&ea.pubkey(), VENUE_HINT_MOCK, ix_kind, entries);
    env.send(&[ix], &[&ea])
}

fn open_with(
    env: &mut Env,
    mm: &MockMarket,
    b: &Keypair,
    nonce: u64,
    sel: ExtrasSel,
    extras: &[AccountMeta],
) -> TxResult {
    let ix = open_short_ix(
        mm,
        &b.pubkey(),
        nonce,
        Q,
        0,
        u64::MAX,
        u64::MAX,
        sel,
        extras,
    );
    env.send(&[ix], &[b])
}

fn close_with(
    env: &mut Env,
    mm: &MockMarket,
    b: &Keypair,
    nonce: u64,
    sel: ExtrasSel,
    extras: &[AccountMeta],
) -> TxResult {
    let ix = close_short_ix(mm, &b.pubkey(), nonce, u64::MAX, sel, extras);
    env.send(&[ix], &[b])
}

fn sel(set: u8, bump0: u8) -> ExtrasSel {
    ExtrasSel {
        set,
        bumps: [bump0, 0, 0, 0],
    }
}

#[test]
fn set_route_broken_is_emergency_or_governance_only() {
    let (mut env, mm) = setup();
    let stranger = env.funded_keypair();
    let res = env.send(
        &[set_route_broken_ix(
            &stranger.pubkey(),
            PauseScope::Market,
            0,
            true,
            Some(mm.market()),
        )],
        &[&stranger],
    );
    assert_err(&res, E::Unauthorized);
    let gov = env.governance.insecure_clone();
    let meta = ok(env.send(
        &[set_route_broken_ix(
            &gov.pubkey(),
            PauseScope::Market,
            0,
            true,
            Some(mm.market()),
        )],
        &[&gov],
    ));
    let ev: RouteBrokenSet = events::one(&meta);
    assert!(ev.broken && ev.market == mm.market());
    assert!(env.market(&mm.market()).has_flag(MARKET_FLAG_ROUTE_BROKEN));
    let ea = env.pause_authority.insecure_clone();
    ok(env.send(
        &[set_route_broken_ix(
            &ea.pubkey(),
            PauseScope::Global,
            5,
            true,
            None,
        )],
        &[&ea],
    ));
    assert_eq!(env.config().venue_route_broken, 1 << 5);
    // Unknown bits and mismatched scopes are refused; the flag moves no funds (I4 runs on
    // every admin-signed instruction).
    let res = env.send(
        &[set_route_broken_ix(
            &ea.pubkey(),
            PauseScope::Global,
            9,
            true,
            None,
        )],
        &[&ea],
    );
    assert_err(&res, E::InvalidParams);
    let res = env.send(
        &[set_route_broken_ix(
            &ea.pubkey(),
            PauseScope::Market,
            0,
            true,
            None,
        )],
        &[&ea],
    );
    assert_err(&res, E::AccountMismatch);
}

#[test]
fn set_venue_extras_validates_and_rotates() {
    let (mut env, _mm) = setup();
    let stranger = env.funded_keypair();
    let res = env.send(
        &[set_venue_extras_ix(
            &stranger.pubkey(),
            VENUE_HINT_MOCK,
            SELL,
            vec![],
        )],
        &[&stranger],
    );
    assert_err(&res, E::Unauthorized);
    let prog = Pubkey::new_unique();
    // Invalid shapes.
    for bad in [
        vec![fixed(prog, false); 5],
        vec![fixed(shortwin::ID, false)],
        vec![fixed(Pubkey::default(), false)],
        vec![template(prog, vec![])],
        vec![template(shortwin::ID, vec![seed_mint()])],
        vec![template(prog, vec![seed_mint(); 5])],
        vec![template(
            prog,
            vec![SeedPartArg {
                kind: SEED_CONST,
                len: 33,
                bytes: [1; 32],
            }],
        )],
        vec![template(
            prog,
            vec![SeedPartArg {
                kind: SEED_PLACEHOLDER,
                len: 9,
                bytes: [0; 32],
            }],
        )],
        vec![ExtraEntryArg {
            kind: 3,
            writable: false,
            key: prog,
            seeds: vec![],
        }],
    ] {
        assert_err(&set_extras(&mut env, SELL, bad), E::BadExtras);
    }
    let ea = env.pause_authority.insecure_clone();
    let res = env.send(
        &[set_venue_extras_ix(
            &ea.pubkey(),
            VENUE_HINT_MOCK,
            3,
            vec![],
        )],
        &[&ea],
    );
    assert_err(&res, E::BadExtras);
    // Rotation: current → previous, versions count per set.
    let a = Pubkey::new_unique();
    let b = Pubkey::new_unique();
    let meta = ok(set_extras(&mut env, SELL, vec![fixed(a, false)]));
    let ev: VenueExtrasSet = events::one(&meta);
    assert_eq!(
        (ev.venue, ev.ix, ev.version, ev.keys[0]),
        (VENUE_HINT_MOCK, SELL, 1, a)
    );
    ok(set_extras(&mut env, SELL, vec![fixed(b, true)]));
    let ve: VenueExtras = read::zc(&env.svm, &mock_extras_key()).unwrap();
    let set = &ve.sets[usize::from(SELL)];
    assert_eq!(
        (
            set.current.version,
            set.current.count,
            set.current.entries[0].key
        ),
        (2, 1, b)
    );
    assert_eq!(set.current.entries[0].writable, 1);
    assert_eq!((set.previous.version, set.previous.entries[0].key), (1, a));
    // Governance may write too.
    let gov = env.governance.insecure_clone();
    ok(env.send(
        &[set_venue_extras_ix(
            &gov.pubkey(),
            VENUE_HINT_MOCK,
            BUY,
            vec![],
        )],
        &[&gov],
    ));
}

/// T-REPLAY-PDA on the mock (ARCH §8.2): the venue starts requiring a per-mint PDA as a
/// trailing swap account (the mock's `required_extras`, like pump's 2026-04-28 change). Exits
/// fail until the Emergency multisig publishes a derivation template; then opens and closes
/// pass it through, re-derived on-chain from the market's mint and the caller's bump.
#[test]
fn a_template_follows_a_new_per_mint_pda() {
    let (mut env, mm) = setup();
    let prog = Pubkey::new_unique();
    let (pda_key, bump) = Pubkey::find_program_address(&[b"extra", mm.mint().as_ref()], &prog);
    let mut b = env.borrower();
    let (_o, nonce, _) = open_ok(&mut env, &mm, &mut b, Q);
    env.tick(&mm, 1);
    // The venue changes its interface.
    update_knobs(&mut env, &mm.pool, |k: &mut Knobs| {
        k.required_extras = vec![pda_key]
    });
    let res = close_with(&mut env, &mm, &b.kp, nonce, ExtrasSel::NONE, &[]);
    assert_eq!(
        custom_code(&res),
        Some(6_011),
        "the mock's ExtrasMismatch: exits are stuck"
    );
    // Emergency publishes the template for the buy and the sell legs.
    let t = template(prog, vec![seed_const(b"extra"), seed_mint()]);
    ok(set_extras(&mut env, BUY, vec![t.clone()]));
    ok(set_extras(&mut env, SELL, vec![t]));
    let extras = [
        AccountMeta::new_readonly(mock_extras_key(), false),
        AccountMeta::new_readonly(pda_key, false),
    ];
    // A wrong bump or a wrong account is refused by short.win's parser.
    let res = close_with(
        &mut env,
        &mm,
        &b.kp,
        nonce,
        sel(EXTRAS_SET_CURRENT, bump.wrapping_sub(1)),
        &extras,
    );
    assert_err(&res, E::BadExtras);
    let wrong = [
        extras[0].clone(),
        AccountMeta::new_readonly(Pubkey::new_unique(), false),
    ];
    assert_err(
        &close_with(
            &mut env,
            &mm,
            &b.kp,
            nonce,
            sel(EXTRAS_SET_CURRENT, bump),
            &wrong,
        ),
        E::BadExtras,
    );
    ok(close_with(
        &mut env,
        &mm,
        &b.kp,
        nonce,
        sel(EXTRAS_SET_CURRENT, bump),
        &extras,
    ));
    // Opens follow the same set.
    ok(open_with(
        &mut env,
        &mm,
        &b.kp,
        7,
        sel(EXTRAS_SET_CURRENT, bump),
        &extras,
    ));
}

/// V2-6: one wrong or hostile Emergency write never bricks exits — the caller selects the
/// `previous` set (or `none` when the venue needs no extras).
#[test]
fn a_hostile_current_set_is_bypassed() {
    let (mut env, mm) = setup();
    let prog = Pubkey::new_unique();
    let (pda_key, bump) = Pubkey::find_program_address(&[b"extra", mm.mint().as_ref()], &prog);
    let t = template(prog, vec![seed_const(b"extra"), seed_mint()]);
    ok(set_extras(&mut env, BUY, vec![t.clone()]));
    ok(set_extras(&mut env, SELL, vec![t]));
    update_knobs(&mut env, &mm.pool, |k: &mut Knobs| {
        k.required_extras = vec![pda_key]
    });
    let good = [
        AccountMeta::new_readonly(mock_extras_key(), false),
        AccountMeta::new_readonly(pda_key, false),
    ];
    let b = env.borrower();
    ok(open_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        sel(EXTRAS_SET_CURRENT, bump),
        &good,
    ));
    env.tick(&mm, 1);
    // A hostile write: the buy set now names a random account.
    let junk = Pubkey::new_unique();
    ok(set_extras(&mut env, BUY, vec![fixed(junk, false)]));
    let hostile = [
        AccountMeta::new_readonly(mock_extras_key(), false),
        AccountMeta::new_readonly(junk, false),
    ];
    let res = close_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        sel(EXTRAS_SET_CURRENT, 0),
        &hostile,
    );
    assert_eq!(
        custom_code(&res),
        Some(6_011),
        "the venue refuses the hostile account"
    );
    ok(close_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        sel(EXTRAS_SET_PREVIOUS, bump),
        &good,
    ));
    // A venue that needs no extras: `none` always works, whatever the sets say.
    update_knobs(&mut env, &mm.pool, |k: &mut Knobs| {
        k.required_extras = vec![]
    });
    ok(open_with(&mut env, &mm, &b.kp, 1, ExtrasSel::NONE, &[]));
    env.tick(&mm, 1);
    ok(close_with(&mut env, &mm, &b.kp, 1, ExtrasSel::NONE, &[]));
}

/// ARCH §2.4 rule 4 at use time: extras must be non-signers, not short.win-owned, not on the
/// instruction's denylist, writable exactly as flagged; the segment must match the selection.
#[test]
fn extras_parser_is_strict() {
    let (mut env, mm) = setup();
    let b = env.borrower();
    let ve = AccountMeta::new_readonly(mock_extras_key(), false);
    // `none` with trailing accounts; an unknown selection; a missing entry.
    let res = open_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        ExtrasSel::NONE,
        std::slice::from_ref(&ve),
    );
    assert_err(&res, E::BadRemainingAccounts);
    // 4 is the first unknown selection (3 is `stable` since M2-SEC-2).
    let res = open_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        sel(4, 0),
        std::slice::from_ref(&ve),
    );
    assert_err(&res, E::BadExtras);
    ok(set_extras(
        &mut env,
        SELL,
        vec![fixed(mm.keys.pool_vault, false)],
    ));
    let res = open_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        sel(EXTRAS_SET_CURRENT, 0),
        std::slice::from_ref(&ve),
    );
    assert_err(&res, E::BadRemainingAccounts);
    // The market's pool vault (denylisted) and the market itself (short.win-owned).
    let res = open_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        sel(EXTRAS_SET_CURRENT, 0),
        &[
            ve.clone(),
            AccountMeta::new_readonly(mm.keys.pool_vault, false),
        ],
    );
    assert_err(&res, E::DeniedAccount);
    ok(set_extras(&mut env, SELL, vec![fixed(mm.market(), false)]));
    let res = open_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        sel(EXTRAS_SET_CURRENT, 0),
        &[ve.clone(), AccountMeta::new_readonly(mm.market(), false)],
    );
    assert_err(&res, E::DeniedAccount);
    // Writable flag mismatch.
    let key = Pubkey::new_unique();
    ok(set_extras(&mut env, SELL, vec![fixed(key, true)]));
    let res = open_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        sel(EXTRAS_SET_CURRENT, 0),
        &[ve.clone(), AccountMeta::new_readonly(key, false)],
    );
    assert_err(&res, E::BadExtras);
    // A signer extra.
    let signer = env.funded_keypair();
    ok(set_extras(
        &mut env,
        SELL,
        vec![fixed(signer.pubkey(), false)],
    ));
    let ix = open_short_ix(
        &mm,
        &b.key(),
        0,
        Q,
        0,
        u64::MAX,
        u64::MAX,
        sel(EXTRAS_SET_CURRENT, 0),
        &[ve.clone(), AccountMeta::new_readonly(signer.pubkey(), true)],
    );
    assert_err(&env.send(&[ix], &[&b.kp, &signer]), E::DeniedAccount);
    // The wrong VenueExtras account (another venue's).
    let other = AccountMeta::new_readonly(pda::venue_extras_seed(5), false);
    let res = open_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        sel(EXTRAS_SET_CURRENT, 0),
        &[other, AccountMeta::new_readonly(signer.pubkey(), false)],
    );
    assert_err(&res, E::BadExtras);
    // An empty `current` set selected explicitly: just the VenueExtras account, then works.
    ok(set_extras(&mut env, SELL, vec![]));
    ok(open_with(
        &mut env,
        &mm,
        &b.kp,
        0,
        sel(EXTRAS_SET_CURRENT, 0),
        &[ve],
    ));
    let _ = EXTRAS_SET_NONE;
}
