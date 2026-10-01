# shortwin-fuzz

short.win's invariant fuzz harness (ARCHITECTURE §8.3, §8.4; M7): seeded **action sequences** on
LiteSVM 0.16 through the program-test harness (`shortwin-litesvm-tests`), so every transaction
runs the harness's invariant hooks — I1–I13, I15–I19 and A26 after every instruction and after
every prefix of a multi-instruction transaction (I14 is structural, plus the fuzzer's omission
check) — against the test-venue build and the sha-checked mainnet-fork fixtures.

```sh
anchor build --tools-version v1.57 --arch v3 --ignore-keys   # target/deploy/{shortwin,mock_cpamm,complete_curve}.so
scripts/build-test-venue.sh                                  # target/test-venue/deploy/shortwin.so
cargo run -p shortwin-fuzz --release -- run --profile all --duration 1h --jobs 8 --out fuzz-out --minimize
cargo run -p shortwin-fuzz --release -- replay fuzz-out/crash-<profile>-<seed>-<sig>.json
cargo run -p shortwin-fuzz --release -- tmin   fuzz-out/crash-<profile>-<seed>-<sig>.json
cargo run -p shortwin-fuzz --release -- one --profile curve --seed 42 --actions 400   # one run, full summary
cargo test -p shortwin-fuzz --release                                                  # smoke, corpus replays, self-tests
```

The program paths are the harness's (`SHORTWIN_SO`, `SHORTWIN_TEST_VENUE_SO`, `MOCK_CPAMM_SO`,
`COMPLETE_CURVE_SO`). The fuzzer needs speed, not the deploy profile's fat LTO:
`CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16` builds it in ~2 min
(CI does the same). `run` without `--duration` does one run; `--runs N` caps the count; each job
(`--jobs`, default: every core) owns one seed stream (`seed + job + k·jobs`); a run's length varies
with its seed from a quarter of `--actions` to all of it (so the wind-down's liveness check meets
states at every depth); the exit status is 1 if any run failed. `run` prints progress every
minute and a summary (also `OUT/summary.json`).

## Why not Crucible

ARCH §8.4 named Crucible (`anchor fuzz`). Anchor 1.2.0 embeds crucible-fuzz-cli 0.2.1, whose
harness crates (crucible-test-context / crucible-fuzzer 0.3.0-alpha.1) pin `litesvm =0.15.2` and
`solana-program-runtime ~4.1.1`; this repository pins `litesvm =0.16.0` and the Agave line at
`~4.2.2` in one lockfile (C1-14), and the 4.1 and 4.2 runtime crates cannot coexist in it. A
Crucible harness could therefore not link `tests/litesvm` — the ~5,200 lines of I1–I18 hooks
and the fork fixtures — without a rewrite on a second Agave line. Crucible's stateful depth (15
actions by default) is also short for the protocol's clock-driven paths (a 3,600 s S3 wait,
72 h–30 d deadlines, 6 h vesting); a run here is 400 actions and warps up to three days per
action. LiteSVM 0.16's `register-tracing` feature can later add sBPF PC-coverage feedback without
a second runtime (not enabled: it would change the shared feature set).

## Runs, seeds, crash files

A **run** is `(profile, seed, action list)`: the world is set up (its choices drawn from
`seed ^ SETUP_SALT`: token program, fee, fixture, pool depth, oracle, the per-market notional
limit at 25 SOL / 1,000 SOL / its 10⁵ SOL bound, the wallet cap; and from `seed ^ M7_SALT`
whether O17's `coverage_authority` is installed — three runs in four, planted as an applied
`Authorities` change leaves it; inert otherwise), every action is applied, then the
**wind-down** (the liveness oracle below). The action list is a pure function of
`(profile, seed)` (`action::stream`); an action is `kind m a b` — the market slot and two numbers
the action interprets modulo the live state (positions, lenders, candidates), so every
subsequence of a recorded list is itself a valid list.

A failure — any panic: an invariant hook in `Env::send` (with the transaction's logs), a fuzzer
oracle (`FUZZ-ORACLE …`), the mock's differential exit oracle — is caught per action and written
as `crash-<profile>-<seed>-<hash>.json`: profile, seed, options, the action list up to and
including the failing action, the **signature** (panic location and first message line, digit
runs and keys masked) and the full message. `replay` re-runs it; `tmin` shrinks it by delta
debugging (ddmin, then single-action deletion) while the signature stays the same, writing
`min-….json`; `run --minimize` does that for the first failure of each signature.

Determinism: the actions are exactly reproducible; the keys a run creates (mints, borrowers) come
from the OS and differ between runs. Program logic does not depend on key bytes except through
PDA bumps (compute units), so a replay reproduces a failure's logic; the corpus reproducers were
each replayed repeatedly.

## Profiles (`--profile`)

| profile | build | venue(s) |
|---|---|---|
| `mock` | test-venue | one mock market (SPL or Token-2022, 0.05–1 % fee, thin / default / deep pool) with the mock's knobs: halt, fee, layout change, over-delivery, interface change |
| `mixed` | test-venue | two mock markets (SPL and Token-2022): global pauses, venue bits and config changes hit both |
| `curve` | release | the pump fork: `fresh` taken to 0–95 % by a real `buy_v2`, `legacy115`, `t22_115`, `near`; completion, real `migrate_v2`, the `→ PumpSwap` transition, then the PumpSwap venue on the new pool |
| `pumpswap` | release | the PumpSwap fork: `fresh` (boost running), `mature` (third-party LP), `cashback`, `legacy261`; a third-party LP |
| `amm-v4` | release | Fartcoin's AMM v4 pool; planted AMM v4 / CPMM candidates of the token |
| `cpmm` | release | USELESS, the Token-2022 pool or the creator-fee pool; an LP provider (the program's own `deposit` / `withdraw`); planted candidates |

## Actions (ARCH §8.4)

| group | actions |
|---|---|
| trading | `open` (log-uniform from just below 0.1 SOL of notional to just above `min(X/20, T)`; one in 32 with `min_proceeds = u64::MAX`, `max_margin = 0` or `max_open_fee = 0`), `open_near_cap` (the largest `q` accepted, by bisection on clones; the refusal one unit above is counted), `close` (one in 32 with `max_buyback_cost = 1`), `add_margin`; on the mock, opens and closes pick an extras selection (`none` / `current` / `previous` / `stable`) |
| lenders | `deposit` (one in 101 of zero tokens), `withdraw` (a fraction, all shares, or exactly the pro-rata allowance), `claim_sol` ×1–4, `cover_deficit`, `sync_lender`, `sweep_trader`, `dust_sweeps` (1 lamport to the trader and a sweep every minute), `lenders_out` (all out, optionally a dust holder stays, then a sweep), `jit_deposit` (`[deposit, exit, withdraw]`) |
| keeper and time | `keeper_seconds` (crank every market, then liquidate / expire / sol_settle every position, each second), `crank`, `keeper_exit`, `silence` (41–940 s without observation — past `COVERAGE_GAP_S`, 40 s since the owner's decision of 2026-10-01 (OWNER-M7-12; was 31–930 s against 30 s): `CoverageGap`; with the role installed, two in three are **degraded mode** (O17): the keeper's `set_coverage_pause(true)` on every market, cleared 1–8 actions later), `warp` (1 s – 3 d), `warp_to_deadline` (±, or past the S3 tail), `outage` (venue halted, fee above the hard cap, or halted and flagged, 130 s – 2 h with the keeper settling: the no-route chain, S3, T-S3-BROKEN) |
| prices and liquidity | `trade`, `shock` (×/÷ 1.05–10, held), `ramp`, `liquidity` (add / remove), `lp_wrapped_crank` (`[withdraw, crank, deposit]` and `[deposit, crank, withdraw]`), `deposit_wrapped_open`, `withdraw_wrapped_exit` |
| atomic bundles (ECON §12) | `pump_crank_dump` (`[push, crank, unwind]` 1–20 s in a row), `contrary_low` (a contrary low observation every second, with or without the keeper's, then `sol_settle`), `attacker_seconds` (1–6 attacker-only seconds up / down / flip before a settlement), `push_then_freeze`, `sandwich_exit` (`[front, close / liquidate / expire / sol_settle, back]`), `held_sandwich`, `dump_around_settle` (a dump or an LP deposit around `sol_settle`) |
| pauses and flags | `gates` (opens paused / route broken / venue halted on the market, I10's fuzz action), `global_pause`, `disable_venue` (re-enabled by Governance after the timelock), `route_broken_venue`, `venue_halt`, `fee_shock`, `layout_shock`, `over_delivery`, `extras_shock` (the mock requires the per-mint PDA a template derives, or fresh keys no template names); each disturbance lasts 1–8 further actions |
| degraded mode (O17, M7) | `coverage_pause`: `set_coverage_pause` pausing or clearing one market, signed by the role's key, Governance, the Emergency vault or a stranger, with the keeper's reason or a bad one — the outcome predicted exactly (`CoverageAuthorityDisabled` while the role is inert, whoever signs — except the Emergency's clear; `Unauthorized`; `InvalidParams`); or the role rotated by Governance's timelocked `Authorities` change (a new key, or the default key: inert); ★`coverage_bracket` (M7-SEC-3): `[set_coverage_pause(false), open_short, set_coverage_pause(true)]` by the role's key — refused `CoveragePaused` on a paused market (the clear delay), ordinary gates otherwise; ★`emergency_clear` (OWNER-M7-3 (b), owner decision of 2026-10-01): the Emergency vault clears a market's coverage pause (after a hijacked key's set, or a no-op) with the role installed, unset or rotated — the whole VM unchanged but the flag and its clock (I4's coverage clause), an open in the clear delay refused `CoveragePaused` —, its set refused (`Unauthorized`, or `CoverageAuthorityDisabled` while the role is unset), and `[Emergency clear, open_short]` refused `CoveragePaused` on a paused market |
| price-wrapped opens (M7-SEC-6) | ★`wrapped_open`: a sale or a buy of the venue within the open's spot–EMA guard, `open_short`, unwound or not, in one transaction (the M5-SEC-3 / M7-SEC-4 / M7-SEC-5 class); the open's terms are I19's |
| extras sets (mock) | `extras_set`: the Emergency's `set_venue_extras` on the sell leg, the buy leg or both — the template the interface change needs, a hostile fixed-key entry, or an empty set |
| mint filter | `mint_shock`: a freeze authority planted on the market's mint for 1–8 actions, `flag_unsafe_mint` by anyone on one in eight (one-way), or on a safe mint (`MintSafe` / `MintUnsafeFlagged`) |
| fees | `sweep_protocol_fees` (anyone, to `config.fee_recipient`) |
| upgrades | `upgrade_shock` (a venue program's ProgramData deploy slot changes, with or without `flag_venue_upgraded`), `revalidate` |
| donations | `donate` to the trader, the `trader_wsol` address, `ATA(trader, WSOL)`, the PumpSwap boost ATA, the pool vault (tokens), the market vault, a position |
| governance and oracle | `config_change` (a parameter of `shortwin_math::params::Rule` at its lower or upper bound or midway, valid, proposed, timelock waited out, applied — as a `Params`, an `EmaParams` (new version: adopted at once without positions, forced after `ema_force_delay_s`) or a `Kink` change; or an `OracleFeed` change to a second account and back, which empties the cache), `config_cancel` (a change cancelled by Governance or the Emergency vault, a `Timelock` change the Emergency may not cancel, or one left past `CONFIG_APPLY_WINDOW_S`), `oracle_crank` (`crank_sol_usd` on a re-planted mainnet `PriceUpdateV2` at the configured account: ±30 % against a 20 % step bound, 0–4 min old; or future-dated, too wide a confidence interval, not owned by the receiver) |
| pool selection | `plant_candidate`, `propose_pool`, `execute_switch`, `fast_path` (incumbent halted or over the fee cap past `pool_fast_route_s`, a window proposal, the window waited out, the execution) |
| graduation | `complete_curve`, `migrate`, `graduation_halt` (PumpSwap `disable_flags` bit 0, M6-SEC-3) |
| I14 | `omit_account`, `replace_account` (a remaining account of open / close / liquidate / sol_settle / crank omitted or replaced by a fresh key) |

## Oracles

- **The hooks** (`Env::send`): I1–I13, I15–I19 and A26 after every instruction, each proper prefix of
  a bundle replayed on a clone; I14's structural part is the harness's. ★I19 (M7-SEC-6) recomputes
  every open's terms — locked terms, deadline, tier, every cap, `N_open` and the notional limits, the
  D4 margin, the fee — from the pre-state and the harness's own venue views.
- **The gate oracle knows the coverage clear delay** (M7-SEC-3): an open within
  `COVERAGE_CLEAR_DELAY_S` of a clear must be refused.
- **I10 / I17 gates** (`world::open_gate`): an `open_short` landing while the market's or the global
  opens pause, the market's or venue's route-broken flag, a disabled venue bit, the upgrade latch or a
  non-executable venue (the harness's view) holds just before it; an exit or top-up refused with an
  open-only code (paused, flagged, latched, disabled, caps, notional, oracle, listing window).
- **Differential exits (mock)**: every keeper `liquidate` / `expire` on an undisturbed mock market goes
  through `tests/litesvm/tests/liquidate_support.rs`'s oracle (included with `#[path]`): the decision —
  including `ExitBoundNotMet` against `⌊B̂_ref·(10⁴ + δ)/10⁴⌋`, so I11's liveness clause — and every
  lamport and token of the waterfall recomputed from `shortwin-math` on the pre-state.
- **I14 omissions**: an instruction with a remaining account omitted must fail (the hooks check nothing
  changed); an accepted replacement by a fresh key is reported as an anomaly for review.
- **`add_margin`** is never refused (ARCH §3.5).
- **I4's coverage clause (O17)**: every `set_coverage_pause` that lands changes nothing on the whole VM
  (every account short.win, the token programs, the venues and the System program own, lamports
  included; the fee payer aside) but bit 6 of that market's `flags`, which then equals `paused`; every
  refusal is the predicted code. (The harness's I4 hook checks the same clause on every short.win
  account and the tracked pool vaults, `invariants::i4_coverage_authority_changes_only_its_flag`;
  the fuzzer's check is wider: the whole VM.) Opens are gated on the flag (the gate oracle above),
  exits never are.
- **The mint filter**: exits, top-ups and a lender's `withdraw` never return a mint-filter code
  (`MintFreezeAuthority`, `MintDisallowedExtension`, `MintMalformed`, `MintUnsafeFlagged`; ARCH §3.0
  step 3); opens are refused while a freeze authority is planted or the market is flagged;
  `flag_unsafe_mint` lands exactly when the mint fails the filter and the market is not yet flagged.
- **Governance**: a proposed change never applies before its timelock (`TimelockNotElapsed`) or after
  its apply window (`PendingChangeExpired`), never after a cancel (`NoPendingChange`); the Emergency
  vault cannot cancel a `Timelock` change (`EmergencyCannotCancel`); a cancelled or lapsed change leaves
  the parameters and the timelock as they were.
- **The oracle**: `crank_sol_usd` never caches a future-dated update, one with too wide a confidence
  interval, or one from an account the receiver does not own.
- **Client bounds**: an open with `min_proceeds = u64::MAX` or a close with `max_buyback_cost = 1`
  never lands; a deposit of zero tokens never lands.
- **Liveness (I8 / I-P6, I-E11)**: the wind-down clears every gate and disturbance, restores prices,
  closes or exits every position — with the keeper's per-second coverage for `settle_persist_n` + 30 s
  (an armed settlement's persistence), then past every deadline with the keeper's 30-second cadence
  (inside `S3_OBS_GAP_S`, so the S3 chain never restarts) for `s3_wait_s` after the migration pause
  and the graduation grace, and two more days — waits out the lenders' locks, syncs lenders behind
  the share epoch and takes every lender out at its allowance; the market must end with no position,
  no receivable, no rent aggregate and nothing borrowed, and the hooks' dust bound holds throughout.
  The keeper never migrates a completed curve in the wind-down: the S3 path must settle a market left
  `PumpMigrating`. A live curve whose real token reserves cannot buy a position back (`Q > rT`: its
  close refuses `CurveStraddle`) is completed first — the straddle's first step (ARCH §5.4), which
  one price-restore push (at most 50×) cannot always undo — and then settles the same way
  (`fuzz:wind-down-curve-completed`; FUZZ-M7D-1).

**Anomalies** (in the summary, not failures): a panic, compute exhaustion or framework error (an
Anchor code of short.win's own, not a venue's error surfacing through its CPI — those are counted as
`venue-cpi-refusal:…` events) inside a short.win instruction the fuzzer built well-formed, an
accepted I14 replacement, a wind-down withdrawal refused even at the allowance (shares worth at most
one raw token are counted as `wind-down-dust-lender` instead).

## Coverage

The summary counts actions by kind (and skips), short.win instructions landed and refused, refusals
by `instruction:Error`, and events (`Liquidate`, `Expire`, `SolSettle(S1/S2/S3)`, `SettleArmed`,
`CoverageGap`, `CoveragePauseSet`, `PoolSwitched`, `VenueStateChanged`, `MintSafetyTripped`,
`fuzz:graduated`, the differential oracle's decisions, near-cap refusals by cap, venue CPI refusals),
and prints **coverage proxies**: short.win instructions that landed (of 30), events emitted, and
short.win error codes returned (of the codes `errors.rs` declares), with the codes never returned —
a branch proxy (`summary.json`'s `coverage`). sBPF PC coverage is not measured (LiteSVM 0.16's
`register-tracing` would add it without a second runtime; it changes the shared feature set).

Not fuzzed: `create_market` and `initialize_config` (setup only), AMM v4 third-party liquidity
(the harness has CPMM LP builders only: `tests/litesvm/src/lp.rs`), `revalidate_venue` with a wrong
slot, extras sets on the real venues.

## Findings

`--include-known` turns off the avoidance of actions that only re-trigger an open finding (none is
open today); replays use the crash file's own options. A corpus file marked `"status": "fixed"` is a
regression: `corpus_replays` requires it to pass.

| id | reproducer | summary | status |
|---|---|---|---|
| FUZZ-M7-1 | `corpus/fuzz-m7-1-curve-trader-wsol.json` (2 actions) | a System transfer to a curve market's `trader_wsol` address survived a curve leg: the curve never names that address (ARCH §2.4 rule 2 normalises it on the WSOL venues only; `TraderAccounts::post_conditions` checks it there) while I7's hook required it closed after every leg; `sweep_trader` recovers the lamports | **resolved in the M7 integration, no program change**: I7's `trader_wsol` clause restated for the curve as a delta (ARCH §2.4 rule 5, §8.3 — the leg leaves the address's lamports unchanged, System-owned, no data; after a WSOL leg it holds nothing), the hook and its mutant test (`invariants_m2_hooks::i7_trader_wsol_on_a_curve_leg_is_a_delta`), a fork regression (`pump_it::exits::a_donation_to_the_trader_wsol_address_survives_curve_legs_until_swept`); the reproducer passes; the fuzzer donates to the address on curve markets again |
| FUZZ-M7D-1 | `corpus/fuzz-m7d-1-curve-straddle-wind-down.json` (3 actions) | the liveness oracle on a fresh curve: two `shock`s buying 98 % of `rT` around an open left `Q > rT`, so `close_short`, `liquidate` and `expire` refused `CurveStraddle`; the wind-down's price restore (one push, at most 50×) did not bring `rT` back above `Q` and its keeper never completed the curve, so the position stayed open (found by the M7D fixes' 10-minute smoke, seed 1790845047; the harness's gap, not the program's) | **resolved in the fuzzer, no program change**: the wind-down completes such a curve (the straddle's first step, ARCH §5.4) and leaves it `PumpMigrating`, as every completed curve, for the S3 path to settle (`SolSettle(S3)`); the liveness oracle is unchanged; the reproducer passes |

## Runs so far (M7)

| run | build | wall | jobs | runs | actions | transactions | hook checkpoints | failures | anomalies |
|---|---|---|---|---|---|---|---|---|---|
| fuzz track, `--seed 20261001` | the M7 program | 3,903 s | 5 | 5,598 | 1,055,276 | 13,591,797 | 20,488,740 | 0 | 4 (the wind-down's `NegativePending`, fixed in the wind-down) |
| fuzz track, confirmation | the M7 program | 724 s | 5 | 1,190 | 225,106 | 2,879,543 | 4,327,270 | 0 | 0 |
| integration, `--seed 20261010` | the final tree (FUZZ-M7-1 resolved) | 605 s | 5 | 822 | 155,022 | 1,961,976 | 2,944,773 | 0 | 0 |
| M7 security review fixes, `--seed 20261011` | the fixed tree (M7-SEC-1 … -9; I19, `wrapped_open`, `coverage_bracket`) | 617 s | 5 | 806 | 155,055 | 2,036,820 | 3,109,137 | 0 | 0 |
| the owner's decisions of 2026-10-01 (default seed) | `COVERAGE_GAP_S` 40, `oracle_stale_s` 600, the Emergency's clear (`emergency_clear`; `silence` past 40 s) | 606 s | 4 | 1,176 | 167,812 | 2,181,234 | 3,342,239 | 0 | 0 |
| the M7D fixes (keeper only; the program byte-identical), default seed | the decisions' tree | 640 s | 4 | 1,103 | 157,856 | 2,113,224 | 3,276,171 | 1 (FUZZ-M7D-1: the wind-down's missing curve completion) | 0 |
| the M7D fixes, after FUZZ-M7D-1 (an interim wind-down that also migrated) | — | 601 s | 4 | 1,188 | 171,370 | 2,289,531 | 3,486,368 | 0 | 0 |
| the M7D fixes, final (default seed) | the wind-down completes a straddled curve and leaves it to S3 | 603 s | 4 | 1,215 | 172,918 | 2,303,424 | 3,513,992 | 0 | 0 |

≈ 2.31 h of wall clock (≈ 10.8 job-hours) in all, plus the M7 security review's runs (mutant builds)
and the mutation runs (below). In the fixes' run, 4,327 `wrapped_open` and 1,127 `coverage_bracket`
actions: 832 wrapped opens landed with I19 recomputing their terms, 585 brackets refused at the clear delay.
In the owner's-decisions run, 3,104 `emergency_clear` actions: 541 clears of a set flag and 780
no-op clears landed under I4's coverage clause (whole VM), 875 opens in the clear delay after an
Emergency clear (alone or bundled) refused `CoveragePaused`; 601 `coverage_bracket`s refused;
5,840 `CoverageGap` events. **The M7 exit criterion "fuzz clean ≥ 24 h"
is not met yet** (ARCHITECTURE §9, OWNER-M7-16): run `run --profile all --duration 24h --jobs 8
--actions 400 --minimize` locally, or let `fuzz-nightly` run for three nights once the branch is on
main (GitHub runs `schedule` on the default branch only).

**Mutation testing (M7-SEC-6).** `scripts/mutants.sh` builds this checkout's tests and fuzzer once
(the oracle) and each of the review's seven one-line mutants of the program (D4 without its EMA
leg, the fee-scaled size cap doubled, the wallet cap removed, Tier C as B, the coverage clear
immediate, the curve caps on the live reserves, the pump caps at the live fee), and requires its
named tests and a short fuzz of its profile to fail against each: a surviving mutant is a missing
oracle. Before I19, the first four survived 15 minutes of `mock,mixed` fuzzing (562 runs) and
M5-SEC-3's revert 12 minutes of `curve` (1,118 runs); with it, every one of the seven is killed by
its tests and by a 3-minute fuzz of its profile (2026-10-01, on the final tree: 10 / 10 / 10 / 133
failures on `mock` for the margin, fee-cap, wallet and tier mutants, 27 for the clear delay, 16 on
`curve`, 1 on `pumpswap`; the wallet mutant also fails `caps_boundary_it`'s M7-SEC-9 regression).

## CI

`.github/workflows/ci.yml`'s `fuzz-nightly` (the `schedule` trigger on main, or `workflow_dispatch`
with `fuzz_duration`): one 5 h shard per profile on every core of its runner, `--minimize`, crash
files uploaded, the job failing on any crash; the `mock` shard first runs `cargo test --release -p
shortwin-fuzz` (the fixed-seed smoke of every profile, ≈ 15 s after the build, the corpus replays and
the harness's self-tests). On every push and pull request the `fuzz-smoke` job runs the same
`cargo test --release -p shortwin-fuzz` (the smoke, the corpus replays — FUZZ-M7-1's reproducer must
pass — and the self-tests); a failed smoke uploads its crash files.
