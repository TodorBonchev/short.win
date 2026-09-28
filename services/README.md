# services

The off-chain services of short.win (ARCHITECTURE §7). A **host** Cargo workspace with its own
`Cargo.lock` (ARCH §1.2): the Agave client crates and tokio never enter the on-chain
workspace's lockfile.

| Crate | What |
|---|---|
| `common/` (`shortwin-services-common`) | The chain-client abstraction (`Chain`: clock, accounts, account headers, program accounts — also by exact size, a `dataSize` filter on JSON-RPC —, blockhash, simulate, send, recent priority fees), its JSON-RPC backend (`RpcChain`), transaction building: v1 (SIMD-0385) with a v0 fallback (ComputeBudget instructions, optional lookup tables), `⌈simulated × 1.1⌉` compute units, sizes, decompilation |
| `keeper/` (`shortwin-keeper`) | Keeper v1 (ARCH §7.1 duties 1–5, 7–10, 12, 14 on Raydium AMM v4 and CPMM, with duty 5's Raydium fast path) and its binary; `shortwin-build-alt`, the protocol address lookup table of A28 (`scripts/build-alt.sh`) |
| `keeper-litesvm/` (`shortwin-keeper-litesvm`) | TEST-ONLY: the in-process LiteSVM backend (the program-test harness's invariant hooks after every instruction of every keeper transaction) and the keeper's scenario tests on the mock venue and on the Raydium mainnet fork |

## Keeper

What it does, per tick (one per slot; `keeper/src/keeper.rs`):

- **Venues** (`venue.rs`): Raydium AMM v4 and CPMM markets (M3), through the adapters' own key
  functions (`shortwin::venues::raydium_v4::{read_keys, trade_keys}`, the same in
  `raydium_cpmm`) with ARCH §3.12's writability in trades (pool and vaults writable; the LP mint
  and the CPMM `AmmConfig` read-only — a config is shared by every pool on it; `trader_wsol` and
  the CPMM observation writable); and the TEST-ONLY mock (feature `test-venue`). Pump markets are
  skipped with a reason until M5–M6.
- **Duty 1, coverage** (`coverage.rs`, `decide.rs`): one observation per second on a market with a
  position near a threshold (live CR below 1.3·L, deadline within 60 s, armed) or with
  `|P_spot/P_ema − 1| > 2 %`; every 30 s on markets with open positions; every 30 s on markets
  without a route (the continuous S3 chain, duty 5); none on markets without positions — except
  a Raydium market inside its listing window (M4V-5): every `listing_interval_s` (60 s) through
  the window and every second in its last `EMA_WARMUP_BUCKETS + 2` (10) seconds, and after it
  until its post-change warm-up is over (`ema_warm == 0` as the next instruction's roll sees it,
  M4R), so the first open after the window passes the program's gate (`EmaWarmingUp`) on a
  `P_ema` the pool's own buckets seeded. After a `PoolSwitched` (any market with open positions
  and `ema_warm > 0` on an executable venue; M4R, duty 5) every second until the warm-up is over:
  the program refuses keeper exits until then, so the positions' plan is `Watch` and no
  `liquidate` / `expire` is sent; the first second the projection shows `ema_warm == 0` (adoption
  + 9 s with per-second observations) the exits go out as usual.
- **Duty 2, bursts**: an observation in the open or last completed bucket that deviates by more
  than 2 % from the spot prices the keeper itself read in that second was not the keeper's (an
  atomic `[pump, crank, dump]` leaves exactly that): one observation every slot for 60 s
  (never while an S3 on an executable venue is due: once per second there, duty 5).
- **Duty 3, exits**: `liquidate` at the first second the program accepts it (the fixed-δ bound;
  retried every second on `ExitBoundNotMet`), `expire` when the expiry bound is met. Trading
  exits try the extras sets `current`, `none`, `previous`, `stable` and keep the first that
  simulates (duty 10's rule; `stable` is M2-SEC-2's set).
- **Duty 4, `sol_settle`**: at most once per second for armed positions and for liquidatable
  positions without a normal buyback (it arms, keeps the clock, disarms on box evidence, or
  settles), and for positions whose S3 is due (on a halted venue: probed by simulation, sent
  only when it settles).
- **Duty 5, no route**: a halted venue (a Raydium admin halt, a CPMM status bit, a pool that stops
  parsing) or a fee above the hard cap starts the S3 chain at the keeper's next tick and is
  observed every 30 s after, so the chain never restarts.
- **Duty 7, batching**: an exit is the market's observation for the second (no separate crank);
  the remaining cranks share transactions, up to 6 per v1 transaction and 4 per v0 transaction
  where the cluster refuses v1 (re-probed hourly), each batch **cut where its transaction would
  exceed the format's size or account limit** (so any mix of the mock's 3-key, AMM v4's 4-key and
  CPMM's 5-key ReadSets packs as densely as fits); each batch is simulated and a market whose
  crank fails is dropped from it and cranked alone.
- **Duties 8, 14, budget and priority** (`fees.rs`): compute-unit limit `⌈simulated × 1.1⌉`;
  priority price = p75 of the recent per-account fees on the transaction's markets, ×1.5 per
  consecutive missed second, capped at the O17 ceiling (a separate ceiling for exits); the O17
  per-market daily observation budget (exits are never throttled).
- **Duty 9, degraded mode** (`alerts.rs`): three consecutive seconds with a required observation
  and none landed (budget, outage, contested blocks), or a `CoverageGap` event from the program,
  raise `Alert::Degraded` — page the Emergency multisig to `set_pause(market)` with reason
  `KeeperCoverage` (`set_coverage_pause` itself waits for O17); ten minutes of restored coverage
  raise `Alert::CoverageRestored`.
- **Duty 10, the venue drift monitor** (`monitor.rs`, config `[venue_watch]`):
  - **Upgrades.** Every tick the keeper reads the 45-byte headers (`getMultipleAccounts` with
    `dataSlice`: the accounts are up to 10 MiB) of the six ProgramData accounts the `VenueGuard`
    latch indexes (pump, pAMM, pfee, AMM v4, CPMM, Jupiter) and compares their deploy slots with
    the latch, by the program's own rule: bit clear and slot ≠ `guard_slot` → flag; bit set and
    slot ≠ `flagged_slot` → a second upgrade during review, refresh. It then sends
    `flag_venue_upgraded(venue)` in that slot at the exit priority ceiling (simulated first; a
    latch someone else already set is not repeated; an unflagged upgrade is retried every second)
    and raises `Alert::VenueUpgraded` once per upgrade and `Alert::VenueUpgradeFlagged` when its
    flag lands — page the Emergency multisig to run the fork suite against the new binary and
    `revalidate_venue(venue, slot)`. Either Raydium program's upgrade blocks opens on every
    Raydium market (one mask). Token-2022's ProgramData is watched too, alert only
    (`Alert::ProgramUpgraded`).
  - **Pool admin actions.** Raydium's pool admin (a 2-of-3 Squads v3 vault with no timelock) can
    halt a pool (AMM v4 `SetParams` status 2–5 or 7 with a future open time; CPMM
    `update_pool_status` bit 2) and raise fees (AMM v4 `SetParams` fees per pool; CPMM
    `update_amm_config` for every pool on a config). A polling keeper watches the accounts those
    instructions write — each market's pool and CPMM config, already in the ReadSet it reads —
    and raises `Alert::VenueAdmin` for every change: route lost / restored (with the status),
    status, open time, and the one-way fee measured against `venue_fee_cap_bps` (opens refused)
    and `venue_fee_hard_cap_bps` (`s3_no_route`), naming the instruction that writes the field. A
    pool that stops parsing is reported as such ("not executable", A63).
- **Duty 12, the Raydium pool monitor, and duty 5's Raydium fast path** (`pool_monitor.rs`,
  config `[pool_select]`; ARCH §2.5 `None`, §2.6):
  - **Discovery.** Every Raydium SOL pool of each covered Raydium or list-only market's mint,
    through `getProgramAccounts` byte filters: AMM v4 `coin_vault_mint` @400 / `pc_vault_mint`
    @432 and CPMM `token_0_mint` @168 / `token_1_mint` @200, the mint on one side and WSOL on
    the other (four calls per mint, each filtered to the one pool size the program verifies —
    `dataSize` 752 B for AMM v4, 637 B for CPMM), at the first sight of a market and every
    `discovery_interval_s` (300 s) after — every `fast_discovery_interval_s` (10 s) while the
    incumbent fails `pool_fail` or a collection window runs (never while a ranking is still in
    progress). The calls can go to another endpoint (`discovery_rpc`, `--discovery-rpc`).
  - **Ranking** (M4-SEC-7: anyone can create pools for a mint, ≈ 0.17 SOL each). Each discovered
    pool is scored once per discovery by its locked SOL `λ·R` — its pool account (a status or
    nonce the verifier would refuse scores nothing) and one batched read of its WSOL vault and
    LP mint, at most `rank_keys_per_tick` (200) accounts a tick and market, a large discovery
    over several ticks with the previous ranking in force — and only the best `max_candidates`
    (8) are evaluated. Unlocked liquidity scores nothing, so junk pools rank last.
  - **Evaluation.** Each ranked candidate's ReadSet, read fresh, goes through the program's own
    verifier (`engine::pool_select::verify_candidate`: the adapters' `verify` and `eligible()`,
    the locked-depth floor at the market's `P_ema`; plus `window_floor_ok`, the window floor of
    a collection window's candidate, M4V-1) on host `AccountInfo`s, which gives the
    program's locked depth `D`; a list-only market's D1 check is the program's `classify_pump`
    on its detection set. Pool selection runs after the tick's exits and crank batch, so its RPC
    work never delays duties 1–4 (the observation that keeps a failing market's `pool_fail`
    chain goes out in a batch of its own right after). Every `eval_interval_s` (30 s),
    and every tick while the fast path is armed, a window runs, the incumbent just started or
    stopped failing, or a waiting candidate meets a market whose last position just closed.
  - **Decision.** The program's own `engine::pool_select::decide` on the market projected to
    the clock (the prologue: `pool_d` refreshed, the `pool_fail` chain stepped), for the deepest
    eligible candidate other than the incumbent and the window's own: `propose_pool` exactly
    when it accepts — a list-only market's first pool, a listing-window replacement (5 %), a
    zero-position adoption (25 %; in the tick the keeper sees the last position closed), the
    fast path's collection window (opened at the first armed second, outbidding a shallower
    window of someone else's); `execute_pool_switch` at the first second after the window (a
    cancelled window is proposed again if the fast path is still armed; a list-only market with
    open positions — M5's curve that graduated elsewhere — collects its candidates in a window
    the same way, M4-SEC-4). A window is opened or improved only with a candidate above the
    window floor (else nothing, `PoolTooThin`). Nothing while positions are open and the
    incumbent is healthy (`PositionsOpen`); a candidate whose own program the `VenueGuard` latch
    holds is left out of the ranking (`VenueUpgraded`; M4-SEC-8: an upgrade of the other Raydium
    program does not hold the fast path up), and a window whose candidate's program is latched
    is executed anyway — the program cancels it (M4V-4) and the best pool on the other program
    opens the next window in the same tick. A failing market **without** positions, which duty 5's no-route cadence
    does not observe, is observed by the monitor (at once, then every 30 s) while a candidate is
    deep enough for the fast path, so its `pool_fail` chain arms. Sends are simulated first, at
    the exit priority ceiling, outside the O17 observation budget.
  - **Reports.** The decoded `PoolProposed` / `PoolSwitched` / `PoolSwitchCancelled` of the
    keeper's transactions; `Alert::PoolSwitched`, `PoolWindow`, `PoolSwitchCancelled` for the
    keeper's actions and for anyone else's seen on chain; `Alert::PoolPinned` when a candidate
    with ≥ 25 % more locked depth has waited `pinned_alert_s` (1 h) behind open positions
    (R4B-10: page the Emergency multisig to pause opens there so the market drains);
    `Alert::PoolSelectRefused` for an unexpected refusal. After an adoption the duty-10 admin
    watch starts afresh on the new pool.
- Per-market metrics (coverage seconds, observations, exits, settlements, CU, lamports, misses,
  bursts, gaps, venue admin changes, pool proposals / executions / switches, degraded) and
  `venue_flags`, `pool_discoveries` in the Prometheus text format (`--metrics`).

**The program's own code decides.** The keeper links the `shortwin` crate (account types,
instruction builders, the venue reader `venues::read_state` and the adapters' account lists, the
market prologue `engine::prologue::run`, the exit references `engine::bounds`, the pool
selection's `engine::pool_select::{verify_candidate, decide}` and `venues::detect::classify_pump`),
`shortwin-venue-state` (the AMM v4 / CPMM / ProgramData readers) and `shortwin-math` (legs, CR,
buyback test, S3 timer): each decision is the program's on a copy of the market projected to
the cluster's clock. Every send is still simulated first, and the simulation is the final word
(a program refusal is reported, never retried blindly).

Not yet (later milestones, ARCH §7.1): Jito bundles with tips for liquidations (the priority-fee
RPC path is built; T-SIZE shows a tip transfer fits the worst Raydium exit); Yellowstone gRPC
subscriptions (the keeper polls `getMultipleAccounts` / `getProgramAccounts`, and watches admin
actions through the accounts they write rather than the admin's transactions; it sees other
parties' pool selection through the market's state, not their transactions); duty 6's
graduation work and the pump venues' lists (M5/M6), including `crank_price` on list-only
markets (their pump re-detection); duties 11 and 13 (`crank_sol_usd`, lender pot).

### Run

```sh
cargo build --manifest-path services/Cargo.toml -p shortwin-keeper --release
services/target/release/shortwin-keeper --rpc https://… --keypair keeper.json \
  --config services/keeper/keeper.example.toml --metrics 127.0.0.1:9464 \
  [--discovery-rpc https://…]   # getProgramAccounts for the pool discovery (default: --rpc)
```

The keypair is a hot fee payer with no protocol authority. `keeper.example.toml` documents every
setting (all optional; the file is the defaults, a unit test checks it). At start the keeper warns
if its `lookup_tables` lack a protocol ALT key.

## The protocol ALT (`shortwin-build-alt`, `scripts/build-alt.sh`)

ARCH §6.4 A28: one address lookup table with the protocol's static accounts, used by every v0
transaction (the keeper's v0 fallback, the frontend). The list (`keeper/src/alt.rs`), in order:

1. the programs a short.win instruction names as accounts (pump, pAMM, pfee, AMM v4, CPMM, SPL
   Token, Token-2022, ATA, System) and short.win's event authority; the WSOL mint; pump `global`,
   `event_authority`, `fee_config`, global volume accumulator, and the same four of PumpSwap; the
   AMM v4 and CPMM vault authorities; the 21 CPMM `AmmConfig`s; short.win's `config`,
   `venue_guard`, `oracle_cache`;
2. the four `VenueExtras` PDAs (an addition to ARCH's enumeration: static protocol accounts that
   every trading instruction with an extras set names);
3. with `--rpc`: the live pump and PumpSwap fee and buyback recipients (`Global` @41, @162, @741;
   `GlobalConfig` @57, @643) and their WSOL ATAs — 17 distinct recipients today.

83 addresses on mainnet (2026-09-28). ComputeBudget and short.win's own id stay static keys (a v0
message cannot load a program id). No sysvar appears: no instruction on these paths takes one.

```sh
scripts/build-alt.sh print                                   # the static part, no RPC
scripts/build-alt.sh print  --rpc https://api.mainnet-beta.solana.com
scripts/build-alt.sh create --rpc <url> --keypair payer.json [--authority auth.json] [--freeze]
scripts/build-alt.sh extend --rpc <url> --keypair payer.json --table <alt>   # add what is missing
scripts/build-alt.sh check  --rpc <url> --table <alt>        # exit 1 if an address is missing
```

`create` derives the table from the authority and the latest finalized slot, extends it in chunks
of 24 and optionally freezes it. A frozen table can never be deactivated (a deactivated table
breaks every v0 transaction naming it) but also never extended: a new pump recipient then rides
inline (31 bytes) until a new table replaces it. Entries are usable one slot after they are
added. Put the address in the keeper's `lookup_tables`.

## Tests

```sh
cargo test --manifest-path services/Cargo.toml --workspace
```

The scenario tests (`keeper-litesvm/tests/scenarios`) load the same builds as the program tests:
short.win's test-venue build (`scripts/build-test-venue.sh`, or `SHORTWIN_TEST_VENUE_SO`) and the
mock venue (`anchor build --tools-version v1.57 --arch v3` → `target/deploy/mock_cpamm.so`, or
`MOCK_CPAMM_SO`) for the mock scenarios, and the release build (`target/deploy/shortwin.so`, or
`SHORTWIN_SO`) with the committed mainnet fixtures (`tests/fixtures`) for the Raydium ones. Every
keeper transaction runs the harness's invariant hooks after every instruction.

Mock scenarios: cadence (30 s idle, per second near a threshold), batching (v1 6 + 1, v0 4 + 3
after a v1 refusal, the protocol ALT, a failing crank dropped and sent alone), bursts (an atomic
pump-crank-dump → 60 s per slot; honest observations start none), degraded mode (outage →
`CoverageGap` → restored after 10 minutes; an exhausted budget), priority (p75, the ceiling, the
fee charged), market selection and dry run, and the exits: a trend and a jump liquidated at the
first second an outside oracle's simulation accepts, a spike above the bound waited out
(`ExitBoundNotMet`) and liquidated the next second, an expiry at the deadline, S3 on a disabled
mock pool (continuous chain; an expired position settles, a healthy one stays frozen), S1 after
arming with one `sol_settle` per second, and extras (a published template, a hostile `current`
bypassed with `previous`).

Raydium scenarios (`raydium*.rs`, the dumped AMM v4 and CPMM programs and pools):

- `raydium`: six markets (one AMM v4, five CPMM: SPL and Token-2022 bases, WSOL as token 0 and 1,
  creator fees) in one v1 crank batch; after a v1 refusal, v0 batches packed by size; a whale's
  real buy on AMM v4 and CPMM liquidated at the first second an outside oracle accepts; healthy
  positions on AMM v4 and a Token-2022 CPMM pool expired at their deadlines.
- `raydium_admin` (the real admin instructions, signed as `GThUX1…` with signature checks off for
  that transaction): an AMM v4 `SetParams` halt reported at once, the S3 chain started that tick
  and kept continuous, the expired position settled in S3 at `max(deadline, chain + 600 s)`, the
  healthy one frozen, the resume reported; fee changes on both programs against the caps (opens
  refused between them; above the hard cap the S3 chain starts while the venue still executes);
  CPMM status bits (swap = lost route, deposit = status only); upgrades of either program flagged
  in the slot they are seen, once, refreshed during review, nothing after `revalidate_venue`, not
  repeated when someone else flagged first; Token-2022 alerted only; the watch switched off.
- `raydium_alt`: the protocol ALT created, extended with the recipients (from the committed
  mainnet dumps), frozen, through the keeper's own chain backend and `alt` module; it holds every
  address the program tests' T-SIZE table assumes; the keeper's v0 crank batch and a liquidation
  run through it.
- `pool_select` (M4; planted sibling pools, `shortwin_litesvm_tests::pool_select`, traded by the
  dumped ELFs): a thin eligible listing (Fartcoin's reversed pool) replaced by the main pool in
  the keeper's first tick, inside the listing window; a list-only market moved onto its deepest
  eligible pool (Fartcoin's real curve: D1 and `graduated_elsewhere`; a thinner CPMM sibling and
  the too-thin reversed pool passed over), a pump coin's list-only market left alone; an
  incumbent halted with two positions open — the window opened on the deepest sibling at the
  first armed second (`pool_fail` for 120 s), executed at the first second after it, positions
  byte for byte unchanged, `P_ema` carried at the adoption, the ring's liquidity the new pool's
  anchor, the new pool observed every second through the post-change warm-up and its depth,
  a clear guard and a ≤ 1 % box after it, the positions closed on the new pool; underwater
  positions liquidated at adoption + 9 s with no exit sent during the warm-up (M4R); a front-running
  outsider's shallower window outbid and the deepest pool adopted; a deeper pool created during
  the window found by the fast discovery, proposed and adopted; a halted pool without
  positions taken through the fast path by the keeper's own chain observations; nothing
  proposed while a position is open and the incumbent healthy (twice the depth waiting), one
  `PoolPinned` alert, the adoption in the tick the position closed; proposals held by a flagged
  Raydium upgrade until `revalidate_venue`; M4V-5's listing-window observations (60 observations
  60 s apart and 10 in the last 10 s, the first open after the window lands; a window nobody
  observed is warmed up in the 8 s after it, the open refused `EmaWarmingUp` until then).

The JSON-RPC backend has its own check against a live local cluster (`common/tests/rpc_local.rs`,
ignored by default): clock, accounts, program accounts, recent fees, and a transfer simulated and
sent as v1 and as v0 (both accepted by an Agave 4.1.2 `solana-test-validator`):

```sh
solana-test-validator --reset --quiet &
SHORTWIN_RPC_URL=http://127.0.0.1:8899 \
  cargo test --manifest-path services/Cargo.toml -p shortwin-services-common --test rpc_local -- --ignored
```

Measured on the mock: a v1 batch of 6 cranks is 1,360 B, 35 accounts, ≈ 146 k CU; a v0 batch of
4 is 1,024 B (965 B with a 3-key protocol ALT). On Raydium: a v1 batch of 6 cranks (1 AMM v4 +
5 CPMM) is 1,627 B, 43 accounts, ≈ 166 k CU; without v1 and without a table, v0 batches of 3
(977–1,010 B); through the built ALT, a v0 batch of 3 is 887 B and an AMM v4 liquidation 705 B.
The worst cases of every Raydium transaction are T-SIZE's (`tests/litesvm/tests/tx_size_raydium.rs`).

## Lockfile and toolchain

The lockfile was seeded from the root `Cargo.lock`: the scenario tests link the LiteSVM harness
(`tests/litesvm`), so this workspace holds the same Agave **4.2.2** runtime line and solana-*
component versions (C1-14), and the RPC client is the matching `solana-rpc-client` 4.2.2 (ARCH
§1.1 names the 4.3 clients for services; they move together with LiteSVM / Mollusk). The
keeper's `solana-address-lookup-table-interface` 3.1 is the version LiteSVM already brought in.
Host toolchain from the root `rust-toolchain.toml`. `deny.toml` here is the root policy plus the
TLS stack's licences (ISC, CDLA-Permissive-2.0); `cargo audit -f services/Cargo.lock` uses the
root's ignore list.
