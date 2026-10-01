//! The M7 security review's program findings, as fixed or measured (regressions of the review's
//! reproducers):
//!
//! - **M7-SEC-3** (fixed): the `coverage_authority` key could land an open on a coverage-paused
//!   market with `[set_coverage_pause(false), open_short, set_coverage_pause(true)]` in one
//!   transaction. A clear of a set flag now records its second (`Market.coverage_cleared_at`) and
//!   opens refuse `CoveragePaused` for `COVERAGE_CLEAR_DELAY_S` after it: the bracket fails, and
//!   so does any open in the delay; a no-op clear records nothing. Since the owner's decision of
//!   2026-10-01 (★OWNER-M7-3 (b)) the Emergency vault may clear the flag too: its clear starts the
//!   same delay, so `[Emergency clear, open_short]` — and the bracket with the key's re-set — is
//!   refused at its open as well.
//! - **M7-SEC-4** (owner item OWNER-M7-11, as built): `N_open` — the launch guard, `min_notional`,
//!   the open fee — is quoted on the post-open live reserves, so a sale in the open's own
//!   transaction (down to the spot–EMA guard's −10 %) records ≈ 0.91 of the honest quote. The
//!   residual is pinned here at its bound, and the review's fix (`max(B̂_spot, B̂_ema)`) is
//!   measured on honest opens: it raises every honest `N_open` (and fee) by the open's own
//!   price impact.
//! - **M7-SEC-5** (fixed): the fee-scaled caps took the live one-way fee, which pump-fees picks
//!   from the live market cap — a sale in the open's own transaction moved the pool into a
//!   higher-fee tier and raised `3.0·φ·X₀` / `7.5·φ·X₁`. They now take `min(φ_live, φ at P_ema)`
//!   (`VenueState::cap_fee_ppm`): on the PumpSwap fork with a tier boundary between the EMA's
//!   market cap and a sale's, the open's caps (and the fee mark `min_open_fee_ppm` they set) are
//!   the EMA tier's.

#![allow(clippy::result_large_err)]

#[allow(unused_imports)]
#[path = "engine_it/common.rs"]
mod ecommon;

#[allow(unused_imports)]
#[path = "pamm_it/common.rs"]
mod pcommon;

const SOL: u64 = 1_000_000_000;

/// M7-SEC-3 and M7-SEC-4 on the mock.
mod mock_venue {
    use shortwin::constants::COVERAGE_CLEAR_DELAY_S;
    use shortwin::errors::ShortwinError as E;
    use shortwin::events::{CoveragePauseSet, Open};
    use shortwin::state::MARKET_FLAG_PAUSED_COVERAGE;
    use shortwin::venues::ExtrasSel;
    use shortwin_litesvm_tests::ix;
    use solana_signer::Signer;

    use super::SOL;
    use crate::ecommon::*;

    fn bracket(env: &mut Env, mm: &MockMarket, b: &Borrower, q: u64) -> TxResult {
        let auth = env.coverage_authority.insecure_clone();
        let key = mm.market();
        let clear = ix::set_coverage_pause(&auth.pubkey(), &key, false, 1);
        let set = ix::set_coverage_pause(&auth.pubkey(), &key, true, 1);
        let open = open_short_ix(
            mm,
            &b.key(),
            b.next_nonce,
            q,
            0,
            u64::MAX,
            u64::MAX,
            ExtrasSel::NONE,
            &[],
        );
        let bkp = b.kp.insecure_clone();
        env.send(&[clear, open, set], &[&auth, &bkp])
    }

    /// M7-SEC-3: the keeper paused the market (degraded); the key holder's bracket is refused
    /// `CoveragePaused` at its open (the whole transaction: no Open, the flag still set); a clear
    /// alone lets opens land only `COVERAGE_CLEAR_DELAY_S` later; a no-op clear records nothing;
    /// and a `[set, clear, open]` on a market without the flag is refused too.
    #[test]
    fn m7_sec_3_the_coverage_key_cannot_bracket_an_open() {
        let (mut env, mm) = setup();
        env.plant_coverage_authority();
        env.crank(&mm);
        env.warp(1);
        let key = mm.market();
        ok(env.set_coverage_pause(&key, true));
        let mut b = env.borrower();
        let (res, _) = env.open(&mm, &mut b, Q);
        assert_err(&res, E::CoveragePaused);

        let res = bracket(&mut env, &mm, &b, Q);
        assert_err(&res, E::CoveragePaused);
        assert!(env.market(&key).has_flag(MARKET_FLAG_PAUSED_COVERAGE));
        assert_eq!(env.market(&key).coverage_cleared_at, 0, "nothing committed");

        // A clear alone: opens wait out the delay.
        let meta = ok(env.set_coverage_pause(&key, false));
        let ev: CoveragePauseSet = events::one(&meta);
        assert!(!ev.paused);
        let t = env.now();
        assert_eq!(env.market(&key).coverage_cleared_at, t);
        for s in 0..COVERAGE_CLEAR_DELAY_S {
            assert_eq!(env.now(), t + s);
            let (res, _) = env.open(&mm, &mut b, Q);
            assert_err(&res, E::CoveragePaused);
            env.warp(1);
            env.crank(&mm);
        }
        // A no-op clear (the flag is clear) records nothing.
        ok(env.set_coverage_pause(&key, false));
        assert_eq!(env.market(&key).coverage_cleared_at, t);
        let (res, _) = env.open(&mm, &mut b, Q);
        let o: Open = events::one(&ok(res));
        assert_eq!(o.q, Q);

        // `[set, clear, open]` on a market without the flag: the clear starts the delay.
        let auth = env.coverage_authority.insecure_clone();
        let set = ix::set_coverage_pause(&auth.pubkey(), &key, true, 1);
        let clear = ix::set_coverage_pause(&auth.pubkey(), &key, false, 1);
        let open = open_short_ix(
            &mm,
            &b.key(),
            b.next_nonce,
            Q,
            0,
            u64::MAX,
            u64::MAX,
            ExtrasSel::NONE,
            &[],
        );
        let bkp = b.kp.insecure_clone();
        let res = env.send(&[set, clear, open], &[&auth, &bkp]);
        assert_err(&res, E::CoveragePaused);
        println!(
            "M7-SEC-3: [clear, open, set] refused CoveragePaused; opens resume {COVERAGE_CLEAR_DELAY_S} s \
             after a clear"
        );
    }

    /// ★OWNER-M7-3 (b) with M7-SEC-3: the Emergency vault's clear starts the clear delay like the
    /// key's — `[Emergency clear, open_short]` and `[Emergency clear, open_short, key set]` are
    /// refused `CoveragePaused` at the open (nothing committed: the flag still set, no clock
    /// recorded); the Emergency's clear alone lets opens land `COVERAGE_CLEAR_DELAY_S` later.
    #[test]
    fn m7_sec_3_an_emergency_clear_cannot_bracket_an_open_either() {
        let (mut env, mm) = setup();
        env.plant_coverage_authority();
        env.crank(&mm);
        env.warp(1);
        let key = mm.market();
        ok(env.set_coverage_pause(&key, true));
        let b = env.borrower();
        let ea = env.pause_authority.insecure_clone();
        let auth = env.coverage_authority.insecure_clone();
        let open = || {
            open_short_ix(
                &mm,
                &b.key(),
                b.next_nonce,
                Q,
                0,
                u64::MAX,
                u64::MAX,
                ExtrasSel::NONE,
                &[],
            )
        };
        let e_clear = ix::set_coverage_pause(&ea.pubkey(), &key, false, 1);
        let k_set = ix::set_coverage_pause(&auth.pubkey(), &key, true, 1);
        let bkp = b.kp.insecure_clone();
        for ixs in [
            vec![e_clear.clone(), open()],
            vec![e_clear.clone(), open(), k_set.clone()],
        ] {
            let res = env.send(&ixs, &[&ea, &auth, &bkp]);
            assert_err(&res, E::CoveragePaused);
            assert!(env.market(&key).has_flag(MARKET_FLAG_PAUSED_COVERAGE));
            assert_eq!(env.market(&key).coverage_cleared_at, 0, "nothing committed");
        }
        // The Emergency's clear alone: opens wait out the delay.
        let meta = ok(env.emergency_clear_coverage(&key));
        let ev: CoveragePauseSet = events::one(&meta);
        assert!(!ev.paused && ev.by == ea.pubkey());
        let t = env.now();
        assert_eq!(env.market(&key).coverage_cleared_at, t);
        let mut b = b;
        for _ in 0..COVERAGE_CLEAR_DELAY_S {
            let (res, _) = env.open(&mm, &mut b, Q);
            assert_err(&res, E::CoveragePaused);
            env.warp(1);
            env.crank(&mm);
        }
        let (res, _) = env.open(&mm, &mut b, Q);
        let o: Open = events::one(&ok(res));
        assert_eq!(o.q, Q);
        println!(
            "OWNER-M7-3 (b): [Emergency clear, open] refused CoveragePaused; opens resume \
             {COVERAGE_CLEAR_DELAY_S} s after the Emergency's clear"
        );
    }

    /// `N_open(q)` on the mock pool now: the sale of `q`, then the buyback of `q` on the post-sale
    /// pricing reserves (the honest notional of an open of `q`).
    fn n_open(env: &Env, mm: &MockMarket, q: u64) -> Option<u64> {
        use shortwin_math::quote::cpmm;
        let r = mock::reserves(&env.svm, mm.mock());
        let fee = mock::state(&env.svm, &mm.mock().pool).fee_ppm;
        let fees = mock::cpmm_fees(fee);
        let s = cpmm::sell_exact_in(q, u128::from(r.base), u128::from(r.quote), &fees)?;
        cpmm::buy_exact_out(q, s.sol_after, s.token_after, &fees).map(|b| b.cost)
    }

    fn least_q(f: impl Fn(u64) -> u64, target: u64, hi: u64) -> u64 {
        let (mut lo, mut hi) = (0u64, hi);
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if f(mid) >= target {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    }

    /// M7-SEC-4 (as built; OWNER-M7-11): 0.5-SOL positions opened as `[sale to 91 % of P_ema,
    /// open_short]` record `N_open` ≈ 0.91 of their honest quote, so the market holds up to
    /// ≈ 1/0.9 of `market_max_notional` in honestly quoted notional — and no more: every wrapped
    /// open records at least `(1 − open_ema_dev)` of its honest quote.
    #[test]
    fn m7_sec_4_a_sale_wrapped_open_is_bounded_by_the_spot_ema_guard() {
        let (mut env, mm) = setup_with(MarketSpec {
            fee_ppm: 10_000,
            base: 10_000_000_000_000_000,
            quote: 10_000_000_000_000,
            lend: 1_000_000_000_000_000,
            ..MarketSpec::default()
        });
        let key = mm.market();
        let params = env.config().params;
        let max = params.market_max_notional;
        let dev = u128::from(params.open_ema_dev_bps);
        let attacker = env.actor(&mm, 5_000_000_000_000_000, 1_000_000_000_000_000);
        let mut b = env.borrower();
        let (mut honest_total, mut recorded_total, mut n) = (0u128, 0u128, 0u32);
        loop {
            for _ in 0..30 {
                env.warp(1);
                env.crank(&mm);
            }
            let q = least_q(
                |q| n_open(&env, &mm, q).unwrap_or(0),
                SOL / 2,
                100_000_000_000_000,
            );
            let honest = n_open(&env, &mm, q).unwrap();
            let spot = env.spot(&mm);
            let p_ema = env.market(&key).ema_price.get();
            let (_, _, sell) = env
                .push_price_ix(&mm, &attacker, p_ema * 91 / 100)
                .expect("a sale");
            let open = open_short_ix(
                &mm,
                &b.key(),
                b.next_nonce,
                q,
                0,
                u64::MAX,
                u64::MAX,
                ExtrasSel::NONE,
                &[],
            );
            let (akp, bkp) = (attacker.kp.insecure_clone(), b.kp.insecure_clone());
            let res = env.send(&[sell, open], &[&akp, &bkp]);
            if res.is_err() {
                assert_eq!(
                    custom_code(&res),
                    Some(code(E::MarketNotionalCap)),
                    "{res:?}"
                );
                break;
            }
            let o: Open = events::one(&res.unwrap());
            b.next_nonce += 1;
            n += 1;
            // The bound: at most `open_ema_dev` below the honest quote.
            assert!(
                u128::from(o.n_open) * 10_000 >= u128::from(honest) * (10_000 - dev),
                "N_open {} vs honest {honest}",
                o.n_open
            );
            honest_total += u128::from(honest);
            recorded_total += u128::from(o.n_open);
            env.push_price(&mm, &attacker, spot);
            assert!(n < 100);
        }
        let m = env.market(&key);
        println!(
            "M7-SEC-4 (residual, OWNER-M7-11): {n} wrapped opens record Σ N_open {:.2} SOL (guard \
             {:.2} SOL), honest Σ N_open {:.2} SOL ({:+.2} %)",
            recorded_total as f64 / 1e9,
            max as f64 / 1e9,
            honest_total as f64 / 1e9,
            100.0 * (honest_total as f64 / max as f64 - 1.0)
        );
        assert!(u128::from(m.total_notional_open) <= u128::from(max));
        assert!(honest_total * (10_000 - dev) <= u128::from(max) * 10_000);
    }

    /// M7-SEC-4's proposed fix measured on honest opens (the owner's numbers): the `Open` event
    /// carries `b_spot` (= `N_open`) and `b_ema`; `max(b_spot, b_ema)` exceeds `N_open` on every
    /// honest open by about the open's own price impact — 0.1, 1 and 3 SOL positions on the
    /// default mock pool (1,000 SOL, 25 bps) and on a deep one (10,000 SOL, 1 %).
    #[test]
    fn m7_sec_4_the_review_fix_raises_honest_notional_by_the_opens_impact() {
        for (label, spec) in [
            ("default 1,000-SOL pool, 25 bps", MarketSpec::default()),
            (
                "deep 10,000-SOL pool, 1 %",
                MarketSpec {
                    fee_ppm: 10_000,
                    base: 10_000_000_000_000_000,
                    quote: 10_000_000_000_000,
                    lend: 1_000_000_000_000_000,
                    ..MarketSpec::default()
                },
            ),
        ] {
            let (mut env, mm) = setup_with(spec);
            let mut rows = Vec::new();
            for target in [SOL / 10, SOL, 3 * SOL] {
                env.warp(30);
                env.crank(&mm);
                let q = least_q(
                    |q| n_open(&env, &mm, q).unwrap_or(0),
                    target,
                    1_000_000_000_000_000,
                );
                let mut b = env.borrower();
                let (res, _) = env.open(&mm, &mut b, q);
                let Ok(meta) = res else { continue };
                let o: Open = events::one(&meta);
                let up = 100.0 * (o.b_spot.max(o.b_ema) as f64 / o.n_open as f64 - 1.0);
                rows.push(format!(
                    "N_open {:.3} SOL: max(b_spot, b_ema) {up:+.3} %",
                    o.n_open as f64 / 1e9
                ));
                assert!(o.b_ema >= o.b_spot, "honest: b_ema ≥ b_spot");
            }
            println!(
                "M7-SEC-4 (the review's fix on honest opens, {label}): {}",
                rows.join("; ")
            );
        }
    }
}

/// M7-SEC-5 on the PumpSwap fork.
mod pumpswap {
    use shortwin::constants::PUMP_AMM_FEE_CONFIG;
    use shortwin_math::caps;
    use shortwin_venue_state::pump::{FEE_TIER_LEN, OFF_FEE_CONFIG_FEE_TIERS};

    use crate::pcommon::*;

    /// Rewrites PumpSwap's `FeeConfig.fee_tiers` in place (same length): tier 0 from 0 at `hi`,
    /// tier 1 from `boundary` at `lo`, the rest at `u128::MAX` (never chosen).
    fn plant_tiers(env: &mut Env, boundary: u128, hi: (u64, u64, u64), lo: (u64, u64, u64)) {
        let mut acc = env
            .svm
            .get_account(&PUMP_AMM_FEE_CONFIG)
            .expect("FeeConfig");
        let at = OFF_FEE_CONFIG_FEE_TIERS;
        let len = u32::from_le_bytes(acc.data[at..at + 4].try_into().unwrap()) as usize;
        assert!(len >= 2, "{len} tiers");
        for i in 0..len {
            let o = at + 4 + i * FEE_TIER_LEN;
            let (threshold, f) = match i {
                0 => (0u128, hi),
                1 => (boundary, lo),
                _ => (u128::MAX, lo),
            };
            acc.data[o..o + 16].copy_from_slice(&threshold.to_le_bytes());
            acc.data[o + 16..o + 24].copy_from_slice(&f.0.to_le_bytes());
            acc.data[o + 24..o + 32].copy_from_slice(&f.1.to_le_bytes());
            acc.data[o + 32..o + 40].copy_from_slice(&f.2.to_le_bytes());
        }
        env.svm
            .set_account(PUMP_AMM_FEE_CONFIG, acc)
            .expect("set FeeConfig");
    }

    /// The pool's market cap now (`effective_quote·supply/base`, pump-fees' tier input).
    fn mcap(env: &Env, pm: &PammMarket) -> u128 {
        let r = pm.pool.reserves(&env.svm).expect("reserves");
        let a = read::account(&env.svm, &pm.pool.mint).expect("mint");
        let supply = shortwin_venue_state::mint::read_base(&a.owner.to_bytes(), &a.data)
            .expect("mint")
            .supply;
        r.effective_quote * u128::from(supply) / u128::from(r.base)
    }

    /// M7-SEC-5: a tier boundary at 96 % of the market cap at `P_ema` (30 bps above it, 40 bps
    /// below); an open wrapped in a sale to ≈ 92 % of the spot pays the 40-bps live fee, but its
    /// caps — and the fee mark `min_open_fee_ppm` its open sets — are the EMA tier's 30 bps. As
    /// built at M7 the mark (and `3.0·φ·X₀`, `7.5·φ·X₁`) took the live 40 bps (+33 %).
    #[test]
    fn m7_sec_5_a_same_transaction_sale_does_not_raise_the_fee_scaled_caps() {
        let (mut env, pm, q) = setup(PammCoin::Mature);
        crank_secs(&mut env, &pm, 30);
        let m0 = market(&env, &pm);
        assert_eq!(m0.open_positions, 0);
        let cap0 = mcap(&env, &pm);
        let boundary = cap0 * 96 / 100;
        plant_tiers(&mut env, boundary, (30, 7, 3), (22, 5, 3));
        crank_secs(&mut env, &pm, 30);
        let st = |env: &Env| {
            let m = market(env, &pm);
            shortwin_litesvm_tests::invariants::venue_view::view(&env.svm, &m)
        };
        let live0 = st(&env).expect("view").fee_ppm;
        // The sale: the base reserve up by √(1/0.92) − 1 (spot ≈ −8 %), market cap below the
        // boundary.
        let r = pm.pool.reserves(&env.svm).expect("reserves");
        let delta =
            u64::try_from((u128::from(r.base) as f64 * ((1.0f64 / 0.92).sqrt() - 1.0)) as u128)
                .unwrap();
        let seller = PammUser::new(&mut env, &pm.pool, 1_000_000_000);
        fork::set_token_amount(&mut env.svm, &seller.base, delta);
        let sell = pamm::pamm_trade_ix(
            &env.svm,
            &pm.pool,
            pamm::PammTrade::Sell,
            &seller.kp.pubkey(),
            &seller.base,
            &seller.wsol,
            delta,
            0,
        );
        let mut b = env.borrower();
        let mut b2 = env.borrower();
        let qq = q / 4;
        let before_open = env.svm.clone();
        let open = open_short_ix(
            &pm,
            &b.key(),
            b.next_nonce,
            qq,
            0,
            u64::MAX,
            u64::MAX,
            ExtrasSel::NONE,
            &[],
        );
        let (skp, bkp) = (seller.kp.insecure_clone(), b.kp.insecure_clone());
        let meta = ok(env.send(&[cu_limit_ix(1_400_000), sell, open], &[&skp, &bkp]));
        let o: Open = events::one(&meta);
        b.next_nonce += 1;
        let m = market(&env, &pm);
        let live1 = st(&env).expect("view").fee_ppm;
        assert!(mcap(&env, &pm) < boundary, "the sale crossed the boundary");
        println!(
            "M7-SEC-5: fee {live0} ppm at the EMA's market cap, {live1} ppm after the sale; the open's \
             mark min_open_fee_ppm {} (q {})",
            m.min_open_fee_ppm, o.q
        );
        assert!(
            live1 > live0,
            "the sale moved the pool into the higher-fee tier"
        );
        assert_eq!(
            m.min_open_fee_ppm, live0,
            "the caps take the EMA tier's fee"
        );
        // The caps at the two fees on the same X₀: the live tier's would be a third larger.
        let x0 = 1_000_000_000_000u64;
        let lo = caps::fee_size_cap(x0, live0, 300).unwrap().unwrap();
        let hi = caps::fee_size_cap(x0, live1, 300).unwrap().unwrap();
        assert!(hi > lo);

        // The largest `q` a sale-wrapped open accepts (bisection on clones, hooks off), then that
        // open with the hooks on: I19 recomputes every cap at `min(φ_live, φ at P_ema)`, so a
        // build that sized it at the live fee (M7-SEC-5 reverted) fails here.
        let wrapped = |env: &mut Env, b: &mut Borrower, q: u64| {
            let sell = pamm::pamm_trade_ix(
                &env.svm,
                &pm.pool,
                pamm::PammTrade::Sell,
                &seller.kp.pubkey(),
                &seller.base,
                &seller.wsol,
                delta,
                0,
            );
            let open = open_short_ix(
                &pm,
                &b.key(),
                b.next_nonce,
                q,
                0,
                u64::MAX,
                u64::MAX,
                ExtrasSel::NONE,
                &[],
            );
            let (skp, bkp) = (seller.kp.insecure_clone(), b.kp.insecure_clone());
            env.send(&[cu_limit_ix(1_400_000), sell, open], &[&skp, &bkp])
        };
        env.check_invariants = false;
        let r0 = pm.pool.reserves(&before_open).expect("reserves");
        let (mut lo_q, mut hi_q) = (1u64, u64::try_from(u128::from(r0.base) / 10).unwrap());
        while hi_q - lo_q > 1 {
            let mid = lo_q + (hi_q - lo_q) / 2;
            env.svm = before_open.clone();
            if wrapped(&mut env, &mut b2, mid).is_ok() {
                lo_q = mid;
            } else {
                hi_q = mid;
            }
        }
        env.svm = before_open.clone();
        env.check_invariants = true;
        let meta = ok(wrapped(&mut env, &mut b2, lo_q));
        let o2: Open = events::one(&meta);
        println!(
            "M7-SEC-5: the largest sale-wrapped open, q {} (I19-checked at the EMA tier's fee)",
            o2.q
        );
    }
}
