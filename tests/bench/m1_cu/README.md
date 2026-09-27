# m1_cu — M1 CU baseline

This is the CU baseline bench named in the M1 exit criteria (ARCHITECTURE §9). It measures the
compute units of every M1 instruction on the SBF program with Mollusk 0.15.1 (ARCH §1.1) and
checks each figure against its budget. For budgets, ARCH §6.1's transaction limit applies where
§6.1 lists the instruction, and a class limit applies where it does not; `results.md` names the
source of each. T-CU starts from this bench and becomes a gate in M3–M7.

The pre-state of each case is built by the LiteSVM harness (`tests/litesvm`, `Env`). That
harness uses the real program from `target/deploy/shortwin.so`, the SPL Token (p-token under the
mainnet feature set), Token-2022 and ATA ELFs that LiteSVM bundles, and a clock under test
control. The bench then:

1. copies every account the instruction names into a fresh Mollusk, loads the same ELFs into
   its program cache, and copies LiteSVM's clock and rent;
2. runs the instruction on Mollusk: the figure the budget applies to;
3. runs the same instruction on LiteSVM through `Env::send`, so the harness's invariant hooks
   also run. This cross-checks the two harnesses.

`src/lib.rs` holds the cases, one or more per M1 instruction (ARCH §3.1–§3.3, §3.9).
`tests/m1_cu.rs` runs them, prints the table and asserts the budgets.

```sh
# from the repository root
anchor build --tools-version v1.57 --arch v3
cargo test --manifest-path tests/bench/m1_cu/Cargo.toml -- --nocapture
# SHORTWIN_SO=<path> overrides the ELF; M1_CU_WRITE=1 rewrites results.md
```

## Status

Every M1 handler is implemented: the M1 merge deleted `ShortwinError::NotImplemented`, and the
bench's stub handling went with it. All 24 cases are measured and within budget (`results.md`,
regenerated at the merge). A case whose Mollusk or LiteSVM run fails, or whose Mollusk figure
exceeds its budget, fails the test.

## Workspace and pins

This is a standalone workspace with its own `Cargo.lock`, like `tests/bench/cu_math`, so
Mollusk does not enter the on-chain workspace's lockfile. The lockfile was seeded from the root
`Cargo.lock`, so anchor 1.2.0, LiteSVM 0.16.0 and the Agave runtime crates resolve to the same
versions as the program tests. The Agave line is held at 4.2.2 (C1-14): Mollusk 0.15.1 declares
`^4.2.0` and does not compile against 4.3.0, and LiteSVM 0.16.0 declares `^4.2.1`. The only
crates added beyond the root lockfile are Mollusk's own (`mollusk-svm*`, `solana-logger`,
`env_logger` and its dependencies), at the versions `tests/bench/cu_math` resolved.

## Findings

- Mollusk and LiteSVM report the same CU for every instruction, to the unit.
- **U5 (CU part):** `flag_venue_upgraded` and `revalidate_venue` consume the same CU with a
  109-byte mock ProgramData and a 10 MiB one (6,026 and 8,434 CU). Account size costs no
  compute units here. The loaded-account-data limits and the fee and latency of a 10 MiB
  account are not measured by this bench.
- Every measured instruction is far below its budget. The largest is `initialize_config` at
  56 k CU of 200 k. `create_market` is about 39–61 k of 250 k, and `crank_sol_usd` is about
  6 k of 60 k.
- Cases that create a PDA for a fresh random key (`create_market`'s market, vault, pool vault
  and trader bumps; a first `deposit`'s `LenderPosition`) vary by ≈ 1.5 k CU per extra bump
  iteration from run to run: the bump search depends on the key.
- M1 review: `Market` PDA re-derivation (`Market::is_pda`, `create_program_address`) added
  ≈ 1.6 k CU to `set_pause(market)`, `claim_sol`, `cover_deficit`, `sync_lender` and
  `sweep_protocol_fees` (M1-SEC-6); `apply_config`'s optional accounts ≈ 0.3 k.
