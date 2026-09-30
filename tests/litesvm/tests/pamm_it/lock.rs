//! ★T-PAMM-LOCK (ARCH §8.2, changelog 56; ECON #105, §12.12 (12); SPEC §14 M6R-1, the owner's
//! decision of 2026-09-30): third-party LP on canonical PumpSwap pools (R-VEN Update H11) against
//! the Raydium liquidity-push protections, on the fork with the `pamm_state` pools and pAMM's own
//! `deposit` / `withdraw` ([`PammLp`]).
//!
//! The pool's **locked depth** is Raydium's metric on its burned creation LP: `D =
//! ⌊λ_bps·isqrt(X·(Y + VQR))/10⁴⌋`, `λ_bps = ⌊(Pool.lp_supply − lp_mint.supply)·10⁴/lp_supply⌋`,
//! read with the LP mint as the ReadSet's P6 and written into `pool_d` by every executable
//! observation. It bounds the size cap (`X₀ ≤ D/(λ_min·√P_ema)`), the tier (`R_eff ≤
//! (locked SOL − VQR⁺)/λ_min`) and the exit reference (`L_b`'s EMA terms `≤ ⌊D·10⁴/λ_min⌋`).
//!
//! | Test | What (`attacks/pumpswap_lp.py`) |
//! |---|---|
//! | [`u33_pool_d_is_the_burned_lp_depth_on_every_pool_role`] | U33: `pool_d = D` recomputed from the pool and its LP mint on every pool role (e/acc's `mature` at λ 92.98 %; the others 100 %, `D = √k`), equal to `pool_select::locked_depth` |
//! | [`the_lp_mint_p6_is_key_checked`] | a crank or an open naming another account at P6 fails `VenueAccountMismatch`; without P6 `BadRemainingAccounts` |
//! | [`a_deposit_and_its_withdrawal_move_neither_d_nor_the_bounds`] | a deposit of 3× the pool drops λ to 25 % and leaves `D`, the size cap's `D/λ_min` term, the tier's bound and the reference cap within 1 bp; the withdrawal restores λ |
//! | [`part_3c_a_deposit_wrapped_around_the_open_inflates_nothing`] | the deposit-at-open shape: `[deposit 3×, open_short, withdraw]` opens the honest cap (1.00×, within 1 bp: the EMA state's own reserve in place of `X_now`) |
//! | [`part_3b_a_held_deposit_bounds_the_next_open_at_lambda_over_lambda_min`] | a deposit of 3× held 1 h: the next open's cap is 2.00× the honest one (`λ/λ_min` of `D`; without the `D` term 2.84×) — a bound, not a safe size: part 3 (e) |
//! | [`part_3a_a_held_deposit_pulled_at_a_pump_does_not_hold_up_the_liquidation`] | M4F-1's shape for a position sized **before** the deposit (95 % of the honest cap): a deposit of 3× held 1 h and pulled as a genuine 3×/120 s pump starts; `L_b` capped at 2× the depth, the position is liquidated in the control's second (as built it stayed open) |
//! | [`part_3e_a_held_deposit_that_sized_the_open_pulled_at_a_pump_gaps_into_s1`] | **M6D-1 (OWNER, open; pinned as built)**: the same deposit held 1 h **sizes** the open (95 % of the 2.00× cap) and leaves in the first second of a genuine 5×/120 s pump: `L_slow` keeps `L_b` at the cap `⌊D·10⁴/λ_min⌋`, every liquidation is refused and the position gaps into **S1** — the lenders below `Q`'s value at the spot, the borrower zeroed (the control, the deposit kept, is liquidated); on a config with `λ_min` = 100 % (option (a) on this PumpSwap-only fork) the held cap is the honest one and the same pump liquidates |
//! | [`part_2_a_deposit_before_the_transition_is_capped_at_d_by_the_first_observation`] | `[migrate_v2, deposit 3×, crank_price]`: the transition's `lp_supply` anchor is inflated, `L_slow` keeps the curve's (≈ 1.35× the depth); the first PumpSwap observation caps it at `D` (1.000; as built 1.353 for an hour) |
//! | [`part_3d_fits2_a_withdraw_wrapped_liquidation_forces_nothing`] | FITS-2 (a) on a pool with 50 % third-party LP: a position just below its threshold, a genuine spike and retreat (the EMA leg true, the spot leg's veto); the attacker's `[withdraw all, liquidate, deposit]` each second is refused `NotLiquidatable`, although at the drained `L_live` alone (#100) the spot leg was true. FITS-2 (b), the keeper's per-second coverage against the withdraw drag, is the keeper's (`services/keeper-litesvm`, `pamm_lp::fits2_b_*`) |
//! | [`fitx1_a_held_drain_then_a_freeze_forces_what_a_withdrawal_for_good_does`] | FITX-1 (inherited residual, ECON §12.12 residual 16): an LP that withdraws for real for 20 s and then withdraw-wraps a crank and the liquidation every second forces exactly the liquidation the same LP's withdrawal for good forces (the same second and buyback cost) |

use shortwin_litesvm_tests::invariants::venue_view;
use shortwin_litesvm_tests::pamm::PammLp;
use shortwin_math::caps;
use shortwin_math::pool_select;
use solana_keypair::Keypair;

use crate::common::*;

const SPL261: PammCoin = PammCoin::Legacy261;

/// `pool_lock_min_bps` of the harness's config.
fn lambda_min(env: &Env) -> u32 {
    env.config().params.pool_lock_min_bps
}

/// The `legacy261` market (λ 100 %, SPL Token) on a config with the per-market notional limit and
/// the wallet cap lifted within their compiled bounds (`market_max_notional` 10¹³, `wallet_cap_bps`
/// 10⁴, as `raydium_v4_support::lift_notional`: the beta's 25-SOL notional limit binds first
/// otherwise, and these tests compare the size caps on `X₀`), `400·q` lent, warmed up, the boost
/// waited out, and the pool grown from 261 to 271 bytes by a small trade (pAMM's `deposit`
/// serializes the pool at 271 bytes: on a shorter one it fails until a trade grows it, R-VEN
/// Update H8). Returns it and `q`.
fn lock_setup() -> (Env, PammMarket, u64) {
    lock_setup_lambda(None)
}

/// [`lock_setup`] with `pool_lock_min_bps` set to `lambda_min` (within its compiled bounds
/// [2,000, 10,000]); on this PumpSwap-only fork 10,000 stands for OWNER-M6D-1's option (a),
/// `λ_min` = 100 % in PumpSwap sizing (part 3 (e)).
fn lock_setup_lambda(lambda_min: Option<u32>) -> (Env, PammMarket, u64) {
    let (mut env, p) = Env::fork_pamm_coin_with(SPL261, |a| {
        a.params.market_max_notional = 10_000_000_000_000;
        a.params.wallet_cap_bps = 10_000;
        if let Some(l) = lambda_min {
            a.params.pool_lock_min_bps = l;
        }
    });
    let q = q_for(&env, &p);
    let pm = list_on(&mut env, &p, q.saturating_mul(400));
    wait_out_boost(&mut env, &pm);
    let u = PammUser::new(&mut env, &pm.pool, 10 * SOL);
    ok(u.buy(&mut env, &pm.pool, q / 1_000, u64::MAX));
    assert!(
        env.svm.get_account(&pm.pool.pool).expect("pool").data.len()
            >= shortwin_venue_state::pump_amm::POOL_LEN_CREATE
    );
    env.warp(1);
    crank(&mut env, &pm);
    (env, pm, q)
}

/// Whether `ixs` (with a 1.4 M CU limit) lands on a clone of the VM — nothing is kept, no hook runs.
fn lands(env: &Env, ixs: &[Instruction], signers: &[&Keypair]) -> Result<(), Option<u32>> {
    let mut all = vec![cu_limit_ix(1_400_000)];
    all.extend_from_slice(ixs);
    let tx = env.transaction(&all, signers);
    let mut svm = env.svm.clone();
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|f| match f.err {
            solana_transaction_error::TransactionError::InstructionError(
                _,
                solana_instruction::error::InstructionError::Custom(c),
            ) => Some(c),
            _ => None,
        })
}

/// The open of `q` by `b` at its next nonce, bounds of its own off.
fn open_ix(pm: &PammMarket, b: &Borrower, q: u64) -> Instruction {
    open_short_ix(
        pm,
        &b.key(),
        b.next_nonce,
        q,
        0,
        u64::MAX,
        u64::MAX,
        ExtrasSel::NONE,
        &[],
    )
}

/// The largest `q` a first open of `b` lands with now — alone, or wrapped in `[deposit lp_out,
/// open, withdraw lp_out]` by `lp` — by bisection on clones of the VM; every refusal above it is a
/// size cap.
fn max_open(env: &Env, pm: &PammMarket, b: &Borrower, wrap: Option<(&PammLp, u64)>) -> u64 {
    let attempt = |q: u64| {
        let mut ixs = Vec::new();
        let mut signers: Vec<&Keypair> = vec![&b.kp];
        if let Some((lp, n)) = wrap {
            ixs.push(lp.deposit_ix(&pm.pool, n));
            signers.push(&lp.kp);
        }
        ixs.push(open_ix(pm, b, q));
        if let Some((lp, n)) = wrap {
            ixs.push(lp.withdraw_ix(&pm.pool, n));
        }
        lands(env, &ixs, &signers)
    };
    let caps = [
        code(E::CapPositionImpact),
        code(E::CapMarketOi),
        code(E::CapWallet),
        code(E::CapUtilization),
        code(E::InsufficientInventory),
    ];
    // From 0.2 SOL of notional (the default `min_notional` is 0.1 SOL) to a quarter of the pool.
    let r = pm.pool.reserves(&env.svm).expect("reserves");
    let lo = u64::try_from(u128::from(r.base) * 200_000_000 / r.effective_quote).expect("u64");
    let (mut lo, mut hi) = (lo, r.base / 4);
    assert!(attempt(lo).is_ok(), "a tiny open lands");
    let over = attempt(hi).expect_err("a quarter of the pool is over the caps");
    assert!(over.is_some_and(|c| caps.contains(&c)), "refused {over:?}");
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        match attempt(mid) {
            Ok(()) => lo = mid,
            Err(c) => {
                assert!(
                    c.is_some_and(|c| caps.contains(&c)),
                    "q {mid}: refused {c:?}"
                );
                hi = mid;
            }
        }
    }
    lo
}

/// The size cap's conservative reserve now, with and without the `D/λ_min` term (M6R-1), from the
/// market's stored state (`L_slow`, `P_ema`, `pool_d`) and the pool's live `X`.
fn x0_now(env: &Env, pm: &PammMarket) -> (u64, u64) {
    let m = market(env, pm);
    let x = pm.pool.reserves(&env.svm).expect("reserves").base;
    let (slow, p) = (m.liq_slow.get(), m.ema_price.get());
    let with =
        caps::conservative_x0(x, slow, p, Some((m.pool_d.get(), lambda_min(env)))).expect("X0");
    let without = caps::conservative_x0(x, slow, p, None).expect("X0");
    (with, without)
}

// ---- U33 and the ReadSet ---------------------------------------------------------------------------

/// U33 (ARCH §12; R-VEN Update H11): the program's `pool_d` after a crank is the pool's locked
/// depth recomputed here from `Pool.lp_supply` and the LP mint's supply on `(X, Y + VQR)` — and the
/// math crate's `pool_select::locked_depth` — on every pool role: e/acc's `mature` pool with
/// third-party LP at λ 92.98 %, every other role's creation LP wholly burned (`D = √k`).
#[test]
fn u33_pool_d_is_the_burned_lp_depth_on_every_pool_role() {
    for coin in PammCoin::ALL {
        let (mut env, pm, _) = setup(coin);
        env.warp(1);
        crank(&mut env, &pm);
        let (lam, d) = pm.pool.locked_depth(&env.svm);
        let st = pm.pool.state(&env.svm);
        let r = pm.pool.reserves(&env.svm).expect("reserves");
        let y = u64::try_from(r.effective_quote).expect("u64");
        let root = (u128::from(r.base) * r.effective_quote).isqrt();
        assert_eq!(
            pool_select::locked_depth(st.lp_supply, pm.pool.lp_mint_supply(&env.svm), r.base, y),
            Ok(d),
            "{coin:?}"
        );
        assert_eq!(market(&env, &pm).pool_d.get(), d, "{coin:?}: pool_d");
        match coin {
            PammCoin::Mature => assert_eq!(lam, 9_298, "e/acc"),
            _ => assert_eq!((lam, d), (10_000, root), "{coin:?}: all burned"),
        }
        eprintln!("U33 {coin:?}: λ {lam} bps, D {d}, √k {root}");
    }
}

/// P6 is key-checked against the live `pool.lp_mint` (ARCH §3.12): another account there —
/// another mint, WSOL — fails `VenueAccountMismatch` on a crank and on an open; a ReadSet without
/// it is `BadRemainingAccounts`.
#[test]
fn the_lp_mint_p6_is_key_checked() {
    let (mut env, pm, q) = setup(SPL261);
    env.warp(1);
    let p6 = |ix: &mut Instruction, key: Pubkey| {
        let at = ix
            .accounts
            .iter()
            .position(|a| a.pubkey == pm.pool.lp_mint)
            .expect("P6 in the ReadSet");
        ix.accounts[at].pubkey = key;
    };
    for other in [
        pm.keys.mint,
        shortwin::constants::WSOL_MINT,
        Pubkey::new_unique(),
    ] {
        let mut ix = crank_price_ix(&pm);
        p6(&mut ix, other);
        assert_err(&env.send(&[ix], &[]), E::VenueAccountMismatch);
        let b = env.borrower();
        let mut ix = open_ix(&pm, &b, q);
        p6(&mut ix, other);
        let kp = b.kp.insecure_clone();
        assert_err(
            &env.send(&[cu_limit_ix(pamm::PAMM_TRADE_CU), ix], &[&kp]),
            E::VenueAccountMismatch,
        );
    }
    let mut ix = crank_price_ix(&pm);
    let at = ix
        .accounts
        .iter()
        .position(|a| a.pubkey == pm.pool.lp_mint)
        .expect("P6");
    ix.accounts.remove(at);
    assert_err(&env.send(&[ix], &[]), E::BadRemainingAccounts);
    crank(&mut env, &pm);
}

// ---- deposits and withdrawals ----------------------------------------------------------------------

/// A deposit of 3× the pool (λ 100 % → 25 %) and its withdrawal leave the locked depth `D` — and
/// with it the size cap's `D/λ_min` term, the tier's locked-SOL bound and the reference cap
/// `⌊D·10⁴/λ_min⌋` — unchanged within 1 bp (λ's whole-bps floor), while `√k` quadruples and comes
/// back.
#[test]
fn a_deposit_and_its_withdrawal_move_neither_d_nor_the_bounds() {
    let (mut env, pm, _) = lock_setup();
    let lp = PammLp::funded_for(&mut env, &pm.pool, 30_000);
    let lam_min = u128::from(lambda_min(&env));
    let read = |env: &mut Env| {
        env.warp(1);
        crank(env, &pm);
        let m = market(env, &pm);
        let (lam, d) = pm.pool.locked_depth(&env.svm);
        assert_eq!(m.pool_d.get(), d);
        let r = pm.pool.reserves(&env.svm).expect("reserves");
        let root = (u128::from(r.base) * r.effective_quote).isqrt();
        let p = m.ema_price.get();
        let term = u128::from(
            caps::conservative_x0(u64::MAX, u128::MAX, p, Some((d, lambda_min(env)))).expect("X0"),
        );
        let locked = shortwin_math::tier::locked_sol(d, p);
        (lam, d, root, term, locked, d * 10_000 / lam_min)
    };
    let within_bp = |a: u128, b: u128| a.abs_diff(b) * 10_000 <= b;
    let (lam0, d0, root0, term0, locked0, cap0) = read(&mut env);
    assert_eq!(lam0, 10_000);
    let minted = lp.deposit(&mut env, &pm.pool, 30_000);
    let (lam1, d1, root1, term1, locked1, cap1) = read(&mut env);
    eprintln!(
        "deposit 3x: λ {lam0} -> {lam1} bps, √k ×{:.4}, D ×{:.6}",
        root1 as f64 / root0 as f64,
        d1 as f64 / d0 as f64
    );
    assert_eq!(lam1, 2_500);
    assert!(root1 > 3 * root0);
    for (a, b, what) in [
        (d1, d0, "D"),
        (term1, term0, "X0's D term"),
        (locked1, locked0, "the tier's locked SOL"),
        (cap1, cap0, "the reference cap"),
    ] {
        assert!(within_bp(a, b), "{what}: {b} -> {a}");
    }
    assert_eq!(lp.lp_balance(&env.svm), minted);
    lp.withdraw_all(&mut env, &pm.pool);
    let (lam2, d2, _, _, _, _) = read(&mut env);
    assert_eq!(lam2, 10_000);
    assert!(within_bp(d2, d0), "D after the withdrawal: {d0} -> {d2}");
}

/// Part 3 (c), the deposit-at-open shape (ARCH §4.6 "not inflatable by a deposit at the open"):
/// `[deposit 3× the pool, open_short, withdraw]` in one transaction opens what the same borrower
/// opens without it (1.00×) — `X₀ = min(X_now, L_slow/√P_ema, D/(λ_min·√P_ema))` drops the
/// deposit-inflated `X_now`, and neither `L_slow` nor `D` moves within the transaction; what is
/// left is the EMA state's own reserve `L_slow/√P_ema`, within 1 bp of the live `X` here.
#[test]
fn part_3c_a_deposit_wrapped_around_the_open_inflates_nothing() {
    let (mut env, pm, _) = lock_setup();
    env.warp(1);
    crank(&mut env, &pm);
    let lp = PammLp::funded_for(&mut env, &pm.pool, 30_000);
    let n = PammLp::lp_for(&env.svm, &pm.pool, 30_000);
    let b = env.borrower();
    let honest = max_open(&env, &pm, &b, None);
    let wrapped = max_open(&env, &pm, &b, Some((&lp, n)));
    let x = pm.pool.reserves(&env.svm).expect("reserves").base;
    eprintln!(
        "3 (c): honest cap q/X {:.3} %, wrapped in a 3x deposit {:.3} % ({} more units: the \
         EMA reprojection L_slow/√P_ema in place of X_now)",
        honest as f64 * 100.0 / x as f64,
        wrapped as f64 * 100.0 / x as f64,
        wrapped.saturating_sub(honest)
    );
    // 1.00×: the deposit lifts `X_now` out of the minimum, leaving the EMA state's own reserve
    // `L_slow/√P_ema` (within 1 bp of the live `X` here), never the deposit.
    assert!(wrapped >= honest && (wrapped - honest) * 10_000 <= honest);
    // The wrapped open at the cap executes (the hooks after every instruction).
    let kp = b.kp.insecure_clone();
    let lk = lp.kp.insecure_clone();
    let meta = ok(env.send(
        &[
            cu_limit_ix(1_400_000),
            lp.deposit_ix(&pm.pool, n),
            open_ix(&pm, &b, wrapped),
            lp.withdraw_ix(&pm.pool, n),
        ],
        &[&kp, &lk],
    ));
    let open: Open = events::one(&meta);
    assert_eq!(open.q, wrapped);
}

/// Part 3 (b): a deposit of 3× the pool held for an hour (a crank every minute) pushes `L_slow`
/// (τ = 1 h) to ≈ 2.84× the pool's depth; an open while it is held is sized on `X₀ = D/(λ_min·√P)`,
/// 2× the honest reserve at λ_min 50 % — the most sizing ever credits the burned LP with (as built
/// `L_slow/√P`, 2.84× here; `attacks/pumpswap_lp.py` 3 (b): 2.87×). A bound, not a safe size: a
/// position that large gaps into S1 if the deposit leaves at a genuine pump (part 3 (e), M6D-1).
#[test]
fn part_3b_a_held_deposit_bounds_the_next_open_at_lambda_over_lambda_min() {
    let (mut env, pm, _) = lock_setup();
    env.warp(1);
    crank(&mut env, &pm);
    let b = env.borrower();
    let honest = max_open(&env, &pm, &b, None);
    let x_honest = pm.pool.reserves(&env.svm).expect("reserves").base;
    let lp = PammLp::funded_for(&mut env, &pm.pool, 30_000);
    lp.deposit(&mut env, &pm.pool, 30_000);
    for _ in 0..60 {
        env.warp(60);
        crank(&mut env, &pm);
    }
    let (with_d, without_d) = x0_now(&env, &pm);
    let held = max_open(&env, &pm, &b, None);
    let ratio = held as f64 / honest as f64;
    eprintln!(
        "3 (b): after 1 h held X0 {:.3}x the honest pool's X (without the D term {:.3}x); the cap \
         q {:.3}x the honest one",
        with_d as f64 / x_honest as f64,
        without_d as f64 / x_honest as f64,
        ratio
    );
    assert!((1.98..=2.02).contains(&(with_d as f64 / x_honest as f64)));
    assert!(without_d as f64 / x_honest as f64 > 2.5, "the D term binds");
    assert!((1.9..=2.05).contains(&ratio), "{ratio}");
    let kp = b.kp.insecure_clone();
    let meta = ok(env.send(&[cu_limit_ix(1_400_000), open_ix(&pm, &b, held)], &[&kp]));
    let open: Open = events::one(&meta);
    assert_eq!(open.q, held);
    lp.withdraw_all(&mut env, &pm.pool);
}

/// One run of part 3 (a) from a snapshot: the position `(b, nonce)` open, optionally a deposit of
/// 3× the pool held an hour (a crank a minute) and withdrawn in the second a genuine 3×/120 s pump
/// starts (other users' buys, a crank every second); the keeper tries `liquidate` every second.
/// Returns the liquidation's second after the pump's start and `L_b / √k` read at its first second.
fn pump_run(
    env: &mut Env,
    pm: &PammMarket,
    b: &Pubkey,
    nonce: u64,
    deposit: bool,
) -> (Option<i64>, f64) {
    let lp = deposit.then(|| {
        let lp = PammLp::funded_for(env, &pm.pool, 30_000);
        lp.deposit(env, &pm.pool, 30_000);
        lp
    });
    for _ in 0..60 {
        env.warp(60);
        crank(env, pm);
    }
    if let Some(lp) = &lp {
        env.warp(1);
        lp.withdraw_all(env, &pm.pool);
    }
    let crowd = PammUser::new(env, &pm.pool, 100_000 * SOL);
    let kp = keeper(env);
    let t0 = env.now();
    let mut l_b = 0.0;
    for s in 1..=240i64 {
        env.warp(1);
        if s <= 120 {
            // 3^(1/120) ≈ 1.009197 a second.
            ok(crowd.push_price(env, &pm.pool, 1_009_197, 1_000_000));
        }
        crank(env, pm);
        if s == 1 {
            let m = market(env, pm);
            let obs = m.observation();
            let r = pm.pool.reserves(&env.svm).expect("reserves");
            let root = (u128::from(r.base) * r.effective_quote).isqrt();
            let cap = m.pool_d.get() * 10_000 / u128::from(lambda_min(env));
            let lb = shortwin_math::bounds::ref_liquidity(root, &obs.ema, &obs.ring, Some(cap))
                .expect("L_b");
            l_b = lb as f64 / root as f64;
        }
        match custom_code(&liquidate(env, pm, &kp, b, nonce)) {
            None => return (Some(env.now() - t0), l_b),
            Some(c) => assert!(
                c == code(E::NotLiquidatable) || c == code(E::ExitBoundNotMet),
                "{c}"
            ),
        }
    }
    (None, l_b)
}

/// Part 3 (a), M4F-1's shape on PumpSwap: a position at 95 % of its cap, sized **before** the
/// deposit; a deposit of 3× the pool held an hour lifts `L_ema` and `L_slow` with it, and is pulled
/// as a genuine 3×/120 s pump starts. The reference `L_b = max(L_live, min(max(L_ema', L_slow),
/// ⌊D·10⁴/λ_min⌋))` credits at most 2× the pool's depth (as built 4×: the position stayed open,
/// ECON §12.12 (12)), and the position is liquidated in the control's second (no deposit). This
/// covers positions sized on the burned LP's depth; one sized while the deposit is held (up to
/// 2× that) is part 3 (e)'s, which the 2× reference does not liquidate (M6D-1).
#[test]
fn part_3a_a_held_deposit_pulled_at_a_pump_does_not_hold_up_the_liquidation() {
    let (mut env, pm, _) = lock_setup();
    env.warp(1);
    crank(&mut env, &pm);
    let mut b = env.borrower();
    let cap = max_open(&env, &pm, &b, None);
    let (_, nonce, _) = open_ok(&mut env, &pm, &mut b, cap / 100 * 95);
    let snap = env.svm.clone();
    let (control, lb_control) = pump_run(&mut env, &pm, &b.key(), nonce, false);
    env.svm = snap;
    let (attacked, lb_attacked) = pump_run(&mut env, &pm, &b.key(), nonce, true);
    eprintln!(
        "3 (a): control liquidated +{control:?} s (L_b/depth {lb_control:.2}); deposit 3x held 1 h, \
         pulled at the pump: +{attacked:?} s (L_b/depth {lb_attacked:.2})"
    );
    let control = control.expect("the control is liquidated");
    let attacked = attacked.expect("liquidated under the pulled deposit");
    assert_eq!(attacked, control, "liquidated in the control's second");
    assert!(lb_attacked <= 2.01, "{lb_attacked}");
}

/// Part 3 (e)'s outcome of one run.
#[derive(Debug)]
enum Gap {
    /// Liquidated `s` seconds after the pump started.
    Liquidated(i64),
    /// S1 `at` seconds after the pump started: the lenders' amount, `Q`'s value at the pool's spot
    /// then, the borrower's amount, the position's `V`, and the liquidations refused
    /// `ExitBoundNotMet` before it.
    S1 {
        at: i64,
        lenders: u64,
        q_value: u64,
        borrower: u64,
        v: u64,
        bound_refusals: u32,
    },
    /// Neither within the horizon.
    Open,
}

/// Part 3 (e)'s position: the honest cap measured, then a deposit of 3× the pool by a third-party
/// LP held an hour (a crank a minute), then an open at 95 % of the cap it allows. Returns the cap
/// held / the honest cap, the borrower's key, the nonce and the LP.
fn sized_on_a_held_deposit(env: &mut Env, pm: &PammMarket) -> (f64, Pubkey, u64, PammLp) {
    env.warp(1);
    crank(env, pm);
    let mut b = env.borrower();
    let honest = max_open(env, pm, &b, None);
    let lp = PammLp::funded_for(env, &pm.pool, 30_000);
    lp.deposit(env, &pm.pool, 30_000);
    for _ in 0..60 {
        env.warp(60);
        crank(env, pm);
    }
    let held = max_open(env, pm, &b, None);
    let (_, nonce, _) = open_ok(env, pm, &mut b, held / 100 * 95);
    (held as f64 / honest as f64, b.key(), nonce, lp)
}

/// One run of part 3 (e) from the state [`sized_on_a_held_deposit`] leaves: a genuine 5×/120 s pump
/// (other users' buys, a crank every second); with `lp`, the provider withdraws every unit in the
/// pump's first second. The keeper tries `liquidate`, then `sol_settle`, every second for 300 s.
fn gap_run(env: &mut Env, pm: &PammMarket, b: &Pubkey, nonce: u64, lp: Option<&PammLp>) -> Gap {
    let pos = env.position(pm, b, nonce);
    let crowd = PammUser::new(env, &pm.pool, 100_000 * SOL);
    let kp = keeper(env);
    let mut bound_refusals = 0;
    for s in 1..=300i64 {
        env.warp(1);
        if let (1, Some(lp)) = (s, lp) {
            lp.withdraw_all(env, &pm.pool);
        }
        if s <= 120 {
            // 5^(1/120) ≈ 1.013503 a second.
            ok(crowd.push_price(env, &pm.pool, 1_013_503, 1_000_000));
        }
        crank(env, pm);
        let res = liquidate(env, pm, &kp, b, nonce);
        if res.is_ok() {
            return Gap::Liquidated(s);
        }
        // Not liquidatable yet, the exit bound at the reference, or (once `V` cannot buy back at
        // all) `UseSolSettle`.
        let c = custom_code(&res).expect("a program error");
        assert!(
            [E::NotLiquidatable, E::ExitBoundNotMet, E::UseSolSettle]
                .iter()
                .any(|&e| c == code(e)),
            "+{s} s: {c}"
        );
        bound_refusals += u32::from(c == code(E::ExitBoundNotMet));
        let meta = ok(settle(env, pm, &kp, b, nonce));
        if let Some(ev) = events::of::<SolSettle>(&meta).pop() {
            assert_eq!(ev.condition, 1, "+{s} s: S{}", ev.condition);
            let r = pm.pool.reserves(&env.svm).expect("reserves");
            let q_value = u128::from(pos.q) * r.effective_quote / u128::from(r.base);
            return Gap::S1 {
                at: s,
                lenders: ev.lender_amount,
                q_value: u64::try_from(q_value).expect("u64"),
                borrower: ev.borrower_amount,
                v: pos.v,
                bound_refusals,
            };
        }
    }
    Gap::Open
}

/// Part 3 (e) — **M6D-1, an open owner item, pinned as built** (ECON §12.12 (12) (e) and residual
/// 16; `attacks/pumpswap_lp.py` 3 (e)): part 3 (b)'s held deposit (3× the pool, an hour) **sizes**
/// the open — `X₀`'s `D/λ_min` term allows 2.00× the honest cap at `λ_min` 50 % — and the LP leaves
/// in the first second of a genuine 5×/120 s pump (LPs pulling liquidity as volatility starts is
/// ordinary behaviour; nothing here is foreign). `L_slow` (τ 1 h) still credits the deposit, so
/// the reference `L_b` sits at the cap `⌊D·10⁴/λ_min⌋` = 2× the drained pool's depth: every
/// liquidation the position's CR calls for is refused `ExitBoundNotMet` (the buyback on the drained
/// pool exceeds the bound at the reference), and the position gaps into **S1** — the lenders get
/// `V − K_s`, below `Q`'s value at the spot, the borrower nothing. The control (the deposit kept)
/// is liquidated. On a config with `pool_lock_min_bps` = 10,000 — on this PumpSwap-only fork the
/// owner's option (a), `λ_min` = 100 % in PumpSwap sizing (here the tier and the reference cap read
/// `D` as well; the sim's option keeps the reference cap at `D/λ_min`, with the same exits) — the
/// held deposit sizes nothing (1.00× the honest cap) and the same pulled pump is liquidated.
#[test]
fn part_3e_a_held_deposit_that_sized_the_open_pulled_at_a_pump_gaps_into_s1() {
    let (mut env, pm, _) = lock_setup();
    let (ratio, key, nonce, lp) = sized_on_a_held_deposit(&mut env, &pm);
    let snap = env.svm.clone();
    let control = gap_run(&mut env, &pm, &key, nonce, None);
    env.svm = snap;
    let pulled = gap_run(&mut env, &pm, &key, nonce, Some(&lp));
    let (mut env_a, pm_a, _) = lock_setup_lambda(Some(10_000));
    let (ratio_a, key_a, nonce_a, lp_a) = sized_on_a_held_deposit(&mut env_a, &pm_a);
    let pulled_a = gap_run(&mut env_a, &pm_a, &key_a, nonce_a, Some(&lp_a));
    eprintln!(
        "3 (e) M6D-1, λ_min 50 % (as built): the held cap {ratio:.3}x the honest one; deposit \
         kept {control:?}; pulled at the pump {pulled:?}"
    );
    if let Gap::S1 {
        lenders, q_value, ..
    } = pulled
    {
        eprintln!(
            "3 (e): the lenders {:.3} SOL against Q's value {:.3} SOL at the spot ({:+.1} %)",
            lenders as f64 / 1e9,
            q_value as f64 / 1e9,
            (lenders as f64 / q_value as f64 - 1.0) * 100.0
        );
    }
    eprintln!(
        "3 (e) option (a), λ_min 100 %: the held cap {ratio_a:.3}x the honest one; pulled at the \
         pump {pulled_a:?}"
    );
    assert!((1.98..=2.02).contains(&ratio), "{ratio}: 2× the honest cap");
    assert!(
        matches!(control, Gap::Liquidated(_)),
        "the control is liquidated: {control:?}"
    );
    let Gap::S1 {
        at,
        lenders,
        q_value,
        borrower,
        v,
        bound_refusals,
    } = pulled
    else {
        panic!("as built the pulled deposit gaps the position into S1: {pulled:?}");
    };
    assert!(bound_refusals > 0, "the liquidations fail the exit bound");
    let Gap::Liquidated(control_s) = control else {
        unreachable!()
    };
    assert!(at > control_s, "S1 after the control's liquidation second");
    assert!(lenders < q_value, "the lenders get less than Q's value");
    assert!(lenders <= v);
    assert_eq!(borrower, 0, "the borrower is zeroed");
    // Option (a): the held deposit sizes nothing, and the same pump is liquidated.
    assert!((0.99..=1.01).contains(&ratio_a), "{ratio_a}");
    assert!(
        matches!(pulled_a, Gap::Liquidated(_)),
        "option (a) liquidates: {pulled_a:?}"
    );
}

// ---- the graduation ---------------------------------------------------------------------------------

/// Part 2 (ARCH §2.5): `[migrate_v2, deposit 3× the new pool, crank_price]` — the transition reads
/// the deposit-inflated `lp_supply` as its anchor, so `L_slow = min(L_slow, anchor)` keeps the
/// curve's value (≈ 1.35× the pool's depth: the curve's `√(vT·vQ)`); the first executable PumpSwap
/// observation (the keeper's crank on the pool) reads the LP mint and caps it at the burned LP's
/// depth `D` (1.000), where as built it stayed at 1.353 for an hour (τ_slow). The deposit then
/// leaves; `D` never moved.
#[test]
fn part_2_a_deposit_before_the_transition_is_capped_at_d_by_the_first_observation() {
    let (mut env, c) = Env::fork_pump_coin(fork::PumpCoin::Near);
    ok(env.list_pump(&c));
    let q = c.state(&env.svm).virtual_token_reserves / 200;
    let pm = env.pump_market_listed(pump::PumpSpec::new(fork::PumpCoin::Near, q * 40), c);
    env.warp(1);
    let u = pump::CurveUser::new(&mut env, &pm.curve);
    let rt = pm.curve.state(&env.svm).real_token_reserves;
    ok(u.buy(&mut env, &pm.curve, rt, u64::MAX));
    env.warp(1);
    crank(&mut env, &pm);
    let slow_curve = env.market(&pm.market()).liq_slow.get();
    let migrator = env.funded_keypair();
    let mig = pump::migrate_v2_ix(&env.svm, &migrator.pubkey(), &pm.curve);
    ok(env.send(&[cu_limit_ix(600_000), mig], &[&migrator]));
    let pam = PammMarket::graduated(&env.svm, &pm);
    let payer = env.payer.pubkey();
    ok(env.send(&pamm::venue_account_ixs(&env.svm, &payer, &pam), &[]));
    let (_, d_honest) = pam.pool.locked_depth(&env.svm);
    let lp = PammLp::funded_for(&mut env, &pam.pool, 30_000);
    lp.deposit(&mut env, &pam.pool, 30_000);
    let anchor = u128::from(pam.pool.state(&env.svm).lp_supply);
    let (lam, d) = pam.pool.locked_depth(&env.svm);
    env.warp(1);
    let meta = ok(env.send(&[crank_price_ix(&pm)], &[]));
    venue_changed(&meta, VenueKind::PumpMigrating, VenueKind::PumpSwap);
    let m = env.market(&pm.market());
    let root = |env: &Env| {
        let r = pam.pool.reserves(&env.svm).expect("reserves");
        (u128::from(r.base) * r.effective_quote).isqrt()
    };
    // The transition: the anchor 4× the pool, L_slow the curve's, pool_d 0 (no LP mint read).
    assert_eq!(m.ema_liq.get(), anchor);
    assert_eq!(m.liq_slow.get(), slow_curve.min(anchor));
    assert_eq!(m.pool_d.get(), 0);
    let built = m.liq_slow.get() as f64 / d as f64;
    // The first PumpSwap observation caps L_slow at D.
    crank(&mut env, &pam);
    let m = env.market(&pm.market());
    assert_eq!(m.pool_d.get(), d);
    assert_eq!(m.liq_slow.get(), slow_curve.min(d));
    eprintln!(
        "part 2: deposit 3x between migrate_v2 and the transition (λ {lam} bps, anchor/√k {:.3}); \
         L_slow/D at the transition {built:.3} (as built for an hour), after the first PumpSwap \
         observation {:.3}",
        anchor as f64 / root(&env) as f64,
        m.liq_slow.get() as f64 / d as f64
    );
    assert!(built > 1.3, "{built}");
    assert!(m.liq_slow.get() <= d);
    // D is the burned creation LP's depth: the honest pool's (within 1 bp).
    assert!(d.abs_diff(d_honest) * 10_000 <= d_honest);
    lp.withdraw_all(&mut env, &pam.pool);
    crank_secs(&mut env, &pam, 9);
    let m = env.market(&pm.market());
    assert_eq!(m.ema_warm, 0);
    assert!(m.liq_slow.get() <= m.pool_d.get());
}

// ---- FITS-2 (a) and FITX-1 on a pool with third-party LP ---------------------------------------------

/// The keeper's view of a position (the program's legs on the harness's own venue view).
fn status(env: &Env, pm: &PammMarket, b: &Pubkey, nonce: u64) -> crate::support::Status {
    crate::support::pool_status(env, pm, b, nonce)
}

/// The liquidation spot leg as #100 judged it, at `L_live` alone (before FITS-2 (a)): `CR(V_eff,
/// B̂(Q; reprice(L_live, P_rs))) < L` at the position's risk fee.
fn spot_leg_at_live(env: &Env, pm: &PammMarket, b: &Pubkey, nonce: u64) -> bool {
    let st = status(env, pm, b, nonce);
    let m = market(env, pm);
    let pos = env.position(pm, b, nonce);
    let view = venue_view::view(&env.svm, &m).expect("a view");
    let (Some((x, y)), Some(pricing), Some(p_rs)) = (view.reserves, view.pricing, st.p_rs) else {
        return false;
    };
    let l_live = shortwin_math::quote::liquidity(u128::from(x), u128::from(y)).expect("L");
    let b_rs = pricing
        .capped(u64::from(pos.venue_fee_hard_cap_bps))
        .buy_repriced(pos.q, l_live, p_rs)
        .map(|b| b.cost);
    shortwin_math::risk::spot_leg(true, st.fresh, st.v_eff, b_rs, pos.cr_liq_bps).expect("leg")
}

/// A position on `pm` near its threshold on a pool with as much third-party LP again (λ 50 %, held
/// by the returned provider): the deposit held 10 min, the position at half the honest cap, then a
/// genuine pump in `×1.005` steps (8 s each) to the last step at which the keeper's view (the
/// program's legs at the pool's depth) still finds it healthy, held there 30 s. Returns the
/// provider, the borrower (nonce 0) and a pool trader.
fn near_threshold(env: &mut Env, pm: &PammMarket) -> (PammLp, Borrower, PammUser) {
    let lp = PammLp::funded_for(env, &pm.pool, 10_000);
    lp.deposit(env, &pm.pool, 10_000);
    for _ in 0..10 {
        env.warp(60);
        crank(env, pm);
    }
    assert_eq!(pm.pool.locked_depth(&env.svm).0, 5_000, "λ 50 %");
    let mut b = env.borrower();
    let cap = max_open(env, pm, &b, None);
    let (_, nonce, _) = open_ok(env, pm, &mut b, cap / 2);
    assert_eq!(nonce, 0);
    let crowd = PammUser::new(env, &pm.pool, 100_000 * SOL);
    for _ in 0..30 {
        env.warp(1);
        crank(env, pm);
    }
    let key = b.key();
    let healthy = |env: &Env| !status(env, pm, &key, 0).cr_liq();
    loop {
        let snap = env.svm.clone();
        env.warp(1);
        ok(crowd.push_price(env, &pm.pool, 1_005, 1_000));
        crank(env, pm);
        for _ in 0..8 {
            env.warp(1);
            crank(env, pm);
        }
        if !healthy(env) {
            env.svm = snap;
            break;
        }
    }
    for _ in 0..30 {
        env.warp(1);
        crank(env, pm);
    }
    assert!(healthy(env), "held just below the threshold");
    (lp, b, crowd)
}

/// Part 3 (d), FITS-2 (a) on PumpSwap (ECON §12.12 (5b) (v)'s shape): on a pool with 50 %
/// third-party LP a position held just below its threshold (healthy at the pool's depth) meets a
/// genuine spike above the threshold (6 s) and its retreat. For some seconds after the retreat the
/// EMA leg (the ring's lagging prices) is true while the spot leg at the pool's depth is false —
/// the spot leg's veto. The attacker, the pool's other LP, then sends `[withdraw all, liquidate,
/// deposit]` each second after the keeper's crank: the spot leg is judged at `max(L_live, L_ema')`,
/// so every attempt is refused `NotLiquidatable`, although at the drained `L_live` alone (#100's
/// rule, checked on the drained state) the spot leg was true — the forced liquidation M6R-1 closes
/// on PumpSwap. The position stays open.
#[test]
fn part_3d_fits2_a_withdraw_wrapped_liquidation_forces_nothing() {
    let (mut env, pm, _) = lock_setup();
    let (lp, b, crowd) = near_threshold(&mut env, &pm);
    let (key, nonce) = (b.key(), 0u64);
    let kp = keeper(&mut env);
    let lk = lp.kp.insecure_clone();
    // The genuine spike (+5 %, 6 s) and its retreat (the same base sold back).
    let x0 = pm.pool.reserves(&env.svm).expect("reserves").base;
    env.warp(1);
    ok(crowd.push_price(&mut env, &pm.pool, 1_050, 1_000));
    let bought = x0 - pm.pool.reserves(&env.svm).expect("reserves").base;
    crank(&mut env, &pm);
    for _ in 0..6 {
        env.warp(1);
        crank(&mut env, &pm);
    }
    env.warp(1);
    ok(crowd.sell(&mut env, &pm.pool, bought, 0));
    crank(&mut env, &pm);
    let (mut vetoed, mut drained_true) = (0, 0);
    for _ in 0..10 {
        env.warp(1);
        crank(&mut env, &pm);
        let st = status(&env, &pm, &key, nonce);
        if !(st.ema_leg && !st.spot_leg) {
            continue;
        }
        vetoed += 1;
        let n = lp.lp_balance(&env.svm);
        // #100's rule on the drained pool (a clone): the spot leg at L_live alone.
        let drained = {
            let tx = env.transaction(&[lp.withdraw_ix(&pm.pool, n)], &[&lk]);
            let mut svm = env.svm.clone();
            svm.send_transaction(tx).expect("withdraw");
            let live = std::mem::replace(&mut env.svm, svm);
            let leg = spot_leg_at_live(&env, &pm, &key, nonce);
            env.svm = live;
            leg
        };
        drained_true += usize::from(drained);
        let res = env.send(
            &[
                cu_limit_ix(1_400_000),
                lp.withdraw_ix(&pm.pool, n),
                liquidate_ix(&pm, &kp.pubkey(), &key, nonce),
                lp.deposit_ix(&pm.pool, n),
            ],
            &[&kp, &lk],
        );
        assert_err(&res, E::NotLiquidatable);
    }
    eprintln!(
        "3 (d) FITS-2 (a): {vetoed} seconds of the spot leg's veto after the spike, each wrapped \
         liquidation refused; the spot leg at the drained L_live true in {drained_true}"
    );
    assert!(
        vetoed > 0,
        "the spike leaves the EMA leg true for some seconds"
    );
    assert!(
        drained_true > 0,
        "the drained spot leg is true: #100 forced the liquidation"
    );
    assert!(env.position(&pm, &key, nonce).q > 0, "open");
}

/// FITX-1 on PumpSwap (the inherited residual; ARCH §2.6 residual 10, ECON §12.12 residual 16): on
/// the same position, the pool's other LP withdraws **for real** for 20 s (nothing foreign: every
/// observation reads the drained pool, so `L_ema` decays to it). From there (1) it stays out: the
/// honest keeper liquidates when the program accepts it; or (2) it re-deposits and withdraw-wraps a
/// crank and a liquidation every second, freezing the genuinely reached low (the keeper's coverage
/// holds `L_ema`, it cannot lift it). The wrapped liquidation forces exactly what the withdrawal
/// for good forces: the same second and the same buyback cost, on the same drained pool.
#[test]
fn fitx1_a_held_drain_then_a_freeze_forces_what_a_withdrawal_for_good_does() {
    let (mut env, pm, _) = lock_setup();
    let (lp, b, _) = near_threshold(&mut env, &pm);
    let (key, nonce) = (b.key(), 0u64);
    let kp = keeper(&mut env);
    let lk = lp.kp.insecure_clone();
    let n = lp.lp_balance(&env.svm);
    lp.withdraw_all(&mut env, &pm.pool);
    for _ in 0..20 {
        env.warp(1);
        crank(&mut env, &pm);
    }
    let snap = env.svm.clone();
    // (1) For good.
    let mut for_good = None;
    for s in 1..=30i64 {
        env.warp(1);
        crank(&mut env, &pm);
        if let Ok(meta) = liquidate(&mut env, &pm, &kp, &key, nonce) {
            let ev: shortwin::events::Liquidate = events::one(&meta);
            for_good = Some((s, ev.buyback_cost));
            break;
        }
    }
    // (2) The freeze.
    env.svm = snap;
    ok(env.send(&[lp.deposit_ix(&pm.pool, n)], &[&lk]));
    let mut frozen = None;
    for s in 1..=30i64 {
        env.warp(1);
        crank(&mut env, &pm);
        let m = lp.lp_balance(&env.svm);
        ok(env.send(
            &[
                lp.withdraw_ix(&pm.pool, m),
                crank_price_ix(&pm),
                lp.deposit_ix(&pm.pool, m),
            ],
            &[&lk],
        ));
        let res = env.send(
            &[
                cu_limit_ix(1_400_000),
                lp.withdraw_ix(&pm.pool, m),
                liquidate_ix(&pm, &kp.pubkey(), &key, nonce),
                lp.deposit_ix(&pm.pool, m),
            ],
            &[&kp, &lk],
        );
        if let Ok(meta) = res {
            let ev: shortwin::events::Liquidate = events::one(&meta);
            frozen = Some((s, ev.buyback_cost));
            break;
        }
    }
    eprintln!("FITX-1 on PumpSwap: for good (second, B) {for_good:?}; the freeze {frozen:?}");
    let (f, z) = (
        for_good.expect("liquidated for good"),
        frozen.expect("the freeze forces it"),
    );
    assert_eq!(f, z, "the same second and buyback cost");
}
