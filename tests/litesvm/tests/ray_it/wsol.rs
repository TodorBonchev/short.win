//! T-RAY-WSOL (ARCH §2.4 rules 2 and 5, §2.4.1 "The ephemeral WSOL account"; A5, A33; C0-3,
//! C1-3; R-RAY §4.2) on both programs: a pre-funded `trader_wsol` address (which blocks
//! `create_account`, never `allocate` / `assign`), lamports donated to the trader PDA, and a
//! pre-created `ATA(trader, WSOL)` holding WSOL never block a Raydium leg; donated lamports are
//! swept to `market_vault` and vest to lenders; the ATA, which no leg uses, is closed by
//! `sweep_trader` later. Every leg still measures exact deltas.

use anchor_lang::solana_program::instruction::AccountMeta as Meta;
use shortwin::events::TraderSwept;
use shortwin_litesvm_tests::engine::{sweep_trader_ix, trader_wsol_ata};
use shortwin_litesvm_tests::pda::ata;
use solana_keypair::Keypair;

use crate::common::*;

fn transfer(from: &Keypair, to: &Pubkey, lamports: u64) -> Instruction {
    let mut data = vec![2, 0, 0, 0];
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: anchor_lang::system_program::ID,
        accounts: vec![Meta::new(from.pubkey(), true), Meta::new(*to, false)],
        data,
    }
}

/// Donations to both trader PDAs before an open, then before a close: each leg sweeps them to
/// `market_vault` as a vesting lender lump (`TraderSwept`), funds exactly what it needs, and
/// trades at the exact quote.
#[test]
fn pre_funded_trader_pdas() {
    for fixture in [FART, USELESS, T22] {
        let (mut env, rm, q) = setup(fixture);
        let griefer = env.funded_keypair();
        // Far more than the WSOL account's rent at its address: the leg sweeps all of it
        // first, then funds exactly `rent + amount` (A5).
        ok(env.send(
            &[
                transfer(&griefer, &rm.keys.trader, 1_234_567),
                transfer(&griefer, &rm.trader_wsol, 3_000_000_000),
            ],
            &[&griefer],
        ));
        let v0 = env.market_vault(&rm.market());
        let vest0 = env.market(&rm.market()).vest_bal;
        let s_exp = rm.quote_sell(&env.svm, q).unwrap().proceeds;
        let mut b = env.borrower();
        let (open, nonce, meta) = open_ok(&mut env, &rm, &mut b, q);
        assert_eq!(open.s, s_exp, "{fixture:?}");
        let swept: TraderSwept = events::one(&meta);
        assert_eq!(swept.lamports, 3_001_234_567);
        let v1 = env.market_vault(&rm.market());
        assert_eq!(v1.lender_sol, v0.lender_sol + 3_001_234_567);
        assert_eq!(v1.protocol_fees, v0.protocol_fees + open.fee_open);
        assert_eq!(env.market(&rm.market()).vest_bal, vest0 + 3_001_234_567);
        assert_eq!(lamports(&env, &rm.keys.trader), 0);
        assert_eq!(lamports(&env, &rm.trader_wsol), 0);

        env.warp(1);
        env.crank(&rm);
        env.warp(1);
        ok(env.send(
            &[
                transfer(&griefer, &rm.keys.trader, 1_000_000),
                transfer(&griefer, &rm.trader_wsol, 5_000_000),
            ],
            &[&griefer],
        ));
        let v1 = env.market_vault(&rm.market());
        let b_exp = rm.quote_buy(&env.svm, q).unwrap().cost;
        let (close, meta) = close_ok(&mut env, &rm, &b, nonce);
        assert_eq!(close.buyback_cost, b_exp);
        let swept: TraderSwept = events::one(&meta);
        assert_eq!(swept.lamports, 6_000_000);
        let v2 = env.market_vault(&rm.market());
        let lender_rent = close.rent - close.rent_protocol;
        assert_eq!(v2.lender_sol, v1.lender_sol + 6_000_000 + lender_rent);
        assert_eq!(lamports(&env, &rm.trader_wsol), 0);
        assert_eq!(lamports(&env, &rm.keys.trader), 0);
    }
}

/// A third party creates `ATA(trader, WSOL)` and wraps SOL into it: no leg uses it (the legs
/// use the PDA-addressed `trader_wsol`), so opens and closes are unaffected; `sweep_trader`
/// closes it and credits lenders (M2-SEC-4's instruction, on a Raydium market).
#[test]
fn a_pre_created_trader_wsol_ata() {
    for fixture in [FART, USELESS] {
        let (mut env, rm, q) = setup(fixture);
        let ata_key = trader_wsol_ata(&rm.market());
        assert_eq!(
            ata_key,
            ata(
                &rm.keys.trader,
                &shortwin::constants::WSOL_MINT,
                &shortwin::constants::TOKEN_PROGRAM_ID
            )
        );
        let payer = env.payer.insecure_clone();
        let created = litesvm_token::CreateAssociatedTokenAccount::new(
            &mut env.svm,
            &payer,
            &shortwin::constants::WSOL_MINT,
        )
        .owner(&rm.keys.trader)
        .token_program_id(&shortwin::constants::TOKEN_PROGRAM_ID)
        .send()
        .expect("create the trader's WSOL ATA");
        assert_eq!(created, ata_key);
        fork::wrap(&mut env, &ata_key, 2_000_000_000);
        let ata_lamports = lamports(&env, &ata_key);
        let mut b = env.borrower();
        let (open, nonce, meta) = open_ok(&mut env, &rm, &mut b, q);
        assert!(
            events::of::<TraderSwept>(&meta).is_empty(),
            "nothing to sweep"
        );
        assert_eq!(open.q, q);
        assert_eq!(
            lamports(&env, &ata_key),
            ata_lamports,
            "untouched by the leg"
        );
        env.warp(1);
        close_ok(&mut env, &rm, &b, nonce);
        assert_eq!(lamports(&env, &ata_key), ata_lamports);
        let v0 = env.market_vault(&rm.market());
        let meta = ok(env.send(&[sweep_trader_ix(&rm.market(), None)], &[]));
        let swept: TraderSwept = events::one(&meta);
        assert_eq!(swept.lamports, ata_lamports);
        assert_eq!(lamports(&env, &ata_key), 0, "closed");
        assert_eq!(
            env.market_vault(&rm.market()).lender_sol,
            v0.lender_sol + ata_lamports
        );
    }
}

/// The `trader_wsol` address pre-funded between the two instructions of one transaction (a
/// front-runner's bundle): an open right after the donation, in the same transaction, still
/// sweeps and trades.
#[test]
fn a_donation_in_the_same_transaction() {
    let (mut env, rm, q) = setup(FART);
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
    let kp = b.kp.insecure_clone();
    let donate = transfer(&kp, &rm.trader_wsol, 10_000_000);
    let meta = ok(env.send(&[donate, open], &[&kp]));
    let swept: TraderSwept = events::of_ix(&meta, 1).into_iter().next().expect("swept");
    assert_eq!(swept.lamports, 10_000_000);
    assert_eq!(lamports(&env, &rm.trader_wsol), 0);
}
