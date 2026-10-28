# Independent pooled runtime qualification tools

This harness loads the **new** program and genuine hash-pinned historical Jupiter Router V2/CPMM bytes. It never imports the previous financial program. LiteSVM's bundled SPL/Token-2022 bytes are tied to the native runner and frozen Cargo lock. Synthetic inputs are explicitly limited to historical account snapshots, initial token-holder allocations, execution clocks, local test ALTs and the stated native-reserve market shock.

The lender tests invoke real registration/deposit/withdraw/claim/generation handlers, including signed minimums, donation isolation and old generation claims. Specific post-income/post-default initial ledgers are labelled synthetic. The position tests open real loans before real close, repay, forced exit and default handlers.

From `simple/`, after rebuilding the new SBF, run:

```sh
cargo test -p shortwin-simple-runtime-tests --tests --target-dir ../target/simple-native
cargo run -p shortwin-simple-runtime-tests --example program_fuzz --target-dir ../target/simple-native -- --cases 32 --seed 103410000 --out target/program-fuzz-new-smoke
```

The runner refuses an existing output directory. It freezes the own-program ELF, native runner, Rust sources/lock (including the local `crates/shortwin-jupiter-route` parser/isolation dependency and license) and every fixture file plus Jupiter candidate, with hashes in `metadata.json`. Each seeded case gets a fresh VM. Eight scenario families exercise valid and invalid opening bounds/nonces, whole close/repay, collateral addition, expiry, exact-grace default, delayed CR confirmation, actual-fill veto and actual CR execution after a labelled synthetic price shock. Every **scenario transaction** checks actual token/lamport conservation, principal, exposure, loan count, booked notional, token cash coverage and rent plus paid SOL reserves. Setup transactions are explicitly excluded from instruction counters; setup failures are counted. Expected rejections preserve protected state.

Case counts are actual counts, not duration or program-wide instruction coverage. The first 32-case smoke is only a short smoke test. A future separately authorized duration campaign uses `--duration-seconds` instead of `--cases`; the two modes are exclusive, and a duration run does not inherit the default 32-case stop. It lets an in-flight case finish after the deadline. **No 24-hour campaign has been started by adding this tool.** Fresh review of this runner, frozen inputs, runtime bounds, invariant scope and final program remains needed before any qualifying campaign. No older fuzz duration applies to this new program.

A mismatch writes `crash.json` with the exact case seed and stops nonzero. `--replay-seed N` runs that one case in a new output directory. To reproduce from an archived run, execute its frozen `runner` with `SHORTWIN_SIMPLE_SO` pointing to its `frozen-root/shortwin_simple.so`, `SHORTWIN_SIMPLE_FIXTURE_ROOT` to its `frozen-root`, and `SHORTWIN_SIMPLE_SOURCE_ROOT` to its `source` directory. These overrides are resolved before any compile-time repository path is accessed, so the archived runner can replay without the original checkout. No live price provenance is inferred.

A readable `summary.json`, zero failures/anomalies/setup/invariant failure counts, the required actual elapsed duration, matching completion/log/summary hashes and verified frozen inputs are all required for a campaign gate. Exit 0 alone is insufficient. Summary/evidence write errors cause a nonzero process exit. The runner is sequential local functional evidence; it does not establish live operator funding, production wallet integration, external review or concurrency coverage.
