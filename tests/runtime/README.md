# Pooled runtime qualification

The harness loads the actual lending program SBF and hash-pinned Jupiter Router V2/CPMM bytes. LiteSVM supplies its bundled SPL/Token-2022 programs, bound to the native runner and Cargo lock. Historical account snapshots, initial allocations, clocks, local test lookup tables and the labelled native-reserve shock are synthetic test inputs.

Lender tests exercise registration, deposit, withdrawal, SOL claims and generation rollover. Position tests open real loans before close, repay, forced exit and default. Any post-income or post-default initial ledger is explicitly labelled synthetic.

## Build and bounded checks

Run from the repository root after building the program SBF:

```sh
python3 scripts/materialize-jupiter-probe.py
cargo test -p shortwin-simple-runtime-tests --tests --locked
cargo run -p shortwin-simple-runtime-tests --example program_fuzz --locked -- \
  --cases 32 --seed 103410000 --out target/program-fuzz-smoke
```

Every output directory must be new. The runner freezes its program ELF, native executable, Rust sources, license, lockfile and fixture inputs, with hashes in `metadata.json`. Each seeded case creates a fresh VM with one actual loan. Eight scenario families cover opening bounds/nonces, close, whole repayment, collateral addition, expiry, exact-grace default, delayed collateral-ratio confirmation, actual-fill veto and actual execution after the labelled price shock.

Every scenario transaction checks token/lamport conservation, principal, exposure, loan count, booked notional, token cash coverage, rent and paid SOL reserves. Expected rejections preserve protected state. Setup transactions are excluded from transaction counters; setup failures are counted separately.

## Duration and replay

Duration mode has no case cap and finishes the case in flight after the deadline:

```sh
cargo run -p shortwin-simple-runtime-tests --example program_fuzz --locked -- \
  --duration-seconds 86400 --seed 103410000 --out target/program-fuzz-24h
python3 services/queue_lifecycle_fuzz.py \
  --duration-seconds 86400 --seed 103410000 --out target/queue-fuzz-24h
```

The financial and SQLite queue targets have separate scopes and results. A mismatch stops the affected target nonzero and retains its exact replay seed. The financial runner writes `crash.json`; the queue target retains `failure.json` and SQLite recovery state. `--cases` and `--duration-seconds` are exclusive. The finite core `model_fuzz` example is a host accounting check, not SBF duration evidence.

To replay a financial failure, use `--replay-seed N` and a fresh output directory. An archived runner can run independently of its original checkout by setting `SHORTWIN_SIMPLE_SO` to its frozen `shortwin_simple.so`, `SHORTWIN_SIMPLE_FIXTURE_ROOT` to `frozen-root`, and `SHORTWIN_SIMPLE_SOURCE_ROOT` to `source`.

A qualifying duration result requires clean process exit, actual requested elapsed duration, readable summary, zero failure/anomaly/setup/invariant counts, all eight financial scenario counters nonzero, and verified frozen inputs plus summary/log/completion hashes. Preserve an independent supervisor's start, end and exit receipts. Exit zero alone is insufficient. Evidence from another source, runner, program or corpus remains bound to those inputs.

These sequential tests do not prove arbitrary multi-loan interleavings, live provider history/capacity, funded operator behavior, wallet integration, external review or universal Jupiter compatibility.

## Captured route matrix

The maintained matrix is `tests/runtime/current-routes.json`; its eight captures are in `tests/fixtures/current-routes`. Six cases expect a complete successful route lifecycle, and two expect exact downstream Scorch refusals.

```sh
python3 scripts/qualify-current-route-suite.py --output target/current-route-suite
```

The suite consumes saved Jupiter V2 build responses and captured account/program bytes. It performs no network reads or transaction sends. The materializer verifies raw HTTP/RPC response hashes, decoded account metadata and the explicit current own-program hash. The runtime replayer checks the materialized hashes independently. It preserves actual mint, DEX, Jupiter, token-program and loader bytes; funding, policy/history, signers and dynamic VM state are labelled synthetic.

A single capture can be materialized and replayed separately:

```sh
python3 scripts/materialize-current-route-candidate.py \
  tests/fixtures/current-routes/bonk-production target/current-candidate \
  --own-program-sha256 REVIEWED_ELF_SHA256
cargo run -p shortwin-simple-runtime-tests --example qualify_current_route --locked -- \
  target/current-candidate/candidate.json target/current-replay
```

The replayer first constructs a bounded v0 transaction using actual captured lookup-table contents. A labelled synthetic ALT fallback can isolate execution compatibility after packet-fit failure; it does not prove production packet feasibility. Receipts include logs, compute use, packet sizes, post-swap fee-bound rollback, sale/full-q buyback, debt/exposure, fees, collateral/rent/refunds, scratch closure, mint metadata and conservation checks.

Captured Pump routes exercise a separately signed native setup allowance, actual actor cost and refund, four setup/funding refusal cases and a subsequent whole-debt buyback with zero setup allowance. `writable_authority.rs` adds unused allowance, read-only prefunding, untracked gifts, invalid authority and signed-boundary tests. SOL conservation sums all external captured account lamports, including initially absent route PDAs; WSOL data deltas are diagnostic and are not counted again on top of backing lamports.

Captured quotes and account reads can span slots, and a quote may correctly fail after the simulated sale changes DEX state. A passing case qualifies only its recorded route/state and program bytes. It does not establish future landing, every quantity, private history or every Jupiter DEX.

## Reviewed deployment — November 3, 2026

The [dated review](../fixtures/jupiter-reviewed-2026-11-03/README.md) qualifies Jupiter deployment slot `462802085` with six fresh successful lifecycles, two expected Scorch refusals, and an actual predecessor-pin initialization refusal on each of two financial-program ELFs. It preserves the exact 24-hour-fuzzed program identity separately from the root rebuild. Fresh quotes, account snapshots, packet feasibility, actual CPI logs, rollback, conservation and cleanup are hash-bound in the retained evidence.

`scripts/qualify-reviewed-route-suite.py` consumes an already built native runner, explicit own-program ELF(s), the dated matrix and retained deployment manifest. It performs no network calls. Its default requires that fresh captures already contain the exact reviewed Jupiter bytes; `--overlay-historical` is a separately labelled compatibility diagnostic. `scripts/capture-jupiter-deployment.py` and `scripts/capture-current-route-matrix.py` are read-only network capture tools and never sign or submit public transactions.

The selected matrix is `tests/runtime/current-routes-2026-11-03.json`. Unfunded or slippage-rejected discovery attempts remain preserved with their actual outcomes and are not promoted as successful cases. The loader pin remains a dated review, not permission to activate the production roles or skip a subsequent upstream-change check.
