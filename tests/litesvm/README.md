# shortwin-litesvm-tests

Program integration tests on LiteSVM 0.16 (ARCHITECTURE §8.2, §8.3).

```sh
anchor build --tools-version v1.57 --arch v3        # target/deploy/shortwin.so + mock_cpamm.so (required)
scripts/build-test-venue.sh                         # target/test-venue/deploy/shortwin.so: the TEST-ONLY
                                                    #   `test-venue` build the engine tests load (A49)
cargo test -p shortwin-litesvm-tests                 # or: SHORTWIN_SO=… SHORTWIN_TEST_VENUE_SO=… MOCK_CPAMM_SO=… cargo test …
cargo test -p shortwin-litesvm-tests --test engine_it -- --nocapture   # the M2 engine on the mock (prints T-TRACE / CU)
ENGINE_FUZZ_SEEDS=16 ENGINE_FUZZ_STEPS=300 cargo test -p shortwin-litesvm-tests --test engine_it fuzz   # longer random walks
MOCK_CPAMM_CASES=1000 cargo test -p shortwin-litesvm-tests --test mock_cpamm   # deeper mock-venue differential run
RAY_MATH_SEQ=400 cargo test -p shortwin-litesvm-tests --test ray_math_it random   # longer random trades on the Raydium fork
cargo test -p shortwin-litesvm-tests --test ray_it -- --nocapture   # the Raydium adapters on the fork (prints T-RAY-TRACE / CU, T-SIZE)
cargo test -p shortwin-litesvm-tests --test raydium_v4_e2e --test raydium_v4_attacks -- --nocapture   # fork E2E and ECON §12 on Fartcoin AMM v4 (prints the figures)
cargo test -p shortwin-litesvm-tests --test raydium_cpmm_e2e -- --nocapture   # fork E2E on CPMM: USELESS, Token-2022, creator fee (prints trace / CU)
cargo test -p shortwin-litesvm-tests --test latch_raydium_it                  # T-LATCH on the dumped ProgramData (real loader upgrades)
cargo test -p shortwin-litesvm-tests --test trace_raydium_it -- --nocapture   # T-RAY-TRACE: each Raydium instruction's call shape, entries, depth and CU
cargo test -p shortwin-litesvm-tests --test tx_size_raydium -- --nocapture    # T-SIZE on Raydium: worst cases as v0 through the protocol ALT (prints the size table)
cargo test -p shortwin-litesvm-tests --test ray_sec_it -- --nocapture         # the M3 security review's regressions (M3-SEC-1–3) on the fork
cargo test -p shortwin-litesvm-tests --test it cu_baseline -- --nocapture   # admin CU table
SHORTWIN_TRACES=full cargo test --release -p shortwin-litesvm-tests --test lender_it traces::   # T-POT, ~4 min (CI job `t-pot`)
```

## Layout

| Path | What |
|---|---|
| `src/env.rs` | `Env`: LiteSVM + `shortwin.so` (release, `Env::new`; or the test-venue build, `Env::new_test_venue_program` / `$SHORTWIN_TEST_VENUE_SO`), admin keys (upgrade authority recorded in the ProgramData, Governance, Emergency, fee recipient), clock control (`warp`, `set_clock`), `send` with the invariant hooks |
| `src/engine.rs` | the M2 engine on the mock: `Env::test_venue` (test-venue build + mock + config with the mock's venue bit + the mock's `VenueExtras` account), `Env::mock_market` (pool, liquidity, `create_market(Mock)`, lender deposit, warm-up — real instructions), `MockMarket` (keys, ReadSet / TradeExtras), the `VenueMarket` trait (keys and the venue's account lists; the mock and Raydium markets implement it), builders of every M2 instruction generic over it, `Borrower`, `Env::open` / `Env::close` |
| `src/drive.rs` | time and price drivers: `Actor` (trades the mock directly), `Env::trade`, `Env::push_price`, `Env::tick` (one crank per second), `Env::spot` |
| `src/attack.rs` | atomic bundles in one transaction: `Env::bundle`, `Env::sandwich` (front-run, victim, back-run; the attacker's result) |
| `src/ix.rs` | instruction builders for every M1 instruction |
| `src/invariants.rs` | `check_all(&svm, market)` (I1, I2, I8) and `snapshot` + `check_ix` (I4, I17 — opens, and since M3-SEC-2 listings on a latched venue — per instruction) |
| `src/invariants/positions.rs` | the position engine's invariants: I6, I7 (`pool_vault` delegate), I13 (ring, newest entry, one-second lag) per market; I3, A26 (snapshot rules), I5, I7, I9, I10, I11, I12 (amounts, persistence, the S3 chain, `p'` per corner), I13 (charges, re-quotes on the mock and on Raydium, EMA steps), I16 (no observation while not executable; the route chains) per instruction (module table) |
| `src/invariants/venue_view.rs` | the hooks' own view of a market's venue (M3): the mock and the Raydium AMM v4 / CPMM pools as short.win's adapter must see them — executable, reserves, fees, `B̂` / `S` — computed from the venue-state readers and `shortwin-math` (never from the adapter), for I13's re-quotes and I16's route rules |
| `src/invariants/lender.rs` | the lender pool's part of I1 (both directions) and I8 (claims, entitlement, allowances with each lender touched alone, locks, calendar, liveness, `S = 0 ⇒ ΣB_Q = 0`) and the instruction-level lender bounds (allowance per withdrawal, claim ≤ entitlement, token conservation, no tokens without shares, locks never decrease) |
| `src/fixtures.rs` | mock venue ProgramData (T-LATCH), raw zero-copy / Borsh / token-account writers, `install_market` |
| `src/mock.rs` | the TEST-ONLY mock venue `mock_cpamm` (M2; `tests/mock_cpamm/INTERFACE.md`): `Env::load_mock_cpamm` (`target/deploy/mock_cpamm.so` or `$MOCK_CPAMM_SO`), pools on SPL / Token-2022 base mints, liquidity, knobs, layout-aware reserves, WSOL and base trader accounts, swap builders, short.win's `quote::cpmm` view of a pool |
| `src/fixtures/mints.rs` | mint fixtures: one real Token-2022 mint per extension, forged TLV mints, the pump.fun shape |
| `src/fork.rs` | the Raydium mainnet fork (M3): the dumped AMM v4 / CPMM programs (deployed from their ProgramData, real deploy slots for T-LATCH; `set_deploy_slot`) and pools of `tests/fixtures/` (sha256-checked against `manifest.json`; `scripts/dump-fixtures.py` writes them), `Env::fork_raydium_v4` / `Env::fork_cpmm` / `Env::load_ray_v4` / `Env::load_cpmm` / `Env::set_fork_clock`, `V4Pool` / `CpmmPool` (readers, quotes, the venues' swap builders), `fund_wsol` / `fund_token` / `trader` / `swap` (direct user swaps through `Env::send`), byte-level admin builders (status, open time, fees, `need_take_pnl`, counters, creator-fee fields, config rates, vault amounts) and the real admin instructions with the admin's signature forged (`admin_send`, hooks included), `ray_log` / `SwapEvent` decoding |
| `src/raydium.rs` | Raydium **markets** on the fork (M3): `Env::ray_fork` (release build + both Raydium programs + the fork's clock), `Env::ray_market` / `Env::list_ray` / `Env::ray_market_listed` (`create_market(RaydiumV4 / RaydiumCpmm)`, lender, listing window), `RayMarket` (the ReadSet / TradeExtras in ARCH §3.12's order from the fixture's own keys; implements `engine::VenueMarket`, so every M2 builder and `Env::open` / `Env::close` / `Env::tick` trade it; quotes), `candidate_read_set` / `create_ray_market_ix`, LP-lock builders (`set_mint_supply`, `supply_for_lambda`, `locked_depth`), `Env::fresh_oracle`, pump-detection fixtures (Fartcoin's mainnet curve, curve and canonical-pool builders), T-SIZE helpers (the protocol ALT, legacy / v0 sizes, `Env::send_v0` with the hooks) |
| `src/tokens.rs` | SPL Token / Token-2022 mints, accounts, holders |
| `src/events.rs`, `src/errors.rs`, `src/read.rs`, `src/pda.rs` | `emit_cpi!` decoding, error assertions, account readers, PDAs |
| `tests/it/` | admin, config (T-CONFIG on-chain), timelock, pause, T-LATCH, T-LAYOUT (runtime offsets), constraint cases, the harness itself, the admin CU table |
| `tests/market_create.rs` | `create_market` (exact state, fee, griefing, detection keys) and `flag_unsafe_mint` |
| `tests/mint_filter_extensions.rs` | the SPEC §3 filter end to end: one fixture per extension, program vs byte-level filter |
| `tests/lender_it/` | the six lender-pool instructions: flows, T-POT, the ARCH §8.2 lender adversarial cases (incl. first-depositor inflation by donation), the hooks' own tests (`hooks.rs`: each check fires on what a program mutant would leave), the lender side of the M1 security review (`security.rs`), `pool_model` trace replays (quick subset by default, `SHORTWIN_TRACES=full` for all) |
| `tests/oracle_crank.rs` | `crank_sol_usd` on the committed mainnet `PriceUpdateV2` dump and mutations of it |
| `tests/engine_it/` | the M2 engine on the mock (test-venue build): open/close happy paths (SPL, Token-2022), every open refusal reachable on the mock, close refusals and what exits never read (I10), trader normalisation grief cases, over-delivery, sandwiches around open and close, T-TRACE and CU, the prologue (lag, `CoverageGap`, forced EMA adoption, record only while executable, chains, T-S3-BROKEN's structural part), the O6 powers (T-REPLAY-PDA on the mock), the landed exits' gates on a healthy position (`landed.rs`), I14 (structure), the M2 hooks' own mutant tests, and the seeded random walk (`fuzz.rs`: opens, closes, top-ups, keeper exits, shocks, deadlines, I10's pause-and-flag action) |
| `tests/settle_it.rs` | `sol_settle` (track settle-o6): the no-op outcomes, S1 / S2 with their waterfalls, the arming clock (stale bucket, re-arm, box-only disarms), share-epoch records (full wipe, rebase, a pre-funded record address, the record key), the account checks, T-TRACE / CU, and T-BOX (the `L` variant, one pushed or frozen second, one attacker second against an armed clock) |
| `tests/route_it.rs` | S3: T-S3-BROKEN (market and venue flag; the flag changes nothing but the S3 chain; closes and liquidations unchanged, second by second; the Emergency-flag S3 raced against an honest expiry, O15), a fee above the hard cap (FV at the locked cap), a disabled venue and a layout change (not executable, the chain, S3 on the frozen EMA point), a healthy position on a halted venue |
| `tests/extras_it.rs` | T-REPLAY-PDA end to end: a per-mint PDA the venue starts requiring, the Emergency flag's S3 escape, the template, a hostile `current` bypassed with `previous`, `none` once the venue drops it; `sol_settle` takes no extras |
| `tests/attacks_settle_it.rs` | ECON §12.10–§12.11 on the mock (`RC`-like pool): `arming.py`, `refute_push.py` (push, push-then-freeze, spot-leg veto), `fv_push.py` (amount pushes, S2 → S1 flips, 5 consecutive attacker-only seconds, the Emergency-flag S3, the `p'` veto), asserted against the sims' outcomes on the same configuration |
| `tests/settle_support.rs` | the shared scenario machinery of the four binaries above and of `security_m2.rs` (included as `mod support`; no tests of its own) |
| `tests/add_margin_it.rs` | `add_margin`: top-ups on SPL / Token-2022, exact payback on close, every gate ignored, snapshots unchanged by governance changes, I14 on its accounts, a seeded walk (`ADD_MARGIN_WALK_SEEDS`) |
| `tests/crank_price_it.rs` | `crank_price`: `CoverageGap` (30 s vs 31 s, from the last integration), same-second floods, the ring rewrite, forced EMA adoption |
| `tests/liquidate_it.rs`, `tests/expire_it.rs` | the keeper exits against `liquidate_support.rs`'s oracle (every decision and amount recomputed from `shortwin-math`): every waterfall branch, every refusal reachable on the mock, what exits ignore (I10), donations, over-delivery, extras, I14, Token-2022, liveness (I-E11), T-TRACE / CU, the hooks' mutants |
| `tests/attacks_exit_it.rs` | ECON §12's exit attacks on ECON's `RA` market, to the lamport: `sandwich.py`, `fixed_delta.py`, `block_spot.py`, `ema_atomic.py`, `ema_near.py`, `ema_atomic_multi.py` (the OI-cap basket: atomic, held, S1 forcing), `ptf_sandwich.py`, `fvcap.py` (the RA rows), `cascade.py` |
| `tests/liquidate_support.rs` | the oracle, environments and drivers of the three binaries above (included as `mod support`; no tests of its own) |
| `tests/invariants_m2_hooks.rs` | the M2 hooks fire on deliberately broken states: I2, I3, A26, I5, I6, I7, I9–I13, I16 |
| `tests/ray_it/` | the Raydium adapters (M3) on the fork, release build: listing (exact state, O13 ages and tiers, every verification / eligibility refusal, D1 precedence with Fartcoin's real graduated curve), open → close to the lamport on both programs (SPL, Token-2022, WSOL on either side, every fee variant incl. the CPMM creator fee on the SOL side, the token side — U11 — and both; every eligible config), the Raydium open refusals, T-LATCH on the dumped ProgramData, T-RAY-PDA (inner instructions), T-RAY-WSOL, T-RAY-DISABLE (every status; a CPMM zero fee on the buy's input is no route, a sell-only one keeps the route; S3 at the deadline on a halted pool), T-RAY-FEE (a zero fee, between and above the caps; risk and FV at the locked cap), liquidation after a real pump and expiry, I14 on every instruction, the hooks' Raydium view, T-RAY-TRACE / CU and T-SIZE (v0 with the protocol ALT, executed) |
| `tests/ray_math_it.rs` | T-RAY-MATH (execution half) on the Raydium fork: direct swaps on the real dumped ELFs (both directions, exact in and exact out, every fixture pool, all 21 CPMM configs × creator-fee sides, AMM v4 fee and `need_take_pnl` changes, CPMM zero-fee configs (M3-SEC-1: `ZeroTradingTokens` on the direction whose creator fee is on its input), seeded sequences: `RAY_MATH_SEQ`, default 40) equal `quote::ray_v4` / `quote::cpmm` on the readers' reserves, with the exact bound passing and one unit tighter failing; negative controls; the fixture integrity and venue PDAs; both readers' route predicates against the ELFs; the byte-level admin builders against the real admin instructions |
| `tests/raydium_v4_e2e.rs` | fork E2E on Fartcoin's AMM v4 pool (M3 exit criterion): lifecycle (list with D1 detection on the real graduated curve → deposit → listing window → Tier C opens → real sells, a profitable close → a real ×2.5 pump, two liquidations → an expiry → a Tier A open and close on day 7 → the lender's full withdrawal); S3 after a real `SetParams` halt; S2 via LP removal — unreachable at the defaults (σ 228 of 2,000 bps: opens are sized on locked depth), reached under an in-bound governance corner (λ_min 20 %, Tier C σ_max 5 %, `fee_size_cap_x100` 500); S1 on a real gap. Every short.win instruction is checked by the differential oracle |
| `tests/raydium_v4_attacks.rs` | ECON §12 on the real Fartcoin pool: exit sandwiches on a liquidation and an expiry (at once and after waiting), atomic EMA bundles on fresh and near-threshold positions and for S1 forcing, a held push, `fv_push` and the flip bundle on the thin S2 pool; the figures are printed (`-- --nocapture`), ECON's structural claims asserted |
| `tests/raydium_v4_support.rs` | the shared differential oracle of the two binaries above (every event field, lamport, token and lender-pool state recomputed from `shortwin-math` and the readers alone; the prologue, open, close, liquidate, expire, `sol_settle`) and their scenario builders (LP redistribution and byte-level LP removal; included as `mod support`; no tests of its own) |
| `tests/raydium_cpmm_e2e.rs` | fork E2E on CPMM (M3 exit criterion): the USELESS, Token-2022 and creator-fee journeys (list → deposit → open → real price moves → close / liquidation / expiry → S3 on an admin halt and on a fee hike above the hard cap → S2 on an out-of-model drained pool → lender exit, to the lamport), the settlement matrix on every fixture pool, U11 through short.win's own exits, O13 ages; exits held to 9 trace entries / 200 k CU, settlements to 2 (+3) / 150 k CU |
| `tests/latch_raydium_it.rs` | T-LATCH on the dumped AMM v4 / CPMM ProgramData (M3): real upgrades by the upgradeable loader (the Squads vault's signature forged), flagged by anyone; opens refused on AMM v4, CPMM and Token-2022 CPMM markets while cranks, top-ups, `sol_settle`, a liquidation after a real pump on the upgraded program, a close and an expiry keep working; revalidation only at the loader's slot; a second upgrade during review; both Raydium bits; pump and Jupiter bits never block Raydium; account checks on the real accounts; `[flag, open]` vs `[open, flag]`; one upgrade per slot; U5: the same CU on 1.4 MB, 0.79 MB and 10 MiB (`ray_it`'s `latch` module is its quick byte-patched subset) |
| `tests/trace_raydium_it.rs` | T-RAY-TRACE / T-TRACE on Raydium (M3): the exact call shape (program and stack height of every CPI) and trace entries of `create_market`, `crank_price`, `open_short`, `close_short`, `liquidate`, `expire`, `sol_settle` and the latch instructions on AMM v4, CPMM and Token-2022 CPMM; peak height 3; the worst-case transactions ≤ 64, and the runtime's refusal of one open more |
| `tests/tx_size_raydium.rs` | T-SIZE on Raydium (M3): every Raydium transaction (`open_short`, `close_short`, `liquidate`, `expire`, `sol_settle`, crank batches) in its worst case (both ComputeBudget instructions, a separate fee payer, a first-time borrower, the largest extras set) fits 1,232 B and 64 accounts as v0 through the protocol ALT — A28 as ARCH enumerates it and the table `scripts/build-alt.sh` builds — and executes that way on the fork; the keeper's crank + liquidate composite and a Jito tip measured |
| `tests/ray_sec_it/` | the M3 security review's findings as regressions on the fork (shares `ray_it/common.rs`): M3-SEC-1 (a CPMM config's trade fee set to 0 by the admin: no route — the S3 chain, frozen healthy positions, no uncapped S1 — and a fee-free risk view, halted or not; the differential of the fee split against the ELF), M3-SEC-2 (Raydium listings wait for the review of a flagged Raydium upgrade; the I17 listing hook), the M3-SEC-3 tripwire (the first listing pins a mint's venue until M4's `propose_pool`; M4 inverts it) |
| `tests/mock_cpamm.rs` | the mock venue itself: execution = `quote::cpmm` (zero creator fee) on generated pools and trade sequences (`MOCK_CPAMM_CASES`, default 96), the documented byte layout and PDAs, the builders vs the Anchor account structs, delegate-aware source, disable, fee change, `layout_version = 1`, over-delivery, required extras, a Token-2022 base with metadata, account validation, liquidity, swap CU and trace |
| `tests/security_m2.rs` | the M2 security review's findings as regressions: M2-SEC-1 (a pushed bucket rolled by a lender instruction no longer collapses the settlement box on an executable venue; S3 amounts stay within 2 % of the honest run), M2-SEC-2 (the `stable` extras set survives quick Emergency writes; Governance pins; the documented limits), M2-SEC-3 (templates take the canonical bump only), M2-SEC-4 (`sweep_trader` reopens lending after a full wipe; its account rules), the M2-SEC-5 tripwire (no `None` path in `sol_settle` until M5, on a planted state), and A49 (the release build refuses the mock venue) |
| `tests/security.rs` | the M1 security review's findings as regressions (timelock ceiling, fee-recipient checks and the sweep's rent guard, `Market` PDA checks, memo-required destinations, future-dated oracle updates, the apply window, the oracle-cache reset, the creation-fee bound; M1-SEC-1 as the owner decided: the Emergency vault cancels parameter changes only) and its refutation tests |

## Invariant hooks (ARCH §8.3: I1–I13, I16, I17 and A26 after every instruction; I14 structural)

`Env::send` runs them on every transaction:

- every existing `Market` an instruction names is tracked from then on, and after the
  transaction every market it created; `Env::install_market` and `Env::track_market` track
  explicitly;
- a transaction of several instructions is replayed prefix by prefix on a clone of the VM;
  after prefix `k`, instruction `k` is checked with `check_ix` against the state after prefix
  `k − 1` (the transaction's start for `k = 1`), with that instruction's own events and signers,
  and every tracked market with `check_all`;
- after the transaction: `check_ix` for its last instruction (against the state after all the
  others) and `check_all` on every tracked market; after a failed transaction `check_all`
  again.

"Admin-signed" (I4) is per instruction: an admin key (the harness's, or the config's current
governance / Emergency / coverage authority) among that instruction's signer metas.

A violation panics with the transaction's logs. Tests that write state straight into the VM
(the lender lab's borrower side) call `Env::assert_invariants` after each write.
`Env::check_invariants = false` is reserved for a test that corrupts the state on purpose
(the sweep refusing a broken I2, the planted fake `Market` of `security.rs`, copies of a
position or market away from their PDAs and overflowed fields in `add_margin_it.rs`, the hooks'
own tests in `lender_it/hooks.rs`, `engine_it/hooks.rs`, `invariants_m2_hooks.rs` and the
`hook_mutants_*` of `liquidate_it.rs` / `expire_it.rs`). The oracles' what-if runs on a clone of
the VM (`liquidate_support.rs`) keep the hooks on.

The I-P6 liveness check caps the realised SOL no lender is allocated after a wind-down at
0.01 SOL (`DEFAULT_LIVENESS_DUST_CAP`); the `pool_model` replays set it, before each program
operation, to the model's own wind-down figure (rounding plus rebase-orphan credit, ECON §9.5)
and assert the program's final figure equals the model's. They also relax `S = 0 ⇒ ΣB_Q = 0`
(`allow_shareless_borrow`): `pool_model` borrows orphan tokens with no shares, which M2's
`open_short` must refuse.

## Conventions

- Add a module to an existing binary, or a new binary under `tests/` when the area is large
  (each binary links LiteSVM once).
- Send every instruction through `Env::send` so the invariant hooks run.
- Positions are in I2 and I4, opens in I17 (the market's venue just before the open must be
  the event's), and listings in I17 too (M3-SEC-2: no `MarketCreated` on a venue whose latch
  mask is flagged just before the instruction); M2's own hooks are `src/invariants/positions.rs`. Keep each check on the state
  the program sees. A synthetic exit event in a hook test needs its position account removed
  first (I3: an exit leaves no position behind).
- Agave: LiteSVM 0.16.0 resolves the Agave runtime crates to 4.2.2, pinned in this crate's
  `Cargo.toml` and the workspace `Cargo.lock` (C1-14; the same line as `tests/bench/cu_math`).
