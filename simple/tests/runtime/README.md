# Independent pooled runtime qualification tools

This harness loads the **new** program and genuine hash-pinned historical Jupiter Router V2/CPMM bytes. It never imports the previous financial program. LiteSVM's bundled SPL/Token-2022 bytes are tied to the native runner and frozen Cargo lock. Synthetic inputs are explicitly limited to historical account snapshots, initial token-holder allocations, execution clocks, local test ALTs and the stated native-reserve market shock.

The lender tests invoke real registration/deposit/withdraw/claim/generation handlers, including signed minimums, donation isolation and old generation claims. Specific post-income/post-default initial ledgers are labelled synthetic. The position tests open real loans before real close, repay, forced exit and default handlers.

From `simple/`, after rebuilding the new SBF, run:

```sh
cargo test -p shortwin-simple-runtime-tests --tests --target-dir ../target/simple-native
cargo run -p shortwin-simple-runtime-tests --example program_fuzz --target-dir ../target/simple-native -- --cases 32 --seed 103410000 --out target/program-fuzz-new-smoke
```

The runner refuses an existing output directory. It freezes the own-program ELF, native runner, Rust sources/lock (including the workspace-local `simple/crates/jupiter-route` parser/isolation dependency and license) and every fixture file plus Jupiter candidate, with hashes in `metadata.json`. Each seeded case gets a fresh VM. Eight scenario families exercise valid and invalid opening bounds/nonces, whole close/repay, collateral addition, expiry, exact-grace default, delayed CR confirmation, actual-fill veto and actual CR execution after a labelled synthetic price shock. Every **scenario transaction** checks actual token/lamport conservation, principal, exposure, loan count, booked notional, token cash coverage and rent plus paid SOL reserves. Setup transactions are explicitly excluded from instruction counters; setup failures are counted. Expected rejections preserve protected state.

Case counts are actual counts, not duration or program-wide instruction coverage. The first 32-case smoke is only a short smoke test. A future separately authorized duration campaign uses `--duration-seconds` instead of `--cases`; the two modes are exclusive, and a duration run does not inherit the default 32-case stop. It lets an in-flight case finish after the deadline. **No 24-hour campaign has been started by adding this tool.** Fresh review of this runner, frozen inputs, runtime bounds, invariant scope and final program remains needed before any qualifying campaign. No older fuzz duration applies to this new program.

A mismatch writes `crash.json` with the exact case seed and stops nonzero. `--replay-seed N` runs that one case in a new output directory. To reproduce from an archived run, execute its frozen `runner` with `SHORTWIN_SIMPLE_SO` pointing to its `frozen-root/shortwin_simple.so`, `SHORTWIN_SIMPLE_FIXTURE_ROOT` to its `frozen-root`, and `SHORTWIN_SIMPLE_SOURCE_ROOT` to its `source` directory. These overrides are resolved before any compile-time repository path is accessed, so the archived runner can replay without the original checkout. No live price provenance is inferred.

A readable `summary.json`, zero failures/anomalies/setup/invariant failure counts, the required actual elapsed duration, matching completion/log/summary hashes and verified frozen inputs are all required for a campaign gate. Exit 0 alone is insufficient. Summary/evidence write errors cause a nonzero process exit. The runner is sequential local functional evidence; it does not establish live operator funding, production wallet integration, external review or concurrency coverage.

## Current route candidates

The separate opt-in `qualify_current_route` example consumes saved Jupiter V2
build responses and current mainnet account/program snapshots. It never fetches
or sends an RPC transaction, changes on-chain semantics, or replaces historical
fixtures. Materialize a read-only capture, then replay it from `simple/`:

```sh
python3 scripts/materialize-current-route-candidate.py \
  target/current-route-qualification-DATE/ROUTE target/current-candidate-ROUTE \
  --own-program-sha256 REVIEWED_ELF_SHA256
cargo run -p shortwin-simple-runtime-tests --example qualify_current_route \
  --target-dir ../target/simple-native --locked -- \
  target/current-candidate-ROUTE/candidate.json target/current-replay-ROUTE
```

Both tools refuse an existing output directory. The materializer checks raw
HTTP/RPC response hashes and every decoded account hash/metadata field. It
requires the explicit reviewed `--own-program-sha256`; it never borrows a
historical ELF identity. The Rust replayer
independently checks the materialized hashes before loading bytes into LiteSVM.
Actual mint, DEX, Jupiter, token-program and loader account bytes remain intact.
The local lender allocation, payer/borrower SOL, reference history, functional
policy and signers are explicitly synthetic. Dynamic transaction sysvars come
from LiteSVM; their captured observations are retained separately.

The materializer also accepts a checked-in archive directory where large exact
RPC replies are `*.json.gz`. It verifies each decompressed reply against its
recorded raw hash, reconstructs address/value bindings from the recorded
`getMultipleAccounts` requests in timestamp order, and checks the final account
bytes, metadata and context slots against `accounts.manifest.json`. The derived
`accounts.snapshot.json` is therefore unnecessary in a clean clone. Compression
and storage hashes in the archive index remain an additional independent check.

The replayer first attempts a bounded v0 transaction using only real captured
lookup-table account contents. A failure to fit the 1232-byte packet is recorded
and may fall back to a clearly labelled synthetic test ALT to isolate execution
compatibility. That fallback does **not** establish production packet feasibility.
Each real table must also pass the runtime's table-state checks. It retains exact
logs, compute usage, packet sizes and hashes for post-swap signed-fee-bound
rollback, the opening sale and full-q buyback. It asserts debt/exposure/vault,
actual upfront fees, collateral/rent/refunds, scratch closure, unchanged real mint
and token metadata, and complete rollback of protected accounts.

For captured Pump routes with a writable isolated authority, opening signs a
5,000,000-lamport native setup allowance within the total temporary-funding cap.
The replayer separately measures the actor's actual setup cost and unused refund;
setup cannot enter the loan's WSOL notional, collateral or protocol/lender fees.
It proves atomic refusal for zero/insufficient allowance, an allowance above the
program maximum, and a total advance above the signed funding cap. The subsequent
Pump full buyback deliberately signs **zero** setup allowance, testing the proposed
keeper default after the borrower's real opening initialized required accounts.
The separate `writable_authority.rs` tests exercise a completely unused 5,000,000
allowance on real CPMM execution, readonly gifts, untracked writable-authority
gifts, invalid authority states and signed setup/funding boundaries.

Native Pump curves move native SOL while AMMs can retain it in WSOL accounts.
Conservation therefore sums all external captured account lamports, including
initially absent route PDAs, curve/fee/rent accounts and backed WSOL vaults,
excluding protocol financial accounts, actors and scratch. Its delta must equal
actual WSOL buy spend minus sale proceeds **plus** separately measured actor setup
costs. WSOL token-data deltas remain in the receipt as a diagnostic and are never
double-counted on top of backing lamports. Exact base-token conservation, full-q
vault credit and all per-loan financial invariants remain independently checked.

Snapshots can span slots and buy quotes precede the simulated sell's DEX state
changes. Such a quote may correctly fail replay and must be recaptured or
explained; it is never made successful by changing pool balances or weakening
minimum outputs. A passing candidate qualifies exactly the named route/state,
not all Jupiter DEXes, all quantities, current landing, or private price history.
Candidate promotion and a fresh final-baseline campaign remain separate steps.
