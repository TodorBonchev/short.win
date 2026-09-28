# services

The off-chain services of short.win (ARCHITECTURE §7). A **host** Cargo workspace with its own
`Cargo.lock` (ARCH §1.2): the Agave client crates and tokio never enter the on-chain
workspace's lockfile.

| Crate | What |
|---|---|
| `common/` (`shortwin-services-common`) | The chain-client abstraction (`Chain`: clock, accounts, program accounts, blockhash, simulate, send, recent priority fees), its JSON-RPC backend (`RpcChain`), transaction building: v1 (SIMD-0385) with a v0 fallback (ComputeBudget instructions, optional lookup tables), `⌈simulated × 1.1⌉` compute units, sizes, decompilation |
| `keeper/` (`shortwin-keeper`) | Keeper v0 (ARCH §7.1 duties 1–4, batching, degraded mode) and its binary |
| `keeper-litesvm/` (`shortwin-keeper-litesvm`) | TEST-ONLY: the in-process LiteSVM backend (the program-test harness's invariant hooks after every instruction of every keeper transaction) and the keeper's scenario tests on the mock venue |

## Keeper v0

What it does, per tick (one per slot; `keeper/src/keeper.rs`):

- **Duty 1, coverage** (`coverage.rs`, `decide.rs`): one observation per second on a market with a
  position near a threshold (live CR below 1.3·L, deadline within 60 s, armed) or with
  `|P_spot/P_ema − 1| > 2 %`; every 30 s on markets with open positions; every 30 s on markets
  without a route (the continuous S3 chain, duty 5); none on markets without positions.
- **Duty 2, bursts**: an observation in the open or last completed bucket that deviates by more
  than 2 % from the spot prices the keeper itself read in that second was not the keeper's (an
  atomic `[pump, crank, dump]` leaves exactly that): one observation every slot for 60 s
  (never while an S3 on an executable venue is due: once per second there, duty 5).
- **Duty 3, exits**: `liquidate` at the first second the program accepts it (the fixed-δ bound;
  retried every second on `ExitBoundNotMet`), `expire` when the expiry bound is met. Trading
  exits try the extras sets `current`, `none`, `previous` and keep the first that simulates
  (duty 10's rule).
- **Duty 4, `sol_settle`**: at most once per second for armed positions and for liquidatable
  positions without a normal buyback (it arms, keeps the clock, disarms on box evidence, or
  settles), and for positions whose S3 is due (on a halted venue: probed by simulation, sent
  only when it settles).
- **Duty 7, batching**: an exit is the market's observation for the second (no separate crank);
  the remaining cranks share transactions, 6 per v1 transaction, 4 per v0 transaction where the
  cluster refuses v1 (re-probed hourly); each batch is simulated and a market whose crank fails
  is dropped from it and cranked alone; batches that come out too large are split.
- **Duties 8, 14, budget and priority** (`fees.rs`): compute-unit limit `⌈simulated × 1.1⌉`;
  priority price = p75 of the recent per-account fees on the transaction's markets, ×1.5 per
  consecutive missed second, capped at the O17 ceiling (a separate ceiling for exits); the O17
  per-market daily observation budget (exits are never throttled).
- **Duty 9, degraded mode** (`alerts.rs`): three consecutive seconds with a required observation
  and none landed (budget, outage, contested blocks), or a `CoverageGap` event from the program,
  raise `Alert::Degraded` — page the Emergency multisig to `set_pause(market)` with reason
  `KeeperCoverage` (`set_coverage_pause` itself waits for O17); ten minutes of restored coverage
  raise `Alert::CoverageRestored`.
- Per-market metrics (coverage seconds, observations, exits, settlements, CU, lamports, misses,
  bursts, gaps, degraded) in the Prometheus text format (`--metrics`).

**The program's own code decides.** The keeper links the `shortwin` crate (account types,
instruction builders, the venue reader `venues::read_state`, the market prologue
`engine::prologue::run`, the exit references `engine::bounds`) and `shortwin-math` (legs, CR,
buyback test, S3 timer): each decision is the program's on a copy of the market projected to
the cluster's clock. Every send is still simulated first, and the simulation is the final word
(a program refusal is reported, never retried blindly).

Not in v0 (later milestones, ARCH §7.1): Jito bundles with tips for liquidations (the priority-fee
RPC path is built); Yellowstone gRPC subscriptions (v0 polls `getMultipleAccounts` /
`getProgramAccounts`); duty 5's Raydium fast path (M4), duty 6's graduation work (M5/M6), duty
10's ProgramData watcher and `flag_venue_upgraded`, duties 11–13 (`crank_sol_usd`, pool monitor,
lender pot); the venue account lists of the MVP venues (M3–M6; v0 knows the TEST-ONLY mock only,
feature `test-venue`).

### Run

```sh
cargo build --manifest-path services/Cargo.toml -p shortwin-keeper --release
services/target/release/shortwin-keeper --rpc https://… --keypair keeper.json \
  --config services/keeper/keeper.example.toml --metrics 127.0.0.1:9464
```

The keypair is a hot fee payer with no protocol authority. `keeper.example.toml` documents every
setting (all optional; the file is the defaults, a unit test checks it).

## Tests

```sh
cargo test --manifest-path services/Cargo.toml --workspace
```

The scenario tests (`keeper-litesvm/tests/scenarios`) load the same builds as the program tests:
short.win's test-venue build (`scripts/build-test-venue.sh`, or `SHORTWIN_TEST_VENUE_SO`) and the
mock venue (`anchor build --tools-version v1.57 --arch v3` → `target/deploy/mock_cpamm.so`, or
`MOCK_CPAMM_SO`). Every keeper transaction runs the harness's invariant hooks after every
instruction. Scenarios: cadence (30 s idle, per second near a threshold), batching (v1 6 + 1,
v0 4 + 3 after a v1 refusal, the protocol ALT, a failing crank dropped and sent alone), bursts
(an atomic pump-crank-dump → 60 s per slot; honest observations start none), degraded mode
(outage → `CoverageGap` → restored after 10 minutes; an exhausted budget), priority (p75, the
ceiling, the fee charged), market selection and dry run, and the exits: a trend and a jump
liquidated at the first second an outside oracle's simulation accepts, a spike above the bound
waited out (`ExitBoundNotMet`) and liquidated the next second, an expiry at the deadline, S3 on a
disabled mock pool (continuous chain; an expired position settles, a healthy one stays frozen),
S1 after arming with one `sol_settle` per second, and extras (a published template, a hostile
`current` bypassed with `previous`).

The JSON-RPC backend has its own check against a live local cluster (`common/tests/rpc_local.rs`,
ignored by default): clock, accounts, program accounts, recent fees, and a transfer simulated and
sent as v1 and as v0 (both accepted by an Agave 4.1.2 `solana-test-validator`):

```sh
solana-test-validator --reset --quiet &
SHORTWIN_RPC_URL=http://127.0.0.1:8899 \
  cargo test --manifest-path services/Cargo.toml -p shortwin-services-common --test rpc_local -- --ignored
```

Measured on the mock: a v1 batch of 6 cranks is 1,360 B, 35 accounts, ≈ 146 k CU; a v0 batch of
4 is 1,024 B (965 B with a 3-key protocol ALT).

## Lockfile and toolchain

The lockfile was seeded from the root `Cargo.lock`: the scenario tests link the LiteSVM harness
(`tests/litesvm`), so this workspace holds the same Agave **4.2.2** runtime line and solana-*
component versions (C1-14), and the RPC client is the matching `solana-rpc-client` 4.2.2 (ARCH
§1.1 names the 4.3 clients for services; they move together with LiteSVM / Mollusk). Host
toolchain from the root `rust-toolchain.toml`. `deny.toml` here is the root policy plus the TLS
stack's licences (ISC, CDLA-Permissive-2.0); `cargo audit -f services/Cargo.lock` uses the root's
ignore list.
