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
cargo test -p shortwin-litesvm-tests --test it cu_baseline -- --nocapture   # admin CU table
SHORTWIN_TRACES=full cargo test --release -p shortwin-litesvm-tests --test lender_it traces::   # T-POT, ~4 min (CI job `t-pot`)
```

## Layout

| Path | What |
|---|---|
| `src/env.rs` | `Env`: LiteSVM + `shortwin.so` (release, `Env::new`; or the test-venue build, `Env::new_test_venue_program` / `$SHORTWIN_TEST_VENUE_SO`), admin keys (upgrade authority recorded in the ProgramData, Governance, Emergency, fee recipient), clock control (`warp`, `set_clock`), `send` with the invariant hooks |
| `src/engine.rs` | the M2 engine on the mock: `Env::test_venue` (test-venue build + mock + config with the mock's venue bit + the mock's `VenueExtras` account), `Env::mock_market` (pool, liquidity, `create_market(Mock)`, lender deposit, warm-up — real instructions), `MockMarket` (keys, ReadSet / TradeExtras), builders of every M2 instruction, `Borrower`, `Env::open` / `Env::close` |
| `src/drive.rs` | time and price drivers: `Actor` (trades the mock directly), `Env::trade`, `Env::push_price`, `Env::tick` (one crank per second), `Env::spot` |
| `src/attack.rs` | atomic bundles in one transaction: `Env::bundle`, `Env::sandwich` (front-run, victim, back-run; the attacker's result) |
| `src/ix.rs` | instruction builders for every M1 instruction |
| `src/invariants.rs` | `check_all(&svm, market)` (I1, I2, I8) and `snapshot` + `check_ix` (I4, I17, per instruction) |
| `src/invariants/positions.rs` | the position engine's invariants: I6, I7 (`pool_vault` delegate), I13 (ring, newest entry, one-second lag) per market; I3, A26 (snapshot rules), I5, I7, I9, I10, I11, I12 (amounts, persistence, the S3 chain, `p'` per corner), I13 (charges, re-quotes on the mock, EMA steps), I16 (no observation while not executable; the route chains) per instruction (module table) |
| `src/invariants/lender.rs` | the lender pool's part of I1 (both directions) and I8 (claims, entitlement, allowances with each lender touched alone, locks, calendar, liveness, `S = 0 ⇒ ΣB_Q = 0`) and the instruction-level lender bounds (allowance per withdrawal, claim ≤ entitlement, token conservation, no tokens without shares, locks never decrease) |
| `src/fixtures.rs` | mock venue ProgramData (T-LATCH), raw zero-copy / Borsh / token-account writers, `install_market` |
| `src/mock.rs` | the TEST-ONLY mock venue `mock_cpamm` (M2; `tests/mock_cpamm/INTERFACE.md`): `Env::load_mock_cpamm` (`target/deploy/mock_cpamm.so` or `$MOCK_CPAMM_SO`), pools on SPL / Token-2022 base mints, liquidity, knobs, layout-aware reserves, WSOL and base trader accounts, swap builders, short.win's `quote::cpmm` view of a pool |
| `src/fixtures/mints.rs` | mint fixtures: one real Token-2022 mint per extension, forged TLV mints, the pump.fun shape |
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
  the event's); M2's own hooks are `src/invariants/positions.rs`. Keep each check on the state
  the program sees. A synthetic exit event in a hook test needs its position account removed
  first (I3: an exit leaves no position behind).
- Agave: LiteSVM 0.16.0 resolves the Agave runtime crates to 4.2.2, pinned in this crate's
  `Cargo.toml` and the workspace `Cargo.lock` (C1-14; the same line as `tests/bench/cu_math`).
