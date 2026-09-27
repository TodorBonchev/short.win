# shortwin-litesvm-tests

Program integration tests on LiteSVM 0.16 (ARCHITECTURE §8.2, §8.3).

```sh
anchor build --tools-version v1.57 --arch v3        # target/deploy/shortwin.so (required)
cargo test -p shortwin-litesvm-tests                 # or: SHORTWIN_SO=/path/to/shortwin.so cargo test …
cargo test -p shortwin-litesvm-tests --test it cu_baseline -- --nocapture   # admin CU table
SHORTWIN_TRACES=full cargo test --release -p shortwin-litesvm-tests --test lender_it traces::   # T-POT, ~4 min (CI job `t-pot`)
```

## Layout

| Path | What |
|---|---|
| `src/env.rs` | `Env`: LiteSVM + `shortwin.so`, admin keys (upgrade authority recorded in the ProgramData, Governance, Emergency, fee recipient), clock control (`warp`, `set_clock`), `send` with the invariant hooks |
| `src/ix.rs` | instruction builders for every M1 instruction |
| `src/invariants.rs` | `check_all(&svm, market)` (I1, I2, I8) and `snapshot` + `check_ix` (I4, I17, per instruction) |
| `src/invariants/lender.rs` | the lender pool's part of I1 (both directions) and I8 (claims, entitlement, allowances with each lender touched alone, locks, calendar, liveness, `S = 0 ⇒ ΣB_Q = 0`) and the instruction-level lender bounds (allowance per withdrawal, claim ≤ entitlement, token conservation, no tokens without shares, locks never decrease) |
| `src/fixtures.rs` | mock venue ProgramData (T-LATCH), raw zero-copy / Borsh / token-account writers, `install_market` |
| `src/fixtures/mints.rs` | mint fixtures: one real Token-2022 mint per extension, forged TLV mints, the pump.fun shape |
| `src/tokens.rs` | SPL Token / Token-2022 mints, accounts, holders |
| `src/events.rs`, `src/errors.rs`, `src/read.rs`, `src/pda.rs` | `emit_cpi!` decoding, error assertions, account readers, PDAs |
| `tests/it/` | admin, config (T-CONFIG on-chain), timelock, pause, T-LATCH, T-LAYOUT (runtime offsets), constraint cases, the harness itself, the admin CU table |
| `tests/market_create.rs` | `create_market` (exact state, fee, griefing, detection keys) and `flag_unsafe_mint` |
| `tests/mint_filter_extensions.rs` | the SPEC §3 filter end to end: one fixture per extension, program vs byte-level filter |
| `tests/lender_it/` | the six lender-pool instructions: flows, T-POT, the ARCH §8.2 lender adversarial cases (incl. first-depositor inflation by donation), the hooks' own tests (`hooks.rs`: each check fires on what a program mutant would leave), the lender side of the M1 security review (`security.rs`), `pool_model` trace replays (quick subset by default, `SHORTWIN_TRACES=full` for all) |
| `tests/oracle_crank.rs` | `crank_sol_usd` on the committed mainnet `PriceUpdateV2` dump and mutations of it |
| `tests/security.rs` | the M1 security review's findings as regressions (timelock ceiling, fee-recipient checks and the sweep's rent guard, `Market` PDA checks, memo-required destinations, future-dated oracle updates, the apply window, the oracle-cache reset, the creation-fee bound; M1-SEC-1 pinned pending the owner) and its refutation tests |

## Invariant hooks (ARCH §8.3: I1, I2, I4, I8, I17 after every instruction)

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
(the sweep refusing a broken I2, the planted fake `Market` of `security.rs`, the hooks' own
tests in `lender_it/hooks.rs`).

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
- M2+ completes the `TRACK:` parts of `src/invariants.rs` (positions in I2/I4, opens in I17);
  keep each check on the state the program sees.
- Agave: LiteSVM 0.16.0 resolves the Agave runtime crates to 4.2.2, pinned in this crate's
  `Cargo.toml` and the workspace `Cargo.lock` (C1-14; the same line as `tests/bench/cu_math`).
