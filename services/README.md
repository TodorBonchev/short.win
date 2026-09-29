# services

The off-chain services of short.win (ARCHITECTURE §7). A **host** Cargo workspace with its own
`Cargo.lock` (ARCH §1.2): the Agave client crates and tokio never enter the on-chain
workspace's lockfile.

| Crate | What |
|---|---|
| `common/` (`shortwin-services-common`) | The chain-client abstraction (`Chain`: clock, accounts, account headers, program accounts — also by exact size, a `dataSize` filter on JSON-RPC —, blockhash, simulate, send, recent priority fees, Jito bundles: send and pre-flight), its JSON-RPC backend (`RpcChain`; bundles through a Jito block engine's `sendBundle`, **UNVERIFIED** against a live engine), transaction building: v1 (SIMD-0385) with a v0 fallback (ComputeBudget instructions, optional lookup tables), `⌈simulated × 1.1⌉` compute units, sizes, decompilation |
| `keeper/` (`shortwin-keeper`) | Keeper v1 (ARCH §7.1 duties 1–5, 7–10, 12, 14 on Raydium AMM v4 and CPMM, with duty 5's Raydium fast path) and its binary; `shortwin-build-alt`, the protocol address lookup table of A28 (`scripts/build-alt.sh`) |
| `keeper-litesvm/` (`shortwin-keeper-litesvm`) | TEST-ONLY: the in-process LiteSVM backend (the program-test harness's invariant hooks after every instruction of every keeper transaction; bundles executed in order and all-or-nothing) and the keeper's scenario tests on the mock venue and on the Raydium and pump mainnet forks |

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
  + 9 s with per-second observations) the exits go out as usual. The per-second cadence then
  lasts for as long as the post-change reference guard is set (`ema_guard == 1`,
  `MarketNeeds::guarded`; M4F-1): every second keeps its honest bucket low, so deposit-wrapped
  cranks in otherwise idle seconds cannot lift the ring's liquidity lows and clear the guard.
- **Duty 2, bursts**: an observation in the open or last completed bucket whose **price**
  deviates by more than 2 % from what the keeper itself read in that second was not the keeper's
  (an atomic `[pump, crank, dump]` leaves exactly that): one observation every slot for 60 s from
  that second (never while an S3 on an executable venue is due: once per second there, duty 5).
  The per-slot liquidity bursts M4F-1 added stay reverted (M4G-2: an atomic `[deposit,
  crank_price, withdraw]` bought a 60-s per-slot burst, 25–150× its cost in the keeper's).
- **Duty 2, liquidity coverage** (FITS-2 (b), owner decision of 2026-09-29): on a market **with
  open positions**, an observation in the open or last completed bucket whose **liquidity**
  extremes (`llo` / `lhi`; the last completed bucket's from the ring entry that integrated it)
  deviate by more than 2 % (`burst_dev_bps`) from the `√(X·Y)` the keeper itself read in that
  second — an LP-wrapped `[withdraw, crank_price, deposit]` (the withdraw drag of `L_ema` /
  `L_slow`) or `[deposit, crank_price, withdraw]` (the freeze) — starts one observation **per
  second** (never per slot) for `liq_cover_s` (10 s) from that second, extended by every
  recurrence (`MarketNeeds::liq_covered`). While the attacker keeps wrapping, every second carries
  the keeper's honest bucket extreme — no attacker-only second — at one keeper crank per attacker
  transaction (1:1); an isolated wrap buys at most 10 keeper observations. It holds `L_ema`, it
  cannot lift a low reached genuinely before the wrapping started (an LP withdrawn for real for a
  few seconds: nothing foreign then) — FITX-1, ECON §12.12 residual 10, owner item O18. A market without open
  positions never gets liquidity-triggered coverage (nothing there reads the liquidity EMA);
  `liq_cover_s = 0` disables it. A venue change drops the depths viewed before it.
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
- **Duty 6, graduation on the pump curve** (`graduation.rs`, config `[graduation]`; M5, ARCH §5.1,
  §5.4, §7.1): the keeper covers `PumpCurve` and `PumpMigrating` markets with the curve adapter's
  own ReadSet and, for a curve exit, its TradeExtras T0–T19 (a fee and a buyback recipient of the
  live `Global`, the live creator's vault); its risk view is the program's own quoter
  (`VenueState::quoter`: `B̂_str` above `rT`).
  - **`migrate_v2` as soon as a curve completes** (ADOPTED Q16): a covered market whose curve is
    complete with `vT > 0` and whose canonical PumpSwap pool does not exist gets pump's
    `migrate_v2` — top-level, alone with its compute budget (A15), its 27 IDL accounts and the
    two boost accounts — at most once a second, then an observation of the market in the same
    second (M5 records `→ PumpMigrating`; M6's `→ PumpSwap` and the new pool's first-trade duties
    follow). A failure raises `Alert::MigrateFailed` once and is retried every second.
  - **The straddle** (`Plan::Straddle`): a liquidatable position whose `Q` exceeds its live
    curve's `rT` cannot exit on the curve (pump reverts 6021, `CurveStraddle`). The keeper builds
    ARCH §5.4's Jito bundle — tx1: its base ATA and `complete_curve(max_sol_cost)` of the
    `complete-curve` helper (`programs/complete_curve`: it reads `rT` at execution, so a curve
    trade between the keeper's read and the bundle cannot break it; `max_sol_cost` = the keeper's
    quote + `complete_slippage_bps` + a rent allowance); tx2: `migrate_v2`; tx3: the exit on the
    new pool (**M6**); the tip on the last — pre-flights it (`simulate_bundle` where the chain can,
    else tx1 alone) and sends it through the block engine (`--block-engine`). Until M6 there is no
    tx3 and a migrated curve stays `PumpMigrating`, so the two-transaction bundle goes out only
    with `send_without_exit` (off by default); otherwise, and without a block engine, the keeper
    raises `Alert::StraddleWaiting` once and calls `sol_settle` for the position each second (it
    arms, keeps the clock or settles S1 / S2). The keeper keeps the tokens it buys
    (`max_complete_lamports` caps the spend).
  - **List-only markets** (M5): a list-only (`None`) market is covered with its detection set
    `[bonding_curve, canonical_pool]` as ReadSet (the program's own for `crank_price` and
    `sol_settle`). One with positions open — a curve that graduated elsewhere — is observed on the
    no-route cadence and its positions are settled in S3 when due (duty 5; M2-SEC-5), and the pool
    monitor collects its Raydium pool in a window (duty 12). One whose detection set shows a live
    or migrating curve again (the program's `classify_pump`) is cranked, which moves it onto the
    curve; the keeper then creates the trader's base ATA and pump UVA the curve legs require
    (`crank_price` has no payer and creates neither), checked once per curve market.
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
  - **The marks** (ECON #100). The plan runs the program's own `carried_fit` on the market as
    its view holds it: `max_q_hwm` (falls only when `open_positions` returns to 0) and
    `min_open_fee_ppm` (the lowest fee an open paid since then; the fee-scaled caps take
    `min(min_open_fee_ppm, φ_cand)`, FIT-2). The keeper has no duty to lower the mark: a large
    position that has closed while others stay open keeps a thinner candidate refused
    (`CarriedPositionsTooLarge`) and the positions exit on the failed pool through the S3 chain
    at fair value (FIT-3, a disclosed residual; `pool_select`'s scenario below).
  - **Reports.** The decoded `PoolProposed` / `PoolSwitched` / `PoolSwitchCancelled` of the
    keeper's transactions; `Alert::PoolSwitched`, `PoolWindow`, `PoolSwitchCancelled` for the
    keeper's actions and for anyone else's seen on chain; `Alert::PoolPinned` when a candidate
    with ≥ 25 % more locked depth has waited `pinned_alert_s` (1 h) behind open positions
    (R4B-10: page the Emergency multisig to pause opens there so the market drains);
    `Alert::PoolSelectRefused` for an unexpected refusal. After an adoption the duty-10 admin
    watch starts afresh on the new pool.
- Per-market metrics (coverage seconds, observations, exits, settlements, CU, lamports, misses,
  bursts, gaps, venue admin changes, pool proposals / executions / switches, migrations,
  straddles, degraded) and
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
RPC path is built; T-SIZE shows a tip transfer fits the worst Raydium exit; the straddle's bundle
is built, duty 6); Yellowstone gRPC subscriptions (the keeper polls `getMultipleAccounts` /
`getProgramAccounts`, and watches admin actions through the accounts they write rather than the
admin's transactions; it sees other parties' pool selection through the market's state, not their
transactions); M6's PumpSwap venue — the straddle's tx3, the `→ PumpSwap` transition's duties
(`trader_base_ata`, the PumpSwap UVA, `extend_account`), `claim_venue_rewards` —; selling the
tokens a straddle leaves the keeper; duties 11 and 13 (`crank_sol_usd`, lender pot).

### Run

```sh
cargo build --manifest-path services/Cargo.toml -p shortwin-keeper --release
services/target/release/shortwin-keeper --rpc https://… --keypair keeper.json \
  --config services/keeper/keeper.example.toml --metrics 127.0.0.1:9464 \
  [--discovery-rpc https://…]   # getProgramAccounts for the pool discovery (default: --rpc)
  [--block-engine https://mainnet.block-engine.jito.wtf]   # the straddle's bundles (duty 6)
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
`SHORTWIN_SO`) with the committed mainnet fixtures (`tests/fixtures`) for the Raydium and pump
ones; `graduation` also loads the straddle helper (`target/deploy/complete_curve.so`, or
`COMPLETE_CURVE_SO`). Every keeper transaction — each transaction of a bundle too — runs the
harness's invariant hooks after every instruction; a bundle executes in order and all-or-nothing
(the in-process backend restores the VM when one of its transactions fails).

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

Pump-curve scenarios (`graduation.rs`, M5 duty 6; the dumped pump, pump-fees and PumpSwap programs
and the near-complete curve): an outside buy completes a covered market's curve and the keeper
migrates it at its next tick (its `migrate_v2` is the harness's own derivation) and observes the
market in the same second (`→ PumpMigrating`), once; an expired position with `Q > rT` waits for
M6's exit transaction by default (one alert, `sol_settle` each second, the curve untouched); with
`send_without_exit` the two-transaction bundle lands atomically (the keeper holds the curve's last
tokens and paid the tip; tx2 is 2 + 51 + 1 = 54 trace entries) and the position's exits pause
(`VenueMigrating`); ★ a curve buy between the keeper's pre-flight and its bundle does not break
it, and a sale that lifts the completion above `max_sol_cost` reverts the whole bundle (pump
6002, nothing committed) before the next second's bundle lands; without a block engine the
straddle waits; below the straddle (`Q ≤ rT`) the keeper expires a curve position on the curve
with the adapter's own account lists; a curve migrated elsewhere with an expired position open goes
`→ None` at the keeper's first crank, which keeps the S3 chain on the no-route cadence and settles
the position in S3 exactly when the chain has run `s3_wait_s` (on the detection set); a list-only
market whose curve becomes readable is cranked onto it, its trader accounts are created once, and
an open lands.

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
  the too-thin reversed pool passed over), a pump coin's list-only market never moved onto a
  Raydium pool (D1; since M5 the keeper cranks it onto its live curve instead); an
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
  observed is warmed up in the 8 s after it, the open refused `EmaWarmingUp` until then); FIT-3
  (#100, a disclosed residual): a position opened at the incumbent's size cap and closed at once
  leaves `max_q_hwm` above a 40 % sibling's cap while two fitting positions stay open — at the
  halt (900 s before their deadline) the keeper's plan is blocked `CarriedPositionsTooLarge`, it
  proposes nothing, and it settles both in S3 at fair value at their due second
  `max(deadline, no_route_since + s3_wait_s)` (lenders `FV` in full).
- `freeze` (M4F-1, M4G-1, M4G-3 after "switch only if all fit"; the M4X-2 shape: a 2,500-SOL λ 50 %
  AMM v4 incumbent halts with a position open, a 320-SOL λ 99.78 % CPMM sibling is the candidate):
  a position at the incumbent's size cap (≈ 3.1 % of the sibling's `X`, above its cap) is **not**
  adopted — the keeper's plan reports `CarriedPositionsTooLarge` and it proposes nothing (neither
  the CPMM sibling nor a 400-SOL AMM v4 one), a direct proposal is refused before and after an
  LP's held 7× deposit in the sibling, and the keeper settles the position in S3 at fair value at
  its deadline (lenders `FV` in full); a position that fits the sibling (95 % of its cap) is
  adopted and, under an attacker's `[deposit 7·LP, crank_price, withdraw]` after the keeper in
  every second, then a genuine 4× ramp: from the adoption the keeper observes every second for as
  long as the guard is set (the lows stay the pool's depth); from +40 s, after the honest guard
  clear, nothing starts a per-slot burst but the foreign liquidity keeps the keeper per second from
  the first wrap (FITS-2 (b)); with keeper misses at +60 / +80 / +100 s (M4G-1) and with a real
  deposit held 600 s and pulled at the pump (M4G-3) too — every run is liquidated at the
  control's second relative to the ramp (the pushes only delay; here by 0 s). #100: at a **1 %**
  sibling's fit limit taken at its own fee (`q` at 76 % of its impact cap, 1.87 % of `X`) the same
  held deposit, freeze and misses with a 4×/30 s ramp ended in S1 (FIT-2); the carried fit's
  fee-scaled caps take `min(min_open_fee_ppm, φ_cand, CARRIED_FEE_CAP_PPM)`, so that position is
  **not adopted** (the plan blocked, a direct proposal refused, no window) and settles in S3 at
  fair value at its due second, and a position at the new fit limit (95 % of the 25-bps class,
  0.75 % of `X₀`) is adopted and liquidated under every push (`fit2_*`; knobs `FIT_FEE`,
  `FIT_PM`, `FIT_RAMP`, `FIT_W`, `FIT_K`, `FIT_ATTACK`, `FIT_SLACK`); FITS-1: a **1 % → 1 %**
  switch (both pools on one 1 % config) at 60 % of the old fit limit (1.48 % of `X`, S1 under the
  pushes before the 25-bps ceiling) is not adopted and settles in S3 at fair value, and one at
  95 % of the 25-bps carried cap is liquidated under every push (`fits1_*`; `FITS_PM`); a position
  carried through a dump-wrapped proposal (FIT-1) is liquidated under them too (`fit1_*`);
  FITV-2 / FITS-2, the withdraw contest (an unlocked LP of 100 % of the counter on a 160-SOL
  λ 99.78 % 1 % pool, λ ≈ 49.9 %): the program-level sweep — around the honest threshold a
  `[withdraw all, liquidate, deposit]` is accepted only where the honest `liquidate` is (refused
  `NotLiquidatable` in the retreat's band; on a build with the spot leg at `L_live` it was
  accepted there), and the I18 spot-leg hook fires on a mutated event; the verifier's
  small-position contest (500 ‰ of the cap, 2.025×) and 25-minute drag (the cap, 1,500 s, 2.05×),
  each in the reproducer's timing and the adversarial one (the attacker after the keeper, from a
  second its idle cadence skips): no forced liquidation, the keeper per second through the drag at
  one crank per wrapped crank (1,859 for 1,859 over the 1,500-s drag); the residual, pinned: in
  the adversarial timing the first wrap's second is the attacker's (`L_ema` 0.917 × `√(X·Y)`,
  then clamped), which leaves a band of ≈ 0.1 % of price under the honest threshold (forced at a
  2.038× top, the honest keeper liquidating at 2.040×); a genuine withdrawal of the LP's half of
  the depth 10 s before, at and 40 s into a pump, and 90 s into a flat 2.03× hold, is liquidated at
  the old rule's second (0 s delay at 500 ‰: the EMA leg's own lag binds; at the cap the exit
  bound, not the spot leg, waits: `L_b`'s `L_slow` credit) (`fits2_*`, `fitv2_*`; knobs `FITV_*`,
  `FITS_*`); FITX-1 (ECON #102), the pinned residual of those drags: the coverage holds `L_ema`
  but cannot lift it, so an LP that first withdraws for real for 20 s (nothing foreign: the keeper
  reads the same depth), re-deposits and then withdraw-wraps a crank every second forces the
  500 ‰ position at 2.025× (+71 s), the 25-bps position at its open cap at 2.010× (+76 s) and the
  cap after a 25-minute freeze at 2.05× (+67 s), where the honest keeper leaves them open — each
  exactly the liquidation the honest keeper makes when the same LP withdraws for good at +1 s (the
  same second, price and buyback cost) (`fitx1_*`; knobs `FITX_HELD`, `FITX_GONE`); and M4G-2 /
  FITS-2 (b), the keeper's cost: one `[deposit 5 %, crank_price,
  withdraw]` buys 8 per-second keeper observations (≤ 10), one every 59 s for an hour 7.9 each
  (603 cranks against the quiet hour's 120; the example 0.03 SOL budget unspent), none on a market
  without positions, and no per-slot burst, while the price-wrapped control still starts bursts
  (`m4g2_*`).

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
