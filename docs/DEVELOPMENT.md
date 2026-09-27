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
| Python | 3.14, standard library only | test-vector generators |

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

The ignored advisories are the ones in `deny.toml`: all "unmaintained", none a vulnerability;
bincode comes with anchor-lang 1.2.0, the other four only with the LiteSVM test harness.

Test vectors from the normative sims: `python3 crates/shortwin-math/tests/vectors/<track>/gen_<topic>.py`.
The conventions are in [`crates/shortwin-math/tests/vectors/README.md`](../crates/shortwin-math/tests/vectors/README.md).

On a machine with little RAM, `CARGO_BUILD_JOBS=4` keeps the LiteSVM and Mollusk builds within
about 6 GB.

## Program tests (LiteSVM)

The program tests (ARCHITECTURE §8.2, §8.3) are the crate `tests/litesvm`
(`shortwin-litesvm-tests`, LiteSVM 0.16.0 on the Agave 4.2.2 runtime crates pinned in the
workspace `Cargo.lock`, C1-14). They load the SBF build, so build it first:

```sh
anchor build --tools-version v1.57 --arch v3          # target/deploy/shortwin.so
cargo test -p shortwin-litesvm-tests                  # every binary below, ~1–2 min (debug)
SHORTWIN_SO=/path/to/shortwin.so cargo test -p shortwin-litesvm-tests   # another build
cargo test -p shortwin -p shortwin-venue-state        # host unit tests of the program and readers
```

| Binary | Covers |
|---|---|
| `--test it` | admin and config (T-CONFIG on-chain: every compiled bound, the timelock), pause, T-LATCH (mock ProgramData), T-LAYOUT (runtime offsets), lender constraint cases, the harness, the admin CU table (`cu_baseline -- --nocapture`) |
| `--test market_create` | `create_market` (exact state, creation fee, griefing, detection keys) and `flag_unsafe_mint` |
| `--test mint_filter_extensions` | the SPEC §3 allowlist filter: one fixture per Token-2022 extension, program vs byte-level filter |
| `--test lender_it` | the six lender-pool instructions, T-POT, the lender adversarial cases (C0-6, C0-9, C0-13, R3A-2, R3A-10, R3B-1, R3B-2, V2-2, V2-12), `pool_model` trace replays |
| `--test oracle_crank` | `crank_sol_usd` on the committed mainnet `PriceUpdateV2` dump (`tests/fixtures/`, checked by `sha256sum -c SHA256SUMS` there) |

Every instruction goes through `Env::send`, which runs the invariant hooks — I1, I2, I8 on every
market a transaction names or creates, I4 and I17 (and the lender transaction bounds) against a
pre-transaction snapshot — after the transaction and, for a transaction of several
instructions, after each instruction (prefixes replayed on a clone of the VM). A violation
panics with the logs. Details and conventions: [`tests/litesvm/README.md`](../tests/litesvm/README.md).

The quick run replays a subset of the `pool_model` traces. All of them (the 40 stranding seeds,
the lock-heavy mix, both ECON §9.6 fuzz runs; about 4 minutes):

```sh
SHORTWIN_TRACES=full cargo test --release -p shortwin-litesvm-tests --test lender_it traces::
```

## Benches

Two standalone Cargo workspaces with their own lockfiles keep Mollusk 0.15.1 out of the
on-chain lockfile (both hold the Agave runtime line at 4.2.2, C1-14):

```sh
# T-CU-MATH (M0b): per-primitive CU of shortwin-math, budget = ARCH §6.1 + 20 %
cargo-build-sbf --tools-version v1.57 --arch v3 \
  --manifest-path tests/bench/cu_math/program/Cargo.toml --sbf-out-dir tests/bench/cu_math/target/deploy
cargo test --manifest-path tests/bench/cu_math/Cargo.toml --release      # CU_MATH_WRITE=1 rewrites results.md

# M1 CU baseline: every M1 instruction on Mollusk, cross-checked on LiteSVM, within its budget
anchor build --tools-version v1.57 --arch v3
cargo test --manifest-path tests/bench/m1_cu/Cargo.toml -- --nocapture   # M1_CU_WRITE=1 rewrites results.md
```

(The `cu_math` bench program is a separate crate, so its build command names its manifest;
the no-bare-`cargo build-sbf` rule is about the short.win program.) The figures are in each
bench's `results.md`; commit them when they change. See
[`tests/bench/cu_math/README.md`](../tests/bench/cu_math/README.md) and
[`tests/bench/m1_cu/README.md`](../tests/bench/m1_cu/README.md).

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
  `codegen-units = 1`. Host-only crates (keeper, indexer, Agave 4.3 clients) go into a
  separate workspace under `services/` with their own lockfile (ARCHITECTURE §1.2).
- **Cargo, benches**: `tests/bench/cu_math` and `tests/bench/m1_cu`, standalone workspaces
  with their own lockfiles (Mollusk 0.15.1).
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
| `clippy` | guard that `shortwin-math` keeps `#![no_std]`, the §2.2 denies and zero runtime deps; clippy `-D warnings` on `shortwin-math` and on the workspace |
| `test` | `cargo test -p shortwin-math -p shortwin-math-wasm` with `PROPTEST_CASES=2000` |
| `vectors` | re-runs every `tests/vectors/*/gen_*.py` with Python 3.14 and fails on any diff; T-CONFIG host part: `config_check.py` prints `OK` for the defaults, and `scripts/test_config_gate.py` |
| `deny` | `cargo deny check` (`deny.toml`) and `cargo audit` on the workspace and both bench lockfiles |
| `wasm` | wasm32 release build of `shortwin-math` and `shortwin-math-wasm` |
| `cu-math` | T-CU-MATH bench (`tests/bench/cu_math`) |
| `anchor-build` | Agave 4.1.2 + Anchor CLI 1.2.0, `anchor build --tools-version v1.57 --arch v3 --ignore-keys`, asserts SBPF v3, uploads the `.so`, IDL and TS types |
| `program-tests` | on the `anchor-build` artifact: fixture checksums, `cargo check -p shortwin --target wasm32-unknown-unknown` (T-LAYOUT), `cargo test -p shortwin -p shortwin-venue-state -p shortwin-litesvm-tests` |
| `codama` | IDL copy and generated client are current; `tsc`; `node --test` (T-LAYOUT Codama leg) |
| `m1-cu` | M1 CU baseline bench (`tests/bench/m1_cu`) within budget |
