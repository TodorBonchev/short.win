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
`target/types/shortwin.ts`.

Verifiable builds (M10) use solana-verify 0.5.2 with the same pins; the `=` form of the
option is required:
`solana-verify build --library-name shortwin --arch v3 --cargo-build-sbf-args="--tools-version v1.57"`.

`anchor build` builds both programs of the Anchor workspace: `shortwin` and the TEST-ONLY mock
venue `tests/mock_cpamm` (`target/deploy/mock_cpamm.so`, below). **Deploy by name**
(`anchor deploy -p shortwin`, or the verified-build path): a bare `anchor deploy` would also
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
cargo test -p shortwin-litesvm-tests                  # every binary below, ~8 min (debug)
cargo test -p shortwin -p shortwin-venue-state        # host unit tests of the program and readers
```

Other builds: `SHORTWIN_SO` (the release program: the M1 binaries and every Raydium binary),
`SHORTWIN_TEST_VENUE_SO` (the test-venue program, M2 binaries), `MOCK_CPAMM_SO` (the mock
venue). `Env::test_venue()` loads the test-venue program and the mock; `Env::new()` the release
program only; `Env::ray_fork()` the release program and the dumped Raydium programs (below). A
stale `target/deploy/shortwin.so` fails the Raydium binaries in confusing ways: rebuild after any
program change.

| Binary | Covers |
|---|---|
| `--test it` | admin and config (T-CONFIG on-chain: every compiled bound, the timelock), pause, T-LATCH (mock ProgramData), T-LAYOUT (runtime offsets), lender constraint cases, the harness, the admin CU table (`cu_baseline -- --nocapture`) |
| `--test market_create` | `create_market` (exact state, creation fee, griefing, detection keys, venue hints) and `flag_unsafe_mint` |
| `--test mint_filter_extensions` | the SPEC §3 allowlist filter: one fixture per Token-2022 extension, program vs byte-level filter |
| `--test lender_it` | the six lender-pool instructions, T-POT, the lender adversarial cases (C0-6, C0-9, C0-13, R3A-2, R3A-10, R3B-1, R3B-2, V2-2, V2-12), `pool_model` trace replays |
| `--test oracle_crank` | `crank_sol_usd` on the committed mainnet `PriceUpdateV2` dump (`tests/fixtures/`, checked by `sha256sum -c SHA256SUMS` there) |
| `--test security` | the M1 security review's findings as regressions and its refutation tests |
| `--test security_m2` | the M2 security review's findings as regressions: M2-SEC-1 (no box collapse on an executable venue), M2-SEC-2 (the `stable` extras set), M2-SEC-3 (canonical template bumps), M2-SEC-4 (`sweep_trader` reopens lending after a full wipe), the M2-SEC-5 tripwire (no `None` path in `sol_settle` until M5; a planted state, hooks off), and A49 (the release build refuses the mock venue) |
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
| `--test ray_sec_it` | M3 security review regressions on the fork: M3-SEC-1 (a CPMM config trade fee of 0 is no route — S3 chain, healthy positions frozen, never an uncapped S1 — and a fee-free risk view; the fee-split predicate against the ELF), M3-SEC-2 (Raydium listings refused while the Raydium latch is set), the M3-SEC-3 tripwire (the first listing pins the venue until M4 `propose_pool`; M4 inverts it) |
| `--test tx_size_raydium` | M3, T-SIZE: every Raydium transaction in its worst case (both ComputeBudget instructions, a separate fee payer, a first-time borrower, the largest extras set) fits 1,232 B and 64 accounts as v0 through the protocol ALT (A28 as enumerated, and the table `scripts/build-alt.sh` builds) and executes that way; crank batches (4 per v0 transaction); the keeper's composite and a Jito tip (`-- --nocapture` prints the table) |

Every instruction goes through `Env::send`, which runs the invariant hooks — I1, I2, I6, I7,
I8, I13 (state part) on every market a transaction names or creates; I3, I4, I5, I7, I9–I13,
I16, I17, A26 (snapshot rules) and the lender transaction bounds against a pre-instruction
snapshot — after the transaction and, for a transaction of several instructions, after each
instruction (prefixes replayed on a clone of the VM). I14 is structural (account lists). A
violation panics with the logs. Only a test that plants an impossible state on purpose (the
hooks' own mutant tests, a copied account, an overflowed field) turns them off. Details and
conventions: [`tests/litesvm/README.md`](../tests/litesvm/README.md).

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
```

A `--refresh` re-pins every byte: the reader unit tests (`crates/shortwin-venue-state/src/
raydium_{v4,cpmm}.rs`) assert decoded values of this dump (vault amounts, counters, λ) and must
be updated from the manifest's new `decoded` fields, and figures printed by the E2E and bench
tests move with them. Scenario builders (`fork::set_*`, byte-level admin setters,
`set_deploy_slot`) change the fork only in ways the tests name; the real admin instructions run
through `fork::admin_send` / `raydium_admin_send` (the admin's signature forged for one
transaction, the invariant hooks included). `Env::fork_raydium_v4` / `fork_cpmm` start from the
release build; a test-venue `Env` calls `load_raydium_programs`, `load_ray_v4` / `load_cpmm` and
`set_fork_clock` itself (not `set_all_venue_program_data`, which would overwrite the real
ProgramData headers).

## Benches

Four standalone Cargo workspaces with their own lockfiles keep Mollusk 0.15.1 out of the
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
# path through 0–3 levels of the TEST-ONLY CPI wrapper (tests/bench/m3_cu/wrapper, never deployed)
anchor build --tools-version v1.57 --arch v3
cargo-build-sbf --tools-version v1.57 --arch v3 --manifest-path tests/bench/m3_cu/wrapper/Cargo.toml \
  --sbf-out-dir tests/bench/m3_cu/target/deploy -- --locked       # the wrapper (t_depth only)
cargo test --manifest-path tests/bench/m3_cu/Cargo.toml --locked -- --nocapture
# M3_CU_WRITE=1 rewrites results.md, M3_TRACE_WRITE=1 trace.md, M3_DEPTH_WRITE=1 depth.md;
# SHORTWIN_SO / M3_WRAPPER_SO override the ELFs
```

(The `cu_math` bench program and the `m3_cu` wrapper are separate crates, so their build
commands name their manifests; the no-bare-`cargo build-sbf` rule is about the short.win
program.) The figures are in each bench's `results.md` (and `trace.md`, `depth.md`); commit them
when they change. The M3 bench uses fixed borrower and attacker keys, so its reports are
byte-identical across runs. See
[`tests/bench/cu_math/README.md`](../tests/bench/cu_math/README.md),
[`tests/bench/m1_cu/README.md`](../tests/bench/m1_cu/README.md),
[`tests/bench/m2_cu/README.md`](../tests/bench/m2_cu/README.md) and
[`tests/bench/m3_cu/README.md`](../tests/bench/m3_cu/README.md).

## Services and the keeper (keeper v1, M3)

`services/` is a separate **host** workspace with its own `Cargo.lock` (ARCHITECTURE §1.2,
§7.1): `common` (the chain client: JSON-RPC backend, v1 / v0 transaction building),
`keeper` (keeper v1: duties 1–4, batching packed by size, degraded mode, priority fees, and
duty 10's venue monitor — ProgramData upgrades flagged with `flag_venue_upgraded`, Raydium pool
admin actions alerted — on Raydium AMM v4 and CPMM markets; the `shortwin-keeper` binary and
`shortwin-build-alt`, the protocol ALT) and the TEST-ONLY `keeper-litesvm` (an in-process
LiteSVM backend that runs the program-test hooks after every instruction of every keeper
transaction, and the scenario tests on the mock and on the Raydium fork). Its lockfile holds the
same Agave 4.2.2 line as the harness it links.

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
  --config services/keeper/keeper.example.toml --metrics 127.0.0.1:9464   # --once, --tick-ms
```

The scenarios load the same builds as the program tests: the mock scenarios the test-venue
build and the mock (`SHORTWIN_TEST_VENUE_SO`, `MOCK_CPAMM_SO`), the Raydium scenarios
(`raydium*.rs`) the release build (`SHORTWIN_SO`) with the committed fork fixtures. The keeper
knows the Raydium venues through the adapters' own account lists and the TEST-ONLY mock venue
only with the feature `test-venue` of its scenario crate; the release keeper binary must not
contain the mock's id (CI checks it). What each duty does and what v1 leaves out:
[`services/README.md`](../services/README.md). Do not share one `CARGO_TARGET_DIR` between two
checkouts of the repository: Cargo keys path crates the same way in both and mixes their
artifacts.

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
```

Put the table's address into the keeper's `lookup_tables` (`keeper.example.toml`). `--freeze` is
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
  `programs/shortwin` and the program tests `tests/litesvm` (which share this lockfile so the
  Agave line LiteSVM resolves is pinned next to the program's, C1-14). The release profile
  keeps `overflow-checks = true` (Anchor refuses to build without it), `lto = "fat"` and
  `codegen-units = 1`. The workspace also holds the TEST-ONLY mock venue `tests/mock_cpamm`
  (an Anchor program, also a member of `Anchor.toml [workspace]`; never deployed, A49).
- **Cargo, services** (`services/Cargo.toml`, own `Cargo.lock`): host-only crates — the keeper
  and its chain client today, the indexer later (ARCHITECTURE §1.2, §7).
- **Cargo, benches**: `tests/bench/cu_math`, `tests/bench/m1_cu`, `tests/bench/m2_cu` and
  `tests/bench/m3_cu` (with its TEST-ONLY CPI wrapper program), standalone workspaces with their
  own lockfiles (Mollusk 0.15.1).
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

## CI

`.github/workflows/ci.yml`, actions pinned to commit SHAs:

| Job | Runs |
|---|---|
| `fmt` | `cargo fmt --all --check` (workspace and the `cu_math` bench) |
| `clippy` | guard that `shortwin-math` keeps `#![no_std]`, the §2.2 denies and zero runtime deps; clippy `-D warnings` on `shortwin-math`, on the workspace, and on `shortwin` alone (the release feature set, without `test-venue`) |
| `test` | `cargo test -p shortwin-math -p shortwin-math-wasm` with `PROPTEST_CASES=2000` |
| `vectors` | re-runs every `tests/vectors/*/gen_*.py` with Python 3.14 and fails on any diff; T-CONFIG host part: `config_check.py` prints `OK` for the defaults, and `scripts/test_config_gate.py` |
| `deny` | `cargo deny check` (`deny.toml`) and `cargo audit` on the workspace and the four bench lockfiles |
| `wasm` | wasm32 release build of `shortwin-math` and `shortwin-math-wasm` |
| `cu-math` | T-CU-MATH bench (`tests/bench/cu_math`) |
| `anchor-build` | Agave 4.1.2 + Anchor CLI 1.2.0, `anchor build --tools-version v1.57 --arch v3 --ignore-keys` (shortwin and the mock) and `scripts/build-test-venue.sh`; asserts SBPF v3 on all three; A49: the release `shortwin.so` carries neither the test-venue marker nor the mock id, the test-venue build and `mock_cpamm.so` do; uploads the `.so` files, IDL and TS types |
| `program-tests` | on the `anchor-build` artifact: fixture checksums (`SHA256SUMS` and `scripts/dump-fixtures.py check`), `cargo check -p shortwin --target wasm32-unknown-unknown` (T-LAYOUT), `cargo test -p shortwin` on the release feature set, `cargo test -p shortwin -p shortwin-venue-state -p shortwin-litesvm-tests` (incl. every Raydium fork binary) |
| `codama` | IDL copy and generated client are current; `tsc`; `node --test` (T-LAYOUT Codama leg) |
| `m1-cu` | M1 CU baseline bench (`tests/bench/m1_cu`) within budget |
| `m2-cu` | M2 CU bench and T-TRACE (`tests/bench/m2_cu`) within budget and 64 trace entries |
| `m3-cu` | M3 CU bench, T-TRACE and stack depth on Raydium (`tests/bench/m3_cu`): builds the TEST-ONLY CPI wrapper (SBPF v3 asserted), every Raydium-path instruction within budget, the worst-case transactions at their exact trace counts, every path through 0–3 wrapper levels |
| `services` | the services workspace: fmt, clippy (also the keeper without `test-venue`), cargo-deny, cargo-audit, unit tests and the LiteSVM scenarios (mock and Raydium fork) on the `anchor-build` artifacts, and the A49 check on the release keeper binary |
| `t-size` | T-SIZE on Raydium (`--test tx_size_raydium`, the size table in the log) and the protocol ALT's static part (`scripts/build-alt.sh print`, uploaded as an artifact) |
