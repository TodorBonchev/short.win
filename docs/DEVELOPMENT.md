# Development

How to set up the toolchain, build and test short.win. The pins come from
[`design/ARCHITECTURE.md`](design/ARCHITECTURE.md) §1.1; the layout from §1.2.

## Pins

| Tool | Version | Pinned in |
|---|---|---|
| Host Rust | 1.98.1 (+ rustfmt, clippy, `wasm32-unknown-unknown`) | `rust-toolchain.toml` |
| Agave / Solana CLI | 4.1.2 | `Anchor.toml [toolchain]`, `Cargo.toml [workspace.metadata.cli]` |
| Anchor CLI and `anchor-lang` | 1.2.0 | `Anchor.toml [toolchain]`, `programs/shortwin/Cargo.toml` |
| Platform-tools / SBPF arch | v1.57 (rustc 1.95) / v3 | the build command (below) |
| Node | 24.21.0 LTS | `mise.toml` |
| pnpm | 12.7.0 | `mise.toml`, `package.json` `packageManager` (with sha512) |
| Python | 3.14, standard library only | test-vector generators, `scripts/config_gate.py`, `scripts/dump-fixtures.py` |

On-chain crates declare `rust-version = "1.89"`. They are compiled by the platform-tools
rustc (1.95), not the host toolchain, and Cargo's resolver 3 uses that `rust-version` to
prefer dependency versions that still build there.

## Setup

**Rust.** Install [rustup](https://rustup.rs). The first `cargo` command in the repository
installs 1.98.1 with rustfmt, clippy and the wasm32 target from `rust-toolchain.toml`
(or run `rustup toolchain install`).

**Agave CLI 4.1.2.**

```sh
sh -c "$(curl -sSfL https://release.anza.xyz/v4.1.2/install)"
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
agave-install init 4.1.2
solana --version            # solana-cli 4.1.2
```

If another version is active, `anchor` switches to 4.1.2 through `agave-install` because of
`[toolchain] solana_version` (and back afterwards).

**Anchor CLI 1.2.0.** Use the canonical repository `github.com/otter-sec/anchor`:

```sh
cargo install --git https://github.com/otter-sec/anchor --tag v1.2.0 avm --locked
avm install 1.2.0 && avm use 1.2.0
anchor --version            # anchor-cli 1.2.0
```

Installing the CLI directly also works and is what CI does:
`cargo install --git https://github.com/otter-sec/anchor --tag v1.2.0 anchor-cli --locked`.

**Platform-tools v1.57** are downloaded to `~/.cache/solana/v1.57` by the first
`anchor build`.

**Node and pnpm.** `mise install` in the repository root (see [mise](https://mise.jdx.dev)),
then `pnpm install --frozen-lockfile`. The only JS package so far is the Codama client
`clients/js` (`@shortwin/client`); `pnpm-workspace.yaml` also lists `packages/*` and `apps/*`.

## The build rule

**Always build the program with `anchor build --tools-version v1.57 --arch v3`. Never run a
bare `cargo build-sbf`.**

Agave 4.1.2 ships `cargo-build-sbf` 4.1.0, which defaults to platform-tools **v1.54**
(rustc 1.89) and SBPF arch **v0**. A bare `cargo build-sbf` therefore produces a different
program binary from the one Anchor, CI and the verifiable build produce. Anchor 1.2.0
defaults to v1.57 / v3 already; we pass the flags anyway so every command line states them.

Check an artifact: `od -An -t x4 -j 48 -N 4 target/deploy/shortwin.so` prints `00000003`
(ELF `e_flags` = SBPF v3). Outputs: `target/deploy/shortwin.so`, `target/idl/shortwin.json`,
`target/types/shortwin.ts`; since M5 also the straddle helper's `target/deploy/complete_curve.so`
and `target/idl/complete_curve.json`.

Verifiable builds (M10) use solana-verify 0.5.2 with the same pins; the `=` form of the
option is required:
`solana-verify build --library-name shortwin --arch v3 --cargo-build-sbf-args="--tools-version v1.57"`.

`anchor build` builds the three programs of the Anchor workspace: `shortwin`, the stateless
straddle helper `programs/complete_curve` (M5; ARCH §5.4, A66: `complete_curve(max_sol_cost)`
reads the curve's `rT` at execution and buys it through pump `buy_v2` with its own signer as
`user`; deployed next to short.win) and the TEST-ONLY mock venue `tests/mock_cpamm`
(`target/deploy/mock_cpamm.so`, below). **Deploy by name** (`anchor deploy -p shortwin`,
`anchor deploy -p complete_curve`, or the verified-build path): a bare `anchor deploy` would also
deploy the mock.

### The test-venue build (M2; A49)

The position engine is tested against a TEST-ONLY constant-product venue, `tests/mock_cpamm`
(program id `BPAZFPkmVtYEWEZHCzFUns7oybh1VWfAvmu3ofM2mQi7`; its interface, byte offsets and
the reader contract are in [`tests/mock_cpamm/INTERFACE.md`](../tests/mock_cpamm/INTERFACE.md)).
short.win reaches it only through `VenueKind::Mock`, which exists only with the Cargo feature
`test-venue`. That feature goes into a **separate build** that never replaces the release
artifact:

```sh
anchor build --tools-version v1.57 --arch v3   # release: target/deploy/shortwin.so (no test-venue), mock_cpamm.so
scripts/build-test-venue.sh                    # TEST-ONLY: target/test-venue/deploy/shortwin.so
```

The script runs `anchor build --tools-version v1.57 --arch v3 --ignore-keys --no-idl -p shortwin
-- --features test-venue` in its own target directory (`target/test-venue`, or
`SHORTWIN_TV_TARGET`), checks SBPF v3 and that the build carries the marker
`shortwin:test-venue:mock-cpamm`. CI checks the other direction: the release `shortwin.so`
contains neither that marker nor the mock's program id (as raw bytes, as its four 8-byte
words, or as the four `lddw` immediates it compiles to), and `mock_cpamm.so` and the
test-venue build do. The mock's keypair (`tests/mock_cpamm/keys/`) is gitignored like every
program keypair.

Because `tests/litesvm` enables `shortwin/test-venue`, a workspace-wide `cargo test` or
`cargo clippy` compiles the program **with** the feature (Cargo unifies features); run
`cargo clippy -p shortwin --all-targets` and `cargo test -p shortwin` on their own to cover the
release feature set (CI does both).

## Everyday commands

```sh
cargo fmt --all --check
cargo clippy -p shortwin-math --all-targets -- -D warnings
cargo clippy --workspace --all-targets -- -D warnings
PROPTEST_CASES=2000 cargo test --workspace           # the CI count; LiteSVM tests need the .so (below)
PROPTEST_CASES=20000 cargo test -p shortwin-math     # deeper local run (can only raise the count)
cargo build -p shortwin-math-wasm --target wasm32-unknown-unknown --release
cargo check -p shortwin --target wasm32-unknown-unknown   # T-LAYOUT wasm32 leg
anchor build --tools-version v1.57 --arch v3
cargo deny check
cargo audit --deny warnings --ignore RUSTSEC-2025-0141 --ignore RUSTSEC-2021-0139 \
  --ignore RUSTSEC-2024-0388 --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2025-0161
```

(The `test` job runs `shortwin-math` at `PROPTEST_CASES=2000`; the M2 gate also runs the
whole workspace at that count.) The ignored advisories are the ones in `deny.toml`: all
"unmaintained", none a vulnerability;
bincode comes with anchor-lang 1.2.0, the other four only with the LiteSVM test harness.

Test vectors from the normative sims: `python3 crates/shortwin-math/tests/vectors/<track>/gen_<topic>.py`.
The conventions are in [`crates/shortwin-math/tests/vectors/README.md`](../crates/shortwin-math/tests/vectors/README.md).

On a machine with little RAM, `CARGO_BUILD_JOBS=4` keeps the LiteSVM and Mollusk builds within
about 6 GB.

## Program tests (LiteSVM)

The program tests (ARCHITECTURE §8.2, §8.3) are the crate `tests/litesvm`
(`shortwin-litesvm-tests`, LiteSVM 0.16.0 on the Agave 4.2.2 runtime crates pinned in the
workspace `Cargo.lock`, C1-14). They load the SBF builds, so build them first:

```sh
anchor build --tools-version v1.57 --arch v3          # target/deploy/shortwin.so and mock_cpamm.so
scripts/build-test-venue.sh                           # target/test-venue/deploy/shortwin.so (M2 tests)
cargo test -p shortwin-litesvm-tests                  # every binary below, ~15 min (debug)
cargo test -p shortwin -p shortwin-venue-state        # host unit tests of the program and readers
```

Other builds: `SHORTWIN_SO` (the release program: the M1 binaries and every Raydium and pump
binary), `SHORTWIN_TEST_VENUE_SO` (the test-venue program, M2 binaries), `MOCK_CPAMM_SO` (the mock
venue), `COMPLETE_CURVE_SO` (the straddle helper: `straddle_it`, the `m5_cu` bench and the
keeper's graduation scenarios); `SHORTWIN_PRE_M4R_SO` and `SHORTWIN_PRE_M4R_TEST_VENUE_SO` name a
release and a test-venue build of the tree before M4R, for `switch_confirm_it`'s ignored lockstep
differentials only. `Env::test_venue()` loads the test-venue program and the mock; `Env::new()` the release
program only; `Env::ray_fork()` the release program and the dumped Raydium programs (below);
`Env::fork_pump_coin(coin)` the release program, the dumped pump, pump-fees and PumpSwap programs,
pump's statics, one coin's accounts, mainnet's Rent and the pump snapshot's clock (M5);
`Env::fork_pamm_coin(coin)` (M6) the same with PumpSwap's statics and one canonical pool role of
the `pamm_state` dump (`Env::plant_negative_vqr` swaps in the synthetic negative-VQR pool). A
stale `target/deploy/shortwin.so` fails the Raydium and pump binaries in confusing ways: rebuild
after any program change (or point `SHORTWIN_SO` at a private build).

**The curve venue bit in tests (M5-SEC-4).** The M5 build refuses to enable the pump-curve venue
(`constants::VENUE_BITS_UNFINISHED`; `initialize_config` and `EnableVenues` fail `InvalidParams`),
so no deployment of it takes curve opens before M6's `→ PumpSwap` transition. The harness's
`Env::initialize_config` therefore sends its arguments without that bit and then writes it into
`GlobalConfig.venue_enabled` directly (`Env::force_venue_bits`, TEST-ONLY), so the curve suites run
the venue as M6 will enable it; `Env::initialize_config_as_sent` sends the arguments unchanged
(the gate's own test). **M6 emptied the set** (`VENUE_BITS_UNFINISHED = 0`): the forcing is now inert and `initialize_config` sends its arguments unchanged; the mechanism stays for a later unfinished venue.

| Binary | Covers |
|---|---|
| `--test it` | admin and config (T-CONFIG on-chain: every compiled bound, the timelock), pause, T-LATCH (mock ProgramData), T-LAYOUT (runtime offsets), lender constraint cases, the harness, the admin CU table (`cu_baseline -- --nocapture`) |
| `--test market_create` | `create_market` (exact state, creation fee, griefing, detection keys, venue hints) and `flag_unsafe_mint` |
| `--test mint_filter_extensions` | the SPEC §3 allowlist filter: one fixture per Token-2022 extension, program vs byte-level filter |
| `--test lender_it` | the six lender-pool instructions, T-POT, the lender adversarial cases (C0-6, C0-9, C0-13, R3A-2, R3A-10, R3B-1, R3B-2, V2-2, V2-12), `pool_model` trace replays |
| `--test oracle_crank` | `crank_sol_usd` on the committed mainnet `PriceUpdateV2` dump (`tests/fixtures/`, checked by `sha256sum -c SHA256SUMS` there) |
| `--test security` | the M1 security review's findings as regressions and its refutation tests |
| `--test security_m2` | the M2 security review's findings as regressions: M2-SEC-1 (no box collapse on an executable venue), M2-SEC-2 (the `stable` extras set), M2-SEC-3 (canonical template bumps), M2-SEC-4 (`sweep_trader` reopens lending after a full wipe), M2-SEC-5 **inverted by M5** (`m2_sec_5_sol_settle_reaches_s3_on_a_none_market`: `sol_settle` on a `None` market takes the detection set as ReadSet and settles S3 after the chain, FV at the cap-only view; a planted state, hooks off from the plant — the real transition runs with hooks on in `pump_it::graduation`), and A49 (the release build refuses the mock venue) |
| `--test mock_cpamm` | the mock venue itself: execution equals `shortwin-math`'s `quote::cpmm` exactly (a differential proptest, `MOCK_CPAMM_CASES`, default 96), the layout at its documented offsets, every knob (disable, fee, layout change, over-delivery, required extras), delegate-aware sources, Token-2022 bases |
| `--test engine_it` | M2 engine: `open_short` / `close_short` flows and every refusal reachable on the mock, the prologue, trader normalisation and griefing, over-delivery, sandwiches around opens and closes, the O6 powers and T-REPLAY-PDA, T-TRACE / CU, the structural I14 test, the hooks' own mutants, and the seeded random walk (`ENGINE_FUZZ_SEEDS`, default 4, and `ENGINE_FUZZ_STEPS`, default 120) over opens, closes, top-ups, keeper exits, shocks, deadlines and I10's pause-and-flag action |
| `--test add_margin_it` | `add_margin` (every gate ignored, snapshots unchanged, I14 on its accounts, a random walk: `ADD_MARGIN_WALK_SEEDS`) |
| `--test crank_price_it` | `crank_price`: `CoverageGap`, same-second floods, the ring rewrite, forced EMA adoption |
| `--test liquidate_it`, `--test expire_it` | the keeper exits against an oracle recomputed from `shortwin-math` alone: every waterfall branch, every refusal, what exits ignore (I10), donations, over-delivery, extras, I14, liveness (I-E11), T-TRACE / CU, hook mutants |
| `--test attacks_exit_it` | ECON §12's exit attacks on ECON's `RA` market, to the lamport: `sandwich.py`, `fixed_delta.py`, `block_spot.py`, `ema_atomic.py`, `ema_near.py`, `ema_atomic_multi.py` (the OI-cap basket: atomic, held, and S1 forcing), `ptf_sandwich.py`, `fvcap.py` (the RA rows), `cascade.py` |
| `--test settle_it` | `sol_settle`: arming, S1 / S2 / S3, epoch records, T-BOX (seeded push / freeze / dump runs) |
| `--test route_it` | T-S3-BROKEN (the flag, a fee above the hard cap, a disabled venue, a layout change; liquidations and closes equal the unflagged run; the Emergency-flag S3 raced against an honest expiry) |
| `--test extras_it` | T-REPLAY-PDA end to end (a per-mint PDA extra recovered with a template; a hostile `current` bypassed) |
| `--test attacks_settle_it` | ECON §12.10–§12.11 on an `RC`-like pool: `arming.py`, `refute_push.py`, `fv_push.py` (incl. the S2 → S1 flip and its limit) |
| `--test invariants_m2_hooks` | the M2 hooks fire on deliberately broken states (I2, I3, A26, I5–I7, I9–I13, I16) |
| `--test ray_math_it` | M3, T-RAY-MATH (execution half): direct swaps on the dumped AMM v4 / CPMM ELFs (both directions, exact in and out, every fixture pool, all 21 CPMM configs × creator-fee sides, fee and `need_take_pnl` changes, seeded sequences: `RAY_MATH_SEQ`, default 40, and `RAY_MATH_SEED`) equal `quote::ray_v4` / `quote::cpmm` on the readers' reserves, the exact bound passing and one unit tighter failing; negative controls; fixture integrity and venue PDAs; both readers' route predicates against the ELFs; the byte-level admin builders against the real admin instructions |
| `--test ray_it` | M3, the Raydium adapters on the fork: listing (exact state, O13, every verification / eligibility refusal, D1 precedence on Fartcoin's real curve), open → close to the lamport on both programs (SPL, Token-2022, WSOL on either side, every fee variant incl. U11), the Raydium open refusals, T-LATCH (byte-patched slots), T-RAY-PDA, T-RAY-WSOL, T-RAY-DISABLE, T-RAY-FEE, liquidation and expiry, I14, the hooks' Raydium view, T-RAY-TRACE / CU and T-SIZE (`-- --nocapture` prints them) |
| `--test raydium_v4_e2e`, `--test raydium_v4_attacks` | M3 fork E2E on Fartcoin (AMM v4) and ECON §12 on the real pool, every short.win instruction against the differential oracle of `raydium_v4_support.rs`: lifecycle (listing window, Tier C → A, close, two liquidations after a real pump, expiry, the lender's exit), S3 after a real `SetParams` halt, S2 via LP removal (unreachable at the defaults; reached under an in-bound governance corner), S1 on a real gap; exit sandwiches, atomic and held pushes, `fv_push` and the flip bundle (`-- --nocapture` prints the figures) |
| `--test raydium_cpmm_e2e` | M3 fork E2E on CPMM: the USELESS, Token-2022 and creator-fee journeys (list → deposit → open → real price moves → close / liquidation / expiry → S3 on an admin halt and on a fee hike above the hard cap → S2 on an out-of-model drained pool → lender exit), the settlement matrix on every fixture pool, U11 through short.win's exits, O13 ages |
| `--test latch_raydium_it` | M3, T-LATCH on the dumped ProgramData: real loader upgrades (the Squads vault's signature forged), opens refused on every Raydium market while exits keep working, revalidation only at the loader's slot, a second upgrade during review, both Raydium bits, `[flag, open]` vs `[open, flag]`, U5 (the same CU on 1.4 MB, 0.79 MB and 10 MiB) |
| `--test trace_raydium_it` | M3, T-RAY-TRACE: the exact call shape (program and stack height of every CPI) and trace entries of every Raydium-path instruction on AMM v4, CPMM and Token-2022 CPMM; peak height 3; the worst-case transactions ≤ 64 and the runtime's refusal of one open more |
| `--test ray_sec_it` | M3 security review regressions on the fork: M3-SEC-1 (a CPMM config trade fee of 0 is no route — S3 chain, healthy positions frozen, never an uncapped S1 — and a fee-free risk view; the fee-split predicate against the ELF), M3-SEC-2 (Raydium listings refused while the Raydium latch is set), M3-SEC-3 as inverted by M4 (`m3_sec_3_propose_pool_unpins_the_first_listing`: a list-only front-run of a Raydium token moved onto its main pool by `propose_pool`; a thin eligible listing replaced by the main pool inside `pool_list_window_s`, the window not extended) |
| `--test pool_select_it` | M4, Raydium pool selection on the fork (release build, dumped programs, **planted** candidate pools — `pool_select::plant_v4` / `plant_cpmm`, a dumped pool's bytes on fresh keys that the dumped ELFs trade): a list-only market's adoption (exact state, O13 / O13b ages, D1 on real and planted curves, base/quote order); the listing window's 5 % margin, never extended; zero-position adoption at 25 % and `PositionsOpen`; AMM v4 ↔ CPMM switches with trades on the new pool; the fast path with open positions (the `pool_fail` chain — halt, fee above the hard cap, never the Emergency flag —, 120 s, the 60 s window keeping the deepest candidate, locked terms and EMA continuity, the ring rewrite and a ≤ 1 % box from the first second, exits on the new pool), cancellations, the latch; every refusal; T-RAY-SWITCH's attacks; #99's switch only if all fit (`carried_fit`: per position and per market on both programs, cancellation at the execution, the dump guard at `P_ema`, a withdrawal-wrapped execution, the mark) and #100's FIT regressions (`carried_fit::fit1_*`: FIT-1's residual — a dump-wrapped proposal carries at most the execution's cap, ≤ `λ·√(P_cand/P_ema)` × a native open's cap, knobs `FIT_R_BPS`, `FIT_Q_PERMILLE`; `carried_fit::fit3_*`: FIT-3's residual — a closed max-size position keeps a fitting candidate refused while positions are open, and they settle S3 at their deadlines with the lenders at FV; the I15 hook `i15_the_carried_fee_and_the_marks`: the fee mark `min_open_fee_ppm` moves only at an open and the last exit); the I15 hook's mutants; T-TRACE / CU (`-- --nocapture` prints them) |
| `--test pool_attacks_it` | M4 pool-capture attacks on the fork (ARCH §2.6's attack table; the attacker's liquidity moves by the dumped CPMM ELF's real `deposit` / `withdraw`): unlocked-LP capture and pull (`D` unchanged), λ burn / locker / "unburn" games, hysteresis and the wash-trading cost of a flip, fast-path front-running, R4B-10 griefing and the pause remedy, mirrored and look-alike quotes, the latch (per candidate program, M4-SEC-8), `venue_switch.py`'s RA / RB / RC rows, no forced exit through a switch; the implementer's two tripwires are **inverted** by the M4 security review (`lambda_games::lp_dilution_no_longer_cancels_the_fast_path`, `…::a_round_trip_under_the_sol_floor_no_longer_cancels_the_fast_path`: an atomic `[deposit LP / sell, execute_pool_switch, withdraw / buy back]` no longer cancels the fast-path window, eligibility being the locked depth at `P_ema`) (`-- --nocapture` prints the figures) |
| `--test m4_sec_it` | the M4 security review's findings as regressions on the fork (each fails on the pre-fix build): M4-SEC-1 (a persistent unlocked deposit no longer blocks the fast path; the halted market adopts and the underwater position is liquidated with and without it), M4-SEC-2 (a window whose arming chain broke — flapping or stale — cancels; the new failure re-arms after 120 s), M4-SEC-3 (eligibility unmovable inside a transaction on a market with a price; the bounded residual on a list-only market), M4-SEC-4 (a list-only market with positions collects the deepest pool; a pump venue cancels the window), M4-SEC-5 (restated by M4R: the adoption writes the anchor `⌊D·10⁴/λ_min⌋` and reads no sample, so a deposit or an unlocked LP's withdrawal around the execution — or around the proposal and the execution — changes nothing; after the warm-up `L_ema` is the pool's depth), M4-SEC-6 (a deeper or thinner candidate: the box is ≤ 1 % wide in `L` from the gate on), M4-SEC-8 (the latch holds only the candidate's program). M4-SEC-7 is the keeper scenario `junk_pools_cost_a_bounded_ranking_after_the_cranks` (`-- --nocapture` prints the figures) |
| `--test m4v_it` | the M4 verification's findings as regressions on the fork: M4V-1 (a collection window's candidate needs its locked SOL alone at `P_ema` above `pool_min_real_sol`: 20- and 13-SOL pools refused `PoolTooThin`), M4V-2 (restated by M4R: deposits around the proposal, the execution or both change nothing; the underwater position is liquidated at adoption + 9 s; a deposit-wrapped crank every second does not block a big position's liquidation — the reference guard), M4V-3 (a `λ < λ_min` candidate: `L_ema = √(X·Y)` ±1 % at the gate, the box ≤ 1 % after it), M4V-4 (a latched window candidate cancels, the chain stays armed), M4V-5 (a captured list-only market opens nothing until `ema_warm = 0`, then `R_eff` refuses the pushed open; since M6-SEC-1 the seed is ≈ 2.73× the pool at the gate and on it a minute later, the opens still refused) |
| `--test m4r_it` | M4R, the post-change confirmation (PROPOSED, O18), on the fork: M4R-1 (a withdraw-wrapped switch and per-second withdraw-wrapped cranks and liquidations force nothing; every liquidation before the gate fails `EmaWarmingUp`; the genuinely liquidatable control is liquidated at +9 / +10 s; the degraded mode with an attacker-only warm-up), M4R-2 (a dump during the halt keeps a thin pool out; the pump-wrapped proposal residual), ★T-SWITCH-CONFIRM (a genuine 2.2× pump liquidated at adoption + 9 s; a keeper back after 600 s liquidates 8–9 s later); the M4R verification's M4X-1 (attacker-only price bundles through the warm-up: the seed rises by at most the largest v0.4.1 step, checked per integration; a position with a ≈ 3× threshold is no longer liquidated at a flat 1.00×), M4X-2 (a deposit freeze held for 900 s: `L_slow` stays at the honest lows while the guard is set, the genuine pump ends in a liquidation, not S1; the I18 mutant) and M4X-3 (a dump inside the collection window is adopted at `P_ema`; the warm-up then lowers the seed by at most one v0.4.1 step per integration since M6-SEC-1 — 1.73× the pool at the gate, on it a minute later —, an open at the gate is refused `PoolListWindow`, after the adopted pool's listing window `PoolTooThin`; the disclosed residual); the final check's M4F-1, restated by #99 (`m4f1_a_deposit_freeze_on_a_fitting_carried_position_only_delays_the_liquidation`: the position sized to fit the candidate, `q/x` 0.71 %, the keeper's coverage rules modelled second by second, a deposit-wrapped crank every second from the adoption or from +40 s under the shipped keeper and under the finding's cadence — every run LIQUIDATED at +168 s at 2.247×, like the control; `m4f1_the_depth_cap_bounds_the_frozen_reference_at_lambda_over_lambda_min`: under the finding's cadence `L_b` held at the Raydium cap `⌊D·10⁴/λ_min⌋`, 1.995× / 1.000× the depth on λ 99.78 / 50 % candidates, both liquidated; `m4f1_an_oversized_carried_position_is_not_adopted_and_settles_in_s3`: the finding's size is refused, S3 at the deadline with lenders at FV); ARCH §8.2; `m4r_explore_liquidation_grid` is ignored (knobs `M4R_GRID`, `M4R_SECS`, `M4R_SOL`) (`-- --nocapture` prints the figures) |
| `--test switch_confirm_it` | M4R's gate and guard: `liquidate` / `expire` fail `EmaWarmingUp` and `sol_settle` evaluates nothing during the warm-up on an executable venue, and only there (a new pool that halts in the warm-up reaches S3); the guard clears at the gate on an honest pool; a market without a switch is never marked; the ignored `lockstep_*` differentials send every instruction of a 90-s scenario to the pre-M4R build too and compare the accounts byte for byte (they run when `SHORTWIN_PRE_M4R_SO` / `SHORTWIN_PRE_M4R_TEST_VENUE_SO` name the pre-M4R release and test-venue builds; `-- --ignored lockstep`) |
| `--test tx_size_raydium` | M3, T-SIZE: every Raydium transaction in its worst case (both ComputeBudget instructions, a separate fee payer, a first-time borrower, the largest extras set) fits 1,232 B and 64 accounts as v0 through the protocol ALT (A28 as enumerated, and the table `scripts/build-alt.sh` builds) and executes that way; crank batches (4 per v0 transaction); the keeper's composite and a Jito tip; M4's `propose_pool` / `execute_pool_switch` in their largest shapes (CPMM incumbent + CPMM candidate, a list-only market's detection set), executed through the table (`-- --nocapture` prints the table) |
| `--test pump_math_it` | M5, T-PUMP-MATH (execution half) on the pump fork: a normal user's `buy_v2` / `sell_v2` / `buy_exact_quote_in_v2` on every live SOL curve of the dump (125-, 151-, 115- and 49-byte layouts, SPL Token and Token-2022) over grids and seeded sequences equal `fees::pump_curve` and `quote::sol_leg` on the readers' state to the lamport — every account delta and the `TradeEvent` —, the exact bound passing and one unit tighter failing; the live and synthetic fee tiers, the creator override, a default creator; 6021 above `rT`, the completing buy, 6005 after completion; the undocumented cap byte; the creator-vault top-up; recipient refusals; the tripwire on the live-supply fee tier (R-PUMP Update U10) |
| `--test pump_it` | M5, the pump-curve adapter on the fork (release build, dumped pump / pump-fees / PumpSwap programs and coins): `listing` (`create_market(Auto)`: curve cache, trader base ATA and pump UVA, the list-only outcomes, every refusal), `flows` (open → close to the lamport on every curve layout), `refusals`, `structure` (I14), `rent` (T-RENT: the creator-vault top-up on sales and buys), `pda` (T-PDA-USER: the trader PDA as pump's `user` under CPI), `straddle` (`CurveStraddle`; `B̂_str` against the executed completion + `migrate_v2` + PumpSwap `buy`), `graduation` (`→ PumpMigrating`, the migration pause and S3 at 70 min, a real `migrate_v2`, `→ None` and S3 on the detection set, `None → PumpCurve` re-detection; M4's carry-overs: a graduated-elsewhere market with a position adopts its Raydium pool through the collection window, and the re-detection clears a running window; since M6 a migrated market moves `→ PumpSwap` and a graduated coin lists on its pool; the M6 security review's `m6_sec_5_*` — a full wipe during the migration pause waits for the curve cashback, without cashback `sweep_trader` alone unlocks the market — and M6-SEC-3 as the owner decided it on 2026-09-30 (ARCH §3.4, SPEC §14): `m6_sec_3_curve_opens_are_refused_exactly_while_create_pool_is_halted` — a curve open fails `GraduationHalted` (6104) exactly while PumpSwap's `disable_flags` bit 0 is set (0x01 / 0x03 / 0x09 / 0x19 / 0xff refused, 0xfe opens, clearing the bit re-enables opens, a close runs under the halt) — and `m6_sec_3_a_position_opened_before_the_create_pool_halt_stalls_until_s3` — a position opened before the halt keeps the stalled-graduation path: `migrate_v2` fails 6017 (`DisabledCreatePool`), the healthy position frozen past the 70-min pause until its deadline, then S3), `exits` (expiry, liquidation, `sweep_trader`'s token leg, the pump latch bits) |
| `--test straddle_it` | M5, T-STRADDLE (curve part) with the `complete_curve` helper (`COMPLETE_CURVE_SO`): tx1 completes at the `rT` it reads, is a no-op on a completed or migrated curve; ★ a buy, a sale, a third-party completion or migration between the keeper's read and the bundle; v0.3's stale tx1 reverting the bundle; `max_sol_cost`; a straddled short.win position (`CurveStraddle`, `B̂_str` stable); every live SOL curve; the helper's own checks; trace / CU / size per transaction (`-- --nocapture`). **M6:** tx3 as built — the keeper's (the transition plus the pool's first bucket; its exit at T + 8, T + 9 without that bucket; a tx3 carrying the exit reverts the bundle `EmaWarmingUp`), the three-transaction bundle against a curve trade before it, the borrower's own tx3 with `close_short` at `B̂_str`, a borrower's self-made straddle (`rT − q + 1`) |
| `--test pump_curve_scenarios`, `--test pump_curve_attacks`, `--test pump_curve_e2e` | M5 fork E2E on the curve (shared `pump_curve_support.rs`, no tests of its own; ≈ 2.5 / 1.5 / 1.3 min debug): ECON §11.2 and §10.1 curve rows replayed second by second against `shortwin_sim` to the lamport (D@50 % / D@80 % / D@90 %, stuck migrations with S3 at +4,200 s; D@0 % refused, `ProceedsBelowMin`); ECON §12 on the curve (expiry and liquidation sandwiches, atomic and held pushes, the OI-cap basket); the lifecycle on a live curve (listing, lenders, opens, a close, liquidations, an expiry, the lenders' exit), completion with positions open (`→ PumpMigrating` → `None`, S3), a curve migrated elsewhere between two reads, a real `migrate_v2` moving the market onto the pool (M6), and a fast `migrate_v2` priced on the pool (M6; the M5-only EMA freeze is gone); §10.1's graduation rows are `graduation_scenarios`' (`-- --nocapture` prints the figures) |
| `--test trace_pump_it` | M5, T-TRACE on the curve: the exact call shape (program and stack height of every CPI) and entries of every curve-path instruction (listing, cranks incl. the venue transitions, opens, closes incl. a completing buyback, liquidate, expire, `sol_settle` on `PumpMigrating` and `None`, `sweep_trader`, the latch), peak height 3; the worst-case transactions ≤ 64 and the runtime's refusal of one open more |
| `--test tx_size_pump_it` | M5, T-SIZE on the curve: every curve transaction in its worst case (the largest extras set, a separate fee payer, a first-time borrower) as v0 through both tables (A28 with its pump part, and `scripts/build-alt.sh`'s), executed that way; crank batches (5 per v0 transaction); the keeper's composite; a tipped liquidation with the largest extras set measured as **not** fitting v0 (`-- --nocapture` prints the table) |
| `--test latch_pump_it` | M5, T-LATCH on the dumped pump / pump-fees / PumpSwap ProgramData: real loader upgrades, opens refused on curve markets while cranks, top-ups, `sol_settle`, `sweep_trader`, closes, liquidations and expiries keep working; revalidation only at the loader's slot; all three bits must clear; U5 on the 10 MiB accounts |
| `--test m5_sec_curve_it` | the M5 security review's findings on the curve as regressions (release build, pump fork; shares `pump_curve_support.rs`; the audit's reproducers, converted): M5-SEC-1 (a full wipe on a live curve reopens once `sweep_trader` has set the venue-reward bit, the trader's pump UVA holding no claimable cashback; a cashback coin's `[pump claim_cashback(user = trader), sweep_trader]` credits the wiped epoch — T-CASHBACK's pump part), M5-SEC-3 (a sale wrapped around the open no longer lifts the Tier D guard or `X₀`: the curve caps read the reserves at `P_ema`), M5-SEC-4 **inverted by M6** (`m5_sec_4_m6_can_enable_the_curve_venue`; `m5_sec_4_an_atomic_complete_and_migrate_graduates_the_market_onto_its_pool`: the atomic `[buy_v2(rT), migrate_v2]` with the pool pushed ≥ 3.5× the last curve EMA moves the market onto the pool, which liquidates only after the 8-integration warm-up), M5-SEC-5 (an unreadable canonical pool keeps the market `PumpMigrating` and fails Raydium adoptions closed; an empty address is graduated elsewhere), M5-SEC-6 (a low-bump market costs short.win no extra CU) |
| `--test pamm_it` | M6, the PumpSwap adapter on the fork (release build, the dumped pAMM / pump / pump-fees programs and the `pamm_state` pools): `listing` (a PumpSwap-class coin lists on its canonical pool with the pAMM init set — Token-2022 301 B, SPL 261 B, 211 B after `extend_account` —; the first observation starts the price marked for the warm-up; the init set key-checked; `None → PumpSwap` re-detection), `graduation` (completion → `PumpMigrating` → a real `migrate_v2` → `PumpSwap`: the cache, `graduated_at`, the ring at `lp_supply`, `(8, 1)`, exits and opens refused `EmaWarmingUp` until 8 integrations, the borrower's close on the pool), `flows` (open → close to the lamport on every pool role, expiry and liquidation after a ×2.8 pump, `sol_settle` no-op and S1 after a held ×10), `refusals` (the warm-up, every `disable_flags` bit, the boost gate, the pAMM / pump-fees latch, the existence requirement, every trade slot substituted), `gates` (T-DISABLE on the ELF, T-ATA / T-PDA-USER, T-RENT, T-CASHBACK pAMM part), `negvqr` (the synthetic pool prices on `vault + VQR`; the tier inputs), `creator` (M6-SEC-4: a stranger's `set_coin_creator` on the dumped ELF costs an exit one retry — `VenueAccountMismatch` until the new creator vault's WSOL ATA exists — and `crank_price` re-caches the creator), `lock` (★T-PAMM-LOCK, the owner's M6R-1 decision of 2026-09-30 and OWNER-M6D-1, ARCH rows 56–57, ECON #105–#106; ARCH §8.2, `attacks/pumpswap_lp.py` parts 2–3; 12 tests of `pamm_it`'s 41: U33 — `pool_d` is the burned LP's depth `D` on every pool role —, the LP mint P6 key-checked, a deposit and its withdrawal moving neither `D` nor the bounds, a deposit wrapped around the open inflating nothing (3 (c)), `part_3b_a_held_deposit_lifts_no_open_the_burned_lp_sizes_it` — a 3× deposit held 1 h: the next open's cap 1.0000× the honest one at `pool_lock_min_bps` 5,000 and 2,000 (2.000× / 2.842× as built at ARCH row 56) —, a held deposit pulled at a pump liquidating a position sized before it with the control (3 (a), +88 s, `L_b` the drained pool's depth), `[migrate_v2, deposit, crank_price]` capped at `D` by the first PumpSwap observation (part 2), FITS-2 (a) and FITX-1 on a pool with third-party LP (3 (d)); **M6D-1's pin, as decided** `part_3e_a_held_deposit_sizes_nothing_and_the_pulled_pump_is_liquidated` — the held cap 1.0000×, the position at 95 % of it liquidated at +59 s when the deposit is pulled at a genuine 5×/120-s pump (the control +61 s), the lenders `Q` back and 0.0632 SOL, +4.20 % of `Q`'s value, the same second at `pool_lock_min_bps` 2,000 (row 56: S1 at +124 s, −37.9 %); **M6E-1's pin** `part_3f_a_deposit_pulled_at_the_deadline_does_not_hold_up_the_expiry` — a 3× / 2× deposit held for the last ≈ 62 min before a position's deadline and pulled at it (3× also 10 min before): expired at +1 s like the control, `L_b = L_live`, while the reference at the cap `2D` would have refused the executed buyback (the `2D` build: +3,347 / +1,910 / +2,536 s); and `capacity_on_the_canonical_pool_fixtures` — the wholly burned pools lose nothing, e/acc's (λ 92.98 %) −0.12 % after the warm-up and −6.13 % after 2 h; ECON §12.12 (12) (e), (f)) |
| `--test graduation_scenarios`, `--test graduation_e2e` | M6 fork E2E through a real graduation (shared `graduation_support.rs`, no tests of its own, which includes `pump_curve_support.rs`; ≈ 19 / 14 s debug): ECON §10.1's twelve graduation rows through completion, `migrate_v2`, the `→ PumpSwap` transition, pump's real `boost_buy_and_burn` on the sim's schedule and PumpSwap ramps, to the lamport against the sim with M6's graduation rule (the module docs list the three differences from ECON's published table; since #105 the normative sim runs M6's graduation rule and the ELF's boost rounding, so only `R_min` at the pre-open 0 % remains — ECON §10.1's as-built column, §11.2); ★T-SEED-FALL (M6-SEC-1: on D@80 % a 0.25× low recorded in post-change second 8 / 4 / 1 delays the liquidation to +14 / +22 / +28 s against the honest +12 s, as the sim; a genuine dump to 0.25× forces nothing on D@80 % or D@97 %; `graduation_e2e::m6_sec_1_*`, the review's reproducer: a 0.1× low in second 1 / 4 / 8 after the atomic attack, the seed at the gate exactly the fall bound, liquidated at +23 / +17 / +12 s against +9 s); a lifecycle through a real graduation, T-VENUE-SWITCH (graduation), the M5-SEC-4 atomic attack and a borrower's self-made straddle, ECON §10.2's borrower bundle, a completing close, the boost gate (`-- --nocapture` prints ECON-format rows) |
| `--test pumpswap_negvqr_e2e` | M6, a lifecycle on the **synthetic** negative-VQR pool (`pamm_synthetic`): opens, a close, liquidations after a ×3 pump, an expiry, S1 after a ×10 gap, the lenders' exit; every quote on `vault + VQR`, the tier input `min(vault, vault + VQR)` |
| `--test graduation_surfpool` | M6, the Surfpool E2E through a real graduation on **live mainnet** (`#[ignore]`d; needs Surfpool 1.6 and network: `scripts/surfpool-e2e.sh`, below) |
| `--test trace_pumpswap_it` | M6, T-TRACE on PumpSwap: the exact call shape and entries of every PumpSwap-path instruction (open 22–23, normalising 26; close 16–17; liquidate / expire 17; crank 2; the `→ PumpSwap` transition 4; the listing 15; `sweep_trader`'s token leg 4), peak height 3 |
| `--test tx_size_pumpswap_it` | M6, T-SIZE on PumpSwap: every transaction in its worst case (the largest extras set, a separate fee payer, a first-time borrower) as v0 through both tables, executed that way; the listing; the transition and tx3; since M6R-1's LP mint (+33 B per PumpSwap ReadSet) 3 cranks per v0 batch (the 4-crank batch pinned over 1,232 B), and the open with the largest (4-entry) extras set and the keeper's `[crank, liquidate]` with it pinned over 1,232 B through A28 — fitting through the build-alt table, the open executed through A28 plus a per-market table (the owner trade-off of ARCH §6.4) (`-- --nocapture` prints the table) |
| `--test latch_pumpswap_it` | M6, T-LATCH on the dumped pAMM / pump-fees ProgramData: a real loader upgrade refuses PumpSwap opens while exits, cranks, `sweep_trader` and `sol_settle` keep working; the PumpSwap mask is pAMM + pump-fees only; U5 |
| `--test replay_0428_it` | M6, T-REPLAY-0428 on the test-venue build (`SHORTWIN_TEST_VENUE_SO`, `MOCK_CPAMM_SO`): pump's 2026-04-28 added-account change, modelled on the mock's `required_extras` knob (the deployed ELFs only accept trailing accounts), recovered through the extras sets (`current`, `previous`, a pinned `stable`, `none`) |

Every instruction goes through `Env::send`, which runs the invariant hooks — I1, I2, I6, I7,
I8, I13 (state part) on every market a transaction names or creates; I3, I4, I5, I7, I9–I13,
I15 (M4: listings, adoptions, windows, Raydium opens on locked depth), I16, I17 (M4: also
adoptions and window candidates while the candidate's own Raydium program is flagged), A26
(snapshot rules) and the lender transaction bounds against a pre-instruction snapshot — after
the transaction and, for a transaction of several instructions, after each instruction
(prefixes replayed on a clone of the VM). I14 is structural (account lists). A violation panics
with the logs. Only a test that plants an impossible state on purpose (the hooks' own mutant
tests, a copied account, an overflowed field) turns them off. Details and conventions:
[`tests/litesvm/README.md`](../tests/litesvm/README.md).

The quick run replays a subset of the `pool_model` traces. All of them (the 40 stranding seeds,
the lock-heavy mix, both ECON §9.6 fuzz runs; about 4 minutes):

```sh
SHORTWIN_TRACES=full cargo test --release -p shortwin-litesvm-tests --test lender_it traces::
```

### The Raydium mainnet fork (M3) and its fixtures

The Raydium binaries run short.win against the **real** Raydium programs, dumped from mainnet
with the pools, vaults, mints, LP mints, observations and configs they trade on
(`tests/fixtures/`; the harness module `tests/litesvm/src/fork.rs`, markets in `src/raydium.rs`).
Each program is deployed from its dumped ProgramData (the real deploy slot and upgrade authority,
for T-LATCH), rebuilt to the mainnet account's sha256; every fixture file is checked against
`manifest.json` when it is loaded. The state accounts are one `getMultipleAccounts` snapshot
(slot 451,187,550, 2026-09-28), so pools, vaults and fee counters agree with each other.

| Fixture | What |
|---|---|
| `programs/raydium_amm_v4.so`, `programs/raydium_cpmm.so` | the deployed ELFs (deploy slots 445,763,204 and 445,763,504) |
| `accounts/raydium/*.bin` | both program accounts and ProgramData headers; AMM v4: Fartcoin `Bzc9NZfM…` and its reversed-order sibling `9GWMrDgH…`, the `AmmConfig` and authority; CPMM: USELESS `Q2sPHPdU…`, the creator-fee pools `FQd8z3…` (WSOL token 0), `3Cv9uPpa…` (WSOL token 1) and `FrZnJAFt…` (both tokens), the Token-2022 pool `7ZRtFcgf…`, each with vaults, mints, LP mint and observation; the authority and all 21 `AmmConfig`s |
| `programs/pump.so`, `programs/pump_fees.so`, `programs/pump_amm.so` (M5, group `pump_programs`, slot 451,672,911) | the deployed pump, pump-fees and PumpSwap ELFs (deploy slots 449,734,335 / 446,465,969 / 446,462,733, `VENUE_GUARD_SLOTS_2026_09_27`) |
| `accounts/pump/*.bin` (M5, group `pump_state`: one `getMultipleAccounts` at slot 451,675,432, 100 entries — the call's cap —, 13 recorded `"absent": true`) | the three program accounts and ProgramData headers; pump's `Global`, both pump-fees `FeeConfig`s, PumpSwap's `GlobalConfig`, both volume accumulators, the fee / buyback / withdraw authorities, WSOL, USDC, PumpSwap's protocol-fee and buyback WSOL ATAs, the Rent sysvar; ten pinned coin **roles**: `fresh` (125 B), `near` (151 B, near completion), `t22_115`, `legacy115` (SPL Token, no creator), `legacy49` (2024 layout), `graduated` (canonical 301-B pool and boost vault), `mayhem`, `usdc`, `legacy150`, `fartcoin` (graduated elsewhere) |
| `accounts/pamm/*.bin` (M6, group `pamm_state`: slot 451,784,607, 76 entries, 18 absent) | PumpSwap's `GlobalConfig`, `FeeConfig`, global volume accumulator, WSOL, Rent, the 16 recipient WSOL ATAs; per pool **role** the pool, mint, vaults, LP mint, coin-creator vault authority and ATA, boost vault and ATA, pool authority and migrated curve: `fresh` (301 B, boost vault 13.98 SOL), `mature` (boost spent, 7 % third-party LP), `cashback`, `legacy211`, `legacy261` |
| `accounts/pamm/pamm_mature_pool_negative_vqr.bin` (M6, group `pamm_synthetic`) | **synthetic**: the `mature` pool with VQR = −⌊vault/2⌋ (no negative-VQR pool exists on mainnet yet), at the same address — load one or the other; `check` verifies it equals its source but for that patch |
| `manifest.json` | per account: address, owner, lamports, executable, rent epoch, space, slot, sha256, purpose, decoded fields; one snapshot record per group |
| `SHA256SUMS` | every fixture file |

`scripts/dump-fixtures.py` (Python 3.14, standard library only) writes and checks them:

```sh
python3 scripts/dump-fixtures.py check           # offline (also the default): every file against the
                                                 #   manifest, the ProgramData rebuilds, cross-references
(cd tests/fixtures && sha256sum -c SHA256SUMS)
python3 scripts/dump-fixtures.py fetch           # fetch only what is missing or invalid (idempotent)
python3 scripts/dump-fixtures.py fetch --refresh # re-dump everything at a new slot
python3 scripts/dump-fixtures.py fetch --rpc URL # default: api.mainnet-beta, then publicnode
python3 scripts/dump-fixtures.py fetch --refresh --group pump_state --coin graduated=MINT
                                                 # re-dump one group (repeatable), a pump coin role
                                                 #   replaced (its expectation is checked at fetch)
python3 scripts/dump-fixtures.py candidates      # live mints that fit each pump coin role
python3 scripts/dump-fixtures.py fetch --group pamm_state --pool negative_vqr=MINT
                                                 # M6: add a PumpSwap pool role (e.g. a real
                                                 #   negative-VQR pool once one exists)
```

A `--refresh` re-pins every byte: the reader unit tests (`crates/shortwin-venue-state/src/
raydium_{v4,cpmm}.rs`, `pump.rs`, `pump_amm.rs`) assert decoded values of this dump (vault
amounts, counters, λ, curve reserves) and must be updated from the manifest's new `decoded`
fields, and figures printed by the E2E and bench tests move with them. The `graduated` role's boost
vault closes on mainnet within the hour after its migration, so a `pump_state` refresh needs a
fresh `--coin graduated=MINT` (from `candidates`); `pump_state` is at the 100-account
`getMultipleAccounts` cap, so a new static needs a second call and a slot-equality rule. Scenario builders (`fork::set_*`, byte-level admin setters,
`set_deploy_slot`) change the fork only in ways the tests name; the real admin instructions run
through `fork::admin_send` / `raydium_admin_send` (the admin's signature forged for one
transaction, the invariant hooks included). `Env::fork_raydium_v4` / `fork_cpmm` start from the
release build; a test-venue `Env` calls `load_raydium_programs`, `load_ray_v4` / `load_cpmm` and
`set_fork_clock` itself (not `set_all_venue_program_data`, which would overwrite the real
ProgramData headers).

**The pump fork (M5).** `Env::fork_pump_coin(PumpCoin::…)` (`tests/litesvm/src/fork.rs`: the pump
groups load lazily, apart from the Raydium fixtures) deploys the three pump programs from their
dumped ProgramData, loads the statics and one coin role, installs mainnet's Rent sysvar (5,080
lamports per byte: pump's venue-side rent is mainnet's, not LiteSVM's default) and the snapshot's
clock. `fork::PumpCurve` gives the readers' state, the live fees, the venue's own charge and
short.win's quotes, and the IDL-exact `buy_v2` / `sell_v2` / `buy_exact_quote_in_v2`;
`tests/litesvm/src/pump.rs` lists markets on it (`Env::pump_market`, `PumpMarket`), trades the
curve as a normal user (`CurveUser`) and builds the graduation path on the real ELFs
(`migrate_v2_ix`, `NewPool`, PumpSwap `extend_account` / `buy`) and the straddle helper's
`complete_curve_ix`. Venue-side scenario builders (`fork::set_curve_reserves`,
`set_curve_complete`, `set_curve_creator`, `set_curve_creator_fee_bps`,
`set_global_creator_fee_configurable`, `set_fee_config`, `set_pump_deploy_slot`) change only
pump's accounts.

## The invariant fuzzer (M7; `tests/fuzz`)

`tests/fuzz` (package `shortwin-fuzz`, a member of the root workspace, ARCHITECTURE §8.4) runs
seeded **action sequences** on LiteSVM through the program-test harness, so every transaction
runs `Env::send`'s I1–I13, I15–I19 and A26 hooks, plus the fuzzer's own oracles (the I10 / I17
gates, the mock's differential exit oracle, I14 omissions, I4's coverage clause on the whole VM,
governance and oracle rules, and the end-of-run liveness wind-down). It loads the same builds as
the program tests. Build it without the deploy profile's fat LTO:

```sh
anchor build --tools-version v1.57 --arch v3 --ignore-keys && scripts/build-test-venue.sh
export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo test -p shortwin-fuzz --release -- --nocapture       # the CI smoke: fixed seeds on every
                                                           # profile, corpus replays, self-tests
cargo run -p shortwin-fuzz --release -- run --profile all --duration 10m --jobs 8 --out fuzz-out --minimize
cargo run -p shortwin-fuzz --release -- one --profile curve --seed 42 --actions 400   # one run, full summary
cargo run -p shortwin-fuzz --release -- replay fuzz-out/crash-<profile>-<seed>-<sig>.json
cargo run -p shortwin-fuzz --release -- tmin   fuzz-out/crash-<profile>-<seed>-<sig>.json  # -> min-….json
```

Profiles: `mock`, `mixed` (test-venue build), `curve`, `pumpswap`, `amm-v4`, `cpmm` (release
build, mainnet forks), or `all`. Each job owns one seed stream; the action list is a pure function
of `(profile, seed)`, so `replay` and `tmin` are deterministic. `fuzz-out/` (gitignored) receives
crash files (profile, seed, action list, signature, logs) and `summary.json` (counts and the
coverage proxies: instructions landed, events, error codes returned). A minimised reproducer of a
finding goes into `tests/fuzz/corpus/`; mark it `"status": "fixed"` once fixed and `corpus_replays`
keeps it as a regression. **The M7 exit criterion "fuzz clean ≥ 24 h"** (ARCHITECTURE §9) is a
`run --profile all --duration 24h --jobs 8 --actions 400 --minimize` with `CARGO_TARGET_DIR`
outside the repository (≈ 270–310 actions/s on 5 jobs), or three nights of CI's `fuzz-nightly`
once the branch is on main. Details: [`tests/fuzz/README.md`](../tests/fuzz/README.md).

**Mutation testing** (M7-SEC-6): `scripts/mutants.sh [--fuzz 3m] [--work DIR] [mutant …]` builds the
tests and the fuzzer once from the checkout, then each of the M7 security review's seven one-line
program mutants (the test-venue or the release build of a private copy), and requires the mutant's
named tests and a short fuzz run of its profile to fail against it — a surviving mutant is a missing
oracle. It needs the unmutated builds the fork tests load (`SHORTWIN_SO`, `SHORTWIN_TEST_VENUE_SO`,
`MOCK_CPAMM_SO`, `COMPLETE_CURVE_SO`, default `target/…`); `--work` outside the repository keeps
the per-mutant builds out of `target/`.

## Benches

Six standalone Cargo workspaces with their own lockfiles keep Mollusk 0.15.1 out of the
on-chain lockfile (all hold the Agave runtime line at 4.2.2, C1-14):

```sh
# T-CU-MATH (M0b): per-primitive CU of shortwin-math, budget = ARCH §6.1 + 20 %
cargo-build-sbf --tools-version v1.57 --arch v3 \
  --manifest-path tests/bench/cu_math/program/Cargo.toml --sbf-out-dir tests/bench/cu_math/target/deploy
cargo test --manifest-path tests/bench/cu_math/Cargo.toml --release      # CU_MATH_WRITE=1 rewrites results.md

# M1 CU baseline: every M1 instruction on Mollusk, cross-checked on LiteSVM, within its budget
anchor build --tools-version v1.57 --arch v3
cargo test --manifest-path tests/bench/m1_cu/Cargo.toml -- --nocapture   # M1_CU_WRITE=1 rewrites results.md

# M2 CU bench and T-TRACE: every M2 instruction on the test-venue build against the mock
scripts/build-test-venue.sh
cargo test --manifest-path tests/bench/m2_cu/Cargo.toml -- --nocapture
# M2_CU_WRITE=1 rewrites results.md, M2_TRACE_WRITE=1 rewrites trace.md

# M3 CU bench, T-TRACE and stack depth on Raydium: the release build on the dumped AMM v4 / CPMM
# programs (Mollusk, cross-checked on LiteSVM), the worst-case Raydium transactions, and every
# path through 0–3 levels of the TEST-ONLY CPI wrapper (tests/bench/m3_cu/wrapper, never deployed);
# since M4 also propose_pool / execute_pool_switch (every writing outcome, ARCH §6.1's 100 k)
anchor build --tools-version v1.57 --arch v3
cargo-build-sbf --tools-version v1.57 --arch v3 --manifest-path tests/bench/m3_cu/wrapper/Cargo.toml \
  --sbf-out-dir tests/bench/m3_cu/target/deploy -- --locked       # the wrapper (t_depth only)
cargo test --manifest-path tests/bench/m3_cu/Cargo.toml --locked -- --nocapture
# M3_CU_WRITE=1 rewrites results.md, M3_TRACE_WRITE=1 trace.md, M3_DEPTH_WRITE=1 depth.md;
# SHORTWIN_SO / M3_WRAPPER_SO override the ELFs

# M5 CU bench, T-TRACE, the T-SIZE preview, T-STRADDLE (curve part) and stack depth on the pump
# curve: the release build and the complete_curve helper on the dumped pump / pump-fees / PumpSwap
# programs and coins (Mollusk, cross-checked on LiteSVM), the worst-case curve transactions with
# their v0 + ALT sizes, the straddle's tx1 (incl. a curve trade between the read and the bundle,
# and the no-op) and tx2, every path through 0–3 levels of its own TEST-ONLY CPI wrapper
anchor build --tools-version v1.57 --arch v3
cargo-build-sbf --tools-version v1.57 --arch v3 --manifest-path tests/bench/m5_cu/wrapper/Cargo.toml \
  --sbf-out-dir tests/bench/m5_cu/target/deploy -- --locked       # the wrapper (t_depth)
cargo test --manifest-path tests/bench/m5_cu/Cargo.toml --locked -- --nocapture
# M5_CU_WRITE=1 rewrites results.md, M5_TRACE_WRITE=1 trace.md, M5_DEPTH_WRITE=1 depth.md;
# SHORTWIN_SO / COMPLETE_CURVE_SO / M5_WRAPPER_SO override the ELFs
# (since M6 its graduated coin lists on PumpSwap with the pAMM init set; Fartcoin stays list-only)

# M6 CU bench, T-TRACE, T-SIZE, T-DEPTH and the graduation transition on PumpSwap: the release
# build on the dumped pAMM / pump / pump-fees programs and the pamm_state pools (Mollusk,
# cross-checked on LiteSVM), the worst-case PumpSwap transactions with their v0 + ALT sizes (the
# transition crank, the straddle's tx3, the post-wipe claim bundle), every path through 0–3
# levels of its own TEST-ONLY CPI wrapper
anchor build --tools-version v1.57 --arch v3
cargo-build-sbf --tools-version v1.57 --arch v3 --manifest-path tests/bench/m6_cu/wrapper/Cargo.toml \
  --sbf-out-dir tests/bench/m6_cu/target/deploy -- --locked       # the wrapper (t_depth)
cargo test --manifest-path tests/bench/m6_cu/Cargo.toml --locked -- --nocapture
# M6_CU_WRITE=1 rewrites results.md, M6_TRACE_WRITE=1 trace.md, M6_DEPTH_WRITE=1 depth.md;
# SHORTWIN_SO / M6_WRAPPER_SO override the ELFs
```

The M7 keeper benches are output directories, not workspaces: the real keeper measured by the
services workspace's scenario tests, in release (`#[ignore]`d in the default suite):

```sh
anchor build --tools-version v1.57 --arch v3 --ignore-keys && scripts/build-test-venue.sh
# ECON §5.8's cost-model inputs (CU per instruction and venue, crank batches, cadence, cost per
# market-day, load at 50 / 200 markets) -> tests/bench/m7_keeper/results.{md,json}; the PumpSwap
# 4-extras T-SIZE through the keeper's tables (OWNER-M7-1) -> tests/bench/m7_keeper/tsize_pumpswap.*;
# T-CASCADE under the keeper's CU-per-slot policy -> tests/bench/m7_cascade/results.{md,json}
M7_COST_WRITE=1 M7_CASCADE_WRITE=1 cargo test --manifest-path services/Cargo.toml --release --locked \
  -p shortwin-keeper-litesvm --test scenarios -- --include-ignored --nocapture m7_cost:: pamm_alt:: cascade::
python3 docs/design/sim/keeper_cost.py > docs/design/sim/out/keeper_cost.txt   # ECON §5.8's tables (and
                                                  # the global budget's, #109); asserts its constants
                                                  # equal m7_keeper/results.json
```

`results.json` records the SHA-256 of the two program builds it measured; re-run after a program
change (the load section's wall times and RPC capacity vary run to run). See
[`tests/bench/m7_keeper/README.md`](../tests/bench/m7_keeper/README.md) and
[`tests/bench/m7_cascade/README.md`](../tests/bench/m7_cascade/README.md).

(The `cu_math` bench program and the `m3_cu` / `m5_cu` / `m6_cu` wrappers are separate crates, so their
build commands name their manifests; the no-bare-`cargo build-sbf` rule is about the short.win
program.) The figures are in each bench's `results.md` (and `trace.md`, `depth.md`); commit them
when they change. The M3, M5 and M6 benches use fixed borrower, keeper and attacker keys (M3 also
draws the keys of its planted M4 pools in a fixed order), so their reports are byte-identical
across runs (the M1 and M2 benches use random keys: their PDA bump searches move the figures by
≈ 1.5 k CU per iteration from run to run). See
[`tests/bench/cu_math/README.md`](../tests/bench/cu_math/README.md),
[`tests/bench/m1_cu/README.md`](../tests/bench/m1_cu/README.md),
[`tests/bench/m2_cu/README.md`](../tests/bench/m2_cu/README.md),
[`tests/bench/m3_cu/README.md`](../tests/bench/m3_cu/README.md),
[`tests/bench/m5_cu/README.md`](../tests/bench/m5_cu/README.md) and
[`tests/bench/m6_cu/README.md`](../tests/bench/m6_cu/README.md).

## Services and the keeper (keeper v1, M3; pool selection, M4)

`services/` is a separate **host** workspace with its own `Cargo.lock` (ARCHITECTURE §1.2,
§7.1): `common` (the chain client: JSON-RPC backend, v1 / v0 transaction building),
`keeper` (keeper v1: duties 1–4, batching packed by size, degraded mode, priority fees,
M4F-1's post-switch coverage — per second while a switch's reference guard is set (its
per-slot bursts on a bucket's liquidity were reverted by the owner's rule of 2026-09-29, ECON #99;
since ECON #101, FITS-2, a foreign bucket liquidity deviation on a market with open positions
starts per-second coverage for `liq_cover_s`, extended while deviations recur) —,
duty 10's venue monitor — ProgramData upgrades flagged with `flag_venue_upgraded`, Raydium pool
admin actions alerted — and, since M4, duty 12's Raydium pool monitor and duty 5's Raydium fast
path — `getProgramAccounts` discovery of every Raydium SOL pool of a mint, the program's own
verifier and `pool_select::decide`, `propose_pool` / `execute_pool_switch`, since #99 only with a
candidate every open position fits (since #100 with the fee-scaled caps at `min(φ_inc, φ_cand)`,
since #101 at most 25 bps)
— on Raydium AMM v4,
CPMM and list-only markets; M4-SEC-7: discovered pools ranked by locked SOL, at most
`rank_keys_per_tick` (200) accounts read a tick and the best `max_candidates` (8) evaluated,
after the tick's exits and crank batch; config `[pool_select]` in `keeper.example.toml`; since M5
duty 6 on the pump curve — `migrate_v2` as soon as a covered curve completes, the straddle bundle
with the `complete_curve` helper through a Jito block engine (`--block-engine`, config
`[graduation]`; since M6 its tx3 is the move onto the pool, `send_without_exit` and `migrate` default
to true, and the keeper's exit lands at the warm-up gate) —, since M6 PumpSwap exits and account
preparation, the move onto the pool after anyone's migration, the post-wipe claim bundle and duty
13's `sweep_trader` (config `[sweep]`), `disable_flags` in duty 10 —,
curve exits with the adapter's own account lists, and list-only markets covered with their
detection set: a curve that graduated elsewhere with positions open is kept on the no-route
cadence and settled in S3, and a list-only market whose curve appears is cranked onto it and its
trader accounts created); the `shortwin-keeper` binary and `shortwin-build-alt`, the protocol
ALT) and the TEST-ONLY
`keeper-litesvm` (an in-process LiteSVM backend that runs the program-test hooks after every
instruction of every keeper transaction, and the scenario tests on the mock and on the Raydium
fork). Its lockfile holds the same Agave 4.2.2 line as the harness it links.

```sh
anchor build --tools-version v1.57 --arch v3 && scripts/build-test-venue.sh
cargo test --manifest-path services/Cargo.toml --workspace          # unit tests + LiteSVM scenarios
cargo clippy --manifest-path services/Cargo.toml --workspace --all-targets -- -D warnings
cargo clippy --manifest-path services/Cargo.toml -p shortwin-keeper --all-targets -- -D warnings  # without test-venue
cargo deny --manifest-path services/Cargo.toml check                # services/deny.toml
cargo audit -f services/Cargo.lock --deny warnings <the ignore list above>

# the RPC backend against a live local validator (ignored by default)
solana-test-validator --reset --quiet &
SHORTWIN_RPC_URL=http://127.0.0.1:8899 \
  cargo test --manifest-path services/Cargo.toml -p shortwin-services-common --test rpc_local -- --ignored

# run the keeper
cargo build --manifest-path services/Cargo.toml -p shortwin-keeper --release
services/target/release/shortwin-keeper --rpc <url> --keypair keeper.json \
  --config services/keeper/keeper.example.toml --metrics 127.0.0.1:9464   # --once, --tick-ms,
                                                  # --discovery-rpc <url> (M4 pool discovery)
```

The scenarios load the same builds as the program tests: the mock scenarios the test-venue
build and the mock (`SHORTWIN_TEST_VENUE_SO`, `MOCK_CPAMM_SO`), the Raydium scenarios
(`raydium*.rs`, and M4's `pool_select.rs` with its planted sibling pools) and M5's pump-curve
scenarios (`graduation.rs`, also the helper: `COMPLETE_CURVE_SO`) and M6's PumpSwap and post-wipe
scenarios (`pamm.rs`, `pamm_lp.rs` — M6R-1: the keeper's per-second liquidity-anomaly coverage holds a PumpSwap withdraw drag at one step, one crank per wrap, and never runs on a market without positions —, `wipe.rs` — incl. `m6_sec_2_*`: a donated pAMM cashback ATA without the trader's pAMM UVA is unlocked by the keeper's `[init_user_volume_accumulator, …, sweep_trader]`) the release build
(`SHORTWIN_SO`) with the committed fork fixtures. The keeper
knows the Raydium venues through the adapters' own account lists and the TEST-ONLY mock venue
only with the feature `test-venue` of its scenario crate; the release keeper binary must not
contain the mock's id (CI checks it). What each duty does and what v1 leaves out:
[`services/README.md`](../services/README.md). **M7:** the O17 degraded mode — the designated
pauser (`[coverage_pause] enabled = true` on exactly one instance, `--coverage-keypair
coverage.json`, a key separate from the fee payer, installed on chain by Governance's timelocked
`Authorities` change) sends `set_coverage_pause` when a market with open positions loses coverage
and clears it 600 s after coverage returns; since the owner's decision of 2026-10-01 the Emergency
multisig can clear a coverage pause too (never set one: stop the pauser first to override its own
pause, ARCHITECTURE §3.1, §7.1 duty 9) —, the keeper budget (`[budget]`; since 2026-10-01 one
global daily budget funding markets in order of lenders' value at risk — `global_daily_lamports`,
`var_floor_lamports`, `reserve_margin_bps`, `tail_share_bps`; `per_market_daily_lamports` is
retired; ARCHITECTURE §7.1 duty 8), duty 11's `crank_sol_usd` (`[oracle]`; `aging_alert_s` 300
with `oracle_stale_s` 600), the T-CASCADE CU-per-slot policy, `shortwin-build-alt market`
(per-market lookup tables) and `shortwin-v1-probe` (U25: does a mainnet leader take v1?
`--rpc <url> --keypair <funded> --send`, two transaction fees). **The degraded-mode runbook and
drill** (install the role, run exactly one pauser, what pages, a lost or compromised key, the
five drill steps on devnet or a local validator): [`services/README.md` "Degraded mode: runbook
and drill"](../services/README.md#degraded-mode-runbook-and-drill-o17); the same steps run
in-process on every CI run (`keeper-litesvm` `coverage_pause`). Do not share one `CARGO_TARGET_DIR` between two
checkouts of the repository: Cargo keys path crates the same way in both and mixes their
artifacts.

### The Surfpool E2E (M6; `scripts/surfpool-e2e.sh`)

`tests/litesvm/tests/graduation_surfpool.rs` runs a real graduation on a **Surfpool 1.6 fork of live
mainnet** (ARCHITECTURE §8.4): only short.win, the harness keys, the protocol ALT and one fixture coin
are planted; pump, pump-fees, PumpSwap and every venue account are mainnet's. It is `#[ignore]`d in
the normal suites (it needs Surfpool and network access; not in CI). The public RPCs refuse Surfpool's
batched `getMultipleAccounts` (HTTP 403 on 2026-09-30), so the script starts a retrying forwarder
first:

```sh
anchor build --tools-version v1.57 --arch v3                 # the release shortwin.so it plants
SURFPOOL=/path/to/surfpool scripts/surfpool-e2e.sh            # forwarder :18999, Surfpool :18899, the test
# or by hand:
python3 scripts/surfpool-rpc-forwarder.py 18999 &           # retries publicnode / api.mainnet-beta
surfpool start --rpc-url http://127.0.0.1:18999 --no-tui --no-studio --no-deploy \
  --port 18899 --ws-port 18900 --airdrop-amount 0
SURFPOOL_RPC=127.0.0.1:18899 cargo test -p shortwin-litesvm-tests --test graduation_surfpool \
  -- --ignored --nocapture                                  # SURFPOOL_TRACE=1: every transaction
```

Tested with the v1.6.0 release binary (`surfpool-linux-x64.tar.gz`, sha256 `cb471b00…`); a run takes
≈ 30 s once built.

### The protocol ALT (A28; `scripts/build-alt.sh`)

One address lookup table holds the protocol's static accounts for every v0 transaction (the
keeper's v0 fallback, the frontend): ARCH §6.4 A28's list, the four `VenueExtras` PDAs, and the
live pump / PumpSwap fee and buyback recipients with their WSOL ATAs (83 addresses on
2026-09-28). The script wraps `shortwin-build-alt` (services workspace; `CARGO_TARGET_DIR` is
honoured):

```sh
scripts/build-alt.sh print                                   # the static part (49 keys), no RPC
scripts/build-alt.sh print  --rpc https://api.mainnet-beta.solana.com   # + the live recipients
scripts/build-alt.sh create --rpc <url> --keypair payer.json [--authority auth.json] [--freeze]
scripts/build-alt.sh extend --rpc <url> --keypair payer.json --table <alt>   # add what is missing
scripts/build-alt.sh check  --rpc <url> --table <alt>        # exit 1 if an address is missing
scripts/build-alt.sh market --rpc <url> --market <market> [--table <alt>] [--keypair payer.json]  # M7
```

Put the table's address into the keeper's `lookup_tables` (`keeper.example.toml`). **This table
(A28 plus the four `VenueExtras` PDAs) is what the M7 measurements recommend as the normative A28**
(OWNER-M7-1, ARCH §6.4): through it the PumpSwap open with the largest extras set and a separate
fee payer fits (1,207–1,208 B), which A28 as enumerated does not. `market` (M7) lists a market's
own table — 21 addresses on PumpSwap, 4,348,480 lamports of rent, recoverable — and creates it
with `--keypair`; it lifts v0 crank batches to 8–10 and is the fallback for the 4-extras open.
`--freeze` is
the owner's call: a frozen table can never be deactivated but can never be extended either (a new
pump recipient then rides inline, 31 B). Re-run `check` whenever pump rotates recipients.
`tests/litesvm/tests/tx_size_raydium.rs` sizes every Raydium transaction through this table, and
the keeper scenario `raydium_alt` builds it on chain with the script's own code.

## Codama client

`clients/js` is generated from the Anchor IDL; the IDL copy and the generated code are
committed. After any change to the program's interface (accounts, args, events, errors):

```sh
anchor build --tools-version v1.57 --arch v3
pnpm install --frozen-lockfile
pnpm --filter @shortwin/client idl:sync     # target/idl/shortwin.json -> clients/js/idl/
pnpm --filter @shortwin/client generate     # -> clients/js/src/generated/
pnpm --filter @shortwin/client check        # tsc + node --test (incl. the T-LAYOUT Codama leg)
```

CI fails if the IDL copy or the generated code is stale. See
[`clients/js/README.md`](../clients/js/README.md).

## Governance config gate (A73)

Every governance proposal passes `scripts/config-gate.sh` before the multisig signs it: the
compiled bounds and relational checks (`shortwin_sim.validate_params`, the same function as
`params::validate`), then `docs/design/sim/config_check.py`'s 1,655-run liveness battery.

```sh
scripts/config-gate.sh kappa_liq_bps=3000 D:rate_base_bps=500     # ECON §15 names; T: = one tier
scripts/config-gate.sh --file proposal.txt --owner-signoff "SPEC §14 …"
python3 scripts/test_config_gate.py                              # the gate's own tests (~1 min)
```

Exit 0 accepted, 1 rejected (a liveness row), 2 rejected (bounds), 3 needs the owner's sign-off
(only rows that leave lenders more than 1 % of Q worse), 64 usage. Parameters the sim does not
model (`params::validate_econ`) are listed, not simulated; the program checks their bounds at
`propose_config`.

## Workspaces

- **Cargo, on-chain** (root `Cargo.toml`, resolver 3, `Cargo.lock` committed):
  `crates/shortwin-math`, `crates/shortwin-math-wasm`, `crates/shortwin-venue-state`,
  `programs/shortwin`, the straddle helper `programs/complete_curve` (M5; crate `complete-curve`,
  also a member of `Anchor.toml [workspace]`; the services workspace links it for its Anchor
  types) and the program tests `tests/litesvm` (which share this lockfile so the
  Agave line LiteSVM resolves is pinned next to the program's, C1-14), and since M7 the
  invariant fuzzer `tests/fuzz` (it links the program tests, so it shares the lockfile too). The release profile
  keeps `overflow-checks = true` (Anchor refuses to build without it), `lto = "fat"` and
  `codegen-units = 1`. The workspace also holds the TEST-ONLY mock venue `tests/mock_cpamm`
  (an Anchor program, also a member of `Anchor.toml [workspace]`; never deployed, A49).
- **Cargo, services** (`services/Cargo.toml`, own `Cargo.lock`): host-only crates — the keeper
  and its chain client today, the indexer later (ARCHITECTURE §1.2, §7).
- **Cargo, benches**: `tests/bench/cu_math`, `tests/bench/m1_cu`, `tests/bench/m2_cu`,
  `tests/bench/m3_cu`, `tests/bench/m5_cu` and `tests/bench/m6_cu` (each of the last three with
  its own TEST-ONLY CPI wrapper program), standalone workspaces with their own lockfiles (Mollusk
  0.15.1). `tests/bench/m7_keeper` and `tests/bench/m7_cascade` (M7) are output only, written by
  the services workspace's scenario tests.
- **`shortwin-math`** (ARCHITECTURE §2.2): `#![no_std]`, zero runtime dependencies,
  `#![deny(clippy::arithmetic_side_effects, clippy::cast_possible_truncation,
  clippy::cast_sign_loss, clippy::unwrap_used)]` (also on the program and
  `shortwin-venue-state`). Every fallible function returns `Result<_, MathError>`, or `Option`
  where the API says a value is undefined. Every division takes an explicit `num::Rounding`,
  chosen per the ECONOMICS §2 rounding rule (in the pool's and lenders' favour). Module docs
  name the owning track and the ECON/ARCH sections. The program calls it for all arithmetic.
- **pnpm**: root `package.json` is private; workspaces per `pnpm-workspace.yaml`
  (`clients/js` today); `pnpm-lock.yaml` committed.

## Program id and keypair

The program id `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` is set in
`programs/shortwin/src/lib.rs` (`declare_id!`) and `Anchor.toml`. Its keypair is **not
committed**: whoever holds it can create the program at that address on any cluster. The
owner keeps it in the gitignored `keys/shortwin-keypair.json`; copy it to
`target/deploy/shortwin-keypair.json` before `anchor deploy`.

Without that file, `anchor build` prints a "Program ID mismatch" warning and writes a
throwaway keypair to `target/deploy/`. The built program is unaffected (the id comes from
`declare_id!`); `--ignore-keys` silences the warning, and CI uses it. Do not run
`anchor keys sync`: it would rewrite `declare_id!` to the throwaway key.

The straddle helper (M5) has its own id, `4B5mGykX9EWwaYdbTvsp8qXCUTbc7caQourz16Gat6LJ`
(`programs/complete_curve/src/lib.rs`, `Anchor.toml`); its keypair is in the gitignored
`programs/complete_curve/keys/complete_curve-keypair.json` (the `*-keypair.json` rule), exists only
in the owner's working tree and must be backed up like short.win's; copy it to
`target/deploy/complete_curve-keypair.json` before `anchor deploy -p complete_curve`. At deploy it
gets the Governance upgrade authority, as the core program (ARCH §8.5).

**`initialize_config` on mainnet (OWNER-E2E-1, M11 deploy runbook).** As a legacy transaction
with a separate fee payer `initialize_config` is 1,294 B, over the 1,232-B limit (the Surfpool
E2E, `graduation_surfpool`). Send it as a **v0 transaction**, or let the upgrade authority pay its
own fee (1,198 B as legacy). If a Squads vault is the authority, check the size budget of its
wrapped transaction first. Nothing in the program changes.

## CI

`.github/workflows/ci.yml`, actions pinned to commit SHAs:

| Job | Runs |
|---|---|
| `fmt` | `cargo fmt --all --check` (workspace and the `cu_math` bench) |
| `clippy` | guard that `shortwin-math` keeps `#![no_std]`, the §2.2 denies and zero runtime deps; clippy `-D warnings` on `shortwin-math`, on the workspace, and on `shortwin` alone (the release feature set, without `test-venue`) |
| `test` | `cargo test -p shortwin-math -p shortwin-math-wasm` with `PROPTEST_CASES=2000` |
| `vectors` | re-runs every `tests/vectors/*/gen_*.py` with Python 3.14 and fails on any diff; T-CONFIG host part: `config_check.py` prints `OK` for the defaults, and `scripts/test_config_gate.py` |
| `deny` | `cargo deny check` (`deny.toml`) and `cargo audit` on the workspace and the six bench lockfiles |
| `wasm` | wasm32 release build of `shortwin-math` and `shortwin-math-wasm` |
| `cu-math` | T-CU-MATH bench (`tests/bench/cu_math`) |
| `anchor-build` | Agave 4.1.2 + Anchor CLI 1.2.0, `anchor build --tools-version v1.57 --arch v3 --ignore-keys` (shortwin, the `complete_curve` helper and the mock) and `scripts/build-test-venue.sh`; asserts SBPF v3 on all four artifacts; A49: the release `shortwin.so` carries neither the test-venue marker nor the mock id (and `complete_curve.so` not the mock id), the test-venue build and `mock_cpamm.so` do; uploads the `.so` files, both IDLs and the TS types |
| `program-tests` | on the `anchor-build` artifact: fixture checksums (`SHA256SUMS` and `scripts/dump-fixtures.py check`), `cargo check -p shortwin --target wasm32-unknown-unknown` (T-LAYOUT), `cargo test -p shortwin` on the release feature set, `cargo test -p shortwin -p shortwin-venue-state -p shortwin-litesvm-tests` (incl. every Raydium and pump fork binary, `straddle_it` with the helper) |
| `codama` | IDL copy and generated client are current; `tsc`; `node --test` (T-LAYOUT Codama leg) |
| `m1-cu` | M1 CU baseline bench (`tests/bench/m1_cu`) within budget |
| `m2-cu` | M2 CU bench and T-TRACE (`tests/bench/m2_cu`) within budget and 64 trace entries |
| `m3-cu` | M3 CU bench, T-TRACE and stack depth on Raydium (`tests/bench/m3_cu`): builds the TEST-ONLY CPI wrapper (SBPF v3 asserted), every Raydium-path instruction (M4's `propose_pool` / `execute_pool_switch` included) within budget, the worst-case transactions at their exact trace counts, every path through 0–3 wrapper levels |
| `m5-cu` | M5 CU bench, T-TRACE, the T-SIZE preview, T-STRADDLE (curve part) and stack depth on the pump curve (`tests/bench/m5_cu`): builds its TEST-ONLY CPI wrapper (SBPF v3 asserted), every curve-path instruction within ARCH §6.1's PROPOSED limits, the worst-case curve transactions at their exact trace counts and v0 + ALT sizes, the straddle's tx1 (the `complete_curve` helper) and tx2 (`migrate_v2`), every path through 0–3 wrapper levels |
| `m6-cu` | M6 CU bench, T-TRACE, T-SIZE, T-DEPTH and the graduation transition on PumpSwap (`tests/bench/m6_cu`): its TEST-ONLY CPI wrapper, every PumpSwap-path instruction within budget, the worst-case transactions at their exact trace counts and v0 + ALT sizes |
| `t-pot` | T-POT: every `pool_model` trace replayed through the program in release (`lender_it traces::`, `SHORTWIN_TRACES=full`) |
| `services` | the services workspace: fmt, clippy (also the keeper without `test-venue`), cargo-deny, cargo-audit, unit tests and the LiteSVM scenarios (mock, Raydium and pump fork, the helper included; M7's degraded-mode drill, duty 11, the PumpSwap tables) on the `anchor-build` artifacts, and the A49 check on the release keeper binary |
| `t-size` | T-SIZE on Raydium (`--test tx_size_raydium`), on the pump curve (`--test tx_size_pump_it`) and on PumpSwap (`--test tx_size_pumpswap_it`), the size tables in the log, and the protocol ALT's static part (`scripts/build-alt.sh print`, uploaded as an artifact) |
| `fuzz-smoke` | M7: `cargo test --release -p shortwin-fuzz` — the fuzzer's fixed-seed smoke on every profile, the corpus replays (a reproducer marked fixed must pass) and its self-tests; crash files uploaded on failure |
| `m7-keeper-bench` | M7: the keeper's cost inputs, the PumpSwap 4-extras T-SIZE and T-CASCADE in release (`m7_cost::`, `pamm_alt::`, `cascade::`; the tests' own gates), the regenerated `tests/bench/m7_*` compared with the committed ones (a warning) |
| `fuzz-nightly` | M7: on the `schedule` trigger (main only) or `workflow_dispatch` with `fuzz_duration`: one 5 h `shortwin-fuzz run` shard per profile, failures minimised and uploaded, the job failing on any crash; every other job skips the `schedule` trigger |
| `mutants` | M7-SEC-6: on the `schedule` trigger (main) or `workflow_dispatch`: `scripts/mutants.sh` — the LiteSVM tests and the fuzzer built once from the checkout (the oracle), then each of the M7 security review's seven one-line program mutants built and run against its named tests and a 3-minute fuzz of its profile; a mutant that survives either fails the job (a missing oracle) |
