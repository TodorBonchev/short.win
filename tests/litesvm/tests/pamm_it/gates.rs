//! M6's test gates on the dumped PumpSwap ELF (ARCH §8.2; R-VEN Update 2026-09-30 H4, H6, H7):
//!
//! - **T-DISABLE** (the hard gate, C0-10): the ELF halts buys under `disable_flags` bit 3 and sales
//!   under bit 4 only; short.win's route predicate reads the buy bit (`PAMM_BUY_DISABLED_BIT`):
//!   under it exits have no route and the S3 chain runs (the halt is S3 evidence), while any
//!   other non-zero value only refuses opens (A24: not S3 evidence);
//! - **T-ATA**: every PumpSwap leg names the ephemeral, non-ATA `trader_wsol` as `user`'s WSOL
//!   account, and PumpSwap takes it;
//! - **T-PDA-USER (pAMM buy path)**: PumpSwap's `buy` with the trader PDA as `user`, signed by
//!   short.win's `invoke_signed` at height 2, nothing deeper than height 3;
//! - **T-RENT (pAMM)**: with the existence requirement met the venue takes no rent on a 301-byte
//!   pool (`Open.venue_rent = 0`, `ert` refunded to the lamport); a 211-byte pool held at its
//!   minimum grows to 271 bytes on the sale and takes `minimum_balance(271) −
//!   minimum_balance(211)` from the trader, netted out of `S`, refunded to the borrower and
//!   reported in `Open.venue_rent`;
//! - **T-CASHBACK (pAMM)**: on a cashback coin short.win's trades accrue WSOL cashback in
//!   `ATA(pAMM UVA(trader), WSOL)`; after a full wipe `sweep_trader` leaves `wipe_rewards_claimed`
//!   clear while it holds any, and `[create_idempotent ATA(trader, WSOL), claim_cashback(user =
//!   trader), sweep_trader]` — signed by the fee payer alone, on a venue halted for buys —
//!   credits it to the wiped epoch's record and sets both bits (M5-SEC-1's remainder).

use shortwin::constants::{PUMP_AMM_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT};
use shortwin::events::TraderSwept;
use shortwin::state::{MARKET_FLAG_WIPE_LIVE, MARKET_FLAG_WIPE_REWARDS_CLAIMED};
use shortwin_litesvm_tests::ix;
use shortwin_math::pool::{WIPE_REWARDS_CLAIMED, WIPE_TRADER_SWEPT};
use solana_message::Message;

use crate::common::*;

// ---- T-DISABLE -----------------------------------------------------------------------------------

/// The ELF itself (a normal trader, no short.win): `buy` fails 6020 exactly under bit 3 and
/// `sell` 6021 exactly under bit 4 — the constants the adapter compiles in.
#[test]
fn t_disable_the_elf_halts_buys_on_bit_3_and_sales_on_bit_4() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Mature);
    let u = PammUser::new(&mut env, &p, 10 * SOL);
    let q = q_for(&env, &p) / 10;
    for bit in 0..8u8 {
        pamm::set_disable_flags(&mut env.svm, 1 << bit);
        env.warp(1);
        let buy = u.buy(&mut env, &p, q, u64::MAX);
        let sell = u.sell(&mut env, &p, q, 0);
        match bit {
            3 => assert_eq!(custom_code(&buy), Some(pamm::PAMM_ERR_DISABLED_BUY)),
            _ => {
                ok(buy);
            }
        }
        match bit {
            4 => assert_eq!(custom_code(&sell), Some(pamm::PAMM_ERR_DISABLED_SELL)),
            _ => {
                ok(sell);
            }
        }
    }
    assert_eq!(
        shortwin::venues::pump_amm::PAMM_BUY_DISABLED_BIT,
        1 << 3,
        "the compiled buy bit is the ELF's"
    );
}

/// Under the buy bit the venue cannot execute: the crank starts the S3 chain, the borrower's
/// close and a keeper's liquidation refuse `NoRoute`, and an expired position settles S3 once
/// the chain has run `s3_wait_s` (the halt is S3 evidence, ARCH §5.3). Clearing the bit clears
/// the chain.
#[test]
fn t_disable_the_buy_bit_is_no_route_and_s3_evidence() {
    let (mut env, pm, q) = setup(PammCoin::Legacy261);
    let mut b = env.borrower();
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q);
    pamm::set_disable_flags(&mut env.svm, 1 << 3);
    env.warp(1);
    crank(&mut env, &pm);
    let m = market(&env, &pm);
    assert_eq!(m.no_route_since, env.now(), "the S3 chain starts");
    assert_err(&env.close(&pm, &b, nonce), E::NoRoute);
    let kp = keeper(&mut env);
    assert_err(&liquidate(&mut env, &pm, &kp, &b.key(), nonce), E::NoRoute);
    // Expired, the chain unbroken for s3_wait_s: S3.
    warp_to_deadline(&mut env, &pm, &b.key(), nonce, 1);
    let wait = env.position(&pm, &b.key(), nonce).s3_wait_s;
    let mut settled = None;
    for _ in 0..(wait / 30 + 3) {
        env.warp(30);
        let meta = ok(settle(&mut env, &pm, &kp, &b.key(), nonce));
        if let Some(ev) = events::of::<SolSettle>(&meta).pop() {
            settled = Some(ev);
            break;
        }
    }
    let ev = settled.expect("S3 under the buy halt");
    assert_eq!(ev.condition, 3, "S3");
    // The halt lifted: the chain clears at the next observation.
    pamm::set_disable_flags(&mut env.svm, 0);
    env.warp(1);
    crank(&mut env, &pm);
    assert_eq!(market(&env, &pm).no_route_since, 0);
}

/// Any other non-zero value — the sell bit, `create_pool` / `deposit` / `withdraw`, bits 5–7 —
/// refuses opens (`VenueFlagged`) and is **not** S3 evidence: the venue executes, the crank keeps
/// the chain clear, and the borrower's close (a buy) executes.
#[test]
fn t_disable_other_bits_block_opens_only() {
    for bit in [0u8, 1, 2, 4, 5, 6, 7] {
        let (mut env, pm, q) = setup(PammCoin::Legacy261);
        let mut b = env.borrower();
        let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, q / 2);
        pamm::set_disable_flags(&mut env.svm, 1 << bit);
        env.warp(1);
        crank(&mut env, &pm);
        assert_eq!(
            market(&env, &pm).no_route_since,
            0,
            "bit {bit}: no S3 chain"
        );
        let mut b2 = env.borrower();
        let (res, _) = env.open(&pm, &mut b2, q / 2);
        assert_err(&res, E::VenueFlagged);
        env.warp(1);
        close_ok(&mut env, &pm, &b, nonce);
    }
}

// ---- T-ATA and T-PDA-USER --------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Inner {
    program: Pubkey,
    accounts: Vec<Pubkey>,
    height: u8,
}

/// The inner instructions of instruction `at` of `ixs`, sent by the harness payer.
fn inner(env: &Env, ixs: &[Instruction], at: usize, meta: &TransactionMetadata) -> Vec<Inner> {
    let msg = Message::new(ixs, Some(&env.payer.pubkey()));
    let keys = msg.account_keys;
    meta.inner_instructions
        .get(at)
        .map(|list| {
            list.iter()
                .map(|i| Inner {
                    program: keys[usize::from(i.instruction.program_id_index)],
                    accounts: i
                        .instruction
                        .accounts
                        .iter()
                        .map(|a| keys[usize::from(*a)])
                        .collect(),
                    height: i.stack_height,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// T-ATA and T-PDA-USER (pAMM): the open's `sell` and the close's `buy` invoke PumpSwap at height 2
/// with the trader PDA as `user` (slot 1), `trader_base_ata` (slot 5) and the ephemeral
/// `trader_wsol` — a PDA of short.win, not `ATA(trader, WSOL)` — as `user`'s WSOL account (slot
/// 6); PumpSwap takes both (the legs execute to the lamport), nothing runs deeper than height 3.
#[test]
fn t_ata_and_t_pda_user_the_trader_pda_trades_with_a_non_ata_wsol_account() {
    let (mut env, pm, q) = setup(PammCoin::Mature);
    let trader = pm.trader();
    let trader_wsol = pda::trader_wsol(&pm.market()).0;
    assert_ne!(trader_wsol, wsol_ata(&trader), "not an ATA");
    let mut b = env.borrower();
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
    let ixs = [cu_limit_ix(pamm::PAMM_TRADE_CU), open];
    let kp = b.kp.insecure_clone();
    let meta = ok(env.send(&ixs, &[&kp]));
    b.next_nonce = 1;
    {
        let label = "sell";
        let calls = inner(&env, &ixs, 1, &meta);
        let pamm_calls: Vec<&Inner> = calls
            .iter()
            .filter(|i| i.program == PUMP_AMM_PROGRAM_ID && i.height == 2)
            .collect();
        assert_eq!(pamm_calls.len(), 1, "{label}: one PumpSwap call");
        assert!(
            calls
                .iter()
                .filter(|i| i.program == PUMP_AMM_PROGRAM_ID && i.height != 2)
                .all(|i| i.height == 3),
            "{label}: PumpSwap's own event self-CPI one level down"
        );
        let c = pamm_calls[0];
        assert_eq!(c.height, 2);
        assert_eq!(c.accounts[1], trader, "{label}: user = the trader PDA");
        assert_eq!(c.accounts[5], pm.trader_base_ata);
        assert_eq!(
            c.accounts[6], trader_wsol,
            "{label}: the non-ATA trader_wsol"
        );
        assert!(calls.iter().all(|i| i.height <= 3), "{label}: height ≤ 3");
    }
    env.warp(1);
    crank(&mut env, &pm);
    let close = close_short_ix(&pm, &b.key(), 0, u64::MAX, ExtrasSel::NONE, &[]);
    let ixs = [cu_limit_ix(pamm::PAMM_TRADE_CU), close];
    let lamports0 = lamports(&env, &trader);
    assert_eq!(
        lamports0, 0,
        "the trader PDA holds nothing between instructions"
    );
    let meta = ok(env.send(&ixs, &[&kp]));
    let calls = inner(&env, &ixs, 1, &meta);
    let c = calls
        .iter()
        .find(|i| i.program == PUMP_AMM_PROGRAM_ID && i.height == 2)
        .expect("PumpSwap buy");
    assert_eq!(
        (c.height, c.accounts[1], c.accounts[6]),
        (2, trader, trader_wsol)
    );
    assert_eq!(
        c.accounts[20],
        pm.trader_uva(),
        "buy: the trader's pAMM UVA"
    );
    assert!(calls.iter().all(|i| i.height <= 3), "buy: height ≤ 3");
    assert_eq!(lamports(&env, &trader), 0);
}

// ---- T-RENT ----------------------------------------------------------------------------------------

/// T-RENT (pAMM) on a 301-byte pool: the borrower pays exactly the margin and the open fee (its
/// `ert` and the WSOL account's rent come back to the lamport), `Open.venue_rent` is 0, and the
/// close's charge is PumpSwap's quote exactly.
#[test]
fn t_rent_no_venue_rent_on_a_full_size_pool() {
    let (mut env, pm, q) = setup(PammCoin::Mature);
    let mut b = env.borrower();
    let before = lamports(&env, &b.key());
    let (open, _, _) = open_ok(&mut env, &pm, &mut b, q);
    let pos = env.position(&pm, &b.key(), open.nonce);
    let rent_pos = env
        .svm
        .minimum_balance_for_rent_exemption(ShortPosition::SPACE);
    let exposure = env
        .svm
        .get_account(&pda::exposure(&pm.market(), &b.key()).0)
        .unwrap()
        .lamports;
    let paid = before - lamports(&env, &b.key());
    assert_eq!(open.venue_rent, 0);
    assert_eq!(
        paid,
        open.m + open.fee_open + rent_pos + exposure,
        "margin, open fee and the two records' rent: nothing else (the harness payer pays the fee)"
    );
    assert!(pos.ert > 0);
}

/// T-RENT (pAMM) on the 211-byte pool held at `minimum_balance(211)` (the fixture's is
/// over-funded at the old rent: its rent is lowered to today's minimum, a plant of the venue's
/// account): PumpSwap grows it to 271 bytes on the sale and tops its rent up from the trader —
/// `minimum_balance(271) − minimum_balance(211)` = 304,800 — which the leg nets out of `S`
/// (`S = sol_leg − 304,800`), reports in `Open.venue_rent`, and refunds to the borrower (the
/// venue took it out of the borrower's `ert`). As on the curve (T-RENT's curve part), an open
/// whose `S_exp` tolerance (`exec_tol_bps`) cannot cover the growth fails `ProceedsBelowMin`, the
/// pool untouched; the growth is paid once.
#[test]
fn t_rent_a_short_pool_grows_on_the_sale() {
    let (mut env, p) = Env::fork_pamm_coin(PammCoin::Legacy211);
    let r = p.reserves(&env.svm).unwrap();
    // 1.8 % of the pool's 934.9 M tokens (19.13 SOL): a sale of ≈ 0.33 SOL, inside Tier C's
    // impact and wallet caps, whose 10-bps tolerance (≈ 330,000) covers the growth.
    assert!(r.effective_quote < 20 * u128::from(SOL));
    let q = r.base / 1_000 * 18;
    let pm = list_on(&mut env, &p, q * 4);
    let min211 = env.svm.minimum_balance_for_rent_exemption(211);
    let min271 = env.svm.minimum_balance_for_rent_exemption(271);
    let mut acc = env.svm.get_account(&p.pool).unwrap();
    assert_eq!(acc.data.len(), 211);
    acc.lamports = min211;
    env.svm.set_account(p.pool, acc).unwrap();
    let tol = u64::from(env.config().params.exec_tol_bps);
    // A sale whose tolerance is below the growth.
    let small = q / 20;
    let s_small = quote_sell(&env, &p, small).expect("quote");
    assert!(s_small * tol / 10_000 < 304_800);
    let mut b = env.borrower();
    let (res, _) = env.open(&pm, &mut b, small);
    assert_err(&res, E::ProceedsBelowMin);
    assert_eq!(env.svm.get_account(&p.pool).unwrap().data.len(), 211);
    env.warp(1);
    let s_exp = quote_sell(&env, &p, q).expect("quote");
    assert!(
        s_exp * tol / 10_000 >= 304_800,
        "the tolerance covers the growth"
    );
    let (open, _, _) = open_ok(&mut env, &pm, &mut b, q);
    let growth = min271 - min211;
    assert_eq!(growth, 304_800);
    assert_eq!(open.venue_rent, growth);
    assert_eq!(open.s, s_exp - growth, "S net of the venue-side rent");
    let acc = env.svm.get_account(&p.pool).unwrap();
    assert_eq!((acc.data.len(), acc.lamports), (271, min271));
    assert_trader_clean(&env, &pm);
}

// ---- T-CASHBACK --------------------------------------------------------------------------------------

/// T-CASHBACK (pAMM part) and M5-SEC-1's remainder, on the dumped cashback coin: a position's
/// open and close accrue cashback in `ATA(pAMM UVA(trader), WSOL)`; a full wipe (the only lender
/// takes out every free token, a 5× gap settles the last position S1); `sweep_trader` alone sets
/// only the trader bit while the cashback is outstanding, so a deposit waits (`WipeSweepPending`);
/// with the venue halted for buys, `[create_idempotent ATA(trader, WSOL), claim_cashback(user =
/// trader), sweep_trader]` — no signature but the fee payer's — moves the cashback through
/// `ATA(trader, WSOL)` into the wiped epoch's record and sets both bits; the deposit lands.
#[test]
fn t_cashback_pumpswap_cashback_reaches_the_wiped_lenders() {
    let (mut env, pm, q) = setup(PammCoin::Cashback);
    let market_key = pm.market();
    let mut b = env.borrower();
    let (_, n0, _) = open_ok(&mut env, &pm, &mut b, q);
    env.warp(1);
    crank(&mut env, &pm);
    close_ok(&mut env, &pm, &b, n0);
    let cashback = tokens(&env, &pm.cashback_ata());
    assert!(cashback > 0, "the trader's trades accrued cashback");

    // The full wipe: a position, every free token withdrawn, a 5× gap, S1.
    let mut b2 = env.borrower();
    let (_, n2, _) = open_ok(&mut env, &pm, &mut b2, q);
    withdraw_all_free(&mut env, &pm);
    let whale = PammUser::new(&mut env, &pm.pool, 1_000 * SOL);
    ok(whale.push_price(&mut env, &pm.pool, 5, 1));
    let kp = keeper(&mut env);
    let mut s1 = None;
    for _ in 0..120 {
        env.warp(1);
        crank(&mut env, &pm);
        let meta = ok(settle(&mut env, &pm, &kp, &b2.key(), n2));
        if let Some(ev) = events::of::<SolSettle>(&meta).pop() {
            s1 = Some(ev);
            break;
        }
    }
    assert_eq!(s1.expect("settled").condition, 1, "S1");
    let m = env.market(&market_key);
    assert_eq!(m.total_shares.get(), 0);
    assert!(m.has_flag(MARKET_FLAG_WIPE_LIVE), "a full wipe");
    let cashback = tokens(&env, &pm.cashback_ata());
    assert!(cashback > 0);

    // The sweep alone: the trader bit only; a deposit waits.
    let meta = ok(env.send(&[pamm::sweep_trader_pamm_ix(&env, &pm)], &[]));
    let ev: TraderSwept = events::one(&meta);
    assert_eq!(ev.wipe_bits & WIPE_TRADER_SWEPT, WIPE_TRADER_SWEPT);
    assert_eq!(
        ev.wipe_bits & WIPE_REWARDS_CLAIMED,
        0,
        "cashback outstanding"
    );
    let amount = q;
    let lender2 = env.funded_keypair();
    let acc = fork::fund_token(
        &mut env,
        &lender2.pubkey(),
        &pm.keys.mint,
        pm.pool.token_program(),
        amount,
    );
    let k = ix::LenderKeys {
        owner: lender2.pubkey(),
        mint: pm.keys.mint,
        token_program: pm.keys.token_program,
        owner_token_account: acc,
    };
    let deposit = |env: &Env| {
        let m = env.market(&market_key);
        ix::with_remaining(
            ix::deposit(&k, amount, 0),
            &ix::epoch_records(&market_key, 0, m.share_epoch),
        )
    };
    let d0 = deposit(&env);
    assert_err(&env.send(&[d0], &[&lender2]), E::WipeSweepPending);

    // The unlocking bundle on a venue halted for buys, signed by the fee payer alone.
    pamm::set_disable_flags(&mut env.svm, 1 << 3);
    let trader = pm.trader();
    let dest = wsol_ata(&trader);
    let payer = env.payer.pubkey();
    let claim = pamm::pamm_claim_cashback_ix(&trader, &dest);
    assert!(claim.accounts.iter().all(|a| !a.is_signer));
    let lender_sol0 = env.market_vault(&market_key).lender_sol;
    let ata_rent = env.svm.minimum_balance_for_rent_exemption(165);
    let meta = ok(env.send(
        &[
            fork::create_ata_idempotent_ix(&payer, &trader, &WSOL_MINT, &TOKEN_PROGRAM_ID),
            claim,
            pamm::sweep_trader_pamm_ix(&env, &pm),
        ],
        &[],
    ));
    let ev: TraderSwept = events::one(&meta);
    assert_eq!(
        ev.lamports,
        cashback + ata_rent,
        "the cashback and the bundler's ATA rent, through the trader"
    );
    assert_eq!(
        ev.wipe_bits & (WIPE_TRADER_SWEPT | WIPE_REWARDS_CLAIMED),
        WIPE_TRADER_SWEPT | WIPE_REWARDS_CLAIMED
    );
    assert_eq!(tokens(&env, &pm.cashback_ata()), 0);
    assert!(
        env.market(&market_key)
            .has_flag(MARKET_FLAG_WIPE_REWARDS_CLAIMED)
    );
    assert_eq!(
        env.market_vault(&market_key).lender_sol - lender_sol0,
        cashback + ata_rent,
        "credited to the wiped epoch's record"
    );
    pamm::set_disable_flags(&mut env.svm, 0);
    let d1 = deposit(&env);
    ok(env.send(&[d1], &[&lender2]));
    assert!(!env.market(&market_key).has_flag(MARKET_FLAG_WIPE_LIVE));
}

/// A non-cashback market's full wipe needs no claim: the pAMM UVA's WSOL ATA does not exist (the
/// normal state), so `sweep_trader` alone sets both bits — whether or not the venue executes.
#[test]
fn t_cashback_nothing_to_claim_sets_both_bits_at_once() {
    let (mut env, pm, q) = setup(PammCoin::Legacy261);
    assert!(
        env.svm
            .get_account(&pm.cashback_ata())
            .is_none_or(|a| a.data.is_empty())
    );
    let mut b = env.borrower();
    let (_, n, _) = open_ok(&mut env, &pm, &mut b, q);
    withdraw_all_free(&mut env, &pm);
    let whale = PammUser::new(&mut env, &pm.pool, 1_000 * SOL);
    ok(whale.push_price(&mut env, &pm.pool, 5, 1));
    let kp = keeper(&mut env);
    let mut done = false;
    for _ in 0..120 {
        env.warp(1);
        crank(&mut env, &pm);
        let meta = ok(settle(&mut env, &pm, &kp, &b.key(), n));
        if !events::of::<SolSettle>(&meta).is_empty() {
            done = true;
            break;
        }
    }
    assert!(done, "settled");
    assert!(env.market(&pm.market()).has_flag(MARKET_FLAG_WIPE_LIVE));
    pamm::set_disable_flags(&mut env.svm, 1 << 3);
    let meta = ok(env.send(&[pamm::sweep_trader_pamm_ix(&env, &pm)], &[]));
    let ev: TraderSwept = events::one(&meta);
    assert_eq!(
        ev.wipe_bits & (WIPE_TRADER_SWEPT | WIPE_REWARDS_CLAIMED),
        WIPE_TRADER_SWEPT | WIPE_REWARDS_CLAIMED
    );
}

/// The seeding lender withdraws every free token (every share whose tokens `T` covers).
fn withdraw_all_free(env: &mut Env, pm: &PammMarket) {
    let m = env.market(&pm.market());
    let pool = m.pool();
    let want = m.tokens_free;
    let lender = env
        .lender(&pm.market(), &pm.lender.pubkey())
        .expect("lender");
    let (mut lo, mut hi) = (0u128, lender.shares);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if shortwin_math::pool::tokens_for_shares(&pool, mid).unwrap() <= u128::from(want) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let kp = pm.lender.insecure_clone();
    ok(env.send(&[ix::withdraw(&pm.lender_keys(), lo, want)], &[&kp]));
    assert_eq!(env.market(&pm.market()).tokens_free, 0);
}
